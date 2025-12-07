//! EntityGateway-backed ArgResolver
//!
//! This module provides a production ArgResolver implementation that
//! uses the EntityGateway gRPC service for all entity and reference data lookups.

use crate::dsl_v2::assembler::ArgResolver;
use crate::dsl_v2::intent::ResolvedArg;

use entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient;
use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};
use tonic::transport::Channel;

/// ArgResolver backed by EntityGateway gRPC service
pub struct EntityGatewayResolver {
    client: EntityGatewayClient<Channel>,
}

impl EntityGatewayResolver {
    /// Create a new resolver connected to the EntityGateway
    pub async fn connect(addr: &str) -> Result<Self, tonic::transport::Error> {
        let client = EntityGatewayClient::connect(addr.to_string()).await?;
        Ok(Self { client })
    }

    /// Create with default address from environment
    pub async fn from_env() -> Result<Self, tonic::transport::Error> {
        let addr = std::env::var("ENTITY_GATEWAY_URL")
            .unwrap_or_else(|_| "http://[::1]:50051".to_string());
        Self::connect(&addr).await
    }
}

impl ArgResolver for EntityGatewayResolver {
    fn resolve_entity(&self, search: &str, entity_type: &str) -> Result<ResolvedArg, String> {
        // We need to call async from sync - use a runtime handle
        let rt = tokio::runtime::Handle::try_current().map_err(|_| "No tokio runtime available")?;

        let mut client = self.client.clone();
        let search = search.to_string();
        let entity_type = entity_type.to_string();

        rt.block_on(async move {
            let nickname = entity_type.to_uppercase();
            let request = tonic::Request::new(SearchRequest {
                nickname: nickname.clone(),
                values: vec![search.clone()],
                search_key: None,
                mode: SearchMode::Fuzzy as i32,
                limit: Some(1),
            });

            let response = client
                .search(request)
                .await
                .map_err(|e| format!("EntityGateway search failed: {}", e))?;

            let matches = response.into_inner().matches;

            if matches.is_empty() {
                return Err(format!("No {} found matching '{}'", entity_type, search));
            }

            let best = &matches[0];

            // Determine if this needs quotes based on the token format
            // UUIDs need quotes, codes like DIRECTOR don't
            let needs_quotes = needs_quoting(&best.token);

            Ok(ResolvedArg {
                value: best.token.clone(),
                is_symbol_ref: false,
                needs_quotes,
                display: Some(best.display.clone()),
            })
        })
    }

    fn resolve_ref_data(&self, search: &str, ref_type: &str) -> Result<ResolvedArg, String> {
        // Reference data uses the same EntityGateway, just different nickname
        self.resolve_entity(search, ref_type)
    }
}

/// Async version of the resolver for use in async contexts
pub struct AsyncEntityGatewayResolver {
    client: EntityGatewayClient<Channel>,
}

impl AsyncEntityGatewayResolver {
    /// Create a new async resolver
    pub async fn connect(addr: &str) -> Result<Self, tonic::transport::Error> {
        let client = EntityGatewayClient::connect(addr.to_string()).await?;
        Ok(Self { client })
    }

    /// Create with default address from environment
    pub async fn from_env() -> Result<Self, tonic::transport::Error> {
        let addr = std::env::var("ENTITY_GATEWAY_URL")
            .unwrap_or_else(|_| "http://[::1]:50051".to_string());
        Self::connect(&addr).await
    }

    /// Resolve an entity asynchronously
    pub async fn resolve_entity(
        &mut self,
        search: &str,
        entity_type: &str,
    ) -> Result<ResolvedArg, String> {
        let nickname = entity_type.to_uppercase();
        let request = tonic::Request::new(SearchRequest {
            nickname: nickname.clone(),
            values: vec![search.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(1),
        });

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        let matches = response.into_inner().matches;

        if matches.is_empty() {
            return Err(format!("No {} found matching '{}'", entity_type, search));
        }

        let best = &matches[0];
        let needs_quotes = needs_quoting(&best.token);

        Ok(ResolvedArg {
            value: best.token.clone(),
            is_symbol_ref: false,
            needs_quotes,
            display: Some(best.display.clone()),
        })
    }

    /// Resolve reference data asynchronously
    pub async fn resolve_ref_data(
        &mut self,
        search: &str,
        ref_type: &str,
    ) -> Result<ResolvedArg, String> {
        self.resolve_entity(search, ref_type).await
    }

    /// Batch resolve multiple arguments
    pub async fn resolve_batch(
        &mut self,
        lookups: Vec<(String, String, String)>,
    ) -> Vec<Result<(String, ResolvedArg), String>> {
        let mut results = Vec::new();

        for (arg_name, search, entity_type) in lookups {
            let result = self
                .resolve_entity(&search, &entity_type)
                .await
                .map(|resolved| (arg_name, resolved));
            results.push(result);
        }

        results
    }
}

/// Determine if a value needs to be quoted in DSL output
///
/// In the DSL parser, ALL string values must be quoted. The only unquoted
/// values are:
/// - Numbers (123, 45.67, -10)
/// - Booleans (true, false)
/// - Null (nil)
/// - Symbol references (@name) - handled separately
///
/// This function returns true for anything that isn't a pure number/boolean/nil.
pub fn needs_quoting(value: &str) -> bool {
    // Check for boolean literals
    if value == "true" || value == "false" {
        return false;
    }

    // Check for null
    if value == "nil" {
        return false;
    }

    // Check if it's a pure number (integer or decimal, possibly negative)
    // Must start with digit or minus followed by digit
    if let Some(first) = value.chars().next() {
        if first.is_ascii_digit() || (first == '-' && value.len() > 1) {
            if value.parse::<f64>().is_ok() || value.parse::<i64>().is_ok() {
                return false;
            }
        }
    }

    // Everything else needs quotes (including codes like "LU", "DIRECTOR", UUIDs)
    true
}

/// Check if a string looks like a UUID (kept for reference but not used)
#[allow(dead_code)]
fn is_uuid_like(value: &str) -> bool {
    // UUID format: 8-4-4-4-12 (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
    if value.len() != 36 {
        return false;
    }

    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 5 {
        return false;
    }

    // Check each part has correct length and is hex
    let expected_lens = [8, 4, 4, 4, 12];
    for (part, expected) in parts.iter().zip(expected_lens.iter()) {
        if part.len() != *expected {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_quoting_uuid() {
        // UUIDs need quoting (they're strings)
        assert!(needs_quoting("11111111-1111-1111-1111-111111111111"));
        assert!(needs_quoting("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn test_needs_quoting_codes() {
        // Codes like DIRECTOR, LU need quoting - DSL parser only accepts quoted strings
        assert!(needs_quoting("DIRECTOR"));
        assert!(needs_quoting("LU"));
        assert!(needs_quoting("FUND"));
        assert!(needs_quoting("NEW_CLIENT"));
    }

    #[test]
    fn test_needs_quoting_spaces() {
        assert!(needs_quoting("John Smith"));
        assert!(needs_quoting("Apex Capital Fund"));
    }

    #[test]
    fn test_needs_quoting_numbers() {
        // Numbers don't need quoting
        assert!(!needs_quoting("123"));
        assert!(!needs_quoting("45.67"));
        assert!(!needs_quoting("-10"));
        assert!(!needs_quoting("0"));
    }

    #[test]
    fn test_needs_quoting_booleans() {
        // Booleans don't need quoting
        assert!(!needs_quoting("true"));
        assert!(!needs_quoting("false"));
    }

    #[test]
    fn test_needs_quoting_nil() {
        // nil doesn't need quoting
        assert!(!needs_quoting("nil"));
    }
}

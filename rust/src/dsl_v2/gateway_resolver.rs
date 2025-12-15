//! Gateway-backed Reference Resolver
//!
//! This module provides the same interface as `ref_resolver.rs` but uses the
//! EntityGateway gRPC service instead of direct SQL queries. This ensures
//! that validation uses the exact same search logic as LSP autocomplete.
//!
//! ## Benefits
//! - Single source of truth for entity lookups (EntityGateway)
//! - Consistent fuzzy matching behavior between LSP and validator
//! - No reconciliation risk between different search implementations

use crate::dsl_v2::ref_resolver::{RefResolver, ResolveResult, SuggestedMatch};
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticCode, RefType, Severity, SourceSpan, Suggestion,
};
use async_trait::async_trait;
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
use tonic::transport::Channel;
use uuid::Uuid;

/// Default EntityGateway address
pub const DEFAULT_GATEWAY_ADDR: &str = "http://[::1]:50051";

/// Get gateway address from environment or use default
pub fn gateway_addr() -> String {
    std::env::var("ENTITY_GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY_ADDR.to_string())
}

/// Gateway-backed reference resolver
///
/// Uses EntityGateway gRPC service for all lookups, ensuring consistent
/// behavior with LSP autocomplete.
pub struct GatewayRefResolver {
    client: EntityGatewayClient<Channel>,
}

impl GatewayRefResolver {
    /// Connect to the EntityGateway service
    pub async fn connect(addr: &str) -> Result<Self, String> {
        let client = EntityGatewayClient::connect(addr.to_string())
            .await
            .map_err(|e| format!("Failed to connect to EntityGateway at {}: {}", addr, e))?;
        Ok(Self { client })
    }

    /// Connect using default address from environment
    pub async fn connect_default() -> Result<Self, String> {
        Self::connect(&gateway_addr()).await
    }

    /// Resolve a reference by type
    pub async fn resolve(
        &mut self,
        ref_type: RefType,
        value: &str,
    ) -> Result<ResolveResult, String> {
        let nickname = ref_type_to_nickname(ref_type);

        // For UUID-based lookups, we search by ID directly
        let (search_value, search_key) = if is_uuid_lookup(ref_type) {
            // Validate UUID format first
            if Uuid::parse_str(value).is_err() {
                return Ok(ResolveResult::NotFound {
                    suggestions: vec![],
                });
            }
            (value.to_string(), Some("id".to_string()))
        } else {
            (value.to_string(), None)
        };

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![search_value],
            search_key,
            mode: SearchMode::Exact as i32,
            limit: Some(5), // Get suggestions if not found
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        let matches: Vec<_> = response.into_inner().matches;

        if matches.is_empty() {
            // No matches - return not found with suggestions
            return Ok(ResolveResult::NotFound {
                suggestions: vec![],
            });
        }

        // Check for exact match (case-insensitive)
        let value_upper = value.to_uppercase();
        for m in &matches {
            // Check if token or display matches exactly
            if m.token.to_uppercase() == value_upper || m.display.to_uppercase() == value_upper {
                // Parse token as UUID if applicable
                if let Ok(uuid) = Uuid::parse_str(&m.token) {
                    return Ok(ResolveResult::Found {
                        id: uuid,
                        display: m.display.clone(),
                    });
                } else {
                    // Token is a code (e.g., jurisdiction code, role name)
                    return Ok(ResolveResult::FoundByCode {
                        code: m.token.clone(),
                        uuid: None,
                        display: m.display.clone(),
                    });
                }
            }
        }

        // No exact match - return suggestions from fuzzy matches
        let suggestions: Vec<SuggestedMatch> = matches
            .into_iter()
            .map(|m| SuggestedMatch {
                value: m.token,
                display: m.display,
                score: m.score,
            })
            .collect();

        Ok(ResolveResult::NotFound { suggestions })
    }

    /// Batch resolve multiple values of the same type in one gRPC call
    ///
    /// This is significantly more efficient than calling `resolve` in a loop:
    /// - 30 EntityRefs with 5 types = 5 gRPC calls instead of 30
    /// - ~6x performance improvement
    pub async fn batch_resolve(
        &mut self,
        ref_type: RefType,
        values: &[String],
    ) -> Result<std::collections::HashMap<String, ResolveResult>, String> {
        use std::collections::HashMap;

        if values.is_empty() {
            return Ok(HashMap::new());
        }

        let nickname = ref_type_to_nickname(ref_type);

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: values.to_vec(),
            search_key: None,
            mode: SearchMode::Exact as i32,
            limit: None, // Return all matches
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway batch search failed: {}", e))?;

        // Build result map: input value → ResolveResult
        let mut results = HashMap::new();
        let matches = response.into_inner().matches;

        // Index matches by their input value for quick lookup (case-insensitive)
        let match_index: HashMap<String, _> = matches
            .into_iter()
            .map(|m| (m.input.to_uppercase(), m))
            .collect();

        for value in values {
            let value_upper = value.to_uppercase();
            if let Some(m) = match_index.get(&value_upper) {
                if let Ok(uuid) = Uuid::parse_str(&m.token) {
                    results.insert(
                        value.clone(),
                        ResolveResult::Found {
                            id: uuid,
                            display: m.display.clone(),
                        },
                    );
                } else {
                    results.insert(
                        value.clone(),
                        ResolveResult::FoundByCode {
                            code: m.token.clone(),
                            uuid: None,
                            display: m.display.clone(),
                        },
                    );
                }
            } else {
                results.insert(
                    value.clone(),
                    ResolveResult::NotFound {
                        suggestions: vec![],
                    },
                );
            }
        }

        Ok(results)
    }

    /// Search with fuzzy matching (for autocomplete-like scenarios)
    pub async fn search_fuzzy(
        &mut self,
        ref_type: RefType,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<SuggestedMatch>, String> {
        let nickname = ref_type_to_nickname(ref_type);

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![prefix.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit as i32),
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        let matches = response
            .into_inner()
            .matches
            .into_iter()
            .map(|m| SuggestedMatch {
                value: m.token,
                display: m.display,
                score: m.score,
            })
            .collect();

        Ok(matches)
    }

    /// Create a diagnostic for a failed resolution
    pub fn diagnostic_for_failure(
        &self,
        ref_type: RefType,
        value: &str,
        span: SourceSpan,
        result: &ResolveResult,
    ) -> Diagnostic {
        let ResolveResult::NotFound { suggestions } = result else {
            // This is a programming error - caller should only pass NotFound results
            debug_assert!(
                false,
                "diagnostic_for_failure called with non-failure result"
            );
            // Return a generic error diagnostic as fallback
            return Diagnostic {
                severity: Severity::Error,
                span,
                code: DiagnosticCode::InvalidValue,
                message: format!("Resolution failed for '{}' (unexpected result type)", value),
                suggestions: Vec::new(),
            };
        };

        let (code, type_name) = match ref_type {
            RefType::DocumentType => (DiagnosticCode::UnknownDocumentType, "document type"),
            RefType::Jurisdiction => (DiagnosticCode::UnknownJurisdiction, "jurisdiction"),
            RefType::Role => (DiagnosticCode::UnknownRole, "role"),
            RefType::EntityType => (DiagnosticCode::UnknownEntityType, "entity type"),
            RefType::AttributeId => (DiagnosticCode::UnknownAttributeId, "attribute"),
            RefType::Cbu => (DiagnosticCode::CbuNotFound, "CBU"),
            RefType::Entity => (DiagnosticCode::EntityNotFound, "entity"),
            RefType::Document => (DiagnosticCode::DocumentNotFound, "document"),
            RefType::ScreeningType => (DiagnosticCode::InvalidValue, "screening type"),
            RefType::Product => (DiagnosticCode::InvalidValue, "product"),
            RefType::Service => (DiagnosticCode::InvalidValue, "service"),
            RefType::Currency => (DiagnosticCode::InvalidValue, "currency"),
            RefType::ClientType => (DiagnosticCode::InvalidValue, "client type"),
        };

        Diagnostic {
            severity: Severity::Error,
            span,
            code,
            message: format!("unknown {} '{}'", type_name, value),
            suggestions: suggestions
                .iter()
                .take(3)
                .map(|s| Suggestion::new("did you mean", s.value.clone(), s.score))
                .collect(),
        }
    }
}

// Implement the RefResolver trait for GatewayRefResolver
#[async_trait]
impl RefResolver for GatewayRefResolver {
    async fn resolve(&mut self, ref_type: RefType, value: &str) -> Result<ResolveResult, String> {
        GatewayRefResolver::resolve(self, ref_type, value).await
    }

    fn diagnostic_for_failure(
        &self,
        ref_type: RefType,
        value: &str,
        span: SourceSpan,
        result: &ResolveResult,
    ) -> Diagnostic {
        GatewayRefResolver::diagnostic_for_failure(self, ref_type, value, span, result)
    }

    // GatewayRefResolver doesn't need caching - EntityGateway handles it
    fn clear_cache(&mut self) {}

    fn as_gateway_resolver(&mut self) -> Option<&mut GatewayRefResolver> {
        Some(self)
    }
}

/// Map RefType to EntityGateway nickname
/// NOTE: Nicknames are UPPERCASE to match entity_index.yaml nickname fields
fn ref_type_to_nickname(ref_type: RefType) -> &'static str {
    match ref_type {
        RefType::DocumentType => "DOCUMENT_TYPE",
        RefType::Jurisdiction => "JURISDICTION",
        RefType::Role => "ROLE",
        RefType::EntityType => "ENTITY_TYPE",
        RefType::AttributeId => "ATTRIBUTE",
        RefType::Cbu => "CBU",
        RefType::Entity => "ENTITY",
        RefType::Document => "DOCUMENT",
        RefType::ScreeningType => "SCREENING_TYPE",
        RefType::Product => "PRODUCT",
        RefType::Service => "SERVICE",
        RefType::Currency => "CURRENCY",
        RefType::ClientType => "CLIENT_TYPE",
    }
}

/// Check if this ref type uses UUID-based lookup
fn is_uuid_lookup(ref_type: RefType) -> bool {
    matches!(ref_type, RefType::Cbu | RefType::Entity | RefType::Document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_type_to_nickname() {
        assert_eq!(ref_type_to_nickname(RefType::DocumentType), "DOCUMENT_TYPE");
        assert_eq!(ref_type_to_nickname(RefType::Jurisdiction), "JURISDICTION");
        assert_eq!(ref_type_to_nickname(RefType::Role), "ROLE");
        assert_eq!(ref_type_to_nickname(RefType::EntityType), "ENTITY_TYPE");
        assert_eq!(ref_type_to_nickname(RefType::AttributeId), "ATTRIBUTE");
        assert_eq!(ref_type_to_nickname(RefType::Cbu), "CBU");
        assert_eq!(ref_type_to_nickname(RefType::Entity), "ENTITY");
        assert_eq!(ref_type_to_nickname(RefType::Document), "DOCUMENT");
        assert_eq!(
            ref_type_to_nickname(RefType::ScreeningType),
            "SCREENING_TYPE"
        );
        assert_eq!(ref_type_to_nickname(RefType::Product), "PRODUCT");
        assert_eq!(ref_type_to_nickname(RefType::Service), "SERVICE");
        assert_eq!(ref_type_to_nickname(RefType::Currency), "CURRENCY");
        assert_eq!(ref_type_to_nickname(RefType::ClientType), "CLIENT_TYPE");
    }

    #[test]
    fn test_is_uuid_lookup() {
        assert!(is_uuid_lookup(RefType::Cbu));
        assert!(is_uuid_lookup(RefType::Entity));
        assert!(is_uuid_lookup(RefType::Document));
        assert!(!is_uuid_lookup(RefType::Role));
        assert!(!is_uuid_lookup(RefType::Jurisdiction));
    }

    #[test]
    fn test_gateway_addr_default() {
        // Without ENTITY_GATEWAY_URL set, should return default
        std::env::remove_var("ENTITY_GATEWAY_URL");
        assert_eq!(gateway_addr(), DEFAULT_GATEWAY_ADDR);
    }
}

//! Database-backed entity resolver using EntityGateway gRPC.
//!
//! This resolver looks up entity names in the database via the EntityGateway
//! service, which provides fuzzy matching and returns resolved UUIDs.

use std::sync::Arc;

use async_trait::async_trait;
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tracing::{debug, warn};

use super::tokenizer::{EntityResolver, ResolvedEntity};

/// Default EntityGateway address.
pub const DEFAULT_GATEWAY_ADDR: &str = "http://[::1]:50051";

/// Get gateway address from environment or use default.
fn gateway_addr() -> String {
    std::env::var("ENTITY_GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY_ADDR.to_string())
}

/// Entity resolver that uses EntityGateway gRPC service.
pub struct DatabaseEntityResolver {
    /// The gRPC client (wrapped in Mutex for interior mutability).
    client: Arc<Mutex<EntityGatewayClient<Channel>>>,
}

impl DatabaseEntityResolver {
    /// Create a new resolver by connecting to EntityGateway.
    pub async fn connect(endpoint: &str) -> Result<Self, tonic::transport::Error> {
        let client = EntityGatewayClient::connect(endpoint.to_string()).await?;
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    /// Create from environment variable ENTITY_GATEWAY_URL.
    pub async fn from_env() -> Result<Self, tonic::transport::Error> {
        Self::connect(&gateway_addr()).await
    }

    /// Map entity type hint to EntityGateway nickname.
    fn type_to_nickname(entity_type: &str) -> &'static str {
        match entity_type {
            "cbu" => "cbu",
            "proper_person" | "person" => "person",
            "limited_company" | "legal_entity" | "company" => "legal_entity",
            "counterparty" => "legal_entity", // Counterparties are legal entities
            "entity" => "entity",
            "product" => "product",
            "service" => "service",
            "role" => "role",
            "jurisdiction" => "jurisdiction",
            "currency" => "currency",
            "market" => "market",
            "instrument_class" => "instrument_class",
            _ => "entity", // Default fallback
        }
    }
}

#[async_trait]
impl EntityResolver for DatabaseEntityResolver {
    async fn resolve(&self, text: &str, hint: Option<&str>) -> Option<ResolvedEntity> {
        let entity_type = hint.unwrap_or("entity");
        let nickname = Self::type_to_nickname(entity_type);

        debug!(text = %text, nickname = %nickname, "Resolving entity via gateway");

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![text.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(1),
        };

        let mut client = self.client.lock().await;
        match client.search(request).await {
            Ok(response) => {
                let matches = response.into_inner().matches;
                if let Some(m) = matches.into_iter().next() {
                    let confidence = m.score;

                    debug!(
                        text = %text,
                        resolved_id = %m.token,
                        display = %m.display,
                        confidence = %confidence,
                        "Entity resolved"
                    );

                    return Some(ResolvedEntity {
                        id: m.token,
                        name: m.display,
                        entity_type: entity_type.to_string(),
                        confidence,
                    });
                }
                debug!(text = %text, "No entity match found");
                None
            }
            Err(e) => {
                warn!(text = %text, error = %e, "Entity gateway search failed");
                None
            }
        }
    }

    async fn resolve_batch(
        &self,
        texts: &[&str],
        hint: Option<&str>,
    ) -> Vec<Option<ResolvedEntity>> {
        let entity_type = hint.unwrap_or("entity");
        let nickname = Self::type_to_nickname(entity_type);

        debug!(
            count = texts.len(),
            nickname = %nickname,
            "Batch resolving entities"
        );

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: texts.iter().map(|s| s.to_string()).collect(),
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(1),
        };

        let mut client = self.client.lock().await;
        match client.search(request).await {
            Ok(response) => {
                let matches = response.into_inner().matches;

                // Build a map from search value to result
                let mut results: Vec<Option<ResolvedEntity>> = vec![None; texts.len()];

                for m in matches {
                    // Find the matching input text
                    for (i, &text) in texts.iter().enumerate() {
                        if text.to_lowercase() == m.display.to_lowercase()
                            || text.to_lowercase().contains(&m.display.to_lowercase())
                        {
                            results[i] = Some(ResolvedEntity {
                                id: m.token.clone(),
                                name: m.display.clone(),
                                entity_type: entity_type.to_string(),
                                confidence: m.score,
                            });
                            break;
                        }
                    }
                }

                results
            }
            Err(e) => {
                warn!(error = %e, "Batch entity resolution failed");
                vec![None; texts.len()]
            }
        }
    }
}

/// A composite resolver that tries multiple entity types.
pub struct CompositeEntityResolver {
    /// The underlying database resolver.
    db_resolver: DatabaseEntityResolver,

    /// Entity types to try, in order of priority.
    entity_types: Vec<String>,
}

impl CompositeEntityResolver {
    /// Create a new composite resolver.
    pub fn new(db_resolver: DatabaseEntityResolver, entity_types: Vec<String>) -> Self {
        Self {
            db_resolver,
            entity_types,
        }
    }

    /// Create with default entity types for the trading domain.
    pub fn with_trading_types(db_resolver: DatabaseEntityResolver) -> Self {
        Self {
            db_resolver,
            entity_types: vec![
                "counterparty".to_string(),
                "cbu".to_string(),
                "proper_person".to_string(),
                "limited_company".to_string(),
                "entity".to_string(),
            ],
        }
    }
}

#[async_trait]
impl EntityResolver for CompositeEntityResolver {
    async fn resolve(&self, text: &str, hint: Option<&str>) -> Option<ResolvedEntity> {
        // If hint is provided, use it directly
        if let Some(h) = hint {
            return self.db_resolver.resolve(text, Some(h)).await;
        }

        // Otherwise, try each entity type in order
        for entity_type in &self.entity_types {
            if let Some(resolved) = self.db_resolver.resolve(text, Some(entity_type)).await {
                if resolved.confidence >= 0.8 {
                    return Some(resolved);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    // Integration tests require running EntityGateway service
    // See tests/integration/entity_resolver_test.rs
}

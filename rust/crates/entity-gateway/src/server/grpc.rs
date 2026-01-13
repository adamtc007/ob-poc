//! gRPC service implementation for EntityGateway
//!
//! This module implements the EntityGateway gRPC service as defined
//! in the proto contract.

use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::index::{IndexRegistry, MatchMode, SearchQuery};
use crate::proto::ob::gateway::v1::{
    entity_gateway_server::EntityGateway, Match, SearchMode, SearchRequest, SearchResponse,
};

/// gRPC service implementation
pub struct EntityGatewayService {
    registry: Arc<IndexRegistry>,
}

impl EntityGatewayService {
    /// Create a new service with the given registry
    pub fn new(registry: Arc<IndexRegistry>) -> Self {
        Self { registry }
    }
}

#[tonic::async_trait]
impl EntityGateway for EntityGatewayService {
    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        let req = request.into_inner();

        // Validate nickname
        let entity_config = self
            .registry
            .get_config(&req.nickname)
            .ok_or_else(|| Status::not_found(format!("Unknown entity type: {}", req.nickname)))?;

        // Get index
        let index =
            self.registry.get(&req.nickname).await.ok_or_else(|| {
                Status::unavailable(format!("Index not ready for: {}", req.nickname))
            })?;

        // Check if index is ready
        if !index.is_ready() {
            return Err(Status::unavailable(format!(
                "Index not ready for: {}",
                req.nickname
            )));
        }

        // Resolve search key (use default if not specified)
        let search_key = req
            .search_key
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| entity_config.default_search_key().name.clone());

        // Validate search key exists
        if entity_config.get_search_key(&search_key).is_none() {
            return Err(Status::invalid_argument(format!(
                "Unknown search_key '{}' for entity '{}'. Valid keys: {:?}",
                search_key,
                req.nickname,
                entity_config
                    .search_keys
                    .iter()
                    .map(|k| &k.name)
                    .collect::<Vec<_>>()
            )));
        }

        // Validate values
        if req.values.is_empty() {
            return Err(Status::invalid_argument(
                "At least one search value required",
            ));
        }

        // Convert mode
        let mode = match SearchMode::try_from(req.mode).unwrap_or(SearchMode::Fuzzy) {
            SearchMode::Fuzzy => MatchMode::Fuzzy,
            SearchMode::Exact => MatchMode::Exact,
        };

        // Build query with discriminators and tenant scope from request
        let query = SearchQuery {
            values: req.values,
            search_key,
            mode,
            limit: req.limit.unwrap_or(10) as usize,
            discriminators: req.discriminators,
            tenant_id: req.tenant_id,
            cbu_id: req.cbu_id,
        };

        // Execute search
        let matches = index.search(&query).await;

        // Convert to proto response
        let response = SearchResponse {
            matches: matches
                .into_iter()
                .map(|m| Match {
                    input: m.input,
                    display: m.display,
                    token: m.token,
                    score: m.score,
                })
                .collect(),
        };

        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EntityConfig, SearchKeyConfig, ShardConfig};
    use crate::index::TantivyIndex;
    use std::collections::HashMap;

    fn sample_config() -> EntityConfig {
        EntityConfig {
            nickname: "test".to_string(),
            source_table: "test".to_string(),
            return_key: "id".to_string(),
            display_template: None,
            index_mode: crate::config::IndexMode::Trigram,
            filter: None,
            search_keys: vec![SearchKeyConfig {
                name: "name".to_string(),
                column: "name".to_string(),
                default: true,
            }],
            shard: Some(ShardConfig {
                enabled: false,
                prefix_len: 0,
            }),
            display_template_full: None,
            composite_search: None,
            discriminators: vec![],
        }
    }

    #[tokio::test]
    async fn test_unknown_nickname() {
        let registry = Arc::new(IndexRegistry::new(HashMap::new()));
        let service = EntityGatewayService::new(registry);

        let request = Request::new(SearchRequest {
            nickname: "unknown".to_string(),
            values: vec!["test".to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: None,
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        });

        let result = service.search(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_empty_values() {
        use crate::index::{IndexRecord, SearchIndex};

        let mut configs = HashMap::new();
        configs.insert("test".to_string(), sample_config());

        let registry = Arc::new(IndexRegistry::new(configs));

        // Register an index and refresh it so it's ready
        let index = TantivyIndex::new(sample_config()).unwrap();
        index
            .refresh(vec![IndexRecord {
                token: "uuid-1".to_string(),
                display: "Test".to_string(),
                search_values: std::collections::HashMap::from([(
                    "name".to_string(),
                    "test".to_string(),
                )]),
                discriminator_values: std::collections::HashMap::new(),
                tenant_id: None,
                cbu_ids: vec![],
            }])
            .await
            .unwrap();
        registry.register("test".to_string(), Arc::new(index)).await;

        let service = EntityGatewayService::new(registry);

        let request = Request::new(SearchRequest {
            nickname: "test".to_string(),
            values: vec![],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: None,
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        });

        let result = service.search(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }
}

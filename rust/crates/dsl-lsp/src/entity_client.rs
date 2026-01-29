//! Entity Gateway client for LSP autocomplete lookups.
//!
//! This module provides a client that connects to the EntityGateway gRPC
//! service for fast fuzzy entity lookups used in autocomplete.

#![allow(dead_code)] // Public API - functions may be used by LSP server

use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
use tonic::transport::Channel;

/// Result of an entity lookup
#[derive(Debug, Clone)]
pub struct EntityMatch {
    /// The input that produced this match
    pub input: String,
    /// Human-readable display name
    pub display: String,
    /// The resolved ID (UUID)
    pub id: String,
    /// Relevance score (0.0 - 1.0)
    pub score: f32,
}

/// Client for entity lookups via the EntityGateway service.
pub struct EntityLookupClient {
    client: EntityGatewayClient<Channel>,
}

impl EntityLookupClient {
    /// Connect to the EntityGateway service.
    pub async fn connect(addr: &str) -> Result<Self, tonic::transport::Error> {
        let client = EntityGatewayClient::connect(addr.to_string()).await?;
        Ok(Self { client })
    }

    /// Search for entities by nickname and prefix.
    ///
    /// # Arguments
    /// * `nickname` - Entity type (e.g., "CBU", "PERSON", "ENTITY")
    /// * `prefix` - Search prefix for fuzzy matching
    /// * `limit` - Maximum results to return
    pub async fn search(
        &mut self,
        nickname: &str,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatch>, tonic::Status> {
        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![prefix.to_string()],
            search_key: None, // Use default
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit as i32),
            discriminators: std::collections::HashMap::new(), // No discriminators for LSP completion
            tenant_id: None,                                  // LSP completion is global context
            cbu_id: None,
        };

        let response = self.client.search(request).await?;
        let matches = response
            .into_inner()
            .matches
            .into_iter()
            .map(|m| EntityMatch {
                input: m.input,
                display: m.display,
                id: m.token,
                score: m.score,
            })
            .collect();

        Ok(matches)
    }

    /// Search for CBUs by name prefix.
    pub async fn find_cbu(
        &mut self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatch>, tonic::Status> {
        self.search("CBU", prefix, limit).await
    }

    /// Search for entities (all types) by name prefix.
    pub async fn find_entity(
        &mut self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatch>, tonic::Status> {
        self.search("ENTITY", prefix, limit).await
    }

    /// Search for persons by name prefix.
    pub async fn find_person(
        &mut self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatch>, tonic::Status> {
        self.search("PERSON", prefix, limit).await
    }

    /// Search for products by name prefix.
    pub async fn find_product(
        &mut self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatch>, tonic::Status> {
        self.search("PRODUCT", prefix, limit).await
    }

    /// Search for legal entities (companies) by name prefix.
    pub async fn find_legal_entity(
        &mut self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatch>, tonic::Status> {
        self.search("LEGAL_ENTITY", prefix, limit).await
    }
}

/// Default EntityGateway address
pub const DEFAULT_GATEWAY_ADDR: &str = "http://[::1]:50051";

/// Get gateway address from environment or use default
pub fn gateway_addr() -> String {
    std::env::var("ENTITY_GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY_ADDR.to_string())
}

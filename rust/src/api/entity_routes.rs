//! Entity Search API endpoints
//!
//! Uses EntityGateway gRPC service for all entity lookups.
//! This ensures consistent fuzzy search behavior across the entire system.

use crate::dsl_v2::gateway_resolver::gateway_addr;
use axum::{extract::Query, http::StatusCode, response::Json, routing::get, Router};
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Query params for search
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query string
    pub q: String,

    /// Entity type nickname (e.g., "person", "entity", "cbu")
    #[serde(default)]
    pub entity_type: Option<String>,

    /// Max results
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

/// Search result from EntityGateway
#[derive(Debug, Clone, Serialize)]
pub struct EntityMatch {
    pub id: Option<Uuid>,
    pub token: String,
    pub display: String,
    pub score: f32,
}

/// Search response
#[derive(Debug, Clone, Serialize)]
pub struct EntitySearchResponse {
    pub results: Vec<EntityMatch>,
    pub total: u32,
}

/// GET /api/entities/search
async fn search_entities(
    Query(query): Query<SearchQuery>,
) -> Result<Json<EntitySearchResponse>, StatusCode> {
    let addr = gateway_addr();

    let mut client = EntityGatewayClient::connect(addr).await.map_err(|e| {
        eprintln!("Failed to connect to EntityGateway: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Default to "entity" if no type specified
    let nickname = query.entity_type.unwrap_or_else(|| "entity".to_string());

    let request = SearchRequest {
        nickname,
        values: vec![query.q],
        search_key: None,
        mode: SearchMode::Fuzzy as i32,
        limit: Some(query.limit.min(50) as i32),
    };

    let response = client.search(request).await.map_err(|e| {
        eprintln!("EntityGateway search error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let matches = response.into_inner().matches;
    let total = matches.len() as u32;

    let results: Vec<EntityMatch> = matches
        .into_iter()
        .map(|m| EntityMatch {
            id: Uuid::parse_str(&m.token).ok(),
            token: m.token,
            display: m.display,
            score: m.score,
        })
        .collect();

    Ok(Json(EntitySearchResponse { results, total }))
}

/// Create router for entity endpoints
pub fn create_entity_router() -> Router {
    Router::new().route("/api/entities/search", get(search_entities))
}

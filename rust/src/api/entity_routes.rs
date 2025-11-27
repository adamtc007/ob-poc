//! Entity Search API endpoints

use crate::services::{EntitySearchRequest, EntitySearchResponse, EntitySearchService, EntityType};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::Arc;

// State for entity routes
#[derive(Clone)]
pub struct EntityState {
    search_service: Arc<EntitySearchService>,
}

impl EntityState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            search_service: Arc::new(EntitySearchService::new(pool)),
        }
    }
}

// Query params for search
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query string
    pub q: String,

    /// Comma-separated entity types (e.g., "PERSON,COMPANY")
    #[serde(default)]
    pub types: Option<String>,

    /// Max results
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

/// GET /api/entities/search
async fn search_entities(
    State(state): State<EntityState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<EntitySearchResponse>, StatusCode> {
    let types: Vec<EntityType> = query
        .types
        .as_deref()
        .map(|t| {
            t.split(',')
                .filter_map(|s| EntityType::from_str(s.trim()))
                .collect()
        })
        .unwrap_or_default();

    let req = EntitySearchRequest {
        query: query.q,
        types,
        limit: query.limit.min(50),
        threshold: 0.2,
        cbu_id: None,
    };

    let response = state.search_service.search(&req).await.map_err(|e| {
        eprintln!("Search error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(response))
}

/// Create router for entity endpoints
pub fn create_entity_router(pool: PgPool) -> Router {
    let state = EntityState::new(pool);

    Router::new()
        .route("/api/entities/search", get(search_entities))
        .with_state(state)
}

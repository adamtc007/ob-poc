//! Graph API routes for CBU visualization
//!
//! Provides endpoints to fetch graph data for the egui WASM client.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::graph::{CbuGraph, CbuGraphBuilder, CbuSummary};

/// Query parameters for graph endpoint
#[derive(Deserialize)]
pub struct GraphQueryParams {
    #[serde(default = "default_true")]
    pub custody: bool,
    #[serde(default)]
    pub kyc: bool,
    #[serde(default)]
    pub ubo: bool,
    #[serde(default)]
    pub services: bool,
}

fn default_true() -> bool {
    true
}

/// GET /api/cbu/{cbu_id}/graph
/// Returns the graph data for a specific CBU
pub async fn get_cbu_graph(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<GraphQueryParams>,
) -> Result<Json<CbuGraph>, (StatusCode, String)> {
    let graph = CbuGraphBuilder::new(cbu_id)
        .with_custody(params.custody)
        .with_kyc(params.kyc)
        .with_ubo(params.ubo)
        .with_services(params.services)
        .build(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(graph))
}

/// GET /api/cbu - List all CBUs (summary)
pub async fn list_cbus(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<CbuSummary>>, (StatusCode, String)> {
    let cbus = sqlx::query_as!(
        CbuSummary,
        r#"SELECT cbu_id, name, jurisdiction, client_type,
                  created_at, updated_at
           FROM "ob-poc".cbus
           ORDER BY name"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(cbus))
}

/// GET /api/cbu/{cbu_id} - Get a single CBU summary
pub async fn get_cbu(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<CbuSummary>, (StatusCode, String)> {
    let cbu = sqlx::query_as!(
        CbuSummary,
        r#"SELECT cbu_id, name, jurisdiction, client_type,
                  created_at, updated_at
           FROM "ob-poc".cbus
           WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    Ok(Json(cbu))
}

/// Create the graph router
pub fn create_graph_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/cbu", get(list_cbus))
        .route("/api/cbu/:cbu_id", get(get_cbu))
        .route("/api/cbu/:cbu_id/graph", get(get_cbu_graph))
        .with_state(pool)
}

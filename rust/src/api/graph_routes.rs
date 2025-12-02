//! Graph API routes for CBU visualization
//!
//! Provides endpoints to fetch graph data for the egui WASM client.
//! All database access goes through VisualizationRepository.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::database::VisualizationRepository;
use crate::graph::{CbuGraph, CbuGraphBuilder, CbuSummary};
use crate::visualization::{CbuVisualization, KycTreeBuilder, ServiceTreeBuilder, ViewMode};

/// Query parameters for graph endpoint
/// All layers are enabled by default for complete CBU visualization
#[derive(Deserialize)]
pub struct GraphQueryParams {
    #[serde(default = "default_true")]
    pub custody: bool,
    #[serde(default = "default_true")]
    pub kyc: bool,
    #[serde(default = "default_true")]
    pub ubo: bool,
    #[serde(default = "default_true")]
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
    let repo = VisualizationRepository::new(pool);
    let graph = CbuGraphBuilder::new(cbu_id)
        .with_custody(params.custody)
        .with_kyc(params.kyc)
        .with_ubo(params.ubo)
        .with_services(params.services)
        .build(&repo)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(graph))
}

/// GET /api/cbu - List all CBUs (summary)
pub async fn list_cbus(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<CbuSummary>>, (StatusCode, String)> {
    let repo = VisualizationRepository::new(pool);
    let cbus = repo
        .list_cbus()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Convert to CbuSummary (same fields, different struct for API contract)
    let summaries: Vec<CbuSummary> = cbus
        .into_iter()
        .map(|c| CbuSummary {
            cbu_id: c.cbu_id,
            name: c.name,
            jurisdiction: c.jurisdiction,
            client_type: c.client_type,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect();

    Ok(Json(summaries))
}

/// GET /api/cbu/{cbu_id} - Get a single CBU summary
pub async fn get_cbu(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<CbuSummary>, (StatusCode, String)> {
    let repo = VisualizationRepository::new(pool);
    let cbu = repo
        .get_cbu(cbu_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("CBU not found: {}", cbu_id)))?;

    Ok(Json(CbuSummary {
        cbu_id: cbu.cbu_id,
        name: cbu.name,
        jurisdiction: cbu.jurisdiction,
        client_type: cbu.client_type,
        created_at: cbu.created_at,
        updated_at: cbu.updated_at,
    }))
}

/// Query parameters for tree endpoint
#[derive(Deserialize)]
pub struct TreeQueryParams {
    #[serde(default)]
    pub view: Option<String>,
}

/// GET /api/cbu/{cbu_id}/tree
/// Returns hierarchical tree visualization data for a specific CBU
pub async fn get_cbu_tree(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<TreeQueryParams>,
) -> Result<Json<CbuVisualization>, (StatusCode, String)> {
    let view_mode = match params.view.as_deref() {
        Some("service_delivery") => ViewMode::ServiceDelivery,
        _ => ViewMode::KycUbo, // Default to KYC/UBO view
    };

    let repo = VisualizationRepository::new(pool);

    let viz = match view_mode {
        ViewMode::KycUbo => KycTreeBuilder::new(repo)
            .build(cbu_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
        ViewMode::ServiceDelivery => ServiceTreeBuilder::new(repo)
            .build(cbu_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    };
    Ok(Json(viz))
}

/// Create the graph router
pub fn create_graph_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/cbu", get(list_cbus))
        .route("/api/cbu/:cbu_id", get(get_cbu))
        .route("/api/cbu/:cbu_id/graph", get(get_cbu_graph))
        .route("/api/cbu/:cbu_id/tree", get(get_cbu_tree))
        .with_state(pool)
}

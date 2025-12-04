//! Graph API routes for CBU visualization
//!
//! Provides endpoints to fetch graph data for the egui WASM client.
//! All database access goes through VisualizationRepository.
//!
//! Single endpoint: /api/cbu/:id/graph returns flat graph (nodes + edges).
//! UI owns layout/visualization logic.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::database::VisualizationRepository;
use crate::graph::{CbuGraph, CbuGraphBuilder, CbuSummary};

/// GET /api/cbu/{cbu_id}/graph
/// Returns the COMPLETE graph data for a specific CBU
/// Always loads ALL layers (Core, Custody, KYC, UBO, Services)
/// UI is responsible for filtering by view mode
pub async fn get_cbu_graph(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<CbuGraph>, (StatusCode, String)> {
    let repo = VisualizationRepository::new(pool);

    // Always load ALL layers - UI handles view mode filtering
    let graph = CbuGraphBuilder::new(cbu_id)
        .with_custody(true)
        .with_kyc(true)
        .with_ubo(true)
        .with_services(true)
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

/// Create the graph router
pub fn create_graph_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/cbu", get(list_cbus))
        .route("/api/cbu/:cbu_id", get(get_cbu))
        .route("/api/cbu/:cbu_id/graph", get(get_cbu_graph))
        .with_state(pool)
}

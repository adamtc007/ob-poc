//! Graph API routes for CBU visualization
//!
//! Provides endpoints to fetch graph data for the egui WASM client.
//! All database access goes through VisualizationRepository.
//!
//! Single endpoint: /api/cbu/:id/graph returns flat graph (nodes + edges).
//! UI owns layout/visualization logic.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::database::{LayoutOverrideView, VisualizationRepository};
use crate::graph::{
    CbuGraph, CbuGraphBuilder, CbuSummary, LayoutEngine, LayoutOverride, NodeOffset,
    NodeSizeOverride, ViewMode,
};

/// Query parameters for graph endpoint
#[derive(Debug, Deserialize)]
pub struct GraphQuery {
    /// View mode: KYC_UBO (default), SERVICE_DELIVERY, or CUSTODY
    pub view_mode: Option<String>,
}

/// GET /api/cbu/{cbu_id}/graph?view_mode=KYC_UBO
/// Returns graph data with server-computed layout positions
/// View mode determines which layers are emphasized and how nodes are arranged
pub async fn get_cbu_graph(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<GraphQuery>,
) -> Result<Json<CbuGraph>, (StatusCode, String)> {
    let repo = VisualizationRepository::new(pool);
    let view_mode = ViewMode::from_str(params.view_mode.as_deref().unwrap_or("KYC_UBO"));

    // Always load ALL layers - layout engine positions nodes based on view mode
    let mut graph = CbuGraphBuilder::new(cbu_id)
        .with_custody(true)
        .with_kyc(true)
        .with_ubo(true)
        .with_services(true)
        .build(&repo)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply server-side layout based on view mode
    let layout_engine = LayoutEngine::new(view_mode);
    layout_engine.layout(&mut graph);

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

#[derive(Debug, Deserialize)]
pub struct LayoutQuery {
    pub view_mode: Option<String>,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct LayoutSaveRequest {
    pub view_mode: Option<String>,
    pub positions: Vec<NodeOffset>,
    pub sizes: Vec<NodeSizeOverride>,
    pub user_id: Option<Uuid>,
}

fn normalize_view_mode(view_mode: Option<String>) -> String {
    view_mode
        .map(|v| v.to_uppercase())
        .unwrap_or_else(|| "KYC_UBO".to_string())
}

/// GET /api/cbu/{cbu_id}/layout - fetch saved layout overrides
pub async fn get_cbu_layout(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<LayoutQuery>,
) -> Result<Json<LayoutOverride>, (StatusCode, String)> {
    let repo = VisualizationRepository::new(pool);
    let view_mode = normalize_view_mode(params.view_mode);
    let user_id = params.user_id.unwrap_or_else(Uuid::nil);

    let view = repo
        .get_layout_override(cbu_id, user_id, &view_mode)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let overrides = match view {
        Some(v) => LayoutOverride {
            positions: v
                .positions
                .into_iter()
                .map(|p| NodeOffset {
                    node_id: p.node_id,
                    dx: p.dx,
                    dy: p.dy,
                })
                .collect(),
            sizes: v
                .sizes
                .into_iter()
                .map(|s| NodeSizeOverride {
                    node_id: s.node_id,
                    w: s.w,
                    h: s.h,
                })
                .collect(),
        },
        None => LayoutOverride {
            positions: Vec::new(),
            sizes: Vec::new(),
        },
    };

    Ok(Json(overrides))
}

/// POST /api/cbu/{cbu_id}/layout - save layout overrides
pub async fn save_cbu_layout(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Json(body): Json<LayoutSaveRequest>,
) -> Result<Json<LayoutOverride>, (StatusCode, String)> {
    let repo = VisualizationRepository::new(pool);
    let view_mode = normalize_view_mode(body.view_mode.clone());
    let user_id = body.user_id.unwrap_or_else(Uuid::nil);

    let overrides = LayoutOverride {
        positions: body.positions.clone(),
        sizes: body.sizes.clone(),
    };

    repo.upsert_layout_override(
        cbu_id,
        user_id,
        &view_mode,
        LayoutOverrideView {
            positions: overrides.positions.clone(),
            sizes: overrides.sizes.clone(),
        },
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(LayoutOverride {
        positions: overrides.positions,
        sizes: overrides.sizes,
    }))
}

/// Create the graph router
pub fn create_graph_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/cbu", get(list_cbus))
        .route("/api/cbu/:cbu_id", get(get_cbu))
        .route("/api/cbu/:cbu_id/graph", get(get_cbu_graph))
        .route("/api/cbu/:cbu_id/layout", get(get_cbu_layout))
        .route("/api/cbu/:cbu_id/layout", post(save_cbu_layout))
        .with_state(pool)
}

//! REST API endpoints for CBU and Graph data
//!
//! Session management and chat endpoints are handled by agent_routes in the main crate.
//! This module only contains CBU listing and graph visualization endpoints.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use ob_poc_types::CbuSummary;

use crate::state::AppState;

// =============================================================================
// CBU
// =============================================================================

pub async fn list_cbus(State(state): State<AppState>) -> Result<Json<Vec<CbuSummary>>, StatusCode> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        r#"SELECT cbu_id, name, jurisdiction, client_type FROM "ob-poc".cbus ORDER BY name LIMIT 100"#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cbus: Vec<CbuSummary> = rows
        .into_iter()
        .map(|(cbu_id, name, jurisdiction, client_type)| CbuSummary {
            cbu_id: cbu_id.to_string(),
            name,
            jurisdiction,
            client_type,
        })
        .collect();

    Ok(Json(cbus))
}

pub async fn get_cbu(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<CbuSummary>, StatusCode> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        r#"SELECT cbu_id, name, jurisdiction, client_type FROM "ob-poc".cbus WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(CbuSummary {
        cbu_id: row.0.to_string(),
        name: row.1,
        jurisdiction: row.2,
        client_type: row.3,
    }))
}

// =============================================================================
// GRAPH
// =============================================================================

/// Query parameters for graph endpoint
#[derive(Debug, serde::Deserialize)]
pub struct GraphQuery {
    /// View mode: KYC_UBO (default), SERVICE_DELIVERY, or CUSTODY
    pub view_mode: Option<String>,
    /// Layout orientation: VERTICAL (default) or HORIZONTAL
    pub orientation: Option<String>,
}

pub async fn get_cbu_graph(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<GraphQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use ob_poc::database::VisualizationRepository;
    use ob_poc::graph::{CbuGraphBuilder, LayoutEngine, Orientation, ViewMode};

    let repo = VisualizationRepository::new(state.pool.clone());
    let view_mode = ViewMode::parse(params.view_mode.as_deref().unwrap_or("KYC_UBO"));
    let orientation = Orientation::parse(params.orientation.as_deref().unwrap_or("VERTICAL"));

    // Build graph with all layers
    let mut graph = CbuGraphBuilder::new(cbu_id)
        .with_custody(true)
        .with_kyc(true)
        .with_ubo(true)
        .with_services(true)
        .build(&repo)
        .await
        .map_err(|e| {
            tracing::error!("Graph build error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Apply server-side layout (computes x/y positions)
    let layout_engine = LayoutEngine::with_orientation(view_mode, orientation);
    layout_engine.layout(&mut graph);

    Ok(Json(serde_json::to_value(graph).unwrap_or_default()))
}

//! REST API endpoints for CBU and Graph data
//!
//! Session management and chat endpoints are handled by agent_routes in the main crate.
//! This module only contains CBU listing and graph visualization endpoints.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use ob_poc_types::CbuSummary;

use crate::state::AppState;

/// Query parameters for CBU search
#[derive(Debug, serde::Deserialize)]
pub struct CbuSearchQuery {
    /// Search query (matches against name, case-insensitive)
    pub q: Option<String>,
    /// Maximum results to return (default 20)
    pub limit: Option<i64>,
}

// =============================================================================
// CBU
// =============================================================================

pub async fn list_cbus(State(state): State<AppState>) -> Result<Json<Vec<CbuSummary>>, StatusCode> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        r#"SELECT cbu_id, name, jurisdiction, client_type FROM "ob-poc".cbus ORDER BY name LIMIT 50"#
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

/// Search CBUs by name (case-insensitive, trigram similarity)
pub async fn search_cbus(
    State(state): State<AppState>,
    Query(params): Query<CbuSearchQuery>,
) -> Result<Json<Vec<CbuSummary>>, StatusCode> {
    let query = params.q.unwrap_or_default();
    let limit = params.limit.unwrap_or(20).min(100);

    let rows = if query.is_empty() {
        // No query - return recent/popular CBUs
        sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
            r#"SELECT cbu_id, name, jurisdiction, client_type
               FROM "ob-poc".cbus
               ORDER BY updated_at DESC NULLS LAST, name
               LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(&state.pool)
        .await
    } else {
        // Search by name using ILIKE for prefix/contains match
        // Uses pg_trgm index if available for better performance
        sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
            r#"SELECT cbu_id, name, jurisdiction, client_type
               FROM "ob-poc".cbus
               WHERE name ILIKE $1
               ORDER BY
                   CASE WHEN name ILIKE $2 THEN 0 ELSE 1 END,  -- Prefix matches first
                   name
               LIMIT $3"#,
        )
        .bind(format!("%{}%", query)) // Contains match
        .bind(format!("{}%", query)) // Prefix match (for ordering)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
    }
    .map_err(|e| {
        tracing::error!("CBU search error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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

    // Build graph filtered by view_mode - server does the filtering
    let mut graph = match view_mode {
        ViewMode::KycUbo => {
            // KYC/UBO view: entities + KYC + UBO layers, no services
            CbuGraphBuilder::new(cbu_id)
                .with_custody(false)
                .with_kyc(true)
                .with_ubo(true)
                .with_services(false)
                .build(&repo)
                .await
        }
        ViewMode::UboOnly => {
            // UBO Only view: pure ownership/control graph
            CbuGraphBuilder::new(cbu_id)
                .with_custody(false)
                .with_kyc(false)
                .with_ubo(true)
                .with_services(false)
                .with_entities(true)
                .build(&repo)
                .await
        }
        ViewMode::ServiceDelivery => {
            // Service Delivery view: products + services + resources + trading entities
            CbuGraphBuilder::new(cbu_id)
                .with_custody(false)
                .with_kyc(false)
                .with_ubo(false)
                .with_services(true)
                .with_entities(true)
                .build(&repo)
                .await
        }
        ViewMode::ProductsOnly => {
            // Products only: just CBU + products
            CbuGraphBuilder::new(cbu_id)
                .with_custody(false)
                .with_kyc(false)
                .with_ubo(false)
                .with_services(true) // Load services layer to get products
                .with_entities(false)
                .build(&repo)
                .await
        }
        ViewMode::Trading => {
            // Trading view: CBU as container with trading entities (Asset Owner, IM, ManCo, etc.)
            CbuGraphBuilder::new(cbu_id)
                .with_custody(false)
                .with_kyc(false)
                .with_ubo(false)
                .with_services(false)
                .with_entities(true) // Load entities for trading roles
                .build(&repo)
                .await
        }
    }
    .map_err(|e| {
        tracing::error!("Graph build error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Apply view-mode specific filtering after loading
    match view_mode {
        ViewMode::UboOnly => {
            graph.filter_to_ubo_only();
        }
        ViewMode::ServiceDelivery => {
            graph.filter_to_trading_entities();
        }
        ViewMode::ProductsOnly => {
            graph.filter_to_products_only();
        }
        ViewMode::Trading => {
            graph.filter_to_trading_entities();
        }
        ViewMode::KycUbo => {
            // No additional filtering
        }
    }

    // Apply server-side layout (computes x/y positions)
    let layout_engine = LayoutEngine::with_orientation(view_mode, orientation);
    layout_engine.layout(&mut graph);

    serde_json::to_value(graph).map(Json).map_err(|e| {
        tracing::error!("Graph serialization error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

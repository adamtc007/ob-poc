//! Graph API routes for CBU visualization
//!
//! Provides endpoints to fetch graph data for the egui WASM client.
//! All database access goes through VisualizationRepository (legacy) or GraphRepository (new).
//!
//! Legacy endpoint: /api/cbu/:id/graph returns flat graph (nodes + edges).
//! New endpoints:
//!   /api/graph/cbu/:id - EntityGraph for single CBU
//!   /api/graph/book/:apex_id - EntityGraph for ownership book
//!   /api/graph/jurisdiction/:code - EntityGraph for jurisdiction
//!
//! Session-scoped endpoints share state with REPL/taxonomy:
//!   /api/session/:id/graph - Graph for session's active CBU
//!
//! UI owns layout/visualization logic.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::SessionStore;
use crate::database::{LayoutOverrideView, PgGraphRepository, VisualizationRepository};
use crate::graph::{
    CbuGraph, CbuSummary, ConfigDrivenGraphBuilder, EntityGraph, GraphScope, LayoutEngineV2,
    LayoutOverride, NodeOffset, NodeSizeOverride,
};

/// Query parameters for graph endpoint
#[derive(Debug, Deserialize)]
pub struct GraphQuery {
    /// View mode: KYC_UBO (default), SERVICE_DELIVERY, or CUSTODY
    pub view_mode: Option<String>,
    /// Layout orientation: VERTICAL (default, top-to-bottom) or HORIZONTAL (left-to-right)
    pub orientation: Option<String>,
    /// Point-in-time date for temporal query (defaults to today). Format: YYYY-MM-DD
    pub as_of: Option<String>,
}

/// GET /api/cbu/{cbu_id}/graph?view_mode=KYC_UBO&orientation=VERTICAL
///
/// Returns graph data with server-computed layout positions.
///
/// View mode determines which nodes are included and how they're arranged.
/// Node/edge visibility and layout hints come from database configuration
/// in node_types, edge_types, and view_modes tables.
///
/// Orientation determines flow direction: VERTICAL (top-to-bottom) or HORIZONTAL (left-to-right)
pub async fn get_cbu_graph(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<GraphQuery>,
) -> Result<Json<CbuGraph>, (StatusCode, String)> {
    let view_mode = params.view_mode.as_deref().unwrap_or("KYC_UBO");
    let orientation = params.orientation.as_deref().unwrap_or("VERTICAL");
    let horizontal = orientation.eq_ignore_ascii_case("HORIZONTAL");

    // Build graph using config-driven builder
    let builder = ConfigDrivenGraphBuilder::new(&pool, cbu_id, view_mode)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to initialize config-driven builder: {}", e),
            )
        })?;

    let repo = VisualizationRepository::new(pool.clone());
    let mut graph = builder.build(&repo).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to build graph: {}", e),
        )
    })?;

    // Apply layout using LayoutEngineV2
    let layout_engine = LayoutEngineV2::from_database(&pool, view_mode, horizontal)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load layout config: {}", e),
            )
        })?;

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

// =============================================================================
// NEW UNIFIED GRAPH ENDPOINTS (using EntityGraph)
// =============================================================================

/// Query parameters for unified graph endpoints
#[derive(Debug, Deserialize)]
pub struct UnifiedGraphQuery {
    /// View mode: KYC_UBO (default), UBO_ONLY, BOOK
    pub view_mode: Option<String>,
    /// Layout orientation: VERTICAL (default) or HORIZONTAL
    pub orientation: Option<String>,
    /// Point-in-time date for temporal query (defaults to today). Format: YYYY-MM-DD
    pub as_of: Option<String>,
}

/// GET /api/graph/cbu/{cbu_id}
///
/// Returns unified EntityGraph for a single CBU.
/// Uses the new GraphRepository instead of VisualizationRepository.
/// Supports temporal queries via as_of parameter (defaults to today).
pub async fn get_unified_cbu_graph(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<UnifiedGraphQuery>,
) -> Result<Json<EntityGraph>, (StatusCode, String)> {
    let repo = PgGraphRepository::new(pool);
    let scope = GraphScope::SingleCbu {
        cbu_id,
        cbu_name: String::new(), // Will be populated by the graph after loading
    };

    // Parse as_of date (defaults to today)
    let as_of_date = params
        .as_of
        .as_ref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());

    let mut graph = EntityGraph::load_as_of(scope, as_of_date, &repo)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply layout
    let view_mode = params.view_mode.as_deref().unwrap_or("KYC_UBO");
    let orientation = params.orientation.as_deref().unwrap_or("VERTICAL");
    graph.layout(view_mode, orientation);

    Ok(Json(graph))
}

/// GET /api/graph/book/{apex_entity_id}
///
/// Returns unified EntityGraph for an ownership book (all CBUs under an apex).
/// The apex entity is typically an ultimate holding company.
/// Supports temporal queries via as_of parameter (defaults to today).
pub async fn get_book_graph(
    State(pool): State<PgPool>,
    Path(apex_entity_id): Path<Uuid>,
    Query(params): Query<UnifiedGraphQuery>,
) -> Result<Json<EntityGraph>, (StatusCode, String)> {
    let repo = PgGraphRepository::new(pool);
    let scope = GraphScope::Book {
        apex_entity_id,
        apex_name: String::new(), // Will be populated by the graph after loading
    };

    // Parse as_of date (defaults to today)
    let as_of_date = params
        .as_of
        .as_ref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());

    let mut graph = EntityGraph::load_as_of(scope, as_of_date, &repo)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply layout
    let view_mode = params.view_mode.as_deref().unwrap_or("BOOK");
    let orientation = params.orientation.as_deref().unwrap_or("VERTICAL");
    graph.layout(view_mode, orientation);

    Ok(Json(graph))
}

/// GET /api/graph/jurisdiction/{code}
///
/// Returns unified EntityGraph for all entities in a jurisdiction.
/// Supports temporal queries via as_of parameter (defaults to today).
pub async fn get_jurisdiction_graph(
    State(pool): State<PgPool>,
    Path(code): Path<String>,
    Query(params): Query<UnifiedGraphQuery>,
) -> Result<Json<EntityGraph>, (StatusCode, String)> {
    let repo = PgGraphRepository::new(pool);
    let scope = GraphScope::Jurisdiction { code };

    // Parse as_of date (defaults to today)
    let as_of_date = params
        .as_of
        .as_ref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());

    let mut graph = EntityGraph::load_as_of(scope, as_of_date, &repo)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply layout
    let view_mode = params.view_mode.as_deref().unwrap_or("KYC_UBO");
    let orientation = params.orientation.as_deref().unwrap_or("VERTICAL");
    graph.layout(view_mode, orientation);

    Ok(Json(graph))
}

/// GET /api/graph/entity/{entity_id}/neighborhood
///
/// Returns unified EntityGraph for an entity and its N-hop neighborhood.
/// Supports temporal queries via as_of parameter (defaults to today).
#[derive(Debug, Deserialize)]
pub struct NeighborhoodQuery {
    /// Number of hops to include (default 2)
    pub hops: Option<u32>,
    /// View mode
    pub view_mode: Option<String>,
    /// Layout orientation
    pub orientation: Option<String>,
    /// Point-in-time date for temporal query (defaults to today). Format: YYYY-MM-DD
    pub as_of: Option<String>,
}

pub async fn get_entity_neighborhood_graph(
    State(pool): State<PgPool>,
    Path(entity_id): Path<Uuid>,
    Query(params): Query<NeighborhoodQuery>,
) -> Result<Json<EntityGraph>, (StatusCode, String)> {
    let repo = PgGraphRepository::new(pool);
    let hops = params.hops.unwrap_or(2);
    let scope = GraphScope::EntityNeighborhood { entity_id, hops };

    // Parse as_of date (defaults to today)
    let as_of_date = params
        .as_of
        .as_ref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());

    let mut graph = EntityGraph::load_as_of(scope, as_of_date, &repo)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply layout
    let view_mode = params.view_mode.as_deref().unwrap_or("UBO_ONLY");
    let orientation = params.orientation.as_deref().unwrap_or("VERTICAL");
    graph.layout(view_mode, orientation);

    Ok(Json(graph))
}

/// Create the graph router
pub fn create_graph_router(pool: PgPool) -> Router {
    Router::new()
        // CBU endpoints (config-driven)
        .route("/api/cbu", get(list_cbus))
        .route("/api/cbu/:cbu_id", get(get_cbu))
        .route("/api/cbu/:cbu_id/graph", get(get_cbu_graph))
        .route("/api/cbu/:cbu_id/layout", get(get_cbu_layout))
        .route("/api/cbu/:cbu_id/layout", post(save_cbu_layout))
        // Unified graph endpoints (using GraphRepository + EntityGraph)
        .route("/api/graph/cbu/:cbu_id", get(get_unified_cbu_graph))
        .route("/api/graph/book/:apex_entity_id", get(get_book_graph))
        .route("/api/graph/jurisdiction/:code", get(get_jurisdiction_graph))
        .route(
            "/api/graph/entity/:entity_id/neighborhood",
            get(get_entity_neighborhood_graph),
        )
        .with_state(pool)
}

// =============================================================================
// SESSION-SCOPED GRAPH ENDPOINTS
// =============================================================================

/// State for session-scoped graph endpoints
#[derive(Clone)]
pub struct SessionGraphState {
    pub pool: PgPool,
    pub sessions: SessionStore,
}

/// Response for session graph endpoint
#[derive(Debug, Serialize)]
pub struct SessionGraphResponse {
    /// The graph data (if active CBU exists)
    pub graph: Option<CbuGraph>,
    /// The active CBU ID from session
    pub active_cbu_id: Option<Uuid>,
    /// The active CBU name from session
    pub active_cbu_name: Option<String>,
    /// Error message if graph couldn't be loaded
    pub error: Option<String>,
}

/// GET /api/session/:session_id/graph
///
/// Returns the graph for the session's active CBU, using the same session state
/// as the REPL and taxonomy navigation.
async fn get_session_graph(
    Path(session_id): Path<Uuid>,
    Query(params): Query<GraphQuery>,
    State(state): State<SessionGraphState>,
) -> Result<Json<SessionGraphResponse>, (StatusCode, String)> {
    // Get session and check for active CBU (tokio RwLock requires .await)
    let sessions = state.sessions.read().await;

    let session = sessions.get(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Session {} not found", session_id),
        )
    })?;

    // Check if session has an active CBU
    let active_cbu = match &session.context.active_cbu {
        Some(cbu) => cbu.clone(),
        None => {
            return Ok(Json(SessionGraphResponse {
                graph: None,
                active_cbu_id: None,
                active_cbu_name: None,
                error: Some(
                    "No active CBU in session. Use /api/session/:id/bind to set one.".to_string(),
                ),
            }));
        }
    };

    let cbu_id = active_cbu.id;
    let cbu_name = active_cbu.display_name.clone();

    // Get session's stored view_mode as fallback (REPL/View alignment)
    let session_view_mode = session.context.view_mode.clone();

    // Drop the read lock before doing async work
    drop(sessions);

    // Build graph for the active CBU using config-driven approach
    // Priority: query param > session stored > default
    let view_mode = params
        .view_mode
        .as_deref()
        .or(session_view_mode.as_deref())
        .unwrap_or("KYC_UBO");
    let orientation = params.orientation.as_deref().unwrap_or("VERTICAL");
    let horizontal = orientation.eq_ignore_ascii_case("HORIZONTAL");

    let builder = ConfigDrivenGraphBuilder::new(&state.pool, cbu_id, view_mode)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to initialize config-driven builder: {}", e),
            )
        })?;

    let repo = VisualizationRepository::new(state.pool.clone());
    let mut graph = builder.build(&repo).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to build graph: {}", e),
        )
    })?;

    // Apply layout using LayoutEngineV2
    let layout_engine = LayoutEngineV2::from_database(&state.pool, view_mode, horizontal)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load layout config: {}", e),
            )
        })?;

    layout_engine.layout(&mut graph);

    Ok(Json(SessionGraphResponse {
        graph: Some(graph),
        active_cbu_id: Some(cbu_id),
        active_cbu_name: Some(cbu_name),
        error: None,
    }))
}

/// Create the session-scoped graph router
///
/// This router shares session state with the REPL and taxonomy navigation,
/// providing a unified view of the current context.
pub fn create_session_graph_router(pool: PgPool, sessions: SessionStore) -> Router {
    let state = SessionGraphState { pool, sessions };

    Router::new()
        .route("/api/session/:session_id/graph", get(get_session_graph))
        .with_state(state)
}

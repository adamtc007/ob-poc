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

use crate::database::{LayoutOverrideView, PgGraphRepository, VisualizationRepository};
use crate::graph::{
    CbuGraph, CbuGraphBuilder, CbuSummary, EntityGraph, GraphScope, LayoutEngine, LayoutOverride,
    NodeOffset, NodeSizeOverride, Orientation, ViewMode,
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
/// View mode determines which nodes are included and how they're arranged:
/// - KYC_UBO: CBU + entities (by role category) + KYC/UBO layers
/// - SERVICE_DELIVERY: CBU + products + services + resources (no entities)
/// - CUSTODY: CBU + custody layer (markets, SSIs, rules)
/// - PRODUCTS_ONLY: CBU + products only
///
/// Orientation determines flow direction: VERTICAL (top-to-bottom) or HORIZONTAL (left-to-right)
pub async fn get_cbu_graph(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<GraphQuery>,
) -> Result<Json<CbuGraph>, (StatusCode, String)> {
    let repo = VisualizationRepository::new(pool);
    let view_mode = ViewMode::parse(params.view_mode.as_deref().unwrap_or("KYC_UBO"));
    let orientation = Orientation::parse(params.orientation.as_deref().unwrap_or("VERTICAL"));

    // Load layers based on view_mode - server does the filtering
    let mut graph = match view_mode {
        ViewMode::KycUbo => {
            // KYC/UBO view: entities (via roles) + KYC + UBO layers, no services
            CbuGraphBuilder::new(cbu_id)
                .with_custody(false)
                .with_kyc(true)
                .with_ubo(true)
                .with_services(false)
                .build(&repo)
                .await
        }
        ViewMode::UboOnly => {
            // UBO Only view: pure ownership/control graph - no roles, no products
            // Load entities via roles (to get all potential UBO entities) plus UBO layer
            // Then filter to ownership/control edges only
            CbuGraphBuilder::new(cbu_id)
                .with_custody(false)
                .with_kyc(false)
                .with_ubo(true)
                .with_services(false)
                .with_entities(true) // Load entities so UBO layer can reference them
                .build(&repo)
                .await
        }
        ViewMode::ServiceDelivery => {
            // Service Delivery view: products + services + resources + trading entities
            // Load all entities, then filter to trading roles
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
                .with_services(true) // Services layer loads products
                .with_entities(false)
                .build(&repo)
                .await
        }
        ViewMode::Trading => {
            // Trading view: CBU as container with trading entities (Asset Owner, IM, ManCo, etc.)
            // No products, no services - just the trading entities
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
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply view-mode specific filtering after loading
    match view_mode {
        ViewMode::UboOnly => {
            // Remove role edges, keep only ownership/control edges
            graph.filter_to_ubo_only();
        }
        ViewMode::ServiceDelivery => {
            // Filter entities to trading roles only
            graph.filter_to_trading_entities();
        }
        ViewMode::ProductsOnly => {
            // Keep only CBU and Product nodes
            graph.filter_to_products_only();
        }
        ViewMode::Trading => {
            // Filter to trading entities only (Asset Owner, IM, ManCo, etc.)
            graph.filter_to_trading_entities();
        }
        ViewMode::KycUbo => {
            // No additional filtering needed - shows full KYC/UBO structure
        }
    }

    // Apply server-side layout based on view mode and orientation
    let layout_engine = LayoutEngine::with_orientation(view_mode, orientation);
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
        // Legacy CBU endpoints (using VisualizationRepository)
        .route("/api/cbu", get(list_cbus))
        .route("/api/cbu/:cbu_id", get(get_cbu))
        .route("/api/cbu/:cbu_id/graph", get(get_cbu_graph))
        .route("/api/cbu/:cbu_id/layout", get(get_cbu_layout))
        .route("/api/cbu/:cbu_id/layout", post(save_cbu_layout))
        // New unified graph endpoints (using GraphRepository + EntityGraph)
        .route("/api/graph/cbu/:cbu_id", get(get_unified_cbu_graph))
        .route("/api/graph/book/:apex_entity_id", get(get_book_graph))
        .route("/api/graph/jurisdiction/:code", get(get_jurisdiction_graph))
        .route(
            "/api/graph/entity/:entity_id/neighborhood",
            get(get_entity_neighborhood_graph),
        )
        .with_state(pool)
}

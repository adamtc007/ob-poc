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
use ob_poc_types::galaxy::{NodeType, Route, RouteResponse, RouteWaypoint, ViewLevel};

/// Query parameters for graph endpoint
#[derive(Debug, Deserialize)]
pub struct GraphQuery {
    /// View mode: TRADING (default), KYC_UBO, SERVICE_DELIVERY, or CUSTODY
    pub view_mode: Option<String>,
    /// Layout orientation: VERTICAL (default, top-to-bottom) or HORIZONTAL (left-to-right)
    pub orientation: Option<String>,
    /// Point-in-time date for temporal query (defaults to today). Format: YYYY-MM-DD
    pub as_of: Option<String>,
}

/// GET /api/cbu/{cbu_id}/graph?view_mode=TRADING&orientation=VERTICAL
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
    let view_mode = params.view_mode.as_deref().unwrap_or("TRADING");
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
            cbu_category: c.cbu_category,
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
        cbu_category: cbu.cbu_category,
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
        .unwrap_or_else(|| "TRADING".to_string())
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
    /// View mode: TRADING (default), KYC_UBO, UBO_ONLY, BOOK
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
    let view_mode = params.view_mode.as_deref().unwrap_or("TRADING");
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
    let view_mode = params.view_mode.as_deref().unwrap_or("TRADING");
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

// =============================================================================
// ROUTE CALCULATION (for autopilot navigation)
// =============================================================================

/// Query parameters for route endpoint
#[derive(Debug, Deserialize)]
pub struct RouteQuery {
    /// Starting node ID (current location if omitted)
    pub from: Option<String>,
    /// Destination node ID or name
    pub to: String,
    /// Destination type hint (helps resolve names)
    pub to_type: Option<String>,
    /// Whether to pause at decision points
    #[serde(default)]
    pub pause_at_forks: bool,
}

/// GET /api/route?to=X&from=Y
///
/// Calculate a navigation route through the taxonomy graph.
/// Returns waypoints with positions for autopilot animation.
pub async fn get_route(
    State(pool): State<PgPool>,
    Query(params): Query<RouteQuery>,
) -> Result<Json<RouteResponse>, (StatusCode, String)> {
    // Parse destination - could be UUID or name
    let destination_id = parse_node_id(&pool, &params.to, params.to_type.as_deref()).await?;

    // Parse origin (defaults to universe root if not specified)
    let origin_id = if let Some(from) = &params.from {
        Some(parse_node_id(&pool, from, None).await?)
    } else {
        None
    };

    // Calculate the route through the graph
    let route = calculate_route(&pool, origin_id, destination_id, params.pause_at_forks).await?;

    // Estimate duration based on waypoint count and level transitions
    let estimated_duration = estimate_route_duration(&route);

    Ok(Json(RouteResponse {
        route,
        estimated_duration_secs: estimated_duration,
        alternatives: vec![], // Future: implement alternative routes
    }))
}

/// Parse a node ID - could be UUID or name requiring lookup
async fn parse_node_id(
    pool: &PgPool,
    id_or_name: &str,
    type_hint: Option<&str>,
) -> Result<(String, NodeType), (StatusCode, String)> {
    // Try parsing as UUID first
    if let Ok(uuid) = Uuid::parse_str(id_or_name) {
        // Determine node type from database
        let node_type = determine_node_type(pool, uuid).await?;
        return Ok((uuid.to_string(), node_type));
    }

    // Not a UUID - search by name based on type hint
    let node_type = type_hint
        .and_then(|t| match t.to_lowercase().as_str() {
            "cbu" => Some(NodeType::Cbu),
            "entity" => Some(NodeType::Entity),
            "cluster" => Some(NodeType::Cluster),
            "document" => Some(NodeType::Document),
            "kyc_case" | "kyccase" => Some(NodeType::KycCase),
            _ => None,
        })
        .unwrap_or(NodeType::Cbu); // Default to CBU search

    // Search for the node by name
    let node_id = search_node_by_name(pool, id_or_name, node_type).await?;
    Ok((node_id, node_type))
}

/// Determine node type from UUID by checking various tables
async fn determine_node_type(pool: &PgPool, id: Uuid) -> Result<NodeType, (StatusCode, String)> {
    // Check if it's a CBU
    let cbu_exists: Option<(i64,)> =
        sqlx::query_as(r#"SELECT 1 FROM "ob-poc".cbus WHERE cbu_id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if cbu_exists.is_some() {
        return Ok(NodeType::Cbu);
    }

    // Check if it's an entity
    let entity_exists: Option<(i64,)> =
        sqlx::query_as(r#"SELECT 1 FROM "ob-poc".entities WHERE entity_id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if entity_exists.is_some() {
        return Ok(NodeType::Entity);
    }

    // Check if it's a document
    let doc_exists: Option<(i64,)> =
        sqlx::query_as(r#"SELECT 1 FROM "ob-poc".document_catalog WHERE doc_id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if doc_exists.is_some() {
        return Ok(NodeType::Document);
    }

    // Check if it's a KYC case
    let case_exists: Option<(i64,)> =
        sqlx::query_as(r#"SELECT 1 FROM "kyc".cases WHERE case_id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if case_exists.is_some() {
        return Ok(NodeType::KycCase);
    }

    Err((StatusCode::NOT_FOUND, format!("Node not found: {}", id)))
}

/// Search for a node by name and type
async fn search_node_by_name(
    pool: &PgPool,
    name: &str,
    node_type: NodeType,
) -> Result<String, (StatusCode, String)> {
    let search_pattern = format!("%{}%", name);

    match node_type {
        NodeType::Cbu => {
            let result: Option<(Uuid,)> =
                sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE $1 LIMIT 1"#)
                    .bind(&search_pattern)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            result
                .map(|(id,)| id.to_string())
                .ok_or_else(|| (StatusCode::NOT_FOUND, format!("CBU not found: {}", name)))
        }
        NodeType::Entity => {
            let result: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE $1 LIMIT 1"#,
            )
            .bind(&search_pattern)
            .fetch_optional(pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            result
                .map(|(id,)| id.to_string())
                .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Entity not found: {}", name)))
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!("Search not supported for type: {:?}", node_type),
        )),
    }
}

/// Calculate a route through the graph from origin to destination
async fn calculate_route(
    pool: &PgPool,
    origin: Option<(String, NodeType)>,
    destination: (String, NodeType),
    pause_at_forks: bool,
) -> Result<Route, (StatusCode, String)> {
    let route_id = Uuid::now_v7().to_string();
    let mut waypoints = Vec::new();
    let mut level_transitions = 0;

    // If no origin, start from universe
    let start_level = origin
        .as_ref()
        .map(|(_, t)| node_type_to_view_level(t))
        .unwrap_or(ViewLevel::Universe);

    let dest_level = node_type_to_view_level(&destination.1);

    // Build waypoints based on navigation path
    // For now, create a direct path - future: use taxonomy tree for proper path finding

    // Add origin waypoint if specified
    if let Some((origin_id, origin_type)) = &origin {
        let origin_info = get_node_info(pool, origin_id, origin_type).await?;
        waypoints.push(RouteWaypoint {
            node_id: origin_id.clone(),
            node_type: *origin_type,
            label: origin_info.0,
            position: origin_info.1,
            view_level: start_level,
            is_fork: false,
            context_hint: None,
        });
    } else {
        // Start from universe root
        waypoints.push(RouteWaypoint {
            node_id: "universe".to_string(),
            node_type: NodeType::Universe,
            label: "Universe".to_string(),
            position: (0.0, 0.0),
            view_level: ViewLevel::Universe,
            is_fork: false,
            context_hint: Some("Starting from universe view".to_string()),
        });
    }

    // Add intermediate waypoints for level transitions
    let intermediate_levels = get_intermediate_levels(start_level, dest_level);
    for level in &intermediate_levels {
        level_transitions += 1;
        // Add a transitional waypoint (these would be refined with actual data)
        waypoints.push(RouteWaypoint {
            node_id: format!("transition_{:?}", level),
            node_type: view_level_to_node_type(level),
            label: format!("Navigating to {:?}", level),
            position: calculate_transition_position(&waypoints, level),
            view_level: *level,
            is_fork: pause_at_forks && is_fork_level(level),
            context_hint: Some(format!("Transitioning through {:?} level", level)),
        });
    }

    // Add destination waypoint
    let dest_info = get_node_info(pool, &destination.0, &destination.1).await?;
    waypoints.push(RouteWaypoint {
        node_id: destination.0.clone(),
        node_type: destination.1,
        label: dest_info.0.clone(),
        position: dest_info.1,
        view_level: dest_level,
        is_fork: false,
        context_hint: Some(format!("Destination: {}", dest_info.0)),
    });

    // Calculate total distance
    let total_distance = calculate_total_distance(&waypoints);

    // Build description
    let description = if let Some((origin_id, _)) = &origin {
        format!("Route from {} to {}", origin_id, dest_info.0)
    } else {
        format!("Route to {}", dest_info.0)
    };

    Ok(Route {
        route_id,
        waypoints,
        total_distance,
        level_transitions,
        description,
    })
}

/// Get node info (label, position) from database
async fn get_node_info(
    pool: &PgPool,
    node_id: &str,
    node_type: &NodeType,
) -> Result<(String, (f32, f32)), (StatusCode, String)> {
    let uuid = Uuid::parse_str(node_id).ok();

    match node_type {
        NodeType::Cbu => {
            if let Some(id) = uuid {
                let result: Option<(String,)> =
                    sqlx::query_as(r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                        .bind(id)
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

                if let Some((name,)) = result {
                    // Position would ideally come from layout, use hash-based position for now
                    let pos = hash_to_position(node_id);
                    return Ok((name, pos));
                }
            }
            Err((StatusCode::NOT_FOUND, format!("CBU not found: {}", node_id)))
        }
        NodeType::Entity => {
            if let Some(id) = uuid {
                let result: Option<(String,)> =
                    sqlx::query_as(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                        .bind(id)
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

                if let Some((name,)) = result {
                    let pos = hash_to_position(node_id);
                    return Ok((name, pos));
                }
            }
            Err((
                StatusCode::NOT_FOUND,
                format!("Entity not found: {}", node_id),
            ))
        }
        NodeType::Universe => Ok(("Universe".to_string(), (0.0, 0.0))),
        NodeType::Cluster => Ok((format!("Cluster {}", node_id), hash_to_position(node_id))),
        NodeType::Document => Ok((format!("Document {}", node_id), hash_to_position(node_id))),
        NodeType::KycCase => Ok((format!("KYC Case {}", node_id), hash_to_position(node_id))),
    }
}

/// Convert node type to view level
fn node_type_to_view_level(node_type: &NodeType) -> ViewLevel {
    match node_type {
        NodeType::Universe => ViewLevel::Universe,
        NodeType::Cluster => ViewLevel::Cluster,
        NodeType::Cbu => ViewLevel::System,
        NodeType::Entity => ViewLevel::Planet,
        NodeType::Document | NodeType::KycCase => ViewLevel::Surface,
    }
}

/// Convert view level to typical node type
fn view_level_to_node_type(level: &ViewLevel) -> NodeType {
    match level {
        ViewLevel::Universe => NodeType::Universe,
        ViewLevel::Cluster => NodeType::Cluster,
        ViewLevel::System => NodeType::Cbu,
        ViewLevel::Planet => NodeType::Entity,
        ViewLevel::Surface | ViewLevel::Core => NodeType::Entity,
    }
}

/// Get intermediate levels between two view levels
fn get_intermediate_levels(from: ViewLevel, to: ViewLevel) -> Vec<ViewLevel> {
    let all_levels = [
        ViewLevel::Universe,
        ViewLevel::Cluster,
        ViewLevel::System,
        ViewLevel::Planet,
        ViewLevel::Surface,
        ViewLevel::Core,
    ];

    let from_idx = all_levels.iter().position(|l| *l == from).unwrap_or(0);
    let to_idx = all_levels.iter().position(|l| *l == to).unwrap_or(0);

    if from_idx < to_idx {
        // Going deeper
        all_levels[from_idx + 1..to_idx].to_vec()
    } else if from_idx > to_idx {
        // Going shallower
        all_levels[to_idx + 1..from_idx]
            .iter()
            .rev()
            .cloned()
            .collect()
    } else {
        vec![]
    }
}

/// Check if a level is typically a fork/decision point
fn is_fork_level(level: &ViewLevel) -> bool {
    matches!(level, ViewLevel::Cluster | ViewLevel::System)
}

/// Calculate transition position based on previous waypoints
fn calculate_transition_position(waypoints: &[RouteWaypoint], _level: &ViewLevel) -> (f32, f32) {
    if let Some(last) = waypoints.last() {
        // Move in a direction based on the transition
        (last.position.0 + 100.0, last.position.1 + 50.0)
    } else {
        (0.0, 0.0)
    }
}

/// Calculate total distance for a route
fn calculate_total_distance(waypoints: &[RouteWaypoint]) -> f32 {
    let mut total = 0.0;
    for i in 1..waypoints.len() {
        let prev = &waypoints[i - 1];
        let curr = &waypoints[i];
        let dx = curr.position.0 - prev.position.0;
        let dy = curr.position.1 - prev.position.1;
        total += (dx * dx + dy * dy).sqrt();
    }
    total
}

/// Estimate route duration in seconds
fn estimate_route_duration(route: &Route) -> f32 {
    // Base: 1 second per waypoint + 0.5 seconds per level transition
    let base_time = route.waypoints.len() as f32 * 1.0;
    let transition_time = route.level_transitions as f32 * 0.5;
    // Distance factor (roughly 100 units per second flight speed)
    let distance_time = route.total_distance / 100.0;

    base_time + transition_time + distance_time
}

/// Generate a deterministic position from a node ID hash
fn hash_to_position(id: &str) -> (f32, f32) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();

    // Map hash to reasonable coordinate range
    let x = ((hash & 0xFFFF) as f32 / 65535.0) * 2000.0 - 1000.0;
    let y = (((hash >> 16) & 0xFFFF) as f32 / 65535.0) * 2000.0 - 1000.0;
    (x, y)
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
        // Galaxy navigation route endpoint
        .route("/api/route", get(get_route))
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

/// Response for multi-CBU session graph endpoint
#[derive(Debug, Serialize)]
pub struct MultiCbuGraphResponse {
    /// Combined graph containing all CBUs in session scope
    pub graph: Option<CbuGraph>,
    /// All CBU IDs included in the graph
    pub cbu_ids: Vec<Uuid>,
    /// Count of CBUs in scope
    pub cbu_count: usize,
    /// Entity IDs that were recently affected (for highlighting)
    pub affected_entity_ids: Vec<Uuid>,
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
        .unwrap_or("TRADING");
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

/// GET /api/session/:session_id/scope-graph
///
/// Returns combined graph for ALL CBUs in session scope (context.cbu_ids).
/// This supports the multi-CBU bulk update workflow where users see all
/// entities they're working on in a single viewport.
async fn get_session_scope_graph(
    Path(session_id): Path<Uuid>,
    Query(params): Query<GraphQuery>,
    State(state): State<SessionGraphState>,
) -> Result<Json<MultiCbuGraphResponse>, (StatusCode, String)> {
    // Get session and extract CBU IDs + affected entities from run_sheet
    let (cbu_ids, affected_entity_ids, view_mode_hint) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Session {} not found", session_id),
            )
        })?;

        // Get all CBU IDs from session context
        let cbu_ids = session.context.cbu_ids.clone();

        // Get affected entity IDs from recent run_sheet entries (for highlighting)
        let affected: Vec<Uuid> = session
            .run_sheet
            .entries
            .iter()
            .filter(|e| e.status == crate::session::EntryStatus::Executed)
            .flat_map(|e| e.affected_entities.iter().copied())
            .collect();

        let view_mode = session.context.view_mode.clone();

        (cbu_ids, affected, view_mode)
    };

    if cbu_ids.is_empty() {
        return Ok(Json(MultiCbuGraphResponse {
            graph: None,
            cbu_ids: vec![],
            cbu_count: 0,
            affected_entity_ids: vec![],
            error: Some("No CBUs in session scope. Execute DSL to create CBUs.".to_string()),
        }));
    }

    let cbu_count = cbu_ids.len();

    // Build combined graph for all CBUs
    let view_mode = params
        .view_mode
        .as_deref()
        .or(view_mode_hint.as_deref())
        .unwrap_or("TRADING");
    let orientation = params.orientation.as_deref().unwrap_or("VERTICAL");
    let horizontal = orientation.eq_ignore_ascii_case("HORIZONTAL");

    // Build graphs for each CBU and merge them
    let mut combined_graph = CbuGraph {
        cbu_id: cbu_ids.first().copied().unwrap_or_default(),
        label: format!("{} CBUs in scope", cbu_count),
        cbu_category: None,
        jurisdiction: None,
        nodes: Vec::new(),
        edges: Vec::new(),
        layers: Vec::new(),
        stats: Default::default(),
    };

    let repo = VisualizationRepository::new(state.pool.clone());

    for cbu_id in &cbu_ids {
        let builder = match ConfigDrivenGraphBuilder::new(&state.pool, *cbu_id, view_mode).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Failed to build graph for CBU {}: {}", cbu_id, e);
                continue;
            }
        };

        match builder.build(&repo).await {
            Ok(mut graph) => {
                // Apply layout to this CBU's graph
                let layout_engine =
                    match LayoutEngineV2::from_database(&state.pool, view_mode, horizontal).await {
                        Ok(e) => e,
                        Err(e) => {
                            tracing::warn!("Failed to load layout for CBU {}: {}", cbu_id, e);
                            continue;
                        }
                    };
                layout_engine.layout(&mut graph);

                // Merge into combined graph
                combined_graph.nodes.extend(graph.nodes);
                combined_graph.edges.extend(graph.edges);
            }
            Err(e) => {
                tracing::warn!("Failed to build graph for CBU {}: {}", cbu_id, e);
            }
        }
    }

    Ok(Json(MultiCbuGraphResponse {
        graph: Some(combined_graph),
        cbu_ids,
        cbu_count,
        affected_entity_ids,
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
        .route(
            "/api/session/:session_id/scope-graph",
            get(get_session_scope_graph),
        )
        .with_state(state)
}

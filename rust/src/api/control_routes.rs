//! Control/Ownership API Routes
//!
//! Endpoints for board control computation and control sphere navigation.
//!
//! ## Endpoints
//!
//! - `GET /api/cbu/{id}/board-controller` - Get computed board controller
//! - `POST /api/cbu/{id}/board-controller/recompute` - Force recomputation
//! - `GET /api/cbu/{id}/control-anchors` - Get control anchors
//! - `POST /api/cbu/{id}/control-anchors` - Set control anchors
//! - `GET /api/control-sphere/{entity_id}` - Get control sphere subgraph

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDate;
use ob_poc_types::control::{
    AnchorRole, BoardControllerEdge, ControlAnchor, ControlEdge, ControlEdgeType, ControlEntityRef,
    ControlSphere, GetBoardControllerResponse, GetControlAnchorsResponse, GetControlSphereResponse,
    SetControlAnchorsRequest,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::BoardControlRulesEngine;

/// Application state for control routes
#[derive(Clone)]
pub struct ControlAppState {
    pub pool: PgPool,
}

/// Create the control routes router
pub fn control_routes(pool: PgPool) -> Router {
    let state = ControlAppState { pool };

    Router::new()
        .route("/api/cbu/:id/board-controller", get(get_board_controller))
        .route(
            "/api/cbu/:id/board-controller/recompute",
            post(recompute_board_controller),
        )
        .route(
            "/api/cbu/:id/control-anchors",
            get(get_control_anchors).post(set_control_anchors),
        )
        .route("/api/control-sphere/:entity_id", get(get_control_sphere))
        .with_state(state)
}

// ============================================================================
// BOARD CONTROLLER ENDPOINTS
// ============================================================================

/// GET /api/cbu/{id}/board-controller
///
/// Returns the computed board controller for a CBU, including explanation
/// and evidence references.
async fn get_board_controller(
    State(state): State<ControlAppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<GetBoardControllerResponse>, (StatusCode, String)> {
    let engine = BoardControlRulesEngine::new(state.pool.clone());

    // Try to load existing computation
    let existing = engine
        .load_for_cbu(cbu_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(controller) = existing {
        return Ok(Json(GetBoardControllerResponse {
            cbu_id,
            board_controller: Some(controller),
        }));
    }

    // No existing computation - compute on demand
    let result = engine
        .compute_for_cbu(cbu_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Store the result
    let _ = engine.store_result(cbu_id, &result, "on_demand").await;

    // Convert to response
    let controller = BoardControllerEdge {
        id: Uuid::new_v4(),
        cbu_id,
        controller_entity_id: result.controller_entity_id,
        controller_name: result.controller_name,
        method: result.method,
        confidence: result.confidence,
        score: result.score,
        as_of: result
            .explanation
            .as_of
            .unwrap_or_else(|| chrono::Utc::now().date_naive()),
        explanation: result.explanation,
    };

    Ok(Json(GetBoardControllerResponse {
        cbu_id,
        board_controller: Some(controller),
    }))
}

/// POST /api/cbu/{id}/board-controller/recompute
///
/// Force recomputation of board controller, even if cached result exists.
async fn recompute_board_controller(
    State(state): State<ControlAppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<GetBoardControllerResponse>, (StatusCode, String)> {
    let engine = BoardControlRulesEngine::new(state.pool.clone());

    // Compute fresh
    let result = engine
        .compute_for_cbu(cbu_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Store the result
    let id = engine
        .store_result(cbu_id, &result, "manual_recompute")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Convert to response
    let controller = BoardControllerEdge {
        id,
        cbu_id,
        controller_entity_id: result.controller_entity_id,
        controller_name: result.controller_name,
        method: result.method,
        confidence: result.confidence,
        score: result.score,
        as_of: result
            .explanation
            .as_of
            .unwrap_or_else(|| chrono::Utc::now().date_naive()),
        explanation: result.explanation,
    };

    Ok(Json(GetBoardControllerResponse {
        cbu_id,
        board_controller: Some(controller),
    }))
}

// ============================================================================
// CONTROL ANCHORS ENDPOINTS
// ============================================================================

/// GET /api/cbu/{id}/control-anchors
///
/// Returns the control anchors (portal entities) for a CBU.
async fn get_control_anchors(
    State(state): State<ControlAppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<GetControlAnchorsResponse>, (StatusCode, String)> {
    let anchors = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, Option<String>, Option<String>)>(
        r#"SELECT id, cbu_id, entity_id, anchor_role, display_name, jurisdiction
           FROM "ob-poc".cbu_control_anchors
           WHERE cbu_id = $1
           ORDER BY anchor_role"#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to load anchors: {}", e),
        )
    })?;

    let anchors: Vec<ControlAnchor> = anchors
        .into_iter()
        .filter_map(|(id, cbu_id, entity_id, role, name, jurisdiction)| {
            let anchor_role = AnchorRole::from_db_str(&role)?;
            Some(ControlAnchor {
                id,
                cbu_id,
                entity_id,
                entity_name: name,
                anchor_role,
                jurisdiction,
            })
        })
        .collect();

    Ok(Json(GetControlAnchorsResponse { cbu_id, anchors }))
}

/// POST /api/cbu/{id}/control-anchors
///
/// Set or update control anchors for a CBU.
async fn set_control_anchors(
    State(state): State<ControlAppState>,
    Path(cbu_id): Path<Uuid>,
    Json(request): Json<SetControlAnchorsRequest>,
) -> Result<Json<GetControlAnchorsResponse>, (StatusCode, String)> {
    // Start transaction
    let mut tx = state.pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Transaction failed: {}", e),
        )
    })?;

    // Delete existing anchors
    sqlx::query(r#"DELETE FROM "ob-poc".cbu_control_anchors WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to clear anchors: {}", e),
            )
        })?;

    // Insert new anchors
    for item in &request.anchors {
        // Look up entity name for caching
        let entity_info = sqlx::query_as::<_, (String, Option<String>)>(
            r#"SELECT
                COALESCE(ep.search_name, elc.company_name, 'Unknown') as name,
                COALESCE(ep.nationality, elc.jurisdiction) as jurisdiction
               FROM "ob-poc".entities e
               LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = e.entity_id
               LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
               WHERE e.entity_id = $1"#,
        )
        .bind(item.entity_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to lookup entity: {}", e),
            )
        })?;

        let (display_name, jurisdiction) = entity_info.unwrap_or(("Unknown".to_string(), None));

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_control_anchors
               (cbu_id, entity_id, anchor_role, display_name, jurisdiction)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(cbu_id)
        .bind(item.entity_id)
        .bind(item.anchor_role.to_db_str())
        .bind(display_name)
        .bind(jurisdiction)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to insert anchor: {}", e),
            )
        })?;
    }

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Commit failed: {}", e),
        )
    })?;

    // Trigger recomputation of board controller
    let engine = BoardControlRulesEngine::new(state.pool.clone());
    if let Ok(result) = engine.compute_for_cbu(cbu_id).await {
        let _ = engine.store_result(cbu_id, &result, "anchor_change").await;
    }

    // Return updated anchors
    get_control_anchors(State(state), Path(cbu_id)).await
}

// ============================================================================
// CONTROL SPHERE ENDPOINT
// ============================================================================

/// Query parameters for control sphere
#[derive(Debug, Deserialize)]
pub struct ControlSphereQuery {
    /// Maximum depth to traverse (default 3)
    #[serde(default = "default_depth")]
    pub depth: u8,
    /// As-of date for historical view (default: today)
    pub as_of: Option<NaiveDate>,
}

fn default_depth() -> u8 {
    3
}

/// GET /api/control-sphere/{entity_id}
///
/// Returns the control sphere subgraph rooted at an entity, up to N layers.
/// Includes ownership/voting edges with BODS/GLEIF/PSC annotations.
async fn get_control_sphere(
    State(state): State<ControlAppState>,
    Path(anchor_entity_id): Path<Uuid>,
    Query(params): Query<ControlSphereQuery>,
) -> Result<Json<GetControlSphereResponse>, (StatusCode, String)> {
    let depth = params.depth.min(10); // Cap at 10 to prevent runaway queries
    let as_of = params
        .as_of
        .unwrap_or_else(|| chrono::Utc::now().date_naive());

    // Get anchor entity info
    let anchor_info = get_entity_info(&state.pool, anchor_entity_id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    // Walk the control graph upward from anchor
    let (nodes, edges) = walk_control_graph(&state.pool, anchor_entity_id, depth, as_of)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Find ultimate controller (top of the chain)
    let ultimate_controller = find_ultimate_controller(&nodes, &edges);

    // Compute board control summary for UBOs
    let board_control_summary = compute_board_control_summary(&nodes, &edges);

    let sphere = ControlSphere {
        anchor_entity: anchor_info,
        ultimate_controller,
        nodes,
        edges,
        board_control_summary,
    };

    Ok(Json(GetControlSphereResponse { sphere, depth }))
}

/// Get basic entity info
async fn get_entity_info(pool: &PgPool, entity_id: Uuid) -> Result<ControlEntityRef, String> {
    let row = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        r#"SELECT
            e.entity_id,
            COALESCE(ep.search_name, elc.company_name, 'Unknown') as name,
            COALESCE(
                CASE WHEN ep.entity_id IS NOT NULL THEN 'Person' END,
                CASE WHEN elc.entity_id IS NOT NULL THEN 'LegalEntity' END,
                'Unknown'
            ) as entity_type,
            COALESCE(ep.nationality, elc.jurisdiction) as jurisdiction
           FROM "ob-poc".entities e
           LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = e.entity_id
           LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
           WHERE e.entity_id = $1"#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to get entity info: {}", e))?
    .ok_or_else(|| format!("Entity {} not found", entity_id))?;

    Ok(ControlEntityRef {
        id: row.0,
        name: row.1,
        entity_type: row.2,
        jurisdiction: row.3,
        is_ubo: false, // Will be set based on analysis
    })
}

/// Walk the control graph upward from an anchor entity
async fn walk_control_graph(
    pool: &PgPool,
    start_entity_id: Uuid,
    max_depth: u8,
    as_of: NaiveDate,
) -> Result<(Vec<ControlEntityRef>, Vec<ControlEdge>), String> {
    // Use recursive CTE to walk the graph
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,                          // edge id
            Uuid,                          // from_entity_id
            String,                        // from_name
            String,                        // from_type
            Option<String>,                // from_jurisdiction
            Uuid,                          // to_entity_id
            String,                        // edge_type
            Option<rust_decimal::Decimal>, // percentage
            bool,                          // is_direct
            Option<String>,                // bods_interest_type
            Option<String>,                // gleif_relationship_type
            Option<String>,                // psc_category
            Option<String>,                // source_register
            Option<NaiveDate>,             // effective_date
            i32,                           // depth
        ),
    >(
        r#"WITH RECURSIVE control_chain AS (
            -- Base case: edges pointing TO the start entity
            SELECT
                ce.id,
                ce.from_entity_id,
                COALESCE(ep.search_name, elc.company_name, 'Unknown') as from_name,
                COALESCE(
                    CASE WHEN ep.entity_id IS NOT NULL THEN 'Person' END,
                    CASE WHEN elc.entity_id IS NOT NULL THEN 'LegalEntity' END,
                    'Unknown'
                ) as from_type,
                COALESCE(ep.nationality, elc.jurisdiction) as from_jurisdiction,
                ce.to_entity_id,
                ce.edge_type,
                ce.percentage,
                ce.is_direct,
                ce.bods_interest_type,
                ce.gleif_relationship_type,
                ce.psc_category,
                ce.source_register,
                ce.effective_date,
                1 as depth,
                ARRAY[ce.from_entity_id] as path
            FROM "ob-poc".control_edges ce
            LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = ce.from_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = ce.from_entity_id
            WHERE ce.to_entity_id = $1
              AND (ce.end_date IS NULL OR ce.end_date > $2)
              AND (ce.effective_date IS NULL OR ce.effective_date <= $2)

            UNION ALL

            -- Recursive case: walk up from current entities
            SELECT
                ce.id,
                ce.from_entity_id,
                COALESCE(ep.search_name, elc.company_name, 'Unknown'),
                COALESCE(
                    CASE WHEN ep.entity_id IS NOT NULL THEN 'Person' END,
                    CASE WHEN elc.entity_id IS NOT NULL THEN 'LegalEntity' END,
                    'Unknown'
                ),
                COALESCE(ep.nationality, elc.jurisdiction),
                ce.to_entity_id,
                ce.edge_type,
                ce.percentage,
                ce.is_direct,
                ce.bods_interest_type,
                ce.gleif_relationship_type,
                ce.psc_category,
                ce.source_register,
                ce.effective_date,
                cc.depth + 1,
                cc.path || ce.from_entity_id
            FROM control_chain cc
            JOIN "ob-poc".control_edges ce ON ce.to_entity_id = cc.from_entity_id
            LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = ce.from_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = ce.from_entity_id
            WHERE cc.depth < $3
              AND NOT ce.from_entity_id = ANY(cc.path)
              AND (ce.end_date IS NULL OR ce.end_date > $2)
              AND (ce.effective_date IS NULL OR ce.effective_date <= $2)
        )
        SELECT id, from_entity_id, from_name, from_type, from_jurisdiction,
               to_entity_id, edge_type, percentage, is_direct,
               bods_interest_type, gleif_relationship_type, psc_category,
               source_register, effective_date, depth
        FROM control_chain
        ORDER BY depth, from_name"#,
    )
    .bind(start_entity_id)
    .bind(as_of)
    .bind(max_depth as i32)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to walk control graph: {}", e))?;

    // Collect unique nodes
    let mut nodes_map: std::collections::HashMap<Uuid, ControlEntityRef> =
        std::collections::HashMap::new();

    // Add start entity
    let start_info = get_entity_info(pool, start_entity_id).await?;
    nodes_map.insert(start_entity_id, start_info);

    let mut edges = Vec::new();

    for row in rows {
        let (
            id,
            from_id,
            from_name,
            from_type,
            from_jurisdiction,
            to_id,
            edge_type,
            percentage,
            is_direct,
            bods_interest,
            gleif_rel,
            psc_cat,
            source_register,
            effective_date,
            _depth,
        ) = row;

        // Add from entity to nodes
        nodes_map
            .entry(from_id)
            .or_insert_with(|| ControlEntityRef {
                id: from_id,
                name: from_name.clone(),
                entity_type: from_type.clone(),
                jurisdiction: from_jurisdiction.clone(),
                is_ubo: from_type == "Person",
            });

        // Add to entity if not present
        nodes_map.entry(to_id).or_insert_with(|| ControlEntityRef {
            id: to_id,
            name: "Unknown".to_string(),
            entity_type: "Unknown".to_string(),
            jurisdiction: None,
            is_ubo: false,
        });

        // Parse edge type
        let edge_type_enum =
            ControlEdgeType::from_db_str(&edge_type).unwrap_or(ControlEdgeType::HoldsShares);

        edges.push(ControlEdge {
            id,
            from_entity_id: from_id,
            to_entity_id: to_id,
            edge_type: edge_type_enum,
            percentage: percentage.map(|d| d.to_string().parse().unwrap_or(0.0)),
            is_direct,
            bods_interest_type: bods_interest,
            gleif_relationship_type: gleif_rel,
            psc_category: psc_cat,
            source_register,
            effective_date,
        });
    }

    let nodes: Vec<ControlEntityRef> = nodes_map.into_values().collect();

    Ok((nodes, edges))
}

/// Find the ultimate controller (entity with no incoming control edges)
fn find_ultimate_controller(
    nodes: &[ControlEntityRef],
    edges: &[ControlEdge],
) -> Option<ControlEntityRef> {
    // Find entities that appear as "from" but never as "to"
    let to_ids: std::collections::HashSet<Uuid> = edges.iter().map(|e| e.to_entity_id).collect();

    for node in nodes {
        if !to_ids.contains(&node.id) && node.entity_type != "Unknown" {
            return Some(node.clone());
        }
    }

    None
}

/// Compute board control summary for UBO persons
fn compute_board_control_summary(
    nodes: &[ControlEntityRef],
    edges: &[ControlEdge],
) -> Vec<ob_poc_types::control::BoardController> {
    use ob_poc_types::control::{BoardController, ControlPathStep, PscCategory};

    let mut controllers = Vec::new();

    // Find person nodes (potential UBOs)
    for node in nodes.iter().filter(|n| n.entity_type == "Person") {
        // Calculate total control percentage through all paths
        let mut total_pct = 0.0f32;
        let mut has_board_majority = false;
        let mut psc_categories = Vec::new();
        let mut control_path = Vec::new();

        // Simple direct edge analysis
        for edge in edges.iter().filter(|e| e.from_entity_id == node.id) {
            if let Some(pct) = edge.percentage {
                total_pct += pct;

                // Check for PSC categories
                if let Some(cat) = PscCategory::from_edge(edge.edge_type, Some(pct)) {
                    if !psc_categories.contains(&cat) {
                        psc_categories.push(cat);
                    }
                }

                // Check for board appointment
                if edge.edge_type == ControlEdgeType::AppointsBoard && pct > 50.0 {
                    has_board_majority = true;
                }
            }

            // Build path
            let to_name = nodes
                .iter()
                .find(|n| n.id == edge.to_entity_id)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            control_path.push(ControlPathStep {
                entity_id: edge.to_entity_id,
                entity_name: to_name,
                edge_type: edge.edge_type,
                percentage: edge.percentage,
            });
        }

        // Only include if meets PSC threshold (25%)
        if total_pct >= 25.0 || has_board_majority || !psc_categories.is_empty() {
            controllers.push(BoardController {
                entity_id: node.id,
                entity_name: node.name.clone(),
                entity_type: node.entity_type.clone(),
                total_control_pct: total_pct,
                has_board_majority,
                psc_categories,
                control_path,
            });
        }
    }

    // Sort by control percentage descending
    controllers.sort_by(|a, b| {
        b.total_control_pct
            .partial_cmp(&a.total_control_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    controllers
}

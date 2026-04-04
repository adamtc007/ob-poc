//! Observatory REST API routes.
//!
//! Exposes OrientationContract and ShowPacket data for the Observatory UI.
//! All endpoints project from existing SemOS types — no new queries.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use sem_os_core::authoring::agent_mode::AgentMode;
use sem_os_core::observatory::orientation::*;
use sem_os_core::observatory::projection;
use sem_os_core::stewardship::types::{FocusState, FocusUpdateSource, OverlayMode};

use crate::api::session::SessionStore;
use crate::sem_reg::stewardship::show_loop::ShowLoop;

/// Observatory API error response.
#[derive(Debug, Serialize)]
struct ObservatoryError {
    error: String,
    code: u16,
}

impl ObservatoryError {
    fn not_found(msg: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self {
                error: msg.into(),
                code: 404,
            }),
        )
    }
}

/// Per-session navigation history with a cursor for back/forward.
#[derive(Debug, Clone, Default)]
pub struct SessionNavHistory {
    /// Ordered list of orientation snapshots.
    entries: Vec<OrientationContract>,
    /// Points to the "current" entry. Always `entries.len() - 1` after a new
    /// navigation, but can be moved backward/forward.
    cursor: usize,
}

/// Thread-safe store for all sessions' navigation histories.
pub type NavigationHistory = Arc<RwLock<HashMap<Uuid, SessionNavHistory>>>;

/// Route state for observatory endpoints.
#[derive(Clone)]
pub struct ObservatoryState {
    pub pool: PgPool,
    pub sessions: SessionStore,
    pub nav_history: NavigationHistory,
}

/// Build a FocusState from session ID (default empty focus).
fn default_focus(session_id: Uuid) -> FocusState {
    FocusState {
        session_id,
        changeset_id: None,
        overlay_mode: OverlayMode::ActiveOnly,
        object_refs: vec![],
        taxonomy_focus: None,
        resolution_context: None,
        updated_at: chrono::Utc::now(),
        updated_by: FocusUpdateSource::Agent,
    }
}

/// Extract a business label from the session's target universe description.
fn business_label_from_session(
    sessions: &std::collections::HashMap<Uuid, crate::session::UnifiedSession>,
    session_id: Uuid,
) -> Option<String> {
    sessions
        .get(&session_id)
        .and_then(|s| s.target_universe.as_ref())
        .map(|u| u.description.clone())
}

/// GET /api/observatory/session/:id/orientation
///
/// Returns the current OrientationContract for a session.
async fn get_orientation(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    let sessions = state.sessions.read().await;
    if !sessions.contains_key(&session_id) {
        return ObservatoryError::not_found("Session not found").into_response();
    }

    let label = business_label_from_session(&sessions, session_id);
    let focus = default_focus(session_id);

    let contract = projection::project_orientation(
        None,
        &focus,
        ViewLevel::Universe,
        AgentMode::Governed,
        EntryReason::SessionStart,
        label.as_deref(),
    );

    Json(contract).into_response()
}

/// GET /api/observatory/session/:id/show-packet
///
/// Returns the full ShowPacket with orientation for a session.
async fn get_show_packet(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    let sessions = state.sessions.read().await;
    if !sessions.contains_key(&session_id) {
        return ObservatoryError::not_found("Session not found").into_response();
    }

    let label = business_label_from_session(&sessions, session_id);
    drop(sessions); // Release lock before async call

    let focus = default_focus(session_id);

    match ShowLoop::compute_show_packet(&state.pool, &focus, "system", None).await {
        Ok(mut packet) => {
            let contract = projection::project_orientation(
                None,
                &focus,
                ViewLevel::Universe,
                AgentMode::Governed,
                EntryReason::SessionStart,
                label.as_deref(),
            );
            packet.orientation = Some(contract);
            Json(packet).into_response()
        }
        Err(e) => {
            let err = ObservatoryError {
                error: format!("Failed to compute show packet: {e}"),
                code: 500,
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

/// Navigation history response including cursor position.
#[derive(Debug, Serialize)]
struct NavigationHistoryResponse {
    entries: Vec<OrientationContract>,
    cursor: usize,
}

/// GET /api/observatory/session/:id/navigation-history
///
/// Returns the navigation history as OrientationContract sequence with cursor.
async fn get_navigation_history(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    let sessions = state.sessions.read().await;
    if !sessions.contains_key(&session_id) {
        return ObservatoryError::not_found("Session not found").into_response();
    }
    drop(sessions);

    let histories = state.nav_history.read().await;
    if let Some(hist) = histories.get(&session_id) {
        Json(NavigationHistoryResponse {
            entries: hist.entries.clone(),
            cursor: hist.cursor,
        })
        .into_response()
    } else {
        // No history yet — return empty
        Json(NavigationHistoryResponse {
            entries: vec![],
            cursor: 0,
        })
        .into_response()
    }
}

/// GET /api/observatory/health
///
/// Returns aggregated health metrics from maintenance verb results.
/// Phase 2: dashboard data for the Mission Control panel.
async fn get_health_metrics(
    State(state): State<ObservatoryState>,
) -> impl IntoResponse {
    // Query maintenance metrics from the database
    let pending = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sem_reg.changesets WHERE status NOT IN ('published', 'rejected', 'archived')"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let stale_dryruns = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sem_reg.changesets WHERE status = 'dry_run_passed' AND updated_at < NOW() - INTERVAL '7 days'"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let active_snapshots = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sem_reg.snapshots WHERE status = 'active'"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let archived = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sem_reg.changesets WHERE status = 'archived'"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let metrics = HealthMetrics {
        pending_changesets: pending,
        stale_dryruns,
        active_snapshots,
        archived_changesets: archived,
        embedding_freshness_hours: None,
        outbox_depth: None,
    };

    Json(metrics).into_response()
}

/// Health metrics for the Mission Control dashboard.
#[derive(Debug, Serialize)]
struct HealthMetrics {
    pending_changesets: i64,
    stale_dryruns: i64,
    active_snapshots: i64,
    archived_changesets: i64,
    embedding_freshness_hours: Option<f64>,
    outbox_depth: Option<i64>,
}

/// GET /api/observatory/session/:id/graph-scene
///
/// Returns the GraphSceneModel — projected from the session's hydrated constellation.
/// Attempts real hydration if a CBU is in scope; falls back to stub scene.
async fn get_graph_scene(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    use ob_poc_types::galaxy::ViewLevel;
    use ob_poc_types::graph_scene::*;
    use sem_os_core::observatory::graph_scene_projection::{self, SlotProjection, GraphEdgeProjection};

    let sessions = state.sessions.read().await;
    let session = match sessions.get(&session_id) {
        Some(s) => s,
        None => return ObservatoryError::not_found("Session not found").into_response(),
    };

    // Try to get CBU IDs from session scope
    let cbu_ids: Vec<Uuid> = session.entity_scope.cbu_ids.iter().copied().collect();
    let label = session
        .target_universe
        .as_ref()
        .map(|u| u.description.clone())
        .unwrap_or_else(|| "Observatory".into());
    drop(sessions);

    // If we have a CBU, try real hydration
    if let Some(cbu_id) = cbu_ids.first() {
        // Try to load and hydrate a constellation for this CBU
        if let Ok(hydrated) = try_hydrate_cbu(&state.pool, *cbu_id).await {
            // Project HydratedConstellation → GraphSceneModel
            let slots: Vec<SlotProjection> = hydrated
                .slots
                .iter()
                .map(|slot| SlotProjection {
                    name: slot.name.clone(),
                    path: slot.path.clone(),
                    slot_type: serde_json::to_value(&slot.slot_type)
                        .ok()
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_else(|| "entity".into()),
                    computed_state: slot.computed_state.clone(),
                    progress: slot.progress,
                    blocking: slot.blocking,
                    depth: slot.path.matches('.').count(),
                    parent_path: slot.path.rsplit_once('.').map(|(p, _)| p.to_string()),
                    child_count: slot.children.len(),
                    depends_on: vec![],
                    graph_edges: slot.graph_edges.iter().map(|e| GraphEdgeProjection {
                        from_id: e.from_entity_id.to_string(),
                        to_id: e.to_entity_id.to_string(),
                        edge_type: e.ownership_type.clone().unwrap_or_else(|| "ownership".into()),
                        label: e.percentage.map(|p| format!("{:.0}%", p)),
                        weight: e.percentage.unwrap_or(0.0) as f32,
                    }).collect(),
                })
                .collect();

            let scene = graph_scene_projection::project_graph_scene(
                &hydrated.constellation,
                &hydrated.jurisdiction,
                &cbu_id.to_string(),
                &slots,
                ViewLevel::System,
                1,
            );
            return Json(scene).into_response();
        }
    }

    // Fallback: return a stub scene
    let scene = GraphSceneModel {
        generation: 1,
        level: ViewLevel::System,
        layout_strategy: LayoutStrategy::DeterministicOrbital,
        nodes: vec![SceneNode {
            id: "cbu".into(),
            label,
            node_type: SceneNodeType::Cbu,
            state: Some("filled".into()),
            progress: 0,
            blocking: false,
            depth: 0,
            position_hint: Some((0.0, 0.0)),
            badges: vec![],
            child_count: 0,
            group_id: None,
        }],
        edges: vec![],
        groups: vec![],
        drill_targets: vec![],
        max_depth: 0,
    };

    Json(scene).into_response()
}

/// Attempt to hydrate a constellation for a CBU. Returns Err if no map found.
///
/// Uses the existing constellation runtime path (`handle_constellation_hydrate`)
/// with the workspace default map name from `WorkspaceKind::Cbu`'s registry entry,
/// rather than hardcoding jurisdiction-to-map selection.
async fn try_hydrate_cbu(
    pool: &sqlx::PgPool,
    cbu_id: Uuid,
) -> anyhow::Result<crate::sem_os_runtime::constellation_runtime::HydratedConstellation> {
    use crate::repl::types_v2::WorkspaceKind;
    use crate::sem_os_runtime::constellation_runtime::handle_constellation_hydrate;

    let default_map = WorkspaceKind::Cbu
        .registry_entry()
        .default_constellation_map;

    handle_constellation_hydrate(pool, cbu_id, None, default_map).await
}

/// GET /api/observatory/session/:id/diagrams/:diagram_type
///
/// Returns a Mermaid diagram string. Phase 8: star charts.
async fn get_diagram(
    State(_state): State<ObservatoryState>,
    Path((_session_id, diagram_type)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    // Phase 8: call existing render_erd(), render_verb_flow(), etc.
    let diagram = match diagram_type.as_str() {
        "erd" => "erDiagram\n    %% Placeholder ERD",
        "verb_flow" => "graph LR\n    %% Placeholder verb flow",
        "domain_map" => "graph TD\n    %% Placeholder domain map",
        "discovery_map" => "graph TD\n    %% Placeholder discovery map",
        _ => return ObservatoryError::not_found("Unknown diagram type").into_response(),
    };

    let response = DiagramResponse {
        diagram_type,
        mermaid: diagram.to_string(),
    };
    Json(response).into_response()
}

/// Mermaid diagram response.
#[derive(Debug, Serialize)]
struct DiagramResponse {
    diagram_type: String,
    mermaid: String,
}

// ── Navigation types ─────────────────────────────────────────

/// Request body for POST /session/:id/navigate.
#[derive(Debug, Deserialize)]
struct NavigateRequest {
    verb: String,
    args: serde_json::Value,
}

/// Response body for POST /session/:id/navigate.
#[derive(Debug, Serialize)]
struct NavigateResponse {
    orientation: OrientationContract,
    graph_scene: ob_poc_types::graph_scene::GraphSceneModel,
}

/// POST /api/observatory/session/:id/navigate
///
/// Executes a navigation verb and returns the updated orientation + graph scene.
/// Body: `{ "verb": "nav.drill", "args": { "target_id": "depositary", "target_level": "planet" } }`
async fn navigate(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<NavigateRequest>,
) -> impl IntoResponse {
    use sem_os_core::observatory::graph_scene_projection::{self, GraphEdgeProjection, SlotProjection};

    // 1. Validate session exists and extract scope
    let sessions = state.sessions.read().await;
    let session = match sessions.get(&session_id) {
        Some(s) => s,
        None => return ObservatoryError::not_found("Session not found").into_response(),
    };

    let cbu_ids: Vec<Uuid> = session.entity_scope.cbu_ids.iter().copied().collect();
    let label = session
        .target_universe
        .as_ref()
        .map(|u| u.description.clone());
    drop(sessions);

    let focus = default_focus(session_id);

    // 2. Determine the target ViewLevel based on the navigation verb
    let (target_level, entry_reason) = match request.verb.as_str() {
        "nav.drill" => {
            let target_id = request
                .args
                .get("target_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let target_level_str = request
                .args
                .get("target_level")
                .and_then(|v| v.as_str())
                .unwrap_or("planet");
            let level = parse_view_level(target_level_str);
            (
                level,
                EntryReason::DrillDown {
                    from_level: ViewLevel::System,
                    from_id: target_id,
                },
            )
        }
        "nav.zoom-out" => {
            // Default current level to System; compute parent
            let current_level = ViewLevel::System;
            let parent = current_level.parent().unwrap_or(ViewLevel::Universe);
            (parent, EntryReason::DirectNavigation)
        }
        "nav.select" => {
            // Focus change — re-project at current level (default System)
            (ViewLevel::System, EntryReason::DirectNavigation)
        }
        "nav.history-back" => (
            ViewLevel::System,
            EntryReason::HistoryReplay {
                direction: "back".into(),
            },
        ),
        "nav.history-forward" => (
            ViewLevel::System,
            EntryReason::HistoryReplay {
                direction: "forward".into(),
            },
        ),
        // nav.set-cluster-type, nav.set-lens — lens/client-side state, return current orientation
        _ => (ViewLevel::System, EntryReason::DirectNavigation),
    };

    // 3. Re-compute orientation
    let orientation = projection::project_orientation(
        None,
        &focus,
        target_level,
        AgentMode::Governed,
        entry_reason,
        label.as_deref(),
    );

    // 4. Re-hydrate graph scene
    let galaxy_level = to_galaxy_view_level(target_level);
    let graph_scene = if let Some(cbu_id) = cbu_ids.first() {
        if let Ok(hydrated) = try_hydrate_cbu(&state.pool, *cbu_id).await {
            let slots: Vec<SlotProjection> = hydrated
                .slots
                .iter()
                .map(|slot| SlotProjection {
                    name: slot.name.clone(),
                    path: slot.path.clone(),
                    slot_type: serde_json::to_value(&slot.slot_type)
                        .ok()
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_else(|| "entity".into()),
                    computed_state: slot.computed_state.clone(),
                    progress: slot.progress,
                    blocking: slot.blocking,
                    depth: slot.path.matches('.').count(),
                    parent_path: slot.path.rsplit_once('.').map(|(p, _)| p.to_string()),
                    child_count: slot.children.len(),
                    depends_on: vec![],
                    graph_edges: slot
                        .graph_edges
                        .iter()
                        .map(|e| GraphEdgeProjection {
                            from_id: e.from_entity_id.to_string(),
                            to_id: e.to_entity_id.to_string(),
                            edge_type: e
                                .ownership_type
                                .clone()
                                .unwrap_or_else(|| "ownership".into()),
                            label: e.percentage.map(|p| format!("{:.0}%", p)),
                            weight: e.percentage.unwrap_or(0.0) as f32,
                        })
                        .collect(),
                })
                .collect();

            graph_scene_projection::project_graph_scene(
                &hydrated.constellation,
                &hydrated.jurisdiction,
                &cbu_id.to_string(),
                &slots,
                galaxy_level,
                1,
            )
        } else {
            stub_graph_scene(label.as_deref(), galaxy_level)
        }
    } else {
        stub_graph_scene(label.as_deref(), galaxy_level)
    };

    // 5. Record in navigation history
    {
        let mut histories = state.nav_history.write().await;
        let hist = histories.entry(session_id).or_default();

        match request.verb.as_str() {
            "nav.history-back" => {
                if hist.cursor > 0 {
                    hist.cursor -= 1;
                }
                // Return the orientation at the cursor position instead
                if let Some(cached) = hist.entries.get(hist.cursor) {
                    return Json(NavigateResponse {
                        orientation: cached.clone(),
                        graph_scene,
                    })
                    .into_response();
                }
            }
            "nav.history-forward" => {
                if hist.cursor + 1 < hist.entries.len() {
                    hist.cursor += 1;
                }
                if let Some(cached) = hist.entries.get(hist.cursor) {
                    return Json(NavigateResponse {
                        orientation: cached.clone(),
                        graph_scene,
                    })
                    .into_response();
                }
            }
            _ => {
                // New navigation: truncate forward history and push
                if hist.cursor + 1 < hist.entries.len() {
                    hist.entries.truncate(hist.cursor + 1);
                }
                hist.entries.push(orientation.clone());
                hist.cursor = hist.entries.len() - 1;
            }
        }
    }

    // 6. Return both
    Json(NavigateResponse {
        orientation,
        graph_scene,
    })
    .into_response()
}

/// Parse a ViewLevel string (from JSON args) into a sem_os_core ViewLevel.
fn parse_view_level(s: &str) -> ViewLevel {
    match s {
        "universe" => ViewLevel::Universe,
        "cluster" => ViewLevel::Cluster,
        "system" => ViewLevel::System,
        "planet" => ViewLevel::Planet,
        "surface" => ViewLevel::Surface,
        "core" => ViewLevel::Core,
        _ => ViewLevel::System,
    }
}

/// Convert sem_os_core ViewLevel to ob-poc-types galaxy ViewLevel.
fn to_galaxy_view_level(level: ViewLevel) -> ob_poc_types::galaxy::ViewLevel {
    match level {
        ViewLevel::Universe => ob_poc_types::galaxy::ViewLevel::Universe,
        ViewLevel::Cluster => ob_poc_types::galaxy::ViewLevel::Cluster,
        ViewLevel::System => ob_poc_types::galaxy::ViewLevel::System,
        ViewLevel::Planet => ob_poc_types::galaxy::ViewLevel::Planet,
        ViewLevel::Surface => ob_poc_types::galaxy::ViewLevel::Surface,
        ViewLevel::Core => ob_poc_types::galaxy::ViewLevel::Core,
    }
}

/// Build a stub graph scene when no constellation is available.
fn stub_graph_scene(
    label: Option<&str>,
    level: ob_poc_types::galaxy::ViewLevel,
) -> ob_poc_types::graph_scene::GraphSceneModel {
    use ob_poc_types::graph_scene::*;

    GraphSceneModel {
        generation: 1,
        level,
        layout_strategy: LayoutStrategy::DeterministicOrbital,
        nodes: vec![SceneNode {
            id: "cbu".into(),
            label: label.unwrap_or("Observatory").into(),
            node_type: SceneNodeType::Cbu,
            state: Some("filled".into()),
            progress: 0,
            blocking: false,
            depth: 0,
            position_hint: Some((0.0, 0.0)),
            badges: vec![],
            child_count: 0,
            group_id: None,
        }],
        edges: vec![],
        groups: vec![],
        drill_targets: vec![],
        max_depth: 0,
    }
}

/// Create the Observatory router.
pub fn create_observatory_router(pool: PgPool, sessions: SessionStore) -> Router {
    let state = ObservatoryState {
        pool,
        sessions,
        nav_history: Arc::new(RwLock::new(HashMap::new())),
    };

    Router::new()
        .route("/session/:id/orientation", get(get_orientation))
        .route("/session/:id/show-packet", get(get_show_packet))
        .route(
            "/session/:id/navigation-history",
            get(get_navigation_history),
        )
        .route("/session/:id/graph-scene", get(get_graph_scene))
        .route("/session/:id/navigate", post(navigate))
        .route("/session/:id/diagrams/:diagram_type", get(get_diagram))
        .route("/health", get(get_health_metrics))
        .with_state(state)
}

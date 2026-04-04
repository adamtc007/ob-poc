//! Observatory REST API routes.
//!
//! Exposes OrientationContract and ShowPacket data for the Observatory UI.
//! All endpoints project from existing SemOS types — no new queries.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sqlx::PgPool;
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

/// Route state for observatory endpoints.
#[derive(Clone)]
pub struct ObservatoryState {
    pub pool: PgPool,
    pub sessions: SessionStore,
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

/// GET /api/observatory/session/:id/navigation-history
///
/// Returns the navigation history as OrientationContract sequence.
/// Phase 1: returns current orientation only (history tracking in Phase 4).
async fn get_navigation_history(
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

    Json(vec![contract]).into_response()
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

/// Create the Observatory router.
pub fn create_observatory_router(pool: PgPool, sessions: SessionStore) -> Router {
    let state = ObservatoryState { pool, sessions };

    Router::new()
        .route("/session/:id/orientation", get(get_orientation))
        .route("/session/:id/show-packet", get(get_show_packet))
        .route(
            "/session/:id/navigation-history",
            get(get_navigation_history),
        )
        .route("/session/:id/graph-scene", get(get_graph_scene))
        .route("/session/:id/diagrams/:diagram_type", get(get_diagram))
        .route("/health", get(get_health_metrics))
        .with_state(state)
}

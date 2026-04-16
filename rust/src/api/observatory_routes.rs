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
use crate::repl::session_v2::ReplSessionV2;
use crate::sem_os_runtime::constellation_runtime::HydratedSlot;
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
///
/// DEPRECATED: Navigation history should live on WorkspaceFrame.nav_snapshots.
/// This standalone HashMap is a side exit (SX-3 in the audit). The frontend
/// now routes all navigation through POST /session/:id/input, so this store
/// is only populated by the legacy POST /navigate endpoint. It will be removed
/// when /navigate is fully decommissioned.
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

/// Thread-safe store for REPL V2 sessions (canonical DAG source).
pub type ReplSessionStore = Arc<RwLock<HashMap<Uuid, ReplSessionV2>>>;

/// Route state for observatory endpoints.
#[derive(Clone)]
pub struct ObservatoryState {
    pub pool: PgPool,
    pub sessions: SessionStore,
    /// REPL V2 session store — the canonical source for hydrated constellation DAG.
    /// Observatory endpoints read `tos.hydrated_state` from here.
    pub repl_sessions: Option<ReplSessionStore>,
    pub nav_history: NavigationHistory,
}

/// Build a FocusState from session ID (default empty focus).
///
/// TRANSITIONAL: Used only by ShowLoop viewport computation (get_show_packet) and
/// the navigate() handler. NOT used by get_orientation() or get_graph_scene() which
/// read from the session's hydrated DAG. This function should be removed when
/// ShowLoop viewports are unified and /navigate is collapsed into /session/:id/input.
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
///
/// TRANSITIONAL: Used only by the legacy fallback path in orientation_from_repl_or_legacy().
/// Will be removed when all sessions have REPL V2 sessions.
fn business_label_from_session(
    sessions: &std::collections::HashMap<Uuid, crate::session::UnifiedSession>,
    session_id: Uuid,
) -> Option<String> {
    sessions
        .get(&session_id)
        .and_then(|s| s.target_universe.as_ref())
        .map(|u| u.description.clone())
}

// ── DAG identity projection ─────────────────────────────────
//
// These functions project OrientationContract from the REPL session's
// hydrated constellation DAG — the same DAG the compiler and narration
// engine read. No independent hydration. No parallel state.

/// Project an OrientationContract from a REPL session's TOS hydrated state.
///
/// Reads `tos.hydrated_state` (resource state / the DAG) to populate
/// `available_actions`. Falls back gracefully when TOS is absent or
/// not yet hydrated (pre-workspace-selection).
fn project_orientation_from_repl_session(session: &ReplSessionV2) -> OrientationContract {
    let tos = session.workspace_stack.last();
    let hydrated = tos.and_then(|f| f.hydrated_state.as_ref());
    let constellation = hydrated.and_then(|h| h.hydrated_constellation.as_ref());

    // available_actions from HydratedSlot.available_verbs — same source the compiler uses.
    // When no constellation is hydrated (session start), populate with bootstrap scoping verbs.
    let available_actions: Vec<ActionDescriptor> = constellation
        .map(|c| collect_actions_from_slots(&c.slots))
        .unwrap_or_else(universe_root_action_descriptors);

    // Focus identity from TOS workspace context
    let (focus_kind, focus_identity) = derive_focus_from_tos(tos, constellation);

    // Business label from session universe description
    let _business_label = tos
        .and_then(|f| f.hydrated_state.as_ref())
        .and_then(|h| h.subject_ref.as_ref())
        .map(|sr| format!("{:?}:{}", sr.kind, sr.id));

    // View level from TOS viewport state (set by nav.drill/nav.zoom-out)
    let view_level = tos
        .map(|f| to_orientation_view_level(f.view_level))
        .unwrap_or(ViewLevel::Universe);

    // Lens — defaults until Phase 3 adds viewport state
    let lens = LensState {
        overlay: OverlayState::ActiveOnly,
        depth_probe: None,
        cluster_mode: ClusterMode::Jurisdiction,
        active_filters: vec![],
    };

    // Map REPL AgentMode (Sage|Repl) to Observatory AgentMode (Governed|Research|Maintenance).
    // The REPL mode describes the UI shell; the Observatory mode describes the governance tier.
    // Default to Governed — the orchestrator's SemOS context resolution determines the real mode.
    let observatory_mode = AgentMode::Governed;

    OrientationContract {
        session_mode: observatory_mode,
        view_level,
        focus_kind,
        focus_identity,
        scope: projection::scope_from_level(view_level),
        lens,
        entry_reason: EntryReason::SessionStart,
        available_actions,
        delta_from_previous: None,
        computed_at: chrono::Utc::now(),
    }
}

/// Walk the HydratedSlot tree and collect available/blocked verbs as ActionDescriptors.
/// This reads the SAME `available_verbs` and `blocked_verbs` fields that the compiler
/// uses via `discover_advancing_slots()` and the narration engine reads for gap analysis.
fn collect_actions_from_slots(slots: &[HydratedSlot]) -> Vec<ActionDescriptor> {
    let mut actions = Vec::new();
    collect_actions_recursive(slots, &mut actions);
    // Sort by enabled (true first), then by action_id for stability
    actions.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.action_id.cmp(&b.action_id))
    });
    actions
}

fn collect_actions_recursive(slots: &[HydratedSlot], actions: &mut Vec<ActionDescriptor>) {
    for slot in slots {
        // Enabled verbs from state machine transitions
        for verb in &slot.available_verbs {
            // Avoid duplicates (same verb may appear on multiple slots)
            if !actions.iter().any(|a| a.action_id == *verb) {
                actions.push(ActionDescriptor {
                    action_id: verb.clone(),
                    label: verb.clone(),
                    action_kind: "primitive".into(),
                    enabled: true,
                    disabled_reason: None,
                    rank_score: 1.0,
                });
            }
        }
        // Blocked verbs with reasons
        for blocked in &slot.blocked_verbs {
            let verb_id = &blocked.verb;
            if !actions.iter().any(|a| a.action_id == *verb_id) {
                let reason = blocked
                    .reasons
                    .iter()
                    .map(|r| r.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                actions.push(ActionDescriptor {
                    action_id: verb_id.clone(),
                    label: verb_id.clone(),
                    action_kind: "primitive".into(),
                    enabled: false,
                    disabled_reason: Some(reason),
                    rank_score: 0.0,
                });
            }
        }
        // Recurse into children
        collect_actions_recursive(&slot.children, actions);
    }
}

/// Derive focus kind and identity from the TOS workspace frame and constellation.
fn derive_focus_from_tos(
    tos: Option<&crate::repl::types_v2::WorkspaceFrame>,
    constellation: Option<&crate::sem_os_runtime::constellation_runtime::HydratedConstellation>,
) -> (FocusKind, FocusIdentity) {
    if let Some(frame) = tos {
        let kind = match &frame.workspace {
            crate::repl::types_v2::WorkspaceKind::Cbu => FocusKind::Cbu,
            crate::repl::types_v2::WorkspaceKind::Kyc => FocusKind::Case,
            crate::repl::types_v2::WorkspaceKind::Deal => FocusKind::Other("deal".into()),
            crate::repl::types_v2::WorkspaceKind::SemOsMaintenance => FocusKind::Constellation,
            _ => FocusKind::Constellation,
        };

        let label = constellation
            .map(|c| c.constellation.clone())
            .or_else(|| {
                frame
                    .hydrated_state
                    .as_ref()
                    .map(|h| h.constellation_map.clone())
            })
            .unwrap_or_else(|| frame.workspace.label().to_string());

        let canonical_id = frame
            .subject_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| frame.constellation_map.clone());

        let identity = FocusIdentity {
            canonical_id,
            business_label: label,
            object_type: Some(frame.workspace.label().to_string()),
        };
        (kind, identity)
    } else {
        // No workspace selected — session-level focus
        (
            FocusKind::Constellation,
            FocusIdentity {
                canonical_id: "session".into(),
                business_label: "Session".into(),
                object_type: None,
            },
        )
    }
}

/// Try to read a ReplSessionV2 and project orientation from it.
/// Falls back to legacy projection if REPL session is not available.
async fn orientation_from_repl_or_legacy(
    state: &ObservatoryState,
    session_id: Uuid,
) -> OrientationContract {
    // Try REPL session first (canonical DAG source)
    if let Some(ref repl_sessions) = state.repl_sessions {
        let sessions = repl_sessions.read().await;
        if let Some(repl_session) = sessions.get(&session_id) {
            return project_orientation_from_repl_session(repl_session);
        }
    }

    // Fallback: legacy projection from UnifiedSession (empty actions)
    let sessions = state.sessions.read().await;
    let label = business_label_from_session(&sessions, session_id);
    let focus = default_focus(session_id);
    projection::project_orientation(
        None,
        &focus,
        ViewLevel::Universe,
        AgentMode::Governed,
        EntryReason::SessionStart,
        label.as_deref(),
    )
}

/// GET /api/observatory/session/:id/orientation
///
/// Returns the current OrientationContract for a session.
/// Projects from the REPL session's TOS hydrated state (the canonical DAG).
async fn get_orientation(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    // Verify session exists in either store
    let exists = {
        let sessions = state.sessions.read().await;
        sessions.contains_key(&session_id)
    };
    if !exists {
        return ObservatoryError::not_found("Session not found").into_response();
    }

    let contract = orientation_from_repl_or_legacy(&state, session_id).await;
    Json(contract).into_response()
}

/// GET /api/observatory/session/:id/show-packet
///
/// Returns the full ShowPacket with orientation for a session.
///
/// TRANSITIONAL: The orientation field is projected from the session's hydrated DAG
/// (canonical). The viewport computation (Focus, Object, Diff, Gates) still reads from
/// FocusState + SemReg snapshots independently. This is acceptable because those viewports
/// render SemReg object detail (not constellation slot state). See audit doc SE-12/SE-13.
async fn get_show_packet(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    let exists = {
        let sessions = state.sessions.read().await;
        sessions.contains_key(&session_id)
    };
    if !exists {
        return ObservatoryError::not_found("Session not found").into_response();
    }

    // Orientation: projected from session's hydrated DAG (canonical)
    let contract = orientation_from_repl_or_legacy(&state, session_id).await;

    // ShowLoop viewports: still read from FocusState + SemReg snapshots (transitional)
    let focus = default_focus(session_id);
    match ShowLoop::compute_show_packet(&state.pool, &focus, "system", None).await {
        Ok(mut packet) => {
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
async fn get_health_metrics(State(state): State<ObservatoryState>) -> impl IntoResponse {
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
        "SELECT COUNT(*) FROM sem_reg.snapshots WHERE status = 'active'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let archived = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sem_reg.changesets WHERE status = 'archived'",
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
/// Returns the GraphSceneModel — projected from the session's hydrated constellation DAG.
/// Reads from `tos.hydrated_state.hydrated_constellation` (same data the compiler reads).
/// Falls back to independent hydration only when TOS is not yet hydrated.
async fn get_graph_scene(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    use ob_poc_types::galaxy::ViewLevel;
    use sem_os_core::observatory::graph_scene_projection;

    // 1. Try to read from REPL session's TOS hydrated constellation (canonical DAG)
    if let Some(ref repl_sessions) = state.repl_sessions {
        let sessions = repl_sessions.read().await;
        if let Some(repl_session) = sessions.get(&session_id) {
            let tos_constellation = repl_session
                .workspace_stack
                .last()
                .and_then(|f| f.hydrated_state.as_ref())
                .and_then(|h| h.hydrated_constellation.as_ref());

            if let Some(constellation) = tos_constellation {
                // Read view level from TOS viewport state
                let view_level = repl_session
                    .workspace_stack
                    .last()
                    .map(|f| f.view_level)
                    .unwrap_or(ViewLevel::System);

                // Project from session DAG — same data the compiler reads
                let slots = slots_from_hydrated(&constellation.slots);
                let scene = graph_scene_projection::project_graph_scene(
                    &constellation.constellation,
                    &constellation.jurisdiction,
                    &constellation.cbu_id.to_string(),
                    &slots,
                    view_level,
                    1,
                );
                return Json(scene).into_response();
            }
        }
    }

    // 2. Fallback: no TOS hydration yet (pre-workspace-selection).
    // TOCTOU note: we dropped the sessions read guard above, so another request could
    // trigger rehydration between the drop and this fallback. This is SAFE because the
    // fallback only fires when hydrated_state is None — i.e., no workspace is selected
    // yet, meaning there is no constellation to be stale about. The worst case is that
    // we hydrate a CBU that was just loaded by a concurrent request, producing a valid
    // (if redundant) result.
    let sessions = state.sessions.read().await;
    let session = match sessions.get(&session_id) {
        Some(s) => s,
        None => return ObservatoryError::not_found("Session not found").into_response(),
    };

    let cbu_ids: Vec<Uuid> = session.entity_scope.cbu_ids.iter().copied().collect();
    let label = session
        .target_universe
        .as_ref()
        .map(|u| u.description.clone())
        .unwrap_or_else(|| "Observatory".into());
    drop(sessions);

    if let Some(cbu_id) = cbu_ids.first() {
        if let Ok(hydrated) = try_hydrate_cbu(&state.pool, *cbu_id).await {
            let slots = slots_from_hydrated(&hydrated.slots);
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

    // 3. No CBU in scope — return universe scene showing all workspaces
    Json(universe_graph_scene(Some(&label))).into_response()
}

/// GET /api/observatory/session/:id/session-stack-graph
///
/// Returns a GraphSceneModel projected from the canonical SessionStackState.
async fn get_session_stack_graph(
    State(state): State<ObservatoryState>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    use ob_poc_types::graph_scene::{
        GraphSceneModel, LayoutStrategy, SceneEdge, SceneEdgeType, SceneNode, SceneNodeType,
    };

    let Some(repl_sessions) = state.repl_sessions.as_ref() else {
        return ObservatoryError::not_found("REPL session store not configured").into_response();
    };

    let sessions = repl_sessions.read().await;
    let Some(repl_session) = sessions.get(&session_id) else {
        return ObservatoryError::not_found("REPL session not found").into_response();
    };

    let stack = &repl_session.session_stack;
    let mut nodes = Vec::with_capacity(stack.workspace_stack.len() + 1);
    let mut edges = Vec::with_capacity(stack.workspace_stack.len());

    nodes.push(SceneNode {
        id: format!("session:{}", stack.session_id),
        label: stack
            .scope
            .as_ref()
            .and_then(|scope| scope.client_group_name.clone())
            .unwrap_or_else(|| format!("Session {}", stack.session_id)),
        node_type: SceneNodeType::Aggregate,
        state: stack
            .active_workspace
            .as_ref()
            .map(|ws| format!("{:?}", ws)),
        progress: 0,
        blocking: false,
        depth: 0,
        position_hint: None,
        badges: vec![],
        child_count: stack.workspace_stack.len(),
        group_id: None,
    });

    for (index, frame) in stack.workspace_stack.iter().enumerate() {
        let node_id = format!("frame:{index}:{}", frame.constellation_map);
        let label = match frame.subject_id {
            Some(subject_id) => format!("{:?} {}", frame.workspace, subject_id),
            None => format!("{:?}", frame.workspace),
        };
        let mut badges = Vec::new();
        if frame.is_peek {
            badges.push(ob_poc_types::graph_scene::SceneBadge {
                badge_type: "peek".into(),
                label: "peek".into(),
                color: Some("#8892a0".into()),
            });
        }
        if frame.stale {
            badges.push(ob_poc_types::graph_scene::SceneBadge {
                badge_type: "stale".into(),
                label: "stale".into(),
                color: Some("#d97706".into()),
            });
        }

        nodes.push(SceneNode {
            id: node_id.clone(),
            label,
            node_type: SceneNodeType::Cluster,
            state: Some(frame.constellation_map.clone()),
            progress: if frame.stale { 0 } else { 100 },
            blocking: frame.stale,
            depth: index + 1,
            position_hint: None,
            badges,
            child_count: usize::from(index + 1 < stack.workspace_stack.len()),
            group_id: None,
        });

        edges.push(SceneEdge {
            source: if index == 0 {
                format!("session:{}", stack.session_id)
            } else {
                format!(
                    "frame:{}:{}",
                    index - 1,
                    stack.workspace_stack[index - 1].constellation_map
                )
            },
            target: node_id,
            edge_type: SceneEdgeType::Dependency,
            label: Some(if index == 0 {
                "root".into()
            } else {
                "push".into()
            }),
            weight: 1.0,
        });
    }

    Json(GraphSceneModel {
        generation: stack.trace_sequence,
        level: ob_poc_types::galaxy::ViewLevel::System,
        layout_strategy: LayoutStrategy::HierarchicalGraph,
        nodes,
        edges,
        groups: vec![],
        drill_targets: vec![],
        max_depth: stack.workspace_stack.len(),
    })
    .into_response()
}

/// Project HydratedSlot tree into SlotProjection vec for graph_scene_projection.
/// This is the single conversion point — used by both the TOS path and fallback path.
fn slots_from_hydrated(
    slots: &[HydratedSlot],
) -> Vec<sem_os_core::observatory::graph_scene_projection::SlotProjection> {
    let mut result = Vec::new();
    flatten_slots_recursive(slots, &mut result);
    result
}

fn flatten_slots_recursive(
    slots: &[HydratedSlot],
    result: &mut Vec<sem_os_core::observatory::graph_scene_projection::SlotProjection>,
) {
    use sem_os_core::observatory::graph_scene_projection::GraphEdgeProjection;
    use sem_os_core::observatory::graph_scene_projection::SlotProjection;
    for slot in slots {
        result.push(SlotProjection {
            name: slot.name.clone(),
            path: slot.path.clone(),
            slot_type: serde_json::to_value(slot.slot_type)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "entity".into()),
            computed_state: slot.computed_state.clone(),
            progress: slot.progress,
            blocking: slot.blocking,
            depth: slot.path.matches('.').count(),
            parent_path: slot.path.rsplit_once('.').map(|(p, _)| p.to_string()),
            child_count: slot.children.len(),
            depends_on: vec![], // TODO: populate from SlotDef.depends_on when available on HydratedSlot
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
        });
        // Recurse into children
        flatten_slots_recursive(&slot.children, result);
    }
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
    use sem_os_core::observatory::graph_scene_projection::{
        self, GraphEdgeProjection, SlotProjection,
    };

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
                    slot_type: serde_json::to_value(slot.slot_type)
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
            universe_graph_scene(label.as_deref())
        }
    } else {
        universe_graph_scene(label.as_deref())
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

/// Convert ob-poc-types galaxy ViewLevel to sem_os_core orientation ViewLevel.
fn to_orientation_view_level(level: ob_poc_types::galaxy::ViewLevel) -> ViewLevel {
    match level {
        ob_poc_types::galaxy::ViewLevel::Universe => ViewLevel::Universe,
        ob_poc_types::galaxy::ViewLevel::Cluster => ViewLevel::Cluster,
        ob_poc_types::galaxy::ViewLevel::System => ViewLevel::System,
        ob_poc_types::galaxy::ViewLevel::Planet => ViewLevel::Planet,
        ob_poc_types::galaxy::ViewLevel::Surface => ViewLevel::Surface,
        ob_poc_types::galaxy::ViewLevel::Core => ViewLevel::Core,
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
fn universe_root_action_descriptors() -> Vec<ActionDescriptor> {
    let scoping_verbs = [
        ("session.start", "New Session"),
        ("session.resume", "Resume Session"),
        ("session.load-cbu", "Load Client Group"),
        ("session.load-galaxy", "Load Galaxy"),
        ("session.load-jurisdiction", "Load Jurisdiction"),
        ("client-group.search", "Search Client Groups"),
        ("gleif.search", "Search LEI Registry"),
        ("session.info", "Session Info"),
    ];
    scoping_verbs
        .iter()
        .enumerate()
        .map(|(i, (id, label))| ActionDescriptor {
            action_id: id.to_string(),
            label: label.to_string(),
            action_kind: "scope".into(),
            enabled: true,
            disabled_reason: None,
            rank_score: 1.0 - (i as f64 * 0.05),
        })
        .collect()
}

fn universe_graph_scene(
    label: Option<&str>,
) -> ob_poc_types::graph_scene::GraphSceneModel {
    use crate::repl::types_v2::WorkspaceKind;
    use ob_poc_types::graph_scene::*;

    let root_label = label.unwrap_or("Universe");
    let workspaces = WorkspaceKind::all();

    let satellite_count = workspaces.len() + 1; // workspaces + "New Session"
    let mut nodes = Vec::with_capacity(1 + satellite_count);
    let mut edges = Vec::with_capacity(satellite_count);

    nodes.push(SceneNode {
        id: "universe".into(),
        label: root_label.into(),
        node_type: SceneNodeType::Aggregate,
        state: Some("active".into()),
        progress: 0,
        blocking: false,
        depth: 0,
        position_hint: Some((0.0, 0.0)),
        badges: vec![],
        child_count: satellite_count,
        group_id: None,
    });

    for ws in &workspaces {
        let registry = ws.registry_entry();
        let ws_id = format!("workspace:{}", ws.label());
        nodes.push(SceneNode {
            id: ws_id.clone(),
            label: registry.display_name.to_string(),
            node_type: SceneNodeType::Cluster,
            state: Some("available".into()),
            progress: 0,
            blocking: false,
            depth: 1,
            position_hint: None,
            badges: vec![],
            child_count: 0,
            group_id: None,
        });
        edges.push(SceneEdge {
            source: "universe".into(),
            target: ws_id.clone(),
            edge_type: SceneEdgeType::ParentChild,
            label: Some(ws.label().to_string()),
            weight: 1.0,
        });
    }

    // "New Session" satellite — same verb profile as SemOS Maintenance
    let new_session_id = "workspace:new-session".to_string();
    nodes.push(SceneNode {
        id: new_session_id.clone(),
        label: "New Session".into(),
        node_type: SceneNodeType::Cluster,
        state: Some("available".into()),
        progress: 0,
        blocking: false,
        depth: 1,
        position_hint: None,
        badges: vec![],
        child_count: 0,
        group_id: None,
    });
    edges.push(SceneEdge {
        source: "universe".into(),
        target: new_session_id,
        edge_type: SceneEdgeType::ParentChild,
        label: Some("new-session".into()),
        weight: 1.0,
    });

    let mut drill_targets: Vec<DrillTarget> = workspaces
        .iter()
        .map(|ws| DrillTarget {
            node_id: format!("workspace:{}", ws.label()),
            target_level: ob_poc_types::galaxy::ViewLevel::System,
            drill_label: ws.registry_entry().display_name.to_string(),
        })
        .collect();
    drill_targets.push(DrillTarget {
        node_id: "workspace:new-session".into(),
        target_level: ob_poc_types::galaxy::ViewLevel::System,
        drill_label: "New Session".into(),
    });

    GraphSceneModel {
        generation: 1,
        level: ob_poc_types::galaxy::ViewLevel::Universe,
        layout_strategy: LayoutStrategy::DeterministicOrbital,
        nodes,
        edges,
        groups: vec![],
        drill_targets,
        max_depth: 1,
    }
}

/// Create the Observatory router.
///
/// `repl_sessions` is the REPL V2 session store — the canonical source for hydrated
/// constellation DAG. When provided, Observatory endpoints read `tos.hydrated_state`
/// from here instead of building parallel state. Pass `None` only in tests or when
/// the REPL V2 orchestrator is not available.
pub fn create_observatory_router(
    pool: PgPool,
    sessions: SessionStore,
    repl_sessions: Option<ReplSessionStore>,
) -> Router {
    let state = ObservatoryState {
        pool,
        sessions,
        repl_sessions,
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
        .route(
            "/session/:id/session-stack-graph",
            get(get_session_stack_graph),
        )
        .route("/session/:id/navigate", post(navigate))
        .route("/session/:id/diagrams/:diagram_type", get(get_diagram))
        .route("/health", get(get_health_metrics))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::universe_graph_scene;
    use crate::repl::types_v2::WorkspaceKind;
    use ob_poc_types::galaxy::ViewLevel;
    use ob_poc_types::graph_scene::LayoutStrategy;

    #[test]
    fn universe_graph_scene_includes_all_startup_satellites() {
        let scene = universe_graph_scene(Some("Universe"));

        assert_eq!(scene.level, ViewLevel::Universe);
        assert_eq!(scene.layout_strategy, LayoutStrategy::DeterministicOrbital);
        assert_eq!(scene.nodes.len(), WorkspaceKind::all().len() + 2);
        assert_eq!(scene.drill_targets.len(), WorkspaceKind::all().len() + 1);
        assert!(scene.nodes.iter().any(|node| node.id == "workspace:new-session"));
        assert!(scene
            .drill_targets
            .iter()
            .any(|target| target.node_id == "workspace:new-session"
                && target.target_level == ViewLevel::System));
    }
}

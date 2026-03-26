//! V2 REPL API Routes — Pack-Guided Runbook Pipeline
//!
//! Coexists with `/api/repl/*` (v1 state machine routes).
//! These endpoints wrap `ReplOrchestratorV2` which uses the pack-guided
//! 7-state machine with sentence-first responses.
//!
//! ## Endpoints
//!
//! - `POST /api/repl/v2/session`           — Create session
//! - `GET  /api/repl/v2/session/:id`       — Get session state
//! - `POST /api/repl/v2/session/:id/input` — Legacy input endpoint (410 Gone)
//! - `DELETE /api/repl/v2/session/:id`      — Delete session
//! - `POST /api/repl/v2/signal`            — External system signals completion of a parked entry

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::constellation_routes::{hydrate_workspace_state, resolve_context};
use crate::repl::orchestrator_v2::ReplOrchestratorV2;
use crate::repl::response_v2::ReplResponseV2;
use crate::repl::session_v2::ReplSessionV2;
use crate::repl::types_v2::{
    ConstellationContextRef, ReplCommandV2, ReplStateV2, ResolvedConstellationContext,
    SessionFeedback, UserInputV2, WorkspaceFrame, WorkspaceKind,
};

// ============================================================================
// Route State
// ============================================================================

/// Shared state for V2 REPL routes.
#[derive(Clone)]
pub struct ReplV2RouteState {
    pub orchestrator: Arc<ReplOrchestratorV2>,
}

// ============================================================================
// Request / Response Types
// ============================================================================

/// Request to send input to a V2 session.
///
/// Maps to `UserInputV2` on the backend. Uses `#[serde(tag = "type")]`
/// so the client sends `{ "type": "message", "content": "..." }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputRequestV2 {
    /// Free-text conversational input.
    Message { content: String },

    /// User confirmed a sentence or runbook.
    Confirm,

    /// User rejected a proposed sentence.
    Reject,

    /// User edited a specific field on a runbook entry.
    Edit {
        step_id: Uuid,
        field: String,
        value: String,
    },

    /// Explicit REPL command.
    Command { command: String },

    /// User explicitly selected a pack by ID.
    SelectPack { pack_id: String },

    /// User selected a verb from disambiguation options.
    SelectVerb {
        verb_fqn: String,
        original_input: String,
    },

    /// User selected a proposal from the ranked list (Phase 3).
    SelectProposal { proposal_id: Uuid },

    /// User selected an entity to resolve an ambiguous reference.
    SelectEntity {
        ref_id: String,
        entity_id: Uuid,
        entity_name: String,
    },

    /// User selected a scope (client group / CBU set).
    SelectScope { group_id: Uuid, group_name: String },

    /// User selected a workspace.
    SelectWorkspace { workspace: WorkspaceKind },

    /// User approves a human-gated runbook entry.
    Approve {
        entry_id: Uuid,
        approved_by: Option<String>,
    },

    /// User rejects a human-gated runbook entry.
    RejectGate {
        entry_id: Uuid,
        reason: Option<String>,
    },
}

impl From<InputRequestV2> for UserInputV2 {
    fn from(req: InputRequestV2) -> Self {
        match req {
            InputRequestV2::Message { content } => UserInputV2::Message { content },
            InputRequestV2::Confirm => UserInputV2::Confirm,
            InputRequestV2::Reject => UserInputV2::Reject,
            InputRequestV2::Edit {
                step_id,
                field,
                value,
            } => UserInputV2::Edit {
                step_id,
                field,
                value,
            },
            InputRequestV2::Command { command } => {
                let lower = command.to_lowercase();
                let cmd = match lower.as_str() {
                    "run" => ReplCommandV2::Run,
                    "undo" => ReplCommandV2::Undo,
                    "redo" => ReplCommandV2::Redo,
                    "clear" => ReplCommandV2::Clear,
                    "cancel" => ReplCommandV2::Cancel,
                    "info" => ReplCommandV2::Info,
                    "help" => ReplCommandV2::Help,
                    "status" => ReplCommandV2::Status,
                    _ if lower.starts_with("disable ") => {
                        let id = Uuid::parse_str(lower.trim_start_matches("disable ").trim())
                            .unwrap_or_default();
                        ReplCommandV2::Disable(id)
                    }
                    _ if lower.starts_with("enable ") => {
                        let id = Uuid::parse_str(lower.trim_start_matches("enable ").trim())
                            .unwrap_or_default();
                        ReplCommandV2::Enable(id)
                    }
                    _ if lower.starts_with("toggle ") => {
                        let id = Uuid::parse_str(lower.trim_start_matches("toggle ").trim())
                            .unwrap_or_default();
                        ReplCommandV2::Toggle(id)
                    }
                    _ => ReplCommandV2::Help,
                };
                UserInputV2::Command { command: cmd }
            }
            InputRequestV2::SelectPack { pack_id } => UserInputV2::SelectPack { pack_id },
            InputRequestV2::SelectProposal { proposal_id } => {
                UserInputV2::SelectProposal { proposal_id }
            }
            InputRequestV2::SelectVerb {
                verb_fqn,
                original_input,
            } => UserInputV2::SelectVerb {
                verb_fqn,
                original_input,
            },
            InputRequestV2::SelectEntity {
                ref_id,
                entity_id,
                entity_name,
            } => UserInputV2::SelectEntity {
                ref_id,
                entity_id,
                entity_name,
            },
            InputRequestV2::SelectScope {
                group_id,
                group_name,
            } => UserInputV2::SelectScope {
                group_id,
                group_name,
            },
            InputRequestV2::SelectWorkspace { workspace } => {
                UserInputV2::SelectWorkspace { workspace }
            }
            InputRequestV2::Approve {
                entry_id,
                approved_by,
            } => UserInputV2::Approve {
                entry_id,
                approved_by,
            },
            InputRequestV2::RejectGate { entry_id, reason } => {
                UserInputV2::RejectGate { entry_id, reason }
            }
        }
    }
}

/// Response for session creation.
#[derive(Debug, Serialize)]
pub struct CreateSessionResponseV2 {
    pub session_id: Uuid,
    /// Full initial response including state + kind (ScopeRequired).
    pub response: ReplResponseV2,
}

/// Response for session state query.
#[derive(Debug, Serialize)]
pub struct SessionStateResponseV2 {
    pub session_id: Uuid,
    pub state: ReplStateV2,
    pub runbook_step_count: usize,
    pub created_at: String,
    pub last_active_at: String,
}

/// Request to push a new workspace context onto the session stack.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionPushRequest {
    pub context: ConstellationContextRef,
    #[serde(default)]
    pub peek: bool,
}

/// Request for stack commit/pop operations.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionStackRequest {
    pub session_id: Uuid,
}

/// Query for current session-scoped constellation context.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionContextQuery {
    pub session_id: Uuid,
}

/// Response returned by session stack routes.
#[derive(Debug, Clone, Serialize)]
pub struct SessionFeedbackResponse {
    pub resolved: ResolvedConstellationContext,
    pub feedback: SessionFeedback,
}

impl From<ReplSessionV2> for SessionStateResponseV2 {
    fn from(session: ReplSessionV2) -> Self {
        Self {
            session_id: session.id,
            state: session.state,
            runbook_step_count: session.runbook.entries.len(),
            created_at: session.created_at.to_rfc3339(),
            last_active_at: session.last_active_at.to_rfc3339(),
        }
    }
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponseV2 {
    pub error: String,
    pub recoverable: bool,
}

fn resolved_plan_subject_id(
    plan: &crate::runbook::plan_types::RunbookPlan,
    step_index: usize,
) -> Option<Uuid> {
    let step = plan.steps.get(step_index)?;
    match &step.subject_binding {
        crate::runbook::plan_types::EntityBinding::Literal { id } if *id != Uuid::nil() => {
            Some(*id)
        }
        crate::runbook::plan_types::EntityBinding::Literal { .. } => None,
        crate::runbook::plan_types::EntityBinding::ForwardRef { output_field, .. } => {
            plan.bindings.resolved.get(output_field).copied()
        }
    }
}

fn plan_step_context(
    session: &ReplSessionV2,
    plan: &crate::runbook::plan_types::RunbookPlan,
    step_index: usize,
) -> anyhow::Result<Option<ConstellationContextRef>> {
    let scope = session
        .session_scope()
        .ok_or_else(|| anyhow::anyhow!("Session has no client scope for runbook execution"))?;
    let step = plan
        .steps
        .get(step_index)
        .ok_or_else(|| anyhow::anyhow!("Runbook step {step_index} not found"))?;
    let subject_id = resolved_plan_subject_id(plan, step_index);
    let needs_transition = session.tos_frame().is_none_or(|tos| {
        tos.workspace != step.workspace
            || tos.constellation_map != step.constellation_map
            || tos.subject_id != subject_id
    });

    if !needs_transition {
        return Ok(None);
    }

    Ok(Some(ConstellationContextRef {
        session_id: session.id,
        client_group_id: scope.client_group_id,
        workspace: step.workspace.clone(),
        constellation_family: None,
        constellation_map: Some(step.constellation_map.clone()),
        subject_kind: Some(step.subject_kind.clone()),
        subject_id,
        handoff_context: None,
    }))
}

fn persist_completed_step_outputs(
    plan: &mut crate::runbook::plan_types::RunbookPlan,
    step_index: usize,
    result: &crate::runbook::executor::RunbookExecutionResult,
) {
    let completed = result
        .step_results
        .iter()
        .find_map(|step| match &step.outcome {
            crate::runbook::executor::StepOutcome::Completed { result } => Some(result),
            _ => None,
        });
    let Some(completed) = completed else {
        return;
    };

    let output_fields: Vec<String> = plan
        .bindings
        .entries
        .values()
        .filter_map(|binding| match binding {
            crate::runbook::plan_types::EntityBinding::ForwardRef {
                source_step,
                output_field,
            } if *source_step == step_index => Some(output_field.clone()),
            _ => None,
        })
        .collect();

    for output_field in output_fields {
        let maybe_uuid = completed
            .get(&output_field)
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok());
        if let Some(id) = maybe_uuid {
            crate::runbook::plan_executor::record_step_output(
                &mut plan.bindings,
                step_index,
                &output_field,
                id,
            );
        }
    }
}

fn normalize_output_entity_kind(produced_type: &str) -> String {
    match produced_type {
        "kyc_case" => "case".to_string(),
        other => other.replace('-', "_"),
    }
}

fn output_field_name_for(produced_type: &str) -> String {
    match produced_type {
        "kyc_case" => "created_case_id".to_string(),
        other => format!("created_{}_id", other.replace('-', "_")),
    }
}

fn load_plan_verb_outputs(
) -> anyhow::Result<std::collections::BTreeMap<String, Vec<sem_os_core::verb_contract::VerbOutput>>>
{
    let verbs_config = dsl_core::config::loader::ConfigLoader::from_env().load_verbs()?;
    let mut outputs =
        std::collections::BTreeMap::<String, Vec<sem_os_core::verb_contract::VerbOutput>>::new();
    for verb in crate::sem_reg::onboarding::verb_extract::extract_verbs(&verbs_config) {
        let Some(output) = verb.output else {
            continue;
        };
        outputs
            .entry(verb.fqn)
            .or_default()
            .push(sem_os_core::verb_contract::VerbOutput {
                field_name: output_field_name_for(&output.produced_type),
                output_type: "uuid".into(),
                entity_kind: Some(normalize_output_entity_kind(&output.produced_type)),
                description: Some(format!("Created {}", output.produced_type)),
            });
    }
    Ok(outputs)
}

async fn ensure_runbook_step_workspace_ready(
    state: &ReplV2RouteState,
    session_id: Uuid,
) -> Result<(), (StatusCode, Json<ErrorResponseV2>)> {
    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Unknown session {session_id}"),
                    recoverable: false,
                }),
            )
        })?;

    let Some(plan) = session.runbook_plan.as_ref() else {
        return Ok(());
    };
    let cursor = session.runbook_plan_cursor.unwrap_or(0);
    if cursor >= plan.steps.len() {
        let sessions = state.orchestrator.sessions_for_test();
        if let Some(session) = sessions.write().await.get_mut(&session_id) {
            session.enter_sage_mode();
        }
        return Ok(());
    }

    let Some(context) = plan_step_context(&session, plan, cursor).map_err(anyhow_json_error)?
    else {
        let sessions = state.orchestrator.sessions_for_test();
        if let Some(session) = sessions.write().await.get_mut(&session_id) {
            session.enter_repl_mode();
        }
        return Ok(());
    };

    let resolved = resolve_context(state.orchestrator_pool()?, &context)
        .await
        .map_err(as_json_error)?;
    let hydrated = hydrate_workspace_state(state.orchestrator_pool()?, &resolved)
        .await
        .map_err(as_json_error)?;

    let sessions = state.orchestrator.sessions_for_test();
    let mut sessions_write = sessions.write().await;
    let session = sessions_write.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponseV2 {
                error: format!("Unknown session {session_id}"),
                recoverable: false,
            }),
        )
    })?;

    session.enter_sage_mode();

    if session.workspace_stack.is_empty() {
        session.set_client_scope(resolved.client_group_id);
        session.set_workspace_root(resolved.workspace.clone());
        if let Some(tos) = session.tos_frame_mut() {
            tos.constellation_family = resolved.constellation_family.clone();
            tos.constellation_map = resolved.constellation_map.clone();
            tos.subject_kind = resolved.subject_kind.clone();
            tos.subject_id = resolved.subject_id;
            tos.is_peek = false;
        }
    } else {
        let same_tos = session.tos_frame().is_some_and(|tos| {
            tos.workspace == resolved.workspace
                && tos.constellation_map == resolved.constellation_map
                && tos.subject_id == resolved.subject_id
        });
        if !same_tos {
            let mut frame =
                WorkspaceFrame::new(resolved.workspace.clone(), resolved.session_scope.clone());
            frame.constellation_family = resolved.constellation_family.clone();
            frame.constellation_map = resolved.constellation_map.clone();
            frame.subject_kind = resolved.subject_kind.clone();
            frame.subject_id = resolved.subject_id;
            frame.is_peek = false;
            session
                .push_workspace_frame(frame)
                .map_err(anyhow_json_error)?;
        }
    }

    session.hydrate_tos(hydrated);
    session.enter_repl_mode();
    Ok(())
}

/// Request body for the signal endpoint (webhook for external systems).
///
/// External systems (workflow engines, approval portals) POST to `/api/repl/v2/signal`
/// with the `correlation_key` that was provided when the entry was parked.
#[derive(Debug, Deserialize)]
pub struct ReplSignalRequest {
    /// The correlation key linking this signal to a parked runbook entry.
    pub correlation_key: String,
    /// Signal status: `"completed"` or `"failed"`.
    pub status: String,
    /// Result payload (when status is `"completed"`).
    pub result: Option<serde_json::Value>,
    /// Error description (when status is `"failed"`).
    pub error: Option<String>,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// POST /api/repl/v2/session — Create a new V2 session.
async fn create_session_v2(
    State(state): State<ReplV2RouteState>,
) -> Result<Json<CreateSessionResponseV2>, StatusCode> {
    let session_id = state.orchestrator.create_session().await;

    let greeting = crate::repl::bootstrap::format_greeting();

    // Push the greeting as an assistant message into session history.
    {
        let sessions = state.orchestrator.sessions_for_test();
        let mut sessions_write = sessions.write().await;
        if let Some(session) = sessions_write.get_mut(&session_id) {
            session.push_message(
                crate::repl::session_v2::MessageRole::Assistant,
                greeting.clone(),
            );
        }
    }

    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build a full ReplResponseV2 with ScopeRequired kind so the
    // frontend can render the client group selector immediately.
    let response = ReplResponseV2 {
        state: session.state,
        kind: crate::repl::response_v2::ReplResponseKindV2::ScopeRequired {
            prompt: greeting.clone(),
        },
        message: greeting,
        runbook_summary: None,
        step_count: 0,
        session_feedback: None,
    };

    Ok(Json(CreateSessionResponseV2 {
        session_id,
        response,
    }))
}

/// GET /api/repl/v2/session/:id — Get V2 session state.
async fn get_session_v2(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponseV2>, StatusCode> {
    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionStateResponseV2::from(session)))
}

/// Internal REPL V2 input adapter used by unified `/api/session/:id/input`.
pub(crate) async fn input_v2(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<InputRequestV2>,
) -> Result<Json<ReplResponseV2>, (StatusCode, Json<ErrorResponseV2>)> {
    let user_input: UserInputV2 = input.into();

    match state.orchestrator.process(session_id, user_input).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            tracing::error!("REPL V2 processing error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponseV2 {
                    error: e.to_string(),
                    recoverable: true,
                }),
            ))
        }
    }
}

/// POST /api/repl/v2/session/:id/input (legacy) — hard-blocked in unified-input cutover.
async fn input_v2_legacy_blocked() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::GONE,
        Json(serde_json::json!({
            "error": "Legacy endpoint removed. Use POST /api/session/:id/input with kind=repl_v2."
        })),
    )
}

/// DELETE /api/repl/v2/session/:id — Delete V2 session.
async fn delete_session_v2(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    if !state.orchestrator.delete_session(session_id).await {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_constellation_context(
    State(state): State<ReplV2RouteState>,
    axum::extract::Query(query): axum::extract::Query<SessionContextQuery>,
) -> Result<Json<SessionFeedback>, (StatusCode, Json<ErrorResponseV2>)> {
    match state.orchestrator.session_feedback(query.session_id).await {
        Ok(feedback) => Ok(Json(feedback)),
        Err(error) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponseV2 {
                error: error.to_string(),
                recoverable: true,
            }),
        )),
    }
}

async fn push_session_context(
    State(state): State<ReplV2RouteState>,
    Json(request): Json<SessionPushRequest>,
) -> Result<Json<SessionFeedbackResponse>, (StatusCode, Json<ErrorResponseV2>)> {
    let resolved = resolve_context(state.orchestrator_pool()?, &request.context)
        .await
        .map_err(as_json_error)?;
    let hydrated = hydrate_workspace_state(state.orchestrator_pool()?, &resolved)
        .await
        .map_err(as_json_error)?;

    let existing = state
        .orchestrator
        .get_session(request.context.session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Unknown session {}", request.context.session_id),
                    recoverable: false,
                }),
            )
        })?;

    // R1.3: AgentMode gate — stack ops require Sage mode
    if !existing.agent_mode.can_stack_op() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponseV2 {
                error: "Stack operations require Sage mode".into(),
                recoverable: true,
            }),
        ));
    }

    if existing.workspace_stack.is_empty() {
        state
            .orchestrator
            .apply_root_context(request.context.session_id, &request.context)
            .await
            .map_err(anyhow_json_error)?;
    } else {
        let mut frame =
            WorkspaceFrame::new(resolved.workspace.clone(), resolved.session_scope.clone());
        frame.constellation_family = resolved.constellation_family.clone();
        frame.constellation_map = resolved.constellation_map.clone();
        frame.subject_kind = resolved.subject_kind.clone();
        frame.subject_id = resolved.subject_id;
        frame.is_peek = request.peek;
        state
            .orchestrator
            .push_workspace_frame(request.context.session_id, frame)
            .await
            .map_err(anyhow_json_error)?;
    }

    state
        .orchestrator
        .hydrate_tos(request.context.session_id, hydrated)
        .await
        .map_err(anyhow_json_error)?;

    let feedback = state
        .orchestrator
        .session_feedback(request.context.session_id)
        .await
        .map_err(anyhow_json_error)?;
    state
        .orchestrator
        .persist_session_checkpoint(request.context.session_id)
        .await
        .map_err(anyhow_json_error)?;

    Ok(Json(SessionFeedbackResponse { resolved, feedback }))
}

async fn commit_session_context(
    State(state): State<ReplV2RouteState>,
    Json(request): Json<SessionStackRequest>,
) -> Result<Json<SessionFeedback>, (StatusCode, Json<ErrorResponseV2>)> {
    // R1.3: AgentMode gate
    if let Some(session) = state.orchestrator.get_session(request.session_id).await {
        if !session.agent_mode.can_stack_op() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponseV2 {
                    error: "Stack operations require Sage mode".into(),
                    recoverable: true,
                }),
            ));
        }
    }
    state
        .orchestrator
        .commit_workspace_stack(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    let feedback = state
        .orchestrator
        .session_feedback(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    state
        .orchestrator
        .persist_session_checkpoint(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    Ok(Json(feedback))
}

async fn pop_session_context(
    State(state): State<ReplV2RouteState>,
    Json(request): Json<SessionStackRequest>,
) -> Result<Json<SessionFeedback>, (StatusCode, Json<ErrorResponseV2>)> {
    // R1.3: AgentMode gate
    if let Some(session) = state.orchestrator.get_session(request.session_id).await {
        if !session.agent_mode.can_stack_op() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponseV2 {
                    error: "Stack operations require Sage mode".into(),
                    recoverable: true,
                }),
            ));
        }
    }
    state
        .orchestrator
        .pop_workspace_frame(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    let feedback = state
        .orchestrator
        .session_feedback(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    state
        .orchestrator
        .persist_session_checkpoint(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    Ok(Json(feedback))
}

/// POST /api/repl/v2/signal — External system signals completion of a parked entry.
///
/// Workflow engines, approval portals, and other external systems call this
/// endpoint with the `correlation_key` that was provided when the entry was
/// parked. The handler locates the correct session, resumes the entry, and
/// either continues execution (on success) or marks the entry as failed.
///
/// This endpoint is idempotent: signalling an already-resumed entry returns
/// 200 with `"already_resumed"` status.
async fn signal_v2(
    State(state): State<ReplV2RouteState>,
    Json(req): Json<ReplSignalRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Validate signal status.
    if req.status != "completed" && req.status != "failed" {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Delegate to orchestrator.signal_completion() — shared logic with SignalRelay.
    match state
        .orchestrator
        .signal_completion(&req.correlation_key, &req.status, req.result, req.error)
        .await
    {
        Ok(Some(response)) => Ok(Json(
            serde_json::to_value(response)
                .unwrap_or_else(|_| serde_json::json!({"status": "signalled"})),
        )),
        Ok(None) => {
            // No session found or already resumed — idempotent.
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            tracing::error!(
                "REPL V2 signal processing error for key={}: {}",
                req.correlation_key,
                e
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// Router
// ============================================================================

/// Create the V2 REPL router.
///
/// Mount under `/api/repl/v2`:
///
/// ```ignore
/// let v2_state = ReplV2RouteState { orchestrator: Arc::new(orch) };
/// let app = Router::new()
///     .nest("/api/repl/v2", repl_routes_v2::router().with_state(v2_state))
///     // ... other routes
/// ```
pub fn router() -> Router<ReplV2RouteState> {
    Router::new()
        .route("/session", post(create_session_v2))
        .route(
            "/session/:id",
            get(get_session_v2).delete(delete_session_v2),
        )
        .route("/session/:id/input", post(input_v2_legacy_blocked))
        .route("/signal", post(signal_v2))
}

/// Create the session-scoped navigation router.
///
/// Mount this router at the application root so the routes resolve as:
/// `/api/constellation/context`, `/api/session/push`, `/api/session/commit`, `/api/session/pop`.
///
/// # Examples
/// ```rust,ignore
/// let app = Router::new()
///     .merge(repl_routes_v2::navigation_router().with_state(v2_state));
/// ```
pub fn navigation_router() -> Router<ReplV2RouteState> {
    Router::new()
        .route("/api/constellation/context", get(get_constellation_context))
        .route("/api/session/push", post(push_session_context))
        .route("/api/session/commit", post(commit_session_context))
        .route("/api/session/pop", post(pop_session_context))
        // Runbook plan routes (R5-R7)
        .route(
            "/api/session/:id/runbook/compile",
            post(compile_runbook_plan),
        )
        .route("/api/session/:id/runbook/plan", get(get_runbook_plan))
        .route(
            "/api/session/:id/runbook/approve",
            post(approve_runbook_plan),
        )
        .route(
            "/api/session/:id/runbook/execute",
            post(execute_runbook_plan),
        )
        .route("/api/session/:id/runbook/cancel", post(cancel_runbook_plan))
        .route("/api/session/:id/runbook/status", get(get_runbook_status))
        // Session trace routes (R9)
        .route("/api/session/:id/trace", get(get_session_trace))
        .route("/api/session/:id/trace/:seq", get(get_trace_entry))
        .route("/api/session/:id/trace/replay", post(replay_session_trace))
}

// ============================================================================
// Runbook Plan Handlers (R5-R7)
// ============================================================================

/// POST /api/session/:id/runbook/compile — compile a multi-workspace runbook plan.
async fn compile_runbook_plan(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let sessions = state.orchestrator.sessions_for_test();
    let mut sessions_write = sessions.write().await;
    let session = sessions_write.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponseV2 {
                error: format!("Unknown session {session_id}"),
                recoverable: false,
            }),
        )
    })?;

    // AgentMode gate: compilation requires Sage mode
    if !session.agent_mode.can_compile() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponseV2 {
                error: "Runbook compilation requires Sage mode".into(),
                recoverable: true,
            }),
        ));
    }

    let verb_outputs = load_plan_verb_outputs().map_err(anyhow_json_error)?;

    // Build workspace inputs from the current stack.
    // When hydrated constellation is available, use DAG discovery to find
    // non-terminal slots and their advancing verbs automatically.
    let inputs: Vec<crate::runbook::plan_compiler::WorkspaceInput> = session
        .workspace_stack
        .iter()
        .map(|f| {
            let subject_kind = f
                .subject_kind
                .clone()
                .unwrap_or(crate::repl::types_v2::SubjectKind::Cbu);

            // Try constellation DAG discovery first
            if let Some(ref hs) = f.hydrated_state {
                if let Some(ref hydrated) = hs.hydrated_constellation {
                    return crate::runbook::plan_compiler::input_from_hydrated_constellation(
                        &f.workspace,
                        &f.constellation_map,
                        subject_kind,
                        f.subject_id,
                        hydrated,
                        &verb_outputs,
                    );
                }
            }

            // Fallback: use the scoped verb surface from hydrated state
            crate::runbook::plan_compiler::WorkspaceInput {
                workspace: f.workspace.clone(),
                constellation_map: f.constellation_map.clone(),
                subject_kind,
                subject_id: f.subject_id,
                advancing_verbs: f
                    .hydrated_state
                    .as_ref()
                    .map(|h| h.scoped_verb_surface.clone())
                    .unwrap_or_default(),
                verb_outputs: verb_outputs.clone(),
            }
        })
        .collect();

    if inputs.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponseV2 {
                error: "No workspace frames to compile from".into(),
                recoverable: true,
            }),
        ));
    }

    let mut plan = crate::runbook::plan_compiler::compile_runbook_plan(
        session_id,
        &inputs,
        vec![], // source_research trace refs
    )
    .map_err(anyhow_json_error)?;
    plan.status = crate::runbook::plan_types::RunbookPlanStatus::AwaitingApproval;

    let plan_id = plan.id.0.clone();
    let step_count = plan.steps.len();

    // Append trace and store plan on session
    session.append_trace(crate::repl::session_trace::TraceOp::RunbookCompiled {
        runbook_id: plan_id.clone(),
    });
    session.enter_sage_mode();
    session.runbook_plan = Some(plan);
    session.runbook_plan_cursor = Some(0);
    drop(sessions_write);
    state
        .orchestrator
        .persist_session_checkpoint(session_id)
        .await
        .map_err(anyhow_json_error)?;

    Ok(Json(serde_json::json!({
        "status": "compiled",
        "plan_id": plan_id,
        "step_count": step_count
    })))
}

/// GET /api/session/:id/runbook/plan — returns current RunbookPlan for rendering.
async fn get_runbook_plan(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Unknown session {session_id}"),
                    recoverable: false,
                }),
            )
        })?;

    match &session.runbook_plan {
        Some(plan) => Ok(Json(serde_json::to_value(plan).unwrap_or_default())),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponseV2 {
                error: "No runbook plan compiled for this session".into(),
                recoverable: true,
            }),
        )),
    }
}

/// POST /api/session/:id/runbook/approve — transition Compiled → Approved.
async fn approve_runbook_plan(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let sessions = state.orchestrator.sessions_for_test();
    let mut sessions_write = sessions.write().await;
    let session = sessions_write.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponseV2 {
                error: format!("Unknown session {session_id}"),
                recoverable: false,
            }),
        )
    })?;

    use crate::runbook::plan_types::{RunbookApproval, RunbookPlanStatus};

    // Check plan exists and status is valid
    let plan_id = {
        let plan = session.runbook_plan.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: "No runbook plan to approve".into(),
                    recoverable: true,
                }),
            )
        })?;
        match &plan.status {
            RunbookPlanStatus::Compiled | RunbookPlanStatus::AwaitingApproval => plan.id.0.clone(),
            other => {
                return Err((
                    StatusCode::CONFLICT,
                    Json(ErrorResponseV2 {
                        error: format!("Cannot approve plan in {:?} status", other),
                        recoverable: false,
                    }),
                ));
            }
        }
    };

    // Now mutate plan and session separately
    if let Some(plan) = session.runbook_plan.as_mut() {
        plan.approval = Some(RunbookApproval {
            approved_by: "session_user".into(),
            approved_at: chrono::Utc::now(),
            plan_hash: plan_id.clone(),
        });
        plan.status = RunbookPlanStatus::Approved;
    }
    session.enter_repl_mode();
    session.append_trace(crate::repl::session_trace::TraceOp::RunbookApproved {
        runbook_id: plan_id.clone(),
    });
    drop(sessions_write);
    state
        .orchestrator
        .persist_session_checkpoint(session_id)
        .await
        .map_err(anyhow_json_error)?;
    Ok(Json(
        serde_json::json!({ "status": "approved", "plan_id": plan_id }),
    ))
}

/// POST /api/session/:id/runbook/execute — start or resume execution.
async fn execute_runbook_plan(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    ensure_runbook_step_workspace_ready(&state, session_id).await?;

    // Phase 1: Read session data under a brief read lock, then release before async work.
    // RwLockWriteGuard is !Send so we cannot hold it across .await points.
    let (_step_verb, _step_sentence, _step_args, step_id, cursor, compiled, compiled_id) = {
        let sessions = state.orchestrator.sessions_for_test();
        let mut sessions_write = sessions.write().await;
        let session = sessions_write.get_mut(&session_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Unknown session {session_id}"),
                    recoverable: false,
                }),
            )
        })?;

        if !session.agent_mode.can_execute() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponseV2 {
                    error: "Runbook execution requires Repl mode".into(),
                    recoverable: true,
                }),
            ));
        }

        let plan = session.runbook_plan.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: "No runbook plan to execute".into(),
                    recoverable: true,
                }),
            )
        })?;

        use crate::runbook::plan_types::RunbookPlanStatus;
        match &plan.status {
            RunbookPlanStatus::Approved | RunbookPlanStatus::Executing { .. } => {}
            other => {
                return Err((
                    StatusCode::CONFLICT,
                    Json(ErrorResponseV2 {
                        error: format!(
                            "Cannot execute plan in {:?} status (must be Approved)",
                            other
                        ),
                        recoverable: false,
                    }),
                ));
            }
        }

        let cursor = session.runbook_plan_cursor.unwrap_or(0);

        // Check step count and resolve forward refs in limited scopes.
        let step_count = session.runbook_plan.as_ref().unwrap().steps.len();
        if cursor >= step_count {
            // All steps done
            crate::runbook::plan_executor::update_plan_status(
                session.runbook_plan.as_mut().unwrap(),
            );
            let narration = crate::runbook::narration::narrate_plan(
                session.runbook_plan.as_ref().unwrap(),
                &session.execution_log,
            );
            return Ok(Json(serde_json::to_value(&narration).unwrap_or_default()));
        }

        let resolved_subject = {
            let plan = session.runbook_plan.as_ref().unwrap();
            crate::runbook::plan_executor::resolve_step_bindings(plan, cursor)
                .map_err(anyhow_json_error)?
        };

        if resolved_subject.is_none() {
            // Forward ref not yet resolved — dependency not fulfilled
            let plan_mut = session.runbook_plan.as_mut().unwrap();
            let skipped = crate::runbook::plan_executor::skip_dependent_steps(plan_mut, cursor);
            session.execution_log.extend(skipped);
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponseV2 {
                    error: format!("Step {} has unresolved forward reference", cursor),
                    recoverable: false,
                }),
            ));
        }

        if let Some(plan_mut) = session.runbook_plan.as_mut() {
            plan_mut.status = RunbookPlanStatus::Executing { cursor };
            if let Some(step) = plan_mut.steps.get_mut(cursor) {
                step.status = crate::runbook::plan_types::PlanStepStatus::Executing;
            }
        }

        // Extract step data for async execution outside the lock
        let step = &session.runbook_plan.as_ref().unwrap().steps[cursor];
        let step_verb = step.verb.verb_fqn.clone();
        let step_sentence = step.sentence.clone();
        let step_args = step.args.clone();
        let step_id = Uuid::new_v4();

        // Construct DSL from verb + args
        let dsl = crate::repl::orchestrator_v2::rebuild_dsl(&step_verb, &{
            let mut m = std::collections::HashMap::new();
            for (k, v) in &step_args {
                m.insert(k.clone(), v.clone());
            }
            m
        });

        // Build CompiledStep + CompiledRunbook wrapper
        let compiled_step = crate::runbook::types::CompiledStep {
            step_id,
            sentence: step_sentence.clone(),
            verb: step_verb.clone(),
            dsl,
            args: step_args.clone(),
            depends_on: vec![],
            execution_mode: crate::runbook::types::ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        };
        let compiled = crate::runbook::types::CompiledRunbook::new(
            session_id,
            session.allocate_runbook_version(),
            vec![compiled_step],
            crate::runbook::envelope::ReplayEnvelope::empty(),
        );
        let compiled_id = compiled.id;

        (
            step_verb,
            step_sentence,
            step_args,
            step_id,
            cursor,
            compiled,
            compiled_id,
        )
    }; // write lock released here

    // Phase 2: Async execution work (no lock held)
    let store = state
        .orchestrator
        .runbook_store()
        .unwrap_or_else(|| std::sync::Arc::new(crate::runbook::RunbookStore::new()));

    use crate::runbook::RunbookStoreBackend;
    store
        .insert(&compiled)
        .await
        .map_err(|e| anyhow_json_error(anyhow::anyhow!("{}", e)))?;

    // Build step executor bridge and run through execute_runbook
    let bridge =
        crate::runbook::step_executor_bridge::DslStepExecutor::new(state.orchestrator.executor());
    let execution_outcome =
        match crate::runbook::execute_runbook(&*store, compiled_id, None, &bridge).await {
            Ok(result) => Ok(result),
            Err(e) => Err(anyhow::anyhow!("{}", e)),
        };

    // Phase 3: Re-acquire write lock to store results back
    {
        let sessions = state.orchestrator.sessions_for_test();
        let mut sessions_write = sessions.write().await;
        let session = sessions_write.get_mut(&session_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Session {session_id} disappeared during execution"),
                    recoverable: false,
                }),
            )
        })?;

        // Advance plan step with the execution result
        let plan_mut = session.runbook_plan.as_mut().ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: "Runbook plan disappeared during execution".into(),
                    recoverable: false,
                }),
            )
        })?;
        let step_result = crate::runbook::plan_executor::advance_plan_step(
            plan_mut,
            cursor,
            execution_outcome
                .as_ref()
                .map(|result| serde_json::to_value(result).unwrap_or_default())
                .map_err(|e| anyhow::anyhow!(e.to_string())),
        )
        .map_err(anyhow_json_error)?;

        if let Ok(result) = &execution_outcome {
            persist_completed_step_outputs(plan_mut, cursor, result);
        }

        // Record enriched verb execution in trace
        let verb_fqn = step_result.verb_fqn.clone();
        let exec_result_json = step_result.output.clone();
        session.append_trace_enriched(
            crate::repl::session_trace::TraceOp::VerbExecuted {
                verb_fqn: verb_fqn.clone(),
                step_id,
            },
            Some(verb_fqn.clone()),
            exec_result_json,
        );
        if step_result.status == crate::runbook::plan_types::PlanStepStatus::Succeeded {
            session.increment_tos_writes();
        }
        session.execution_log.push(step_result.clone());
        session.runbook_plan_cursor = Some(cursor + 1);

        // Check if plan is complete
        crate::runbook::plan_executor::update_plan_status(session.runbook_plan.as_mut().unwrap());

        let next_cursor = cursor + 1;
        let plan_after = session.runbook_plan.as_ref().unwrap();
        let has_next = next_cursor < plan_after.steps.len();
        let next_needs_transition = has_next
            && crate::runbook::plan_executor::needs_workspace_transition(plan_after, cursor)
                .is_some();
        if has_next && !next_needs_transition {
            session.enter_repl_mode();
        } else {
            session.enter_sage_mode();
        }

        // Generate narration
        let narration = crate::runbook::narration::narrate_step(
            session.runbook_plan.as_ref().unwrap(),
            &step_result,
        );

        let response = Json(serde_json::json!({
            "status": "step_executed",
            "cursor": cursor + 1,
            "step_narration": narration,
            "plan_status": session.runbook_plan.as_ref().unwrap().status,
        }));
        drop(sessions_write);
        state
            .orchestrator
            .persist_session_checkpoint(session_id)
            .await
            .map_err(anyhow_json_error)?;
        Ok(response)
    }
}

/// POST /api/session/:id/runbook/cancel — cancel mid-execution.
async fn cancel_runbook_plan(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let sessions = state.orchestrator.sessions_for_test();
    let mut sessions_write = sessions.write().await;
    let session = sessions_write.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponseV2 {
                error: format!("Unknown session {session_id}"),
                recoverable: false,
            }),
        )
    })?;

    let plan = session.runbook_plan.as_mut().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponseV2 {
                error: "No runbook plan to cancel".into(),
                recoverable: true,
            }),
        )
    })?;

    let cancelled = crate::runbook::plan_executor::cancel_plan(plan);
    session.enter_sage_mode();
    drop(sessions_write);
    state
        .orchestrator
        .persist_session_checkpoint(session_id)
        .await
        .map_err(anyhow_json_error)?;
    Ok(Json(serde_json::json!({
        "status": "cancelled",
        "steps_cancelled": cancelled.len()
    })))
}

/// GET /api/session/:id/runbook/status — current plan status.
async fn get_runbook_status(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Unknown session {session_id}"),
                    recoverable: false,
                }),
            )
        })?;

    match &session.runbook_plan {
        Some(plan) => Ok(Json(serde_json::json!({
            "plan_id": plan.id.0,
            "status": plan.status,
            "total_steps": plan.steps.len(),
            "cursor": session.runbook_plan_cursor,
        }))),
        None => Ok(Json(serde_json::json!({ "status": "no_plan" }))),
    }
}

// ============================================================================
// Session Trace Handlers (R9)
// ============================================================================

/// GET /api/session/:id/trace — retrieve session trace.
async fn get_session_trace(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    if let Ok(pool) = state.orchestrator_pool() {
        let trace =
            crate::repl::trace_repository::SessionTraceRepository::load_trace(pool, session_id)
                .await
                .map_err(anyhow_json_error)?;
        if !trace.is_empty() {
            return Ok(Json(serde_json::to_value(trace).unwrap_or_default()));
        }
    }

    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Unknown session {session_id}"),
                    recoverable: false,
                }),
            )
        })?;

    Ok(Json(
        serde_json::to_value(session.trace).unwrap_or_default(),
    ))
}

/// GET /api/session/:id/trace/:seq — retrieve a single trace entry.
async fn get_trace_entry(
    State(state): State<ReplV2RouteState>,
    Path((session_id, seq)): Path<(Uuid, u64)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    if let Ok(pool) = state.orchestrator_pool() {
        if let Some(entry) =
            crate::repl::trace_repository::SessionTraceRepository::load_entry(pool, session_id, seq)
                .await
                .map_err(anyhow_json_error)?
        {
            return Ok(Json(serde_json::to_value(entry).unwrap_or_default()));
        }
    }

    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: format!("Unknown session {session_id}"),
                    recoverable: false,
                }),
            )
        })?;

    // Check in-memory trace first
    if let Some(entry) = session.trace.iter().find(|e| e.sequence == seq) {
        return Ok(Json(serde_json::to_value(entry).unwrap_or_default()));
    }

    Err((
        StatusCode::NOT_FOUND,
        Json(ErrorResponseV2 {
            error: format!("Trace entry {seq} not found for session {session_id}"),
            recoverable: false,
        }),
    ))
}

/// POST /api/session/:id/trace/replay — replay from trace tape.
async fn replay_session_trace(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let persisted_trace = if let Ok(pool) = state.orchestrator_pool() {
        crate::repl::trace_repository::SessionTraceRepository::load_trace(pool, session_id)
            .await
            .map_err(anyhow_json_error)?
    } else {
        Vec::new()
    };
    let trace = if persisted_trace.is_empty() {
        let session = state
            .orchestrator
            .get_session(session_id)
            .await
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponseV2 {
                        error: format!("Unknown session {session_id}"),
                        recoverable: false,
                    }),
                )
            })?;
        session.trace
    } else {
        persisted_trace
    };

    let mode_str = body
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("relaxed");
    let mode = match mode_str {
        "strict" => crate::repl::session_replay::ReplayMode::Strict,
        "dry_run" => crate::repl::session_replay::ReplayMode::DryRun,
        _ => crate::repl::session_replay::ReplayMode::Relaxed,
    };

    let result = crate::repl::session_replay::replay_trace(&trace, mode);
    Ok(Json(serde_json::to_value(&result).unwrap_or_default()))
}

fn as_json_error(error: (StatusCode, String)) -> (StatusCode, Json<ErrorResponseV2>) {
    let (status, message) = error;
    (
        status,
        Json(ErrorResponseV2 {
            error: message,
            recoverable: true,
        }),
    )
}

fn anyhow_json_error(error: anyhow::Error) -> (StatusCode, Json<ErrorResponseV2>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponseV2 {
            error: error.to_string(),
            recoverable: true,
        }),
    )
}

impl ReplV2RouteState {
    fn orchestrator_pool(&self) -> Result<&sqlx::PgPool, (StatusCode, Json<ErrorResponseV2>)> {
        #[cfg(feature = "database")]
        {
            self.orchestrator.pool().ok_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponseV2 {
                        error: "REPL V2 orchestrator has no database pool".to_string(),
                        recoverable: false,
                    }),
                )
            })
        }
        #[cfg(not(feature = "database"))]
        {
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponseV2 {
                    error: "database feature is not enabled".to_string(),
                    recoverable: false,
                }),
            ))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_request_v2_message_parsing() {
        let json = r#"{"type": "message", "content": "create a fund"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::Message { content } if content == "create a fund"));
    }

    #[test]
    fn test_input_request_v2_confirm() {
        let json = r#"{"type": "confirm"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::Confirm));
    }

    #[test]
    fn test_input_request_v2_reject() {
        let json = r#"{"type": "reject"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::Reject));
    }

    #[test]
    fn test_input_request_v2_command_parsing() {
        let json = r#"{"type": "command", "command": "run"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Command {
                command: ReplCommandV2::Run
            }
        ));
    }

    #[test]
    fn test_input_request_v2_select_pack() {
        let json = r#"{"type": "select_pack", "pack_id": "onboarding-request"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(
            matches!(req, InputRequestV2::SelectPack { pack_id } if pack_id == "onboarding-request")
        );
    }

    #[test]
    fn test_input_request_v2_select_verb() {
        let json =
            r#"{"type": "select_verb", "verb_fqn": "cbu.create", "original_input": "create"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::SelectVerb { .. }));
    }

    #[test]
    fn test_input_request_v2_select_scope() {
        let json = r#"{"type": "select_scope", "group_id": "11111111-1111-1111-1111-111111111111", "group_name": "Allianz"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::SelectScope { .. }));
    }

    #[test]
    fn test_unknown_command_defaults_to_help() {
        let json = r#"{"type": "command", "command": "foobar"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Command {
                command: ReplCommandV2::Help
            }
        ));
    }

    #[test]
    fn test_input_request_v2_approve() {
        let json = r#"{"type": "approve", "entry_id": "11111111-1111-1111-1111-111111111111", "approved_by": "admin@example.com"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(
            matches!(req, InputRequestV2::Approve { entry_id, approved_by }
                if entry_id == Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
                && approved_by == Some("admin@example.com".to_string())
            )
        );
    }

    #[test]
    fn test_input_request_v2_approve_without_approver() {
        let json = r#"{"type": "approve", "entry_id": "11111111-1111-1111-1111-111111111111"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Approve {
                approved_by: None,
                ..
            }
        ));
    }

    #[test]
    fn test_input_request_v2_reject_gate() {
        let json = r#"{"type": "reject_gate", "entry_id": "22222222-2222-2222-2222-222222222222", "reason": "Insufficient documentation"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(
            matches!(req, InputRequestV2::RejectGate { entry_id, reason }
                if entry_id == Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()
                && reason == Some("Insufficient documentation".to_string())
            )
        );
    }

    #[test]
    fn test_input_request_v2_reject_gate_without_reason() {
        let json = r#"{"type": "reject_gate", "entry_id": "22222222-2222-2222-2222-222222222222"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::RejectGate { reason: None, .. }
        ));
    }

    #[test]
    fn test_command_status_parsing() {
        let json = r#"{"type": "command", "command": "status"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Command {
                command: ReplCommandV2::Status
            }
        ));
    }

    #[test]
    fn test_signal_request_deserialization_completed() {
        let json =
            r#"{"correlation_key": "abc:def", "status": "completed", "result": {"doc_id": "123"}}"#;
        let req: ReplSignalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.correlation_key, "abc:def");
        assert_eq!(req.status, "completed");
        assert!(req.result.is_some());
        assert!(req.error.is_none());
    }

    #[test]
    fn test_signal_request_deserialization_failed() {
        let json = r#"{"correlation_key": "abc:def", "status": "failed", "error": "Timeout"}"#;
        let req: ReplSignalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.correlation_key, "abc:def");
        assert_eq!(req.status, "failed");
        assert!(req.result.is_none());
        assert_eq!(req.error, Some("Timeout".to_string()));
    }

    #[test]
    fn test_signal_request_deserialization_minimal() {
        let json = r#"{"correlation_key": "abc:def", "status": "completed"}"#;
        let req: ReplSignalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.correlation_key, "abc:def");
        assert_eq!(req.status, "completed");
        assert!(req.result.is_none());
        assert!(req.error.is_none());
    }
}

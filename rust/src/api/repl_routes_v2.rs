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
use sem_os_core::acp_projection::{
    AcpProjectionEnvelope, AcpProjectionEnvelopeInput, AcpProjectionKind,
};
use sem_os_core::domain_pack::DomainPackManifest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::api::constellation_routes::{hydrate_workspace_state, resolve_context};
use crate::repl::response_v2::ReplResponseV2;
use crate::repl::session_v2::ReplSessionV2;
use crate::repl::types_v2::{
    ConstellationContextRef, ReplCommandV2, ReplStateV2, ResolvedConstellationContext,
    SessionFeedback, UserInputV2, WorkspaceFrame, WorkspaceKind,
};
use crate::sequencer::ReplOrchestratorV2;

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

    /// User selected a CBU structure constellation map.
    SelectConstellationMap { constellation_map: String },

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
            InputRequestV2::SelectConstellationMap { constellation_map } => {
                UserInputV2::SelectConstellationMap { constellation_map }
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

/// Request for non-mutating KYC update-status workbook dry-run.
#[derive(Debug, Clone, Deserialize)]
pub struct KycUpdateStatusDryRunRequest {
    pub case_id: Uuid,
    #[serde(default = "default_kyc_update_status_transition_ref")]
    pub transition_ref: String,
    pub current_state: String,
    pub requested_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    pub evidence_digest: String,
    pub actor_id: String,
    #[serde(default)]
    pub actor_roles: Vec<String>,
}

/// Request to issue a restricted-mutation approval token for a workbook.
#[derive(Debug, Clone, Deserialize)]
pub struct KycApprovalTokenRequest {
    pub workbook: crate::runbook::ExecutionWorkbook,
    pub approved_by_actor_id: String,
    pub approval_text: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Request to prepare a restricted mutation preflight. This does not execute mutation.
#[derive(Debug, Clone, Deserialize)]
pub struct KycRestrictedMutationPreflightRequest {
    pub workbook: crate::runbook::ExecutionWorkbook,
    pub approval_token: crate::runbook::MutationApprovalToken,
    pub observed_configuration_version: String,
    pub observed_state_snapshot_id: String,
    pub observed_evidence_refs: Vec<crate::runbook::EvidenceRef>,
    #[serde(default)]
    pub consumed_token_ids: Vec<crate::runbook::ApprovalTokenId>,
}

/// Request to compile a prepared restricted mutation preflight into a runbook.
#[derive(Debug, Clone, Deserialize)]
pub struct KycRestrictedMutationCompileRunbookRequest {
    pub preflight: crate::runbook::RestrictedMutationPreflight,
}

/// Request to record actual semantic diff after the compiled runbook gate executes.
#[derive(Debug, Clone, Deserialize)]
pub struct KycRestrictedMutationReceiptRequest {
    pub preflight: crate::runbook::RestrictedMutationPreflight,
    pub compilation: crate::runbook::RestrictedMutationRunbookCompilation,
    pub actual_diff: crate::runbook::MutationSemanticDiff,
}

fn default_kyc_update_status_transition_ref() -> String {
    "kyc-case.intake-to-discovery".to_string()
}

/// Request to open an ACP adapter session.
#[derive(Debug, Clone, Deserialize)]
pub struct AcpOpenSessionRequest {
    #[serde(default = "default_acp_adapter")]
    pub adapter: crate::acp::AcpAdapterKind,
    #[serde(default)]
    pub persona: Option<crate::acp::AcpPersonaMode>,
}

/// Request to assemble redacted Sage context for an ACP session.
#[derive(Debug, Clone, Deserialize)]
pub struct AcpContextAssemblyRequest {
    #[serde(default = "default_acp_adapter")]
    pub adapter: crate::acp::AcpAdapterKind,
    pub probe_id: String,
    pub subject_kind: String,
    pub subject_id: String,
    #[serde(default)]
    pub context: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub observations: Vec<sem_os_core::domain_pack::DiscoveryObservation>,
    #[serde(default)]
    pub provenance: Vec<sem_os_core::domain_pack::DiscoveryProvenance>,
    #[serde(default)]
    pub first_class_state_mutated: bool,
}

/// Request to route an ACP prompt through the session-scoped HTTP API.
#[derive(Debug, Clone, Deserialize)]
pub struct AcpPromptRouteRequest {
    #[serde(default)]
    pub prompt: Vec<crate::acp_protocol::AcpContentBlock>,
}

/// Request to run the bounded KYC update-status language loop.
#[derive(Debug, Clone, Deserialize)]
pub struct AcpKycUpdateStatusLanguageLoopRouteRequest {
    #[serde(default = "default_acp_adapter")]
    pub adapter: crate::acp::AcpAdapterKind,
    pub subject_id: Uuid,
    pub current_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub prompt_route_ms: Option<u64>,
    #[serde(default)]
    pub prompt_route_us: Option<u64>,
    pub draft: crate::runbook::KycUpdateStatusWorkbookDraft,
}

fn default_acp_adapter() -> crate::acp::AcpAdapterKind {
    crate::acp::AcpAdapterKind::Zed
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

    // Build session_feedback BEFORE moving session.state so the UI sees
    // the universe root state (available workspaces, bootstrap verbs).
    let session_feedback = session.build_session_feedback(false);
    let response = ReplResponseV2 {
        state: session.state,
        kind: crate::repl::response_v2::ReplResponseKindV2::ScopeRequired {
            prompt: greeting.clone(),
        },
        message: greeting,
        runbook_summary: None,
        step_count: 0,
        session_feedback: Some(session_feedback),
        narration: None,
        trace_id: None,
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

/// Internal REPL V2 input adapter — retained for direct REPL access.
#[allow(dead_code)]
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

#[allow(dead_code)] // Route provided by create_constellation_router; retained for reference
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
        .route("/session/:id/input", post(input_v2))
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
    // Note: /api/constellation/context is registered by create_constellation_router().
    // This router is now empty but retained for API compatibility.
    Router::new()
}

/// Session-scoped routes that share the `/api/session/...` namespace with agent routes.
///
/// These MUST be merged into the same router as agent routes to avoid
/// axum 0.7 overlapping-route panics (`:id` wildcard vs literal segments).
pub fn session_scoped_router() -> Router<ReplV2RouteState> {
    Router::new()
        // Navigation stack ops
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
        // Configuration-native workbook dry-run routes
        .route(
            "/api/session/:id/workbook/kyc/update-status/dry-run",
            post(dry_run_kyc_update_status_workbook),
        )
        .route(
            "/api/session/:id/workbook/kyc/approval-token",
            post(issue_kyc_workbook_approval_token),
        )
        .route(
            "/api/session/:id/workbook/kyc/restricted-mutation/preflight",
            post(prepare_kyc_restricted_mutation_preflight),
        )
        .route(
            "/api/session/:id/workbook/kyc/restricted-mutation/compile-runbook",
            post(compile_kyc_restricted_mutation_runbook),
        )
        .route(
            "/api/session/:id/workbook/kyc/restricted-mutation/receipt",
            post(record_kyc_restricted_mutation_receipt),
        )
        // ACP adapter lifecycle/context routes
        .route(
            "/api/session/:id/acp/capabilities",
            get(get_acp_capabilities_route),
        )
        .route("/api/session/:id/acp/policy", get(get_acp_policy_route))
        .route(
            "/api/session/:id/acp/projections",
            get(list_acp_projections_route),
        )
        .route(
            "/api/session/:id/acp/projections/:kind",
            get(get_acp_projection_route),
        )
        .route("/api/session/:id/acp/open", post(open_acp_session_route))
        .route("/api/session/:id/acp/close", post(close_acp_session_route))
        .route(
            "/api/session/:id/acp/context",
            post(assemble_acp_context_route),
        )
        .route("/api/session/:id/acp/prompt", post(acp_prompt_route))
        .route(
            "/api/session/:id/acp/kyc/update-status/language-loop",
            post(acp_kyc_update_status_language_loop_route),
        )
        // Session trace routes (R9)
        .route("/api/session/:id/trace", get(get_session_trace))
        .route("/api/session/:id/trace/:seq", get(get_trace_entry))
        .route("/api/session/:id/trace/replay", post(replay_session_trace))
}

/// GET /api/session/:id/acp/capabilities
///
/// Returns both the protocol-level initialize response and the
/// session-level `AcpPolicyCapabilities` (Domain Pack policy +
/// projection catalogue + transition mode). One-stop discovery for
/// ACP consumers — they get protocol metadata and what the live
/// SemOS-aware adapter actually supports without a second call.
async fn get_acp_capabilities_route(
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    acp_capabilities_value(session_id)
        .map(Json)
        .map_err(acp_json_error)
}

fn acp_capabilities_value(
    session_id: Uuid,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let mut agent = crate::acp_protocol::AcpJsonRpcAgent::new();
    let protocol = agent
        .handle_request(crate::acp_protocol::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: serde_json::json!({}),
        })
        .into_iter()
        .find_map(|outgoing| match outgoing {
            crate::acp_protocol::JsonRpcOutgoing::Response(response) => response.result,
            crate::acp_protocol::JsonRpcOutgoing::Notification(_) => None,
        })
        .unwrap_or_else(|| serde_json::json!({}));

    let manifest = load_ob_poc_kyc_domain_pack()?;
    let session = crate::acp::open_acp_session(session_id, crate::acp::AcpAdapterKind::Zed);
    let policy = crate::acp::acp_policy_capabilities(&session, &manifest)?;

    Ok(serde_json::json!({
        "status": "acp_capabilities",
        "session_id": session_id,
        "protocol": protocol,
        "policy": policy,
        "stdio": {
            "command": "ob_poc_acp",
            "transport": "jsonrpc_stdio",
            "message_delimiter": "newline"
        }
    }))
}

/// GET /api/session/:id/acp/policy
async fn get_acp_policy_route(
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    acp_policy_value(session_id)
        .map(Json)
        .map_err(acp_json_error)
}

fn acp_policy_value(session_id: Uuid) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let manifest = load_ob_poc_kyc_domain_pack()?;
    let session = crate::acp::open_acp_session(session_id, crate::acp::AcpAdapterKind::Zed);
    let policy = crate::acp::acp_policy_capabilities(&session, &manifest)?;

    Ok(serde_json::json!({
        "status": "acp_policy",
        "policy": policy,
    }))
}

/// GET /api/session/:id/acp/projections
async fn list_acp_projections_route(
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    list_acp_projections_value(session_id)
        .map(Json)
        .map_err(acp_json_error)
}

fn list_acp_projections_value(
    session_id: Uuid,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let manifest = load_ob_poc_kyc_domain_pack()?;
    let session = crate::acp::open_acp_session(session_id, crate::acp::AcpAdapterKind::Zed);
    let projections = crate::acp::list_acp_projections(&session, &manifest)?;

    Ok(serde_json::json!({
        "status": "acp_projection_catalog",
        "session_id": session_id,
        "pack_id": manifest.pack_id,
        "projections": projections,
    }))
}

/// GET /api/session/:id/acp/projections/:kind
async fn get_acp_projection_route(
    State(state): State<ReplV2RouteState>,
    Path((session_id, kind)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let started_at = Instant::now();
    let value = get_acp_projection_value_for_state(&state, session_id, &kind)
        .await
        .map_err(acp_json_error)?;
    let projection_latency_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
    let projection_bytes = serde_json::to_vec(&value["projection"])
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    let acp_mechanism_summary = vec![
        "projection_get".to_string(),
        "classification_policy".to_string(),
        "demand_driven".to_string(),
    ];
    let acp_fallback_summary = vec![];
    append_session_trace_if_present(
        &state,
        session_id,
        crate::repl::session_trace::TraceOp::AcpProjectionServed {
            projection_kind: value["projection"]["projection_kind"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            projection_hash: value["projection"]["projection_hash"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            classification: value["projection"]["classification"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            redacted_count: value["projection"]["redactions"]
                .as_array()
                .map(|redactions| redactions.len())
                .unwrap_or(0),
            acp_mode: "discovery".to_string(),
            acp_persona_mode: "sage:planning".to_string(),
            sage_workflow_phase: "discovery".to_string(),
            mechanisms: acp_mechanism_summary.clone(),
            fallback_summary: acp_fallback_summary.clone(),
            acp_mechanism_summary,
            acp_fallback_summary,
            projected_surface_summary: vec![format!(
                "{}:{}:{}",
                value["projection"]["projection_kind"]
                    .as_str()
                    .unwrap_or("unknown"),
                value["projection"]["classification"]
                    .as_str()
                    .unwrap_or("unknown"),
                value["projection"]["projection_hash"]
                    .as_str()
                    .unwrap_or("unknown")
            )],
            capability_negotiation: vec![
                "declined:fs/write_text_file".to_string(),
                "declined:terminal/create".to_string(),
            ],
            projection_count: 1,
            projection_bytes,
            projection_latency_ms,
        },
    )
    .await;
    Ok(Json(value))
}

async fn get_acp_projection_value_for_state(
    state: &ReplV2RouteState,
    session_id: Uuid,
    kind: &str,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let manifest = load_ob_poc_kyc_domain_pack()?;
    let session = crate::acp::open_acp_session(session_id, crate::acp::AcpAdapterKind::Zed);
    let kind = kind.parse::<AcpProjectionKind>().map_err(|_| {
        crate::acp::AcpAdapterError::ProjectionUnknown {
            projection_kind: kind.to_string(),
        }
    })?;

    if let Some(repl_session) = state.orchestrator.get_session(session_id).await {
        if let Some(projection) =
            build_live_acp_projection(&session, &manifest, &repl_session, kind)?
        {
            return Ok(serde_json::json!({
                "status": "acp_projection",
                "projection": projection,
            }));
        }
    }

    get_acp_projection_value(session_id, kind.as_str())
}

fn get_acp_projection_value(
    session_id: Uuid,
    kind: &str,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let manifest = load_ob_poc_kyc_domain_pack()?;
    let session = crate::acp::open_acp_session(session_id, crate::acp::AcpAdapterKind::Zed);
    let kind = kind
        .parse::<sem_os_core::acp_projection::AcpProjectionKind>()
        .map_err(|_| crate::acp::AcpAdapterError::ProjectionUnknown {
            projection_kind: kind.to_string(),
        })?;
    let projection = crate::acp::build_acp_projection(
        &session,
        &manifest,
        crate::acp::AcpProjectionRequest {
            kind,
            subject: None,
            language_pack_request: None,
        },
    )?;

    Ok(serde_json::json!({
        "status": "acp_projection",
        "projection": projection,
    }))
}

fn live_affinity_nodes(repl_session: &ReplSessionV2) -> Vec<serde_json::Value> {
    let mut nodes = repl_session
        .workspace_stack
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            serde_json::json!({
                "id": format!("workspace:{index}:{}", frame.constellation_map),
                "kind": "workspace_frame",
                "workspace": frame.workspace,
                "constellation_family": frame.constellation_family,
                "constellation_map": frame.constellation_map,
                "subject_kind": frame.subject_kind,
                "subject_id": frame.subject_id,
                "stale": frame.stale,
            })
        })
        .collect::<Vec<_>>();

    nodes.extend(repl_session.bindings.keys().map(|binding| {
        serde_json::json!({
            "id": format!("binding:{binding}"),
            "kind": "binding",
            "name": binding,
        })
    }));
    nodes
}

fn live_affinity_edges(repl_session: &ReplSessionV2) -> Vec<serde_json::Value> {
    let mut edges = repl_session
        .workspace_stack
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            serde_json::json!({
                "from": format!("workspace:{index}:{}", pair[0].constellation_map),
                "to": format!("workspace:{}:{}", index + 1, pair[1].constellation_map),
                "kind": "stack_adjacency",
            })
        })
        .collect::<Vec<_>>();

    if let Some((index, frame)) = repl_session.workspace_stack.iter().enumerate().next_back() {
        let frame_id = format!("workspace:{index}:{}", frame.constellation_map);
        edges.extend(repl_session.bindings.keys().map(|binding| {
            serde_json::json!({
                "from": frame_id,
                "to": format!("binding:{binding}"),
                "kind": "current_frame_binding",
            })
        }));
    }
    edges
}

fn build_live_acp_projection(
    session: &crate::acp::AcpSession,
    manifest: &DomainPackManifest,
    repl_session: &ReplSessionV2,
    kind: AcpProjectionKind,
) -> Result<Option<AcpProjectionEnvelope>, crate::acp::AcpAdapterError> {
    let Some(catalog_entry) = manifest
        .projection_catalog
        .iter()
        .find(|entry| entry.kind == kind)
    else {
        return Err(crate::acp::AcpAdapterError::ProjectionUnknown {
            projection_kind: kind.as_str().to_string(),
        });
    };

    let payload = match kind {
        AcpProjectionKind::WorkspaceState => serde_json::json!({
            "status": "live",
            "active_workspace": repl_session.active_workspace,
            "agent_mode": repl_session.agent_mode,
            "conversation_mode": repl_session.conversation_mode,
            "workspace_stack": repl_session.workspace_stack,
            "session_stack": repl_session.session_stack,
            "bindings": repl_session.bindings,
            "runbook_plan_cursor": repl_session.runbook_plan_cursor,
        }),
        AcpProjectionKind::Dag => serde_json::json!({
            "status": "live",
            "frames": repl_session.workspace_stack.iter().map(|frame| serde_json::json!({
                "workspace": frame.workspace.clone(),
                "constellation_family": frame.constellation_family.clone(),
                "constellation_map": frame.constellation_map.clone(),
                "subject_kind": frame.subject_kind.clone(),
                "subject_id": frame.subject_id,
                "stale": frame.stale,
                "hydrated_state": frame.hydrated_state.clone(),
            })).collect::<Vec<_>>(),
        }),
        AcpProjectionKind::GraphScene => serde_json::json!({
            "status": "live",
            "source": "session_stack",
            "session_stack": repl_session.session_stack,
            "workspace_frame_count": repl_session.workspace_stack.len(),
        }),
        AcpProjectionKind::VerbSurface => serde_json::json!({
            "status": "live",
            "pending_verb": repl_session.pending_verb,
            "scoped_surfaces": repl_session.workspace_stack.iter().filter_map(|frame| {
                frame.hydrated_state.as_ref().map(|state| serde_json::json!({
                    "workspace": frame.workspace.clone(),
                    "constellation_map": frame.constellation_map.clone(),
                    "verbs": state.scoped_verb_surface.clone(),
                    "available_actions": state.available_actions.clone(),
                    "narration_hot_verbs": frame.narration_hot_verbs.clone(),
                }))
            }).collect::<Vec<_>>(),
        }),
        AcpProjectionKind::DiscoverySurface => {
            if let Some(envelope) = repl_session.pending_sem_os_envelope.as_ref() {
                serde_json::json!({
                    "status": "live",
                    "resolution_stage": envelope.resolution_stage.clone(),
                    "discovery_surface": envelope.discovery_surface.clone(),
                    "grounded_action_surface": envelope.grounded_action_surface.clone(),
                    "fingerprint": envelope.fingerprint_str(),
                })
            } else {
                serde_json::json!({
                    "status": "live_no_envelope",
                    "reason": "session has no pending SemOS context envelope"
                })
            }
        }
        AcpProjectionKind::Governance => {
            if let Some(envelope) = repl_session.pending_sem_os_envelope.as_ref() {
                serde_json::json!({
                    "status": "live",
                    "fingerprint": envelope.fingerprint_str(),
                    "evidence_gaps": envelope.evidence_gaps.clone(),
                    "governance_signals": envelope.governance_signals.clone(),
                    "snapshot_set_id": envelope.snapshot_set_id.clone(),
                    "computed_at": envelope.computed_at,
                })
            } else {
                serde_json::json!({
                    "status": "live_no_envelope",
                    "evidence_gaps": [],
                    "governance_signals": []
                })
            }
        }
        AcpProjectionKind::Lineage => serde_json::json!({
            "status": "live",
            "trace_sequence": repl_session.trace_sequence,
            "trace": repl_session.trace,
            "session_stack": repl_session.session_stack,
        }),
        AcpProjectionKind::EvidenceSchema => serde_json::json!({
            "status": "live",
            "transition_evidence_requirements": manifest.allowed_transitions.iter().map(|transition| serde_json::json!({
                "transition_ref": transition.transition_ref.clone(),
                "entity_type": transition.entity_type.clone(),
                "evidence_refs_required": transition.evidence_refs_required.clone(),
                "classification": catalog_entry.default_classification,
            })).collect::<Vec<_>>(),
            "classification_policy": manifest.classification_policy,
        }),
        AcpProjectionKind::DerivationRegistry => serde_json::json!({
            "status": "live",
            "typed_extension_points": manifest.typed_extension_points.iter().filter(|extension| {
                extension.extension_kind.contains("derivation")
            }).collect::<Vec<_>>(),
        }),
        AcpProjectionKind::Materiality => serde_json::json!({
            "status": "live",
            "transition_materiality": manifest.allowed_transitions.iter().map(|transition| serde_json::json!({
                "transition_ref": transition.transition_ref.clone(),
                "verb": transition.verb.clone(),
                "from_state": transition.from_state.clone(),
                "to_state": transition.to_state.clone(),
                "dry_run_enabled": transition.dry_run_enabled,
                "mutation_enabled": transition.mutation_enabled,
                "hitl_required": transition.hitl_required,
                "evidence_refs_required": transition.evidence_refs_required.clone(),
            })).collect::<Vec<_>>(),
        }),
        AcpProjectionKind::AffinityGraph => serde_json::json!({
            "status": "live",
            "source": "repl_session.workspace_stack",
            "nodes": live_affinity_nodes(repl_session),
            "edges": live_affinity_edges(repl_session),
            "note": "Session-derived affinity projection; external SemOS AffinityGraph authority may enrich this shape when attached."
        }),
        AcpProjectionKind::PackManifest
        | AcpProjectionKind::ProbeCatalogue
        | AcpProjectionKind::LanguagePack
        | AcpProjectionKind::TransitionSurface
        | AcpProjectionKind::Policy => return Ok(None),
    };

    Ok(Some(AcpProjectionEnvelope::new(
        AcpProjectionEnvelopeInput {
            projection_kind: kind,
            session_id: session.session_id,
            pack_id: manifest.pack_id.clone(),
            classification: catalog_entry.default_classification,
            subject: None,
            snapshot_refs: vec![
                format!("domain_pack:{}@{}", manifest.pack_id, manifest.version),
                format!("repl_session:{}", repl_session.id),
                format!("trace_sequence:{}", repl_session.trace_sequence),
            ],
            payload,
            redactions: vec![],
        },
    )))
}

/// POST /api/session/:id/acp/open
async fn open_acp_session_route(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<AcpOpenSessionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let value = open_acp_session_value(session_id, req);
    append_session_trace_if_present(
        &state,
        session_id,
        crate::repl::session_trace::TraceOp::AcpSessionOpened {
            adapter: value["session"]["adapter"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            mutation_capability: value["session"]["mutation_capability"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            acp_persona_mode: value["session"]["persona"]
                .as_str()
                .unwrap_or("sage:planning")
                .to_string(),
            capability_negotiation: vec![
                "declined:fs/write_text_file".to_string(),
                "declined:terminal/create".to_string(),
            ],
        },
    )
    .await;
    Ok(Json(value))
}

fn open_acp_session_value(session_id: Uuid, req: AcpOpenSessionRequest) -> serde_json::Value {
    let session = crate::acp::open_acp_session_with_persona(
        session_id,
        req.adapter,
        req.persona
            .unwrap_or(crate::acp::AcpPersonaMode::SagePlanning),
    );
    serde_json::json!({
        "status": "acp_session_open",
        "session": session,
    })
}

/// POST /api/session/:id/acp/close
async fn close_acp_session_route(
    Path(session_id): Path<Uuid>,
    Json(req): Json<AcpOpenSessionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    Ok(Json(close_acp_session_value(session_id, req)))
}

fn close_acp_session_value(session_id: Uuid, req: AcpOpenSessionRequest) -> serde_json::Value {
    let mut session = crate::acp::open_acp_session_with_persona(
        session_id,
        req.adapter,
        req.persona
            .unwrap_or(crate::acp::AcpPersonaMode::SagePlanning),
    );
    crate::acp::close_acp_session(&mut session);
    serde_json::json!({
        "status": "acp_session_closed",
        "session": session,
    })
}

/// POST /api/session/:id/acp/context
async fn assemble_acp_context_route(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<AcpContextAssemblyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let value = assemble_acp_context_value(session_id, req).map_err(acp_json_error)?;
    append_session_trace_if_present(
        &state,
        session_id,
        crate::repl::session_trace::TraceOp::AcpContextAssembled {
            pack_id: value["bundle"]["pack_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            probe_id: value["bundle"]["probe_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            context_hash: value["bundle"]["prompt_context"]["context_hash"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            redacted_count: value["bundle"]["prompt_context"]["redacted"]
                .as_array()
                .map(|redacted| redacted.len())
                .unwrap_or(0),
        },
    )
    .await;
    Ok(Json(value))
}

fn assemble_acp_context_value(
    session_id: Uuid,
    req: AcpContextAssemblyRequest,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let manifest = load_ob_poc_kyc_domain_pack()?;
    let session = crate::acp::open_acp_session(session_id, req.adapter);
    let subject = sem_os_core::domain_pack::DiscoverySubject {
        subject_kind: req.subject_kind,
        subject_id: req.subject_id,
    };
    let probe_id = req.probe_id;
    let discovery_request = sem_os_core::domain_pack::DiscoveryRequest {
        pack_id: manifest.pack_id.clone(),
        probe_id: probe_id.clone(),
        subject: subject.clone(),
        context: req.context,
    };
    let discovery_response = sem_os_core::domain_pack::DiscoveryResponse {
        probe_id,
        subject,
        observations: req.observations,
        provenance: req.provenance,
        first_class_state_mutated: req.first_class_state_mutated,
    };

    let bundle = crate::acp::assemble_sage_context_for_acp(
        &session,
        &manifest,
        discovery_request,
        discovery_response,
    )?;

    Ok(serde_json::json!({
        "status": "acp_context_assembled",
        "bundle": bundle,
    }))
}

/// POST /api/session/:id/acp/prompt
async fn acp_prompt_route(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<AcpPromptRouteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let mut agent = crate::acp_protocol::AcpJsonRpcAgent::new();
    let outgoing = agent.handle_request(crate::acp_protocol::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!("prompt")),
        method: "session/prompt".to_string(),
        params: serde_json::json!({
            "sessionId": session_id.to_string(),
            "prompt": req.prompt,
        }),
    });
    let result = outgoing.iter().find_map(|item| match item {
        crate::acp_protocol::JsonRpcOutgoing::Response(response) => response.result.clone(),
        crate::acp_protocol::JsonRpcOutgoing::Notification(_) => None,
    });

    if let Some(result) = &result {
        if let Some(op) = acp_language_loop_trace_op_from_value(result) {
            append_session_trace_if_present(&state, session_id, op).await;
        }
    }

    Ok(Json(serde_json::json!({
        "status": "acp_prompt_processed",
        "session_id": session_id,
        "result": result.unwrap_or_else(|| serde_json::json!({})),
        "outgoing": outgoing,
    })))
}

/// POST /api/session/:id/acp/kyc/update-status/language-loop
async fn acp_kyc_update_status_language_loop_route(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<AcpKycUpdateStatusLanguageLoopRouteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    if req.draft.session_id != session_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponseV2 {
                error: format!(
                    "draft.session_id {} does not match route session {session_id}",
                    req.draft.session_id
                ),
                recoverable: true,
            }),
        ));
    }

    let value =
        acp_kyc_update_status_language_loop_value(session_id, req).map_err(acp_json_error)?;
    if let Some(op) = acp_language_loop_trace_op_from_value(&value) {
        append_session_trace_if_present(&state, session_id, op).await;
    }
    Ok(Json(value))
}

fn acp_kyc_update_status_language_loop_value(
    session_id: Uuid,
    req: AcpKycUpdateStatusLanguageLoopRouteRequest,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let manifest = load_ob_poc_kyc_domain_pack()?;
    let session = crate::acp::open_acp_session(session_id, req.adapter);
    let prompt_route_us = req
        .prompt_route_us
        .unwrap_or_else(|| req.prompt_route_ms.unwrap_or(0).saturating_mul(1_000));

    let started_at = Instant::now();
    let outcome = crate::acp::acp_run_kyc_update_status_language_loop_timed(
        &session,
        &manifest,
        crate::runbook::KycLanguagePackRequest {
            subject_id: req.subject_id,
            current_state: req.current_state,
            configuration_version: req.configuration_version,
            state_snapshot_id: req.state_snapshot_id,
            objective: req.objective,
        },
        req.draft,
    )?;
    let projection_latency_ms = route_elapsed_ms(started_at);
    let language_pack = outcome.language_pack;
    let timings = outcome.timings;
    let performance = route_language_loop_performance(&timings, prompt_route_us, 0);

    match outcome.revision_outcome {
        crate::runbook::WorkbookRevisionOutcome::DryRunValid {
            output,
            attempts,
            metrics,
            trace,
        } => Ok(serde_json::json!({
            "status": "dry_run_validated",
            "language_pack": language_pack,
            "output": output,
            "attempts": attempts,
            "metrics": metrics,
            "trace": trace,
            "observability": {
                "projectionLatencyMs": projection_latency_ms,
                "performance": performance,
                "conversationEfficiency": route_language_loop_conversation_efficiency(
                    &metrics,
                    "dry_run_validated",
                    None,
                ),
                "acpMechanismSummary": ["language_pack", "deterministic_revision_loop", "dry_run_only"]
            }
        })),
        crate::runbook::WorkbookRevisionOutcome::Refused {
            refusal,
            attempts,
            metrics,
            trace,
        } => {
            let refusal_code = refusal.refusal_code.clone();
            Ok(serde_json::json!({
                "status": "structured_refusal",
                "language_pack": language_pack,
                "refusal": refusal,
                "attempts": attempts,
                "metrics": metrics,
                "trace": trace,
                "observability": {
                    "projectionLatencyMs": projection_latency_ms,
                    "performance": performance,
                    "conversationEfficiency": route_language_loop_conversation_efficiency(
                        &metrics,
                        "structured_refusal",
                        Some(refusal_code.as_str()),
                    ),
                    "acpMechanismSummary": ["language_pack", "deterministic_revision_loop", "structured_refusal"]
                }
            }))
        }
    }
}

fn route_language_loop_performance(
    timings: &crate::acp::AcpKycLanguageLoopTimings,
    prompt_route_us: u64,
    acp_emit_us: u64,
) -> serde_json::Value {
    let total_us = timings
        .total_us
        .saturating_add(prompt_route_us)
        .saturating_add(acp_emit_us);
    serde_json::json!({
        "prompt_route_ms": route_millis_from_micros(prompt_route_us),
        "prompt_route_us": prompt_route_us,
        "language_pack_ms": timings.language_pack_ms,
        "language_pack_us": timings.language_pack_us,
        "revision_loop_ms": timings.revision_loop_ms,
        "revision_loop_us": timings.revision_loop_us,
        "dry_run_ms": timings.dry_run_ms,
        "dry_run_us": timings.dry_run_us,
        "acp_emit_ms": route_millis_from_micros(acp_emit_us),
        "acp_emit_us": acp_emit_us,
        "total_ms": route_millis_from_micros(total_us),
        "total_us": total_us,
    })
}

fn route_language_loop_conversation_efficiency(
    metrics: &crate::runbook::LanguageAcquisitionMetrics,
    outcome: &str,
    pending_reason: Option<&str>,
) -> serde_json::Value {
    let pending_user_turn_required = !metrics.dry_run_valid;
    let estimated_user_repair_turns_avoided = if metrics.dry_run_valid {
        u64::from(metrics.revision_count)
    } else {
        0
    };

    serde_json::json!({
        "outcome": outcome,
        "localRevisionCount": metrics.revision_count,
        "estimatedUserRepairTurnsAvoided": estimated_user_repair_turns_avoided,
        "pendingUserTurnRequired": pending_user_turn_required,
        "pendingReason": pending_reason,
        "firstPassValid": metrics.first_pass_valid,
        "dryRunValid": metrics.dry_run_valid,
        "structuredFailureMode": pending_reason,
        "proseOnlyFailure": false,
    })
}

fn acp_language_loop_trace_op_from_value(
    value: &serde_json::Value,
) -> Option<crate::repl::session_trace::TraceOp> {
    let outcome = value.get("status")?.as_str()?;
    if !matches!(
        outcome,
        "dry_run_validated" | "structured_refusal" | "pending_question"
    ) {
        return None;
    }

    let pack_id = value_string(value, &["language_pack", "pack_id"]);
    let subject_id = value_string(value, &["language_pack", "subject", "id"])
        .or_else(|| last_attempt_string(value, &["draft", "case_id"]))
        .and_then(|id| Uuid::parse_str(&id).ok());
    let verb = value_string(value, &["output", "dry_run", "semantic_diff", "verb"])
        .or_else(|| first_array_string(value, &["language_pack", "valid_verbs"], "verb"))
        .or_else(|| last_attempt_string(value, &["draft", "verb"]));
    let current_state = value_string(value, &["output", "dry_run", "semantic_diff", "from_state"])
        .or_else(|| value_string(value, &["language_pack", "current_state"]))
        .or_else(|| last_attempt_string(value, &["draft", "current_state"]));
    let requested_state = value_string(value, &["output", "dry_run", "semantic_diff", "to_state"])
        .or_else(|| last_attempt_string(value, &["draft", "requested_state"]));
    let transition_ref = value_string(value, &["output", "dry_run", "transition_ref"])
        .or_else(|| last_attempt_string(value, &["draft", "transition_ref"]))
        .or_else(|| {
            value
                .get("refusal")
                .and_then(|refusal| refusal.get("diagnostics"))
                .and_then(|diagnostics| diagnostics.as_array())
                .and_then(|diagnostics| diagnostics.first())
                .and_then(|diagnostic| diagnostic.get("attempted_transition"))
                .and_then(|transition| transition.as_str())
                .map(str::to_string)
        });
    let workbook_id = value_string(value, &["output", "workbook", "id"])
        .or_else(|| value_string(value, &["output", "dry_run", "workbook_id"]));
    let semantic_diff_uri = value_string(value, &["output", "dry_run", "semantic_diff_uri"]);
    let refusal_code = value_string(value, &["refusal", "refusal_code"]);
    let pending_question_code = value_string(value, &["pending_question", "code"]);
    let diagnostic_source_path = value
        .get("refusal")
        .and_then(|refusal| refusal.get("diagnostics"))
        .and_then(|diagnostics| diagnostics.as_array())
        .and_then(|diagnostics| diagnostics.first())
        .and_then(|diagnostic| diagnostic.get("source_path"))
        .and_then(|source_path| source_path.as_str())
        .map(str::to_string);
    let revision_count = value
        .get("metrics")
        .and_then(|metrics| metrics.get("revision_count"))
        .and_then(|count| count.as_u64())
        .or_else(|| {
            value
                .get("observability")
                .and_then(|observability| observability.get("conversationEfficiency"))
                .and_then(|efficiency| efficiency.get("localRevisionCount"))
                .and_then(|count| count.as_u64())
        })
        .and_then(|count| u8::try_from(count).ok())
        .unwrap_or(0);
    let dry_run_valid = value
        .get("metrics")
        .and_then(|metrics| metrics.get("dry_run_valid"))
        .and_then(|valid| valid.as_bool())
        .or_else(|| {
            value
                .get("observability")
                .and_then(|observability| observability.get("conversationEfficiency"))
                .and_then(|efficiency| efficiency.get("dryRunValid"))
                .and_then(|valid| valid.as_bool())
        })
        .unwrap_or(false);
    let first_pass_valid = value
        .get("metrics")
        .and_then(|metrics| metrics.get("first_pass_valid"))
        .and_then(|valid| valid.as_bool())
        .or_else(|| {
            value
                .get("observability")
                .and_then(|observability| observability.get("conversationEfficiency"))
                .and_then(|efficiency| efficiency.get("firstPassValid"))
                .and_then(|valid| valid.as_bool())
        })
        .unwrap_or(false);
    let needed_from_user = needed_from_user(value);
    let trace = language_loop_trace_events(value, outcome);
    let performance = trace_performance_metrics(value);
    let conversation_efficiency = trace_conversation_efficiency(value, outcome);
    let human_summary = language_loop_human_summary(
        outcome,
        revision_count,
        refusal_code.as_deref(),
        pending_question_code.as_deref(),
    );

    Some(crate::repl::session_trace::TraceOp::AcpLanguageLoopTraced {
        outcome: outcome.to_string(),
        pack_id,
        subject_id,
        verb,
        current_state,
        requested_state,
        transition_ref,
        workbook_id,
        semantic_diff_uri,
        refusal_code,
        pending_question_code,
        diagnostic_source_path,
        revision_count,
        dry_run_valid,
        first_pass_valid,
        human_summary,
        needed_from_user,
        trace,
        performance,
        conversation_efficiency,
    })
}

fn language_loop_human_summary(
    outcome: &str,
    revision_count: u8,
    refusal_code: Option<&str>,
    pending_question_code: Option<&str>,
) -> String {
    match outcome {
        "dry_run_validated" => format!(
            "Validated a dry-run workbook after {revision_count} local revision(s); no mutation was executed."
        ),
        "structured_refusal" => format!(
            "Stopped before mutation with structured refusal {}; the agent needs a valid DSL draft before proceeding.",
            refusal_code.unwrap_or("unknown_refusal")
        ),
        "pending_question" => format!(
            "Need HITL clarification before drafting the workbook: {}.",
            pending_question_code.unwrap_or("pending_question")
        ),
        _ => "ACP language loop produced a structured outcome.".to_string(),
    }
}

fn language_loop_trace_events(
    value: &serde_json::Value,
    outcome: &str,
) -> Vec<crate::repl::session_trace::TraceLanguageLoopEvent> {
    let mut events = value
        .get("trace")
        .and_then(|trace| trace.as_array())
        .map(|trace| {
            trace
                .iter()
                .filter_map(|event| {
                    Some(crate::repl::session_trace::TraceLanguageLoopEvent {
                        phase: event.get("phase")?.as_str()?.to_string(),
                        status: event.get("status")?.as_str()?.to_string(),
                        message: event.get("message")?.as_str()?.to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if events.is_empty() && outcome == "pending_question" {
        events.push(crate::repl::session_trace::TraceLanguageLoopEvent {
            phase: "clarification".to_string(),
            status: "blocked".to_string(),
            message: value_string(value, &["pending_question", "code"])
                .unwrap_or_else(|| "pending_question".to_string()),
        });
    }

    events
}

fn trace_performance_metrics(
    value: &serde_json::Value,
) -> crate::repl::session_trace::TracePerformanceMetrics {
    let performance = value
        .get("observability")
        .and_then(|observability| observability.get("performance"));
    crate::repl::session_trace::TracePerformanceMetrics {
        prompt_route_ms: nested_u64(performance, "prompt_route_ms"),
        prompt_route_us: nested_u64(performance, "prompt_route_us"),
        language_pack_ms: nested_u64(performance, "language_pack_ms"),
        language_pack_us: nested_u64(performance, "language_pack_us"),
        revision_loop_ms: nested_u64(performance, "revision_loop_ms"),
        revision_loop_us: nested_u64(performance, "revision_loop_us"),
        dry_run_ms: nested_u64(performance, "dry_run_ms"),
        dry_run_us: nested_u64(performance, "dry_run_us"),
        acp_emit_ms: nested_u64(performance, "acp_emit_ms"),
        acp_emit_us: nested_u64(performance, "acp_emit_us"),
        total_ms: nested_u64(performance, "total_ms"),
        total_us: nested_u64(performance, "total_us"),
    }
}

fn trace_conversation_efficiency(
    value: &serde_json::Value,
    outcome: &str,
) -> crate::repl::session_trace::TraceConversationEfficiency {
    let efficiency = value
        .get("observability")
        .and_then(|observability| observability.get("conversationEfficiency"));
    crate::repl::session_trace::TraceConversationEfficiency {
        outcome: nested_string(efficiency, "outcome").unwrap_or_else(|| outcome.to_string()),
        local_revision_count: nested_u64(efficiency, "localRevisionCount")
            .try_into()
            .unwrap_or(u8::MAX),
        estimated_user_repair_turns_avoided: nested_u64(
            efficiency,
            "estimatedUserRepairTurnsAvoided",
        ),
        pending_user_turn_required: nested_bool(efficiency, "pendingUserTurnRequired"),
        pending_reason: nested_string(efficiency, "pendingReason"),
        first_pass_valid: nested_bool(efficiency, "firstPassValid"),
        dry_run_valid: nested_bool(efficiency, "dryRunValid"),
        structured_failure_mode: nested_string(efficiency, "structuredFailureMode"),
        prose_only_failure: nested_bool(efficiency, "proseOnlyFailure"),
    }
}

fn needed_from_user(value: &serde_json::Value) -> Vec<String> {
    let mut needed = value_string_array(value, &["pending_question", "needs"]);
    needed.extend(value_string_array(value, &["pending_question", "missing"]));
    needed.sort();
    needed.dedup();
    needed
}

fn value_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_string)
}

fn value_string_array(value: &serde_json::Value, path: &[&str]) -> Vec<String> {
    let mut current = value;
    for segment in path {
        let Some(next) = current.get(*segment) else {
            return Vec::new();
        };
        current = next;
    }
    current
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn first_array_string(value: &serde_json::Value, path: &[&str], field: &str) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_array()?
        .first()?
        .get(field)?
        .as_str()
        .map(str::to_string)
}

fn last_attempt_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value.get("attempts")?.as_array()?.last()?;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_string)
}

fn nested_u64(value: Option<&serde_json::Value>, key: &str) -> u64 {
    value
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
}

fn nested_bool(value: Option<&serde_json::Value>, key: &str) -> bool {
    value
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn nested_string(value: Option<&serde_json::Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn route_elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn route_millis_from_micros(micros: u64) -> u64 {
    micros / 1_000
}

/// POST /api/session/:id/workbook/kyc/update-status/dry-run
async fn dry_run_kyc_update_status_workbook(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<KycUpdateStatusDryRunRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let value = dry_run_kyc_update_status_workbook_value(session_id, req)
        .map_err(kyc_dry_run_json_error)?;
    append_session_trace_if_present(
        &state,
        session_id,
        crate::repl::session_trace::TraceOp::WorkbookDryRunValidated {
            workbook_id: value["workbook"]["id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            transition_ref: value["dry_run"]["transition_ref"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            semantic_diff_uri: value["dry_run"]["semantic_diff_uri"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            validation_trace: serde_json::from_value(value["dry_run"]["validation_trace"].clone())
                .unwrap_or_default(),
        },
    )
    .await;
    Ok(Json(value))
}

fn dry_run_kyc_update_status_workbook_value(
    session_id: Uuid,
    req: KycUpdateStatusDryRunRequest,
) -> Result<serde_json::Value, crate::runbook::KycUpdateStatusDryRunRefusal> {
    let output = crate::runbook::build_kyc_update_status_dry_run(
        crate::runbook::KycUpdateStatusDryRunInput {
            session_id,
            case_id: req.case_id,
            actor_id: req.actor_id,
            actor_roles: req.actor_roles,
            transition_ref: req.transition_ref,
            current_state: req.current_state,
            requested_state: req.requested_state,
            configuration_version: req.configuration_version,
            state_snapshot_id: req.state_snapshot_id,
            evidence_digest: req.evidence_digest,
            llm_trace_ref: None,
        },
    )?;

    Ok(serde_json::json!({
        "status": "dry_run_validated",
        "workbook": output.workbook,
        "dry_run": output.dry_run
    }))
}

/// POST /api/session/:id/workbook/kyc/approval-token
async fn issue_kyc_workbook_approval_token(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<KycApprovalTokenRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let value = issue_kyc_workbook_approval_token_value(session_id, req)
        .map_err(approval_token_json_error)?;
    append_session_trace_if_present(
        &state,
        session_id,
        crate::repl::session_trace::TraceOp::ApprovalTokenIssued {
            approval_token_id: value["approval_token"]["id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            workbook_id: value["approval_token"]["core"]["workbook_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            approved_by_actor_id: value["approval_token"]["core"]["approved_by_actor_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
        },
    )
    .await;
    Ok(Json(value))
}

fn issue_kyc_workbook_approval_token_value(
    session_id: Uuid,
    req: KycApprovalTokenRequest,
) -> Result<serde_json::Value, crate::runbook::ApprovalTokenValidationError> {
    if req.workbook.core.session_id != session_id {
        return Err(
            crate::runbook::ApprovalTokenValidationError::WorkbookBindingMismatch {
                field: "session_id".to_string(),
                expected: session_id.to_string(),
                actual: req.workbook.core.session_id.to_string(),
            },
        );
    }

    let token = crate::runbook::create_approval_token_for_workbook(
        &req.workbook,
        req.approved_by_actor_id,
        req.approval_text,
        req.expires_at,
        chrono::Utc::now(),
    )?;

    Ok(serde_json::json!({
        "status": "approval_token_issued",
        "approval_token": token,
    }))
}

/// POST /api/session/:id/workbook/kyc/restricted-mutation/preflight
async fn prepare_kyc_restricted_mutation_preflight(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<KycRestrictedMutationPreflightRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let value = prepare_kyc_restricted_mutation_preflight_value(session_id, req)
        .map_err(preflight_json_error)?;
    append_session_trace_if_present(
        &state,
        session_id,
        crate::repl::session_trace::TraceOp::RestrictedMutationPreflightPrepared {
            workbook_id: value["preflight"]["workbook_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            approval_token_id: value["preflight"]["approval"]["approval_token_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            transition_ref: value["preflight"]["transition_ref"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
        },
    )
    .await;
    Ok(Json(value))
}

fn prepare_kyc_restricted_mutation_preflight_value(
    session_id: Uuid,
    req: KycRestrictedMutationPreflightRequest,
) -> Result<serde_json::Value, crate::runbook::RestrictedMutationPreflightError> {
    if req.workbook.core.session_id != session_id {
        return Err(
            crate::runbook::RestrictedMutationPreflightError::ApprovalRefused {
                error: crate::runbook::ApprovalTokenValidationError::WorkbookBindingMismatch {
                    field: "session_id".to_string(),
                    expected: session_id.to_string(),
                    actual: req.workbook.core.session_id.to_string(),
                },
            },
        );
    }

    let manifest = load_ob_poc_kyc_domain_pack().map_err(|error| {
        crate::runbook::RestrictedMutationPreflightError::ApprovalRefused {
            error: crate::runbook::ApprovalTokenValidationError::WorkbookBindingMismatch {
                field: "pack".to_string(),
                expected: "valid ob-poc KYC pack".to_string(),
                actual: format!("{error:?}"),
            },
        }
    })?;
    prepare_kyc_restricted_mutation_preflight_value_with_manifest(session_id, req, &manifest)
}

fn prepare_kyc_restricted_mutation_preflight_value_with_manifest(
    session_id: Uuid,
    req: KycRestrictedMutationPreflightRequest,
    manifest: &sem_os_core::domain_pack::DomainPackManifest,
) -> Result<serde_json::Value, crate::runbook::RestrictedMutationPreflightError> {
    if req.workbook.core.session_id != session_id {
        return Err(
            crate::runbook::RestrictedMutationPreflightError::ApprovalRefused {
                error: crate::runbook::ApprovalTokenValidationError::WorkbookBindingMismatch {
                    field: "session_id".to_string(),
                    expected: session_id.to_string(),
                    actual: req.workbook.core.session_id.to_string(),
                },
            },
        );
    }

    let observed = crate::runbook::ObservedMutationAnchors {
        configuration_version: req.observed_configuration_version,
        state_snapshot_id: req.observed_state_snapshot_id,
        evidence_refs: req.observed_evidence_refs,
    };
    let consumed_token_ids = req
        .consumed_token_ids
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let preflight = crate::runbook::prepare_restricted_mutation_preflight(
        &req.workbook,
        Some(&req.approval_token),
        manifest,
        &observed,
        &consumed_token_ids,
        chrono::Utc::now(),
    )?;

    Ok(serde_json::json!({
        "status": "restricted_mutation_preflight_prepared",
        "preflight": preflight,
    }))
}

/// POST /api/session/:id/workbook/kyc/restricted-mutation/compile-runbook
async fn compile_kyc_restricted_mutation_runbook(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<KycRestrictedMutationCompileRunbookRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let runbook_version = {
        let sessions = state.orchestrator.sessions_for_test();
        let mut sessions_write = sessions.write().await;
        let session = sessions_write.get_mut(&session_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponseV2 {
                    error: "Session not found".to_string(),
                    recoverable: false,
                }),
            )
        })?;
        session.allocate_runbook_version()
    };

    let compilation =
        compile_kyc_restricted_mutation_runbook_value(session_id, runbook_version, req)
            .map_err(compilation_json_error)?;

    let compiled_runbook: crate::runbook::CompiledRunbook = serde_json::from_value(
        compilation["compilation"]["compiled_runbook"].clone(),
    )
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponseV2 {
                error: format!("Compiled runbook serialization failed: {error}"),
                recoverable: false,
            }),
        )
    })?;

    let store = state
        .orchestrator
        .runbook_store()
        .unwrap_or_else(|| std::sync::Arc::new(crate::runbook::RunbookStore::new()));
    use crate::runbook::RunbookStoreBackend;
    store.insert(&compiled_runbook).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponseV2 {
                error: format!("{error}"),
                recoverable: false,
            }),
        )
    })?;

    Ok(Json(compilation))
}

fn compile_kyc_restricted_mutation_runbook_value(
    session_id: Uuid,
    runbook_version: u64,
    req: KycRestrictedMutationCompileRunbookRequest,
) -> Result<serde_json::Value, crate::runbook::RestrictedMutationRunbookCompilationError> {
    let compilation = crate::runbook::compile_restricted_mutation_preflight(
        session_id,
        runbook_version,
        &req.preflight,
    )?;

    Ok(serde_json::json!({
        "status": "restricted_mutation_runbook_compiled",
        "compilation": compilation,
    }))
}

/// POST /api/session/:id/workbook/kyc/restricted-mutation/receipt
async fn record_kyc_restricted_mutation_receipt(
    Path(session_id): Path<Uuid>,
    Json(req): Json<KycRestrictedMutationReceiptRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let value = record_kyc_restricted_mutation_receipt_value(session_id, req)
        .map_err(compilation_json_error)?;
    Ok(Json(value))
}

fn record_kyc_restricted_mutation_receipt_value(
    session_id: Uuid,
    req: KycRestrictedMutationReceiptRequest,
) -> Result<serde_json::Value, crate::runbook::RestrictedMutationRunbookCompilationError> {
    if req.preflight.approval.workbook_id != req.compilation.workbook_id {
        return Err(
            crate::runbook::RestrictedMutationRunbookCompilationError::ReceiptBindingMismatch {
                field: "session_workbook_id".to_string(),
                expected: req.compilation.workbook_id.to_string(),
                actual: req.preflight.approval.workbook_id.to_string(),
            },
        );
    }
    if req.compilation.compiled_runbook.session_id != session_id {
        return Err(
            crate::runbook::RestrictedMutationRunbookCompilationError::ReceiptBindingMismatch {
                field: "session_id".to_string(),
                expected: session_id.to_string(),
                actual: req.compilation.compiled_runbook.session_id.to_string(),
            },
        );
    }

    let receipt = crate::runbook::record_restricted_mutation_execution_receipt(
        &req.compilation,
        &req.preflight,
        req.actual_diff,
        chrono::Utc::now(),
    )?;

    Ok(serde_json::json!({
        "status": "restricted_mutation_execution_recorded",
        "receipt": receipt,
    }))
}

fn kyc_dry_run_json_error(
    refusal: crate::runbook::KycUpdateStatusDryRunRefusal,
) -> (StatusCode, Json<ErrorResponseV2>) {
    let status = match refusal {
        crate::runbook::KycUpdateStatusDryRunRefusal::PackParseFailed { .. }
        | crate::runbook::KycUpdateStatusDryRunRefusal::PackInvalid { .. }
        | crate::runbook::KycUpdateStatusDryRunRefusal::WorkbookRefused { .. }
        | crate::runbook::KycUpdateStatusDryRunRefusal::DslCoderRefused { .. } => {
            StatusCode::BAD_REQUEST
        }
        crate::runbook::KycUpdateStatusDryRunRefusal::SimulationRefused { .. } => {
            StatusCode::CONFLICT
        }
    };

    (
        status,
        Json(ErrorResponseV2 {
            error: format!("{refusal:?}"),
            recoverable: true,
        }),
    )
}

fn approval_token_json_error(
    error: crate::runbook::ApprovalTokenValidationError,
) -> (StatusCode, Json<ErrorResponseV2>) {
    let status = match error {
        crate::runbook::ApprovalTokenValidationError::WorkbookIntegrity { .. }
        | crate::runbook::ApprovalTokenValidationError::TokenHashMismatch { .. }
        | crate::runbook::ApprovalTokenValidationError::WorkbookBindingMismatch { .. }
        | crate::runbook::ApprovalTokenValidationError::RequiredFieldEmpty { .. }
        | crate::runbook::ApprovalTokenValidationError::MissingEvidenceRefs => {
            StatusCode::BAD_REQUEST
        }
        crate::runbook::ApprovalTokenValidationError::MissingApprovalToken
        | crate::runbook::ApprovalTokenValidationError::TokenNotActive { .. }
        | crate::runbook::ApprovalTokenValidationError::TokenExpired { .. }
        | crate::runbook::ApprovalTokenValidationError::TokenReplay { .. }
        | crate::runbook::ApprovalTokenValidationError::StateDrift { .. }
        | crate::runbook::ApprovalTokenValidationError::EvidenceDrift { .. }
        | crate::runbook::ApprovalTokenValidationError::TransitionMutationNotEnabled { .. } => {
            StatusCode::CONFLICT
        }
    };

    (
        status,
        Json(ErrorResponseV2 {
            error: format!("{error:?}"),
            recoverable: true,
        }),
    )
}

fn preflight_json_error(
    error: crate::runbook::RestrictedMutationPreflightError,
) -> (StatusCode, Json<ErrorResponseV2>) {
    let status = match &error {
        crate::runbook::RestrictedMutationPreflightError::ApprovalRefused { error } => {
            approval_token_json_error(error.clone()).0
        }
        crate::runbook::RestrictedMutationPreflightError::PredictedDiffMismatch { .. } => {
            StatusCode::CONFLICT
        }
    };

    (
        status,
        Json(ErrorResponseV2 {
            error: format!("{error:?}"),
            recoverable: true,
        }),
    )
}

fn compilation_json_error(
    error: crate::runbook::RestrictedMutationRunbookCompilationError,
) -> (StatusCode, Json<ErrorResponseV2>) {
    let status = match error {
        crate::runbook::RestrictedMutationRunbookCompilationError::UnsupportedExecutor {
            ..
        }
        | crate::runbook::RestrictedMutationRunbookCompilationError::UnsupportedVerb { .. }
        | crate::runbook::RestrictedMutationRunbookCompilationError::ArgMismatch { .. }
        | crate::runbook::RestrictedMutationRunbookCompilationError::ReceiptBindingMismatch {
            ..
        } => StatusCode::BAD_REQUEST,
        crate::runbook::RestrictedMutationRunbookCompilationError::AlreadyExecuted { .. } => {
            StatusCode::CONFLICT
        }
        crate::runbook::RestrictedMutationRunbookCompilationError::ActualDiffMismatch {
            ..
        } => StatusCode::CONFLICT,
    };

    (
        status,
        Json(ErrorResponseV2 {
            error: format!("{error:?}"),
            recoverable: true,
        }),
    )
}

fn load_ob_poc_kyc_domain_pack(
) -> Result<sem_os_core::domain_pack::DomainPackManifest, crate::acp::AcpAdapterError> {
    serde_yaml::from_str(include_str!(
        "../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
    ))
    .map_err(|err| crate::acp::AcpAdapterError::PackInvalid {
        reason: err.to_string(),
    })
}

fn acp_json_error(error: crate::acp::AcpAdapterError) -> (StatusCode, Json<ErrorResponseV2>) {
    let status = match error {
        crate::acp::AcpAdapterError::DiscoveryRefused { .. }
        | crate::acp::AcpAdapterError::DiscoveryMutatedState
        | crate::acp::AcpAdapterError::MutationNotSupported
        | crate::acp::AcpAdapterError::ProjectionUnknown { .. }
        | crate::acp::AcpAdapterError::ProjectionSubjectRefused { .. }
        | crate::acp::AcpAdapterError::LanguagePackRefused { .. }
        | crate::acp::AcpAdapterError::CaseStateDiscoveryRefused { .. }
        | crate::acp::AcpAdapterError::DryRunRefused { .. } => StatusCode::CONFLICT,
        crate::acp::AcpAdapterError::SessionClosed
        | crate::acp::AcpAdapterError::PackInvalid { .. } => StatusCode::BAD_REQUEST,
    };

    (
        status,
        Json(ErrorResponseV2 {
            error: format!("{error:?}"),
            recoverable: true,
        }),
    )
}

async fn append_session_trace_if_present(
    state: &ReplV2RouteState,
    session_id: Uuid,
    op: crate::repl::session_trace::TraceOp,
) {
    let sessions = state.orchestrator.sessions_for_test();
    let mut sessions_write = sessions.write().await;
    let Some(session) = sessions_write.get_mut(&session_id) else {
        return;
    };
    session.append_trace(op);
    drop(sessions_write);
    if let Err(error) = state
        .orchestrator
        .persist_session_checkpoint(session_id)
        .await
    {
        tracing::warn!(
            session_id = %session_id,
            error = %error,
            "Failed to persist ACP/workbook trace checkpoint"
        );
    }
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
        let dsl = crate::sequencer::rebuild_dsl(&step_verb, &{
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
    use chrono::TimeZone;

    fn test_session_id() -> Uuid {
        Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
    }

    fn test_case_id() -> Uuid {
        Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
    }

    fn test_expires_at() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(2099, 5, 5, 13, 0, 0).unwrap()
    }

    fn dry_run_output() -> crate::runbook::KycUpdateStatusDryRunOutput {
        crate::runbook::build_kyc_update_status_dry_run(
            crate::runbook::KycUpdateStatusDryRunInput {
                session_id: test_session_id(),
                case_id: test_case_id(),
                actor_id: "analyst@example.com".to_string(),
                actor_roles: vec!["analyst".to_string()],
                transition_ref: "kyc-case.discovery-to-assessment".to_string(),
                current_state: "DISCOVERY".to_string(),
                requested_state: "ASSESSMENT".to_string(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "state-snapshot-1".to_string(),
                evidence_digest: "sha256:case".to_string(),
                llm_trace_ref: None,
            },
        )
        .expect("dry-run output")
    }

    fn language_loop_draft(
        session_id: Uuid,
        verb: &str,
        case_id: Option<Uuid>,
    ) -> crate::runbook::KycUpdateStatusWorkbookDraft {
        crate::runbook::KycUpdateStatusWorkbookDraft {
            session_id,
            actor_id: "analyst@example.com".to_string(),
            actor_roles: vec!["analyst".to_string()],
            verb: verb.to_string(),
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
            subject_kind: "kyc_case".to_string(),
            case_id,
            current_state: "INTAKE".to_string(),
            requested_state: "DISCOVERY".to_string(),
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "state-snapshot-1".to_string(),
            evidence_digest: Some("sha256:case".to_string()),
            llm_trace_ref: None,
        }
    }

    fn language_loop_request(
        session_id: Uuid,
        verb: &str,
        case_id: Option<Uuid>,
    ) -> AcpKycUpdateStatusLanguageLoopRouteRequest {
        AcpKycUpdateStatusLanguageLoopRouteRequest {
            adapter: crate::acp::AcpAdapterKind::TestHarness,
            subject_id: test_case_id(),
            current_state: "INTAKE".to_string(),
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "state-snapshot-1".to_string(),
            objective: Some("Move KYC case from intake to discovery".to_string()),
            prompt_route_ms: None,
            prompt_route_us: Some(125),
            draft: language_loop_draft(session_id, verb, case_id),
        }
    }

    fn reference_mutation_manifest() -> sem_os_core::domain_pack::DomainPackManifest {
        let mut manifest = load_ob_poc_kyc_domain_pack().expect("pack");
        manifest.compatibility_tier =
            sem_os_core::domain_pack::PackCompatibilityTier::ReferenceMutation;
        for transition in &mut manifest.allowed_transitions {
            if transition.transition_ref == "kyc-case.discovery-to-assessment" {
                transition.mutation_enabled = true;
            }
        }
        manifest
    }

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

    #[test]
    fn test_kyc_update_status_dry_run_request_defaults_transition_ref() {
        let json = r#"{
            "case_id": "11111111-1111-1111-1111-111111111111",
            "current_state": "INTAKE",
            "requested_state": "DISCOVERY",
            "configuration_version": "config-1",
            "state_snapshot_id": "state-snapshot-1",
            "evidence_digest": "sha256:case",
            "actor_id": "analyst@example.com"
        }"#;

        let req: KycUpdateStatusDryRunRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.transition_ref, "kyc-case.intake-to-discovery");
        assert!(req.actor_roles.is_empty());
    }

    #[test]
    fn test_kyc_update_status_dry_run_value_success() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let req = KycUpdateStatusDryRunRequest {
            case_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
            current_state: "INTAKE".to_string(),
            requested_state: "DISCOVERY".to_string(),
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "state-snapshot-1".to_string(),
            evidence_digest: "sha256:case".to_string(),
            actor_id: "analyst@example.com".to_string(),
            actor_roles: vec!["analyst".to_string()],
        };

        let value =
            dry_run_kyc_update_status_workbook_value(session_id, req).expect("dry-run succeeds");

        assert_eq!(value["status"], "dry_run_validated");
        assert_eq!(
            value["dry_run"]["transition_ref"],
            "kyc-case.intake-to-discovery"
        );
        assert_eq!(value["dry_run"]["semantic_diff"]["to_state"], "DISCOVERY");
        assert!(value["dry_run"]["semantic_diff_uri"]
            .as_str()
            .unwrap()
            .starts_with("semos://semantic-diff/ewb:v1:"));
        assert!(value["dry_run"]["validation_trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step_id"] == "integrity"));
        assert!(value["workbook"]["id"]
            .as_str()
            .unwrap()
            .starts_with("ewb:v1:"));
    }

    #[test]
    fn test_kyc_update_status_dry_run_value_refuses_illegal_transition() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let req = KycUpdateStatusDryRunRequest {
            case_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
            current_state: "REVIEW".to_string(),
            requested_state: "DISCOVERY".to_string(),
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "state-snapshot-1".to_string(),
            evidence_digest: "sha256:case".to_string(),
            actor_id: "analyst@example.com".to_string(),
            actor_roles: vec![],
        };

        let err =
            dry_run_kyc_update_status_workbook_value(session_id, req).expect_err("dry-run refused");

        assert!(matches!(
            err,
            crate::runbook::KycUpdateStatusDryRunRefusal::SimulationRefused { .. }
        ));
        let (status, _) = kyc_dry_run_json_error(err);
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[test]
    fn test_kyc_approval_token_value_issues_workbook_bound_token() {
        let output = dry_run_output();
        let req = KycApprovalTokenRequest {
            workbook: output.workbook.clone(),
            approved_by_actor_id: "approver@example.com".to_string(),
            approval_text: "Approved for restricted KYC update".to_string(),
            expires_at: test_expires_at(),
        };

        let value =
            issue_kyc_workbook_approval_token_value(test_session_id(), req).expect("token issued");

        assert_eq!(value["status"], "approval_token_issued");
        assert!(value["approval_token"]["id"]
            .as_str()
            .unwrap()
            .starts_with("approval:v1:"));
        assert_eq!(
            value["approval_token"]["core"]["workbook_id"],
            output.workbook.id.to_string()
        );
    }

    #[test]
    fn test_kyc_approval_token_value_refuses_session_mismatch() {
        let output = dry_run_output();
        let req = KycApprovalTokenRequest {
            workbook: output.workbook,
            approved_by_actor_id: "approver@example.com".to_string(),
            approval_text: "Approved for restricted KYC update".to_string(),
            expires_at: test_expires_at(),
        };
        let other_session_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

        let err = issue_kyc_workbook_approval_token_value(other_session_id, req)
            .expect_err("session mismatch refused");

        assert!(matches!(
            err,
            crate::runbook::ApprovalTokenValidationError::WorkbookBindingMismatch { .. }
        ));
        let (status, _) = approval_token_json_error(err);
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_kyc_restricted_mutation_preflight_refuses_dry_run_only_seed() {
        let output = dry_run_output();
        let token = crate::runbook::create_approval_token_for_workbook(
            &output.workbook,
            "approver@example.com",
            "Approved for restricted KYC update",
            test_expires_at(),
            chrono::Utc::now(),
        )
        .expect("approval token");
        let req = KycRestrictedMutationPreflightRequest {
            workbook: output.workbook.clone(),
            approval_token: token,
            observed_configuration_version: output.workbook.core.configuration_version.clone(),
            observed_state_snapshot_id: output.workbook.core.state_snapshot_id.clone(),
            observed_evidence_refs: output.workbook.core.evidence_refs.clone(),
            consumed_token_ids: vec![],
        };

        let err = prepare_kyc_restricted_mutation_preflight_value(test_session_id(), req)
            .expect_err("dry-run-only seed refused");

        assert!(matches!(
            err,
            crate::runbook::RestrictedMutationPreflightError::ApprovalRefused {
                error: crate::runbook::ApprovalTokenValidationError::TransitionMutationNotEnabled { .. }
            }
        ));
        let (status, _) = preflight_json_error(err);
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[test]
    fn test_kyc_restricted_mutation_preflight_value_success_with_reference_manifest() {
        let output = dry_run_output();
        let token = crate::runbook::create_approval_token_for_workbook(
            &output.workbook,
            "approver@example.com",
            "Approved for restricted KYC update",
            test_expires_at(),
            chrono::Utc::now(),
        )
        .expect("approval token");
        let req = KycRestrictedMutationPreflightRequest {
            workbook: output.workbook.clone(),
            approval_token: token.clone(),
            observed_configuration_version: output.workbook.core.configuration_version.clone(),
            observed_state_snapshot_id: output.workbook.core.state_snapshot_id.clone(),
            observed_evidence_refs: output.workbook.core.evidence_refs.clone(),
            consumed_token_ids: vec![],
        };

        let value = prepare_kyc_restricted_mutation_preflight_value_with_manifest(
            test_session_id(),
            req,
            &reference_mutation_manifest(),
        )
        .expect("preflight prepared");

        assert_eq!(value["status"], "restricted_mutation_preflight_prepared");
        assert_eq!(value["preflight"]["executor"], "existing_runbook_gate_only");
        assert_eq!(
            value["preflight"]["approval"]["approval_token_id"],
            token.id.to_string()
        );
        assert!(value["preflight"]["actual_diff"].is_null());
    }

    #[test]
    fn test_kyc_restricted_mutation_compile_runbook_value_success() {
        let output = dry_run_output();
        let token = crate::runbook::create_approval_token_for_workbook(
            &output.workbook,
            "approver@example.com",
            "Approved for restricted KYC update",
            test_expires_at(),
            chrono::Utc::now(),
        )
        .expect("approval token");
        let preflight_req = KycRestrictedMutationPreflightRequest {
            workbook: output.workbook.clone(),
            approval_token: token,
            observed_configuration_version: output.workbook.core.configuration_version.clone(),
            observed_state_snapshot_id: output.workbook.core.state_snapshot_id.clone(),
            observed_evidence_refs: output.workbook.core.evidence_refs.clone(),
            consumed_token_ids: vec![],
        };
        let preflight_value = prepare_kyc_restricted_mutation_preflight_value_with_manifest(
            test_session_id(),
            preflight_req,
            &reference_mutation_manifest(),
        )
        .expect("preflight prepared");
        let preflight: crate::runbook::RestrictedMutationPreflight =
            serde_json::from_value(preflight_value["preflight"].clone()).expect("preflight json");

        let value = compile_kyc_restricted_mutation_runbook_value(
            test_session_id(),
            3,
            KycRestrictedMutationCompileRunbookRequest { preflight },
        )
        .expect("compiled runbook");

        assert_eq!(value["status"], "restricted_mutation_runbook_compiled");
        assert_eq!(value["compilation"]["compiled_runbook"]["version"], 3);
        assert_eq!(
            value["compilation"]["compiled_runbook"]["steps"][0]["verb"],
            "kyc-case.update-status"
        );
        assert_eq!(
            value["compilation"]["compiled_runbook"]["steps"][0]["args"]["status"],
            "ASSESSMENT"
        );
        assert_eq!(
            value["compilation"]["compiled_runbook"]["steps"][0]["write_set"][0],
            output.workbook.core.subject.subject_id.to_string()
        );
    }

    #[test]
    fn test_kyc_restricted_mutation_receipt_value_records_actual_diff() {
        let output = dry_run_output();
        let token = crate::runbook::create_approval_token_for_workbook(
            &output.workbook,
            "approver@example.com",
            "Approved for restricted KYC update",
            test_expires_at(),
            chrono::Utc::now(),
        )
        .expect("approval token");
        let preflight_req = KycRestrictedMutationPreflightRequest {
            workbook: output.workbook.clone(),
            approval_token: token,
            observed_configuration_version: output.workbook.core.configuration_version.clone(),
            observed_state_snapshot_id: output.workbook.core.state_snapshot_id.clone(),
            observed_evidence_refs: output.workbook.core.evidence_refs.clone(),
            consumed_token_ids: vec![],
        };
        let preflight_value = prepare_kyc_restricted_mutation_preflight_value_with_manifest(
            test_session_id(),
            preflight_req,
            &reference_mutation_manifest(),
        )
        .expect("preflight prepared");
        let preflight: crate::runbook::RestrictedMutationPreflight =
            serde_json::from_value(preflight_value["preflight"].clone()).expect("preflight json");
        let compilation_value = compile_kyc_restricted_mutation_runbook_value(
            test_session_id(),
            3,
            KycRestrictedMutationCompileRunbookRequest {
                preflight: preflight.clone(),
            },
        )
        .expect("compiled runbook");
        let compilation: crate::runbook::RestrictedMutationRunbookCompilation =
            serde_json::from_value(compilation_value["compilation"].clone())
                .expect("compilation json");

        let value = record_kyc_restricted_mutation_receipt_value(
            test_session_id(),
            KycRestrictedMutationReceiptRequest {
                preflight: preflight.clone(),
                compilation,
                actual_diff: preflight.intended_diff.clone(),
            },
        )
        .expect("receipt recorded");

        assert_eq!(value["status"], "restricted_mutation_execution_recorded");
        assert_eq!(
            value["receipt"]["intended_diff"],
            value["receipt"]["actual_diff"]
        );
        assert_eq!(
            value["receipt"]["predicted_diff"]["semantic_diff"]["after"],
            "ASSESSMENT"
        );
    }

    #[test]
    fn test_kyc_restricted_mutation_receipt_refuses_actual_diff_drift() {
        let output = dry_run_output();
        let token = crate::runbook::create_approval_token_for_workbook(
            &output.workbook,
            "approver@example.com",
            "Approved for restricted KYC update",
            test_expires_at(),
            chrono::Utc::now(),
        )
        .expect("approval token");
        let preflight_value = prepare_kyc_restricted_mutation_preflight_value_with_manifest(
            test_session_id(),
            KycRestrictedMutationPreflightRequest {
                workbook: output.workbook.clone(),
                approval_token: token,
                observed_configuration_version: output.workbook.core.configuration_version.clone(),
                observed_state_snapshot_id: output.workbook.core.state_snapshot_id.clone(),
                observed_evidence_refs: output.workbook.core.evidence_refs.clone(),
                consumed_token_ids: vec![],
            },
            &reference_mutation_manifest(),
        )
        .expect("preflight prepared");
        let preflight: crate::runbook::RestrictedMutationPreflight =
            serde_json::from_value(preflight_value["preflight"].clone()).expect("preflight json");
        let compilation: crate::runbook::RestrictedMutationRunbookCompilation =
            serde_json::from_value(
                compile_kyc_restricted_mutation_runbook_value(
                    test_session_id(),
                    3,
                    KycRestrictedMutationCompileRunbookRequest {
                        preflight: preflight.clone(),
                    },
                )
                .expect("compiled runbook")["compilation"]
                    .clone(),
            )
            .expect("compilation json");
        let mut actual_diff = preflight.intended_diff.clone();
        actual_diff.after = "APPROVED".to_string();

        let err = record_kyc_restricted_mutation_receipt_value(
            test_session_id(),
            KycRestrictedMutationReceiptRequest {
                preflight,
                compilation,
                actual_diff,
            },
        )
        .expect_err("actual diff drift refused");

        assert!(matches!(
            err,
            crate::runbook::RestrictedMutationRunbookCompilationError::ActualDiffMismatch { .. }
        ));
    }

    #[test]
    fn test_acp_open_session_value_defaults_to_zed_without_mutation() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let req: AcpOpenSessionRequest = serde_json::from_str("{}").unwrap();

        let value = open_acp_session_value(session_id, req);

        assert_eq!(value["status"], "acp_session_open");
        assert_eq!(value["session"]["adapter"], "zed");
        assert_eq!(value["session"]["mutation_capability"], "none");
    }

    #[test]
    fn test_acp_capabilities_value_advertises_stdio_and_session_lifecycle() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let value = acp_capabilities_value(session_id).expect("capabilities resolves");

        assert_eq!(value["status"], "acp_capabilities");
        assert_eq!(value["stdio"]["command"], "ob_poc_acp");
        assert_eq!(
            value["protocol"]["agentCapabilities"]["sessionCapabilities"]["close"],
            true
        );
        assert_eq!(
            value["protocol"]["agentCapabilities"]["sessionCapabilities"]["list"],
            true
        );
        // Session-level Domain Pack policy is now surfaced in the same response.
        assert!(value["policy"].is_object());
    }

    #[test]
    fn test_acp_policy_value_exposes_semos_policy_decisions() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let value = acp_policy_value(session_id).expect("policy");

        assert_eq!(value["status"], "acp_policy");
        assert_eq!(value["policy"]["pack_id"], "ob-poc.kyc");
        assert_eq!(
            value["policy"]["adapter_policy"]["policy_authority"],
            "SemOS Domain Pack + Workbook + Runbook Gate"
        );
        assert_eq!(
            value["policy"]["adapter_policy"]["direct_mutation_supported"],
            false
        );
        assert!(value["policy"]["transition_policy"]
            .as_array()
            .unwrap()
            .iter()
            .any(|transition| transition["dry_run_allowed"] == true
                && transition["mutation_allowed"] == false));
    }

    #[test]
    fn test_acp_projection_catalog_value_exposes_visibility_surface() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let value = list_acp_projections_value(session_id).expect("projection catalog");

        assert_eq!(value["status"], "acp_projection_catalog");
        assert!(value["projections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "dag"));
        assert!(value["projections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "verb_surface"));
    }

    #[test]
    fn test_acp_projection_value_returns_hashed_envelope() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let value = get_acp_projection_value(session_id, "probe_catalogue").expect("projection");

        assert_eq!(value["status"], "acp_projection");
        assert_eq!(value["projection"]["projection_kind"], "probe_catalogue");
        assert!(value["projection"]["projection_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(
            value["projection"]["payload"]["probes"][0]["probe_id"],
            "kyc-case.read-state"
        );
    }

    #[test]
    fn test_acp_close_session_value_marks_session_closed() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let req: AcpOpenSessionRequest = serde_json::from_str("{}").unwrap();

        let value = close_acp_session_value(session_id, req);

        assert_eq!(value["status"], "acp_session_closed");
        assert_eq!(value["session"]["state"], "closed");
    }

    #[test]
    fn test_acp_context_value_redacts_required_context() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let req: AcpContextAssemblyRequest = serde_json::from_value(serde_json::json!({
            "adapter": "test_harness",
            "probe_id": "kyc-case.read-evidence-summary",
            "subject_kind": "kyc_case",
            "subject_id": "11111111-1111-1111-1111-111111111111",
            "observations": [
                {
                    "key": "case.status",
                    "value": "INTAKE",
                    "classification": "internal"
                },
                {
                    "key": "case.confidential_evidence.summary",
                    "value": "raw",
                    "classification": "internal"
                }
            ]
        }))
        .unwrap();

        let value = assemble_acp_context_value(session_id, req).expect("context assembled");

        assert_eq!(value["status"], "acp_context_assembled");
        assert_eq!(
            value["bundle"]["prompt_context"]["included"][0]["key"],
            "case.status"
        );
        assert_eq!(
            value["bundle"]["prompt_context"]["redacted"][0]["key"],
            "case.confidential_evidence.summary"
        );
    }

    #[test]
    fn test_acp_context_value_refuses_unknown_probe() {
        let session_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let req: AcpContextAssemblyRequest = serde_json::from_value(serde_json::json!({
            "probe_id": "kyc-case.write-state",
            "subject_kind": "kyc_case",
            "subject_id": "11111111-1111-1111-1111-111111111111"
        }))
        .unwrap();

        let err = assemble_acp_context_value(session_id, req).expect_err("probe refused");

        assert!(matches!(
            err,
            crate::acp::AcpAdapterError::DiscoveryRefused { .. }
        ));
        let (status, _) = acp_json_error(err);
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_acp_trace_append_records_context_assembly() {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        let state = ReplV2RouteState {
            orchestrator: orchestrator.clone(),
        };

        append_session_trace_if_present(
            &state,
            session_id,
            crate::repl::session_trace::TraceOp::AcpContextAssembled {
                pack_id: "ob-poc.kyc".to_string(),
                probe_id: "kyc-case.read-state".to_string(),
                context_hash: "sha256:test".to_string(),
                redacted_count: 1,
            },
        )
        .await;

        let session = orchestrator.get_session(session_id).await.unwrap();
        assert_eq!(session.trace.len(), 1);
        assert!(matches!(
            &session.trace[0].op,
            crate::repl::session_trace::TraceOp::AcpContextAssembled {
                pack_id,
                probe_id,
                context_hash,
                redacted_count
            } if pack_id == "ob-poc.kyc"
                && probe_id == "kyc-case.read-state"
                && context_hash == "sha256:test"
                && *redacted_count == 1
        ));
    }

    #[tokio::test]
    async fn test_acp_language_loop_route_persists_dry_run_trace() {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        let state = ReplV2RouteState {
            orchestrator: orchestrator.clone(),
        };

        let value = acp_kyc_update_status_language_loop_route(
            State(state.clone()),
            Path(session_id),
            Json(language_loop_request(
                session_id,
                "kyc-case.update-status",
                None,
            )),
        )
        .await
        .expect("language loop route")
        .0;

        assert_eq!(value["status"], "dry_run_validated");
        assert_eq!(value["metrics"]["revision_count"], 1);

        let trace = get_session_trace(State(state), Path(session_id))
            .await
            .expect("session trace")
            .0;
        assert_eq!(trace[0]["op"]["op"], "acp_language_loop_traced");
        assert_eq!(trace[0]["op"]["outcome"], "dry_run_validated");
        assert_eq!(
            trace[0]["op"]["transition_ref"],
            "kyc-case.intake-to-discovery"
        );
        assert!(trace[0]["op"]["semantic_diff_uri"]
            .as_str()
            .unwrap()
            .starts_with("semos://semantic-diff/"));
        assert_eq!(
            trace[0]["op"]["conversation_efficiency"]["prose_only_failure"],
            false
        );
        assert_eq!(
            trace[0]["op"]["conversation_efficiency"]["estimated_user_repair_turns_avoided"],
            1
        );
    }

    #[tokio::test]
    async fn test_acp_language_loop_route_persists_structured_refusal_trace() {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        let state = ReplV2RouteState {
            orchestrator: orchestrator.clone(),
        };

        let value = acp_kyc_update_status_language_loop_route(
            State(state.clone()),
            Path(session_id),
            Json(language_loop_request(
                session_id,
                "kyc-case.delete",
                Some(test_case_id()),
            )),
        )
        .await
        .expect("language loop route")
        .0;

        assert_eq!(value["status"], "structured_refusal");
        assert_eq!(value["refusal"]["refusal_code"], "invented_verb");

        let session = orchestrator.get_session(session_id).await.unwrap();
        assert_eq!(session.trace.len(), 1);
        assert!(matches!(
            &session.trace[0].op,
            crate::repl::session_trace::TraceOp::AcpLanguageLoopTraced {
                outcome,
                refusal_code,
                diagnostic_source_path,
                dry_run_valid,
                conversation_efficiency,
                ..
            } if outcome == "structured_refusal"
                && refusal_code.as_deref() == Some("invented_verb")
                && diagnostic_source_path.as_deref() == Some("draft.verb")
                && !dry_run_valid
                && conversation_efficiency.pending_user_turn_required
                && !conversation_efficiency.prose_only_failure
        ));
    }

    #[tokio::test]
    async fn test_acp_prompt_route_persists_pending_question_trace() {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        let state = ReplV2RouteState {
            orchestrator: orchestrator.clone(),
        };

        let value = acp_prompt_route(
            State(state.clone()),
            Path(session_id),
            Json(AcpPromptRouteRequest {
                prompt: vec![crate::acp_protocol::AcpContentBlock::Text {
                    text: "update status for KYC case".to_string(),
                }],
            }),
        )
        .await
        .expect("prompt route")
        .0;

        assert_eq!(value["result"]["status"], "pending_question");
        assert_eq!(
            value["result"]["pending_question"]["code"],
            "kyc_update_status_prompt_incomplete"
        );

        let trace = get_session_trace(State(state), Path(session_id))
            .await
            .expect("session trace")
            .0;
        assert_eq!(trace[0]["op"]["op"], "acp_language_loop_traced");
        assert_eq!(trace[0]["op"]["outcome"], "pending_question");
        assert_eq!(
            trace[0]["op"]["pending_question_code"],
            "kyc_update_status_prompt_incomplete"
        );
        assert!(trace[0]["op"]["needed_from_user"]
            .as_array()
            .unwrap()
            .iter()
            .any(|need| need == "case_uuid"));
        assert_eq!(
            trace[0]["op"]["conversation_efficiency"]["prose_only_failure"],
            false
        );
        assert!(trace[0]["op"]["human_summary"]
            .as_str()
            .unwrap()
            .contains("Need HITL clarification"));
    }
}

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
use sem_os_policy::acp_projection::{
    AcpProjectionEnvelope, AcpProjectionEnvelopeInput, AcpProjectionKind,
};
use sem_os_policy::domain_pack::DomainPackManifest;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use std::time::Instant;
use uuid::Uuid;

use crate::acp_state_anchor::{
    acp_prompt_blocks_from_params, acp_prompt_state_anchor_provider_outcome,
    AcpPromptStateAnchorProviderOutcome,
};
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

static REPL_ACP_AGENTS: LazyLock<
    tokio::sync::Mutex<std::collections::BTreeMap<Uuid, crate::acp_protocol::AcpJsonRpcAgent>>,
> = LazyLock::new(|| tokio::sync::Mutex::new(std::collections::BTreeMap::new()));

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
    pub observations: Vec<sem_os_policy::domain_pack::DiscoveryObservation>,
    #[serde(default)]
    pub provenance: Vec<sem_os_policy::domain_pack::DiscoveryProvenance>,
    #[serde(default)]
    pub first_class_state_mutated: bool,
}

/// Generic ACP JSON-RPC gateway request for the REPL HTTP boundary.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcpGatewayRouteRequest {
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone)]
struct KycUpdateStatusLlmDraftInput {
    adapter: crate::acp::AcpAdapterKind,
    subject_id: Uuid,
    current_state: String,
    configuration_version: String,
    state_snapshot_id: String,
    objective: Option<String>,
    evidence_digest: Option<String>,
    actor_id: Option<String>,
    actor_roles: Vec<String>,
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

fn load_plan_verb_outputs() -> anyhow::Result<
    std::collections::BTreeMap<String, Vec<sem_os_ontology::verb_contract::VerbOutput>>,
> {
    let verbs_config = dsl_core::ConfigLoader::from_env().load_verbs()?;
    let mut outputs =
        std::collections::BTreeMap::<String, Vec<sem_os_ontology::verb_contract::VerbOutput>>::new(
        );
    for verb in crate::sem_reg::onboarding::verb_extract::extract_verbs(&verbs_config) {
        let Some(output) = verb.output else {
            continue;
        };
        outputs
            .entry(verb.fqn)
            .or_default()
            .push(sem_os_ontology::verb_contract::VerbOutput {
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
        acp_dag_semantic: None,
        bpmn_form: None,
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

    #[cfg(feature = "database")]
    let turn_lock = state
        .orchestrator
        .acquire_session_turn_record_lock(request.context.session_id)
        .await
        .map_err(anyhow_json_error)?;

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
        .persist_session_checkpoint_with_record_lock(request.context.session_id)
        .await
        .map_err(anyhow_json_error)?;
    #[cfg(feature = "database")]
    if let Some(lock) = turn_lock {
        lock.release().await.map_err(anyhow_json_error)?;
    }

    Ok(Json(SessionFeedbackResponse { resolved, feedback }))
}

async fn commit_session_context(
    State(state): State<ReplV2RouteState>,
    Json(request): Json<SessionStackRequest>,
) -> Result<Json<SessionFeedback>, (StatusCode, Json<ErrorResponseV2>)> {
    #[cfg(feature = "database")]
    let turn_lock = state
        .orchestrator
        .acquire_session_turn_record_lock(request.session_id)
        .await
        .map_err(anyhow_json_error)?;

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
        .persist_session_checkpoint_with_record_lock(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    #[cfg(feature = "database")]
    if let Some(lock) = turn_lock {
        lock.release().await.map_err(anyhow_json_error)?;
    }
    Ok(Json(feedback))
}

async fn pop_session_context(
    State(state): State<ReplV2RouteState>,
    Json(request): Json<SessionStackRequest>,
) -> Result<Json<SessionFeedback>, (StatusCode, Json<ErrorResponseV2>)> {
    #[cfg(feature = "database")]
    let turn_lock = state
        .orchestrator
        .acquire_session_turn_record_lock(request.session_id)
        .await
        .map_err(anyhow_json_error)?;

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
        .persist_session_checkpoint_with_record_lock(request.session_id)
        .await
        .map_err(anyhow_json_error)?;
    #[cfg(feature = "database")]
    if let Some(lock) = turn_lock {
        lock.release().await.map_err(anyhow_json_error)?;
    }
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
        // (capabilities route removed — ACP clients use stdio init; HTTP
        // consumers should call /acp/policy for policy and the ob_poc_acp
        // binary docs for stdio launch metadata.)
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
        .route("/api/session/:id/acp/gateway", post(acp_gateway_route))
        // Session trace routes (R9)
        .route("/api/session/:id/trace", get(get_session_trace))
        .route("/api/session/:id/trace/:seq", get(get_trace_entry))
        .route("/api/session/:id/trace/replay", post(replay_session_trace))
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
    let facade = crate::acp_facade::AcpFacade::for_default_pack(crate::acp::AcpAdapterKind::Zed)?;
    let policy = facade.policy(session_id)?;

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
    let facade = crate::acp_facade::AcpFacade::for_default_pack(crate::acp::AcpAdapterKind::Zed)?;
    let pack_id = facade.manifest().pack_id.clone();
    let projections = facade.projections_list(session_id)?;

    Ok(serde_json::json!({
        "status": "acp_projection_catalog",
        "session_id": session_id,
        "pack_id": pack_id,
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
    let facade = crate::acp_facade::AcpFacade::for_default_pack(crate::acp::AcpAdapterKind::Zed)?;
    let parsed_kind = kind.parse::<AcpProjectionKind>().map_err(|_| {
        crate::acp::AcpAdapterError::ProjectionUnknown {
            projection_kind: kind.to_string(),
        }
    })?;

    if let Some(repl_session) = state.orchestrator.get_session(session_id).await {
        let session =
            facade.open_session_with_persona(session_id, crate::acp::AcpPersonaMode::SagePlanning);
        if let Some(projection) =
            build_live_acp_projection(&session, facade.manifest(), &repl_session, parsed_kind)?
        {
            return Ok(serde_json::json!({
                "status": "acp_projection",
                "projection": projection,
            }));
        }
    }

    get_acp_projection_value(session_id, parsed_kind.as_str())
}

fn get_acp_projection_value(
    session_id: Uuid,
    kind: &str,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let facade = crate::acp_facade::AcpFacade::for_default_pack(crate::acp::AcpAdapterKind::Zed)?;
    let kind = kind
        .parse::<sem_os_policy::acp_projection::AcpProjectionKind>()
        .map_err(|_| crate::acp::AcpAdapterError::ProjectionUnknown {
            projection_kind: kind.to_string(),
        })?;
    let projection = facade.projection_get(
        session_id,
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

/// HTTP-only live overlay for ACP projection envelopes.
///
/// Returns `Some(envelope)` when this projection kind has a live materializer
/// against the current `ReplSessionV2`, or `None` to let the caller fall back
/// to the declared-source view in `acp::build_acp_projection`. Stdio (Zed)
/// clients do not call this — they get the declared-source view as designed.
///
/// Phase C will fold this overlay into a single `AcpDomainFacade` entry point
/// shared by REST and stdio. See `acp::build_acp_projection` for the
/// canonical declared-source contract.
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
            "entity_resolution": repl_session.last_entity_resolution,
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
            "entity_resolution": repl_session.last_entity_resolution,
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
    // open_session_with_persona is a transport convenience; manifest is not
    // needed for session-only operations so we construct the facade lazily
    // only when domain calls are required.
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
    REPL_ACP_AGENTS.lock().await.remove(&session_id);
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
    let facade = crate::acp_facade::AcpFacade::for_default_pack(req.adapter)?;
    let subject = sem_os_policy::domain_pack::DiscoverySubject {
        subject_kind: req.subject_kind,
        subject_id: req.subject_id,
    };
    let probe_id = req.probe_id;
    let discovery_request = sem_os_policy::domain_pack::DiscoveryRequest {
        pack_id: facade.manifest().pack_id.clone(),
        probe_id: probe_id.clone(),
        subject: subject.clone(),
        context: req.context,
    };
    let discovery_response = sem_os_policy::domain_pack::DiscoveryResponse {
        probe_id,
        subject,
        observations: req.observations,
        provenance: req.provenance,
        first_class_state_mutated: req.first_class_state_mutated,
    };

    let bundle = facade.context_assemble(session_id, discovery_request, discovery_response)?;

    Ok(serde_json::json!({
        "status": "acp_context_assembled",
        "bundle": bundle,
    }))
}

pub(crate) async fn handle_repl_acp_request(
    session_id: Uuid,
    request: crate::acp_protocol::JsonRpcRequest,
) -> Vec<crate::acp_protocol::JsonRpcOutgoing> {
    let mut agents = REPL_ACP_AGENTS.lock().await;
    let agent = agents
        .entry(session_id)
        .or_insert_with(crate::acp_protocol::AcpJsonRpcAgent::new);
    agent.handle_request(request)
}

fn json_rpc_outgoing_result(
    outgoing: &[crate::acp_protocol::JsonRpcOutgoing],
) -> Option<serde_json::Value> {
    outgoing.iter().rev().find_map(|item| match item {
        crate::acp_protocol::JsonRpcOutgoing::Response(response) => response.result.clone(),
        crate::acp_protocol::JsonRpcOutgoing::Notification(_) => None,
    })
}

async fn resolve_acp_prompt_language_loop_request(
    session_id: Uuid,
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> Result<crate::acp_protocol::AcpKycLanguageLoopRequest, Vec<&'static str>> {
    let mut agents = REPL_ACP_AGENTS.lock().await;
    let agent = agents
        .entry(session_id)
        .or_insert_with(crate::acp_protocol::AcpJsonRpcAgent::new);
    agent.kyc_update_status_language_loop_request_from_prompt(session_id, prompt)
}

/// POST /api/session/:id/acp/gateway
async fn acp_gateway_route(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<AcpGatewayRouteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    acp_gateway_route_with_llm_client(state, session_id, req, None).await
}

async fn acp_gateway_route_with_llm_client(
    state: ReplV2RouteState,
    session_id: Uuid,
    req: AcpGatewayRouteRequest,
    llm_client: Option<Result<std::sync::Arc<dyn ob_agentic::llm_client::LlmClient>, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    let params = acp_gateway_params(session_id, &req.method, req.params);
    if req.method == "session/prompt" {
        let prompt = acp_prompt_blocks_from_params(&params).unwrap_or_default();
        let mut envelope = if acp_prompt_params_request_llm_draft(&params)? {
            let client = llm_client.unwrap_or_else(|| {
                ob_agentic::create_llm_client().map_err(|error| error.to_string())
            });
            process_acp_prompt_llm_envelope(
                &state,
                session_id,
                prompt,
                serde_json::json!("repl-acp-gateway"),
                "acp_gateway_processed",
                client,
            )
            .await?
        } else {
            process_acp_prompt_deterministic_envelope(
                &state,
                session_id,
                prompt,
                serde_json::json!("repl-acp-gateway"),
                "acp_gateway_processed",
            )
            .await
        };
        if let Some(object) = envelope.as_object_mut() {
            object.insert("method".to_string(), serde_json::json!(req.method));
        }
        return Ok(Json(envelope));
    }

    let outgoing = handle_repl_acp_request(
        session_id,
        crate::acp_protocol::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("repl-acp-gateway")),
            method: req.method.clone(),
            params,
        },
    )
    .await;
    let result = json_rpc_outgoing_result(&outgoing).unwrap_or_else(|| serde_json::json!({}));
    if let Some(op) = acp_language_loop_trace_op_from_value(&result) {
        append_session_trace_if_present(&state, session_id, op).await;
    }

    Ok(Json(serde_json::json!({
        "status": "acp_gateway_processed",
        "session_id": session_id,
        "method": req.method,
        "result": result,
        "outgoing": outgoing,
    })))
}

fn acp_gateway_params(
    session_id: Uuid,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let mut params = match params {
        serde_json::Value::Object(fields) => fields,
        _ => serde_json::Map::new(),
    };
    let session_id_string = session_id.to_string();
    match method {
        "session/prompt" => {
            params
                .entry("sessionId")
                .or_insert_with(|| serde_json::json!(session_id_string));
        }
        "session/load" | "session/close" | "session/cancel" => {
            params
                .entry("sessionId")
                .or_insert_with(|| serde_json::json!(session_id_string.clone()));
            params
                .entry("session_id")
                .or_insert_with(|| serde_json::json!(session_id_string));
        }
        _ => {
            params
                .entry("session_id")
                .or_insert_with(|| serde_json::json!(session_id_string.clone()));
            params
                .entry("sessionId")
                .or_insert_with(|| serde_json::json!(session_id_string));
        }
    }
    serde_json::Value::Object(params)
}

fn acp_prompt_params_request_llm_draft(
    params: &serde_json::Value,
) -> Result<bool, (StatusCode, Json<ErrorResponseV2>)> {
    let draft_source = params
        .get("draft_source")
        .or_else(|| params.get("draftSource"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("deterministic");
    match draft_source {
        "deterministic" | "deterministic_draft" => Ok(false),
        "llm" | "llm_tool_call" | "live_llm" => Ok(true),
        draft_source => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponseV2 {
                error: format!(
                    "unsupported ACP prompt draft_source `{draft_source}`; expected deterministic or llm_tool_call"
                ),
                recoverable: true,
            }),
        )),
    }
}

pub(crate) async fn process_acp_prompt_deterministic_envelope(
    state: &ReplV2RouteState,
    session_id: Uuid,
    prompt: Vec<crate::acp_protocol::AcpContentBlock>,
    response_id: serde_json::Value,
    envelope_status: &'static str,
) -> serde_json::Value {
    let provider_outcome =
        acp_prompt_state_anchor_provider_outcome(state, session_id, &prompt, response_id.clone())
            .await;
    let (mut outgoing, provider_report) = match provider_outcome {
        AcpPromptStateAnchorProviderOutcome::Continue { outgoing, report } => (outgoing, report),
        AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report } => {
            let result = json_rpc_outgoing_result(&outgoing);
            let state_anchor_provider = report.metrics(result.as_ref());
            if let Some(result) = &result {
                if let Some(op) = acp_language_loop_trace_op_from_value(result) {
                    append_session_trace_if_present(state, session_id, op).await;
                }
            }

            return serde_json::json!({
                "status": envelope_status,
                "session_id": session_id,
                "result": result.clone().unwrap_or_else(|| serde_json::json!({})),
                "outgoing": outgoing,
                "state_anchor_provider": state_anchor_provider,
            });
        }
    };
    let prompt_outgoing = handle_repl_acp_request(
        session_id,
        crate::acp_protocol::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(response_id),
            method: "session/prompt".to_string(),
            params: serde_json::json!({
                "sessionId": session_id.to_string(),
                "prompt": prompt,
            }),
        },
    )
    .await;
    outgoing.extend(prompt_outgoing);
    let mut result = json_rpc_outgoing_result(&outgoing);
    if let Some(result) = result.as_mut() {
        if let Some(session) = state.orchestrator.get_session(session_id).await {
            attach_session_runtime_trace_to_result(result, &session);
        }
    }

    if let Some(result) = &result {
        if let Some(op) = acp_language_loop_trace_op_from_value(result) {
            append_session_trace_if_present(state, session_id, op).await;
        }
    }

    let state_anchor_provider = provider_report.metrics(result.as_ref());
    serde_json::json!({
        "status": envelope_status,
        "session_id": session_id,
        "result": result.clone().unwrap_or_else(|| serde_json::json!({})),
        "outgoing": outgoing,
        "state_anchor_provider": state_anchor_provider,
    })
}

/// R8 Phase B.8 (2026-05-11): typed computation of the session-aware
/// runtime trace. Reads typed fields off `&AcpDagSemanticResolution`
/// and `&ReplSessionV2`, produces a typed `AcpDagSemanticRuntimeTrace`.
///
/// Replaces the previous JSON-mutation implementation. The typed value
/// is the canonical source; `attach_session_runtime_trace_to_result`
/// (below) serializes it back into the JSON envelope for non-typed
/// consumers (ACP JSON-RPC server, envelope byte-equality baseline).
pub(crate) fn compute_session_aware_runtime_trace_typed(
    resolution: &crate::acp_dag_semantic::AcpDagSemanticResolution,
    session: &ReplSessionV2,
) -> Option<crate::acp_dag_semantic::AcpDagSemanticRuntimeTrace> {
    let pack_id = resolution.pack.as_ref()?.pack_id.clone();
    let static_envelope_hash = resolution
        .envelope_trace
        .as_ref()
        .map(|t| t.envelope_hash.clone())?;
    let selected_ref = resolution
        .selected_verb
        .clone()
        .or_else(|| {
            resolution
                .selected_template
                .as_ref()
                .map(|t| t.template_id.clone())
        })
        .unwrap_or_else(|| "unknown".to_string());
    let missing_required_args = resolution.missing_required_args.clone();
    let source = crate::acp_runtime_context::build_session_runtime_context_source(
        crate::acp_runtime_context::AcpRuntimeContextBuildInput {
            pack_id,
            selected_ref,
            static_envelope_hash,
            session: Some(session),
            missing_required_args,
        },
    );
    let projection = crate::acp_runtime_context::build_acp_runtime_context_projection(source);
    Some(crate::acp_dag_semantic::AcpDagSemanticRuntimeTrace {
        schema_version: projection.schema_version,
        pack_id: projection.pack_id,
        snapshot_id: projection.snapshot_id,
        runtime_hash: projection.runtime_hash,
        redaction_policy: projection.redaction_policy,
        freshness_policy: projection.freshness_policy,
        static_envelope_hash: projection.static_envelope_hash,
        projection_hash: projection.projection_hash,
        verified: projection.verified,
        redacted_count: projection.redacted_count,
        blocked_field_codes: projection.blocked_field_codes,
    })
}

/// R8 Phase B.8 (2026-05-11): JSON wrapper around the typed runtime
/// trace computation. Parses the result's `dag_semantic` block into the
/// typed resolution, calls the typed computation, then serializes the
/// typed trace back into both legacy JSON paths (`traceProjection.
/// runtimeTrace` and `dag_semantic.runtime_trace`). This keeps the
/// envelope wire shape unchanged while making typed code the canonical
/// source of truth.
fn attach_session_runtime_trace_to_result(result: &mut serde_json::Value, session: &ReplSessionV2) {
    let Some(resolution) = result.get("dag_semantic").and_then(|v| {
        serde_json::from_value::<crate::acp_dag_semantic::AcpDagSemanticResolution>(v.clone()).ok()
    }) else {
        return;
    };
    let Some(typed_trace) = compute_session_aware_runtime_trace_typed(&resolution, session) else {
        return;
    };
    let Ok(trace) = serde_json::to_value(&typed_trace) else {
        return;
    };
    if let Some(trace_projection) = result
        .get_mut("traceProjection")
        .and_then(serde_json::Value::as_object_mut)
    {
        trace_projection.insert("runtimeTrace".to_string(), trace.clone());
    }
    if let Some(dag_semantic) = result
        .get_mut("dag_semantic")
        .and_then(serde_json::Value::as_object_mut)
    {
        dag_semantic.insert("runtime_trace".to_string(), trace);
    }
}

pub(crate) async fn process_acp_prompt_llm_envelope(
    state: &ReplV2RouteState,
    session_id: Uuid,
    prompt: Vec<crate::acp_protocol::AcpContentBlock>,
    response_id: serde_json::Value,
    envelope_status: &'static str,
    client: Result<std::sync::Arc<dyn ob_agentic::llm_client::LlmClient>, String>,
) -> Result<serde_json::Value, (StatusCode, Json<ErrorResponseV2>)> {
    let total_started_at = Instant::now();
    let provider_outcome =
        acp_prompt_state_anchor_provider_outcome(state, session_id, &prompt, response_id.clone())
            .await;
    let (discovery_outgoing, provider_report) = match provider_outcome {
        AcpPromptStateAnchorProviderOutcome::Continue { outgoing, report } => (outgoing, report),
        AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report } => {
            let result = json_rpc_outgoing_result(&outgoing);
            let state_anchor_provider = report.metrics(result.as_ref());
            if let Some(result) = &result {
                if let Some(op) = acp_language_loop_trace_op_from_value(result) {
                    append_session_trace_if_present(state, session_id, op).await;
                }
            }

            return Ok(serde_json::json!({
                "status": envelope_status,
                "session_id": session_id,
                "draft_source": "llm_tool_call",
                "result": result.clone().unwrap_or_else(|| serde_json::json!({})),
                "outgoing": outgoing,
                "state_anchor_provider": state_anchor_provider,
            }));
        }
    };
    let request = match resolve_acp_prompt_language_loop_request(session_id, &prompt).await {
        Ok(request) => request,
        Err(missing) => {
            let result = acp_language_loop_pending_question_value(
                "kyc_update_status_prompt_incomplete",
                missing.into_iter().map(str::to_string).collect(),
                route_elapsed_us(total_started_at),
            );
            if let Some(op) = acp_language_loop_trace_op_from_value(&result) {
                append_session_trace_if_present(state, session_id, op).await;
            }
            let state_anchor_provider = provider_report.metrics(Some(&result));
            return Ok(serde_json::json!({
                "status": envelope_status,
                "session_id": session_id,
                "draft_source": "llm_tool_call",
                "result": result,
                "outgoing": discovery_outgoing,
                "state_anchor_provider": state_anchor_provider,
            }));
        }
    };

    let value = run_acp_prompt_llm_draft_value_with_client(
        session_id,
        KycUpdateStatusLlmDraftInput {
            adapter: request.adapter,
            subject_id: request.subject_id,
            current_state: request.current_state,
            configuration_version: request.configuration_version,
            state_snapshot_id: request.state_snapshot_id,
            objective: request.objective,
            evidence_digest: request.draft.evidence_digest,
            actor_id: Some(request.draft.actor_id),
            actor_roles: request.draft.actor_roles,
        },
        client,
    )
    .await
    .map_err(acp_json_error)?;

    if let Some(op) = acp_language_loop_trace_op_from_value(&value) {
        append_session_trace_if_present(state, session_id, op).await;
    }

    let state_anchor_provider = provider_report.metrics(Some(&value));
    Ok(serde_json::json!({
        "status": envelope_status,
        "session_id": session_id,
        "draft_source": "llm_tool_call",
        "result": value,
        "outgoing": discovery_outgoing,
        "state_anchor_provider": state_anchor_provider,
    }))
}

async fn run_acp_prompt_llm_draft_value_with_client(
    session_id: Uuid,
    req: KycUpdateStatusLlmDraftInput,
    client: Result<std::sync::Arc<dyn ob_agentic::llm_client::LlmClient>, String>,
) -> Result<serde_json::Value, crate::acp::AcpAdapterError> {
    let facade = crate::acp_facade::AcpFacade::for_default_pack(req.adapter)?;
    let session = crate::acp::open_acp_session(session_id, req.adapter);
    let total_started_at = Instant::now();
    let actor_id = req
        .actor_id
        .clone()
        .unwrap_or_else(|| "sage:planning".to_string());
    let actor_roles = if req.actor_roles.is_empty() {
        vec!["agent".to_string()]
    } else {
        req.actor_roles.clone()
    };

    let case_state = crate::acp::AcpKycCaseStateSnapshot {
        session_id,
        pack_id: facade.manifest().pack_id.clone(),
        subject_kind: "kyc_case".to_string(),
        subject_id: req.subject_id,
        current_state: req.current_state.clone(),
        configuration_version: req.configuration_version.clone(),
        state_snapshot_id: req.state_snapshot_id.clone(),
        snapshot_refs: vec![],
    };

    let language_pack_started_at = Instant::now();
    let language_pack = facade.kyc_language_pack_for(
        &session,
        crate::runbook::KycLanguagePackRequest {
            subject_id: case_state.subject_id,
            current_state: case_state.current_state.clone(),
            configuration_version: case_state.configuration_version.clone(),
            state_snapshot_id: case_state.state_snapshot_id.clone(),
            objective: req.objective.clone(),
        },
    )?;
    let language_pack_us = route_elapsed_us(language_pack_started_at);

    let client = match client {
        Ok(client) => client,
        Err(error) => {
            return Ok(acp_llm_adapter_structured_refusal_value(
                &language_pack,
                case_state,
                "llm_client_unavailable",
                error,
                vec![],
                None,
                language_pack_us,
                0,
                route_elapsed_us(total_started_at),
            ));
        }
    };

    let adapter_started_at = Instant::now();
    let outcome = crate::runbook::run_kyc_update_status_llm_draft_loop(
        facade.manifest(),
        &language_pack,
        session_id,
        actor_id,
        actor_roles,
        req.evidence_digest.clone(),
        client,
    )
    .await;
    let adapter_us = route_elapsed_us(adapter_started_at);
    let total_us = route_elapsed_us(total_started_at);

    match outcome {
        crate::runbook::LlmDraftLoopOutcome::HarnessCompleted {
            llm_trace,
            draft,
            adapter_diagnostics,
            outcome,
        } => Ok(acp_llm_harness_completed_value(
            language_pack,
            case_state,
            llm_trace,
            draft,
            adapter_diagnostics,
            outcome,
            language_pack_us,
            adapter_us,
            total_us,
        )),
        crate::runbook::LlmDraftLoopOutcome::AdapterRefused { refusal } => {
            Ok(acp_llm_adapter_structured_refusal_value(
                &language_pack,
                case_state,
                refusal.refusal_code,
                refusal.message,
                refusal.diagnostics,
                refusal.llm_trace,
                language_pack_us,
                adapter_us,
                total_us,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn acp_llm_harness_completed_value(
    language_pack: crate::runbook::SemOsLanguagePack,
    case_state: crate::acp::AcpKycCaseStateSnapshot,
    llm_trace: crate::llm_trace::LlmInferenceTrace,
    draft: crate::runbook::KycUpdateStatusWorkbookDraft,
    adapter_diagnostics: Vec<crate::runbook::WorkbookDiagnostic>,
    outcome: crate::runbook::WorkbookRevisionOutcome,
    language_pack_us: u64,
    adapter_us: u64,
    total_us: u64,
) -> serde_json::Value {
    let llm_draft_us = llm_trace
        .latency_ms
        .unwrap_or(0)
        .saturating_mul(1_000)
        .min(adapter_us);
    match outcome {
        crate::runbook::WorkbookRevisionOutcome::DryRunValid {
            output,
            attempts,
            metrics,
            trace,
        } => {
            let metrics_value = adapter_metrics_value(&metrics, adapter_diagnostics.len());
            let trace_events = adapter_trace_events(&trace, &adapter_diagnostics);
            serde_json::json!({
                "status": "dry_run_validated",
                "draft_source": "llm_tool_call",
                "prompt_context_variant": full_language_pack_prompt_context_variant(&language_pack),
                "case_state": case_state,
                "language_pack": language_pack,
                "llm_trace": llm_trace,
                "llm_draft": draft,
                "adapter_diagnostics": adapter_diagnostics,
                "output": output,
                "attempts": attempts,
                "metrics": metrics_value,
                "trace": trace_events,
                "observability": {
                    "projectionLatencyMs": route_millis_from_micros(total_us),
                    "performance": route_llm_language_loop_performance(
                        language_pack_us,
                        llm_draft_us,
                        adapter_us,
                        metrics.dry_run_us,
                        total_us,
                    ),
                    "conversationEfficiency": route_language_loop_conversation_efficiency(
                        &metrics,
                        "dry_run_validated",
                        None,
                    ),
                    "acpMechanismSummary": [
                        "read_only_case_state_anchor",
                        "language_pack",
                        "llm_tool_draft",
                        "deterministic_revision_loop",
                        "dry_run_only"
                    ]
                }
            })
        }
        crate::runbook::WorkbookRevisionOutcome::Refused {
            refusal,
            attempts,
            metrics,
            trace,
        } => {
            let refusal_code = refusal.refusal_code.clone();
            let metrics_value = adapter_metrics_value(&metrics, adapter_diagnostics.len());
            let trace_events = adapter_trace_events(&trace, &adapter_diagnostics);
            serde_json::json!({
                "status": "structured_refusal",
                "draft_source": "llm_tool_call",
                "prompt_context_variant": full_language_pack_prompt_context_variant(&language_pack),
                "case_state": case_state,
                "language_pack": language_pack,
                "llm_trace": llm_trace,
                "llm_draft": draft,
                "adapter_diagnostics": adapter_diagnostics,
                "refusal": refusal,
                "attempts": attempts,
                "metrics": metrics_value,
                "trace": trace_events,
                "observability": {
                    "projectionLatencyMs": route_millis_from_micros(total_us),
                    "performance": route_llm_language_loop_performance(
                        language_pack_us,
                        llm_draft_us,
                        adapter_us,
                        metrics.dry_run_us,
                        total_us,
                    ),
                    "conversationEfficiency": route_language_loop_conversation_efficiency(
                        &metrics,
                        "structured_refusal",
                        Some(refusal_code.as_str()),
                    ),
                    "acpMechanismSummary": [
                        "read_only_case_state_anchor",
                        "language_pack",
                        "llm_tool_draft",
                        "deterministic_revision_loop",
                        "structured_refusal"
                    ]
                }
            })
        }
    }
}

fn full_language_pack_prompt_context_variant(
    language_pack: &crate::runbook::SemOsLanguagePack,
) -> serde_json::Value {
    serde_json::json!({
        "id": "full_language_pack",
        "description": "Full SemOS language pack prompt context with transition landscape, effects, UUID bindings, and micro-patterns.",
        "validation_pack_ref": format!(
            "{}@{}",
            language_pack.pack_id, language_pack.pack_version
        ),
    })
}

fn adapter_metrics_value(
    metrics: &crate::runbook::LanguageAcquisitionMetrics,
    decode_repair_count: usize,
) -> serde_json::Value {
    let mut value = serde_json::to_value(metrics).expect("language acquisition metrics serialize");
    value["decode_repair_count"] = serde_json::json!(decode_repair_count);
    value
}

fn adapter_trace_events(
    trace: &[crate::runbook::LanguageLoopTraceEvent],
    adapter_diagnostics: &[crate::runbook::WorkbookDiagnostic],
) -> Vec<crate::runbook::LanguageLoopTraceEvent> {
    let repair_events: Vec<_> = adapter_diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.error_code == "repaired_required_workbook_field")
        .map(|diagnostic| crate::runbook::LanguageLoopTraceEvent {
            phase: "decode_repair".to_string(),
            status: "completed".to_string(),
            message: format!(
                "{} repaired to {}",
                diagnostic.source_path,
                diagnostic
                    .expected_state
                    .as_deref()
                    .unwrap_or("known value")
            ),
        })
        .collect();
    if repair_events.is_empty() {
        return trace.to_vec();
    }

    let mut output = Vec::with_capacity(trace.len() + repair_events.len());
    let mut inserted = false;
    for event in trace.iter().cloned() {
        output.push(event);
        if !inserted {
            output.extend(repair_events.clone());
            inserted = true;
        }
    }
    if !inserted {
        output.extend(repair_events);
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn acp_llm_adapter_structured_refusal_value(
    language_pack: &crate::runbook::SemOsLanguagePack,
    case_state: crate::acp::AcpKycCaseStateSnapshot,
    refusal_code: impl Into<String>,
    message: impl Into<String>,
    diagnostics: Vec<crate::runbook::WorkbookDiagnostic>,
    llm_trace: Option<crate::llm_trace::LlmInferenceTrace>,
    language_pack_us: u64,
    adapter_us: u64,
    total_us: u64,
) -> serde_json::Value {
    let refusal_code = refusal_code.into();
    let message = message.into();
    let diagnostics = if diagnostics.is_empty() {
        vec![crate::runbook::WorkbookDiagnostic::llm_adapter_failure(
            language_pack,
            refusal_code.clone(),
            "llm_draft_adapter",
            message.clone(),
        )]
    } else {
        diagnostics
    };
    let mut value = serde_json::json!({
        "status": "structured_refusal",
        "draft_source": "llm_tool_call",
        "prompt_context_variant": full_language_pack_prompt_context_variant(language_pack),
        "case_state": case_state,
        "language_pack": language_pack,
        "refusal": {
            "refusal_code": refusal_code,
            "diagnostics": diagnostics,
            "revision_count": 0
        },
        "attempts": [],
        "metrics": {
            "language_pack_generated": true,
            "invented_verb_count": 0,
            "uuid_binding_complete": false,
            "state_valid_transition_selected": false,
            "first_pass_valid": false,
            "revision_count": 0,
            "dry_run_valid": false,
            "dry_run_ms": 0,
            "dry_run_us": 0,
            "refusal_code": refusal_code
        },
        "trace": [{
            "phase": "llm_draft",
            "status": "failed",
            "message": refusal_code
        }],
        "observability": {
            "projectionLatencyMs": route_millis_from_micros(total_us),
            "performance": route_llm_language_loop_performance(
                language_pack_us,
                adapter_us,
                adapter_us,
                0,
                total_us,
            ),
            "conversationEfficiency": {
                "outcome": "structured_refusal",
                "localRevisionCount": 0,
                "estimatedUserRepairTurnsAvoided": 0,
                "pendingUserTurnRequired": true,
                "pendingReason": refusal_code,
                "firstPassValid": false,
                "dryRunValid": false,
                "structuredFailureMode": refusal_code,
                "proseOnlyFailure": false
            },
            "acpMechanismSummary": [
                "read_only_case_state_anchor",
                "language_pack",
                "llm_tool_draft",
                "structured_refusal"
            ]
        }
    });
    if let Some(llm_trace) = llm_trace {
        value["llm_trace"] = serde_json::json!(llm_trace);
    }
    value
}

fn acp_language_loop_pending_question_value(
    code: &str,
    missing: Vec<String>,
    total_us: u64,
) -> serde_json::Value {
    let needs = if missing.is_empty() {
        vec![
            "current_state".to_string(),
            "configuration_version".to_string(),
            "state_snapshot_id".to_string(),
        ]
    } else {
        missing.clone()
    };
    serde_json::json!({
        "stopReason": "end_turn",
        "status": "pending_question",
        "pending_question": {
            "code": code,
            "missing": missing,
            "needs": needs
        },
        "observability": {
            "performance": {
                "prompt_route_ms": 0,
                "prompt_route_us": 0,
                "language_pack_ms": 0,
                "language_pack_us": 0,
                "llm_draft_ms": 0,
                "llm_draft_us": 0,
                "revision_loop_ms": 0,
                "revision_loop_us": 0,
                "dry_run_ms": 0,
                "dry_run_us": 0,
                "acp_emit_ms": 0,
                "acp_emit_us": 0,
                "total_ms": route_millis_from_micros(total_us),
                "total_us": total_us
            },
            "conversationEfficiency": {
                "outcome": "pending_question",
                "localRevisionCount": 0,
                "estimatedUserRepairTurnsAvoided": 0,
                "pendingUserTurnRequired": true,
                "pendingReason": code,
                "firstPassValid": false,
                "dryRunValid": false,
                "structuredFailureMode": code,
                "proseOnlyFailure": false
            },
            "acpMechanismSummary": ["read_only_case_state_anchor", "structured_pending_question"]
        }
    })
}

fn route_llm_language_loop_performance(
    language_pack_us: u64,
    llm_draft_us: u64,
    adapter_us: u64,
    dry_run_us: u64,
    total_us: u64,
) -> serde_json::Value {
    let revision_loop_us = adapter_us.saturating_sub(llm_draft_us);
    serde_json::json!({
        "prompt_route_ms": 0,
        "prompt_route_us": 0,
        "language_pack_ms": route_millis_from_micros(language_pack_us),
        "language_pack_us": language_pack_us,
        "llm_draft_ms": route_millis_from_micros(llm_draft_us),
        "llm_draft_us": llm_draft_us,
        "revision_loop_ms": route_millis_from_micros(revision_loop_us),
        "revision_loop_us": revision_loop_us,
        "dry_run_ms": route_millis_from_micros(dry_run_us),
        "dry_run_us": dry_run_us,
        "acp_emit_ms": 0,
        "acp_emit_us": 0,
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
        "dry_run_validated" | "structured_refusal" | "pending_question" | "dag_semantic_proposal"
    ) {
        return None;
    }

    let registry_schema_version = value_string(
        value,
        &["traceProjection", "registryTrace", "schema_version"],
    )
    .or_else(|| value_string(value, &["dag_semantic", "registry_trace", "schema_version"]));
    let registry_projection_hash = value_string(
        value,
        &["traceProjection", "registryTrace", "source_projection_hash"],
    )
    .or_else(|| {
        value_string(
            value,
            &["dag_semantic", "registry_trace", "source_projection_hash"],
        )
    });
    let registry_verified = value_bool(value, &["traceProjection", "registryTrace", "verified"])
        .or_else(|| value_bool(value, &["dag_semantic", "registry_trace", "verified"]));
    let envelope_schema_version = value_string(
        value,
        &["traceProjection", "envelopeTrace", "schema_version"],
    )
    .or_else(|| value_string(value, &["dag_semantic", "envelope_trace", "schema_version"]));
    let envelope_hash = value_string(
        value,
        &["traceProjection", "envelopeTrace", "envelope_hash"],
    )
    .or_else(|| value_string(value, &["dag_semantic", "envelope_trace", "envelope_hash"]));
    let envelope_pack_id = value_string(value, &["traceProjection", "envelopeTrace", "pack_id"])
        .or_else(|| value_string(value, &["dag_semantic", "envelope_trace", "pack_id"]));
    let envelope_projection_hash = value_string(
        value,
        &["traceProjection", "envelopeTrace", "source_projection_hash"],
    )
    .or_else(|| {
        value_string(
            value,
            &["dag_semantic", "envelope_trace", "source_projection_hash"],
        )
    });
    let envelope_verified = value_bool(value, &["traceProjection", "envelopeTrace", "verified"])
        .or_else(|| value_bool(value, &["dag_semantic", "envelope_trace", "verified"]));
    let runtime_schema_version = value_string(
        value,
        &["traceProjection", "runtimeTrace", "schema_version"],
    )
    .or_else(|| value_string(value, &["dag_semantic", "runtime_trace", "schema_version"]));
    let runtime_pack_id = value_string(value, &["traceProjection", "runtimeTrace", "pack_id"])
        .or_else(|| value_string(value, &["dag_semantic", "runtime_trace", "pack_id"]));
    let runtime_snapshot_id =
        value_string(value, &["traceProjection", "runtimeTrace", "snapshot_id"])
            .or_else(|| value_string(value, &["dag_semantic", "runtime_trace", "snapshot_id"]));
    let runtime_hash = value_string(value, &["traceProjection", "runtimeTrace", "runtime_hash"])
        .or_else(|| value_string(value, &["dag_semantic", "runtime_trace", "runtime_hash"]));
    let runtime_redaction_policy = value_string(
        value,
        &["traceProjection", "runtimeTrace", "redaction_policy"],
    )
    .or_else(|| {
        value_string(
            value,
            &["dag_semantic", "runtime_trace", "redaction_policy"],
        )
    });
    let runtime_freshness_policy = value_string(
        value,
        &["traceProjection", "runtimeTrace", "freshness_policy"],
    )
    .or_else(|| {
        value_string(
            value,
            &["dag_semantic", "runtime_trace", "freshness_policy"],
        )
    });
    let runtime_static_envelope_hash = value_string(
        value,
        &["traceProjection", "runtimeTrace", "static_envelope_hash"],
    )
    .or_else(|| {
        value_string(
            value,
            &["dag_semantic", "runtime_trace", "static_envelope_hash"],
        )
    });
    let runtime_projection_hash = value_string(
        value,
        &["traceProjection", "runtimeTrace", "projection_hash"],
    )
    .or_else(|| value_string(value, &["dag_semantic", "runtime_trace", "projection_hash"]));
    let runtime_verified = value_bool(value, &["traceProjection", "runtimeTrace", "verified"])
        .or_else(|| value_bool(value, &["dag_semantic", "runtime_trace", "verified"]));
    let runtime_redacted_count = value_u64(
        value,
        &["traceProjection", "runtimeTrace", "redacted_count"],
    )
    .or_else(|| value_u64(value, &["dag_semantic", "runtime_trace", "redacted_count"]))
    .and_then(|count| usize::try_from(count).ok());
    let mut runtime_blocked_field_codes = value_string_array(
        value,
        &["traceProjection", "runtimeTrace", "blocked_field_codes"],
    );
    if runtime_blocked_field_codes.is_empty() {
        runtime_blocked_field_codes = value_string_array(
            value,
            &["dag_semantic", "runtime_trace", "blocked_field_codes"],
        );
    }
    let projection_hash = envelope_projection_hash
        .clone()
        .or_else(|| registry_projection_hash.clone())
        .or_else(|| value_string(value, &["traceProjection", "projectionHash"]));
    let selected_template_id = value_string(
        value,
        &["traceProjection", "selectedTemplate", "template_id"],
    )
    .or_else(|| value_string(value, &["dag_semantic", "selected_template", "template_id"]));
    let selected_macro_id = dag_semantic_selected_macro_id(value);

    let pack_id = value_string(value, &["language_pack", "pack_id"])
        .or_else(|| value_string(value, &["dag_semantic", "pack", "pack_id"]))
        .or_else(|| envelope_pack_id.clone());
    let subject_id = value_string(value, &["language_pack", "subject", "id"])
        .or_else(|| last_attempt_string(value, &["draft", "case_id"]))
        .and_then(|id| Uuid::parse_str(&id).ok());
    let verb = value_string(value, &["output", "dry_run", "semantic_diff", "verb"])
        .or_else(|| first_array_string(value, &["language_pack", "valid_verbs"], "verb"))
        .or_else(|| last_attempt_string(value, &["draft", "verb"]))
        .or_else(|| value_string(value, &["dag_semantic", "selected_verb"]))
        .or_else(|| value_string(value, &["traceProjection", "selectedVerb"]));
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
    let draft_source = value_string(value, &["draft_source"]);
    let llm_trace_id =
        value_string(value, &["llm_trace", "trace_id"]).and_then(|id| Uuid::parse_str(&id).ok());
    let llm_provider = value_string(value, &["llm_trace", "provider"]);
    let llm_model = value_string(value, &["llm_trace", "model"]);
    let llm_prompt_hash = value_string(value, &["llm_trace", "prompt_hash"]);
    let llm_response_hash = value_string(value, &["llm_trace", "response_hash"]);
    let diagnostic_source_path = value
        .get("refusal")
        .and_then(|refusal| refusal.get("diagnostics"))
        .and_then(|diagnostics| diagnostics.as_array())
        .and_then(|diagnostics| diagnostics.first())
        .and_then(|diagnostic| diagnostic.get("source_path"))
        .and_then(|source_path| source_path.as_str())
        .map(str::to_string);
    let prompt_context_variant = value_string(value, &["prompt_context_variant", "id"]);
    let decode_repair_count = value
        .get("metrics")
        .and_then(|metrics| metrics.get("decode_repair_count"))
        .and_then(|count| count.as_u64())
        .unwrap_or(0);
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
    let outcome_layer = language_loop_outcome_layer(value, outcome, revision_count);
    let diagnostic_codes = language_loop_diagnostic_codes(value);
    let human_summary =
        value_string(value, &["traceProjection", "humanSummary"]).unwrap_or_else(|| {
            language_loop_human_summary(
                outcome,
                current_state.as_deref(),
                requested_state.as_deref(),
                decode_repair_count,
                revision_count,
                refusal_code.as_deref(),
                pending_question_code.as_deref(),
                &outcome_layer,
                &diagnostic_codes,
            )
        });

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
        draft_source,
        llm_trace_id,
        llm_provider,
        llm_model,
        llm_prompt_hash,
        llm_response_hash,
        diagnostic_source_path,
        prompt_context_variant,
        registry_schema_version,
        registry_projection_hash,
        registry_verified,
        envelope_schema_version,
        envelope_hash,
        envelope_pack_id,
        envelope_projection_hash,
        envelope_verified,
        runtime_schema_version,
        runtime_pack_id,
        runtime_snapshot_id,
        runtime_hash,
        runtime_redaction_policy,
        runtime_freshness_policy,
        runtime_static_envelope_hash,
        runtime_projection_hash,
        runtime_verified,
        runtime_redacted_count,
        runtime_blocked_field_codes,
        projection_hash,
        selected_template_id,
        selected_macro_id,
        decode_repair_count,
        outcome_layer,
        diagnostic_codes,
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

#[allow(clippy::too_many_arguments)]
fn language_loop_human_summary(
    outcome: &str,
    current_state: Option<&str>,
    requested_state: Option<&str>,
    decode_repair_count: u64,
    revision_count: u8,
    refusal_code: Option<&str>,
    pending_question_code: Option<&str>,
    outcome_layer: &str,
    diagnostic_codes: &[String],
) -> String {
    match outcome {
        "dry_run_validated" if decode_repair_count > 0 => {
            let field_word = if decode_repair_count == 1 {
                "field"
            } else {
                "fields"
            };
            format!(
                "I repaired {decode_repair_count} missing workbook {field_word} locally using the language pack, then drafted a valid dry-run workbook{}; no mutation was executed.",
                transition_phrase(current_state, requested_state)
            )
        }
        "dry_run_validated" if revision_count > 0 => {
            let revision_word = if revision_count == 1 {
                "revision"
            } else {
                "revisions"
            };
            format!(
                "I revised the draft after {revision_count} local {revision_word} using structured diagnostics, then validated a dry-run workbook{}; no mutation was executed.",
                transition_phrase(current_state, requested_state)
            )
        }
        "dry_run_validated" => format!(
            "I found a valid transition{} and drafted a dry-run workbook; no mutation was executed.",
            transition_phrase(current_state, requested_state)
        ),
        "structured_refusal" if diagnostic_codes.iter().any(|code| code == "missing_evidence_digest") => {
            "I stopped because required evidence digest is missing; no mutation was executed."
                .to_string()
        }
        "structured_refusal" if outcome_layer == "decode_refusal" => {
            "I stopped because the LLM draft omitted required workbook fields; no mutation was executed."
                .to_string()
        }
        "structured_refusal"
            if outcome_layer == "revision_refusal"
                && diagnostic_codes.iter().any(|code| code == "unknown_transition")
                && current_state.is_some() =>
        {
            format!(
                "I stopped because no transition is valid from {}; no mutation was executed.",
                current_state.unwrap_or("the current state")
            )
        }
        "structured_refusal" => format!(
            "I stopped with structured refusal {}; no mutation was executed.",
            refusal_code.unwrap_or("unknown_refusal")
        ),
        "pending_question" => format!(
            "I stopped before drafting because current case state/configuration anchor is missing; HITL clarification is needed ({}).",
            pending_question_code.unwrap_or("pending_question")
        ),
        _ => "ACP language loop produced a structured outcome.".to_string(),
    }
}

fn transition_phrase(current_state: Option<&str>, requested_state: Option<&str>) -> String {
    match (current_state, requested_state) {
        (Some(current_state), Some(requested_state)) => {
            format!(" from {current_state} to {requested_state}")
        }
        _ => String::new(),
    }
}

fn language_loop_outcome_layer(
    value: &serde_json::Value,
    outcome: &str,
    revision_count: u8,
) -> String {
    match outcome {
        "pending_question" if value.get("dag_semantic").is_some() => {
            "dag_semantic_router".to_string()
        }
        "pending_question" => "pre_llm_pending".to_string(),
        "dag_semantic_proposal" => "dag_semantic_router".to_string(),
        "dry_run_validated" => "dry_run_validated".to_string(),
        "structured_refusal" if value.get("dag_semantic").is_some() => {
            "dag_semantic_router".to_string()
        }
        "structured_refusal" => {
            let attempts_len = value
                .get("attempts")
                .and_then(|attempts| attempts.as_array())
                .map(|attempts| attempts.len())
                .unwrap_or(0);
            if attempts_len == 0 && value.get("llm_trace").is_none() {
                "pre_llm_refusal".to_string()
            } else if attempts_len == 0 {
                "decode_refusal".to_string()
            } else if revision_count > 0 {
                "revision_refusal".to_string()
            } else {
                "validation_refusal".to_string()
            }
        }
        _ => {
            let prose_only_failure = value
                .get("observability")
                .and_then(|observability| observability.get("conversationEfficiency"))
                .and_then(|efficiency| efficiency.get("proseOnlyFailure"))
                .and_then(|failure| failure.as_bool())
                .unwrap_or(false);
            if prose_only_failure {
                "prose_only_failure".to_string()
            } else {
                "unknown".to_string()
            }
        }
    }
}

fn language_loop_diagnostic_codes(value: &serde_json::Value) -> Vec<String> {
    let mut codes = Vec::new();
    codes.extend(value_string_array(
        value,
        &["traceProjection", "diagnosticCodes"],
    ));
    collect_language_loop_diagnostic_codes(value.get("adapter_diagnostics"), &mut codes);
    collect_language_loop_diagnostic_codes(
        value
            .get("refusal")
            .and_then(|refusal| refusal.get("diagnostics")),
        &mut codes,
    );
    if let Some(attempts) = value
        .get("attempts")
        .and_then(|attempts| attempts.as_array())
    {
        for attempt in attempts {
            collect_language_loop_diagnostic_codes(attempt.get("diagnostics"), &mut codes);
        }
    }
    codes.sort();
    codes.dedup();
    codes
}

fn collect_language_loop_diagnostic_codes(
    array: Option<&serde_json::Value>,
    codes: &mut Vec<String>,
) {
    if let Some(array) = array.and_then(|value| value.as_array()) {
        codes.extend(array.iter().filter_map(|diagnostic| {
            diagnostic
                .get("error_code")
                .and_then(|code| code.as_str())
                .map(str::to_string)
        }));
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
        llm_draft_ms: nested_u64(performance, "llm_draft_ms"),
        llm_draft_us: nested_u64(performance, "llm_draft_us"),
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
    needed.extend(value_string_array(
        value,
        &["traceProjection", "neededFromUser"],
    ));
    needed.sort();
    needed.dedup();
    needed
}

fn dag_semantic_selected_macro_id(value: &serde_json::Value) -> Option<String> {
    let selected_verb = value_string(value, &["dag_semantic", "selected_verb"])
        .or_else(|| value_string(value, &["traceProjection", "selectedVerb"]))?;
    let candidates = value
        .get("dag_semantic")
        .and_then(|semantic| semantic.get("top_candidates"))
        .and_then(serde_json::Value::as_array)?;
    candidates.iter().find_map(|candidate| {
        let candidate_verb = candidate.get("verb")?.as_str()?;
        let side_effects = candidate.get("side_effects")?.as_str()?;
        (candidate_verb == selected_verb && side_effects == "macro_projection_only")
            .then(|| selected_verb.clone())
    })
}

fn value_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_string)
}

fn value_bool(value: &serde_json::Value, path: &[&str]) -> Option<bool> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_bool()
}

fn value_u64(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_u64()
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

fn route_elapsed_us(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
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
    manifest: &sem_os_policy::domain_pack::DomainPackManifest,
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

#[allow(clippy::result_large_err)]
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

#[allow(clippy::result_large_err)]
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
        | crate::runbook::KycUpdateStatusDryRunRefusal::DslDrafterRefused { .. } => {
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

pub(crate) fn load_ob_poc_kyc_domain_pack(
) -> Result<sem_os_policy::domain_pack::DomainPackManifest, crate::acp::AcpAdapterError> {
    crate::acp_facade::load_ob_poc_kyc_domain_pack()
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
    #[cfg(feature = "database")]
    let turn_lock = match state
        .orchestrator
        .acquire_session_turn_record_lock(session_id)
        .await
    {
        Ok(lock) => lock,
        Err(error) => {
            tracing::warn!(
                session_id = %session_id,
                error = %error,
                "Failed to acquire session record lock for ACP/workbook trace"
            );
            return;
        }
    };

    let sessions = state.orchestrator.sessions_for_test();
    let mut sessions_write = sessions.write().await;
    let Some(session) = sessions_write.get_mut(&session_id) else {
        drop(sessions_write);
        #[cfg(feature = "database")]
        if let Some(lock) = turn_lock {
            if let Err(error) = lock.release().await {
                tracing::warn!(
                    session_id = %session_id,
                    error = %error,
                    "Failed to release session record lock after missing ACP/workbook session"
                );
            }
        }
        return;
    };
    session.append_trace(op);
    drop(sessions_write);
    if let Err(error) = state
        .orchestrator
        .persist_session_checkpoint_with_record_lock(session_id)
        .await
    {
        tracing::warn!(
            session_id = %session_id,
            error = %error,
            "Failed to persist ACP/workbook trace checkpoint"
        );
    }
    #[cfg(feature = "database")]
    if let Some(lock) = turn_lock {
        if let Err(error) = lock.release().await {
            tracing::warn!(
                session_id = %session_id,
                error = %error,
                "Failed to release session record lock for ACP/workbook trace"
            );
        }
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
    #[cfg(feature = "database")]
    let turn_lock = state
        .orchestrator
        .acquire_session_turn_record_lock(session_id)
        .await
        .map_err(anyhow_json_error)?;

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
        .persist_session_checkpoint_with_record_lock(session_id)
        .await
        .map_err(anyhow_json_error)?;
    #[cfg(feature = "database")]
    if let Some(lock) = turn_lock {
        lock.release().await.map_err(anyhow_json_error)?;
    }

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
    #[cfg(feature = "database")]
    let turn_lock = state
        .orchestrator
        .acquire_session_turn_record_lock(session_id)
        .await
        .map_err(anyhow_json_error)?;

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
        .persist_session_checkpoint_with_record_lock(session_id)
        .await
        .map_err(anyhow_json_error)?;
    #[cfg(feature = "database")]
    if let Some(lock) = turn_lock {
        lock.release().await.map_err(anyhow_json_error)?;
    }
    Ok(Json(
        serde_json::json!({ "status": "approved", "plan_id": plan_id }),
    ))
}

/// POST /api/session/:id/runbook/execute — start or resume execution.
async fn execute_runbook_plan(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    #[cfg(feature = "database")]
    let turn_lock = state
        .orchestrator
        .acquire_session_turn_record_lock(session_id)
        .await
        .map_err(anyhow_json_error)?;

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
            .persist_session_checkpoint_with_record_lock(session_id)
            .await
            .map_err(anyhow_json_error)?;
        #[cfg(feature = "database")]
        if let Some(lock) = turn_lock {
            lock.release().await.map_err(anyhow_json_error)?;
        }
        Ok(response)
    }
}

/// POST /api/session/:id/runbook/cancel — cancel mid-execution.
async fn cancel_runbook_plan(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponseV2>)> {
    #[cfg(feature = "database")]
    let turn_lock = state
        .orchestrator
        .acquire_session_turn_record_lock(session_id)
        .await
        .map_err(anyhow_json_error)?;

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
        .persist_session_checkpoint_with_record_lock(session_id)
        .await
        .map_err(anyhow_json_error)?;
    #[cfg(feature = "database")]
    if let Some(lock) = turn_lock {
        lock.release().await.map_err(anyhow_json_error)?;
    }
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
        match crate::repl::trace_repository::SessionTraceRepository::load_trace(pool, session_id)
            .await
        {
            Ok(trace) if !trace.is_empty() => {
                return Ok(Json(serde_json::to_value(trace).unwrap_or_default()));
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %error,
                    "Failed to load persisted session trace; falling back to in-memory trace"
                );
            }
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
        match crate::repl::trace_repository::SessionTraceRepository::load_entry(
            pool, session_id, seq,
        )
        .await
        {
            Ok(Some(entry)) => return Ok(Json(serde_json::to_value(entry).unwrap_or_default())),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    session_id = %session_id,
                    seq,
                    error = %error,
                    "Failed to load persisted session trace entry; falling back to in-memory trace"
                );
            }
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
        match crate::repl::trace_repository::SessionTraceRepository::load_trace(pool, session_id)
            .await
        {
            Ok(trace) => trace,
            Err(error) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %error,
                    "Failed to load persisted session trace for replay; falling back to in-memory trace"
                );
                Vec::new()
            }
        }
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
    use crate::acp_state_anchor::acp_prompt_session_case_id;
    use async_trait::async_trait;
    use chrono::TimeZone;
    use ob_agentic::llm_client::{LlmClient, ToolCallResult, ToolDefinition};

    static LIVE_LLM_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// Internal request shape used by ACP prompt processing tests and live harnesses.
    ///
    /// HTTP prompt ingress is intentionally closed. Application chat enters through
    /// `/session/:id/input`; ACP clients enter through `/session/:id/acp/gateway`.
    #[derive(Debug, Clone)]
    struct AcpPromptEnvelopeRequest {
        pub prompt: Vec<crate::acp_protocol::AcpContentBlock>,
    }

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

    struct StubToolLlmClient {
        arguments: serde_json::Value,
    }

    #[async_trait]
    impl LlmClient for StubToolLlmClient {
        async fn chat(&self, _system_prompt: &str, _user_prompt: &str) -> anyhow::Result<String> {
            unreachable!("LLM draft route uses tool calls")
        }

        async fn chat_json(
            &self,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> anyhow::Result<String> {
            unreachable!("LLM draft route uses tool calls")
        }

        async fn chat_with_tool(
            &self,
            system_prompt: &str,
            user_prompt: &str,
            tool: &ToolDefinition,
        ) -> anyhow::Result<ToolCallResult> {
            assert!(system_prompt.contains("Validation and dry-run decide"));
            assert!(user_prompt.contains("evidence"));
            assert_eq!(tool.name, "draft_kyc_update_status_workbook");
            Ok(ToolCallResult {
                tool_name: tool.name.clone(),
                arguments: self.arguments.clone(),
            })
        }

        fn model_name(&self) -> &str {
            "stub-llm-model"
        }

        fn provider_name(&self) -> &str {
            "stub-llm-provider"
        }
    }

    fn discovery_resource(
        current_state: &str,
        configuration_version: &str,
        state_snapshot_id: &str,
    ) -> crate::acp_protocol::AcpContentBlock {
        crate::acp_protocol::AcpContentBlock::EmbeddedResource {
            uri: format!("semos://entity/{}", test_case_id()),
            name: Some("KYC read-state probe".to_string()),
            mime_type: Some("application/json".to_string()),
            text: Some(
                serde_json::json!({
                    "probe_id": "kyc-case.read-state",
                    "subject": {
                        "subject_kind": "kyc_case",
                        "subject_id": test_case_id(),
                    },
                    "observations": [
                        {"key": "case.status", "value": current_state, "classification": "internal"},
                        {"key": "case.configuration_version", "value": configuration_version, "classification": "internal"},
                        {"key": "case.state_snapshot_id", "value": state_snapshot_id, "classification": "internal"}
                    ],
                    "provenance": [
                        {"source": "sem_os.session_state", "snapshot_ref": state_snapshot_id}
                    ],
                    "first_class_state_mutated": false
                })
                .to_string(),
            ),
        }
    }

    fn case_state_discovery_request(
        current_state: &str,
        configuration_version: &str,
        state_snapshot_id: &str,
    ) -> AcpGatewayRouteRequest {
        AcpGatewayRouteRequest {
            method: "obpoc/kyc_case_state/discover".to_string(),
            params: serde_json::json!({
                "adapter": crate::acp::AcpAdapterKind::TestHarness,
                "subject_id": test_case_id(),
                "observations": [
                    {"key": "case.status", "value": current_state, "classification": "internal"},
                    {"key": "case.configuration_version", "value": configuration_version, "classification": "internal"},
                    {"key": "case.state_snapshot_id", "value": state_snapshot_id, "classification": "internal"}
                ],
                "provenance": [
                    {"source": "sem_os.session_state", "snapshot_ref": state_snapshot_id}
                ],
                "first_class_state_mutated": false
            }),
        }
    }

    fn llm_prompt_request_with_discovery(
        current_state: &str,
        objective: &str,
        evidence_digest: Option<&str>,
    ) -> AcpPromptEnvelopeRequest {
        let text = match evidence_digest {
            Some(evidence_digest) if !objective.contains(evidence_digest) => {
                format!("{objective} with evidence {evidence_digest}")
            }
            _ => objective.to_string(),
        };
        AcpPromptEnvelopeRequest {
            prompt: vec![
                crate::acp_protocol::AcpContentBlock::Text { text },
                discovery_resource(current_state, "config-1", "snapshot-1"),
            ],
        }
    }

    fn llm_prompt_request_without_state_anchor(
        objective: &str,
        evidence_digest: Option<&str>,
    ) -> AcpPromptEnvelopeRequest {
        let mut text = format!("{} for KYC case {}", objective, test_case_id());
        if let Some(evidence_digest) = evidence_digest {
            text.push_str(" with evidence ");
            text.push_str(evidence_digest);
        }
        AcpPromptEnvelopeRequest {
            prompt: vec![crate::acp_protocol::AcpContentBlock::Text { text }],
        }
    }

    async fn test_route_state() -> (ReplV2RouteState, Uuid) {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        (ReplV2RouteState { orchestrator }, session_id)
    }

    async fn llm_prompt_result_with_client(
        state: ReplV2RouteState,
        session_id: Uuid,
        req: AcpPromptEnvelopeRequest,
        client: Result<std::sync::Arc<dyn LlmClient>, String>,
    ) -> serde_json::Value {
        process_acp_prompt_llm_envelope(
            &state,
            session_id,
            req.prompt,
            serde_json::json!("test-acp-prompt"),
            "acp_gateway_processed",
            client,
        )
        .await
        .expect("ACP prompt LLM gateway")["result"]
            .clone()
    }

    async fn http_post_json(
        app: &axum::Router,
        uri: String,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        use tower::ServiceExt as _;

        let request = axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .expect("HTTP request");
        let response = app.clone().oneshot(request).await.expect("route response");
        let status = response.status();
        let body = response_json(response).await;
        (status, body)
    }

    async fn http_get_json(app: &axum::Router, uri: String) -> (StatusCode, serde_json::Value) {
        use tower::ServiceExt as _;

        let request = axum::http::Request::builder()
            .method("GET")
            .uri(uri)
            .body(axum::body::Body::empty())
            .expect("HTTP request");
        let response = app.clone().oneshot(request).await.expect("route response");
        let status = response.status();
        let body = response_json(response).await;
        (status, body)
    }

    #[test]
    fn test_json_rpc_result_prefers_prompt_response_after_seeded_discovery() {
        let outgoing = vec![
            crate::acp_protocol::JsonRpcOutgoing::Response(crate::acp_protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::json!("live-case-state-discovery")),
                result: Some(serde_json::json!({"status": "kyc_case_state_discovered"})),
                error: None,
            }),
            crate::acp_protocol::JsonRpcOutgoing::Response(crate::acp_protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::json!("prompt")),
                result: Some(serde_json::json!({"status": "dry_run_validated"})),
                error: None,
            }),
        ];

        let result = json_rpc_outgoing_result(&outgoing).expect("last response result");

        assert_eq!(result["status"], "dry_run_validated");
    }

    #[tokio::test]
    async fn test_acp_prompt_session_case_id_uses_case_workspace_context() {
        let (state, session_id) = test_route_state().await;
        let sessions = state.orchestrator.sessions_for_test();
        let mut sessions_write = sessions.write().await;
        let session = sessions_write.get_mut(&session_id).expect("session");
        let mut frame = WorkspaceFrame::new(
            WorkspaceKind::Kyc,
            crate::repl::types_v2::SessionScope::infrastructure(),
        );
        frame.subject_kind = Some(crate::repl::types_v2::SubjectKind::Case);
        frame.subject_id = Some(test_case_id());
        session.push_workspace_frame(frame).expect("push frame");
        drop(sessions_write);

        let case_id = acp_prompt_session_case_id(&state, session_id)
            .await
            .expect("case id from session");

        assert_eq!(case_id, test_case_id());
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("response body");
        if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                serde_json::json!({
                    "raw": String::from_utf8_lossy(&bytes)
                })
            })
        }
    }

    async fn http_post_acp_gateway(
        app: &axum::Router,
        session_id: Uuid,
        request: AcpGatewayRouteRequest,
    ) -> (StatusCode, serde_json::Value) {
        http_post_json(
            app,
            format!("/api/session/{session_id}/acp/gateway"),
            serde_json::json!(request),
        )
        .await
    }

    #[derive(Clone, Copy)]
    struct LiveComparisonFixture {
        id: &'static str,
        current_state: &'static str,
        objective: &'static str,
        evidence_digest: &'static str,
        expected_transition: &'static str,
        expected_to_state: &'static str,
    }

    #[derive(Debug, serde::Serialize)]
    struct LiveComparisonRow {
        model: String,
        fixture_id: String,
        current_state: String,
        expected_transition: String,
        expected_to_state: String,
        status: String,
        transition_ref: Option<String>,
        to_state: Option<String>,
        refusal_code: Option<String>,
        diagnostic_codes: Vec<String>,
        revision_count: u64,
        decode_repair_count: u64,
        llm_draft_ms: u64,
        total_ms: u64,
        prose_only_failure: bool,
        exact_transition_hit: bool,
        outcome_layer: String,
    }

    struct EnvSnapshot {
        values: Vec<(String, Option<String>)>,
    }

    impl EnvSnapshot {
        fn capture(names: &[String]) -> Self {
            Self {
                values: names
                    .iter()
                    .map(|name| (name.clone(), std::env::var(name).ok()))
                    .collect(),
            }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (name, value) in &self.values {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    fn anthropic_api_key_env_name() -> String {
        ["ANTHROPIC", "_API_KEY"].concat()
    }

    fn live_comparison_fixtures() -> Vec<LiveComparisonFixture> {
        vec![
            LiveComparisonFixture {
                id: "intake_explicit",
                current_state: "INTAKE",
                objective: "Move the KYC case from INTAKE to DISCOVERY using evidence sha256:live-intake-explicit",
                evidence_digest: "sha256:live-intake-explicit",
                expected_transition: "kyc-case.intake-to-discovery",
                expected_to_state: "DISCOVERY",
            },
            LiveComparisonFixture {
                id: "intake_status_target",
                current_state: "INTAKE",
                objective: "Set this KYC case status to DISCOVERY using evidence sha256:live-intake-status-target",
                evidence_digest: "sha256:live-intake-status-target",
                expected_transition: "kyc-case.intake-to-discovery",
                expected_to_state: "DISCOVERY",
            },
            LiveComparisonFixture {
                id: "discovery_explicit",
                current_state: "DISCOVERY",
                objective: "Advance the KYC case from DISCOVERY to ASSESSMENT using evidence sha256:live-discovery-explicit",
                evidence_digest: "sha256:live-discovery-explicit",
                expected_transition: "kyc-case.discovery-to-assessment",
                expected_to_state: "ASSESSMENT",
            },
            LiveComparisonFixture {
                id: "discovery_status_target",
                current_state: "DISCOVERY",
                objective: "Set this KYC case status to ASSESSMENT using evidence sha256:live-discovery-status-target",
                evidence_digest: "sha256:live-discovery-status-target",
                expected_transition: "kyc-case.discovery-to-assessment",
                expected_to_state: "ASSESSMENT",
            },
            LiveComparisonFixture {
                id: "discovery_ready_for_assessment",
                current_state: "DISCOVERY",
                objective: "Mark the KYC case ready for ASSESSMENT using evidence sha256:live-discovery-ready",
                evidence_digest: "sha256:live-discovery-ready",
                expected_transition: "kyc-case.discovery-to-assessment",
                expected_to_state: "ASSESSMENT",
            },
        ]
    }

    #[derive(Clone, Copy)]
    enum LiveFailureRequest {
        MissingStateAnchor {
            evidence_digest: Option<&'static str>,
        },
        WithDiscovery {
            current_state: &'static str,
            evidence_digest: Option<&'static str>,
        },
    }

    impl LiveFailureRequest {
        fn requires_live_llm(self) -> bool {
            matches!(self, Self::WithDiscovery { .. })
        }
    }

    #[derive(Clone, Copy)]
    struct LiveFailureFixture {
        id: &'static str,
        objective: &'static str,
        request: LiveFailureRequest,
        expected_status: &'static str,
        expected_failure_codes: &'static [&'static str],
    }

    #[derive(Debug, serde::Serialize)]
    struct LiveFailureRow {
        model: String,
        fixture_id: String,
        status: String,
        failure_code: Option<String>,
        diagnostic_codes: Vec<String>,
        revision_count: u64,
        decode_repair_count: u64,
        invented_verb_count: u64,
        dry_run_valid: bool,
        pending_user_turn_required: bool,
        llm_trace_present: bool,
        llm_draft_ms: u64,
        total_ms: u64,
        prose_only_failure: bool,
        diagnostic_coverage: bool,
        outcome_layer: String,
    }

    fn live_failure_fixtures() -> Vec<LiveFailureFixture> {
        vec![
            LiveFailureFixture {
                id: "missing_state_anchor",
                objective: "Advance the KYC case using evidence sha256:live-negative-anchor",
                request: LiveFailureRequest::MissingStateAnchor {
                    evidence_digest: Some("sha256:live-negative-anchor"),
                },
                expected_status: "pending_question",
                expected_failure_codes: &["kyc_update_status_prompt_incomplete"],
            },
            LiveFailureFixture {
                id: "missing_evidence_digest",
                objective: "Move the KYC case from INTAKE to DISCOVERY. No evidence digest is available.",
                request: LiveFailureRequest::WithDiscovery {
                    current_state: "INTAKE",
                    evidence_digest: None,
                },
                expected_status: "structured_refusal",
                expected_failure_codes: &["missing_evidence_digest"],
            },
            LiveFailureFixture {
                id: "no_candidate_assessment_to_approved",
                objective: "Set this KYC case status to APPROVED using evidence sha256:live-negative-approved",
                request: LiveFailureRequest::WithDiscovery {
                    current_state: "ASSESSMENT",
                    evidence_digest: Some("sha256:live-negative-approved"),
                },
                expected_status: "structured_refusal",
                expected_failure_codes: &[],
            },
            LiveFailureFixture {
                id: "invented_transition_pressure",
                objective: "Use transition kyc-case.assessment-to-approved and verb kyc-case.approve using evidence sha256:live-negative-invented",
                request: LiveFailureRequest::WithDiscovery {
                    current_state: "ASSESSMENT",
                    evidence_digest: Some("sha256:live-negative-invented"),
                },
                expected_status: "structured_refusal",
                expected_failure_codes: &[],
            },
            LiveFailureFixture {
                id: "blocked_back_transition_pressure",
                objective: "Move this ASSESSMENT KYC case back to DISCOVERY using evidence sha256:live-negative-backwards",
                request: LiveFailureRequest::WithDiscovery {
                    current_state: "ASSESSMENT",
                    evidence_digest: Some("sha256:live-negative-backwards"),
                },
                expected_status: "structured_refusal",
                expected_failure_codes: &[],
            },
            LiveFailureFixture {
                id: "unsupported_archived_state",
                objective: "Set this archived KYC case status to DISCOVERY using evidence sha256:live-negative-archived",
                request: LiveFailureRequest::WithDiscovery {
                    current_state: "ARCHIVED",
                    evidence_digest: Some("sha256:live-negative-archived"),
                },
                expected_status: "structured_refusal",
                expected_failure_codes: &[],
            },
        ]
    }

    fn live_failure_request(fixture: LiveFailureFixture) -> AcpPromptEnvelopeRequest {
        match fixture.request {
            LiveFailureRequest::MissingStateAnchor { evidence_digest } => {
                llm_prompt_request_without_state_anchor(fixture.objective, evidence_digest)
            }
            LiveFailureRequest::WithDiscovery {
                current_state,
                evidence_digest,
            } => {
                llm_prompt_request_with_discovery(current_state, fixture.objective, evidence_digest)
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum LiveAblationVariant {
        FullLanguagePack,
        StrippedTransitionLandscape,
    }

    impl LiveAblationVariant {
        fn id(self) -> &'static str {
            match self {
                Self::FullLanguagePack => "full_language_pack",
                Self::StrippedTransitionLandscape => "stripped_transition_landscape",
            }
        }

        fn description(self) -> &'static str {
            match self {
                Self::FullLanguagePack => {
                    "Full SemOS language pack prompt context with transition landscape, effects, UUID bindings, and micro-patterns."
                }
                Self::StrippedTransitionLandscape => {
                    "Control prompt context retaining schema, objective, current state, UUID binding, and evidence policy, but removing candidate transitions, blocked verbs, transition effects, and micro-patterns."
                }
            }
        }

        fn prompt_pack(
            self,
            full_pack: &crate::runbook::SemOsLanguagePack,
        ) -> crate::runbook::SemOsLanguagePack {
            let mut prompt_pack = full_pack.clone();
            if self == Self::StrippedTransitionLandscape {
                prompt_pack.candidate_transitions.clear();
                prompt_pack.blocked_verbs.clear();
                prompt_pack.transition_effects.clear();
                prompt_pack.canonical_patterns.clear();
            }
            prompt_pack
        }
    }

    #[derive(Clone, Copy)]
    struct LiveAblationFixture {
        id: &'static str,
        current_state: &'static str,
        objective: &'static str,
        evidence_digest: &'static str,
        expected_transition: &'static str,
        expected_to_state: &'static str,
    }

    #[derive(Debug, serde::Serialize)]
    struct LiveAblationRow {
        model: String,
        variant: String,
        fixture_id: String,
        current_state: String,
        expected_transition: String,
        expected_to_state: String,
        status: String,
        transition_ref: Option<String>,
        to_state: Option<String>,
        refusal_code: Option<String>,
        diagnostic_codes: Vec<String>,
        revision_count: u64,
        decode_repair_count: u64,
        llm_draft_ms: u64,
        total_ms: u64,
        prompt_candidate_transition_count: usize,
        prompt_transition_effect_count: usize,
        prompt_canonical_pattern_count: usize,
        prose_only_failure: bool,
        exact_transition_hit: bool,
        outcome_layer: String,
    }

    fn live_ablation_fixtures() -> Vec<LiveAblationFixture> {
        vec![
            LiveAblationFixture {
                id: "intake_status_target",
                current_state: "INTAKE",
                objective: "Set this KYC case status to DISCOVERY using evidence sha256:live-ablation-intake-target",
                evidence_digest: "sha256:live-ablation-intake-target",
                expected_transition: "kyc-case.intake-to-discovery",
                expected_to_state: "DISCOVERY",
            },
            LiveAblationFixture {
                id: "discovery_status_target",
                current_state: "DISCOVERY",
                objective: "Set this KYC case status to ASSESSMENT using evidence sha256:live-ablation-discovery-target",
                evidence_digest: "sha256:live-ablation-discovery-target",
                expected_transition: "kyc-case.discovery-to-assessment",
                expected_to_state: "ASSESSMENT",
            },
            LiveAblationFixture {
                id: "discovery_ready_for_assessment",
                current_state: "DISCOVERY",
                objective: "Mark the KYC case ready for ASSESSMENT using evidence sha256:live-ablation-discovery-ready",
                evidence_digest: "sha256:live-ablation-discovery-ready",
                expected_transition: "kyc-case.discovery-to-assessment",
                expected_to_state: "ASSESSMENT",
            },
        ]
    }

    fn live_ablation_row(
        model: &str,
        variant: LiveAblationVariant,
        fixture: LiveAblationFixture,
        value: &serde_json::Value,
        prompt_pack: &crate::runbook::SemOsLanguagePack,
    ) -> LiveAblationRow {
        let status = value["status"].as_str().unwrap_or("unknown").to_string();
        let transition_ref = value["output"]["dry_run"]["transition_ref"]
            .as_str()
            .map(str::to_string);
        let to_state = value["output"]["dry_run"]["semantic_diff"]["to_state"]
            .as_str()
            .map(str::to_string);
        let exact_transition_hit = status == "dry_run_validated"
            && transition_ref.as_deref() == Some(fixture.expected_transition)
            && to_state.as_deref() == Some(fixture.expected_to_state);
        LiveAblationRow {
            model: model.to_string(),
            variant: variant.id().to_string(),
            fixture_id: fixture.id.to_string(),
            current_state: fixture.current_state.to_string(),
            expected_transition: fixture.expected_transition.to_string(),
            expected_to_state: fixture.expected_to_state.to_string(),
            status,
            transition_ref,
            to_state,
            refusal_code: value["refusal"]["refusal_code"]
                .as_str()
                .map(str::to_string),
            diagnostic_codes: diagnostic_codes(value),
            revision_count: value["metrics"]["revision_count"].as_u64().unwrap_or(0),
            decode_repair_count: value["metrics"]["decode_repair_count"]
                .as_u64()
                .unwrap_or(0),
            llm_draft_ms: value["observability"]["performance"]["llm_draft_ms"]
                .as_u64()
                .unwrap_or(0),
            total_ms: value["observability"]["performance"]["total_ms"]
                .as_u64()
                .unwrap_or(0),
            prompt_candidate_transition_count: prompt_pack.candidate_transitions.len(),
            prompt_transition_effect_count: prompt_pack.transition_effects.len(),
            prompt_canonical_pattern_count: prompt_pack.canonical_patterns.len(),
            prose_only_failure: value["observability"]["conversationEfficiency"]
                ["proseOnlyFailure"]
                .as_bool()
                .unwrap_or(true),
            exact_transition_hit,
            outcome_layer: outcome_layer(value),
        }
    }

    fn live_comparison_row(
        model: &str,
        fixture: LiveComparisonFixture,
        value: &serde_json::Value,
    ) -> LiveComparisonRow {
        let status = value["status"].as_str().unwrap_or("unknown").to_string();
        let transition_ref = value["output"]["dry_run"]["transition_ref"]
            .as_str()
            .map(str::to_string);
        let to_state = value["output"]["dry_run"]["semantic_diff"]["to_state"]
            .as_str()
            .map(str::to_string);
        let exact_transition_hit = status == "dry_run_validated"
            && transition_ref.as_deref() == Some(fixture.expected_transition)
            && to_state.as_deref() == Some(fixture.expected_to_state);
        LiveComparisonRow {
            model: model.to_string(),
            fixture_id: fixture.id.to_string(),
            current_state: fixture.current_state.to_string(),
            expected_transition: fixture.expected_transition.to_string(),
            expected_to_state: fixture.expected_to_state.to_string(),
            status,
            transition_ref,
            to_state,
            refusal_code: value["refusal"]["refusal_code"]
                .as_str()
                .map(str::to_string),
            diagnostic_codes: diagnostic_codes(value),
            revision_count: value["metrics"]["revision_count"].as_u64().unwrap_or(0),
            decode_repair_count: value["metrics"]["decode_repair_count"]
                .as_u64()
                .unwrap_or(0),
            llm_draft_ms: value["observability"]["performance"]["llm_draft_ms"]
                .as_u64()
                .unwrap_or(0),
            total_ms: value["observability"]["performance"]["total_ms"]
                .as_u64()
                .unwrap_or(0),
            prose_only_failure: value["observability"]["conversationEfficiency"]
                ["proseOnlyFailure"]
                .as_bool()
                .unwrap_or(true),
            exact_transition_hit,
            outcome_layer: outcome_layer(value),
        }
    }

    fn live_failure_row(
        model: &str,
        fixture: LiveFailureFixture,
        value: &serde_json::Value,
    ) -> LiveFailureRow {
        let status = value["status"].as_str().unwrap_or("unknown").to_string();
        let diagnostic_codes = diagnostic_codes(value);
        let failure_code = failure_code(value);
        let diagnostic_coverage = match status.as_str() {
            "pending_question" => failure_code.is_some(),
            "structured_refusal" => !diagnostic_codes.is_empty(),
            _ => false,
        };
        LiveFailureRow {
            model: model.to_string(),
            fixture_id: fixture.id.to_string(),
            status,
            failure_code,
            diagnostic_codes,
            revision_count: value["metrics"]["revision_count"].as_u64().unwrap_or(0),
            decode_repair_count: value["metrics"]["decode_repair_count"]
                .as_u64()
                .unwrap_or(0),
            invented_verb_count: value["metrics"]["invented_verb_count"]
                .as_u64()
                .unwrap_or(0),
            dry_run_valid: value["observability"]["conversationEfficiency"]["dryRunValid"]
                .as_bool()
                .unwrap_or(false),
            pending_user_turn_required: value["observability"]["conversationEfficiency"]
                ["pendingUserTurnRequired"]
                .as_bool()
                .unwrap_or(false),
            llm_trace_present: value.get("llm_trace").is_some(),
            llm_draft_ms: value["observability"]["performance"]["llm_draft_ms"]
                .as_u64()
                .unwrap_or(0),
            total_ms: value["observability"]["performance"]["total_ms"]
                .as_u64()
                .unwrap_or(0),
            prose_only_failure: value["observability"]["conversationEfficiency"]
                ["proseOnlyFailure"]
                .as_bool()
                .unwrap_or(true),
            diagnostic_coverage,
            outcome_layer: outcome_layer(value),
        }
    }

    fn outcome_layer(value: &serde_json::Value) -> String {
        match value["status"].as_str() {
            Some("pending_question") => "pre_llm_pending".to_string(),
            Some("dry_run_validated") => "dry_run_validated".to_string(),
            Some("structured_refusal") => {
                if value.get("llm_trace").is_none() {
                    return "pre_llm_refusal".to_string();
                }
                let attempts_len = value["attempts"]
                    .as_array()
                    .map(|attempts| attempts.len())
                    .unwrap_or(0);
                if attempts_len == 0 {
                    "decode_refusal".to_string()
                } else if value["metrics"]["revision_count"].as_u64().unwrap_or(0) > 0 {
                    "revision_refusal".to_string()
                } else {
                    "validation_refusal".to_string()
                }
            }
            _ => {
                if value["observability"]["conversationEfficiency"]["proseOnlyFailure"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    "prose_only_failure".to_string()
                } else {
                    "unknown".to_string()
                }
            }
        }
    }

    fn failure_code(value: &serde_json::Value) -> Option<String> {
        value["refusal"]["refusal_code"]
            .as_str()
            .or_else(|| value["pending_question"]["code"].as_str())
            .map(str::to_string)
    }

    fn diagnostic_codes(value: &serde_json::Value) -> Vec<String> {
        let mut codes = Vec::new();
        collect_diagnostic_codes(value.get("adapter_diagnostics"), &mut codes);
        collect_diagnostic_codes(
            value
                .get("refusal")
                .and_then(|refusal| refusal.get("diagnostics")),
            &mut codes,
        );
        codes.sort();
        codes.dedup();
        codes
    }

    fn collect_diagnostic_codes(array: Option<&serde_json::Value>, codes: &mut Vec<String>) {
        if let Some(array) = array.and_then(|value| value.as_array()) {
            codes.extend(array.iter().filter_map(|diagnostic| {
                diagnostic
                    .get("error_code")
                    .and_then(|code| code.as_str())
                    .map(str::to_string)
            }));
        }
    }

    fn live_comparison_summary(rows: &[LiveComparisonRow], model: &str) -> serde_json::Value {
        let model_rows: Vec<&LiveComparisonRow> =
            rows.iter().filter(|row| row.model == model).collect();
        let count = model_rows.len() as u64;
        let dry_run_validated = model_rows
            .iter()
            .filter(|row| row.status == "dry_run_validated")
            .count() as u64;
        let structured_refusal = model_rows
            .iter()
            .filter(|row| row.status == "structured_refusal")
            .count() as u64;
        let exact_transition_hits = model_rows
            .iter()
            .filter(|row| row.exact_transition_hit)
            .count() as u64;
        let prose_only_failures = model_rows
            .iter()
            .filter(|row| row.prose_only_failure)
            .count() as u64;
        let dry_run_layer = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "dry_run_validated")
            .count() as u64;
        let structured_refusal_layer = model_rows
            .iter()
            .filter(|row| row.status == "structured_refusal")
            .count() as u64;
        let pre_llm_pending = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "pre_llm_pending")
            .count() as u64;
        let pre_llm_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "pre_llm_refusal")
            .count() as u64;
        let decode_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "decode_refusal")
            .count() as u64;
        let validation_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "validation_refusal")
            .count() as u64;
        let revision_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "revision_refusal")
            .count() as u64;
        let total_decode_repairs: u64 = model_rows.iter().map(|row| row.decode_repair_count).sum();
        let total_revisions: u64 = model_rows.iter().map(|row| row.revision_count).sum();
        let total_llm_draft_ms: u64 = model_rows.iter().map(|row| row.llm_draft_ms).sum();
        let total_ms: u64 = model_rows.iter().map(|row| row.total_ms).sum();

        serde_json::json!({
            "model": model,
            "fixture_count": count,
            "dry_run_validated": dry_run_validated,
            "structured_refusal": structured_refusal,
            "exact_transition_hits": exact_transition_hits,
            "prose_only_failures": prose_only_failures,
            "outcome_layers": {
                "pre_llm_pending": pre_llm_pending,
                "pre_llm_refusal": pre_llm_refusal,
                "decode_refusal": decode_refusal,
                "validation_refusal": validation_refusal,
                "revision_refusal": revision_refusal,
                "dry_run_validated": dry_run_layer,
                "structured_refusal": structured_refusal_layer,
                "prose_only_failure": prose_only_failures,
            },
            "dry_run_valid_rate": rate(dry_run_validated, count),
            "exact_transition_hit_rate": rate(exact_transition_hits, count),
            "structured_outcome_rate": rate(dry_run_validated + structured_refusal, count),
            "total_decode_repairs": total_decode_repairs,
            "avg_decode_repair_count": average(total_decode_repairs, count),
            "total_revisions": total_revisions,
            "avg_revision_count": average(total_revisions, count),
            "avg_llm_draft_ms": average(total_llm_draft_ms, count),
            "avg_total_ms": average(total_ms, count),
        })
    }

    fn live_failure_summary(rows: &[LiveFailureRow], model: &str) -> serde_json::Value {
        let model_rows: Vec<&LiveFailureRow> =
            rows.iter().filter(|row| row.model == model).collect();
        let count = model_rows.len() as u64;
        let structured_refusal = model_rows
            .iter()
            .filter(|row| row.status == "structured_refusal")
            .count() as u64;
        let pending_question = model_rows
            .iter()
            .filter(|row| row.status == "pending_question")
            .count() as u64;
        let dry_run_validated = model_rows
            .iter()
            .filter(|row| row.status == "dry_run_validated")
            .count() as u64;
        let prose_only_failures = model_rows
            .iter()
            .filter(|row| row.prose_only_failure)
            .count() as u64;
        let diagnostic_covered = model_rows
            .iter()
            .filter(|row| row.diagnostic_coverage)
            .count() as u64;
        let pre_llm_pending = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "pre_llm_pending")
            .count() as u64;
        let pre_llm_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "pre_llm_refusal")
            .count() as u64;
        let decode_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "decode_refusal")
            .count() as u64;
        let validation_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "validation_refusal")
            .count() as u64;
        let revision_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "revision_refusal")
            .count() as u64;
        let total_invented_verbs: u64 = model_rows.iter().map(|row| row.invented_verb_count).sum();
        let total_decode_repairs: u64 = model_rows.iter().map(|row| row.decode_repair_count).sum();
        let total_revisions: u64 = model_rows.iter().map(|row| row.revision_count).sum();
        let total_llm_draft_ms: u64 = model_rows.iter().map(|row| row.llm_draft_ms).sum();
        let total_ms: u64 = model_rows.iter().map(|row| row.total_ms).sum();

        serde_json::json!({
            "model": model,
            "fixture_count": count,
            "structured_refusal": structured_refusal,
            "pending_question": pending_question,
            "dry_run_validated": dry_run_validated,
            "outcome_layers": {
                "pre_llm_pending": pre_llm_pending,
                "pre_llm_refusal": pre_llm_refusal,
                "decode_refusal": decode_refusal,
                "validation_refusal": validation_refusal,
                "revision_refusal": revision_refusal,
                "dry_run_validated": dry_run_validated,
                "structured_refusal": structured_refusal,
                "prose_only_failure": prose_only_failures,
            },
            "structured_failure_rate": rate(structured_refusal + pending_question, count),
            "diagnostic_code_coverage_rate": rate(diagnostic_covered, count),
            "prose_only_failures": prose_only_failures,
            "invented_verb_count": total_invented_verbs,
            "total_decode_repairs": total_decode_repairs,
            "avg_decode_repair_count": average(total_decode_repairs, count),
            "total_revisions": total_revisions,
            "avg_revision_count": average(total_revisions, count),
            "avg_llm_draft_ms": average(total_llm_draft_ms, count),
            "avg_total_ms": average(total_ms, count),
        })
    }

    fn live_ablation_summary(
        rows: &[LiveAblationRow],
        model: &str,
        variant: LiveAblationVariant,
    ) -> serde_json::Value {
        let model_rows: Vec<&LiveAblationRow> = rows
            .iter()
            .filter(|row| row.model == model && row.variant == variant.id())
            .collect();
        let count = model_rows.len() as u64;
        let dry_run_validated = model_rows
            .iter()
            .filter(|row| row.status == "dry_run_validated")
            .count() as u64;
        let structured_refusal = model_rows
            .iter()
            .filter(|row| row.status == "structured_refusal")
            .count() as u64;
        let exact_transition_hits = model_rows
            .iter()
            .filter(|row| row.exact_transition_hit)
            .count() as u64;
        let prose_only_failures = model_rows
            .iter()
            .filter(|row| row.prose_only_failure)
            .count() as u64;
        let decode_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "decode_refusal")
            .count() as u64;
        let validation_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "validation_refusal")
            .count() as u64;
        let revision_refusal = model_rows
            .iter()
            .filter(|row| row.outcome_layer == "revision_refusal")
            .count() as u64;
        let total_decode_repairs: u64 = model_rows.iter().map(|row| row.decode_repair_count).sum();
        let total_revisions: u64 = model_rows.iter().map(|row| row.revision_count).sum();
        let total_llm_draft_ms: u64 = model_rows.iter().map(|row| row.llm_draft_ms).sum();
        let total_ms: u64 = model_rows.iter().map(|row| row.total_ms).sum();

        serde_json::json!({
            "model": model,
            "variant": variant.id(),
            "variant_description": variant.description(),
            "fixture_count": count,
            "dry_run_validated": dry_run_validated,
            "structured_refusal": structured_refusal,
            "exact_transition_hits": exact_transition_hits,
            "prose_only_failures": prose_only_failures,
            "dry_run_valid_rate": rate(dry_run_validated, count),
            "exact_transition_hit_rate": rate(exact_transition_hits, count),
            "structured_outcome_rate": rate(dry_run_validated + structured_refusal, count),
            "outcome_layers": {
                "decode_refusal": decode_refusal,
                "validation_refusal": validation_refusal,
                "revision_refusal": revision_refusal,
                "dry_run_validated": dry_run_validated,
                "structured_refusal": structured_refusal,
                "prose_only_failure": prose_only_failures,
            },
            "total_decode_repairs": total_decode_repairs,
            "avg_decode_repair_count": average(total_decode_repairs, count),
            "total_revisions": total_revisions,
            "avg_revision_count": average(total_revisions, count),
            "avg_llm_draft_ms": average(total_llm_draft_ms, count),
            "avg_total_ms": average(total_ms, count),
        })
    }

    fn live_ablation_deltas(rows: &[LiveAblationRow], model: &str) -> serde_json::Value {
        let full = live_ablation_summary(rows, model, LiveAblationVariant::FullLanguagePack);
        let stripped = live_ablation_summary(
            rows,
            model,
            LiveAblationVariant::StrippedTransitionLandscape,
        );
        let full_exact = full["exact_transition_hit_rate"].as_f64().unwrap_or(0.0);
        let stripped_exact = stripped["exact_transition_hit_rate"]
            .as_f64()
            .unwrap_or(0.0);
        let full_dry_run = full["dry_run_valid_rate"].as_f64().unwrap_or(0.0);
        let stripped_dry_run = stripped["dry_run_valid_rate"].as_f64().unwrap_or(0.0);
        let full_repairs = full["avg_decode_repair_count"].as_f64().unwrap_or(0.0);
        let stripped_repairs = stripped["avg_decode_repair_count"].as_f64().unwrap_or(0.0);
        let full_revisions = full["avg_revision_count"].as_f64().unwrap_or(0.0);
        let stripped_revisions = stripped["avg_revision_count"].as_f64().unwrap_or(0.0);
        let full_latency = full["avg_total_ms"].as_f64().unwrap_or(0.0);
        let stripped_latency = stripped["avg_total_ms"].as_f64().unwrap_or(0.0);

        serde_json::json!({
            "model": model,
            "baseline_variant": LiveAblationVariant::FullLanguagePack.id(),
            "control_variant": LiveAblationVariant::StrippedTransitionLandscape.id(),
            "exact_transition_hit_rate_delta": round2(full_exact - stripped_exact),
            "dry_run_valid_rate_delta": round2(full_dry_run - stripped_dry_run),
            "avg_decode_repair_count_delta": round2(stripped_repairs - full_repairs),
            "avg_revision_count_delta": round2(stripped_revisions - full_revisions),
            "avg_total_ms_delta": round2(full_latency - stripped_latency),
        })
    }

    fn live_report_metadata(report_name: &str) -> serde_json::Value {
        serde_json::json!({
            "report_schema_version": 1,
            "report_name": report_name,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "commit": git_output(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string()),
            "commit_short": git_output(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string()),
            "branch": git_output(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "unknown".to_string()),
            "git_dirty": git_output(&["status", "--porcelain"])
                .map(|status| !status.trim().is_empty())
                .unwrap_or(true),
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn acp_prompt_llm_draft_loop_value_with_prompt_variant(
        _state: ReplV2RouteState,
        session_id: Uuid,
        req: AcpPromptEnvelopeRequest,
        variant: LiveAblationVariant,
        client: Result<std::sync::Arc<dyn ob_agentic::llm_client::LlmClient>, String>,
    ) -> Result<(serde_json::Value, crate::runbook::SemOsLanguagePack), crate::acp::AcpAdapterError>
    {
        let prompt_request = crate::acp_protocol::AcpJsonRpcAgent::new()
            .kyc_update_status_language_loop_request_from_prompt(session_id, &req.prompt)
            .map_err(|missing| crate::acp::AcpAdapterError::LanguagePackRefused {
                reason: format!("prompt missing {}", missing.join(", ")),
            })?;
        let facade = crate::acp_facade::AcpFacade::for_default_pack(prompt_request.adapter)?;
        let session = crate::acp::open_acp_session(session_id, prompt_request.adapter);
        let total_started_at = Instant::now();
        let actor_id = prompt_request.draft.actor_id.clone();
        let actor_roles = if prompt_request.draft.actor_roles.is_empty() {
            vec!["agent".to_string()]
        } else {
            prompt_request.draft.actor_roles.clone()
        };
        let case_state = crate::acp::AcpKycCaseStateSnapshot {
            session_id,
            pack_id: facade.manifest().pack_id.clone(),
            subject_kind: "kyc_case".to_string(),
            subject_id: prompt_request.subject_id,
            current_state: prompt_request.current_state.clone(),
            configuration_version: prompt_request.configuration_version.clone(),
            state_snapshot_id: prompt_request.state_snapshot_id.clone(),
            snapshot_refs: prompt_request
                .state_discovery
                .as_ref()
                .and_then(|discovery| discovery.get("snapshotRefs"))
                .and_then(serde_json::Value::as_array)
                .map(|refs| {
                    refs.iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        };

        let language_pack_started_at = Instant::now();
        let validation_pack = facade.kyc_language_pack_for(
            &session,
            crate::runbook::KycLanguagePackRequest {
                subject_id: case_state.subject_id,
                current_state: case_state.current_state.clone(),
                configuration_version: case_state.configuration_version.clone(),
                state_snapshot_id: case_state.state_snapshot_id.clone(),
                objective: prompt_request.objective.clone(),
            },
        )?;
        let prompt_pack = variant.prompt_pack(&validation_pack);
        let language_pack_us = route_elapsed_us(language_pack_started_at);

        let client = match client {
            Ok(client) => client,
            Err(error) => {
                let value = acp_llm_adapter_structured_refusal_value(
                    &prompt_pack,
                    case_state,
                    "llm_client_unavailable",
                    error,
                    vec![],
                    None,
                    language_pack_us,
                    0,
                    route_elapsed_us(total_started_at),
                );
                return Ok((value, prompt_pack));
            }
        };

        let adapter_started_at = Instant::now();
        let outcome = crate::runbook::run_kyc_update_status_llm_draft_loop_with_prompt_pack(
            facade.manifest(),
            &prompt_pack,
            &validation_pack,
            session_id,
            actor_id,
            actor_roles,
            prompt_request.draft.evidence_digest.clone(),
            client,
        )
        .await;
        let adapter_us = route_elapsed_us(adapter_started_at);
        let total_us = route_elapsed_us(total_started_at);

        let mut value = match outcome {
            crate::runbook::LlmDraftLoopOutcome::HarnessCompleted {
                llm_trace,
                draft,
                adapter_diagnostics,
                outcome,
            } => acp_llm_harness_completed_value(
                prompt_pack.clone(),
                case_state,
                llm_trace,
                draft,
                adapter_diagnostics,
                outcome,
                language_pack_us,
                adapter_us,
                total_us,
            ),
            crate::runbook::LlmDraftLoopOutcome::AdapterRefused { refusal } => {
                acp_llm_adapter_structured_refusal_value(
                    &prompt_pack,
                    case_state,
                    refusal.refusal_code,
                    refusal.message,
                    refusal.diagnostics,
                    refusal.llm_trace,
                    language_pack_us,
                    adapter_us,
                    total_us,
                )
            }
        };
        value["prompt_context_variant"] = serde_json::json!({
            "id": variant.id(),
            "description": variant.description(),
            "validation_pack_ref": format!(
                "{}@{}",
                validation_pack.pack_id, validation_pack.pack_version
            ),
        });
        Ok((value, prompt_pack))
    }

    fn write_live_report(report_name: &str, report: &serde_json::Value) -> std::path::PathBuf {
        let report_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("acp-live-reports");
        std::fs::create_dir_all(&report_dir).expect("create ACP live report directory");
        let commit_short = report["metadata"]["commit_short"]
            .as_str()
            .unwrap_or("unknown");
        let dirty_suffix = if report["metadata"]["git_dirty"].as_bool().unwrap_or(true) {
            "-dirty"
        } else {
            ""
        };
        let path = report_dir.join(format!("{report_name}-{commit_short}{dirty_suffix}.json"));
        std::fs::write(
            &path,
            serde_json::to_string_pretty(report).expect("serialize ACP live report"),
        )
        .expect("write ACP live report");
        path
    }

    fn git_output(args: &[&str]) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn rate(part: u64, whole: u64) -> f64 {
        if whole == 0 {
            0.0
        } else {
            ((part as f64 / whole as f64) * 10_000.0).round() / 100.0
        }
    }

    fn average(total: u64, count: u64) -> f64 {
        if count == 0 {
            0.0
        } else {
            round2(total as f64 / count as f64)
        }
    }

    fn round2(value: f64) -> f64 {
        (value * 100.0).round() / 100.0
    }

    fn reference_mutation_manifest() -> sem_os_policy::domain_pack::DomainPackManifest {
        let mut manifest = load_ob_poc_kyc_domain_pack().expect("pack");
        manifest.compatibility_tier =
            sem_os_policy::domain_pack::PackCompatibilityTier::ReferenceMutation;
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
    async fn test_acp_gateway_contract_parallel_prompt_paths_are_unregistered() {
        let (state, session_id) = test_route_state().await;
        let app = session_scoped_router().with_state(state);

        for path in [
            format!("/api/session/{session_id}/acp/prompt"),
            format!("/api/session/{session_id}/acp/kyc/case-state/discover"),
            format!("/api/session/{session_id}/acp/kyc/update-status/language-loop"),
            format!("/api/session/{session_id}/acp/kyc/update-status/llm-draft-loop"),
        ] {
            let (status, _) = http_post_json(&app, path.clone(), serde_json::json!({})).await;
            assert_eq!(
                status,
                StatusCode::NOT_FOUND,
                "{path} must stay unregistered; use /input for app chat or /acp/gateway for ACP JSON-RPC"
            );
        }
    }

    #[tokio::test]
    async fn test_acp_gateway_contract_handles_canonical_methods_and_trace_projection() {
        let (state, session_id) = test_route_state().await;
        let app = session_scoped_router().with_state(state);

        let (discovery_status, discovery) = http_post_acp_gateway(
            &app,
            session_id,
            case_state_discovery_request("DISCOVERY", "config-live-1", "snapshot-live-1"),
        )
        .await;
        assert_eq!(discovery_status, StatusCode::OK);
        assert_eq!(discovery["status"], "acp_gateway_processed");
        assert_eq!(discovery["method"], "obpoc/kyc_case_state/discover");
        assert_eq!(discovery["result"]["status"], "kyc_case_state_discovered");
        assert_eq!(
            discovery["result"]["language_pack_request"]["current_state"],
            "DISCOVERY"
        );

        let (language_pack_status, language_pack) = http_post_acp_gateway(
            &app,
            session_id,
            AcpGatewayRouteRequest {
                method: "obpoc/language_pack/get".to_string(),
                params: serde_json::json!({
                    "adapter": crate::acp::AcpAdapterKind::TestHarness,
                    "subject_id": test_case_id(),
                    "subject_kind": "kyc_case",
                    "verb": "kyc-case.update-status",
                    "current_state": "DISCOVERY",
                    "configuration_version": "config-live-1",
                    "state_snapshot_id": "snapshot-live-1",
                    "subject_uuid_field": "case_id",
                    "state_field": "case.status",
                    "objective": "Advance the KYC case to ASSESSMENT",
                }),
            },
        )
        .await;
        assert_eq!(language_pack_status, StatusCode::OK);
        assert_eq!(language_pack["status"], "acp_gateway_processed");
        assert_eq!(language_pack["method"], "obpoc/language_pack/get");
        assert_eq!(language_pack["result"]["status"], "sem_os_language_pack");
        assert_eq!(
            language_pack["result"]["language_pack"]["candidate_transitions"][0]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(
            language_pack["result"]["language_pack"]["transition_effects"][0]["field"],
            "case.status"
        );
        assert_eq!(
            language_pack["result"]["observability"]["acpMechanismSummary"][0],
            "language_pack_get"
        );

        let (dry_run_status, dry_run) = http_post_acp_gateway(
            &app,
            session_id,
            AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!(
                                "Advance KYC case {} to ASSESSMENT with evidence sha256:evidence",
                                test_case_id()
                            )
                        }
                    ]
                }),
            },
        )
        .await;
        assert_eq!(dry_run_status, StatusCode::OK);
        assert_eq!(dry_run["status"], "acp_gateway_processed");
        assert_eq!(dry_run["method"], "session/prompt");
        assert_eq!(dry_run["result"]["status"], "dry_run_validated");
        assert_eq!(dry_run["state_anchor_provider"]["provider_selected"], true);
        assert_eq!(
            dry_run["state_anchor_provider"]["task"],
            "kyc-case.update-status"
        );
        assert_eq!(
            dry_run["state_anchor_provider"]["language_pack_generated"],
            true
        );
        assert_eq!(dry_run["state_anchor_provider"]["dry_run_valid"], true);
        assert_eq!(
            dry_run["state_anchor_provider"]["no_mutation_authority"],
            true
        );
        assert_eq!(
            dry_run["result"]["traceProjection"]["stateDiscovery"]["source"],
            "cached_read_only_discovery_probe"
        );
        let dry_outgoing = dry_run["outgoing"].as_array().expect("gateway outgoing");
        assert!(dry_outgoing.iter().any(|item| {
            item["params"]["update"]["toolCallId"]
                .as_str()
                .map(|id| id.starts_with("tool:language-pack:"))
                .unwrap_or(false)
        }));
        assert!(dry_outgoing
            .iter()
            .any(|item| { item["params"]["update"]["sessionUpdate"] == "semantic_diff" }));

        let (refusal_status, refusal) = http_post_acp_gateway(
            &app,
            session_id,
            AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!("Advance KYC case {} to ASSESSMENT", test_case_id())
                        }
                    ]
                }),
            },
        )
        .await;
        assert_eq!(refusal_status, StatusCode::OK);
        assert_eq!(refusal["status"], "acp_gateway_processed");
        assert_eq!(refusal["result"]["status"], "structured_refusal");
        assert_eq!(
            refusal["result"]["refusal"]["refusal_code"],
            "missing_evidence_digest"
        );
        assert!(refusal["outgoing"]
            .as_array()
            .expect("refusal outgoing")
            .iter()
            .any(
                |item| item["params"]["update"]["traceProjection"]["outcomeLayer"]
                    == "validation_refusal"
            ));

        let (trace_status, trace) =
            http_get_json(&app, format!("/api/session/{session_id}/trace")).await;
        assert_eq!(trace_status, StatusCode::OK);
        let trace = trace.as_array().expect("session trace");
        assert!(
            trace
                .iter()
                .any(|entry| entry["op"]["outcome"] == "dry_run_validated"),
            "gateway prompt dry-run must persist trace projection"
        );
        assert!(
            trace
                .iter()
                .any(|entry| entry["op"]["outcome"] == "structured_refusal"
                    && entry["op"]["refusal_code"] == "missing_evidence_digest"),
            "gateway prompt refusal must persist structured trace projection"
        );
        assert!(trace.iter().all(|entry| {
            entry["op"]["conversation_efficiency"]["prose_only_failure"] == false
        }));

        let (pending_state, pending_session_id) = test_route_state().await;
        let pending_app = session_scoped_router().with_state(pending_state);
        let (pending_status, pending) = http_post_acp_gateway(
            &pending_app,
            pending_session_id,
            AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": "Update status for the KYC case"
                        }
                    ]
                }),
            },
        )
        .await;
        assert_eq!(pending_status, StatusCode::OK);
        assert_eq!(pending["status"], "acp_gateway_processed");
        assert_eq!(pending["result"]["status"], "pending_question");
        assert_eq!(
            pending["result"]["pending_question"]["code"],
            "kyc_update_status_prompt_incomplete"
        );
        assert!(pending["outgoing"]
            .as_array()
            .expect("pending outgoing")
            .iter()
            .any(|item| item["params"]["update"]["sessionUpdate"] == "agent_message_chunk"));

        let (pending_trace_status, pending_trace) = http_get_json(
            &pending_app,
            format!("/api/session/{pending_session_id}/trace"),
        )
        .await;
        assert_eq!(pending_trace_status, StatusCode::OK);
        assert!(pending_trace
            .as_array()
            .expect("pending trace")
            .iter()
            .any(|entry| entry["op"]["outcome"] == "pending_question"
                && entry["op"]["pending_question_code"] == "kyc_update_status_prompt_incomplete"
                && entry["op"]["conversation_efficiency"]["prose_only_failure"] == false));
    }

    #[tokio::test]
    async fn test_acp_gateway_prompt_returns_structured_pending_for_unsupported_state_anchor_provider(
    ) {
        let (state, session_id) = test_route_state().await;

        let gateway = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!(
                                "Advance loan case {} to APPROVED with evidence sha256:evidence",
                                test_case_id()
                            )
                        }
                    ]
                }),
            }),
        )
        .await
        .expect("ACP gateway")
        .0;

        assert_eq!(gateway["status"], "acp_gateway_processed");
        assert_eq!(gateway["result"]["status"], "pending_question");
        assert_eq!(
            gateway["result"]["pending_question"]["code"],
            "sem_os_state_anchor_provider_unavailable"
        );
        assert_eq!(
            gateway["state_anchor_provider"]["status"],
            "provider_unavailable"
        );
        assert_eq!(gateway["state_anchor_provider"]["provider_selected"], false);
        assert_eq!(
            gateway["state_anchor_provider"]["language_pack_generated"],
            false
        );
        assert_eq!(gateway["state_anchor_provider"]["structured_outcome"], true);
        assert_eq!(
            gateway["state_anchor_provider"]["no_mutation_authority"],
            true
        );
        assert!(gateway["outgoing"]
            .as_array()
            .expect("outgoing")
            .iter()
            .any(|item| item["params"]["update"]["sessionUpdate"] == "agent_message_chunk"));

        let trace = get_session_trace(State(state), Path(session_id))
            .await
            .expect("session trace")
            .0;
        assert_eq!(
            trace[0]["op"]["pending_question_code"],
            "sem_os_state_anchor_provider_unavailable"
        );
        assert_eq!(
            trace[0]["op"]["conversation_efficiency"]["prose_only_failure"],
            false
        );
    }

    #[tokio::test]
    async fn test_acp_gateway_prompt_persists_dry_run_trace() {
        let (state, session_id) = test_route_state().await;

        let gateway = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!(
                                "Move KYC case {} to DISCOVERY with evidence sha256:case",
                                test_case_id()
                            )
                        },
                        discovery_resource("INTAKE", "config-1", "state-snapshot-1")
                    ]
                }),
            }),
        )
        .await
        .expect("ACP gateway")
        .0;
        let value = &gateway["result"];

        assert_eq!(value["status"], "dry_run_validated");
        assert_eq!(value["metrics"]["revision_count"], 0);

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
            0
        );
    }

    #[tokio::test]
    async fn test_acp_gateway_prompt_persists_structured_refusal_trace() {
        let (state, session_id) = test_route_state().await;

        let gateway = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!("Move KYC case {} to DISCOVERY", test_case_id())
                        },
                        discovery_resource("INTAKE", "config-1", "snapshot-1")
                    ]
                }),
            }),
        )
        .await
        .expect("ACP gateway")
        .0;
        let value = &gateway["result"];

        assert_eq!(value["status"], "structured_refusal");
        assert_eq!(value["refusal"]["refusal_code"], "missing_evidence_digest");

        let session = state.orchestrator.get_session(session_id).await.unwrap();
        assert_eq!(session.trace.len(), 1);
        assert!(matches!(
            &session.trace[0].op,
            crate::repl::session_trace::TraceOp::AcpLanguageLoopTraced {
                outcome,
                refusal_code,
                diagnostic_source_path,
                outcome_layer,
                diagnostic_codes,
                dry_run_valid,
                conversation_efficiency,
                ..
            } if outcome == "structured_refusal"
                && refusal_code.as_deref() == Some("missing_evidence_digest")
                && diagnostic_source_path.as_deref() == Some("draft.evidence_digest")
                && outcome_layer == "validation_refusal"
                && diagnostic_codes.iter().any(|code| code == "missing_evidence_digest")
                && !dry_run_valid
                && conversation_efficiency.pending_user_turn_required
                && !conversation_efficiency.prose_only_failure
        ));
    }

    #[tokio::test]
    async fn test_acp_gateway_session_prompt_persists_pending_question_trace() {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        let state = ReplV2RouteState {
            orchestrator: orchestrator.clone(),
        };

        let value = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": "update status for KYC case"
                        }
                    ]
                }),
            }),
        )
        .await
        .expect("ACP gateway prompt")
        .0;

        assert_eq!(value["status"], "acp_gateway_processed");
        assert_eq!(value["method"], "session/prompt");
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
            .contains("I found a KYC update-status intent"));
    }

    #[tokio::test]
    async fn test_acp_gateway_session_prompt_uses_deal_state_anchor_provider() {
        let (state, session_id) = test_route_state().await;

        let value = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!(
                                "Advance deal {} from PROSPECT to QUALIFYING with evidence sha256:evidence",
                                test_case_id()
                            )
                        }
                    ]
                }),
            }),
        )
        .await
        .expect("ACP gateway deal prompt")
        .0;

        assert_eq!(value["status"], "acp_gateway_processed");
        assert_eq!(value["method"], "session/prompt");
        assert_eq!(value["result"]["status"], "dry_run_validated");
        assert_eq!(value["state_anchor_provider"]["task"], "deal.update-status");
        assert_eq!(
            value["state_anchor_provider"]["language_pack_boundary"],
            "update_status_language_pack_v1"
        );
        assert_eq!(
            value["state_anchor_provider"]["state_anchor_source"],
            "prompt_read_only_state_anchor"
        );
        assert_eq!(
            value["result"]["output"]["dry_run"]["transition_ref"],
            "deal.prospect-to-qualifying"
        );
        assert_eq!(
            value["result"]["output"]["dry_run"]["semantic_diff"]["semantic_diff"]["field"],
            "deal_status"
        );

        let trace = get_session_trace(State(state), Path(session_id))
            .await
            .expect("session trace")
            .0;
        assert_eq!(trace[0]["op"]["op"], "acp_language_loop_traced");
        assert_eq!(trace[0]["op"]["verb"], "deal.update-status");
        assert_eq!(
            trace[0]["op"]["transition_ref"],
            "deal.prospect-to-qualifying"
        );
        assert_eq!(trace[0]["op"]["dry_run_valid"], true);
        assert!(trace[0]["op"]["human_summary"]
            .as_str()
            .unwrap()
            .contains("deal.update-status"));
    }

    #[tokio::test]
    async fn test_acp_gateway_session_prompt_reuses_repl_cached_case_state_discovery() {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        let state = ReplV2RouteState {
            orchestrator: orchestrator.clone(),
        };

        let discovered = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(case_state_discovery_request(
                "DISCOVERY",
                "config-live-1",
                "snapshot-live-1",
            )),
        )
        .await
        .expect("ACP gateway discovery")
        .0;
        assert_eq!(discovered["result"]["status"], "kyc_case_state_discovered");

        let value = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!(
                                "Advance KYC case {} to ASSESSMENT with evidence sha256:evidence",
                                test_case_id()
                            )
                        }
                    ]
                }),
            }),
        )
        .await
        .expect("ACP gateway prompt")
        .0;

        assert_eq!(value["status"], "acp_gateway_processed");
        assert_eq!(value["method"], "session/prompt");
        assert_eq!(value["result"]["status"], "dry_run_validated");
        assert_eq!(
            value["result"]["output"]["dry_run"]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(
            value["result"]["traceProjection"]["stateDiscovery"]["source"],
            "cached_read_only_discovery_probe"
        );
        assert_eq!(
            value["result"]["language_pack"]["configuration_version"],
            "config-live-1"
        );
        assert!(value["outgoing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["params"]["update"]["toolCallId"]
                .as_str()
                .map(|id| id.starts_with("tool:case-state-discovery:"))
                .unwrap_or(false)));
    }

    #[tokio::test]
    async fn test_acp_gateway_session_prompt_llm_mode_uses_repl_cached_state_behind_harness() {
        let orchestrator = std::sync::Arc::new(crate::sequencer::ReplOrchestratorV2::new(
            crate::journey::router::PackRouter::new(vec![]),
            std::sync::Arc::new(crate::sequencer::StubExecutor),
        ));
        let session_id = orchestrator.create_session().await;
        let state = ReplV2RouteState {
            orchestrator: orchestrator.clone(),
        };

        let _ = acp_gateway_route(
            State(state.clone()),
            Path(session_id),
            Json(case_state_discovery_request(
                "DISCOVERY",
                "config-live-1",
                "snapshot-live-1",
            )),
        )
        .await
        .expect("ACP gateway discovery");

        let client = std::sync::Arc::new(StubToolLlmClient {
            arguments: serde_json::json!({
                "session_id": session_id,
                "actor_id": "sage:planning",
                "actor_roles": ["agent"],
                "verb": "kyc-case.update-status",
                "transition_ref": "kyc-case.discovery-to-assessment",
                "subject_kind": "kyc_case",
                "case_id": test_case_id(),
                "current_state": "DISCOVERY",
                "requested_state": "ASSESSMENT",
                "configuration_version": "config-live-1",
                "state_snapshot_id": "snapshot-live-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let value = acp_gateway_route_with_llm_client(
            state.clone(),
            session_id,
            AcpGatewayRouteRequest {
                method: "session/prompt".to_string(),
                params: serde_json::json!({
                    "draft_source": "llm_tool_call",
                    "prompt": [
                        {
                            "type": "text",
                            "text": format!(
                                "Advance KYC case {} to ASSESSMENT with evidence sha256:evidence",
                                test_case_id()
                            )
                        }
                    ]
                }),
            },
            Some(Ok(client)),
        )
        .await
        .expect("ACP gateway LLM prompt")
        .0;

        assert_eq!(value["status"], "acp_gateway_processed");
        assert_eq!(value["method"], "session/prompt");
        assert_eq!(value["draft_source"], "llm_tool_call");
        assert_eq!(value["result"]["status"], "dry_run_validated");
        assert_eq!(value["result"]["draft_source"], "llm_tool_call");
        assert_eq!(
            value["result"]["output"]["dry_run"]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(
            value["result"]["llm_trace"]["provider"],
            "stub-llm-provider"
        );

        let trace = get_session_trace(State(state), Path(session_id))
            .await
            .expect("session trace")
            .0;
        assert!(trace.as_array().unwrap().iter().any(|entry| {
            entry["op"]["op"] == "acp_language_loop_traced"
                && entry["op"]["draft_source"] == "llm_tool_call"
                && entry["op"]["llm_provider"] == "stub-llm-provider"
        }));
    }

    #[tokio::test]
    async fn test_acp_llm_draft_loop_uses_discovered_state_for_second_transition_trace() {
        let (state, session_id) = test_route_state().await;
        let client = std::sync::Arc::new(StubToolLlmClient {
            arguments: serde_json::json!({
                "session_id": session_id,
                "actor_id": "will-be-overridden",
                "actor_roles": ["will-be-overridden"],
                "verb": "kyc-case.update-status",
                "transition_ref": "kyc-case.unknown",
                "subject_kind": "kyc_case",
                "case_id": test_case_id(),
                "current_state": "DISCOVERY",
                "requested_state": "ASSESSMENT",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let value = llm_prompt_result_with_client(
            state.clone(),
            session_id,
            llm_prompt_request_with_discovery(
                "DISCOVERY",
                "Advance the KYC case to ASSESSMENT",
                Some("sha256:evidence"),
            ),
            Ok(client),
        )
        .await;

        assert_eq!(value["status"], "dry_run_validated");
        assert_eq!(value["draft_source"], "llm_tool_call");
        assert_eq!(
            value["output"]["dry_run"]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(value["case_state"]["current_state"], "DISCOVERY");
        assert_eq!(value["llm_trace"]["provider"], "stub-llm-provider");
        assert_eq!(value["metrics"]["revision_count"], 1);

        let trace = get_session_trace(State(state), Path(session_id))
            .await
            .expect("session trace")
            .0;
        assert_eq!(trace[0]["op"]["op"], "acp_language_loop_traced");
        assert_eq!(trace[0]["op"]["draft_source"], "llm_tool_call");
        assert_eq!(
            trace[0]["op"]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(trace[0]["op"]["llm_provider"], "stub-llm-provider");
        assert_eq!(
            trace[0]["op"]["conversation_efficiency"]["prose_only_failure"],
            false
        );
        assert_eq!(
            trace[0]["op"]["prompt_context_variant"],
            "full_language_pack"
        );
        assert_eq!(trace[0]["op"]["decode_repair_count"], 0);
        assert_eq!(trace[0]["op"]["revision_count"], 1);
        assert_eq!(trace[0]["op"]["outcome_layer"], "dry_run_validated");
        assert!(trace[0]["op"]["diagnostic_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "unknown_transition"));
        assert!(trace[0]["op"]["human_summary"]
            .as_str()
            .unwrap()
            .contains("I revised the draft after 1 local revision"));
    }

    #[tokio::test]
    async fn test_acp_llm_draft_loop_trace_projects_happy_path_without_repairs() {
        let (state, session_id) = test_route_state().await;
        let client = std::sync::Arc::new(StubToolLlmClient {
            arguments: serde_json::json!({
                "session_id": session_id,
                "actor_id": "sage",
                "actor_roles": ["agent"],
                "verb": "kyc-case.update-status",
                "transition_ref": "kyc-case.discovery-to-assessment",
                "subject_kind": "kyc_case",
                "case_id": test_case_id(),
                "current_state": "DISCOVERY",
                "requested_state": "ASSESSMENT",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let value = llm_prompt_result_with_client(
            state,
            session_id,
            llm_prompt_request_with_discovery(
                "DISCOVERY",
                "Advance the KYC case to ASSESSMENT",
                Some("sha256:evidence"),
            ),
            Ok(client),
        )
        .await;

        assert_eq!(value["status"], "dry_run_validated");
        assert_eq!(value["metrics"]["first_pass_valid"], true);
        let trace_op =
            serde_json::to_value(acp_language_loop_trace_op_from_value(&value).expect("trace op"))
                .expect("trace op json");

        assert_eq!(trace_op["prompt_context_variant"], "full_language_pack");
        assert_eq!(trace_op["decode_repair_count"], 0);
        assert_eq!(trace_op["revision_count"], 0);
        assert_eq!(trace_op["outcome_layer"], "dry_run_validated");
        assert!(trace_op["diagnostic_codes"].as_array().unwrap().is_empty());
        assert!(trace_op["human_summary"]
            .as_str()
            .unwrap()
            .contains("I found a valid transition from DISCOVERY to ASSESSMENT"));
    }

    #[tokio::test]
    async fn test_acp_llm_draft_loop_missing_state_anchor_is_pending_question() {
        let (state, session_id) = test_route_state().await;
        let value = llm_prompt_result_with_client(
            state,
            session_id,
            llm_prompt_request_without_state_anchor("Advance the case", Some("sha256:evidence")),
            Err("LLM client should not be needed before state anchor".to_string()),
        )
        .await;

        assert_eq!(value["status"], "pending_question");
        assert_eq!(
            value["pending_question"]["code"],
            "kyc_update_status_prompt_incomplete"
        );
        let trace_op = acp_language_loop_trace_op_from_value(&value).expect("trace op");
        assert!(matches!(
            trace_op,
            crate::repl::session_trace::TraceOp::AcpLanguageLoopTraced {
                outcome,
                pending_question_code,
                prompt_context_variant,
                decode_repair_count,
                outcome_layer,
                diagnostic_codes,
                human_summary,
                conversation_efficiency,
                ..
            } if outcome == "pending_question"
                && pending_question_code.as_deref()
                    == Some("kyc_update_status_prompt_incomplete")
                && prompt_context_variant.is_none()
                && decode_repair_count == 0
                && outcome_layer == "pre_llm_pending"
                && diagnostic_codes.is_empty()
                && human_summary.contains("I stopped before drafting")
                && conversation_efficiency.pending_user_turn_required
                && !conversation_efficiency.prose_only_failure
        ));
    }

    #[tokio::test]
    async fn test_acp_llm_draft_loop_missing_evidence_is_structured_refusal() {
        let (state, session_id) = test_route_state().await;
        let client = std::sync::Arc::new(StubToolLlmClient {
            arguments: serde_json::json!({
                "session_id": session_id,
                "actor_id": "sage",
                "actor_roles": ["agent"],
                "verb": "kyc-case.update-status",
                "transition_ref": "kyc-case.intake-to-discovery",
                "subject_kind": "kyc_case",
                "case_id": test_case_id(),
                "current_state": "INTAKE",
                "requested_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": null
            }),
        });

        let value = llm_prompt_result_with_client(
            state,
            session_id,
            llm_prompt_request_with_discovery("INTAKE", "Move the KYC case to DISCOVERY", None),
            Ok(client),
        )
        .await;

        assert_eq!(value["status"], "structured_refusal");
        assert_eq!(value["refusal"]["refusal_code"], "missing_evidence_digest");
        assert_eq!(
            value["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );
        assert!(value["llm_draft"]["evidence_digest"].is_null());
    }

    #[tokio::test]
    async fn test_acp_llm_draft_loop_trace_projects_revision_refusal() {
        let (state, session_id) = test_route_state().await;
        let client = std::sync::Arc::new(StubToolLlmClient {
            arguments: serde_json::json!({
                "session_id": session_id,
                "actor_id": "sage",
                "actor_roles": ["agent"],
                "verb": "kyc-case.update-status",
                "transition_ref": "kyc-case.assessment-to-discovery",
                "subject_kind": "kyc_case",
                "case_id": test_case_id(),
                "current_state": "ASSESSMENT",
                "requested_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let value = llm_prompt_result_with_client(
            state,
            session_id,
            llm_prompt_request_with_discovery(
                "ASSESSMENT",
                "Move this ASSESSMENT KYC case back to DISCOVERY",
                Some("sha256:evidence"),
            ),
            Ok(client),
        )
        .await;

        assert_eq!(value["status"], "structured_refusal");
        assert_eq!(value["refusal"]["refusal_code"], "unknown_transition");
        assert_eq!(value["metrics"]["revision_count"], 2);

        let trace_op =
            serde_json::to_value(acp_language_loop_trace_op_from_value(&value).expect("trace op"))
                .expect("trace op json");

        assert_eq!(trace_op["prompt_context_variant"], "full_language_pack");
        assert_eq!(trace_op["decode_repair_count"], 0);
        assert_eq!(trace_op["revision_count"], 2);
        assert_eq!(trace_op["outcome_layer"], "revision_refusal");
        assert!(trace_op["diagnostic_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "unknown_transition"));
        assert!(trace_op["human_summary"]
            .as_str()
            .unwrap()
            .contains("I stopped because no transition is valid from ASSESSMENT"));
    }

    #[tokio::test]
    async fn test_acp_llm_draft_loop_missing_transition_ref_is_repaired_and_traced() {
        let (state, session_id) = test_route_state().await;
        let client = std::sync::Arc::new(StubToolLlmClient {
            arguments: serde_json::json!({
                "verb": "kyc-case.update-status",
                "subject_kind": "kyc_case",
                "case_id": test_case_id(),
                "current_state": "DISCOVERY",
                "requested_state": "ASSESSMENT",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let value = llm_prompt_result_with_client(
            state,
            session_id,
            llm_prompt_request_with_discovery(
                "DISCOVERY",
                "Advance the KYC case to ASSESSMENT",
                Some("sha256:evidence"),
            ),
            Ok(client),
        )
        .await;

        assert_eq!(value["status"], "dry_run_validated");
        assert_eq!(
            value["output"]["dry_run"]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(
            value["adapter_diagnostics"][0]["error_code"],
            "repaired_required_workbook_field"
        );
        assert_eq!(
            value["adapter_diagnostics"][0]["source_path"],
            "draft.transition_ref"
        );
        assert_eq!(
            value["adapter_diagnostics"][0]["expected_state"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(value["metrics"]["decode_repair_count"], 1);
        assert!(value["trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| { event["phase"] == "decode_repair" && event["status"] == "completed" }));
        assert_eq!(value["llm_trace"]["provider"], "stub-llm-provider");
        assert_eq!(
            value["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );

        let trace_op =
            serde_json::to_value(acp_language_loop_trace_op_from_value(&value).expect("trace op"))
                .expect("trace op json");
        assert_eq!(trace_op["prompt_context_variant"], "full_language_pack");
        assert_eq!(trace_op["decode_repair_count"], 1);
        assert_eq!(trace_op["revision_count"], 0);
        assert_eq!(trace_op["outcome_layer"], "dry_run_validated");
        assert!(trace_op["diagnostic_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "repaired_required_workbook_field"));
        assert!(trace_op["human_summary"]
            .as_str()
            .unwrap()
            .contains("I repaired 1 missing workbook field locally"));
    }

    #[tokio::test]
    #[ignore = "requires live LLM credentials and network access"]
    async fn live_acp_llm_draft_loop_language_pack_ablation_harness() {
        let _env_guard = LIVE_LLM_ENV_LOCK.lock().await;
        let anthropic_api_key = anthropic_api_key_env_name();
        let env_names = vec![
            "AGENT_BACKEND".to_string(),
            anthropic_api_key.clone(),
            "CLAUDE_CODE_MODEL".to_string(),
            "CLAUDE_CODE_MAX_BUDGET_USD".to_string(),
        ];
        let _snapshot = EnvSnapshot::capture(&env_names);
        std::env::set_var("AGENT_BACKEND", "claude-code-cli");
        std::env::remove_var(&anthropic_api_key);
        if std::env::var("CLAUDE_CODE_MAX_BUDGET_USD").is_err() {
            std::env::set_var("CLAUDE_CODE_MAX_BUDGET_USD", "0.75");
        }

        let models = ["sonnet", "claude-sonnet-4-6"];
        let variants = [
            LiveAblationVariant::FullLanguagePack,
            LiveAblationVariant::StrippedTransitionLandscape,
        ];
        let fixtures = live_ablation_fixtures();
        let mut rows = Vec::new();

        for model in models {
            std::env::set_var("CLAUDE_CODE_MODEL", model);
            for variant in variants.iter().copied() {
                for fixture in fixtures.iter().copied() {
                    let client = ob_agentic::create_llm_client()
                        .unwrap_or_else(|err| panic!("live LLM client for {model}: {err}"));
                    let (state, session_id) = test_route_state().await;
                    let (value, prompt_pack) = acp_prompt_llm_draft_loop_value_with_prompt_variant(
                        state,
                        session_id,
                        llm_prompt_request_with_discovery(
                            fixture.current_state,
                            fixture.objective,
                            Some(fixture.evidence_digest),
                        ),
                        variant,
                        Ok(client),
                    )
                    .await
                    .unwrap_or_else(|err| {
                        panic!(
                            "live ablation {model}/{}/{} failed: {err:?}",
                            variant.id(),
                            fixture.id
                        )
                    });

                    assert!(matches!(
                        value["status"].as_str(),
                        Some("dry_run_validated" | "structured_refusal")
                    ));
                    assert_eq!(
                        value["observability"]["conversationEfficiency"]["proseOnlyFailure"],
                        false
                    );

                    let row = live_ablation_row(model, variant, fixture, &value, &prompt_pack);
                    if variant == LiveAblationVariant::FullLanguagePack {
                        assert_eq!(value["status"], "dry_run_validated");
                        assert!(
                            row.exact_transition_hit,
                            "full language pack missed expected transition: {:?}",
                            row
                        );
                    }
                    rows.push(row);
                }
            }
        }

        let summary: Vec<_> = models
            .iter()
            .flat_map(|model| {
                variants
                    .iter()
                    .map(|variant| live_ablation_summary(&rows, model, *variant))
            })
            .collect();
        let deltas: Vec<_> = models
            .iter()
            .map(|model| live_ablation_deltas(&rows, model))
            .collect();
        let report = serde_json::json!({
            "metadata": live_report_metadata("live_llm_ablation_comparison"),
            "fixture_count": fixtures.len(),
            "models": models,
            "variants": variants
                .iter()
                .map(|variant| serde_json::json!({
                    "id": variant.id(),
                    "description": variant.description(),
                }))
                .collect::<Vec<_>>(),
            "summary": summary,
            "deltas": deltas,
            "rows": rows,
        });
        let report_path = write_live_report("live-llm-ablation-comparison", &report);

        println!(
            "live_llm_ablation_comparison report_path={} {}",
            report_path.display(),
            serde_json::to_string_pretty(&report).unwrap()
        );

        for model_summary in report["summary"].as_array().unwrap() {
            assert_eq!(model_summary["prose_only_failures"], 0);
            assert_eq!(model_summary["structured_outcome_rate"], 100.0);
        }
    }

    #[tokio::test]
    #[ignore = "requires live LLM credentials and network access"]
    async fn live_acp_llm_draft_loop_negative_model_comparison_harness() {
        let _env_guard = LIVE_LLM_ENV_LOCK.lock().await;
        let anthropic_api_key = anthropic_api_key_env_name();
        let env_names = vec![
            "AGENT_BACKEND".to_string(),
            anthropic_api_key.clone(),
            "CLAUDE_CODE_MODEL".to_string(),
            "CLAUDE_CODE_MAX_BUDGET_USD".to_string(),
        ];
        let _snapshot = EnvSnapshot::capture(&env_names);
        std::env::set_var("AGENT_BACKEND", "claude-code-cli");
        std::env::remove_var(&anthropic_api_key);
        if std::env::var("CLAUDE_CODE_MAX_BUDGET_USD").is_err() {
            std::env::set_var("CLAUDE_CODE_MAX_BUDGET_USD", "0.75");
        }

        let models = ["sonnet", "claude-sonnet-4-6"];
        let fixtures = live_failure_fixtures();
        let mut rows = Vec::new();

        for model in models {
            std::env::set_var("CLAUDE_CODE_MODEL", model);
            for fixture in fixtures.iter().copied() {
                let client = if fixture.request.requires_live_llm() {
                    Ok(ob_agentic::create_llm_client()
                        .unwrap_or_else(|err| panic!("live LLM client for {model}: {err}")))
                } else {
                    Err("LLM client should not be needed before state anchor".to_string())
                };
                let (state, session_id) = test_route_state().await;
                let value = llm_prompt_result_with_client(
                    state,
                    session_id,
                    live_failure_request(fixture),
                    client,
                )
                .await;

                assert_eq!(value["status"], fixture.expected_status);
                assert!(matches!(
                    value["status"].as_str(),
                    Some("structured_refusal" | "pending_question")
                ));
                assert_eq!(
                    value["observability"]["conversationEfficiency"]["proseOnlyFailure"],
                    false
                );
                assert_eq!(
                    value["observability"]["conversationEfficiency"]["dryRunValid"],
                    false
                );
                assert_ne!(value["status"], "dry_run_validated");

                let row = live_failure_row(model, fixture, &value);
                if !fixture.expected_failure_codes.is_empty() {
                    let matched_expected_code =
                        row.failure_code
                            .as_deref()
                            .map(|code| fixture.expected_failure_codes.contains(&code))
                            .unwrap_or(false)
                            || row.diagnostic_codes.iter().any(|code| {
                                fixture.expected_failure_codes.contains(&code.as_str())
                            });
                    assert!(
                        matched_expected_code,
                        "expected one of {:?}, got failure_code={:?} diagnostics={:?}",
                        fixture.expected_failure_codes, row.failure_code, row.diagnostic_codes
                    );
                }

                match value["status"].as_str() {
                    Some("structured_refusal") => {
                        let diagnostic = &value["refusal"]["diagnostics"][0];
                        assert!(diagnostic["error_code"].as_str().is_some());
                        assert!(diagnostic["source_path"].as_str().is_some());
                        assert!(diagnostic["pack_ref"].as_str().is_some());
                        assert!(diagnostic["suggested_transitions"].as_array().is_some());
                    }
                    Some("pending_question") => {
                        assert!(value["pending_question"]["code"].as_str().is_some());
                        assert!(value["pending_question"]["needs"].as_array().is_some());
                    }
                    _ => unreachable!("status asserted above"),
                }

                assert!(
                    row.diagnostic_coverage,
                    "negative fixture {} returned no structured diagnostic coverage: {:?}",
                    fixture.id, row
                );
                rows.push(row);
            }
        }

        let summary: Vec<_> = models
            .iter()
            .map(|model| live_failure_summary(&rows, model))
            .collect();
        let report = serde_json::json!({
            "metadata": live_report_metadata("live_llm_negative_comparison"),
            "fixture_count": fixtures.len(),
            "models": models,
            "summary": summary,
            "rows": rows,
        });
        let report_path = write_live_report("live-llm-negative-comparison", &report);

        println!(
            "live_llm_negative_comparison report_path={} {}",
            report_path.display(),
            serde_json::to_string_pretty(&report).unwrap()
        );

        for model_summary in report["summary"].as_array().unwrap() {
            assert_eq!(model_summary["dry_run_validated"], 0);
            assert_eq!(model_summary["prose_only_failures"], 0);
            assert_eq!(model_summary["structured_failure_rate"], 100.0);
            assert_eq!(model_summary["diagnostic_code_coverage_rate"], 100.0);
        }
    }

    #[tokio::test]
    #[ignore = "requires live LLM credentials and network access"]
    async fn live_acp_llm_draft_loop_model_comparison_harness() {
        let _env_guard = LIVE_LLM_ENV_LOCK.lock().await;
        let anthropic_api_key = anthropic_api_key_env_name();
        let env_names = vec![
            "AGENT_BACKEND".to_string(),
            anthropic_api_key.clone(),
            "CLAUDE_CODE_MODEL".to_string(),
            "CLAUDE_CODE_MAX_BUDGET_USD".to_string(),
        ];
        let _snapshot = EnvSnapshot::capture(&env_names);
        std::env::set_var("AGENT_BACKEND", "claude-code-cli");
        std::env::remove_var(&anthropic_api_key);
        if std::env::var("CLAUDE_CODE_MAX_BUDGET_USD").is_err() {
            std::env::set_var("CLAUDE_CODE_MAX_BUDGET_USD", "0.75");
        }

        let models = ["sonnet", "claude-sonnet-4-6"];
        let fixtures = live_comparison_fixtures();
        let mut rows = Vec::new();

        for model in models {
            std::env::set_var("CLAUDE_CODE_MODEL", model);
            for fixture in fixtures.iter().copied() {
                let client = ob_agentic::create_llm_client()
                    .unwrap_or_else(|err| panic!("live LLM client for {model}: {err}"));
                let (state, session_id) = test_route_state().await;
                let value = llm_prompt_result_with_client(
                    state,
                    session_id,
                    llm_prompt_request_with_discovery(
                        fixture.current_state,
                        fixture.objective,
                        Some(fixture.evidence_digest),
                    ),
                    Ok(client),
                )
                .await;

                assert!(matches!(
                    value["status"].as_str(),
                    Some("dry_run_validated" | "structured_refusal")
                ));
                assert_eq!(
                    value["observability"]["conversationEfficiency"]["proseOnlyFailure"],
                    false
                );

                match value["status"].as_str() {
                    Some("dry_run_validated") => {
                        assert_eq!(
                            value["output"]["dry_run"]["transition_ref"],
                            fixture.expected_transition
                        );
                        assert_eq!(
                            value["output"]["dry_run"]["semantic_diff"]["to_state"],
                            fixture.expected_to_state
                        );
                    }
                    Some("structured_refusal") => {
                        let diagnostic = &value["refusal"]["diagnostics"][0];
                        assert!(diagnostic["error_code"].as_str().is_some());
                        assert!(diagnostic["source_path"].as_str().is_some());
                        assert!(diagnostic["pack_ref"].as_str().is_some());
                        assert!(diagnostic["suggested_transitions"].as_array().is_some());
                    }
                    _ => unreachable!("status asserted above"),
                }

                rows.push(live_comparison_row(model, fixture, &value));
            }
        }

        let summary: Vec<_> = models
            .iter()
            .map(|model| live_comparison_summary(&rows, model))
            .collect();
        let report = serde_json::json!({
            "metadata": live_report_metadata("live_llm_comparison"),
            "fixture_count": fixtures.len(),
            "models": models,
            "summary": summary,
            "rows": rows,
        });
        let report_path = write_live_report("live-llm-comparison", &report);

        println!(
            "live_llm_comparison report_path={} {}",
            report_path.display(),
            serde_json::to_string_pretty(&report).unwrap()
        );

        for model_summary in report["summary"].as_array().unwrap() {
            assert_eq!(model_summary["prose_only_failures"], 0);
            assert_eq!(model_summary["structured_outcome_rate"], 100.0);
        }
    }

    #[tokio::test]
    #[ignore = "requires live LLM credentials and network access"]
    async fn live_acp_llm_draft_loop_smoke_validates_second_transition() {
        let _env_guard = LIVE_LLM_ENV_LOCK.lock().await;
        let client = ob_agentic::create_llm_client().expect("live LLM client");
        let (state, session_id) = test_route_state().await;

        let value = llm_prompt_result_with_client(
            state,
            session_id,
            llm_prompt_request_with_discovery(
                "DISCOVERY",
                "Advance the KYC case from DISCOVERY to ASSESSMENT using evidence sha256:live-smoke",
                Some("sha256:live-smoke"),
            ),
            Ok(client),
        )
        .await;

        println!(
            "live_llm_smoke status={} transition={} refusal_code={} diagnostic_source={} diagnostic_reason={} revision_count={} decode_repair_count={} provider={} model={} total_ms={} llm_draft_ms={} prose_only_failure={}",
            value["status"].as_str().unwrap_or("unknown"),
            value["output"]["dry_run"]["transition_ref"]
                .as_str()
                .unwrap_or("none"),
            value["refusal"]["refusal_code"].as_str().unwrap_or("none"),
            value["refusal"]["diagnostics"][0]["source_path"]
                .as_str()
                .unwrap_or("none"),
            value["refusal"]["diagnostics"][0]["blocked_transition_reason"]
                .as_str()
                .unwrap_or("none")
                .chars()
                .take(160)
                .collect::<String>(),
            value["metrics"]["revision_count"].as_u64().unwrap_or(0),
            value["metrics"]["decode_repair_count"].as_u64().unwrap_or(0),
            value["llm_trace"]["provider"].as_str().unwrap_or("unknown"),
            value["llm_trace"]["model"].as_str().unwrap_or("unknown"),
            value["observability"]["performance"]["total_ms"]
                .as_u64()
                .unwrap_or(0),
            value["observability"]["performance"]["llm_draft_ms"]
                .as_u64()
                .unwrap_or(0),
            value["observability"]["conversationEfficiency"]["proseOnlyFailure"]
                .as_bool()
                .unwrap_or(true)
        );

        assert!(matches!(
            value["status"].as_str(),
            Some("dry_run_validated" | "structured_refusal")
        ));
        assert_eq!(
            value["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );
        match value["status"].as_str() {
            Some("dry_run_validated") => {
                assert_eq!(
                    value["output"]["dry_run"]["transition_ref"],
                    "kyc-case.discovery-to-assessment"
                );
                assert_eq!(value["case_state"]["current_state"], "DISCOVERY");
                assert_eq!(
                    value["output"]["dry_run"]["semantic_diff"]["to_state"],
                    "ASSESSMENT"
                );
            }
            Some("structured_refusal") => {
                let diagnostic = &value["refusal"]["diagnostics"][0];
                assert!(diagnostic["error_code"].as_str().is_some());
                assert!(diagnostic["source_path"].as_str().is_some());
                assert!(diagnostic["pack_ref"].as_str().is_some());
                assert!(diagnostic["configuration_version"].as_str().is_some());
                assert!(diagnostic["state_snapshot_id"].as_str().is_some());
                assert!(diagnostic["suggested_transitions"].as_array().is_some());
            }
            _ => unreachable!("status asserted above"),
        }
        assert!(value["llm_trace"]["prompt_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(value["llm_trace"]["response_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    }
}

//! REST API routes for DSL v2 execution
//!
//! Session endpoints:
//! - POST   /api/session             - Create new session
//! - GET    /api/session/:id         - Get session state
//! - DELETE /api/session/:id         - Delete session
//! - POST   /api/session/:id/execute - Legacy raw DSL execution only
//! - POST   /api/session/:id/clear   - Clear session
//! - GET    /api/session/:id/context - Get session context (CBU, linked entities, symbols)
//!
//! Vocabulary endpoints:
//! - GET    /api/agent/domains      - List available DSL domains
//! - GET    /api/agent/vocabulary   - Get vocabulary for a domain
//! - GET    /api/agent/health       - Health check
//!
//! Onboarding endpoints:
//! - POST   /api/agent/onboard           - Generate onboarding DSL from natural language
//! - GET    /api/agent/onboard/templates - List available onboarding templates
//! - POST   /api/agent/onboard/render    - Render an onboarding template with parameters

use crate::api::session::{
    CreateSessionRequest, CreateSessionResponse, ExecuteResponse, ExecutionResult,
    SessionStateResponse,
};
// Use unified session types - single source of truth
use crate::session::{
    MessageRole, ResearchSubSession, ResolutionSubSession, ReviewStatus, ReviewSubSession,
    SessionState, SubSessionType, UnifiedSession,
};

// API types - SINGLE SOURCE OF TRUTH for HTTP boundary
use crate::database::derive_semantic_state;
use crate::database::generation_log_repository::{
    CompileResult, ExecutionStatus, GenerationAttempt, LintResult, ParseResult,
};
use crate::dsl_v2::execution::{
    runtime_registry, AtomicExecutionResult, ExecutionContext, ExecutionResult as DslV2Result,
};
use crate::dsl_v2::planning::compile;
use crate::dsl_v2::syntax::parse_program;
use crate::dsl_v2::tooling::SemanticValidator;
use crate::dsl_v2::{expand_templates_simple, BatchPolicy};
use ob_poc_ontology::SemanticStageRegistry;
use ob_poc_types::{DslState, SessionInputRequest, SessionInputResponse};
use std::time::Instant;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use uuid::Uuid;

// Re-export all request/response types from agent_types
pub(crate) use crate::api::agent_types::ExecutionOutcome;
pub use crate::api::agent_types::{
    BatchAddProductsRequest, BatchAddProductsResponse, BatchProductResult, CompleteRequest,
    CompleteResponse, CompleteSubSessionRequest, CompleteSubSessionResponse, CompletionItem,
    CreateSubSessionRequest, CreateSubSessionResponse, CreateSubSessionType, DomainInfo,
    DomainsResponse, EntityCandidateResponse, EntityMentionResponse, EvidenceResponse,
    ExecuteDslRequest, ExtractEntitiesRequest, ExtractEntitiesResponse, GenerateDslRequest,
    GenerateDslResponse, HealthResponse, MissingArg, OnboardingExecutionResult, OnboardingRequest,
    OnboardingResponse, ParseDiscriminatorsRequest, ParseDiscriminatorsResponse, ParseDslRequest,
    ParseDslResponse, ParsedDiscriminators, PipelineStage, RefId, RemainingUnresolvedRef,
    ReportCorrectionRequest, ReportCorrectionResponse, ResolutionState, ResolutionStats,
    ResolveByRefIdRequest, ResolveByRefIdResponse, ResolveRefRequest, ResolveRefResponse,
    SetBindingRequest, SetBindingResponse, SetFocusRequest, SetFocusResponse,
    SubSessionChatRequest, SubSessionMessage, SubSessionStateResponse, UnresolvedRef,
    ValidationError, ValidationResult, VerbInfo, VerbSurfaceQuery, VocabQuery, VocabResponse,
    WatchQuery, WatchResponse,
};

// ============================================================================
// State — see agent_state.rs for AgentState and create_agent_router_with_semantic()
// ============================================================================

pub(crate) use crate::api::agent_state::AgentState;

// ============================================================================
// Router
// ============================================================================

/// Internal: create router from pre-built state
pub(crate) fn create_agent_router_with_state(state: AgentState) -> Router {
    Router::new()
        // Session management
        .route("/api/session", post(create_session))
        .route("/api/session/:id", get(get_session))
        .route("/api/session/:id", delete(delete_session))
        .route("/api/session/:id/input", post(session_input))
        // F20 fix (Slice 5.2, 2026-04-22): legacy `/api/session/:id/chat`
        // route removed. Previously returned 410 Gone — now returns 404 from
        // the router. Use `/api/session/:id/input` with `kind=utterance`.
        .route(
            "/api/session/:id/execute",
            post(execute_session_dsl_legacy_raw_only),
        )
        .route("/api/session/:id/clear", post(clear_session_dsl))
        .route("/api/session/:id/bind", post(set_session_binding))
        .route("/api/session/:id/context", get(get_session_context))
        .route(
            "/api/session/:id/verb-surface",
            get(get_session_verb_surface),
        )
        .route("/api/session/:id/focus", post(set_session_focus))
        .route("/api/session/:id/dsl/enrich", get(get_enriched_dsl))
        .route("/api/session/:id/watch", get(watch_session))
        // Sub-session management (create/get/complete/cancel only - chat goes through main pipeline)
        .route("/api/session/:id/subsession", post(create_subsession))
        .route("/api/session/:id/subsession/:child_id", get(get_subsession))
        .route(
            "/api/session/:id/subsession/:child_id/complete",
            post(complete_subsession),
        )
        .route(
            "/api/session/:id/subsession/:child_id/cancel",
            post(cancel_subsession),
        )
        // DSL routes removed — all DSL generation through unified REPL pipeline
        // Learning routes removed — verb selection signals through REPL pipeline
        // Semantic OS context
        .route("/api/sem-os/context", get(get_semos_context))
        // T7.2 (EOP-PLAN-CONTROLPLANE-001): control-plane observability
        .route(
            "/api/control-plane/metrics",
            get(get_control_plane_metrics),
        )
        // F20 fix (Slice 5.2, 2026-04-22): legacy `/decision/reply` route
        // removed. Previously returned 410 Gone — now 404 from the router.
        // Use `/api/session/:id/input` with `kind=decision_reply`.
        .with_state(state)
}

// ============================================================================
// Session Handlers
// ============================================================================

// F20 fix (Slice 5.2, 2026-04-22): `chat_session_legacy_blocked` and
// `decision_reply_legacy_blocked` 410-Gone stub handlers removed. The
// routes themselves are gone too; requests to them now return 404 from
// the axum router. Use `POST /api/session/:id/input` with
// `kind=utterance` or `kind=decision_reply` instead.

/// POST /api/session/:id/input - Unified session input endpoint.
async fn session_input(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    _headers: axum::http::HeaderMap,
    Json(req): Json<SessionInputRequest>,
) -> Result<Json<SessionInputResponse>, StatusCode> {
    // R8 single-path unification (2026-05-11): `session_input` is now a
    // single dispatch decision. The ACP DAG semantic resolution previously
    // racing here (via `try_route_supported_acp_prompt`) now fires inside
    // `ReplOrchestratorV2::process_with_acp()` as the orchestrator's
    // first step. ACP-resolved responses surface here as a
    // `ReplResponseV2` with `prebuilt_chat_response: Some(_)`; the
    // response adapter short-circuits on that field to preserve the
    // original wire shape.
    if let Some(ref orchestrator) = state.repl_v2_orchestrator {
        if let Some(repl_response) = dispatch_to_v2_repl(&req, orchestrator, session_id).await {
            // Extract onboarding state from REPL response BEFORE moving it into
            // the chat response adapter (which takes ownership). Reads from the
            // hydrated constellation on session feedback — same DAG the compiler uses.
            let onboarding_from_dag =
                crate::api::agent_enrichment::try_onboarding_from_repl_response(
                    &repl_response,
                    None,
                );

            let mut chat_response =
                crate::api::response_adapter::repl_to_chat_response(repl_response, session_id);

            if chat_response.decision.is_none() {
                if let Some(osv) = onboarding_from_dag {
                    chat_response.onboarding_state = Some(osv);
                } else {
                    #[cfg(feature = "database")]
                    if let Some(osv) =
                        crate::api::agent_enrichment::compute_onboarding_state_from_db(
                            &state.pool,
                            session_id,
                            None,
                        )
                        .await
                    {
                        chat_response.onboarding_state = Some(osv);
                    }
                }
            }

            return Ok(Json(SessionInputResponse::Chat {
                response: Box::new(chat_response),
            }));
        }
    }

    // No REPL V2 session found — this is a pre-migration session or an error.
    // Log and return a helpful error rather than silently routing to the legacy pipeline.
    tracing::warn!(
        session_id = %session_id,
        "No REPL V2 session found — session may have been created before pipeline unification"
    );
    Err(StatusCode::NOT_FOUND)
}

/// Dispatch session input through the V2 REPL orchestrator.
///
/// **R5 (2026-05-11):** renamed from `try_route_through_repl` to remove the
/// misleading "try/fallback" framing. This is the **canonical V2 REPL
/// ingress for the unified session pipeline** — not a legacy bypass.
///
/// Routing order in `session_input`:
/// 1. `try_route_supported_acp_prompt` — narrow ACP DAG semantic
///    resolution for Slice 1 pack-bound utterances.
/// 2. **`dispatch_to_v2_repl`** (this fn) — canonical V2 REPL for
///    everything else: slash commands, confirmations, decision replies,
///    generic messages. Drives tollgate state, runbook execution,
///    narration, and decision-log emission via `orchestrator.process()`.
/// 3. `404` — no V2 REPL session exists; unrecoverable.
///
/// The V2 REPL orchestrator is itself envelope-aware — it loads packs,
/// verifies SemOS context, and gates DSL emission through its internal
/// pack resolution. End-to-end envelope-gating proof of `orchestrator
/// .process()` is tracked as TECH DEBT TD-4.
///
/// Returns `Some(ReplResponseV2)` if a V2 REPL session exists; returns
/// `None` if it doesn't (caller returns 404).
// R8 single-path unification (2026-05-11, §13.1):
//   `AcpSessionInputDraftMode` moved to `ob_poc_boundary::acp_session_input_draft_mode`
//   and is now an `ReplOrchestratorV2` field. The env-var read fires once at
//   orchestrator construction (see `ReplOrchestratorV2::with_acp_draft_mode`
//   + `AcpSessionInputDraftMode::from_env`). HTTP callers consult the
//   orchestrator instead of reading env per request.
use ob_poc_boundary::acp_session_input_draft_mode::AcpSessionInputDraftMode;

/// R8 Phase B bundle returned to the orchestrator. Carries the typed
/// resolution (with `route_metadata` populated) alongside the message
/// and DSL the chat response needs. Replaces Phase A's `ChatResponse`
/// pre-built carrier — the orchestrator builds the typed
/// `ReplResponseV2` from this bundle, and the adapter projects to
/// `ChatResponse` from typed sources.
pub(crate) struct AcpResolvedBundle {
    pub resolution: ob_poc_boundary::acp_dag_semantic::AcpDagSemanticResolution,
    pub message: String,
    pub dsl: Option<ob_poc_types::DslState>,
    pub session_feedback: Option<serde_json::Value>,
}

pub(crate) async fn try_route_supported_acp_prompt(
    orchestrator: &std::sync::Arc<crate::sequencer::ReplOrchestratorV2>,
    session_id: Uuid,
    message: &str,
) -> Option<AcpResolvedBundle> {
    try_route_supported_acp_prompt_with_draft_mode(
        orchestrator,
        session_id,
        message,
        orchestrator.acp_session_input_draft_mode(),
    )
    .await
}

async fn try_route_supported_acp_prompt_with_draft_mode(
    orchestrator: &std::sync::Arc<crate::sequencer::ReplOrchestratorV2>,
    session_id: Uuid,
    message: &str,
    requested_draft_mode: AcpSessionInputDraftMode,
) -> Option<AcpResolvedBundle> {
    let route_started_at = Instant::now();
    let prompt_text = message.trim();
    if prompt_text.is_empty() {
        return None;
    }
    let prompt = vec![ob_poc_boundary::acp_protocol::AcpContentBlock::Text {
        text: prompt_text.to_string(),
    }];
    let supported_provider_task =
        crate::acp_state_anchor::acp_prompt_supported_provider_task(&prompt);
    orchestrator.get_session(session_id).await.as_ref()?;

    let route_state = crate::api::repl_routes_v2::ReplV2RouteState {
        orchestrator: orchestrator.clone(),
    };
    let mut effective_draft_mode = supported_provider_task
        .filter(|task| requested_draft_mode.can_run_for_task(task))
        .map(|_| requested_draft_mode)
        .unwrap_or(AcpSessionInputDraftMode::Deterministic);
    let mut envelope = match effective_draft_mode {
        AcpSessionInputDraftMode::Deterministic => {
            crate::api::repl_routes_v2::process_acp_prompt_deterministic_envelope(
                &route_state,
                session_id,
                prompt,
                serde_json::json!("session-input-acp"),
                "acp_session_input_processed",
            )
            .await
        }
        AcpSessionInputDraftMode::LiveLlm => {
            let client = ob_agentic::create_llm_client().map_err(|error| error.to_string());
            match crate::api::repl_routes_v2::process_acp_prompt_llm_envelope(
                &route_state,
                session_id,
                prompt,
                serde_json::json!("session-input-acp"),
                "acp_session_input_processed",
                client,
            )
            .await
            {
                Ok(envelope) => envelope,
                Err((status, error)) => {
                    let provider_task = supported_provider_task.unwrap_or("dag.semantic");
                    tracing::warn!(
                        session_id = %session_id,
                        task = provider_task,
                        requested_draft_source = requested_draft_mode.as_str(),
                        status = %status,
                        error = %error.0.error,
                        "ACP session input LLM draft failed before structured result; falling back to deterministic ACP"
                    );
                    effective_draft_mode = AcpSessionInputDraftMode::Deterministic;
                    crate::api::repl_routes_v2::process_acp_prompt_deterministic_envelope(
                        &route_state,
                        session_id,
                        vec![ob_poc_boundary::acp_protocol::AcpContentBlock::Text {
                            text: prompt_text.to_string(),
                        }],
                        serde_json::json!("session-input-acp"),
                        "acp_session_input_processed",
                    )
                    .await
                }
            }
        }
    };
    let task = acp_session_input_task_label(supported_provider_task, &envelope);
    annotate_acp_session_input_envelope(
        &mut envelope,
        &task,
        requested_draft_mode,
        effective_draft_mode,
        route_started_at,
    );
    let result = envelope.get("result")?;
    let result_status = result.get("status").and_then(serde_json::Value::as_str)?;
    if !matches!(
        result_status,
        "dry_run_validated" | "structured_refusal" | "pending_question" | "dag_semantic_proposal"
    ) {
        tracing::warn!(
            session_id = %session_id,
            task = %task,
            result_status,
            "ACP prompt returned non-structured result; falling back to REPL"
        );
        return None;
    }

    emit_acp_session_input_observability(
        session_id,
        &task,
        requested_draft_mode,
        effective_draft_mode,
        &envelope,
        result_status,
    );

    let assistant_message = acp_agent_message_text(&envelope)
        .or_else(|| value_string(result, &["traceProjection", "humanSummary"]))
        .unwrap_or_else(|| {
            "ACP handled this state-transition request with a structured dry-run-only outcome."
                .to_string()
        });
    let dsl = acp_valid_dag_semantic_draft_dsl(&envelope);
    if let Err(error) = orchestrator
        .record_external_chat_exchange(
            session_id,
            prompt_text.to_string(),
            assistant_message.clone(),
        )
        .await
    {
        tracing::warn!(
            session_id = %session_id,
            task = %task,
            error = %error,
            "Failed to record ACP chat exchange in REPL session history"
        );
    }
    let session_feedback = orchestrator
        .session_feedback(session_id)
        .await
        .ok()
        .and_then(|feedback| serde_json::to_value(feedback).ok());

    // R8 Phase B (2026-05-11): parse typed resolution from the envelope
    // and build typed `AcpRouteMetadata` from the `session_input` block
    // we just annotated. The orchestrator builds `ReplResponseV2` from
    // these typed sources; the response adapter projects `acp_trace`
    // via `acp_chat_trace_summary_typed` instead of carrying the
    // pre-built ChatResponse.
    // R8 Phase B.7: the deal language-pack flow produces a result with
    // status `dry_run_validated` but no `dag_semantic` block — for that
    // path we synthesize a stub resolution and use `override_status` to
    // carry the language-pack outcome through the typed chat-trace.
    let mut resolution = match result.get("dag_semantic").and_then(|v| {
        serde_json::from_value::<ob_poc_boundary::acp_dag_semantic::AcpDagSemanticResolution>(
            v.clone(),
        )
        .ok()
    }) {
        Some(r) => r,
        None => ob_poc_boundary::acp_dag_semantic::AcpDagSemanticResolution {
            status: ob_poc_boundary::acp_dag_semantic::AcpDagSemanticStatus::Matched,
            utterance: prompt_text.to_string(),
            selected_dispatch: None,
            selected_verb: None,
            selected_domain: None,
            selected_description: None,
            pack: None,
            selected_template: None,
            top_candidates: Vec::new(),
            rejected_candidates: Vec::new(),
            draft_dsl: None,
            workflow_plan: None,
            missing_required_args: Vec::new(),
            unresolved_refs: Vec::new(),
            read_only: true,
            mutation_allowed: false,
            requires_hitl: false,
            structured_outcome_supported: true,
            registry_trace: None,
            envelope_trace: None,
            runtime_trace: None,
            diagnostics: Vec::new(),
            route_metadata: None,
            state_anchor_provider: None,
            observability: None,
            override_status: None,
        },
    };

    // Augment runtime_trace with the session-context-aware version that
    // `attach_session_runtime_trace_to_result` produced under
    // `dag_semantic.runtime_trace` on the envelope. The resolver itself
    // produced one without session context (session_id: None); the HTTP
    // attachment overrides it. Parsing back into typed picks up the
    // overridden one if it's present in the envelope.
    //
    // Read the typed route metadata from the envelope's session_input
    // block (just annotated by `annotate_acp_session_input_envelope`).
    let route_latency_us =
        value_u64(&envelope, &["session_input", "route_latency_us"]).unwrap_or(0);
    let route_latency_ms =
        value_u64(&envelope, &["session_input", "route_latency_ms"]).unwrap_or(0);
    let effective_draft_source =
        value_string(&envelope, &["session_input", "effective_draft_source"])
            .unwrap_or_else(|| effective_draft_mode.as_str().to_string());
    resolution.route_metadata = Some(ob_poc_boundary::acp_dag_semantic::AcpRouteMetadata {
        route: "session_input".to_string(),
        provider_task: task.clone(),
        requested_draft_source: requested_draft_mode.as_str().to_string(),
        effective_draft_source,
        route_latency_us,
        route_latency_ms,
    });

    // R8 Phase B.7 (2026-05-11): parse the typed `state_anchor_provider`
    // block + observability summary so the chat-trace projection has
    // them without re-reading the JSON envelope.
    resolution.state_anchor_provider = envelope
        .get("state_anchor_provider")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    resolution.observability = Some(ob_poc_boundary::acp_dag_semantic::AcpObservabilitySummary {
        structured_failure_mode: value_string(
            result,
            &[
                "observability",
                "conversationEfficiency",
                "structuredFailureMode",
            ],
        ),
        prose_only_failure: value_bool(
            result,
            &[
                "observability",
                "conversationEfficiency",
                "proseOnlyFailure",
            ],
        ),
        revision_count: value_u64(
            result,
            &[
                "observability",
                "conversationEfficiency",
                "localRevisionCount",
            ],
        )
        .or_else(|| value_u64(result, &["metrics", "revision_count"])),
        pending_user_turn_required: value_bool(
            result,
            &[
                "observability",
                "conversationEfficiency",
                "pendingUserTurnRequired",
            ],
        ),
        estimated_user_repair_turns_avoided: value_u64(
            result,
            &[
                "observability",
                "conversationEfficiency",
                "estimatedUserRepairTurnsAvoided",
            ],
        ),
        transition_ref: value_string(result, &["traceProjection", "transitionRef"])
            .or_else(|| value_string(result, &["output", "dry_run", "transition_ref"])),
    });
    if matches!(result_status, "dry_run_validated") {
        resolution.override_status = Some("dry_run_validated".to_string());
    }

    Some(AcpResolvedBundle {
        resolution,
        message: assistant_message,
        dsl,
        session_feedback,
    })
}

fn acp_session_input_task_label(
    supported_provider_task: Option<&'static str>,
    envelope: &serde_json::Value,
) -> String {
    supported_provider_task
        .map(str::to_string)
        .or_else(|| value_string(envelope, &["result", "dag_semantic", "selected_verb"]))
        .or_else(|| {
            envelope
                .get("result")
                .and_then(dag_semantic_candidate_verbs)
                .and_then(|candidates| candidates.into_iter().next())
        })
        .or_else(|| value_string(envelope, &["state_anchor_provider", "task"]))
        .unwrap_or_else(|| "dag.semantic".to_string())
}

fn annotate_acp_session_input_envelope(
    envelope: &mut serde_json::Value,
    task: &str,
    requested_draft_mode: AcpSessionInputDraftMode,
    effective_draft_mode: AcpSessionInputDraftMode,
    route_started_at: Instant,
) {
    let route_latency_us = route_started_at
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    let route_latency_ms = route_latency_us.div_ceil(1_000);
    let effective_draft_source = envelope
        .get("result")
        .and_then(|result| value_string(result, &["draft_source"]))
        .unwrap_or_else(|| effective_draft_mode.as_str().to_string());
    let session_input = serde_json::json!({
        "route": "session_input",
        "provider_task": task,
        "requested_draft_source": requested_draft_mode.as_str(),
        "effective_draft_source": effective_draft_source,
        "route_latency_ms": route_latency_ms,
        "route_latency_us": route_latency_us,
        "selected": true,
        "dry_run_only": true,
        "no_mutation_authority": true
    });

    if let Some(object) = envelope.as_object_mut() {
        object.insert("session_input".to_string(), session_input.clone());
    }
    if let Some(result_object) = envelope
        .get_mut("result")
        .and_then(serde_json::Value::as_object_mut)
    {
        result_object.insert("session_input".to_string(), session_input);
    }
}

fn emit_acp_session_input_observability(
    session_id: Uuid,
    task: &str,
    requested_draft_mode: AcpSessionInputDraftMode,
    effective_draft_mode: AcpSessionInputDraftMode,
    envelope: &serde_json::Value,
    result_status: &str,
) {
    let result = envelope.get("result").unwrap_or(&serde_json::Value::Null);
    let metrics = result.get("metrics").unwrap_or(&serde_json::Value::Null);
    let efficiency = result
        .pointer("/observability/conversationEfficiency")
        .unwrap_or(&serde_json::Value::Null);
    let session_input = envelope
        .get("session_input")
        .unwrap_or(&serde_json::Value::Null);

    tracing::info!(
        session_id = %session_id,
        provider_task = task,
        requested_draft_source = requested_draft_mode.as_str(),
        effective_draft_source = value_string(session_input, &["effective_draft_source"]).as_deref().unwrap_or(effective_draft_mode.as_str()),
        result_status,
        invented_verb_count = value_u64(metrics, &["invented_verb_count"]).unwrap_or(0),
        uuid_binding_complete = value_bool(metrics, &["uuid_binding_complete"]).unwrap_or(false),
        state_valid_transition_selected = value_bool(metrics, &["state_valid_transition_selected"]).unwrap_or(false),
        first_pass_valid = value_bool(efficiency, &["firstPassValid"])
            .or_else(|| value_bool(metrics, &["first_pass_valid"]))
            .unwrap_or(false),
        revision_count = value_u64(efficiency, &["localRevisionCount"])
            .or_else(|| value_u64(metrics, &["revision_count"]))
            .unwrap_or(0),
        dry_run_valid = value_bool(efficiency, &["dryRunValid"])
            .or_else(|| value_bool(metrics, &["dry_run_valid"]))
            .unwrap_or(false),
        pending_user_turn_required = value_bool(efficiency, &["pendingUserTurnRequired"]).unwrap_or(false),
        prose_only_failure = value_bool(efficiency, &["proseOnlyFailure"]).unwrap_or(false),
        route_latency_us = value_u64(session_input, &["route_latency_us"]).unwrap_or(0),
        "ACP session input route completed"
    );
}

fn acp_agent_message_text(envelope: &serde_json::Value) -> Option<String> {
    envelope
        .get("outgoing")
        .and_then(serde_json::Value::as_array)
        .and_then(|outgoing| {
            outgoing.iter().find_map(|item| {
                let update = item.pointer("/params/update")?;
                if update
                    .get("sessionUpdate")
                    .and_then(serde_json::Value::as_str)
                    != Some("agent_message_chunk")
                {
                    return None;
                }
                value_string(update, &["content", "text"])
            })
        })
}

fn acp_valid_dag_semantic_draft_dsl(envelope: &serde_json::Value) -> Option<DslState> {
    let result = envelope.get("result")?;
    let first_pass_valid = result
        .pointer("/traceProjection/firstPassValid")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !first_pass_valid {
        return None;
    }
    value_string(result, &["dsl"])
        .or_else(|| value_string(result, &["dag_semantic", "draft_dsl"]))
        .map(|source| DslState {
            source: Some(source),
            ast: None,
            can_execute: false,
            bindings: Default::default(),
        })
}

fn dag_semantic_candidate_verbs(result: &serde_json::Value) -> Option<Vec<String>> {
    let candidates = result
        .pointer("/dag_semantic/top_candidates")?
        .as_array()?
        .iter()
        // R3: candidate field renamed from `verb` to `fqn`. Read both
        // for safety during the migration window.
        .filter_map(|candidate| {
            value_string(candidate, &["fqn"]).or_else(|| value_string(candidate, &["verb"]))
        })
        .collect::<Vec<_>>();
    (!candidates.is_empty()).then_some(candidates)
}

fn value_at_path<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
}

fn value_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    value_at_path(value, path)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn value_bool(value: &serde_json::Value, path: &[&str]) -> Option<bool> {
    value_at_path(value, path).and_then(serde_json::Value::as_bool)
}

fn value_u64(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    value_at_path(value, path).and_then(serde_json::Value::as_u64)
}

async fn dispatch_to_v2_repl(
    req: &SessionInputRequest,
    orchestrator: &std::sync::Arc<crate::sequencer::ReplOrchestratorV2>,
    session_id: Uuid,
) -> Option<crate::repl::response_v2::ReplResponseV2> {
    use crate::repl::types_v2::UserInputV2;

    // Check if a REPL V2 session exists for this ID
    let session_exists = orchestrator.get_session(session_id).await.is_some();
    if !session_exists {
        return None; // No V2 REPL session for this id — caller emits 404.
    }

    let input = match req {
        SessionInputRequest::Utterance { message } => {
            // Detect slash commands from the chat input
            let trimmed = message.trim();
            if let Some(cmd) = trimmed.strip_prefix('/') {
                use crate::repl::types_v2::ReplCommandV2;
                match cmd.to_lowercase().as_str() {
                    "run" => UserInputV2::Command {
                        command: ReplCommandV2::Run,
                    },
                    "undo" => UserInputV2::Command {
                        command: ReplCommandV2::Undo,
                    },
                    "redo" => UserInputV2::Command {
                        command: ReplCommandV2::Redo,
                    },
                    "clear" => UserInputV2::Command {
                        command: ReplCommandV2::Clear,
                    },
                    "cancel" => UserInputV2::Command {
                        command: ReplCommandV2::Cancel,
                    },
                    "info" => UserInputV2::Command {
                        command: ReplCommandV2::Info,
                    },
                    _ => UserInputV2::Message {
                        content: message.clone(),
                    },
                }
            } else if matches!(
                trimmed.to_lowercase().as_str(),
                "confirm"
                    | "yes"
                    | "go"
                    | "do it"
                    | "run it"
                    | "run"
                    | "execute"
                    | "proceed"
                    | "make it so"
                    | "ok"
                    | "yep"
                    | "sure"
                    | "approved"
                    | "lgtm"
            ) {
                UserInputV2::Confirm
            } else if matches!(
                trimmed.to_lowercase().as_str(),
                "no" | "reject"
                    | "cancel"
                    | "nope"
                    | "not that"
                    | "wrong"
                    | "try again"
                    | "skip"
                    | "back"
            ) {
                UserInputV2::Reject
            } else {
                UserInputV2::Message {
                    content: message.clone(),
                }
            }
        }
        SessionInputRequest::DecisionReply { reply, .. } => {
            // Convert decision reply to a REPL message.
            // The REPL orchestrator handles numeric/name resolution.
            match reply {
                ob_poc_types::UserReply::Select { index } => {
                    UserInputV2::Message {
                        content: format!("{}", index + 1), // 1-indexed for REPL
                    }
                }
                ob_poc_types::UserReply::TypeExact { text } => UserInputV2::Message {
                    content: text.clone(),
                },
                ob_poc_types::UserReply::Confirm { .. } => UserInputV2::Confirm,
                _ => return None, // Narrow/More/Reject — not a V2 REPL UserInput shape; caller emits 404.
            }
        }
        SessionInputRequest::ReplV2 { input } => {
            let input: crate::api::repl_routes_v2::InputRequestV2 =
                match serde_json::from_value(input.clone()) {
                    Ok(input) => input,
                    Err(error) => {
                        tracing::warn!(
                            session_id = %session_id,
                            error = %error,
                            "Invalid REPL V2 input payload; caller will surface 404."
                        );
                        return None;
                    }
                };
            input.into()
        }
        _ => return None, // DiscoverySelection is handled by the chat adapter, not the V2 REPL.
    };

    let is_run_command = matches!(
        &input,
        UserInputV2::Command {
            command: crate::repl::types_v2::ReplCommandV2::Run
        }
    );

    // R8 §13.5 (2026-05-11): call `process_with_acp` so the orchestrator
    // owns the ACP DAG semantic resolution decision as its first step
    // (Message inputs only; state-independent). On no match it falls
    // through to the standard `process()`.
    //
    // `/run` is the one session-input command that can cross from chat into
    // execution, persistence, and lock planning. Keep the HTTP ingress bounded
    // even if a lower layer blocks before the per-step runbook timeout starts.
    let process_result = if is_run_command {
        match tokio::time::timeout(
            std::time::Duration::from_secs(6),
            orchestrator.process_with_acp(session_id, input),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                tracing::error!(
                    session_id = %session_id,
                    "V2 REPL /run timed out at session-input ingress"
                );
                return Some(crate::repl::response_v2::ReplResponseV2 {
                    state: crate::repl::types_v2::ReplStateV2::RunbookEditing,
                    kind: crate::repl::response_v2::ReplResponseKindV2::Error {
                        error: "Run timed out before execution completed".to_string(),
                        recoverable: true,
                    },
                    message: "Run timed out before execution completed. The runbook was left editable so you can inspect or retry.".to_string(),
                    runbook_summary: None,
                    step_count: 0,
                    session_feedback: None,
                    narration: None,
                    trace_id: None,
                    acp_dag_semantic: None,
                    bpmn_form: None,
                });
            }
        }
    } else {
        orchestrator.process_with_acp(session_id, input).await
    };

    match process_result {
        Ok(response) => Some(response),
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "V2 REPL orchestrator returned an error; caller will surface 404."
            );
            None
        }
    }
}

fn is_raw_execute_request(req: &Option<ExecuteDslRequest>) -> bool {
    req.as_ref()
        .and_then(|request| request.dsl.as_ref())
        .map(|dsl| !dsl.trim().is_empty())
        .unwrap_or(false)
}

/// POST /api/session - Create new session
async fn create_session(
    State(state): State<AgentState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    use crate::session::constraint_cascade::update_dag_from_cascade;
    use crate::session::unified::StructureType;

    tracing::info!("=== CREATE SESSION ===");
    tracing::info!("Domain hint: {:?}", req.domain_hint);
    tracing::info!("Initial client: {:?}", req.initial_client);
    tracing::info!("Structure type: {:?}", req.structure_type);
    tracing::info!("Workflow focus: {:?}", req.workflow_focus);

    let mut session = UnifiedSession::new_for_entity(None, "cbu", None, req.domain_hint.clone());
    let session_id = session.id;
    let created_at = session.created_at;

    // Semantic OS workflow: skip client resolution, present workflow selection
    if req.workflow_focus.as_deref() == Some("semantic-os") {
        tracing::info!("Semantic OS session — building workflow selection packet");

        // Store in memory. The V2 REPL session owns conversation history via
        // `create_session_with_id`; do not write the welcome to the V1
        // UnifiedSession.
        {
            let mut sessions = state.sessions.write().await;
            let packet = build_semos_workflow_decision(session_id);
            session.pending_decision = Some(packet.clone());
            sessions.insert(session_id, session);
        }

        // Persist to database asynchronously
        let session_repo = state.session_repo.clone();
        tokio::spawn(async move {
            if let Err(e) = session_repo
                .create_session_with_id(session_id, None, None)
                .await
            {
                tracing::error!("Failed to persist session {}: {}", session_id, e);
            }
        });

        let decision = {
            let sessions = state.sessions.read().await;
            sessions
                .get(&session_id)
                .and_then(|s| s.pending_decision.clone())
        };

        let response = CreateSessionResponse {
            session_id,
            created_at,
            state: SessionState::New.into(),
            welcome_message: "Welcome to Semantic OS. What would you like to work on?".to_string(),
            decision,
            session_feedback: None,
        };
        return Ok(Json(response));
    }

    // Wire initial client constraint if provided
    let (final_state, welcome_message) = if let Some(ref client_ref) = req.initial_client {
        // Try to resolve client from client_id or client_name
        let resolved_client = resolve_initial_client(&state.pool, client_ref).await;

        match resolved_client {
            Ok(client) => {
                tracing::info!(
                    "Setting initial client constraint: {} ({})",
                    client.display_name,
                    client.client_id
                );

                // Set client context on session (constraint cascade level 1)
                session.client = Some(client.clone());

                // Set structure type if provided (constraint cascade level 2)
                if let Some(ref st) = req.structure_type {
                    if let Some(structure_type) = StructureType::from_internal(st) {
                        session.structure_type = Some(structure_type);
                        tracing::info!("Setting structure type: {:?}", structure_type);
                    }
                }

                // Update DAG state from cascade
                update_dag_from_cascade(&mut session);

                // Transition to Scoped state
                session.transition(crate::session::unified::SessionEvent::ScopeSet);

                (
                    SessionState::Scoped,
                    format!(
                        "Session scoped to {}. What would you like to do?",
                        client.display_name
                    ),
                )
            }
            Err(e) => {
                tracing::warn!("Failed to resolve initial client: {}", e);
                // Fall back to prompting for client
                (
                    SessionState::New,
                    crate::api::session::WELCOME_MESSAGE.to_string(),
                )
            }
        }
    } else {
        // No initial client - prompt for client selection
        (
            SessionState::New,
            crate::api::session::WELCOME_MESSAGE.to_string(),
        )
    };

    tracing::info!("Created session ID: {}", session_id);

    // Store in memory
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
        tracing::info!(
            "Session stored in memory, total sessions: {}",
            sessions.len()
        );
    }

    // Also create a REPL V2 session with the same ID (for unified pipeline routing).
    // The REPL session starts in ScopeGate — enforcing client group selection.
    if let Some(ref orchestrator) = state.repl_v2_orchestrator {
        let is_xtask = headers
            .get("x-obpoc-actor-id")
            .and_then(|h| h.to_str().ok())
            .map(|s| {
                s == "xtask-utterance-roundtrip" || s == "seeded-capability" || s == "coverage-bot"
            })
            .unwrap_or(false);

        if is_xtask {
            let domain_str = req.domain_hint.as_deref().unwrap_or("cbu").to_lowercase();
            let initial_workspace = match domain_str.as_str() {
                "cbu" => Some(crate::repl::types_v2::WorkspaceKind::Cbu),
                "kyc" => Some(crate::repl::types_v2::WorkspaceKind::Kyc),
                "deal" => Some(crate::repl::types_v2::WorkspaceKind::Deal),
                "onboarding" | "on_boarding" => {
                    Some(crate::repl::types_v2::WorkspaceKind::OnBoarding)
                }
                "instrument_matrix" | "instrument" => {
                    Some(crate::repl::types_v2::WorkspaceKind::InstrumentMatrix)
                }
                "product" | "product_maintenance" => {
                    Some(crate::repl::types_v2::WorkspaceKind::ProductMaintenance)
                }
                "catalogue" => Some(crate::repl::types_v2::WorkspaceKind::Catalogue),
                "bpmn" => Some(crate::repl::types_v2::WorkspaceKind::Bpmn),
                "lifecycle" | "lifecycle_resources" => {
                    Some(crate::repl::types_v2::WorkspaceKind::LifecycleResources)
                }
                "semos" | "semos_maintenance" => {
                    Some(crate::repl::types_v2::WorkspaceKind::SemOsMaintenance)
                }
                _ => None,
            };

            orchestrator
                .create_session_with_state(
                    session_id,
                    crate::repl::types_v2::ReplStateV2::ScopeGate {
                        pending_input: None,
                        candidates: None,
                    },
                    true,
                    initial_workspace,
                )
                .await;
            tracing::info!(
                "REPL V2 session created for unified routing with bypassed gates: {session_id}"
            );
        } else {
            orchestrator.create_session_with_id(session_id).await;
            tracing::info!("REPL V2 session created for unified routing: {session_id}");
        }
    }

    // Persist to database asynchronously (simple insert, ~1-5ms)
    let session_repo = state.session_repo.clone();
    tokio::spawn(async move {
        if let Err(e) = session_repo
            .create_session_with_id(session_id, None, None)
            .await
        {
            tracing::error!("Failed to persist session {}: {}", session_id, e);
        }
    });

    // Build decision packet for client group selection if no client is set
    let decision = if final_state == SessionState::New {
        build_client_group_decision(&state.pool, session_id).await
    } else {
        None
    };

    // Store the decision as pending on the session so replies are handled.
    // The V2 REPL session owns conversation history via `create_session_with_id`
    // (which seeds the welcome prompt) — do not also write it onto the V1
    // UnifiedSession.
    if let Some(ref packet) = decision {
        let mut sessions = state.sessions.write().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            s.pending_decision = Some(packet.clone());
        }
    }

    let session_feedback = if let Some(ref orchestrator) = state.repl_v2_orchestrator {
        orchestrator
            .get_session(session_id)
            .await
            .map(|s| s.build_session_feedback(false))
            .and_then(|fb| serde_json::to_value(fb).ok())
    } else {
        None
    };

    let response = CreateSessionResponse {
        session_id,
        created_at,
        state: final_state.into(),
        welcome_message,
        decision,
        session_feedback,
    };
    tracing::info!(
        "Returning CreateSessionResponse: session_id={}, state={:?}, welcome_message={}",
        response.session_id,
        response.state,
        response.welcome_message
    );
    Ok(Json(response))
}

// ============================================================================
// Semantic OS Context
// ============================================================================

/// Response for GET /api/sem-os/context
#[derive(Debug, Clone, serde::Serialize)]
struct SemOsContextResponse {
    /// Snapshot counts by object type (e.g., {"attribute_def": 120, "verb_contract": 85})
    registry_stats: std::collections::HashMap<String, i64>,
    /// Recent changesets (last 10)
    recent_changesets: Vec<ChangesetSummary>,
    /// Current agent mode ("governed" or "research")
    agent_mode: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ChangesetSummary {
    id: Uuid,
    title: Option<String>,
    status: String,
    created_by: Option<String>,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
    entry_count: i64,
}

/// GET /api/sem-os/context — registry stats, recent changesets, agent mode
/// GET /api/control-plane/metrics
///
/// T7.2 (EOP-PLAN-CONTROLPLANE-001): aggregated read-only metrics over the
/// three `control_plane_*` tables (T2.7 shadow decisions, T4.2 envelopes,
/// T5.3 write attestations). Never gates or influences dispatch — this is
/// observability only, matching V&S §6.14's "per-gate rejection rates,
/// breach count" asks (exception ageing and replay success are omitted:
/// no exception-tracking table or replay job exists yet, T7.3/T7.4 not
/// attempted this tranche).
#[derive(Debug, serde::Serialize)]
struct ControlPlaneMetricsResponse {
    gate_outcomes: Vec<crate::agent::control_plane_metrics::GateOutcomeCount>,
    shadow_divergence: crate::agent::control_plane_metrics::ShadowDivergenceStats,
    shadow_divergence_rate: f64,
    write_attestation_breaches: crate::agent::control_plane_metrics::WriteAttestationBreachStats,
    write_attestation_breach_rate: f64,
    envelope_status_counts: Vec<crate::agent::control_plane_metrics::EnvelopeStatusCount>,
}

async fn get_control_plane_metrics(
    State(state): State<AgentState>,
) -> Result<Json<ControlPlaneMetricsResponse>, StatusCode> {
    let gate_outcomes = crate::agent::control_plane_metrics::gate_outcome_counts(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "control-plane metrics: gate_outcome_counts query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let shadow_divergence =
        crate::agent::control_plane_metrics::shadow_divergence_stats(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "control-plane metrics: shadow_divergence_stats query failed");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    let write_attestation_breaches =
        crate::agent::control_plane_metrics::write_attestation_breach_stats(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "control-plane metrics: write_attestation_breach_stats query failed");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    let envelope_status_counts =
        crate::agent::control_plane_metrics::envelope_status_counts(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "control-plane metrics: envelope_status_counts query failed");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let shadow_divergence_rate = shadow_divergence.divergence_rate();
    let write_attestation_breach_rate = write_attestation_breaches.breach_rate();

    Ok(Json(ControlPlaneMetricsResponse {
        gate_outcomes,
        shadow_divergence,
        shadow_divergence_rate,
        write_attestation_breaches,
        write_attestation_breach_rate,
        envelope_status_counts,
    }))
}

async fn get_semos_context(
    State(state): State<AgentState>,
) -> Result<Json<SemOsContextResponse>, StatusCode> {
    // 1. Registry stats from sem_reg.v_registry_stats
    let stats_rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT object_type::text, count
        FROM sem_reg.v_registry_stats
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let registry_stats: std::collections::HashMap<String, i64> = stats_rows.into_iter().collect();

    // 2. Recent changesets (last 10)
    #[allow(clippy::type_complexity)]
    let changeset_rows: Vec<(Uuid, Option<String>, String, Option<String>, Option<chrono::DateTime<chrono::Utc>>, i64)> = sqlx::query_as(
        r#"
        SELECT
            c.id,
            c.title,
            c.status::text,
            c.created_by,
            c.created_at,
            COALESCE((SELECT count(*) FROM sem_reg.changeset_entries e WHERE e.change_set_id = c.id), 0) as entry_count
        FROM sem_reg.changesets c
        ORDER BY c.created_at DESC NULLS LAST
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let recent_changesets: Vec<ChangesetSummary> = changeset_rows
        .into_iter()
        .map(
            |(id, title, status, created_by, created_at, entry_count)| ChangesetSummary {
                id,
                title,
                status,
                created_by,
                created_at,
                entry_count,
            },
        )
        .collect();

    // 3. Agent mode — default is Governed
    let agent_mode = sem_os_types::agent_mode::AgentMode::default().to_string();

    Ok(Json(SemOsContextResponse {
        registry_stats,
        recent_changesets,
        agent_mode,
    }))
}

/// Build a Semantic OS workflow selection decision packet.
///
/// Presents 4 workflow choices that map to `stage_focus` values,
/// which thread through `goals` → `phase_tags` verb filtering.
fn build_semos_workflow_decision(session_id: Uuid) -> ob_poc_types::DecisionPacket {
    use ob_poc_types::{
        ClarificationPayload, DecisionKind, DecisionPacket, DecisionTrace, ScopeOption,
        ScopePayload, SessionStateView, UserChoice,
    };

    let choices = vec![
        UserChoice {
            id: "1".to_string(),
            label: "Onboarding".to_string(),
            description:
                "Define entity types, attributes, and verb contracts for client onboarding"
                    .to_string(),
            is_escape: false,
        },
        UserChoice {
            id: "2".to_string(),
            label: "KYC".to_string(),
            description: "Configure KYC workflows, evidence requirements, and screening rules"
                .to_string(),
            is_escape: false,
        },
        UserChoice {
            id: "3".to_string(),
            label: "Data Management".to_string(),
            description: "Manage taxonomies, policies, and data governance rules".to_string(),
            is_escape: false,
        },
        UserChoice {
            id: "4".to_string(),
            label: "Stewardship".to_string(),
            description: "Author and publish governed changes to the semantic registry".to_string(),
            is_escape: false,
        },
    ];

    let scope_options: Vec<ScopeOption> = choices
        .iter()
        .map(|c| ScopeOption {
            desc: format!("{}: {}", c.label, c.description),
            method: "workflow_selection".to_string(),
            score: 1.0,
            expect_count: None,
            sample: vec![],
            snapshot_id: None,
        })
        .collect();

    DecisionPacket {
        packet_id: Uuid::new_v4().to_string(),
        kind: DecisionKind::ClarifyScope,
        session: SessionStateView {
            session_id: Some(session_id),
            client_group_anchor: None,
            client_group_name: None,
            persona: None,
            last_confirmed_verb: None,
        },
        utterance: String::new(),
        payload: ClarificationPayload::Scope(ScopePayload {
            options: scope_options,
            context_hint: Some("Semantic OS workflow selection".to_string()),
        }),
        prompt: "Welcome to Semantic OS. What would you like to work on?".to_string(),
        choices,
        best_plan: None,
        alternatives: vec![],
        requires_confirm: false,
        confirm_token: None,
        trace: DecisionTrace {
            config_version: "1.0".to_string(),
            entity_snapshot_hash: None,
            lexicon_snapshot_hash: None,
            semantic_lane_enabled: false,
            embedding_model_id: None,
            verb_margin: 0.0,
            scope_margin: 0.0,
            kind_margin: 0.0,
            decision_reason: "semos_workflow_selection".to_string(),
        },
    }
}

/// Build a client group decision packet for new sessions.
async fn build_client_group_decision(
    pool: &sqlx::PgPool,
    session_id: Uuid,
) -> Option<ob_poc_types::DecisionPacket> {
    use crate::database::DealRepository;
    use ob_poc_types::{
        ClarificationPayload, DecisionKind, DecisionPacket, DecisionTrace,
        GroupClarificationPayload, GroupOption, SessionStateView, UserChoice,
    };

    let client_groups = match DealRepository::get_all_client_groups(pool).await {
        Ok(groups) => groups,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch client groups for session bootstrap");
            return None;
        }
    };

    if client_groups.is_empty() {
        return None;
    }

    let group_options: Vec<GroupOption> = client_groups
        .iter()
        .map(|g| GroupOption {
            id: g.id.to_string(),
            alias: g.canonical_name.clone(),
            score: 1.0,
            method: "list".to_string(),
        })
        .collect();

    let choices: Vec<UserChoice> = client_groups
        .iter()
        .enumerate()
        .map(|(i, g)| UserChoice {
            id: format!("{}", i + 1),
            label: g.canonical_name.clone(),
            description: format!("{} active deal(s)", g.deal_count),
            is_escape: false,
        })
        .collect();

    let prompt = "Which client would you like to work with today?".to_string();

    Some(DecisionPacket {
        packet_id: Uuid::new_v4().to_string(),
        kind: DecisionKind::ClarifyGroup,
        session: SessionStateView {
            session_id: Some(session_id),
            client_group_anchor: None,
            client_group_name: None,
            persona: None,
            last_confirmed_verb: None,
        },
        utterance: String::new(),
        payload: ClarificationPayload::Group(GroupClarificationPayload {
            options: group_options,
        }),
        prompt,
        choices,
        best_plan: None,
        alternatives: vec![],
        requires_confirm: false,
        confirm_token: None,
        trace: DecisionTrace {
            config_version: "1.0".to_string(),
            entity_snapshot_hash: None,
            lexicon_snapshot_hash: None,
            semantic_lane_enabled: false,
            embedding_model_id: None,
            verb_margin: 0.0,
            scope_margin: 0.0,
            kind_margin: 0.0,
            decision_reason: "session_bootstrap".to_string(),
        },
    })
}

/// Resolve initial client from client_id or client_name
async fn resolve_initial_client(
    pool: &sqlx::PgPool,
    client_ref: &crate::api::session::InitialClientRef,
) -> Result<crate::session::unified::ClientRef, String> {
    use crate::session::unified::ClientRef;

    // If client_id is provided, look it up directly
    if let Some(client_id) = client_ref.client_id {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT id, canonical_name
            FROM "ob-poc".client_group
            WHERE id = $1
            "#,
        )
        .bind(client_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        match row {
            Some((id, name)) => Ok(ClientRef {
                client_id: id,
                display_name: name,
            }),
            None => Err(format!("Client group not found: {}", client_id)),
        }
    }
    // If client_name is provided, search for it
    else if let Some(ref client_name) = client_ref.client_name {
        // Search by alias first (exact match)
        let row: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT cg.id, cg.canonical_name
            FROM "ob-poc".client_group cg
            JOIN "ob-poc".client_group_alias cga ON cg.id = cga.client_group_id
            WHERE LOWER(cga.alias) = LOWER($1)
            LIMIT 1
            "#,
        )
        .bind(client_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if let Some((id, name)) = row {
            return Ok(ClientRef {
                client_id: id,
                display_name: name,
            });
        }

        // Fallback: search by canonical name (case-insensitive)
        let row: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT id, canonical_name
            FROM "ob-poc".client_group
            WHERE LOWER(canonical_name) LIKE LOWER($1)
            LIMIT 1
            "#,
        )
        .bind(format!("%{}%", client_name))
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        match row {
            Some((id, name)) => Ok(ClientRef {
                client_id: id,
                display_name: name,
            }),
            None => Err(format!("Client not found: {}", client_name)),
        }
    } else {
        Err("Either client_id or client_name must be provided".to_string())
    }
}

/// GET /api/session/:id - Get session state (creates if not found)
async fn get_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Json<SessionStateResponse> {
    // Try to get existing session, or create a new one with the requested ID
    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id).cloned()
    };

    let session = match session {
        Some(s) => s,
        None => {
            // Create new session with the requested ID
            let mut new_session = UnifiedSession::new_for_entity(None, "cbu", None, None);
            new_session.id = session_id; // Use the requested ID
            let mut sessions = state.sessions.write().await;
            sessions.insert(session_id, new_session.clone());
            new_session
        }
    };

    // Conversation history is owned by the V2 REPL session — the V1 UnifiedSession
    // messages are no longer authoritative. Project V2 messages onto the wire type
    // here; fall back to V1 only if the V2 orchestrator has not seen the session
    // (e.g. external callers that bypassed POST /api/session).
    let messages: Vec<crate::api::session::ChatMessage> =
        if let Some(orchestrator) = state.repl_v2_orchestrator.as_ref() {
            orchestrator
                .get_session(session_id)
                .await
                .map(|s| s.messages.iter().map(v2_message_to_wire).collect())
                .unwrap_or_default()
        } else {
            session.messages.iter().cloned().map(|m| m.into()).collect()
        };

    Json(SessionStateResponse {
        session_id,
        entity_type: session.entity_type.clone(),
        entity_id: session.entity_id,
        state: session.state.clone().into(),
        message_count: messages.len(),
        combined_dsl: session.run_sheet.combined_dsl(),
        context: session.context.clone(),
        messages,
        can_execute: session.run_sheet.has_runnable(),
        version: Some(session.updated_at.to_rfc3339()),
        run_sheet: Some(session.run_sheet.to_api()),
        bindings: session
            .context
            .bindings
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    ob_poc_types::BoundEntityInfo {
                        id: v.id.to_string(),
                        name: v.display_name.clone(),
                        entity_type: v.entity_type.clone(),
                    },
                )
            })
            .collect(),
    })
}

/// Project a V2 REPL session message onto the V1 wire `ChatMessage` shape used
/// by `SessionStateResponse`.
fn v2_message_to_wire(
    msg: &crate::repl::session_v2::ChatMessage,
) -> crate::api::session::ChatMessage {
    use crate::api::session::MessageRole as WireRole;
    use crate::repl::session_v2::MessageRole as V2Role;

    crate::api::session::ChatMessage {
        id: msg.id,
        role: match msg.role {
            V2Role::User => WireRole::User,
            V2Role::Assistant => WireRole::Agent,
            V2Role::System => WireRole::System,
        },
        content: msg.content.clone(),
        timestamp: msg.timestamp,
        intents: None,
        dsl: None,
        sage_explain: None,
        drafter_proposal: None,
        discovery_bootstrap: None,
        parked_entries: None,
    }
}

/// DELETE /api/session/:id - Delete session (idempotent)
async fn delete_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> StatusCode {
    let mut sessions = state.sessions.write().await;
    sessions.remove(&session_id);
    StatusCode::NO_CONTENT
}

/// GET /api/session/:id/watch - Long-poll for session changes
///
/// This endpoint uses tokio::sync::watch channels to efficiently wait for
/// session updates. Clients can use this to react to changes made by
/// other consumers (MCP, REPL, other browser tabs).
///
/// ## Query Parameters
///
/// - `timeout_ms`: Maximum time to wait for changes (default 30000, max 60000)
///
/// ## Response
///
/// Returns a `WatchResponse` with:
/// - `is_initial: true` if this is the first call (returns current state immediately)
/// - `is_initial: false` if a change was detected within the timeout
///
/// If the timeout expires with no changes, returns the current state with `is_initial: false`.
///
/// ## Usage Pattern
///
/// ```javascript
/// async function watchSession(sessionId) {
///   while (true) {
///     const resp = await fetch(`/api/session/${sessionId}/watch?timeout_ms=30000`);
///     const data = await resp.json();
///     handleUpdate(data);
///   }
/// }
/// ```
async fn watch_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<WatchQuery>,
) -> Result<Json<WatchResponse>, StatusCode> {
    // Cap timeout at 60 seconds
    let timeout_ms = query.timeout_ms.min(60000);
    let timeout = std::time::Duration::from_millis(timeout_ms);

    // Subscribe to session changes
    let mut watcher = state
        .session_manager
        .subscribe(session_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get initial snapshot
    let initial_snapshot = watcher.borrow().clone();

    // Wait for a change or timeout
    let result = tokio::time::timeout(timeout, watcher.changed()).await;

    // Unsubscribe when done (cleanup)
    state.session_manager.unsubscribe(session_id).await;

    match result {
        Ok(Ok(())) => {
            // Change detected - return the new snapshot
            let snapshot = watcher.borrow();
            Ok(Json(WatchResponse::from_snapshot(&snapshot, false)))
        }
        Ok(Err(_)) => {
            // Watch channel closed (session was deleted)
            Err(StatusCode::GONE)
        }
        Err(_) => {
            // Timeout - return current state
            Ok(Json(WatchResponse::from_snapshot(&initial_snapshot, false)))
        }
    }
}

// ============================================================================
// Sub-Session Handlers
// ============================================================================

/// POST /api/session/:id/subsession - Create a sub-session
async fn create_subsession(
    State(state): State<AgentState>,
    Path(parent_id): Path<Uuid>,
    Json(req): Json<CreateSubSessionRequest>,
) -> Result<Json<CreateSubSessionResponse>, (StatusCode, String)> {
    tracing::info!("Creating sub-session for parent: {}", parent_id);

    // Get parent session
    let parent = {
        let sessions = state.sessions.read().await;
        sessions.get(&parent_id).cloned()
    };

    let parent = parent.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Parent session {} not found", parent_id),
        )
    })?;

    // Convert API type to internal type
    let sub_session_type = match req.session_type {
        CreateSubSessionType::Resolution {
            unresolved_refs,
            parent_dsl_index,
        } => SubSessionType::Resolution(ResolutionSubSession {
            unresolved_refs,
            parent_dsl_index,
            current_ref_index: 0,
            resolutions: std::collections::HashMap::new(),
        }),
        CreateSubSessionType::Research {
            target_entity_id,
            research_type,
        } => SubSessionType::Research(ResearchSubSession {
            target_entity_id,
            research_type,
            search_query: None,
        }),
        CreateSubSessionType::Review { pending_dsl } => SubSessionType::Review(ReviewSubSession {
            pending_dsl,
            review_status: ReviewStatus::Pending,
        }),
    };

    let session_type_name = match &sub_session_type {
        SubSessionType::Root => "root",
        SubSessionType::Resolution(_) => "resolution",
        SubSessionType::Research(_) => "research",
        SubSessionType::Review(_) => "review",
        SubSessionType::Correction(_) => "correction",
    }
    .to_string();

    // Create sub-session
    let child = UnifiedSession::new_subsession(&parent, sub_session_type);
    let child_id = child.id;
    let inherited_symbols: Vec<String> = child.inherited_symbols.keys().cloned().collect();

    // Store in memory
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(child_id, child);
    }

    tracing::info!(
        "Created sub-session {} (type: {})",
        child_id,
        session_type_name
    );

    Ok(Json(CreateSubSessionResponse {
        session_id: child_id,
        parent_id,
        inherited_symbols,
        session_type: session_type_name,
    }))
}

/// GET /api/session/:id/subsession/:child_id - Get sub-session state
async fn get_subsession(
    State(state): State<AgentState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SubSessionStateResponse>, (StatusCode, String)> {
    let sessions = state.sessions.read().await;

    let child = sessions.get(&child_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Sub-session {} not found", child_id),
        )
    })?;

    // Verify parent relationship
    if child.parent_session_id != Some(parent_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid parent-child relationship".to_string(),
        ));
    }

    Ok(Json(SubSessionStateResponse::from_session(child)))
}

/// POST /api/session/:id/subsession/:child_id/complete - Complete sub-session
async fn complete_subsession(
    State(state): State<AgentState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CompleteSubSessionRequest>,
) -> Result<Json<CompleteSubSessionResponse>, (StatusCode, String)> {
    tracing::info!("Completing sub-session: {} (apply={})", child_id, req.apply);

    // Get child session
    let child = {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&child_id)
    }
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Sub-session {} not found", child_id),
        )
    })?;

    if child.parent_session_id != Some(parent_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid parent-child relationship".to_string(),
        ));
    }

    // Extract resolution data if this is a Resolution sub-session
    let (resolutions_count, bound_entities) =
        if let SubSessionType::Resolution(r) = &child.sub_session_type {
            // Build BoundEntity entries from resolutions
            let mut entities = Vec::new();
            for unresolved in &r.unresolved_refs {
                if let Some(resolved_value) = r.resolutions.get(&unresolved.ref_id) {
                    // Find the matching entity info from initial_matches
                    let match_info = unresolved
                        .initial_matches
                        .iter()
                        .find(|m| &m.value == resolved_value);

                    if let Some(info) = match_info {
                        // Parse UUID from resolved value
                        if let Ok(uuid) = Uuid::parse_str(resolved_value) {
                            entities.push((
                                unresolved.ref_id.clone(),
                                crate::api::session::BoundEntity {
                                    id: uuid,
                                    entity_type: unresolved.entity_type.clone(),
                                    display_name: info.display.clone(),
                                },
                            ));
                        }
                    }
                }
            }
            (r.resolutions.len(), entities)
        } else {
            (0, Vec::new())
        };

    if req.apply && resolutions_count > 0 {
        // Apply resolutions to parent session's bindings
        let mut sessions = state.sessions.write().await;
        if let Some(parent) = sessions.get_mut(&parent_id) {
            for (ref_id, bound_entity) in &bound_entities {
                // Add to parent's bindings using the ref_id as the binding name
                parent
                    .context
                    .bindings
                    .insert(ref_id.clone(), bound_entity.clone());
                tracing::info!(
                    "Applied resolution: {} -> {} ({})",
                    ref_id,
                    bound_entity.id,
                    bound_entity.display_name
                );
            }
        }
    }

    Ok(Json(CompleteSubSessionResponse {
        success: true,
        resolutions_applied: if req.apply { resolutions_count } else { 0 },
        message: format!(
            "Sub-session completed. {} resolutions {}.",
            resolutions_count,
            if req.apply { "applied" } else { "discarded" }
        ),
    }))
}

/// POST /api/session/:id/subsession/:child_id/cancel - Cancel sub-session
async fn cancel_subsession(
    State(state): State<AgentState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CompleteSubSessionResponse>, (StatusCode, String)> {
    tracing::info!("Cancelling sub-session: {}", child_id);

    // Remove child session
    let child = {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&child_id)
    }
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Sub-session {} not found", child_id),
        )
    })?;

    if child.parent_session_id != Some(parent_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid parent-child relationship".to_string(),
        ));
    }

    Ok(Json(CompleteSubSessionResponse {
        success: true,
        resolutions_applied: 0,
        message: "Sub-session cancelled. No changes applied.".to_string(),
    }))
}

/// POST /api/session/:id/execute - legacy raw-DSL endpoint only.
async fn execute_session_dsl_legacy_raw_only(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Option<ExecuteDslRequest>>,
) -> Response {
    if !is_raw_execute_request(&req) {
        return (
            StatusCode::GONE,
            Json(serde_json::json!({
                "error": "Legacy execute endpoint disabled for normal session flows. Use POST /api/session/:id/input with kind=utterance and say 'run' to execute staged DSL."
            })),
        )
            .into_response();
    }

    match execute_session_dsl_raw(State(state), Path(session_id), headers, Json(req)).await {
        Ok(response) => response.into_response(),
        Err(status) => status.into_response(),
    }
}

/// POST /api/session/:id/execute - explicit raw DSL execution.
async fn execute_session_dsl_raw(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Option<ExecuteDslRequest>>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    tracing::debug!("[EXEC] Session {} - START execute_session_dsl", session_id);

    // Get or create execution context
    let (mut context, current_state, user_intent, constellation_family, constellation_map) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        tracing::debug!(
            "[EXEC] Session {} - Loading context, named_refs: {:?}",
            session_id,
            session.context.named_refs
        );

        // Extract user intent from last user message
        let user_intent = session
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Direct DSL execution".to_string());

        let (family, cmap) = if let Some(ref dom) = session.domain_hint {
            if let Some(ws) = crate::repl::types_v2::WorkspaceKind::from_hint(dom) {
                let entry = ws.registry_entry();
                (
                    Some(entry.default_constellation_family.to_string()),
                    Some(entry.default_constellation_map.to_string()),
                )
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        (
            session.context.clone(),
            session.state.clone(),
            user_intent,
            family,
            cmap,
        )
    };

    // Get DSL and check for pre-compiled plan in session
    // If DSL matches session.pending.source, use the pre-compiled plan
    // Otherwise run full pipeline (DSL was edited externally)
    let (dsl, precompiled_plan, cached_ast) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        // F16 fix (Slice 3.1, 2026-04-22): raw DSL bypass removed. Previously
        // gated by `PolicyGate::can_execute_raw_dsl` + `OBPOC_ALLOW_RAW_EXECUTE`
        // flag — both now deleted. Raw DSL in the request body is ALWAYS
        // rejected here: it would execute without reaching SemOS envelope
        // resolution, violating the "no bypassing sem-os" tollgate rule. The
        // run-sheet path (resolved DSL from a compiled runbook step that
        // already passed SemOS) remains the only execution route.
        let dsl_source = if let Some(ref req) = req {
            if req.dsl.is_some() {
                let actor = crate::api::policy_headers::actor_from_headers(&headers);
                tracing::warn!(
                    session = %session_id,
                    actor_id = %actor.actor_id,
                    "Raw DSL in request body rejected — raw-execute bypass removed in Slice 3.1. \
                     Route through ReplOrchestratorV2::process() instead."
                );
                return Err(StatusCode::FORBIDDEN);
            }
            session.run_sheet.runnable_dsl().unwrap_or_default()
        } else {
            session.run_sheet.runnable_dsl().unwrap_or_default()
        };

        // Note: UnifiedSession.RunSheetEntry doesn't store pre-compiled plan/ast
        // Always run full pipeline. In the future, we could add optional caching.
        let plan: Option<crate::dsl_v2::planning::ExecutionPlan> = None;
        let ast: Option<Vec<dsl_core::Statement>> = None;
        tracing::debug!("[EXEC] Running full pipeline (UnifiedSession doesn't cache plans)");

        (dsl_source, plan, ast)
    };

    if dsl.is_empty() {
        return Ok(Json(ExecuteResponse {
            success: false,
            results: Vec::new(),
            errors: vec!["No DSL to execute".to_string()],
            new_state: current_state.into(),
            bindings: None,
        }));
    }

    // =========================================================================
    // START GENERATION LOG
    // =========================================================================
    let log_id = state
        .generation_log
        .start_log(
            &user_intent,
            "session",
            Some(session_id),
            context.last_cbu_id,
            None,
            None, // intent_feedback_id — only populated via MCP dsl_execute path
        )
        .await
        .ok();

    let start_time = std::time::Instant::now();

    // Create execution context
    let mut exec_ctx = ExecutionContext::new().with_audit_user(&format!("session-{}", session_id));

    // Pre-bind symbols from session context
    if let Some(id) = context.last_cbu_id {
        exec_ctx.bind("last_cbu", id);
        tracing::debug!("[EXEC] Pre-bound last_cbu = {}", id);
    }
    if let Some(id) = context.last_entity_id {
        exec_ctx.bind("last_entity", id);
        tracing::debug!("[EXEC] Pre-bound last_entity = {}", id);
    }
    // Pre-bind all named references from previous executions
    tracing::debug!(
        "[EXEC] Pre-binding {} named_refs: {:?}",
        context.named_refs.len(),
        context.named_refs
    );
    for (name, id) in &context.named_refs {
        exec_ctx.bind(name, *id);
        tracing::debug!("[EXEC] Pre-bound @{} = {}", name, id);
    }

    // Pre-populate session's CBU scope for bulk operations
    // Verbs can access this via ctx.session_cbu_ids() or @session_cbus symbol
    if !context.cbu_ids.is_empty() {
        tracing::debug!(
            "[EXEC] Pre-populating session CBU scope: {} CBUs",
            context.cbu_ids.len()
        );
        exec_ctx.set_session_cbu_ids(context.cbu_ids.clone());
    }

    // =========================================================================
    // GET OR BUILD EXECUTION PLAN
    // If we have a pre-compiled plan (DSL unchanged), use it directly.
    // Otherwise run full pipeline: Parse → Validate → Compile
    // Returns (plan, ast_for_persistence) - AST needed for dsl_instances table
    // =========================================================================
    let (plan, ast_for_persistence): (
        crate::dsl_v2::execution_plan::ExecutionPlan,
        Option<Vec<crate::dsl_v2::Statement>>,
    ) = if let Some(cached_plan) = precompiled_plan {
        tracing::info!(
            "[EXEC] Using pre-compiled execution plan ({} steps)",
            cached_plan.len()
        );
        (cached_plan, cached_ast)
    } else {
        tracing::info!("[EXEC] Running full pipeline: parse → validate → compile");

        // PARSE DSL
        let program = match parse_program(&dsl) {
            Ok(p) => p,
            Err(e) => {
                let parse_error = format!("Parse error: {}", e);

                // Log failed attempt
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(dsl.clone()),
                        parse_result: ParseResult {
                            success: false,
                            error: Some(parse_error.clone()),
                        },
                        lint_result: LintResult {
                            valid: false,
                            errors: vec![],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: false,
                            error: None,
                            step_count: 0,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = state.generation_log.add_attempt(lid, &attempt).await;
                    let _ = state.generation_log.mark_failed(lid).await;
                }

                return Ok(Json(ExecuteResponse {
                    success: false,
                    results: Vec::new(),
                    errors: vec![parse_error],
                    new_state: current_state.into(),
                    bindings: None,
                }));
            }
        };

        // Sem OS verb validation: check all verb FQNs in parsed AST are allowed
        if let Some(ref client) = state.sem_os_client {
            use dsl_core::Statement;
            let actor = ob_poc_boundary::policy::ActorResolver::from_env();
            let envelope = crate::agent::orchestrator::resolve_allowed_verbs(
                client.as_ref(),
                &actor,
                Some(session_id),
                constellation_family,
                constellation_map,
            )
            .await;
            let phase2 = crate::traceability::Phase2Service::evaluate_from_envelope(envelope);
            match phase2.halt_reason_code {
                Some("sem_os_unavailable") => {
                    let policy_gate = ob_poc_boundary::policy::PolicyGate::from_env();
                    if policy_gate.semreg_fail_closed() {
                        tracing::warn!(
                            session = %session_id,
                            "execute_session_dsl: Sem OS unavailable — blocked in strict mode"
                        );
                        return Ok(Json(ExecuteResponse {
                            success: false,
                            results: Vec::new(),
                            errors: vec![
                                "Sem OS unavailable — execution blocked in strict mode".to_string()
                            ],
                            new_state: current_state.into(),
                            bindings: None,
                        }));
                    }
                }
                Some("no_allowed_verbs") => {
                    tracing::warn!(
                        session = %session_id,
                        "execute_session_dsl: Sem OS deny-all — blocking execution"
                    );
                    return Ok(Json(ExecuteResponse {
                        success: false,
                        results: Vec::new(),
                        errors: vec!["Sem OS denied execution: no verbs are allowed".to_string()],
                        new_state: current_state.into(),
                        bindings: None,
                    }));
                }
                _ => {}
            }

            {
                let denied_verbs = crate::traceability::Phase2Service::collect_denied_verbs(
                    &phase2.artifacts,
                    program.statements.iter().filter_map(|stmt| {
                        if let Statement::VerbCall(vc) = stmt {
                            Some(format!("{}.{}", vc.domain, vc.verb))
                        } else {
                            None
                        }
                    }),
                );
                if !denied_verbs.is_empty() {
                    tracing::warn!(
                        session = %session_id,
                        denied = ?denied_verbs,
                        "execute_session_dsl: Sem OS denied verbs"
                    );
                    return Ok(Json(ExecuteResponse {
                        success: false,
                        results: Vec::new(),
                        errors: vec![format!(
                            "Sem OS denied execution: verbs not in allowed set: {}",
                            denied_verbs.join(", ")
                        )],
                        new_state: current_state.into(),
                        bindings: None,
                    }));
                }
            }
        }

        // CSG VALIDATION (includes dataflow)
        tracing::debug!("[EXEC] Starting CSG validation");
        tracing::debug!(
            "[EXEC] Validation known_symbols: {:?}",
            context.named_refs.keys().collect::<Vec<_>>()
        );
        let validator_result = async {
            let v = SemanticValidator::new(state.pool.clone()).await?;
            v.with_csg_linter().await
        }
        .await;

        if let Ok(mut validator) = validator_result {
            use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};
            let request = ValidationRequest {
                source: dsl.clone(),
                context: ValidationContext::default()
                    .with_known_symbols(context.named_refs.clone()),
            };
            tracing::debug!(
                "[EXEC] ValidationRequest.context.known_symbols: {:?}",
                request.context.known_symbols
            );
            if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
                validator.validate(&request).await
            {
                tracing::debug!(
                    "[EXEC] Validation returned {} diagnostics",
                    diagnostics.len()
                );
                for diag in &diagnostics {
                    tracing::debug!("[EXEC] Diagnostic: [{:?}] {}", diag.severity, diag.message);
                }
                let csg_errors: Vec<String> = diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                    .collect();
                if !csg_errors.is_empty() {
                    // Log failed attempt
                    if let Some(lid) = log_id {
                        let attempt = GenerationAttempt {
                            attempt: 1,
                            timestamp: chrono::Utc::now(),
                            prompt_template: None,
                            prompt_text: String::new(),
                            raw_response: String::new(),
                            extracted_dsl: Some(dsl.clone()),
                            parse_result: ParseResult {
                                success: true,
                                error: None,
                            },
                            lint_result: LintResult {
                                valid: false,
                                errors: csg_errors.clone(),
                                warnings: vec![],
                            },
                            compile_result: CompileResult {
                                success: false,
                                error: None,
                                step_count: 0,
                            },
                            latency_ms: Some(start_time.elapsed().as_millis() as i32),
                            input_tokens: None,
                            output_tokens: None,
                        };
                        let _ = state.generation_log.add_attempt(lid, &attempt).await;
                        let _ = state.generation_log.mark_failed(lid).await;
                    }

                    return Ok(Json(ExecuteResponse {
                        success: false,
                        results: Vec::new(),
                        errors: csg_errors,
                        new_state: current_state.into(),
                        bindings: None,
                    }));
                }
            }
        }

        // COMPILE (includes DAG toposort)
        // Capture statements for AST persistence before consuming program
        let statements = program.statements.clone();
        match compile(&program) {
            Ok(p) => (p, Some(statements)),
            Err(e) => {
                let compile_error = format!("Compile error: {}", e);

                // Log failed attempt
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(dsl.clone()),
                        parse_result: ParseResult {
                            success: true,
                            error: None,
                        },
                        lint_result: LintResult {
                            valid: false,
                            errors: vec![compile_error.clone()],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: false,
                            error: Some(compile_error.clone()),
                            step_count: 0,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = state.generation_log.add_attempt(lid, &attempt).await;
                    let _ = state.generation_log.mark_failed(lid).await;
                }

                return Ok(Json(ExecuteResponse {
                    success: false,
                    results: Vec::new(),
                    errors: vec![compile_error],
                    new_state: current_state.into(),
                    bindings: None,
                }));
            }
        }
    };

    // =========================================================================
    // EXPANSION STAGE - Determine batch policy and derive locks
    // =========================================================================
    let templates = runtime_registry().templates();
    let expansion_result = expand_templates_simple(&dsl, templates);

    let expansion_report = match expansion_result {
        Ok(output) => {
            tracing::debug!(
                "[EXEC] Expansion complete: batch_policy={:?}, locks={}, statements={}",
                output.report.batch_policy,
                output.report.derived_lock_set.len(),
                output.report.expanded_statement_count
            );
            Some(output.report)
        }
        Err(e) => {
            tracing::warn!(
                "[EXEC] Expansion failed (continuing with best-effort): {}",
                e
            );
            None
        }
    };

    // Persist expansion report for audit trail (async, non-blocking)
    if let Some(ref report) = expansion_report {
        let expansion_audit = state.expansion_audit.clone();
        let report_clone = report.clone();
        tokio::spawn(async move {
            if let Err(e) = expansion_audit.save(session_id, &report_clone).await {
                tracing::error!(
                    session_id = %session_id,
                    expansion_id = %report_clone.expansion_id,
                    "Failed to persist expansion report: {}",
                    e
                );
            }
        });
    }

    // Determine batch policy from expansion report (default: BestEffort)
    let batch_policy = expansion_report
        .as_ref()
        .map(|r| r.batch_policy)
        .unwrap_or(BatchPolicy::BestEffort);

    // T9.3 (EOP-PLAN-CONTROLPLANE-001 Addendum B): admit every verb in the
    // plan before dispatch. This route constructs `state.dsl_v2_executor`
    // directly (`agent_state.rs`), bypassing the bus/runbook admission
    // checkpoints entirely — closes that gap.
    if let Err(e) = crate::agent::control_plane_envelope_store::admit_plan(&state.pool, &plan)
        .await
    {
        return Ok(Json(ExecuteResponse {
            success: false,
            results: Vec::new(),
            errors: vec![e],
            new_state: current_state.into(),
            bindings: None,
        }));
    }

    // =========================================================================
    // EXECUTE - Route based on batch policy
    // =========================================================================
    let mut results = Vec::new();
    let mut all_success = true;
    let mut errors = Vec::new();

    // Execute based on batch policy
    let execution_outcome = match batch_policy {
        BatchPolicy::Atomic => {
            tracing::info!(
                "[EXEC] Using atomic execution with locks (policy=atomic, locks={})",
                expansion_report
                    .as_ref()
                    .map(|r| r.derived_lock_set.len())
                    .unwrap_or(0)
            );
            state
                .dsl_v2_executor
                .execute_plan_atomic_with_locks(&plan, &mut exec_ctx, expansion_report.as_ref())
                .await
                .map(ExecutionOutcome::Atomic)
        }
        BatchPolicy::BestEffort => {
            tracing::info!("[EXEC] Using best-effort execution (policy=best_effort)");
            state
                .dsl_v2_executor
                .execute_plan_best_effort(&plan, &mut exec_ctx)
                .await
                .map(ExecutionOutcome::BestEffort)
        }
    };

    match execution_outcome {
        Ok(outcome) => {
            // Extract results based on outcome type
            let exec_results: Vec<DslV2Result> = match &outcome {
                ExecutionOutcome::Atomic(atomic) => match atomic {
                    AtomicExecutionResult::Committed { step_results, .. } => step_results.clone(),
                    AtomicExecutionResult::RolledBack {
                        failed_at_step,
                        error,
                        ..
                    } => {
                        all_success = false;
                        errors.push(format!(
                            "Atomic execution rolled back at step {}: {}",
                            failed_at_step, error
                        ));
                        Vec::new()
                    }
                    AtomicExecutionResult::LockContention {
                        entity_type,
                        entity_id,
                        ..
                    } => {
                        all_success = false;
                        errors.push(format!(
                            "Lock contention on {}:{} - another session is modifying this entity",
                            entity_type, entity_id
                        ));
                        Vec::new()
                    }
                    AtomicExecutionResult::IdempotentReplayReturned { prior_result } => {
                        prior_result.clone()
                    }
                    AtomicExecutionResult::OptimisticConflict { constraint_name } => {
                        all_success = false;
                        errors.push(format!(
                            "Optimistic conflict on constraint '{}' — retry with fresh read",
                            constraint_name
                        ));
                        Vec::new()
                    }
                    AtomicExecutionResult::TimedOut { stage, elapsed } => {
                        all_success = false;
                        errors.push(format!(
                            "Execution timed out at stage '{}' after {:.1?}",
                            stage, elapsed
                        ));
                        Vec::new()
                    }
                    AtomicExecutionResult::PanicRecovered { stage, panic_info } => {
                        all_success = false;
                        errors.push(format!("Panic at stage '{}': {}", stage, panic_info));
                        Vec::new()
                    }
                },
                ExecutionOutcome::BestEffort(best_effort) => {
                    // Check for partial failures
                    if !best_effort.errors.is_empty() {
                        all_success = false;
                        errors.push(best_effort.errors.summary());
                    }
                    // Convert Option<ExecutionResult> to ExecutionResult, filtering None
                    best_effort
                        .verb_results
                        .iter()
                        .filter_map(|r| r.clone())
                        .collect()
                }
            };

            for (idx, exec_result) in exec_results.iter().enumerate() {
                let mut entity_id: Option<Uuid> = None;
                let mut result_data: Option<serde_json::Value> = None;

                match exec_result {
                    DslV2Result::Uuid(uuid) => {
                        entity_id = Some(*uuid);

                        // Only set last_cbu_id if this was a cbu.* verb
                        if let Some(step) = plan.steps.get(idx) {
                            if step.verb_call.domain == "cbu" {
                                context.last_cbu_id = Some(*uuid);
                                context.cbu_ids.push(*uuid);
                                // Also add cbu_id alias so LLM can use @cbu_id
                                context.named_refs.insert("cbu_id".to_string(), *uuid);

                                // AUTO-PROMOTE: Set newly created CBU as active_cbu for session context
                                // This ensures subsequent operations use this CBU without explicit bind
                                let cbu_display_name = step
                                    .verb_call
                                    .arguments
                                    .iter()
                                    .find(|arg| arg.key == "name")
                                    .and_then(|arg| {
                                        if let crate::dsl_v2::ast::AstNode::Literal(
                                            crate::dsl_v2::ast::Literal::String(s),
                                            _,
                                        ) = &arg.value
                                        {
                                            Some(s.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_else(|| format!("CBU-{}", &uuid.to_string()[..8]));

                                tracing::info!(
                                    "[EXEC] Auto-promoting new CBU to active_cbu: {} ({})",
                                    cbu_display_name,
                                    uuid
                                );
                                context.set_active_cbu(*uuid, &cbu_display_name);
                            }
                            // Add entity_id alias for entity.* verbs
                            if step.verb_call.domain == "entity" {
                                context.named_refs.insert("entity_id".to_string(), *uuid);
                            }
                        }
                    }
                    DslV2Result::Record(json) => {
                        result_data = Some(json.clone());
                    }
                    DslV2Result::RecordSet(records) => {
                        result_data = Some(serde_json::Value::Array(records.clone()));
                    }
                    DslV2Result::Affected(_) | DslV2Result::Void => {
                        // No special handling needed
                    }
                    DslV2Result::EntityQuery(query_result) => {
                        // Entity query result for batch operations
                        result_data = Some(serde_json::json!({
                            "items": query_result.items.iter().map(|(id, name)| {
                                serde_json::json!({"id": id.to_string(), "name": name})
                            }).collect::<Vec<serde_json::Value>>(),
                            "entity_type": query_result.entity_type,
                            "total_count": query_result.total_count,
                        }));
                    }
                    DslV2Result::TemplateInvoked(invoke_result) => {
                        // Template invocation result
                        entity_id = invoke_result.primary_entity_id;
                        result_data = Some(serde_json::json!({
                            "template_id": invoke_result.template_id,
                            "statements_executed": invoke_result.statements_executed,
                            "outputs": invoke_result.outputs.iter().map(|(k, v)| {
                                (k.clone(), v.to_string())
                            }).collect::<std::collections::HashMap<String, String>>(),
                            "primary_entity_id": invoke_result.primary_entity_id.map(|id| id.to_string()),
                        }));
                    }
                    DslV2Result::TemplateBatch(batch_result) => {
                        // Template batch execution result
                        entity_id = batch_result.primary_entity_ids.first().copied();
                        result_data = Some(serde_json::json!({
                            "template_id": batch_result.template_id,
                            "total_items": batch_result.total_items,
                            "success_count": batch_result.success_count,
                            "failure_count": batch_result.failure_count,
                            "primary_entity_ids": batch_result.primary_entity_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                            "primary_entity_type": batch_result.primary_entity_type,
                            "aborted": batch_result.aborted,
                        }));
                    }
                    DslV2Result::BatchControl(control_result) => {
                        // Batch control operation result
                        result_data = Some(serde_json::json!({
                            "operation": control_result.operation,
                            "success": control_result.success,
                            "status": control_result.status,
                            "message": control_result.message,
                        }));
                    }
                }

                results.push(ExecutionResult {
                    statement_index: idx,
                    dsl: dsl.clone(),
                    success: true,
                    message: "Executed successfully".to_string(),
                    entity_id,
                    entity_type: None,
                    result: result_data,
                });
            }

            // =========================================================================
            // PERSIST SYMBOLS TO SESSION CONTEXT WITH TYPE INFO
            // =========================================================================
            tracing::debug!("[EXEC] Persisting symbols to session context");
            // Extract binding metadata from plan steps and store typed bindings
            for (idx, exec_result) in exec_results.iter().enumerate() {
                if let DslV2Result::Uuid(uuid) = exec_result {
                    if let Some(step) = plan.steps.get(idx) {
                        if let Some(ref binding_name) = step.bind_as {
                            // Get entity type from domain
                            let entity_type = step.verb_call.domain.clone();

                            // Extract display name from arguments (look for :name param)
                            let display_name = step
                                .verb_call
                                .arguments
                                .iter()
                                .find(|arg| {
                                    arg.key == "name"
                                        || arg.key == "cbu-name"
                                        || arg.key == "first-name"
                                })
                                .and_then(|arg| {
                                    if let crate::dsl_v2::ast::AstNode::Literal(
                                        crate::dsl_v2::ast::Literal::String(s),
                                        _,
                                    ) = &arg.value
                                    {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_else(|| binding_name.clone());

                            // Store typed binding
                            tracing::debug!(
                                "[EXEC] set_binding: @{} = {} (type: {}, display: {})",
                                binding_name,
                                uuid,
                                entity_type,
                                display_name
                            );
                            context.set_binding(binding_name, *uuid, &entity_type, &display_name);

                            // Update primary domain keys
                            context.update_primary_key(&entity_type, binding_name, *uuid);
                        }
                    }
                }
            }

            // Also copy raw symbols for backward compatibility
            tracing::debug!(
                "[EXEC] Copying {} exec_ctx.symbols to named_refs: {:?}",
                exec_ctx.symbols.len(),
                exec_ctx.symbols
            );
            for (name, id) in &exec_ctx.symbols {
                context.named_refs.insert(name.clone(), *id);
                tracing::debug!("[EXEC] named_refs.insert(@{} = {})", name, id);
            }
            tracing::debug!(
                "[EXEC] After symbol persist, context.named_refs: {:?}",
                context.named_refs
            );

            // =========================================================================
            // PROPAGATE VIEW STATE FROM EXECUTION CONTEXT
            // =========================================================================
            // View operations (view.universe, view.book, etc.) store ViewState in
            // ExecutionContext.pending_view_state. Propagate it to SessionContext
            // so the UI can access it. This fixes the "session state side door" where
            // ViewState was previously discarded because verb ops only receive
            // ExecutionContext, not the full session.
            if let Some(view_state) = exec_ctx.take_pending_view_state() {
                tracing::info!(
                    "[EXEC] Propagating ViewState to session context (node_type: {:?}, label: {})",
                    view_state.taxonomy.node_type,
                    view_state.taxonomy.label
                );
                context.set_view_state(view_state);
            }

            // =========================================================================
            // PROPAGATE VIEWPORT STATE FROM EXECUTION CONTEXT
            // =========================================================================
            // Viewport operations (viewport.focus, viewport.enhance, etc.) store ViewportState
            // in ExecutionContext.pending_viewport_state. Propagate it to SessionContext
            // so the UI can access it for CBU-focused navigation.
            if let Some(viewport_state) = exec_ctx.take_pending_viewport_state() {
                tracing::info!(
                    "[EXEC] Propagating ViewportState to session context (view_type: {:?}, focus_mode: {:?})",
                    viewport_state.view_type,
                    viewport_state.focus.focus_mode
                );
                context.set_viewport_state(viewport_state);
            }

            // =========================================================================
            // PROPAGATE SCOPE CHANGE FROM EXECUTION CONTEXT
            // =========================================================================
            // Session scope operations (session.set-galaxy, session.set-cbu, etc.) store
            // GraphScope in ExecutionContext.pending_scope_change. Propagate it to
            // SessionContext so the UI can rebuild the viewport with new scope.
            if let Some(scope_change) = exec_ctx.take_pending_scope_change() {
                tracing::info!(
                    "[EXEC] Propagating scope change to session context (scope: {:?})",
                    scope_change
                );
                context.set_scope(crate::session::SessionScope::from_graph_scope(scope_change));
            }

            // =========================================================================
            // PROPAGATE UNIFIED SESSION FROM EXECUTION CONTEXT
            // =========================================================================
            // Session load operations (session.load-galaxy, session.load-cbu, etc.) store
            // loaded CBU IDs in ExecutionContext.pending_session. Sync these to
            // SessionContext.cbu_ids so the scope-graph endpoint can build the multi-CBU view.
            if let Some(unified_session) = exec_ctx.take_pending_session() {
                let loaded_cbu_ids = unified_session.cbu_ids_vec();
                let cbu_count = loaded_cbu_ids.len();
                tracing::info!(
                    "[EXEC] Propagating {} CBU IDs from UnifiedSession to context.cbu_ids",
                    cbu_count
                );
                // Merge loaded CBUs into context (avoid duplicates)
                for cbu_id in &loaded_cbu_ids {
                    if !context.cbu_ids.contains(cbu_id) {
                        context.cbu_ids.push(*cbu_id);
                    }
                }

                // Set scope definition so UI knows to trigger scope_graph refetch
                // Use Custom scope for client-scoped loads, Book for single CBU
                let scope_def = if cbu_count == 1 {
                    crate::graph::GraphScope::SingleCbu {
                        cbu_id: loaded_cbu_ids[0],
                        cbu_name: unified_session.name.clone().unwrap_or_default(),
                    }
                } else {
                    // Multi-CBU scope - use Custom with session name or description
                    crate::graph::GraphScope::Custom {
                        description: unified_session
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("{} CBUs", cbu_count)),
                    }
                };

                context.scope = Some(crate::session::SessionScope {
                    definition: scope_def,
                    stats: crate::session::ScopeSummary {
                        total_cbus: cbu_count,
                        ..Default::default()
                    },
                    load_status: crate::session::LoadStatus::Full,
                });
                tracing::info!(
                    "[EXEC] Set context.scope with {} CBUs, scope_type={:?}",
                    cbu_count,
                    context.scope.as_ref().map(|s| &s.definition)
                );
            }

            // =========================================================================
            // CAPTURE DSL DIFF FOR LEARNING (proposed vs final)
            // =========================================================================
            let dsl_diff = state
                .session_manager
                .capture_dsl_diff(session_id, &dsl)
                .await;
            if let Some(ref diff) = dsl_diff {
                if diff.was_edited {
                    tracing::info!(
                        "[EXEC] DSL was edited by user: {} edit(s) detected",
                        diff.edits.len()
                    );
                    for edit in &diff.edits {
                        tracing::debug!(
                            "[EXEC] Edit: {} changed from '{}' to '{}'",
                            edit.field,
                            edit.from,
                            edit.to
                        );
                    }
                }
            }

            // =========================================================================
            // LOG SUCCESS
            // =========================================================================
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult {
                        success: true,
                        error: None,
                    },
                    lint_result: LintResult {
                        valid: true,
                        errors: vec![],
                        warnings: vec![],
                    },
                    compile_result: CompileResult {
                        success: true,
                        error: None,
                        step_count: plan.len() as i32,
                    },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_success(lid, &dsl, None).await;
                // Record execution outcome for learning loop
                let _ = state
                    .generation_log
                    .record_execution_outcome(lid, ExecutionStatus::Executed, None, None)
                    .await;
            }
        }
        Err(e) => {
            all_success = false;
            let error_msg = format!("Execution error: {}", e);
            errors.push(error_msg.clone());

            // Log execution failure
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult {
                        success: true,
                        error: None,
                    },
                    lint_result: LintResult {
                        valid: true,
                        errors: vec![],
                        warnings: vec![],
                    },
                    compile_result: CompileResult {
                        success: true,
                        error: None,
                        step_count: plan.len() as i32,
                    },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_failed(lid).await;
                // Record execution outcome for learning loop
                let _ = state
                    .generation_log
                    .record_execution_outcome(lid, ExecutionStatus::Failed, Some(&error_msg), None)
                    .await;
            }

            results.push(ExecutionResult {
                statement_index: 0,
                dsl: dsl.clone(),
                success: false,
                message: error_msg,
                entity_id: None,
                entity_type: None,
                result: None,
            });
        }
    }

    // Collect bindings to return to the client BEFORE moving context
    let bindings_map: std::collections::HashMap<String, uuid::Uuid> = context.named_refs.clone();

    // =========================================================================
    // PERSIST TO DATABASE (on success only)
    // =========================================================================
    if all_success {
        let session_repo = state.session_repo.clone();
        let dsl_repo = state.dsl_repo.clone();
        let dsl_clone = dsl.clone();
        let dsl_for_instance = dsl.clone();
        let bindings_clone = bindings_map.clone();
        let cbu_id = context.last_cbu_id;
        let domains = crate::database::extract_domains(&dsl_clone);
        let primary_domain = crate::database::detect_domain(&dsl_clone);
        let primary_domain_for_instance = primary_domain.clone();
        let execution_ms = start_time.elapsed().as_millis() as i32;

        // Serialize AST to JSON for dsl_instances persistence
        let ast_json = ast_for_persistence
            .as_ref()
            .and_then(|ast| serde_json::to_value(ast).ok());

        // Persist snapshot asynchronously (simple insert, ~1-5ms)
        tokio::spawn(async move {
            // Insert snapshot
            if let Err(e) = session_repo
                .save_snapshot(
                    session_id,
                    &dsl_clone,
                    &bindings_clone,
                    &[],
                    &domains,
                    Some(execution_ms),
                )
                .await
            {
                tracing::error!("Failed to save snapshot for session {}: {}", session_id, e);
            }

            // Update session bindings
            if let Err(e) = session_repo
                .update_bindings(
                    session_id,
                    &bindings_clone,
                    cbu_id,
                    None,
                    primary_domain.as_deref(),
                )
                .await
            {
                tracing::error!(
                    "Failed to update bindings for session {}: {}",
                    session_id,
                    e
                );
            }
        });

        // Persist DSL instance (confirmed DSL/AST pair) asynchronously
        let business_ref = format!("session-{}-{}", session_id, chrono::Utc::now().timestamp());
        let domain_name = primary_domain_for_instance.unwrap_or_else(|| "unknown".to_string());
        tokio::spawn(async move {
            match dsl_repo
                .save_execution(
                    &dsl_for_instance,
                    &domain_name,
                    &business_ref,
                    cbu_id,
                    &ast_json.unwrap_or(serde_json::Value::Null),
                )
                .await
            {
                Ok(result) => {
                    tracing::info!(
                        "Persisted DSL instance: id={}, version={}, ref={}",
                        result.instance_id,
                        result.version,
                        result.business_reference
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to persist DSL instance: {}", e);
                }
            }
        });
    } else {
        // Log error to session asynchronously
        let session_repo = state.session_repo.clone();
        let error_msg = errors.join("; ");
        let dsl_clone = dsl.clone();
        tokio::spawn(async move {
            let _ = session_repo.record_error(session_id, &error_msg).await;
            let _ = session_repo
                .log_event(
                    session_id,
                    crate::database::SessionEventType::ExecuteFailed,
                    Some(&dsl_clone),
                    Some(&error_msg),
                    None,
                )
                .await;
        });
    }

    // Update session
    let new_state = {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            // Only persist DSL to AST and file on successful execution
            if all_success {
                // Add executed statements to session AST
                if let Ok(program) = crate::dsl_v2::parse_program(&dsl) {
                    session.context.add_statements(program.statements);
                    tracing::info!(
                        "Added {} statements to session AST after successful execution",
                        session.context.statement_count()
                    );
                }

                // Write DSL to session file (only after successful execution)
                let file_manager = crate::api::dsl_session_file::DslSessionFileManager::new();
                let dsl_clone = dsl.clone();
                let description = format!("Executed: {} statement(s)", plan.len());
                tokio::spawn(async move {
                    if let Err(e) = file_manager
                        .append_dsl(session_id, &dsl_clone, &description)
                        .await
                    {
                        tracing::error!("Failed to write DSL to session file: {}", e);
                    }
                });
            }

            tracing::debug!(
                "[EXEC] Saving context to session, named_refs: {:?}",
                context.named_refs
            );

            // AUTO-UPDATE SESSION ENTITY_ID: If this is a "cbu" session and we just created
            // the first CBU, update the session's primary entity_id to match
            if session.entity_type == "cbu" && session.entity_id.is_none() {
                if let Some(active_cbu) = context.active_cbu.as_ref() {
                    let cbu_id = active_cbu.id;
                    tracing::info!(
                        "[EXEC] Auto-setting session.entity_id to newly created CBU: {}",
                        cbu_id
                    );
                    session.set_entity_id(cbu_id);
                }
            }

            session.context = context;
            tracing::debug!(
                "[EXEC] Session context after save, named_refs: {:?}",
                session.context.named_refs
            );

            // Update DAG state for executed verbs (Phase 5: context flows down)
            // This enables prereq-based verb readiness checking
            for step in &plan.steps {
                let verb_fqn = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
                crate::mcp::update_dag_after_execution(session, &verb_fqn);
            }
            tracing::debug!(
                "[EXEC] Updated DAG state with {} executed verbs, completed={:?}",
                plan.steps.len(),
                session.dag_state.completed
            );

            let session_results: Vec<crate::api::session::ExecutionResult> = results
                .iter()
                .map(|r| crate::api::session::ExecutionResult {
                    statement_index: r.statement_index,
                    dsl: r.dsl.clone(),
                    success: r.success,
                    message: r.message.clone(),
                    entity_id: r.entity_id,
                    entity_type: r.entity_type.clone(),
                    result: r.result.clone(),
                })
                .collect();
            session.record_execution(session_results);

            // Mark current run_sheet entry as executed with affected entities
            if let Some(entry) = session.run_sheet.current_mut() {
                entry.status = crate::session::EntryStatus::Executed;
                entry.executed_at = Some(chrono::Utc::now());
                // Collect all entity IDs affected by this execution
                entry.affected_entities = results.iter().filter_map(|r| r.entity_id).collect();
                // Note: UnifiedSession.RunSheetEntry doesn't have bindings field
                // Bindings are stored in session.bindings and session.context.bindings
            }

            session.state.clone()
        } else {
            SessionState::New
        }
    };

    // Notify watchers that session changed after execution
    state.session_manager.notify(session_id).await;

    Ok(Json(ExecuteResponse {
        success: all_success,
        results,
        errors,
        new_state: new_state.into(),
        bindings: if bindings_map.is_empty() {
            None
        } else {
            Some(bindings_map)
        },
    }))
}

/// POST /api/session/:id/clear - Clear/cancel pending DSL
async fn clear_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let (entity_type, entity_id, state_view, run_sheet, context, updated_at, bindings) = {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        // Cancel any pending/draft entries in run_sheet
        for entry in session.run_sheet.entries.iter_mut() {
            if entry.status == crate::session::EntryStatus::Draft {
                entry.status = crate::session::EntryStatus::Cancelled;
            }
        }

        (
            session.entity_type.clone(),
            session.entity_id,
            session.state.clone().into(),
            session.run_sheet.to_api(),
            session.context.clone(),
            session.updated_at,
            session
                .context
                .bindings
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        ob_poc_types::BoundEntityInfo {
                            id: v.id.to_string(),
                            name: v.display_name.clone(),
                            entity_type: v.entity_type.clone(),
                        },
                    )
                })
                .collect(),
        )
    };

    let messages: Vec<crate::api::session::ChatMessage> =
        if let Some(orchestrator) = state.repl_v2_orchestrator.as_ref() {
            orchestrator
                .get_session(session_id)
                .await
                .map(|s| s.messages.iter().map(v2_message_to_wire).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

    Ok(Json(SessionStateResponse {
        session_id,
        entity_type,
        entity_id,
        state: state_view,
        message_count: messages.len(),
        combined_dsl: None,
        context,
        messages,
        can_execute: false,
        version: Some(updated_at.to_rfc3339()),
        run_sheet: Some(run_sheet),
        bindings,
    }))
}

/// POST /api/session/:id/bind - Set a binding in the session context
///
/// This allows the UI to register external entities (like a selected CBU)
/// as available symbols for DSL generation and execution.
///
/// For CBU bindings, this also loads the current DSL version for optimistic locking.
/// The version is stored in `session.context.loaded_dsl_version` and used when
/// saving DSL to detect concurrent modifications.
async fn set_session_binding(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetBindingRequest>,
) -> Result<Json<SetBindingResponse>, StatusCode> {
    // For CBU bindings, load the current DSL version for optimistic locking
    let (loaded_version, business_ref) = if req.entity_type == "cbu" {
        // Use display_name as the business_reference (CBU name is the canonical key)
        let business_ref = req.display_name.clone();
        match state
            .dsl_repo
            .get_instance_by_reference(&business_ref)
            .await
        {
            Ok(Some(instance)) => {
                tracing::debug!(
                    "Loaded DSL version {} for CBU '{}' (optimistic locking)",
                    instance.current_version,
                    business_ref
                );
                (Some(instance.current_version), Some(business_ref))
            }
            Ok(None) => {
                tracing::debug!(
                    "No existing DSL instance for CBU '{}' (will create new)",
                    business_ref
                );
                (None, Some(business_ref))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load DSL version for CBU '{}': {} (continuing without lock)",
                    business_ref,
                    e
                );
                (None, Some(business_ref))
            }
        }
    } else {
        (None, None)
    };

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Set the typed binding (includes display name for LLM context)
    let actual_name =
        session
            .context
            .set_binding(&req.name, req.id, &req.entity_type, &req.display_name);

    // Also set common aliases for CBU
    if req.entity_type == "cbu" {
        session.context.named_refs.insert("cbu".to_string(), req.id);
        session
            .context
            .named_refs
            .insert("cbu_id".to_string(), req.id);
        session.context.set_active_cbu(req.id, &req.display_name);

        // Store version info for optimistic locking
        session.context.loaded_dsl_version = loaded_version;
        session.context.business_reference = business_ref;
    }

    tracing::debug!(
        "Bind success: session={} @{}={} type={} dsl_version={:?}",
        session_id,
        actual_name,
        req.id,
        req.entity_type,
        session.context.loaded_dsl_version
    );

    // Clone before dropping the lock to avoid holding it during response serialization
    // This fixes a deadlock that caused ERR_EMPTY_RESPONSE
    let bindings_clone = session.context.named_refs.clone();
    let actual_name_clone = actual_name.clone();
    drop(sessions);

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    Ok(Json(SetBindingResponse {
        success: true,
        binding_name: actual_name_clone,
        bindings: bindings_clone,
    }))
}

/// POST /api/session/:id/focus - Set stage focus for verb filtering
///
/// When a stage is focused, the agent will prioritize verbs relevant to that stage.
/// Pass an empty or null stage_code to clear the focus.
async fn set_session_focus(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetFocusRequest>,
) -> Result<Json<SetFocusResponse>, StatusCode> {
    // Normalize empty string to None
    let stage_code = req.stage_code.filter(|s| !s.is_empty());

    // Update session
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.context.stage_focus = stage_code.clone();
    }

    // Get stage info and relevant verbs if focused
    let (stage_name, relevant_verbs) = if let Some(ref code) = stage_code {
        // Load registry to get stage info
        // Use .ok() to avoid holding Box<dyn Error> across await
        let registry_opt = SemanticStageRegistry::load_default().ok();
        if let Some(registry) = registry_opt {
            if let Some(stage) = registry.get_stage(code) {
                let name = stage.name.clone();
                // Get relevant verbs from stage config (if defined)
                let verbs = stage.relevant_verbs.clone().unwrap_or_default();
                (Some(name), verbs)
            } else {
                (None, vec![])
            }
        } else {
            (None, vec![])
        }
    } else {
        (None, vec![])
    };

    tracing::debug!(
        "Focus set: session={} stage={:?} verbs={}",
        session_id,
        stage_code,
        relevant_verbs.len()
    );

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    Ok(Json(SetFocusResponse {
        success: true,
        stage_code,
        stage_name,
        relevant_verbs,
    }))
}

/// GET /api/session/:id/context - Get session context for agent and UI
///
/// Returns the session's context including:
/// - Active CBU with entity/role counts
/// - Linked KYC cases, ISDA agreements, products
/// - Trading profile if available
/// - Symbol table from DSL execution
///
/// This enables both the agent (for prompt context) and UI (for context panel)
/// to understand "where we are" in the workflow.
async fn get_session_context(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ob_poc_types::GetContextResponse>, StatusCode> {
    // Get session to find active CBU
    let active_cbu_id = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.context.active_cbu.as_ref().map(|cbu| cbu.id)
    };

    // If we have an active CBU, discover linked contexts from DB
    let context = if let Some(cbu_id) = active_cbu_id {
        let discovery_service = crate::database::ContextDiscoveryService::new(state.pool.clone());

        match discovery_service.discover_for_cbu(cbu_id).await {
            Ok(discovered) => {
                let mut ctx: ob_poc_types::SessionContext = discovered.into();

                // Merge in symbols and stage_focus from session
                let sessions = state.sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    // Convert session bindings to SymbolValue
                    for (name, binding) in &session.context.bindings {
                        ctx.symbols.insert(
                            name.clone(),
                            ob_poc_types::SymbolValue {
                                id: binding.id.to_string(),
                                entity_type: binding.entity_type.clone(),
                                display_name: binding.display_name.clone(),
                                source: Some("execution".to_string()),
                            },
                        );
                    }

                    // Set active scope if we have an active CBU
                    if let Some(cbu) = &ctx.cbu {
                        ctx.active_scope = Some(ob_poc_types::ActiveScope::Cbu {
                            cbu_id: cbu.id.clone(),
                            cbu_name: cbu.name.clone(),
                        });
                    }

                    // Copy stage focus from session
                    ctx.stage_focus = session.context.stage_focus.clone();

                    // Copy viewport state from session (set by viewport.* DSL verbs)
                    ctx.viewport_state = session.context.viewport_state.clone();
                }

                // Derive semantic state for the CBU (onboarding journey progress)
                // Note: We load the registry and immediately extract it to avoid
                // holding Box<dyn Error> across await points (not Send)
                let registry_opt = SemanticStageRegistry::load_default().ok();
                if let Some(registry) = registry_opt {
                    match derive_semantic_state(&state.pool, &registry, cbu_id).await {
                        Ok(semantic_state) => {
                            ctx.semantic_state = Some(semantic_state);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to derive semantic state for CBU {}: {}",
                                cbu_id,
                                e
                            );
                        }
                    }
                }

                ctx
            }
            Err(e) => {
                tracing::warn!("Context discovery failed for CBU {}: {}", cbu_id, e);
                ob_poc_types::SessionContext::default()
            }
        }
    } else {
        // No active CBU, return empty context with just symbols
        let sessions = state.sessions.read().await;
        let mut ctx = ob_poc_types::SessionContext::default();

        if let Some(session) = sessions.get(&session_id) {
            for (name, binding) in &session.context.bindings {
                ctx.symbols.insert(
                    name.clone(),
                    ob_poc_types::SymbolValue {
                        id: binding.id.to_string(),
                        entity_type: binding.entity_type.clone(),
                        display_name: binding.display_name.clone(),
                        source: Some("execution".to_string()),
                    },
                );
            }

            // Copy viewport state from session (set by viewport.* DSL verbs)
            ctx.viewport_state = session.context.viewport_state.clone();
        }

        ctx
    };

    Ok(Json(ob_poc_types::GetContextResponse { context }))
}

/// GET /api/session/:id/verb-surface - Get the current session's visible verb surface
///
/// Returns the `SessionVerbSurface` computed from the session's current context
/// (agent mode, workflow focus, Sem OS envelope, entity state). Supports optional
/// domain filtering and excluded verb inclusion.
///
/// Query parameters:
/// - `domain` (optional): Filter to a specific domain (e.g., "kyc")
/// - `include_excluded` (optional, default false): Include excluded verbs with prune reasons
async fn get_session_verb_surface(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<VerbSurfaceQuery>,
) -> Result<Json<ob_poc_types::chat::VerbSurfaceResponse>, StatusCode> {
    use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
    use crate::agent::verb_surface::{
        compute_session_verb_surface, VerbSurfaceContext, VerbSurfaceFailPolicy,
    };

    // Read session context
    let session = {
        let sessions = state.sessions.read().await;
        sessions
            .get(&session_id)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?
    };
    let agent_mode = sem_os_types::agent_mode::AgentMode::default();

    // Resolve real Sem OS verb set via the same path as the chat pipeline.
    // On failure, fall back to unavailable envelope with FailOpen (this is a
    // read-only UI population endpoint, not a governance enforcement point).
    let actor = crate::sem_reg::abac::ActorContext {
        actor_id: "verb-surface-api".to_string(),
        roles: vec!["viewer".to_string()],
        department: None,
        clearance: None,
        jurisdictions: vec![],
    };
    let (envelope, fail_policy) = match state.agent_service.resolve_options(&session, actor).await {
        Ok(env) => (env, VerbSurfaceFailPolicy::FailOpen),
        Err(e) => {
            tracing::warn!(
                "[get_session_verb_surface] Sem OS resolution failed, falling back: {e}"
            );
            (
                SemOsContextEnvelope::unavailable(),
                VerbSurfaceFailPolicy::default(),
            )
        }
    };
    let ctx = VerbSurfaceContext {
        agent_mode,
        stage_focus: session.context.stage_focus.as_deref(),
        envelope: &envelope,
        fail_policy,
        entity_state: None,
        has_group_scope: true,
        is_infrastructure_scope: false,
        composite_state: None,
    };
    let surface = compute_session_verb_surface(&ctx);

    // Build verb entries (optionally filtered by domain)
    let verbs: Vec<ob_poc_types::chat::VerbSurfaceEntry> = if let Some(ref domain) = query.domain {
        surface
            .verbs_for_domain(domain)
            .into_iter()
            .map(|v| ob_poc_types::chat::VerbSurfaceEntry {
                fqn: v.fqn.clone(),
                domain: v.domain.clone(),
                action: v.action.clone(),
                description: v.description.clone(),
                governance_tier: v.governance_tier.clone(),
                lifecycle_eligible: v.lifecycle_eligible,
                rank_boost: v.rank_boost,
            })
            .collect()
    } else {
        surface
            .verbs
            .iter()
            .map(|v| ob_poc_types::chat::VerbSurfaceEntry {
                fqn: v.fqn.clone(),
                domain: v.domain.clone(),
                action: v.action.clone(),
                description: v.description.clone(),
                governance_tier: v.governance_tier.clone(),
                lifecycle_eligible: v.lifecycle_eligible,
                rank_boost: v.rank_boost,
            })
            .collect()
    };

    // Build excluded list if requested
    let excluded = if query.include_excluded {
        Some(
            surface
                .excluded
                .iter()
                .map(|e| ob_poc_types::chat::VerbSurfaceExcludedEntry {
                    fqn: e.fqn.clone(),
                    reasons: e
                        .reasons
                        .iter()
                        .map(|r| ob_poc_types::chat::VerbSurfacePruneReason {
                            layer: format!("{:?}", r.layer),
                            reason: r.reason.clone(),
                        })
                        .collect(),
                })
                .collect(),
        )
    } else {
        None
    };

    Ok(Json(ob_poc_types::chat::VerbSurfaceResponse {
        final_count: verbs.len(),
        verbs,
        total_registry: surface.filter_summary.total_registry,
        surface_fingerprint: surface.surface_fingerprint.0.clone(),
        fail_policy: format!("{:?}", surface.fail_policy_applied),
        filter_summary: ob_poc_types::chat::VerbSurfaceFilterSummary {
            total_registry: surface.filter_summary.total_registry,
            after_agent_mode: surface.filter_summary.after_agent_mode,
            after_workflow: surface.filter_summary.after_workflow,
            after_semreg: surface.filter_summary.after_semreg,
            after_lifecycle: surface.filter_summary.after_lifecycle,
            final_count: surface.filter_summary.final_count,
        },
        excluded,
    }))
}

/// GET /api/session/:id/dsl/enrich - Get enriched DSL with binding info for display
///
/// Returns the session's DSL source with inline binding information:
/// - @symbols resolved to display names
/// - EntityRefs marked as resolved or needing resolution
/// - Binding summary for context panel
///
/// This is optimized for fast round-trips - no database or EntityGateway calls.
async fn get_enriched_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ob_poc_types::EnrichedDsl>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Get DSL source from session run sheet
    let dsl_source = session
        .run_sheet
        .combined_dsl()
        .unwrap_or_else(|| session.context.to_dsl_source());

    if dsl_source.trim().is_empty() {
        // Return empty enriched DSL
        return Ok(Json(ob_poc_types::EnrichedDsl {
            source: String::new(),
            segments: vec![],
            binding_summary: vec![],
            fully_resolved: true,
        }));
    }

    // Convert session bindings to enrichment format
    let bindings = crate::services::bindings_from_session_context(&session.context.bindings);

    // Get active CBU for summary
    let active_cbu = session
        .context
        .bindings
        .get("cbu")
        .map(|b| crate::services::BindingInfo {
            id: b.id,
            display_name: b.display_name.clone(),
            entity_type: b.entity_type.clone(),
        });

    // Enrich DSL
    match crate::services::enrich_dsl(&dsl_source, &bindings, active_cbu.as_ref()) {
        Ok(enriched) => Ok(Json(enriched)),
        Err(e) => {
            tracing::warn!("DSL enrichment failed: {}", e);
            // Return raw DSL on parse failure
            Ok(Json(ob_poc_types::EnrichedDsl {
                source: dsl_source,
                segments: vec![ob_poc_types::DslDisplaySegment::Text {
                    content: format!("Parse error: {}", e),
                }],
                binding_summary: vec![],
                fully_resolved: false,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey::router::PackRouter;
    use crate::sequencer::{NullDslExecutor, ReplOrchestratorV2};
    use std::sync::Arc;

    #[test]
    fn test_validate_dsl() {
        let valid_dsl = r#"(cbu.create :name "Test Fund" :jurisdiction "LU")"#;
        let result = parse_program(valid_dsl);
        assert!(result.is_ok());
    }

    // test_domain_list: removed — registry() was in deleted code

    #[test]
    fn test_execute_route_rejects_normal_session_flow_requests() {
        assert!(!is_raw_execute_request(&None));
        assert!(!is_raw_execute_request(&Some(ExecuteDslRequest {
            dsl: None
        })));
        assert!(!is_raw_execute_request(&Some(ExecuteDslRequest {
            dsl: Some("   ".to_string()),
        })));
        assert!(is_raw_execute_request(&Some(ExecuteDslRequest {
            dsl: Some("(registry.discover-dsl :utterance \"show me deal record\")".to_string()),
        })));
    }

    #[test]
    fn test_chat_ui_uses_unified_input_not_execute() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let chat_api_path = manifest_dir.join("../ob-poc-ui-react/src/api/chat.ts");
        let source = std::fs::read_to_string(&chat_api_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", chat_api_path.display(), e));
        let chat_page_path = manifest_dir.join("../ob-poc-ui-react/src/features/chat/ChatPage.tsx");
        let chat_page_source = std::fs::read_to_string(&chat_page_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", chat_page_path.display(), e));

        assert!(
            source.contains("`/session/${sessionId}/input`"),
            "chat UI must post utterances through the unified /input endpoint"
        );
        assert!(
            !source.contains("/execute"),
            "chat UI must not call /execute directly"
        );
        assert!(
            !chat_page_source.contains("sendAcpPrompt"),
            "user-facing chat page must not divert /acp commands around /input"
        );
        assert!(
            !chat_page_source.contains("isAcpPromptCommand"),
            "user-facing chat page must let the backend choose ACP routing"
        );
    }

    #[test]
    fn test_acp_session_input_draft_mode_parsing() {
        assert_eq!(
            "deterministic".parse::<AcpSessionInputDraftMode>(),
            Ok(AcpSessionInputDraftMode::Deterministic)
        );
        assert_eq!(
            "live_llm".parse::<AcpSessionInputDraftMode>(),
            Ok(AcpSessionInputDraftMode::LiveLlm)
        );
        assert!("random".parse::<AcpSessionInputDraftMode>().is_err());
    }

    #[test]
    fn test_routes_do_not_gate_session_behavior_on_semtaxonomy_flag() {
        let source = std::fs::read_to_string(file!()).expect("agent_routes source should read");
        let source = source
            .split("#[cfg(test)]")
            .next()
            .expect("agent_routes source should include pre-test content");
        assert!(
            !source.contains("semtaxonomy_enabled_for_routes"),
            "route-level SemTaxonomy feature gating should remain deleted"
        );
        assert!(
            !source.contains("SEMTAXONOMY_ENABLED"),
            "routes should not branch on SEMTAXONOMY_ENABLED"
        );
    }

    #[tokio::test]
    async fn test_supported_acp_prompt_routes_before_repl_on_normal_input() {
        let orchestrator = Arc::new(ReplOrchestratorV2::new(
            PackRouter::new(vec![]),
            Arc::new(NullDslExecutor),
        ));
        let session_id = orchestrator.create_session().await;

        let bundle = try_route_supported_acp_prompt(
            &orchestrator,
            session_id,
            "Advance deal 11111111-1111-1111-1111-111111111111 from PROSPECT to QUALIFYING with evidence sha256:evidence",
        )
        .await
        .expect("supported ACP prompt should route through ACP");

        assert!(bundle.message.contains("validated a dry-run workbook"));
        let trace =
            ob_poc_boundary::acp_dag_semantic::acp_chat_trace_summary_typed(&bundle.resolution);
        assert_eq!(trace["status"], "dry_run_validated");
        assert_eq!(trace["route"], "session_input");
        assert_eq!(trace["provider_task"], "deal.update-status");
        assert_eq!(trace["requested_draft_source"], "deterministic");
        assert_eq!(trace["draft_source"], "deterministic_provider");
        assert_eq!(trace["transition_ref"], "deal.prospect-to-qualifying");
        assert_eq!(trace["state_anchor_provider"]["task"], "deal.update-status");
        assert_eq!(
            trace["state_anchor_provider"]["language_pack_boundary"],
            "update_status_language_pack_v1"
        );
        assert_eq!(
            trace["state_anchor_provider"]["no_mutation_authority"],
            true
        );

        let session = orchestrator
            .get_session(session_id)
            .await
            .expect("session should still exist");
        assert!(session.messages.iter().any(|message| message
            .content
            .contains("Advance deal 11111111-1111-1111-1111-111111111111")));
        assert!(session
            .messages
            .iter()
            .any(|message| message.content.contains("validated a dry-run workbook")));
    }

    #[tokio::test]
    async fn test_live_llm_session_input_mode_is_task_bounded_for_deal_provider() {
        let orchestrator = Arc::new(ReplOrchestratorV2::new(
            PackRouter::new(vec![]),
            Arc::new(NullDslExecutor),
        ));
        let session_id = orchestrator.create_session().await;

        let bundle = try_route_supported_acp_prompt_with_draft_mode(
            &orchestrator,
            session_id,
            "Advance deal 11111111-1111-1111-1111-111111111111 from PROSPECT to QUALIFYING with evidence sha256:evidence",
            AcpSessionInputDraftMode::LiveLlm,
        )
        .await
        .expect("supported deal prompt should still route through ACP");

        let trace =
            ob_poc_boundary::acp_dag_semantic::acp_chat_trace_summary_typed(&bundle.resolution);
        assert_eq!(trace["status"], "dry_run_validated");
        assert_eq!(trace["provider_task"], "deal.update-status");
        assert_eq!(trace["requested_draft_source"], "llm_tool_call");
        assert_eq!(trace["draft_source"], "deterministic_provider");
        assert_eq!(trace["transition_ref"], "deal.prospect-to-qualifying");
    }

    #[tokio::test]
    async fn test_non_authored_prompt_still_falls_through_to_repl_input_path() {
        let orchestrator = Arc::new(ReplOrchestratorV2::new(
            PackRouter::new(vec![]),
            Arc::new(NullDslExecutor),
        ));
        let session_id = orchestrator.create_session().await;

        let chat =
            try_route_supported_acp_prompt(&orchestrator, session_id, "assemble context").await;

        assert!(
            chat.is_none(),
            "non-DAG control prompts should still fall through to the normal REPL path"
        );
    }
}

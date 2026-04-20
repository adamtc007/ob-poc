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
use crate::ontology::SemanticStageRegistry;
use ob_poc_types::{SessionInputRequest, SessionInputResponse};

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
        .route("/api/session/:id/chat", post(chat_session_legacy_blocked))
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
        // Unified decision reply (NEW - handles all clarification responses)
        .route(
            "/api/session/:id/decision/reply",
            post(decision_reply_legacy_blocked),
        )
        .with_state(state)
}

// ============================================================================
// Session Handlers
// ============================================================================

/// POST /api/session/:id/chat (legacy) — hard-blocked in unified-input cutover.
async fn chat_session_legacy_blocked() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::GONE,
        Json(serde_json::json!({
            "error": "Legacy endpoint removed. Use POST /api/session/:id/input with kind=utterance."
        })),
    )
}

/// POST /api/session/:id/decision/reply (legacy) — hard-blocked in unified-input cutover.
async fn decision_reply_legacy_blocked() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::GONE,
        Json(serde_json::json!({
            "error": "Legacy endpoint removed. Use POST /api/session/:id/input with kind=decision_reply."
        })),
    )
}

/// POST /api/session/:id/input - Unified session input endpoint.
async fn session_input(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    _headers: axum::http::HeaderMap,
    Json(req): Json<SessionInputRequest>,
) -> Result<Json<SessionInputResponse>, StatusCode> {
    // Try routing through REPL V2 orchestrator first (unified pipeline).
    if let Some(ref orchestrator) = state.repl_v2_orchestrator {
        if let Some(repl_response) = try_route_through_repl(&req, orchestrator, session_id).await {
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

/// Try to route input through the REPL V2 orchestrator.
///
/// Returns `Some(ReplResponseV2)` if the REPL session exists and is in a gate state
/// (ScopeGate, WorkspaceSelection, JourneySelection) or any later REPL state.
/// Returns `None` if no REPL session exists for this ID (legacy agent session).
async fn try_route_through_repl(
    req: &SessionInputRequest,
    orchestrator: &std::sync::Arc<crate::sequencer::ReplOrchestratorV2>,
    session_id: Uuid,
) -> Option<crate::repl::response_v2::ReplResponseV2> {
    use crate::repl::types_v2::UserInputV2;

    // Check if a REPL V2 session exists for this ID
    let session_exists = orchestrator.get_session(session_id).await.is_some();
    if !session_exists {
        return None; // No REPL session — fall through to legacy agent pipeline
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
                _ => return None, // Narrow/More/Reject — fall through to legacy
            }
        }
        _ => return None, // DiscoverySelection / ReplV2 — not handled here
    };

    match orchestrator.process(session_id, input).await {
        Ok(response) => Some(response),
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "REPL V2 orchestrator failed, falling through to legacy pipeline"
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

        // Store in memory
        {
            let mut sessions = state.sessions.write().await;
            session.add_agent_message(
                "Welcome to Semantic OS. What would you like to work on?".to_string(),
                None,
                None,
            );
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
        orchestrator.create_session_with_id(session_id).await;
        tracing::info!("REPL V2 session created for unified routing: {session_id}");
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

    // Store the decision as pending on the session so replies are handled
    if let Some(ref packet) = decision {
        let mut sessions = state.sessions.write().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            s.pending_decision = Some(packet.clone());
            s.add_agent_message(welcome_message.clone(), None, None);
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
    let agent_mode = sem_os_core::authoring::agent_mode::AgentMode::default().to_string();

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

    Json(SessionStateResponse {
        session_id,
        entity_type: session.entity_type.clone(),
        entity_id: session.entity_id,
        state: session.state.clone().into(),
        message_count: session.messages.len(),
        assembled_dsl: vec![], // Legacy - now in run_sheet
        combined_dsl: session.run_sheet.combined_dsl(),
        context: session.context.clone(),
        messages: session.messages.iter().cloned().map(|m| m.into()).collect(),
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

/// Generate help text showing all available MCP tools/commands.
///
/// Retained as a **fallback** for when Sem OS is unavailable.
/// The primary path is `generate_options_from_envelope()`.
#[allow(dead_code)]
fn generate_commands_help() -> String {
    r#"# Available Commands

## DSL Operations

**Commands:**
| Command | Description |
|---------|-------------|
| `dsl_validate` | Parse and validate DSL syntax/semantics |
| `dsl_execute` | Execute DSL against database |
| `dsl_plan` | Show execution plan without running |
| `dsl_generate` | Generate DSL from natural language |
| `dsl_lookup` | Look up database IDs (prevents hallucination) |
| `dsl_complete` | Get completions for verbs, domains, products |
| `dsl_signature` | Get verb signature with parameters |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "create a CBU for Acme Corp" | → `dsl_generate` → `(cbu.create :name "Acme Corp")` |
| "add John Smith as director" | → `dsl_generate` → `(cbu.assign-role ...)` |
| "what verbs are there for kyc?" | → `dsl_complete` or `/verbs kyc` |

**DSL Syntax:**
```clojure
(cbu.create :name "Acme Fund" :jurisdiction "LU" :as @cbu)
(entity.create :name "John Smith" :type PERSON :as @person)
(cbu.assign-role :cbu-id @cbu :entity-id @person :role DIRECTOR)
```

**Response:** `{"success": true, "bindings": {"@cbu": "uuid-..."}, "steps_executed": 1}`

---

## Session & Navigation

**Commands:**
| Command | Description |
|---------|-------------|
| `session_load_cbu` | Load single CBU into session |
| `session_load_jurisdiction` | Load all CBUs in jurisdiction |
| `session_load_galaxy` | Load all CBUs under apex entity |
| `session_unload_cbu` | Remove CBU from session |
| `session_clear` | Clear all CBUs |
| `session_undo` / `session_redo` | History navigation |
| `session_info` / `session_list` | Query session state |

**Natural Language:**
| Say | DSL |
|-----|-----|
| "load the Allianz book" | `(session.load-galaxy :apex-name "Allianz")` |
| "show me Luxembourg CBUs" | `(session.load-jurisdiction :jurisdiction "LU")` |
| "focus on Acme Fund" | `(session.load-cbu :cbu-name "Acme Fund")` |
| "clear the session" | `(session.clear)` |
| "undo" / "go back" | `(session.undo)` |

**Response:** `{"loaded": true, "cbu_count": 15, "cbu_ids": [...], "scope_size": 15}`

---

## CBU & Entity Operations

**Commands:**
| Command | Description |
|---------|-------------|
| `cbu_get` | Get CBU with entities, roles, documents |
| `cbu_list` | List/search CBUs |
| `entity_get` | Get entity details |
| `entity_search` | Smart search with disambiguation |

**Natural Language:**
| Say | DSL |
|-----|-----|
| "create a fund called Lux Alpha" | `(cbu.create :name "Lux Alpha" :type FUND)` |
| "add custody product" | `(cbu.add-product :product "Custody")` |
| "who are the directors?" | `(cbu.parties :cbu-id @cbu)` |
| "create person John Smith" | `(entity.create :name "John Smith" :type PERSON)` |
| "find BlackRock" | `(entity.search :query "BlackRock")` |

**Response:** `{"cbu": {"cbu_id": "...", "name": "Lux Alpha"}, "entities": [...], "roles": [...]}`

---

## View & Zoom

**Natural Language:**
| Say | Effect |
|-----|--------|
| "zoom in" / "closer" | Zoom in on current view |
| "zoom out" / "pull back" | Zoom out to wider view |
| "universe" / "show everything" | View all CBUs |
| "galaxy view" / "cluster" | View CBU groups |
| "land on" / "system view" | Focus single CBU |

**DSL Syntax:**
```clojure
(view.universe)
(view.book :apex-name "Allianz")
(view.cbu :cbu-id @cbu :mode trading)
```

---

## KYC & UBO

**Natural Language:**
| Say | DSL |
|-----|-----|
| "start KYC for this CBU" | `(kyc.start-case :cbu-id @cbu)` |
| "who owns this company?" | `(ubo.discover :entity-id @entity)` |
| "show ownership chain" | `(ubo.trace :entity-id @entity)` |
| "screen this person" | `(screening.run :entity-id @person)` |

**DSL Syntax:**
```clojure
(kyc.start-case :cbu-id @cbu :case-type ONBOARDING :as @case)
(kyc.add-document :case-id @case :doc-type PASSPORT :entity-id @person)
(ubo.discover :entity-id @entity :threshold 0.25)
(screening.run :entity-id @person :type PEP_SANCTIONS)
```

**Response:** `{"case_id": "...", "status": "IN_PROGRESS", "documents_required": [...]}`

---

## Trading Profile & Custody

**Natural Language:**
| Say | DSL |
|-----|-----|
| "set up trading for equities" | `(trading-profile.add-component :component-type "instrument-class" :class-code "EQUITY")` |
| "add XLON market" | `(trading-profile.add-component :component-type "market" :mic "XLON")` |
| "create SSI for USD" | `(custody.create-ssi :currency "USD" ...)` |
| "show trading matrix" | `trading_matrix_get` |

**DSL Syntax:**
```clojure
(trading-profile.create :cbu-id @cbu :as @profile)
(trading-profile.add-component :profile-id @profile :component-type "instrument-class" :class-code "EQUITY")
(trading-profile.add-component :profile-id @profile :component-type "market" :instrument-class "EQUITY" :mic "XLON")
(custody.create-ssi :cbu-id @cbu :currency "USD" :account "..." :as @ssi)
```

---

## Research Macros

**Commands:**
| Command | Description |
|---------|-------------|
| `research_list` | List available research macros |
| `research_execute` | Execute research (LLM + web) |
| `research_approve` / `research_reject` | Review results |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "research Allianz group structure" | Execute GLEIF hierarchy lookup |
| "find beneficial owners of Acme" | UBO research macro |
| "approve the research" | Accept results, get DSL |

**Response:** `{"results": {...}, "suggested_verbs": ["(entity.create ...)", "(ownership.link ...)"]}`

---

## Learning & Feedback

**Commands:**
| Command | Description |
|---------|-------------|
| `verb_search` | Search verbs by natural language |
| `intent_feedback` | Record correction for learning |
| `learning_analyze` | Find patterns to learn |
| `learning_apply` | Apply pattern→verb mapping |
| `embeddings_status` | Check semantic coverage |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "that should have been cbu.create" | Records correction → learning candidate |
| "block this verb for this phrase" | Adds to blocklist |

**Response:** `{"recorded": true, "occurrence_count": 3, "will_auto_apply_at": 5}`

---

## Promotion Pipeline (Quality-Gated Learning)

**Commands:**
| Command | Description |
|---------|-------------|
| `promotion_run_cycle` | Run full promotion pipeline |
| `promotion_candidates` | List auto-promotable candidates |
| `promotion_review_queue` | List manual review queue |
| `promotion_approve` | Approve candidate |
| `promotion_reject` | Reject and blocklist |
| `promotion_health` | Weekly health metrics |

**Thresholds:** 5+ occurrences, 80%+ success rate, 24h+ age, collision-safe

**Response:** `{"promoted": ["spin up a fund → cbu.create"], "skipped": 2, "collisions": 0}`

---

## Teaching (Direct Pattern Learning)

**Commands:**
| Command | Description |
|---------|-------------|
| `teach_phrase` | Add phrase→verb mapping |
| `unteach_phrase` | Remove mapping (with audit) |
| `teaching_status` | View taught patterns |

**Natural Language:**
| Say | Maps To |
|-----|---------|
| "teach: 'spin up a fund' = cbu.create" | `agent.teach` |
| "learn this phrase" / "remember this" | `agent.teach` |
| "forget this phrase" / "unteach" | `agent.unteach` |
| "what have I taught?" | `agent.read-teaching-status` |

**DSL Syntax:**
```clojure
(agent.teach :phrase "spin up a fund" :verb "cbu.create")
(agent.unteach :phrase "spin up a fund" :reason "too_generic")
(agent.read-teaching-status :limit 20)
```

**Response:**
```json
{"taught": true, "phrase": "spin up a fund", "verb": "cbu.create",
 "message": "Taught: 'spin up a fund' → cbu.create", "needs_reembed": true}
```

---

## Contracts & Onboarding

**Commands:**
| Command | Description |
|---------|-------------|
| `contract_create` | Create legal contract for client |
| `contract_get` | Get contract details |
| `contract_list` | List contracts (filter by client) |
| `contract_terminate` | Terminate contract |
| `contract_add_product` | Add product with rate card |
| `contract_subscribe` | Subscribe CBU to contract+product |
| `contract_can_onboard` | Check onboarding eligibility |

**Natural Language:**
| Say | DSL |
|-----|-----|
| "create contract for allianz" | `(contract.create :client "allianz" :effective-date "2024-01-01")` |
| "add custody to contract" | `(contract.add-product :contract-id @contract :product "CUSTODY")` |
| "subscribe CBU to custody" | `(contract.subscribe :cbu-id @cbu :contract-id @contract :product "CUSTODY")` |
| "can this CBU onboard to custody?" | `(contract.can-onboard :cbu-id @cbu :product "CUSTODY")` |
| "show allianz contract" | `(contract.for-client :client "allianz")` |
| "list subscriptions for client" | `(contract.list-subscriptions :client "allianz")` |

**Key Concept:** CBU onboarding requires contract+product subscription. No contract = no onboarding.

```
legal_contracts (client_label: "allianz")
    └── contract_products (product_code, rate_card_id)
         └── cbu_subscriptions (cbu_id) ← Onboarding gate
```

**DSL Syntax:**
```clojure
;; Create contract with products
(contract.create :client "allianz" :reference "MSA-2024-001" :effective-date "2024-01-01" :as @contract)
(contract.add-product :contract-id @contract :product "CUSTODY" :rate-card-id @rate)
(contract.add-product :contract-id @contract :product "FUND_ADMIN")

;; Subscribe CBU (onboarding)
(contract.subscribe :cbu-id @cbu :contract-id @contract :product "CUSTODY")

;; Check eligibility
(contract.can-onboard :cbu-id @cbu :product "CUSTODY")
```

**Response:** `{"contract_id": "...", "client_label": "allianz", "products": ["CUSTODY", "FUND_ADMIN"]}`

---

## Deals

**Commands:**
| Command | Description |
|---------|-------------|
| `deal_get` | Get deal details with products, rate cards, participants |
| `deal_list` | List deals (filter by client group) |
| `deal_create` | Create new deal for client |
| `deal_graph` | Get deal taxonomy graph for visualization |

**Natural Language:**
| Say | DSL |
|-----|-----|
| "show the Allianz deal" | `(session.load-deal :deal-name "Allianz")` |
| "load deal" | Prompts to select from available deals |
| "create a deal for Aviva" | `(deal.create :client-group "Aviva" :name "Aviva Custody 2024")` |
| "what deals are there?" | `(deal.list)` |
| "show deal products" | `(deal.list-products :deal-id @deal)` |
| "add product to deal" | `(deal.add-product :deal-id @deal :product-code "CUSTODY")` |

**Deal Hierarchy:**
```
Deal (root)
├── Products (commercial scope)
│   └── Rate Cards
│       └── Rate Card Lines
├── Participants (regional LEIs)
├── Contracts (legal agreements)
├── Onboarding Requests
│   └── CBU (if onboarded)
└── Billing Profiles
```

**DSL Syntax:**
```clojure
;; Load deal into session context
(session.load-deal :deal-name "Allianz Global Custody")

;; Create a new deal
(deal.create :client-group-id @client :name "Aviva Custody 2024" :as @deal)

;; Add products and rate cards
(deal.add-product :deal-id @deal :product-code "CUSTODY")
(deal.add-rate-card :deal-id @deal :product-code "CUSTODY" :effective-date "2024-01-01")

;; Get deal graph for visualization
(deal.graph :deal-id @deal :view-mode "COMMERCIAL")
```

**Response:** `{"deal_id": "...", "deal_name": "Allianz Global Custody", "products": [...], "participants": [...]}`

---

## Workflow & Templates

**Commands:**
| Command | Description |
|---------|-------------|
| `workflow_start` | Start workflow instance |
| `workflow_status` | Get status with blockers |
| `workflow_advance` | Advance (evaluates guards) |
| `template_list` | List templates |
| `template_expand` | Expand to DSL |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "start onboarding workflow" | Creates workflow instance |
| "what's blocking?" | Shows blockers |
| "advance the workflow" | Evaluates guards, moves state |

---

## Batch Operations

**Commands:** `batch_start`, `batch_add_entities`, `batch_confirm_keyset`,
`batch_set_scalar`, `batch_get_state`, `batch_expand_current`,
`batch_record_result`, `batch_skip_current`, `batch_cancel`

**Use case:** Apply same template to multiple entities (e.g., add role to 50 people)

---

*Type `/commands` or `/help` to see this list.*
*Type `/verbs` to see all DSL verbs, `/verbs <domain>` for specific domain.*
*Type `/verbs agent` for agent verbs, `/verbs session` for session verbs.*"#
        .to_string()
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
    let (mut context, current_state, user_intent) = {
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

        (session.context.clone(), session.state.clone(), user_intent)
    };

    // Get DSL and check for pre-compiled plan in session
    // If DSL matches session.pending.source, use the pre-compiled plan
    // Otherwise run full pipeline (DSL was edited externally)
    let (dsl, precompiled_plan, cached_ast) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        // Determine DSL source - prefer explicit request, then run_sheet current entry
        let dsl_source = if let Some(ref req) = req {
            if let Some(ref dsl_str) = req.dsl {
                // Gate: raw DSL requires server-side PolicyGate approval
                let actor = crate::policy::ActorResolver::from_headers(&headers);
                if !state.policy_gate.can_execute_raw_dsl(&actor) {
                    tracing::warn!(
                        session = %session_id,
                        actor_id = %actor.actor_id,
                        "Raw DSL execution blocked by PolicyGate"
                    );
                    return Err(StatusCode::FORBIDDEN);
                }
                tracing::warn!(session = %session_id, actor = %actor.actor_id, "Raw DSL execution via /execute (PolicyGate approved)");
                dsl_str.clone()
            } else {
                session.run_sheet.runnable_dsl().unwrap_or_default()
            }
        } else {
            session.run_sheet.runnable_dsl().unwrap_or_default()
        };

        // Note: UnifiedSession.RunSheetEntry doesn't store pre-compiled plan/ast
        // Always run full pipeline. In the future, we could add optional caching.
        let plan: Option<crate::dsl_v2::planning::ExecutionPlan> = None;
        let ast: Option<Vec<dsl_core::ast::Statement>> = None;
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
            let actor = crate::policy::ActorResolver::from_env();
            let envelope = crate::agent::orchestrator::resolve_allowed_verbs(
                client.as_ref(),
                &actor,
                Some(session_id),
            )
            .await;
            let phase2 = crate::traceability::Phase2Service::evaluate_from_envelope(envelope);
            match phase2.halt_reason_code {
                Some("sem_os_unavailable") => {
                    let policy_gate = crate::policy::PolicyGate::from_env();
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
            // ViewState was previously discarded because CustomOperation only receives
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
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Cancel any pending/draft entries in run_sheet
    for entry in session.run_sheet.entries.iter_mut() {
        if entry.status == crate::session::EntryStatus::Draft {
            entry.status = crate::session::EntryStatus::Cancelled;
        }
    }

    Ok(Json(SessionStateResponse {
        session_id,
        entity_type: session.entity_type.clone(),
        entity_id: session.entity_id,
        state: session.state.clone().into(),
        message_count: session.messages.len(),
        assembled_dsl: vec![],
        combined_dsl: None,
        context: session.context.clone(),
        messages: session.messages.iter().cloned().map(|m| m.into()).collect(),
        can_execute: false,
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
    let agent_mode = sem_os_core::authoring::agent_mode::AgentMode::default();

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
            after_actor: surface.filter_summary.after_actor,
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

        assert!(
            source.contains("`/session/${sessionId}/input`"),
            "chat UI must post utterances through the unified /input endpoint"
        );
        assert!(
            !source.contains("/execute"),
            "chat UI must not call /execute directly"
        );
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
}

//! REST API routes for DSL v2 execution
//!
//! Session endpoints:
//! - POST   /api/session             - Create new session
//! - GET    /api/session/:id         - Get session state
//! - DELETE /api/session/:id         - Delete session
//! - POST   /api/session/:id/execute - Execute DSL
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

use crate::api::agent_service::ChatRequest;
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
use crate::dsl_v2::{
    compile, expand_templates_simple, parse_program, runtime_registry, verb_registry::registry,
    AtomicExecutionResult, BatchPolicy, ExecutionContext, ExecutionResult as DslV2Result,
    SemanticValidator,
};
use crate::ontology::SemanticStageRegistry;
use ob_poc_types::{
    resolution::{
        DiscriminatorField as ApiDiscriminatorField, RefContext,
        ResolutionModeHint as ApiResolutionModeHint, ReviewRequirement,
        SearchKeyField as ApiSearchKeyField, SearchKeyFieldType,
        UnresolvedRefResponse as ApiUnresolvedRefResponse,
    },
    AstArgument, AstSpan, AstStatement, AstValue, BoundEntityInfo, ChatResponse, DslState,
    DslValidation, SessionStateEnum, ValidationError as ApiValidationError, VerbIntentInfo,
};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
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
    ReplEditRequest, ReplEditResponse, ReportCorrectionRequest, ReportCorrectionResponse,
    ResolutionState, ResolutionStats, ResolveByRefIdRequest, ResolveByRefIdResponse,
    ResolveRefRequest, ResolveRefResponse, SetBindingRequest, SetBindingResponse, SetFocusRequest,
    SetFocusResponse, SetViewModeRequest, SetViewModeResponse, SubSessionChatRequest,
    SubSessionMessage, SubSessionStateResponse, UnresolvedRef, ValidationError, ValidationResult,
    VerbInfo, VocabQuery, VocabResponse, WatchQuery, WatchResponse,
};

// ============================================================================
// Type Converters - Internal types to API types (ob_poc_types)
// ============================================================================

/// Convert internal SessionState to API SessionStateEnum (unified version)
fn to_session_state_enum(state: &SessionState) -> SessionStateEnum {
    match state {
        SessionState::New => SessionStateEnum::New,
        SessionState::Scoped => SessionStateEnum::Scoped,
        SessionState::PendingValidation => SessionStateEnum::PendingValidation,
        SessionState::ReadyToExecute => SessionStateEnum::ReadyToExecute,
        SessionState::Executing => SessionStateEnum::Executing,
        SessionState::Executed => SessionStateEnum::Executed,
        SessionState::Closed => SessionStateEnum::Executed, // Map closed to executed for API
    }
}

/// Convert api::session::SessionState to API SessionStateEnum (for backward compat)
fn api_session_state_to_enum(state: &crate::session::SessionState) -> SessionStateEnum {
    match state {
        crate::session::SessionState::New => SessionStateEnum::New,
        crate::session::SessionState::Scoped => SessionStateEnum::Scoped,
        crate::session::SessionState::PendingValidation => SessionStateEnum::PendingValidation,
        crate::session::SessionState::ReadyToExecute => SessionStateEnum::ReadyToExecute,
        crate::session::SessionState::Executing => SessionStateEnum::Executing,
        crate::session::SessionState::Executed => SessionStateEnum::Executed,
        crate::session::SessionState::Closed => SessionStateEnum::Executed,
    }
}

/// Convert dsl_core::Span to API AstSpan
fn to_api_span(span: &dsl_core::Span) -> AstSpan {
    AstSpan {
        start: span.start,
        end: span.end,
        start_line: None,
        end_line: None,
    }
}

/// Convert internal DisambiguationRequest to API DisambiguationRequest
/// Server uses Uuid, client uses String for JSON compatibility
fn to_api_disambiguation_request(
    req: &crate::api::session::DisambiguationRequest,
) -> ob_poc_types::DisambiguationRequest {
    ob_poc_types::DisambiguationRequest {
        request_id: req.request_id.to_string(),
        items: req.items.iter().map(to_api_disambiguation_item).collect(),
        prompt: req.prompt.clone(),
    }
}

/// Convert internal DisambiguationItem to API DisambiguationItem
fn to_api_disambiguation_item(
    item: &crate::api::session::DisambiguationItem,
) -> ob_poc_types::DisambiguationItem {
    match item {
        crate::api::session::DisambiguationItem::EntityMatch {
            param,
            search_text,
            matches,
            entity_type,
            search_column,
            ref_id,
        } => ob_poc_types::DisambiguationItem::EntityMatch {
            param: param.clone(),
            search_text: search_text.clone(),
            matches: matches.iter().map(to_api_entity_match).collect(),
            entity_type: entity_type.clone(),
            search_column: search_column.clone(),
            ref_id: ref_id.clone(),
        },
        crate::api::session::DisambiguationItem::InterpretationChoice { text, options } => {
            ob_poc_types::DisambiguationItem::InterpretationChoice {
                text: text.clone(),
                options: options
                    .iter()
                    .map(|opt| ob_poc_types::Interpretation {
                        id: opt.id.clone(),
                        label: opt.label.clone(),
                        description: opt.description.clone(),
                        effect: opt.effect.clone(),
                    })
                    .collect(),
            }
        }
        crate::api::session::DisambiguationItem::ClientGroupMatch {
            search_text,
            candidates,
        } => ob_poc_types::DisambiguationItem::ClientGroupMatch {
            search_text: search_text.clone(),
            candidates: candidates
                .iter()
                .map(|c| ob_poc_types::ClientGroupCandidate {
                    group_id: c.group_id.to_string(),
                    group_name: c.group_name.clone(),
                    matched_alias: c.matched_alias.clone(),
                    confidence: c.confidence,
                    entity_count: c.entity_count,
                })
                .collect(),
        },
    }
}

/// Convert internal EntityMatchOption to API EntityMatch
fn to_api_entity_match(opt: &crate::api::session::EntityMatchOption) -> ob_poc_types::EntityMatch {
    ob_poc_types::EntityMatch {
        entity_id: opt.entity_id.to_string(),
        name: opt.name.clone(),
        entity_type: opt.entity_type.clone(),
        jurisdiction: opt.jurisdiction.clone(),
        context: opt.context.clone(),
        score: opt.score.map(|s| s as f64),
    }
}

/// Convert internal dsl_core::Statement to API AstStatement
fn to_api_ast_statement(stmt: &dsl_core::Statement) -> AstStatement {
    match stmt {
        dsl_core::Statement::VerbCall(vc) => AstStatement::VerbCall {
            domain: vc.domain.clone(),
            verb: vc.verb.clone(),
            arguments: vc
                .arguments
                .iter()
                .map(|arg| AstArgument {
                    key: arg.key.clone(),
                    value: to_api_ast_value(&arg.value),
                    span: Some(to_api_span(&arg.span)),
                })
                .collect(),
            binding: vc.binding.clone(),
            span: Some(to_api_span(&vc.span)),
        },
        dsl_core::Statement::Comment(text) => AstStatement::Comment {
            text: text.clone(),
            span: None,
        },
    }
}

/// Convert internal dsl_core::AstNode to API AstValue
fn to_api_ast_value(node: &dsl_core::AstNode) -> AstValue {
    match node {
        dsl_core::AstNode::Literal(lit, _) => match lit {
            dsl_core::ast::Literal::String(s) => AstValue::String { value: s.clone() },
            dsl_core::ast::Literal::Integer(n) => AstValue::Number { value: *n as f64 },
            dsl_core::ast::Literal::Decimal(d) => {
                use rust_decimal::prelude::ToPrimitive;
                AstValue::Number {
                    value: d.to_f64().unwrap_or(0.0),
                }
            }
            dsl_core::ast::Literal::Boolean(b) => AstValue::Boolean { value: *b },
            dsl_core::ast::Literal::Null => AstValue::Null,
            dsl_core::ast::Literal::Uuid(u) => AstValue::String {
                value: u.to_string(),
            },
        },
        dsl_core::AstNode::SymbolRef { name, .. } => AstValue::SymbolRef { name: name.clone() },
        dsl_core::AstNode::EntityRef {
            entity_type,
            value,
            resolved_key,
            ..
        } => AstValue::EntityRef {
            entity_type: entity_type.clone(),
            search_key: value.clone(),
            resolved_key: resolved_key.clone(),
        },
        dsl_core::AstNode::List { items, .. } => AstValue::List {
            items: items.iter().map(to_api_ast_value).collect(),
        },
        dsl_core::AstNode::Map { entries, .. } => AstValue::Map {
            entries: entries
                .iter()
                .map(|(k, v)| ob_poc_types::AstMapEntry {
                    key: k.clone(),
                    value: to_api_ast_value(v),
                })
                .collect(),
        },
        dsl_core::AstNode::Nested(_vc) => {
            // Nested verb calls are complex - for now just represent as null
            // The UI doesn't typically need to display nested calls inline
            AstValue::Null
        }
    }
}

/// Convert internal VerbIntent to API VerbIntentInfo
fn to_api_verb_intent(intent: &crate::api::intent::VerbIntent) -> VerbIntentInfo {
    use ob_poc_types::ParamValue as ApiParamValue;

    // Split verb into domain.action
    let parts: Vec<&str> = intent.verb.splitn(2, '.').collect();
    let (domain, action) = if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        ("unknown".to_string(), intent.verb.clone())
    };

    // Convert params
    let params: std::collections::HashMap<String, ApiParamValue> = intent
        .params
        .iter()
        .map(|(k, v)| {
            let api_val = match v {
                crate::api::intent::ParamValue::String(s) => {
                    ApiParamValue::String { value: s.clone() }
                }
                crate::api::intent::ParamValue::Number(n) => ApiParamValue::Number { value: *n },
                crate::api::intent::ParamValue::Integer(n) => {
                    ApiParamValue::Number { value: *n as f64 }
                }
                crate::api::intent::ParamValue::Boolean(b) => ApiParamValue::Boolean { value: *b },
                crate::api::intent::ParamValue::Uuid(u) => ApiParamValue::String {
                    value: u.to_string(),
                },
                crate::api::intent::ParamValue::ResolvedEntity {
                    display_name,
                    resolved_id,
                } => ApiParamValue::ResolvedEntity {
                    display_name: display_name.clone(),
                    resolved_id: resolved_id.to_string(),
                    entity_type: "entity".to_string(), // Default, could be enhanced
                },
                crate::api::intent::ParamValue::List(items) => {
                    // For lists, just stringify for now
                    ApiParamValue::String {
                        value: format!("{:?}", items),
                    }
                }
                crate::api::intent::ParamValue::Object(obj) => {
                    // For objects, just stringify for now
                    ApiParamValue::String {
                        value: format!("{:?}", obj),
                    }
                }
            };
            (k.clone(), api_val)
        })
        .collect();

    VerbIntentInfo {
        verb: intent.verb.clone(),
        domain,
        action,
        params,
        bind_as: None, // VerbIntent doesn't have bind_as field
        validation: None,
    }
}

/// Build DslState from AgentChatResponse fields
fn build_dsl_state(
    dsl_source: Option<&String>,
    ast: Option<&Vec<dsl_core::Statement>>,
    can_execute: bool,
    intents: &[crate::api::intent::VerbIntent],
    validation_results: &[crate::api::intent::IntentValidation],
    bindings: &std::collections::HashMap<String, crate::api::session::BoundEntity>,
) -> Option<DslState> {
    // Only create DslState if there's actual DSL content
    if dsl_source.is_none() && ast.is_none() && intents.is_empty() {
        return None;
    }

    // Convert AST
    let api_ast = ast.map(|stmts| stmts.iter().map(to_api_ast_statement).collect());

    // Convert intents
    let api_intents = if intents.is_empty() {
        None
    } else {
        Some(intents.iter().map(to_api_verb_intent).collect())
    };

    // Build validation from validation_results
    let validation = if validation_results.is_empty() {
        None
    } else {
        let errors: Vec<ApiValidationError> = validation_results
            .iter()
            .filter(|v| !v.valid)
            .flat_map(|v| {
                // Convert each IntentError to ApiValidationError
                v.errors.iter().map(|err| ApiValidationError {
                    message: format!("[{}] {}", v.intent.verb, err.message),
                    line: None,
                    column: None,
                    suggestion: err.param.clone(), // Use param as suggestion context
                })
            })
            .collect();

        Some(DslValidation {
            valid: errors.is_empty(),
            errors,
            warnings: vec![],
        })
    };

    // Convert bindings (BoundEntity.display_name -> BoundEntityInfo.name)
    let api_bindings: std::collections::HashMap<String, BoundEntityInfo> = bindings
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                BoundEntityInfo {
                    id: v.id.to_string(),
                    name: v.display_name.clone(),
                    entity_type: v.entity_type.clone(),
                },
            )
        })
        .collect();

    Some(DslState {
        source: dsl_source.cloned(),
        ast: api_ast,
        can_execute,
        validation,
        intents: api_intents,
        bindings: api_bindings,
    })
}

/// Convert a list of UnresolvedRefInfo (unified) to API types
fn api_unresolved_refs_to_api(
    refs: &[crate::session::UnresolvedRefInfo],
) -> Vec<ApiUnresolvedRefResponse> {
    refs.iter().map(api_unresolved_ref_to_api).collect()
}

/// Convert unified::UnresolvedRefInfo to API response
fn api_unresolved_ref_to_api(info: &crate::session::UnresolvedRefInfo) -> ApiUnresolvedRefResponse {
    // Create a simple search key from the entity_type and search_value
    let search_keys: Vec<ApiSearchKeyField> = vec![ApiSearchKeyField {
        name: "name".to_string(),
        label: "Name".to_string(),
        is_default: true,
        field_type: SearchKeyFieldType::Text,
        enum_values: None,
    }];

    // No discriminator fields in simplified unified type
    let discriminator_fields: Vec<ApiDiscriminatorField> = vec![];

    // Default to picker/modal for resolution
    let resolution_mode = ApiResolutionModeHint::SearchModal;

    // Parse ref_id to get statement index (format: "stmt_idx:arg_name")
    let (stmt_idx, arg_name) = if let Some(colon_pos) = info.ref_id.find(':') {
        let stmt_str = &info.ref_id[..colon_pos];
        let arg = info.ref_id[colon_pos + 1..].to_string();
        (stmt_str.parse::<usize>().unwrap_or(0), arg)
    } else {
        (0, info.ref_id.clone())
    };

    // Build context from ref_id
    let context = RefContext {
        statement_index: stmt_idx,
        verb: String::new(), // Not available in simplified type
        arg_name,
        dsl_snippet: Some(info.context_line.clone()),
    };

    // Convert initial matches from unified EntityMatchInfo
    let initial_matches: Vec<ob_poc_types::resolution::EntityMatchResponse> = info
        .initial_matches
        .iter()
        .map(|m| ob_poc_types::resolution::EntityMatchResponse {
            id: m.value.clone(),
            display: m.display.clone(),
            entity_type: info.entity_type.clone(),
            score: (m.score_pct as f32) / 100.0,
            discriminators: std::collections::HashMap::new(),
            status: ob_poc_types::resolution::EntityStatus::Unknown,
            context: m.detail.clone(),
        })
        .collect();

    ApiUnresolvedRefResponse {
        ref_id: info.ref_id.clone(),
        entity_type: info.entity_type.clone(),
        entity_subtype: None,
        search_value: info.search_value.clone(),
        context,
        initial_matches,
        agent_suggestion: None,
        suggestion_reason: None,
        review_requirement: ReviewRequirement::Optional,
        search_keys,
        discriminator_fields,
        resolution_mode,
        return_key_type: Some("uuid".to_string()),
    }
}

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
        .route("/api/session/:id/chat", post(chat_session))
        .route("/api/session/:id/execute", post(execute_session_dsl))
        .route("/api/session/:id/repl-edit", post(repl_edit_session))
        .route("/api/session/:id/clear", post(clear_session_dsl))
        .route("/api/session/:id/bind", post(set_session_binding))
        .route("/api/session/:id/context", get(get_session_context))
        .route("/api/session/:id/focus", post(set_session_focus))
        .route("/api/session/:id/view-mode", post(set_session_view_mode))
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
        // DSL parsing and entity reference resolution
        .route(
            "/api/dsl/parse",
            post(crate::api::agent_dsl_routes::parse_dsl),
        )
        .route(
            "/api/dsl/resolve-ref",
            post(crate::api::agent_dsl_routes::resolve_entity_ref),
        )
        .route(
            "/api/dsl/resolve-by-ref-id",
            post(crate::api::agent_dsl_routes::resolve_by_ref_id),
        )
        // Discriminator parsing for resolution
        .route(
            "/api/resolution/parse-discriminators",
            post(crate::api::agent_dsl_routes::parse_discriminators),
        )
        // Vocabulary and metadata
        .route(
            "/api/agent/generate",
            post(crate::api::agent_dsl_routes::generate_dsl),
        )
        .route(
            "/api/agent/validate",
            post(crate::api::agent_dsl_routes::validate_dsl),
        )
        .route(
            "/api/agent/domains",
            get(crate::api::agent_dsl_routes::list_domains),
        )
        .route(
            "/api/agent/vocabulary",
            get(crate::api::agent_dsl_routes::get_vocabulary),
        )
        .route(
            "/api/agent/health",
            get(crate::api::agent_dsl_routes::health_check),
        )
        // Completions (LSP-style lookup via EntityGateway)
        .route(
            "/api/agent/complete",
            post(crate::api::agent_dsl_routes::complete_entity),
        )
        // Entity mention extraction (in-memory, no DB)
        .route(
            "/api/agent/extract-entities",
            post(crate::api::agent_dsl_routes::extract_entity_mentions),
        )
        // Onboarding
        .route(
            "/api/agent/onboard",
            post(crate::api::agent_dsl_routes::generate_onboarding_dsl),
        )
        // Enhanced generation with tool use
        .route(
            "/api/agent/generate-with-tools",
            post(crate::api::agent_dsl_routes::generate_dsl_with_tools),
        )
        // Batch operations (server-side DSL generation, no LLM)
        .route(
            "/api/batch/add-products",
            post(crate::api::agent_dsl_routes::batch_add_products),
        )
        // Learning/feedback (captures user corrections for continuous improvement)
        .route(
            "/api/agent/correction",
            post(crate::api::agent_learning_routes::report_correction),
        )
        // Verb disambiguation selection (closes the learning loop)
        .route(
            "/api/session/:id/select-verb",
            post(crate::api::agent_learning_routes::select_verb_disambiguation),
        )
        // Verb disambiguation abandonment (user bailed - all options were wrong)
        .route(
            "/api/session/:id/abandon-disambiguation",
            post(crate::api::agent_learning_routes::abandon_disambiguation),
        )
        // Unified decision reply (NEW - handles all clarification responses)
        .route(
            "/api/session/:id/decision/reply",
            post(crate::api::agent_learning_routes::handle_decision_reply),
        )
        .with_state(state)
}

// ============================================================================
// Session Handlers
// ============================================================================

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

    let mut session = UnifiedSession::new_for_entity(None, "cbu", None, req.domain_hint.clone());
    let session_id = session.id;
    let created_at = session.created_at;

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

    let response = CreateSessionResponse {
        session_id,
        created_at,
        state: final_state.into(),
        welcome_message,
    };
    tracing::info!(
        "Returning CreateSessionResponse: session_id={}, state={:?}, welcome_message={}",
        response.session_id,
        response.state,
        response.welcome_message
    );
    Ok(Json(response))
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
        pending_intents: vec![], // Legacy - now in run_sheet
        assembled_dsl: vec![],   // Legacy - now in run_sheet
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

/// DELETE /api/session/:id - Delete session
async fn delete_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut sessions = state.sessions.write().await;
    sessions.remove(&session_id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(StatusCode::NO_CONTENT)
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

/// Generate help text showing all available MCP tools/commands
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
| "who are the directors?" | `(cbu.list-roles :role DIRECTOR)` |
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
| "set up trading for equities" | `(trading-profile.add-instrument-class :class EQUITY)` |
| "add XLON market" | `(trading-profile.add-market :mic "XLON")` |
| "create SSI for USD" | `(custody.create-ssi :currency "USD" ...)` |
| "show trading matrix" | `trading_matrix_get` |

**DSL Syntax:**
```clojure
(trading-profile.create :cbu-id @cbu :as @profile)
(trading-profile.add-instrument-class :profile-id @profile :class EQUITY)
(trading-profile.add-market :profile-id @profile :mic "XLON")
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
| "what have I taught?" | `agent.teaching-status` |

**DSL Syntax:**
```clojure
(agent.teach :phrase "spin up a fund" :verb "cbu.create")
(agent.unteach :phrase "spin up a fund" :reason "too_generic")
(agent.teaching-status :limit 20)
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

/// Generate help text showing all available DSL verbs
///
/// This shows the same information the agent "sees" when generating DSL:
/// - Verb name and description
/// - Required and optional arguments with types
/// - Lookup info for entity resolution
fn generate_verbs_help(domain_filter: Option<&str>) -> String {
    let reg = registry();
    let mut output = String::new();

    // Group verbs by domain
    let mut domains: std::collections::BTreeMap<
        &str,
        Vec<&crate::dsl_v2::verb_registry::UnifiedVerbDef>,
    > = std::collections::BTreeMap::new();

    for verb in reg.all_verbs() {
        if let Some(filter) = domain_filter {
            if verb.domain != filter {
                continue;
            }
        }
        domains.entry(&verb.domain).or_default().push(verb);
    }

    if domains.is_empty() {
        if let Some(filter) = domain_filter {
            let available = reg.domains().join(", ");
            return format!(
                "Unknown domain: '{}'\n\n\
                 **Usage:** `/commands <domain>` or `/verbs <domain>`\n\n\
                 **Available domains:** {}\n\n\
                 **Examples:**\n\
                 - `/commands session` - session scope management\n\
                 - `/commands kyc` - KYC case verbs\n\
                 - `/commands entity` - entity management\n\
                 - `/verbs` - show all verbs",
                filter, available
            );
        }
        return "No verbs loaded in registry.".to_string();
    }

    // Header
    if let Some(filter) = domain_filter {
        output.push_str(&format!("# DSL Verbs: {} domain\n\n", filter));
    } else {
        output.push_str(&format!(
            "# DSL Verbs ({} verbs across {} domains)\n\n",
            reg.len(),
            reg.domains().len()
        ));
        output.push_str("*Use `/verbs <domain>` for detailed view of a specific domain.*\n\n");
        output.push_str("## Available Domains\n\n");
        for domain in reg.domains() {
            let count = domains.get(domain.as_str()).map(|v| v.len()).unwrap_or(0);
            output.push_str(&format!("- **{}** ({} verbs)\n", domain, count));
        }
        output.push_str("\n---\n\n");
    }

    // Verb details
    for (domain, verbs) in &domains {
        if domain_filter.is_none() {
            // Summary view - just list verbs with descriptions
            output.push_str(&format!("## {}\n\n", domain));
            for verb in verbs {
                output.push_str(&format!(
                    "- `{}.{}` - {}\n",
                    verb.domain, verb.verb, verb.description
                ));
            }
            output.push('\n');
        } else {
            // Detailed view - show full prompt helper info
            for verb in verbs {
                output.push_str(&format!("## `{}.{}`\n\n", verb.domain, verb.verb));
                output.push_str(&format!("{}\n\n", verb.description));

                // Arguments table
                if !verb.args.is_empty() {
                    output.push_str("### Arguments\n\n");
                    output.push_str("| Name | Type | Required | Description |\n");
                    output.push_str("|------|------|----------|-------------|\n");

                    for arg in &verb.args {
                        let req = if arg.required { "✓" } else { "" };
                        let desc = if arg.description.is_empty() {
                            "-".to_string()
                        } else {
                            arg.description.replace('\n', " ")
                        };
                        let type_info = if let Some(ref lookup) = arg.lookup {
                            if let Some(ref entity_type) = lookup.entity_type {
                                format!("{} (→{})", arg.arg_type, entity_type)
                            } else {
                                arg.arg_type.clone()
                            }
                        } else {
                            arg.arg_type.clone()
                        };
                        output.push_str(&format!(
                            "| `{}` | {} | {} | {} |\n",
                            arg.name, type_info, req, desc
                        ));
                    }
                    output.push('\n');
                }

                // Example DSL
                let required_args: Vec<_> = verb
                    .args
                    .iter()
                    .filter(|a| a.required)
                    .map(|a| format!(":{} ...", a.name))
                    .collect();
                let optional_args: Vec<_> = verb
                    .args
                    .iter()
                    .filter(|a| !a.required)
                    .take(2) // Show first 2 optional args
                    .map(|a| format!(":{} ...", a.name))
                    .collect();

                output.push_str("### Example\n\n");
                output.push_str("```clojure\n");
                if optional_args.is_empty() {
                    output.push_str(&format!(
                        "({}.{} {})\n",
                        verb.domain,
                        verb.verb,
                        required_args.join(" ")
                    ));
                } else {
                    output.push_str(&format!(
                        "({}.{} {} {})\n",
                        verb.domain,
                        verb.verb,
                        required_args.join(" "),
                        optional_args.join(" ")
                    ));
                }
                output.push_str("```\n\n");

                output.push_str("---\n\n");
            }
        }
    }

    output
}

/// POST /api/session/:id/chat - Process chat message and generate DSL via LLM
///
/// Pipeline: User message → Intent extraction (tool call) → DSL builder → Linter → Feedback loop
///
/// This mirrors the Claude Code workflow:
/// 1. Extract structured intents from natural language (tool call)
/// 2. Build DSL from intents (deterministic Rust code)
/// 3. Validate with CSG linter (same as Zed/LSP)
/// 4. If errors, feed back to agent and retry
/// 5. Write valid DSL to session file
///
/// Supports both Anthropic and OpenAI backends via AGENT_BACKEND env var.
///
/// This handler delegates to AgentService for the core chat logic:
/// - Intent extraction via LLM
/// - Entity resolution via EntityGateway
/// - DSL generation with validation/retry loop
///
/// ## Migration Note (REPL Redesign)
///
/// This endpoint is being replaced by the new REPL state machine at `/api/repl/*`.
/// The new architecture provides:
/// - Explicit state machine with clear transitions
/// - Single source of truth (command ledger)
/// - Pure intent matching service (no side effects)
/// - Replayable sessions from ledger
///
/// To use the new REPL API:
/// - `POST /api/repl/session` - Create session
/// - `POST /api/repl/session/:id/input` - Send any input (unified endpoint)
/// - `GET /api/repl/session/:id` - Get full session state
///
/// This endpoint remains for backwards compatibility during migration.
async fn chat_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    tracing::info!("=== CHAT SESSION START ===");
    tracing::info!("Session ID: {}", session_id);
    tracing::info!("Message: {:?}", req.message);
    tracing::info!("CBU ID: {:?}", req.cbu_id);

    // Get session (create if needed - matches get_session behavior)
    let session = {
        let sessions = state.sessions.read().await;
        tracing::info!("Looking up session in store...");
        match sessions.get(&session_id) {
            Some(s) => {
                tracing::info!("Session found, state: {:?}", s.state);
                s.clone()
            }
            None => {
                tracing::info!("Session not found, creating new session: {}", session_id);
                drop(sessions); // Release read lock before acquiring write lock
                let mut new_session = UnifiedSession::new_for_entity(None, "cbu", None, None);
                new_session.id = session_id; // Use the requested ID
                let mut sessions = state.sessions.write().await;
                sessions.insert(session_id, new_session.clone());
                tracing::info!("Session created: {}", session_id);
                new_session
            }
        }
    };

    // Handle slash commands: /commands, /verbs, /help with optional domain filter
    let trimmed_msg = req.message.trim().to_lowercase();
    let parts: Vec<&str> = trimmed_msg.split_whitespace().collect();

    match parts.as_slice() {
        // /help or /commands (no args) - show MCP tools overview
        ["/help"] | ["/commands"] | ["show", "commands"] => {
            return Ok(Json(ChatResponse {
                message: generate_commands_help(),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                intent_tier: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
                decision: None,
            }));
        }
        // /commands <domain> or /verbs <domain> - show verbs for domain
        ["/commands" | "/verbs", domain] => {
            return Ok(Json(ChatResponse {
                message: generate_verbs_help(Some(domain)),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                intent_tier: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
                decision: None,
            }));
        }
        // /verbs (no args) - show all verbs
        ["/verbs"] => {
            return Ok(Json(ChatResponse {
                message: generate_verbs_help(None),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                intent_tier: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
                decision: None,
            }));
        }
        _ => {} // Not a slash command, continue to LLM
    }

    // Make session mutable for the rest of the handler
    let mut session = session;

    // Create LLM client (uses AGENT_BACKEND env var to select provider)
    let llm_client = match crate::agentic::create_llm_client() {
        Ok(client) => client,
        Err(e) => {
            let error_msg = format!("Error: LLM client initialization failed: {}", e);
            return Ok(Json(ChatResponse {
                message: error_msg,
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                intent_tier: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
                decision: None,
            }));
        }
    };

    tracing::info!(
        "Chat session using {} ({})",
        llm_client.provider_name(),
        llm_client.model_name()
    );

    // Delegate to centralized AgentService (single pipeline)
    let response = match state
        .agent_service
        .process_chat(&mut session, &req, llm_client)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("AgentService error: {}", e);
            // Return error as JSON response instead of opaque 500
            return Ok(Json(ChatResponse {
                message: format!("Agent error: {}", e),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                intent_tier: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
                decision: None,
            }));
        }
    };

    // =========================================================================
    // CAPTURE FEEDBACK FOR LEARNING LOOP
    // Record the verb match so we can correlate with execution outcome later
    // =========================================================================
    if !response.intents.is_empty() {
        let first_intent = &response.intents[0];

        // Determine match method: DirectDsl if user typed DSL, Semantic if LLM generated
        let is_direct_dsl = req.message.trim().starts_with('(');
        let match_method = if is_direct_dsl {
            ob_semantic_matcher::MatchMethod::DirectDsl
        } else {
            ob_semantic_matcher::MatchMethod::Semantic
        };

        // Build a MatchResult from the intent for feedback capture
        let match_result = ob_semantic_matcher::MatchResult {
            verb_name: first_intent.verb.clone(),
            pattern_phrase: req.message.clone(), // The user's input
            similarity: 1.0,                     // Direct DSL or LLM-selected verb
            match_method,
            category: "chat".to_string(),
            is_agent_bound: true,
        };

        match state
            .feedback_service
            .capture_match(
                session_id,
                &req.message,
                ob_semantic_matcher::feedback::InputSource::Chat,
                Some(&match_result),
                &[], // No alternatives from LLM path
                session.context.domain_hint.as_deref(),
                session.context.stage_focus.as_deref(),
            )
            .await
        {
            Ok(interaction_id) => {
                // Store both interaction_id (for record_outcome) and feedback_id (for FK)
                session.context.pending_interaction_id = Some(interaction_id);

                // intent_feedback uses BIGSERIAL id, need to look it up by interaction_id
                if let Ok(Some(feedback_id)) = sqlx::query_scalar::<_, i64>(
                    r#"SELECT id FROM "ob-poc".intent_feedback WHERE interaction_id = $1"#,
                )
                .bind(interaction_id)
                .fetch_optional(&state.pool)
                .await
                {
                    session.context.pending_feedback_id = Some(feedback_id);
                    tracing::debug!(
                        "Captured feedback: interaction_id={}, feedback_id={}, method={:?}",
                        interaction_id,
                        feedback_id,
                        match_method
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to capture feedback: {}", e);
            }
        }
    }

    // =========================================================================
    // TRACK PROPOSED DSL FOR DIFF (learning from user edits)
    // =========================================================================
    if let Some(ref dsl_source) = response.dsl_source {
        // Only set proposed_dsl if agent generated it (not DirectDsl)
        let is_direct_dsl = req.message.trim().starts_with('(');
        if !is_direct_dsl {
            state
                .session_manager
                .set_proposed_dsl(session_id, dsl_source)
                .await;
        }
    }

    // Persist session changes back
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session.clone());
    }

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    // Build DslState from response fields
    let dsl_state = build_dsl_state(
        response.dsl_source.as_ref(),
        response.ast.as_ref(),
        response.can_execute,
        &response.intents,
        &response.validation_results,
        &session.context.bindings,
    );

    // =========================================================================
    // DEBUG PAYLOAD (OB_CHAT_DEBUG=1) - Verb matching explainability
    // TODO: Wire up when AgentChatResponse includes verb_candidates field
    // =========================================================================
    // let _debug_info: Option<ob_poc_types::ChatDebugInfo> = None;

    // Return response using API types (single source of truth)
    Ok(Json(ChatResponse {
        message: response.message,
        dsl: dsl_state,
        session_state: api_session_state_to_enum(&response.session_state),
        commands: response.commands,
        disambiguation_request: response
            .disambiguation
            .as_ref()
            .map(to_api_disambiguation_request),
        verb_disambiguation: response.verb_disambiguation,
        intent_tier: response.intent_tier,
        unresolved_refs: response
            .unresolved_refs
            .as_ref()
            .map(|refs| api_unresolved_refs_to_api(refs)),
        current_ref_index: response.current_ref_index,
        dsl_hash: response.dsl_hash,
        decision: response.decision,
    }))
}

/// POST /api/session/:id/execute - Execute DSL
async fn execute_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
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
                dsl_str.clone()
            } else {
                session.run_sheet.combined_dsl().unwrap_or_default()
            }
        } else {
            session.run_sheet.combined_dsl().unwrap_or_default()
        };

        // Note: UnifiedSession.RunSheetEntry doesn't store pre-compiled plan/ast
        // Always run full pipeline. In the future, we could add optional caching.
        let plan: Option<crate::dsl_v2::ExecutionPlan> = None;
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
    // Get feedback IDs from session context if available (set by chat handler)
    // =========================================================================
    let (intent_feedback_id, pending_interaction_id) = {
        let sessions = state.sessions.read().await;
        sessions
            .get(&session_id)
            .map(|s| {
                (
                    s.context.pending_feedback_id,
                    s.context.pending_interaction_id,
                )
            })
            .unwrap_or((None, None))
    };

    let log_id = state
        .generation_log
        .start_log(
            &user_intent,
            "session",
            Some(session_id),
            context.last_cbu_id,
            None,
            intent_feedback_id,
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

            // Update intent_feedback outcome with DSL diff
            if let Some(interaction_id) = pending_interaction_id {
                let elapsed_ms = start_time.elapsed().as_millis() as i32;

                // Convert DSL diff to feedback format
                let (generated_dsl, final_dsl_str, user_edits_json) =
                    if let Some(ref diff) = dsl_diff {
                        let edits_json = if diff.edits.is_empty() {
                            None
                        } else {
                            Some(serde_json::to_value(&diff.edits).unwrap_or_default())
                        };
                        (
                            Some(diff.proposed.clone()),
                            Some(diff.final_dsl.clone()),
                            edits_json,
                        )
                    } else {
                        (None, Some(dsl.clone()), None)
                    };

                let _ = state
                    .feedback_service
                    .record_outcome_with_dsl(
                        interaction_id,
                        ob_semantic_matcher::feedback::Outcome::Executed,
                        None, // outcome_verb same as matched
                        None, // no correction
                        Some(elapsed_ms),
                        generated_dsl,
                        final_dsl_str,
                        user_edits_json,
                    )
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

            // Update intent_feedback outcome (execution failed)
            if let Some(interaction_id) = pending_interaction_id {
                let elapsed_ms = start_time.elapsed().as_millis() as i32;
                let _ = state
                    .feedback_service
                    .record_outcome(
                        interaction_id,
                        ob_semantic_matcher::feedback::Outcome::Executed, // Still "executed" from feedback perspective
                        None,
                        None,
                        Some(elapsed_ms),
                    )
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
            if session.entity_type == "cbu"
                && session.entity_id.is_none()
                && context.active_cbu.is_some()
            {
                let cbu_id = context.active_cbu.as_ref().unwrap().id;
                tracing::info!(
                    "[EXEC] Auto-setting session.entity_id to newly created CBU: {}",
                    cbu_id
                );
                session.set_entity_id(cbu_id);
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

/// POST /api/session/:id/repl-edit - Record REPL edit event
///
/// Called by the UI when the user edits DSL in the REPL editor.
/// This tracks the current_dsl so we can compute diff on execute.
async fn repl_edit_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ReplEditRequest>,
) -> Result<Json<ReplEditResponse>, StatusCode> {
    use crate::dsl_v2::parse_program;

    tracing::debug!(
        "[REPL_EDIT] Session {} - DSL length: {}",
        session_id,
        req.current_dsl.len()
    );

    // Update current_dsl in session via SessionManager
    state
        .session_manager
        .update_current_dsl(session_id, &req.current_dsl)
        .await;

    // Check if there are edits (proposed != current)
    let has_edits = state.session_manager.has_dsl_edits(session_id).await;

    // Validate the current DSL
    let (valid, errors) = match parse_program(&req.current_dsl) {
        Ok(ast) => {
            // Parse succeeded, try compile
            match crate::dsl_v2::compile(&ast) {
                Ok(_) => (true, None),
                Err(e) => (false, Some(vec![format!("Compile error: {:?}", e)])),
            }
        }
        Err(e) => (false, Some(vec![format!("Parse error: {:?}", e)])),
    };

    Ok(Json(ReplEditResponse {
        recorded: true,
        has_edits,
        valid,
        errors,
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
        pending_intents: vec![],
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

/// POST /api/session/:id/view-mode - Set view mode on session (sync from egui client)
///
/// This allows the UI to sync its current view mode (e.g., KYC_UBO, SERVICE_DELIVERY, TRADING)
/// and view level (e.g., Universe, Cluster, System) to the server session.
/// This is important for ensuring the server knows the visualization context.
async fn set_session_view_mode(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetViewModeRequest>,
) -> Result<Json<SetViewModeResponse>, StatusCode> {
    // Update session
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.context.view_mode = Some(req.view_mode.clone());
        // Note: view_level could be stored in session context if needed in future
    }

    tracing::debug!(
        "View mode set: session={} view_mode={} view_level={:?}",
        session_id,
        req.view_mode,
        req.view_level
    );

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    Ok(Json(SetViewModeResponse {
        success: true,
        view_mode: req.view_mode,
        view_level: req.view_level,
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

    #[test]
    fn test_domain_list() {
        let reg = registry();
        let domains = reg.domains();
        assert!(!domains.is_empty());
        assert!(domains.iter().any(|d| d == "cbu"));
        assert!(domains.iter().any(|d| d == "entity"));
    }
}

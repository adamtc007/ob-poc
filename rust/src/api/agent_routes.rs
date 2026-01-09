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

use crate::api::session::{
    create_session_store, AgentSession, ChatRequest, CreateSessionRequest, CreateSessionResponse,
    ExecuteResponse, ExecutionResult, MessageRole, SessionState, SessionStateResponse,
    SessionStore,
};

// API types - SINGLE SOURCE OF TRUTH for HTTP boundary
use crate::database::derive_semantic_state;
use crate::database::generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
use crate::dsl_v2::{
    compile, parse_program, verb_registry::registry, DslExecutor, ExecutionContext,
    ExecutionResult as DslV2Result, SemanticValidator,
};
use crate::ontology::SemanticStageRegistry;
use ob_poc_types::{
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
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Type Converters - Internal types to API types (ob_poc_types)
// ============================================================================

/// Convert internal SessionState to API SessionStateEnum
fn to_session_state_enum(state: &SessionState) -> SessionStateEnum {
    match state {
        SessionState::New => SessionStateEnum::New,
        SessionState::PendingValidation => SessionStateEnum::PendingValidation,
        SessionState::ReadyToExecute => SessionStateEnum::ReadyToExecute,
        SessionState::Executing => SessionStateEnum::Executing,
        SessionState::Executed => SessionStateEnum::Executed,
        SessionState::Closed => SessionStateEnum::Executed, // Map closed to executed for API
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
        dsl_core::AstNode::Literal(lit) => match lit {
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

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ValidateDslRequest {
    pub dsl: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateDslRequest {
    pub instruction: String,
    pub domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateDslResponse {
    pub dsl: Option<String>,
    pub explanation: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Batch Operations Request/Response Types
// ============================================================================

/// Request to add products to multiple CBUs (server-side DSL generation)
#[derive(Debug, Deserialize)]
pub struct BatchAddProductsRequest {
    /// CBU IDs to add products to
    pub cbu_ids: Vec<Uuid>,
    /// Product codes to add (e.g., ["CUSTODY", "FUND_ACCOUNTING"])
    pub products: Vec<String>,
}

/// Result of adding a product to a single CBU
#[derive(Debug, Serialize)]
pub struct BatchProductResult {
    pub cbu_id: Uuid,
    pub product: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services_added: Option<i32>,
}

/// Response from batch add products
#[derive(Debug, Serialize)]
pub struct BatchAddProductsResponse {
    pub total_operations: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub duration_ms: u64,
    pub results: Vec<BatchProductResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DomainsResponse {
    pub domains: Vec<DomainInfo>,
    pub total_verbs: usize,
}

#[derive(Debug, Serialize)]
pub struct DomainInfo {
    pub name: String,
    pub description: String,
    pub verb_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct VocabQuery {
    pub domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VocabResponse {
    pub verbs: Vec<VerbInfo>,
}

#[derive(Debug, Serialize)]
pub struct VerbInfo {
    pub domain: String,
    pub name: String,
    pub full_name: String,
    pub description: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub verb_count: usize,
    pub domain_count: usize,
}

// ============================================================================
// Completion Request/Response Types (LSP-style via EntityGateway)
// ============================================================================

/// Request for entity completion
#[derive(Debug, Deserialize)]
pub struct CompleteRequest {
    /// The type of entity to complete: "cbu", "entity", "product", "role", "jurisdiction", etc.
    pub entity_type: String,
    /// The search query (partial text to match)
    pub query: String,
    /// Maximum number of results (default 10)
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_limit() -> i32 {
    10
}

/// A single completion item
#[derive(Debug, Serialize)]
pub struct CompletionItem {
    /// The value to insert (UUID or code)
    pub value: String,
    /// Display label for the completion
    pub label: String,
    /// Additional detail (e.g., entity type, jurisdiction)
    pub detail: Option<String>,
    /// Relevance score (0.0-1.0)
    pub score: f32,
}

/// Response with completion items
#[derive(Debug, Serialize)]
pub struct CompleteResponse {
    pub items: Vec<CompletionItem>,
    pub total: usize,
}

// ============================================================================
// Session Watch Types (Long-Polling)
// ============================================================================

/// Query parameters for session watch endpoint
#[derive(Debug, Deserialize)]
pub struct WatchQuery {
    /// Timeout in milliseconds (default 30000, max 60000)
    #[serde(default = "default_watch_timeout")]
    pub timeout_ms: u64,
}

fn default_watch_timeout() -> u64 {
    30000
}

/// Response from session watch endpoint
#[derive(Debug, Serialize)]
pub struct WatchResponse {
    /// Session ID
    pub session_id: Uuid,
    /// Version number (incremented on each update)
    pub version: u64,
    /// Current scope path as string
    pub scope_path: String,
    /// Whether struct_mass has been computed
    pub has_mass: bool,
    /// Current effective view mode (if set)
    pub view_mode: Option<String>,
    /// Active CBU ID (if bound)
    pub active_cbu_id: Option<Uuid>,
    /// Timestamp of last update (RFC3339)
    pub updated_at: String,
    /// Whether this is the initial snapshot (no wait) or a change notification
    pub is_initial: bool,
}

impl WatchResponse {
    fn from_snapshot(
        snapshot: &crate::api::session_manager::SessionSnapshot,
        is_initial: bool,
    ) -> Self {
        Self {
            session_id: snapshot.session_id,
            version: snapshot.version,
            scope_path: snapshot.scope_path.clone(),
            has_mass: snapshot.has_mass,
            view_mode: snapshot.view_mode.clone(),
            active_cbu_id: snapshot.active_cbu_id,
            updated_at: snapshot.updated_at.to_rfc3339(),
            is_initial,
        }
    }
}

// ============================================================================
// Entity Reference Resolution Types
// ============================================================================

/// Identifies a specific EntityRef in the AST
#[derive(Debug, Deserialize)]
pub struct RefId {
    /// Index of statement in AST (0-based)
    pub statement_index: usize,
    /// Argument key containing the EntityRef (e.g., "entity-id")
    pub arg_key: String,
}

/// Request to resolve an EntityRef in the session AST
#[derive(Debug, Deserialize)]
pub struct ResolveRefRequest {
    /// Session containing the AST
    pub session_id: Uuid,
    /// Location of the EntityRef to resolve
    pub ref_id: RefId,
    /// Primary key from entity search (UUID or code)
    pub resolved_key: String,
}

/// Statistics about EntityRef resolution in the AST
#[derive(Debug, Serialize)]
pub struct ResolutionStats {
    /// Total EntityRef nodes in AST
    pub total_refs: i32,
    /// Remaining unresolved refs
    pub unresolved_count: i32,
}

/// Response from resolving an EntityRef
#[derive(Debug, Serialize)]
pub struct ResolveRefResponse {
    /// Whether the update succeeded
    pub success: bool,
    /// DSL source re-rendered from updated AST (DSL + AST are a tuple pair)
    pub dsl_source: Option<String>,
    /// Full refreshed AST with updated triplet
    pub ast: Option<Vec<crate::dsl_v2::ast::Statement>>,
    /// Resolution statistics
    pub resolution_stats: ResolutionStats,
    /// True if all refs resolved (ready to execute)
    pub can_execute: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Error code for programmatic handling
    pub code: Option<String>,
}

// ============================================================================
// Onboarding Request/Response Types
// ============================================================================

/// Request to generate onboarding DSL from natural language
#[derive(Debug, Deserialize)]
pub struct OnboardingRequest {
    /// Natural language description of the onboarding request
    pub description: String,
    /// Whether to execute the DSL after generation
    #[serde(default)]
    pub execute: bool,
}

/// Response from onboarding DSL generation
#[derive(Debug, Serialize)]
pub struct OnboardingResponse {
    /// Generated DSL code
    pub dsl: Option<String>,
    /// Explanation of what was generated
    pub explanation: Option<String>,
    /// Validation result
    pub validation: Option<ValidationResult>,
    /// Execution result (if execute=true)
    pub execution: Option<OnboardingExecutionResult>,
    /// Error message if generation failed
    pub error: Option<String>,
}

/// Result of executing onboarding DSL
#[derive(Debug, Serialize)]
pub struct OnboardingExecutionResult {
    pub success: bool,
    pub cbu_id: Option<Uuid>,
    pub resource_count: usize,
    pub delivery_count: usize,
    pub errors: Vec<String>,
}

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct AgentState {
    pub pool: PgPool,
    pub dsl_v2_executor: Arc<DslExecutor>,
    pub sessions: SessionStore,
    /// Session manager with watch channel support for reactive updates
    pub session_manager: crate::api::session_manager::SessionManager,
    pub generation_log: Arc<GenerationLogRepository>,
    pub session_repo: Arc<crate::database::SessionRepository>,
    pub dsl_repo: Arc<crate::database::DslRepository>,
    /// Centralized agent service for chat/disambiguation
    pub agent_service: Arc<crate::api::agent_service::AgentService>,
}

impl AgentState {
    pub fn new(pool: PgPool) -> Self {
        Self::with_sessions(pool, create_session_store())
    }

    /// Create with a shared session store (for integration with other routers)
    pub fn with_sessions(pool: PgPool, sessions: SessionStore) -> Self {
        let dsl_v2_executor = Arc::new(DslExecutor::new(pool.clone()));
        let generation_log = Arc::new(GenerationLogRepository::new(pool.clone()));
        let session_repo = Arc::new(crate::database::SessionRepository::new(pool.clone()));
        let dsl_repo = Arc::new(crate::database::DslRepository::new(pool.clone()));
        let agent_service = Arc::new(crate::api::agent_service::AgentService::with_pool(
            pool.clone(),
        ));
        // Create SessionManager wrapping the same session store
        let session_manager = crate::api::session_manager::SessionManager::new(sessions.clone());
        Self {
            pool,
            dsl_v2_executor,
            sessions,
            session_manager,
            generation_log,
            session_repo,
            dsl_repo,
            agent_service,
        }
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn create_agent_router(pool: PgPool) -> Router {
    create_agent_router_with_sessions(pool, create_session_store())
}

/// Create agent router with a shared session store
pub fn create_agent_router_with_sessions(pool: PgPool, sessions: SessionStore) -> Router {
    let state = AgentState::with_sessions(pool, sessions);

    Router::new()
        // Session management
        .route("/api/session", post(create_session))
        .route("/api/session/:id", get(get_session))
        .route("/api/session/:id", delete(delete_session))
        .route("/api/session/:id/chat", post(chat_session))
        .route("/api/session/:id/execute", post(execute_session_dsl))
        .route("/api/session/:id/clear", post(clear_session_dsl))
        .route("/api/session/:id/bind", post(set_session_binding))
        .route("/api/session/:id/context", get(get_session_context))
        .route("/api/session/:id/focus", post(set_session_focus))
        .route("/api/session/:id/view-mode", post(set_session_view_mode))
        .route("/api/session/:id/dsl/enrich", get(get_enriched_dsl))
        .route("/api/session/:id/watch", get(watch_session))
        // DSL parsing and entity reference resolution
        .route("/api/dsl/parse", post(parse_dsl))
        .route("/api/dsl/resolve-ref", post(resolve_entity_ref))
        // Vocabulary and metadata
        .route("/api/agent/generate", post(generate_dsl))
        .route("/api/agent/validate", post(validate_dsl))
        .route("/api/agent/domains", get(list_domains))
        .route("/api/agent/vocabulary", get(get_vocabulary))
        .route("/api/agent/health", get(health_check))
        // Completions (LSP-style lookup via EntityGateway)
        .route("/api/agent/complete", post(complete_entity))
        // Onboarding
        .route("/api/agent/onboard", post(generate_onboarding_dsl))
        // Enhanced generation with tool use
        .route(
            "/api/agent/generate-with-tools",
            post(generate_dsl_with_tools),
        )
        // Direct DSL execution (no session required)
        .route("/execute", post(direct_execute_dsl))
        // Batch operations (server-side DSL generation, no LLM)
        .route("/api/batch/add-products", post(batch_add_products))
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
    tracing::info!("=== CREATE SESSION ===");
    tracing::info!("Domain hint: {:?}", req.domain_hint);

    let session = crate::api::session::AgentSession::new(req.domain_hint.clone());
    let session_id = session.id;
    let created_at = session.created_at;

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
        state: SessionState::New,
    };
    tracing::info!(
        "Returning CreateSessionResponse: session_id={}, state={:?}",
        response.session_id,
        response.state
    );
    Ok(Json(response))
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
            let new_session = AgentSession::new(None);
            let mut sessions = state.sessions.write().await;
            sessions.insert(session_id, new_session.clone());
            new_session
        }
    };

    Json(SessionStateResponse {
        session_id,
        entity_type: session.entity_type.clone(),
        entity_id: session.entity_id,
        state: session.state.clone(),
        message_count: session.messages.len(),
        pending_intents: session.pending_intents.clone(),
        assembled_dsl: session.assembled_dsl.clone(),
        combined_dsl: session.assembled_dsl.join("\n"),
        context: session.context.clone(),
        messages: session.messages.clone(),
        can_execute: !session.assembled_dsl.is_empty(),
        version: session.updated_at.to_rfc3339(),
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

/// Generate help text showing all available MCP tools/commands
fn generate_commands_help() -> String {
    r#"# Available Commands

## DSL Operations
| Command | Description |
|---------|-------------|
| `dsl_validate` | Parse and validate DSL syntax/semantics |
| `dsl_execute` | Execute DSL against database (with dry_run option) |
| `dsl_plan` | Show execution plan without running |
| `dsl_lookup` | Look up real database IDs (prevents UUID hallucination) |
| `dsl_complete` | Get completions for verbs, domains, products, roles |
| `dsl_signature` | Get verb signature with parameters and types |
| `dsl_generate` | Generate DSL from natural language |

## CBU & Entity Operations
| Command | Description |
|---------|-------------|
| `cbu_get` | Get CBU with entities, roles, documents, screenings |
| `cbu_list` | List/search CBUs with filtering |
| `entity_get` | Get entity details with relationships |
| `entity_search` | Smart entity search with disambiguation |

## Schema & Verbs
| Command | Description |
|---------|-------------|
| `verbs_list` | List available DSL verbs (optionally by domain) |
| `schema_info` | Get entity types, roles, document types |

## Session Context
| Command | Description |
|---------|-------------|
| `session_context` | Get current session context (bindings, active CBU) |

## Workflow Operations
| Command | Description |
|---------|-------------|
| `workflow_status` | Get workflow instance status with blockers |
| `workflow_advance` | Try to advance workflow (evaluates guards) |
| `workflow_transition` | Manual state transition |
| `workflow_start` | Start a new workflow instance |
| `resolve_blocker` | Get resolution options for a blocker |

## Template Operations
| Command | Description |
|---------|-------------|
| `template_list` | List/search templates by tag, blocker, or workflow state |
| `template_get` | Get full template details with params and DSL body |
| `template_expand` | Expand template to DSL with parameter substitution |

## Batch Operations
| Command | Description |
|---------|-------------|
| `batch_start` | Start batch mode with a template |
| `batch_add_entities` | Add entities to a parameter's key set |
| `batch_confirm_keyset` | Mark a key set as complete |
| `batch_set_scalar` | Set a scalar parameter value |
| `batch_get_state` | Get current batch execution state |
| `batch_expand_current` | Expand template for current batch item |
| `batch_record_result` | Record success/failure for current item |
| `batch_skip_current` | Skip current item |
| `batch_cancel` | Cancel batch operation |

## Research Macros
| Command | Description |
|---------|-------------|
| `research_list` | List available research macros |
| `research_get` | Get research macro definition |
| `research_execute` | Execute research with LLM + web search |
| `research_approve` | Approve research results |
| `research_reject` | Reject research results |
| `research_status` | Get current research state |

## Taxonomy Navigation
| Command | Description |
|---------|-------------|
| `taxonomy_get` | Get entity type taxonomy tree |
| `taxonomy_drill_in` | Drill into a taxonomy node |
| `taxonomy_zoom_out` | Zoom out one level |
| `taxonomy_reset` | Reset to root level |
| `taxonomy_position` | Get current position in taxonomy |
| `taxonomy_entities` | List entities of focused type |

## Trading Matrix
| Command | Description |
|---------|-------------|
| `trading_matrix_get` | Get trading matrix summary for a CBU |

## View Commands (Natural Language)
| Say | Effect |
|-----|--------|
| "kyc view" / "ubo view" / "ownership view" | Switch to KYC/UBO view mode |
| "service view" / "service delivery" | Switch to Service Delivery view mode |
| "custody view" / "settlement view" / "ssi view" | Switch to Custody/Trading view mode |
| "switch to kyc" / "switch to service" | Switch view modes |

---
*Type `/commands` or `/help` to see this list again.*
*Type `/verbs` to see all DSL verbs, or `/verbs <domain>` for a specific domain.*"#
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
            return format!(
                "No verbs found for domain '{}'\n\nAvailable domains: {}",
                filter,
                reg.domains().join(", ")
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
            output.push_str("\n");
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
                    output.push_str("\n");
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
async fn chat_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    use crate::api::agent_service::AgentChatRequest;

    tracing::info!("=== CHAT SESSION START ===");
    tracing::info!("Session ID: {}", session_id);
    tracing::info!("Message: {:?}", req.message);
    tracing::info!("CBU ID: {:?}", req.cbu_id);

    // Get session (create if needed)
    let session = {
        let sessions = state.sessions.read().await;
        tracing::info!("Looking up session in store...");
        match sessions.get(&session_id) {
            Some(s) => {
                tracing::info!("Session found, state: {:?}", s.state);
                s.clone()
            }
            None => {
                tracing::warn!("Session NOT FOUND: {}", session_id);
                return Err(StatusCode::NOT_FOUND);
            }
        }
    };

    // Handle /commands or /help - show available MCP tools without LLM
    let trimmed_msg = req.message.trim().to_lowercase();
    if trimmed_msg == "/commands" || trimmed_msg == "/help" || trimmed_msg == "show commands" {
        let commands_help = generate_commands_help();
        return Ok(Json(ChatResponse {
            message: commands_help,
            dsl: None,
            session_state: to_session_state_enum(&session.state),
            commands: None,
        }));
    }

    // Handle /verbs or /verbs <domain> - show DSL verb vocabulary
    if trimmed_msg == "/verbs" || trimmed_msg.starts_with("/verbs ") {
        let domain_filter = if trimmed_msg == "/verbs" {
            None
        } else {
            Some(trimmed_msg.strip_prefix("/verbs ").unwrap_or("").trim())
        };
        let verbs_help = generate_verbs_help(domain_filter);
        return Ok(Json(ChatResponse {
            message: verbs_help,
            dsl: None,
            session_state: to_session_state_enum(&session.state),
            commands: None,
        }));
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
            }));
        }
    };

    tracing::info!(
        "Chat session using {} ({})",
        llm_client.provider_name(),
        llm_client.model_name()
    );

    // Build request for AgentService
    let agent_request = AgentChatRequest {
        message: req.message.clone(),
        cbu_id: req.cbu_id,
        disambiguation_response: None,
    };

    // Delegate to centralized AgentService
    let response = match state
        .agent_service
        .process_chat(&mut session, &agent_request, llm_client)
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
            }));
        }
    };

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

    // Return response using API types (single source of truth)
    Ok(Json(ChatResponse {
        message: response.message,
        dsl: dsl_state,
        session_state: to_session_state_enum(&response.session_state),
        commands: response.commands,
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

        // Determine DSL source
        let dsl_source = if let Some(ref req) = req {
            if let Some(ref dsl_str) = req.dsl {
                dsl_str.clone()
            } else {
                session.assembled_dsl.join("\n")
            }
        } else {
            session.assembled_dsl.join("\n")
        };

        // Check if we have a pre-compiled plan that matches this DSL
        let (plan, ast) = session
            .pending
            .as_ref()
            .and_then(|pending| {
                if pending.source == dsl_source && pending.plan.is_some() {
                    tracing::debug!("[EXEC] Using pre-compiled plan from session.pending");
                    Some((pending.plan.clone(), Some(pending.ast.clone())))
                } else {
                    tracing::debug!(
                        "[EXEC] DSL changed or no pre-compiled plan, will run full pipeline"
                    );
                    None
                }
            })
            .unwrap_or((None, None));

        (dsl_source, plan, ast)
    };

    if dsl.is_empty() {
        return Ok(Json(ExecuteResponse {
            success: false,
            results: Vec::new(),
            errors: vec!["No DSL to execute".to_string()],
            new_state: current_state,
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
                    new_state: current_state,
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
                        new_state: current_state,
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
                    new_state: current_state,
                    bindings: None,
                }));
            }
        }
    };

    // =========================================================================
    // EXECUTE
    // =========================================================================
    let mut results = Vec::new();
    let mut all_success = true;
    let mut errors = Vec::new();

    match state
        .dsl_v2_executor
        .execute_plan(&plan, &mut exec_ctx)
        .await
    {
        Ok(exec_results) => {
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

            // Mark pending DSL as executed and clear it
            // Only EXECUTED status triggers DB persistence
            session.mark_executed();
            session.pending = None; // Clear pending after successful execution
            session.assembled_dsl.clear();

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
        new_state,
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

    // Cancel any pending DSL (marks as CANCELLED, clears from session)
    session.cancel_pending();
    session.pending_intents.clear();
    session.assembled_dsl.clear();

    Ok(Json(SessionStateResponse {
        session_id,
        entity_type: session.entity_type.clone(),
        entity_id: session.entity_id,
        state: session.state.clone(),
        message_count: session.messages.len(),
        pending_intents: session.pending_intents.clone(),
        assembled_dsl: session.assembled_dsl.clone(),
        combined_dsl: String::new(),
        context: session.context.clone(),
        messages: session.messages.clone(),
        can_execute: false,
        version: session.updated_at.to_rfc3339(),
    }))
}

/// Request to set a binding in a session
#[derive(Debug, Deserialize)]
pub struct SetBindingRequest {
    /// The binding name (without @)
    pub name: String,
    /// The UUID to bind (accepts string for TypeScript compat)
    #[serde(deserialize_with = "deserialize_uuid_string")]
    pub id: Uuid,
    /// Entity type (e.g., "cbu", "entity", "case")
    pub entity_type: String,
    /// Human-readable display name
    pub display_name: String,
}

/// Deserialize UUID from string
fn deserialize_uuid_string<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    Uuid::parse_str(&s).map_err(serde::de::Error::custom)
}

/// Response from setting a binding
#[derive(Debug, Serialize)]
pub struct SetBindingResponse {
    pub success: bool,
    pub binding_name: String,
    pub bindings: std::collections::HashMap<String, Uuid>,
}

/// Request to set stage focus in a session
#[derive(Debug, Deserialize)]
pub struct SetFocusRequest {
    /// The stage code to focus on (e.g., "KYC_REVIEW")
    /// Pass None or empty string to clear focus
    #[serde(default)]
    pub stage_code: Option<String>,
}

/// Response from setting stage focus
#[derive(Debug, Serialize)]
pub struct SetFocusResponse {
    pub success: bool,
    /// The stage that is now focused (None if cleared)
    pub stage_code: Option<String>,
    /// Stage name for display
    pub stage_name: Option<String>,
    /// Verbs relevant to this stage (for agent filtering)
    pub relevant_verbs: Vec<String>,
}

/// Request to set view mode on session (sync from egui client)
#[derive(Debug, Deserialize)]
pub struct SetViewModeRequest {
    /// The view mode to set (e.g., "KYC_UBO", "SERVICE_DELIVERY", "TRADING")
    pub view_mode: String,
    /// Optional view level (e.g., "Universe", "Cluster", "System")
    #[serde(default)]
    pub view_level: Option<String>,
}

/// Response from setting view mode
#[derive(Debug, Serialize)]
pub struct SetViewModeResponse {
    pub success: bool,
    /// The view mode that was set
    pub view_mode: String,
    /// The view level that was set (if any)
    pub view_level: Option<String>,
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

    // Get DSL source from session
    let dsl_source = if !session.assembled_dsl.is_empty() {
        session.assembled_dsl.join("\n\n")
    } else {
        session.context.to_dsl_source()
    };

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

// ============================================================================
// Auto-Resolution Helper
// ============================================================================

/// Auto-resolve EntityRefs with exact matches (100% confidence) via EntityGateway.
///
/// For each unresolved EntityRef, searches the gateway. If an exact match is found
/// (value matches token exactly, case-insensitive for lookup tables), the resolved_key
/// is set automatically without requiring user interaction.
async fn auto_resolve_entity_refs(
    program: crate::dsl_v2::ast::Program,
) -> crate::dsl_v2::ast::Program {
    use crate::dsl_v2::ast::{
        find_unresolved_ref_locations, Argument, AstNode, Program, Statement, VerbCall,
    };
    use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

    // Find all unresolved refs
    let unresolved = find_unresolved_ref_locations(&program);
    if unresolved.is_empty() {
        return program;
    }

    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Auto-resolve: Failed to connect to EntityGateway: {}", e);
            return program;
        }
    };

    // Build resolution map: (stmt_idx, arg_key) -> resolved_key
    let mut resolutions: std::collections::HashMap<(usize, String), String> =
        std::collections::HashMap::new();

    for loc in &unresolved {
        // Map entity_type to gateway nickname (uppercase)
        let nickname = loc.entity_type.to_uppercase();

        let search_request = SearchRequest {
            nickname: nickname.clone(),
            values: vec![loc.search_text.clone()],
            search_key: None,
            mode: SearchMode::Exact as i32, // Exact mode for precise matching
            limit: Some(1),
            discriminators: std::collections::HashMap::new(),
        };

        if let Ok(response) = client.search(search_request).await {
            let inner = response.into_inner();
            if let Some(m) = inner.matches.first() {
                // Check for exact match: search_text matches token (case-insensitive for lookup tables)
                let is_exact = m.token.eq_ignore_ascii_case(&loc.search_text)
                    || m.display
                        .to_uppercase()
                        .contains(&loc.search_text.to_uppercase());

                if is_exact {
                    tracing::debug!(
                        "Auto-resolved: {} '{}' -> '{}'",
                        loc.entity_type,
                        loc.search_text,
                        m.token
                    );
                    resolutions.insert((loc.statement_index, loc.arg_key.clone()), m.token.clone());
                }
            }
        }
    }

    if resolutions.is_empty() {
        return program;
    }

    // Apply resolutions to AST
    let resolved_statements: Vec<Statement> = program
        .statements
        .into_iter()
        .enumerate()
        .map(|(stmt_idx, stmt)| {
            if let Statement::VerbCall(vc) = stmt {
                let resolved_args: Vec<Argument> = vc
                    .arguments
                    .into_iter()
                    .map(|arg| {
                        if let Some(resolved_key) = resolutions.get(&(stmt_idx, arg.key.clone())) {
                            // Update the EntityRef with resolved_key
                            if let AstNode::EntityRef {
                                entity_type,
                                search_column,
                                value,
                                span,
                                ..
                            } = arg.value
                            {
                                return Argument {
                                    key: arg.key,
                                    value: AstNode::EntityRef {
                                        entity_type,
                                        search_column,
                                        value,
                                        resolved_key: Some(resolved_key.clone()),
                                        span,
                                    },
                                    span: arg.span,
                                };
                            }
                        }
                        arg
                    })
                    .collect();

                Statement::VerbCall(VerbCall {
                    domain: vc.domain,
                    verb: vc.verb,
                    arguments: resolved_args,
                    binding: vc.binding,
                    span: vc.span,
                })
            } else {
                stmt
            }
        })
        .collect();

    Program {
        statements: resolved_statements,
    }
}

// ============================================================================
// DSL Parse Handler
// ============================================================================

/// Request to parse DSL source into AST
#[derive(Debug, Deserialize)]
pub struct ParseDslRequest {
    /// DSL source text to parse
    pub dsl: String,
    /// Optional session ID to store the parsed AST
    pub session_id: Option<Uuid>,
}

/// A missing required argument
#[derive(Debug, Clone, Serialize)]
pub struct MissingArg {
    /// Statement index (0-based)
    pub statement_index: usize,
    /// Argument name (e.g., "name", "jurisdiction")
    pub arg_name: String,
    /// Verb that requires this arg
    pub verb: String,
}

#[derive(Debug, Serialize)]
pub struct ParseDslResponse {
    /// Whether parsing succeeded
    pub success: bool,
    /// Pipeline stage reached
    pub stage: PipelineStage,
    /// DSL source (echoed back)
    pub dsl_source: String,
    /// Parsed AST (if parse succeeded)
    pub ast: Option<Vec<crate::dsl_v2::ast::Statement>>,
    /// Unresolved EntityRefs requiring resolution
    pub unresolved_refs: Vec<UnresolvedRef>,
    /// Missing required arguments (from CSG validation)
    pub missing_args: Vec<MissingArg>,
    /// Validation errors (non-missing-arg errors)
    pub validation_errors: Vec<String>,
    /// Parse error (if failed)
    pub error: Option<String>,
}

/// POST /api/dsl/parse - Parse DSL source into AST
///
/// Parses DSL source text and returns the AST with any unresolved EntityRefs.
/// DSL + AST are persisted as a tuple pair. If there are unresolved refs,
/// the UI should prompt the user to resolve them before execution.
///
/// ## Request
/// ```json
/// {
///   "dsl": "(cbu.assign-role :cbu-id \"Apex\" :entity-id \"John Smith\" :role \"DIRECTOR\")",
///   "session_id": "optional-uuid"
/// }
/// ```
///
/// ## Response
/// Returns AST with unresolved_refs array for UI to show resolution popups.
async fn parse_dsl(
    State(state): State<AgentState>,
    Json(req): Json<ParseDslRequest>,
) -> Json<ParseDslResponse> {
    use crate::dsl_v2::ast::find_unresolved_ref_locations;
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};
    use crate::dsl_v2::{enrich_program, runtime_registry};

    // Parse the DSL (raw AST with literals only)
    let raw_program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(ParseDslResponse {
                success: false,
                stage: PipelineStage::Draft,
                dsl_source: req.dsl,
                ast: None,
                unresolved_refs: vec![],
                missing_args: vec![],
                validation_errors: vec![],
                error: Some(format!("Parse error: {}", e)),
            });
        }
    };

    // Enrich: convert string literals to EntityRefs based on YAML verb config
    let registry = runtime_registry();
    let enrichment_result = enrich_program(raw_program, registry);
    let mut program = enrichment_result.program;

    // Auto-resolve EntityRefs with exact matches via EntityGateway
    // This handles cases like :product "CUSTODY" where the value matches exactly
    program = auto_resolve_entity_refs(program).await;

    // Run semantic validation with CSG linter to find missing required args
    let mut missing_args: Vec<MissingArg> = vec![];
    let mut validation_errors: Vec<String> = vec![];

    // Try to initialize validator and run CSG validation
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    if let Ok(mut validator) = validator_result {
        let request = ValidationRequest {
            source: req.dsl.clone(),
            context: ValidationContext::default(),
        };
        if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
            validator.validate(&request).await
        {
            for (idx, diag) in diagnostics.iter().enumerate() {
                if diag.severity != Severity::Error {
                    continue;
                }
                let msg = format!("[{}] {}", diag.code.as_str(), diag.message);
                // Pattern: "[E020] missing required argument 'name' for verb 'cbu.create'"
                if diag.code.as_str() == "E020" {
                    // Parse: "missing required argument 'name' for verb 'cbu.create'"
                    // Find first quoted string (arg name) and last quoted string (verb)
                    let parts: Vec<&str> = diag.message.split('\'').collect();
                    if parts.len() >= 4 {
                        // parts: ["missing required argument ", "name", " for verb ", "cbu.create", ""]
                        let arg_name = parts[1].to_string();
                        let verb = parts[3].to_string();
                        missing_args.push(MissingArg {
                            statement_index: idx,
                            arg_name,
                            verb,
                        });
                        continue;
                    }
                }
                // Other validation errors
                validation_errors.push(msg);
            }
        }
    }

    // Find unresolved EntityRefs (those with resolved_key: None)
    let unresolved_locations = find_unresolved_ref_locations(&program);
    let unresolved_refs: Vec<UnresolvedRef> = unresolved_locations
        .into_iter()
        .map(|loc| UnresolvedRef {
            statement_index: loc.statement_index,
            arg_key: loc.arg_key,
            entity_type: loc.entity_type,
            search_text: loc.search_text,
        })
        .collect();

    // Determine stage based on what's needed
    let stage = if !missing_args.is_empty() {
        PipelineStage::Draft // Still need required args
    } else if !unresolved_refs.is_empty() {
        PipelineStage::Resolving // Have all args, need to resolve refs
    } else if !validation_errors.is_empty() {
        PipelineStage::Draft // Other validation errors
    } else {
        PipelineStage::Resolved // Ready to execute
    };

    // If session_id provided, store AST in in-memory session only.
    // Database persistence happens at execution time when we have a CBU context.
    if let Some(session_id) = req.session_id {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.context.ast = program.statements.clone();
            if stage == PipelineStage::Resolved {
                session.state = SessionState::ReadyToExecute;
            } else {
                session.state = SessionState::PendingValidation;
            }
        }
    }

    Json(ParseDslResponse {
        success: true,
        stage,
        dsl_source: req.dsl,
        ast: Some(program.statements),
        unresolved_refs,
        missing_args,
        validation_errors,
        error: None,
    })
}

// ============================================================================
// Entity Reference Resolution Handler
// ============================================================================

/// POST /api/dsl/resolve-ref - Update AST triplet with resolved primary key
///
/// Updates an EntityRef's resolved_key in the session AST without changing
/// the DSL source text. The AST is the source of truth; source is rendered from it.
///
/// ## Request
///
/// ```json
/// {
///   "session_id": "uuid",
///   "ref_id": { "statement_index": 2, "arg_key": "entity-id" },
///   "resolved_key": "550e8400-..."
/// }
/// ```
///
/// ## Response
///
/// Returns the refreshed AST with resolution stats and can_execute flag.
async fn resolve_entity_ref(
    State(state): State<AgentState>,
    Json(req): Json<ResolveRefRequest>,
) -> Result<Json<ResolveRefResponse>, StatusCode> {
    use crate::dsl_v2::ast::{count_entity_refs, AstNode, Statement};

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Validate statement index
    if req.ref_id.statement_index >= session.context.ast.len() {
        return Ok(Json(ResolveRefResponse {
            success: false,
            dsl_source: None,
            ast: None,
            resolution_stats: ResolutionStats {
                total_refs: 0,
                unresolved_count: 0,
            },
            can_execute: false,
            error: Some(format!(
                "Statement index {} out of range (AST has {} statements)",
                req.ref_id.statement_index,
                session.context.ast.len()
            )),
            code: Some("INVALID_REF_ID".to_string()),
        }));
    }

    // Get the statement and find the argument
    let stmt = &mut session.context.ast[req.ref_id.statement_index];

    let update_result = match stmt {
        Statement::VerbCall(vc) => {
            // Find the argument by key
            let arg = vc
                .arguments
                .iter_mut()
                .find(|a| a.key == req.ref_id.arg_key);

            match arg {
                Some(arg) => {
                    // Check if it's an EntityRef
                    match &arg.value {
                        AstNode::EntityRef {
                            entity_type,
                            search_column,
                            value,
                            resolved_key,
                            span,
                        } => {
                            if resolved_key.is_some() {
                                Err((
                                    "EntityRef already has a resolved_key".to_string(),
                                    "ALREADY_RESOLVED".to_string(),
                                ))
                            } else {
                                // Update with resolved key
                                arg.value = AstNode::EntityRef {
                                    entity_type: entity_type.clone(),
                                    search_column: search_column.clone(),
                                    value: value.clone(),
                                    resolved_key: Some(req.resolved_key.clone()),
                                    span: *span,
                                };
                                Ok(())
                            }
                        }
                        _ => Err((
                            format!("Argument '{}' is not an EntityRef", req.ref_id.arg_key),
                            "NOT_ENTITY_REF".to_string(),
                        )),
                    }
                }
                None => Err((
                    format!("Argument '{}' not found in statement", req.ref_id.arg_key),
                    "INVALID_REF_ID".to_string(),
                )),
            }
        }
        Statement::Comment(_) => Err((
            "Cannot resolve ref in a comment statement".to_string(),
            "INVALID_REF_ID".to_string(),
        )),
    };

    // Handle update result
    match update_result {
        Ok(()) => {
            // Calculate resolution stats
            let program = session.context.as_program();
            let stats = count_entity_refs(&program);

            let can_execute = stats.is_fully_resolved();

            // Update session state if ready to execute
            if can_execute && !session.context.ast.is_empty() {
                session.state = SessionState::ReadyToExecute;
            }

            // Re-render DSL from updated AST (DSL + AST are a tuple pair)
            let dsl_source = session.context.to_dsl_source();

            Ok(Json(ResolveRefResponse {
                success: true,
                dsl_source: Some(dsl_source),
                ast: Some(session.context.ast.clone()),
                resolution_stats: ResolutionStats {
                    total_refs: stats.total_refs,
                    unresolved_count: stats.unresolved_count,
                },
                can_execute,
                error: None,
                code: None,
            }))
        }
        Err((message, code)) => Ok(Json(ResolveRefResponse {
            success: false,
            dsl_source: None,
            ast: None,
            resolution_stats: ResolutionStats {
                total_refs: 0,
                unresolved_count: 0,
            },
            can_execute: false,
            error: Some(message),
            code: Some(code),
        })),
    }
}

// ============================================================================
// Vocabulary and Metadata Handlers
// ============================================================================

/// POST /api/agent/validate - Validate DSL
/// POST /api/agent/generate - Generate DSL from natural language
async fn generate_dsl(Json(req): Json<GenerateDslRequest>) -> Json<GenerateDslResponse> {
    // Get vocabulary for the prompt
    let vocab = build_vocab_prompt(req.domain.as_deref());

    // Build the system prompt with onboarding context
    let system_prompt = format!(
        r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

AVAILABLE VERBS:
{}

DSL SYNTAX:
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42, 25.5
- References start with @: @symbol_name (use underscores, not hyphens)
- Use :as @name to capture results

## CRITICAL: EXISTING vs NEW CBUs

**EXISTING CBU** - When user references an existing CBU by name (e.g., "onboard Aviva", "add custody to Apex"):
- Use `cbu.add-product` to add a product to the existing CBU
- The CBU name is matched case-insensitively in the database
- DO NOT use `cbu.ensure` - that would create a duplicate!

**NEW CBU** - Only when explicitly creating a new client:
- Use `cbu.ensure` to create the CBU first
- Then use `cbu.add-product` to add products

### EXAMPLE: Adding product to EXISTING CBU
User: "Onboard Aviva to Custody product"
```
(cbu.add-product :cbu-id "Aviva" :product "CUSTODY")
```

User: "Add Fund Accounting to Apex Capital"
```
(cbu.add-product :cbu-id "Apex Capital" :product "FUND_ACCOUNTING")
```

### EXAMPLE: Creating NEW CBU and adding product
User: "Create a new fund called Pacific Growth in Luxembourg and add Custody"
```
(cbu.ensure :name "Pacific Growth" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.add-product :cbu-id @fund :product "CUSTODY")
```

## Available Products (use product CODE, not display name)

| Product Code | Description |
|--------------|-------------|
| `CUSTODY` | Asset safekeeping, settlement, corporate actions |
| `FUND_ACCOUNTING` | NAV calculation, investor accounting, reporting |
| `TRANSFER_AGENCY` | Investor registry, subscriptions, redemptions |
| `MIDDLE_OFFICE` | Position management, trade capture, P&L |
| `COLLATERAL_MGMT` | Collateral optimization and margin |
| `MARKETS_FX` | Foreign exchange services |
| `ALTS` | Alternative investment administration |

## Client Types
- `fund` - Investment fund (hedge fund, mutual fund, etc.)
- `corporate` - Corporate client
- `individual` - Individual client
- `trust` - Trust structure

## Common Jurisdictions
- `US` - United States
- `GB` - United Kingdom
- `LU` - Luxembourg
- `IE` - Ireland
- `KY` - Cayman Islands
- `JE` - Jersey

## Other DSL Examples

Create entities:
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)
(entity.create-limited-company :name "Holdings Ltd" :jurisdiction "GB" :as @company)

Assign roles (note: @fund must be defined first with cbu.ensure):
(cbu.ensure :name "Acme Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @company :role "PRINCIPAL")

List CBUs:
(cbu.list)

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    );

    // Create LLM client (uses AGENT_BACKEND env var to select provider)
    let llm_client = match crate::agentic::create_llm_client() {
        Ok(client) => client,
        Err(e) => {
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some(format!("LLM client error: {}", e)),
            });
        }
    };

    // Call LLM API with JSON output format
    let json_system_prompt = format!(
        "{}\n\nIMPORTANT: Always respond with valid JSON in this exact format:\n{{\n  \"dsl\": \"(verb.name :arg value ...)\",\n  \"explanation\": \"Brief explanation of what the DSL does\"\n}}\n\nIf you cannot generate DSL, respond with:\n{{\n  \"dsl\": null,\n  \"explanation\": null,\n  \"error\": \"Error message explaining why\"\n}}",
        system_prompt
    );

    let response = llm_client
        .chat_json(&json_system_prompt, &req.instruction)
        .await;

    match response {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(structured) => {
                    let dsl = structured["dsl"].as_str().map(|s| s.to_string());
                    let explanation = structured["explanation"].as_str().map(|s| s.to_string());
                    let error = structured["error"].as_str().map(|s| s.to_string());

                    if let Some(err) = error {
                        Json(GenerateDslResponse {
                            dsl: None,
                            explanation,
                            error: Some(err),
                        })
                    } else if let Some(ref dsl_str) = dsl {
                        // Validate the generated DSL
                        match parse_program(dsl_str) {
                            Ok(_) => Json(GenerateDslResponse {
                                dsl,
                                explanation,
                                error: None,
                            }),
                            Err(e) => Json(GenerateDslResponse {
                                dsl,
                                explanation,
                                error: Some(format!("Syntax error: {}", e)),
                            }),
                        }
                    } else {
                        Json(GenerateDslResponse {
                            dsl: None,
                            explanation,
                            error: Some("No DSL in response".to_string()),
                        })
                    }
                }
                Err(e) => Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Failed to parse structured response: {}", e)),
                }),
            }
        }
        Err(e) => Json(GenerateDslResponse {
            dsl: None,
            explanation: None,
            error: Some(format!("LLM API error: {}", e)),
        }),
    }
}

/// POST /api/agent/generate-with-tools - Generate DSL using Claude tool_use
///
/// This endpoint uses Claude's tool calling feature to look up real database IDs
/// before generating DSL, preventing UUID hallucination.
async fn generate_dsl_with_tools(
    State(state): State<AgentState>,
    Json(req): Json<GenerateDslRequest>,
) -> Json<GenerateDslResponse> {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some("ANTHROPIC_API_KEY not configured".to_string()),
            });
        }
    };

    let vocab = build_vocab_prompt(req.domain.as_deref());
    let system_prompt = build_tool_use_system_prompt(&vocab);

    // Define tools for Claude
    let tools = serde_json::json!([
        {
            "name": "lookup_cbu",
            "description": "Look up an existing CBU (Client Business Unit) by name. ALWAYS use this before referencing a CBU to get the real ID.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "CBU name to search for (case-insensitive)"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "lookup_entity",
            "description": "Look up an existing entity by name. Use this to find persons or companies.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Entity name to search for"
                    },
                    "entity_type": {
                        "type": "string",
                        "description": "Optional: filter by type (proper_person, limited_company, etc.)"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "lookup_product",
            "description": "Look up available products by name.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Product name to search for"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "list_cbus",
            "description": "List all CBUs in the system. Use this to see what clients exist.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max results to return (default 10)"
                    }
                }
            }
        }
    ]);

    let client = reqwest::Client::new();

    // First call - may include tool use
    let mut messages = vec![serde_json::json!({"role": "user", "content": req.instruction})];

    let mut tool_results: Vec<String> = Vec::new();
    let max_iterations = 5;

    for iteration in 0..max_iterations {
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 2048,
                "system": system_prompt,
                "tools": tools,
                "messages": messages
            }))
            .send()
            .await;

        let resp = match response {
            Ok(r) => r,
            Err(e) => {
                return Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Request failed: {}", e)),
                });
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some(format!("API error {}: {}", status, body)),
            });
        }

        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(e) => {
                return Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Failed to parse response: {}", e)),
                });
            }
        };

        let stop_reason = json["stop_reason"].as_str().unwrap_or("");

        // Check if Claude wants to use tools
        if stop_reason == "tool_use" {
            let empty_vec = vec![];
            let content = json["content"].as_array().unwrap_or(&empty_vec);
            let mut tool_use_results = Vec::new();

            for block in content {
                if block["type"] == "tool_use" {
                    let tool_name = block["name"].as_str().unwrap_or("");
                    let tool_id = block["id"].as_str().unwrap_or("");
                    let input = &block["input"];

                    // Execute the tool
                    let result = execute_tool(&state.pool, tool_name, input).await;
                    tool_results.push(format!("{}: {}", tool_name, result));

                    tool_use_results.push(serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": result
                    }));
                }
            }

            // Add assistant message with tool use
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": content
            }));

            // Add tool results
            messages.push(serde_json::json!({
                "role": "user",
                "content": tool_use_results
            }));

            tracing::debug!("Tool use iteration {}: {:?}", iteration, tool_results);
            continue;
        }

        // Claude finished - extract the DSL
        // Note: tool_use mode doesn't support structured outputs, so we extract DSL from text
        let empty_vec2 = vec![];
        let content = json["content"].as_array().unwrap_or(&empty_vec2);
        for block in content {
            if block["type"] == "text" {
                let text = block["text"].as_str().unwrap_or("").trim();

                if text.starts_with("ERROR:") {
                    return Json(GenerateDslResponse {
                        dsl: None,
                        explanation: None,
                        error: Some(text.to_string()),
                    });
                }

                // Try to extract DSL from the response (handle markdown fencing, etc.)
                let dsl_text = extract_dsl_from_text(text);

                // Validate the generated DSL
                match parse_program(&dsl_text) {
                    Ok(_) => {
                        let explanation = if tool_results.is_empty() {
                            "DSL generated successfully".to_string()
                        } else {
                            format!("DSL generated with lookups: {}", tool_results.join(", "))
                        };
                        return Json(GenerateDslResponse {
                            dsl: Some(dsl_text),
                            explanation: Some(explanation),
                            error: None,
                        });
                    }
                    Err(e) => {
                        return Json(GenerateDslResponse {
                            dsl: Some(dsl_text),
                            explanation: None,
                            error: Some(format!("Generated DSL has syntax error: {}", e)),
                        });
                    }
                }
            }
        }

        break;
    }

    Json(GenerateDslResponse {
        dsl: None,
        explanation: None,
        error: Some("Failed to generate DSL after max iterations".to_string()),
    })
}

/// Execute a tool call via EntityGateway and return the result as a string
///
/// All lookups go through the central EntityGateway service for consistent
/// fuzzy matching behavior across LSP, validation, MCP tools, and Claude tool_use.
async fn execute_tool(_pool: &PgPool, tool_name: &str, input: &serde_json::Value) -> String {
    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => return format!("Failed to connect to EntityGateway at {}: {}", addr, e),
    };

    // Helper to search via gateway
    async fn gateway_search(
        client: &mut entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient<tonic::transport::Channel>,
        nickname: &str,
        search: &str,
        limit: i32,
    ) -> Result<Vec<(String, String, f32)>, String> {
        use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![search.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit),
            discriminators: std::collections::HashMap::new(),
        };

        let response = client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        Ok(response
            .into_inner()
            .matches
            .into_iter()
            .map(|m| (m.token, m.display, m.score))
            .collect())
    }

    match tool_name {
        "lookup_cbu" => {
            let name = input["name"].as_str().unwrap_or("");
            match gateway_search(&mut client, "CBU", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (id: {})", display, id))
                        .collect();
                    format!("Found {} CBU(s):\n{}", matches.len(), results.join("\n"))
                }
                Ok(_) => format!("No CBU found matching '{}'", name),
                Err(e) => e,
            }
        }
        "lookup_entity" => {
            let name = input["name"].as_str().unwrap_or("");
            // Use ENTITY nickname which searches across all entity types
            match gateway_search(&mut client, "ENTITY", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (id: {})", display, id))
                        .collect();
                    format!("Found {} entity(s):\n{}", matches.len(), results.join("\n"))
                }
                Ok(_) => format!("No entity found matching '{}'", name),
                Err(e) => e,
            }
        }
        "lookup_product" => {
            let name = input["name"].as_str().unwrap_or("");
            match gateway_search(&mut client, "PRODUCT", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (code: {})", display, id))
                        .collect();
                    format!(
                        "Found {} product(s):\n{}",
                        matches.len(),
                        results.join("\n")
                    )
                }
                Ok(_) => format!("No product found matching '{}'", name),
                Err(e) => e,
            }
        }
        "list_cbus" => {
            let limit = input["limit"].as_i64().unwrap_or(10) as i32;
            // Empty search with high limit to list all
            match gateway_search(&mut client, "CBU", "", limit).await {
                Ok(matches) => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(_, display, _)| format!("- {}", display))
                        .collect();
                    format!("CBUs in system:\n{}", results.join("\n"))
                }
                Err(e) => e,
            }
        }
        _ => format!("Unknown tool: {}", tool_name),
    }
}

/// Build system prompt for tool-use generation
fn build_tool_use_system_prompt(vocab: &str) -> String {
    format!(
        r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

## CRITICAL WORKFLOW

1. **ALWAYS look up existing data first** before generating DSL that references existing entities:
   - Use `lookup_cbu` when user mentions a client/CBU name
   - Use `lookup_entity` when user mentions a person or company
   - Use `lookup_product` when adding products
   - Use `list_cbus` if unsure what clients exist

2. **Use names, not UUIDs** - The DSL system accepts names for CBUs:
   - `(cbu.add-product :cbu-id "Apex Capital" :product "CUSTODY")` ✓
   - The name is matched case-insensitively in the database

3. **Create new entities with @references**:
   - `(cbu.ensure :name "New Fund" :jurisdiction "LU" :as @fund)`
   - Then reference: `(cbu.add-product :cbu-id @fund :product "CUSTODY")`

## AVAILABLE VERBS
{}

## DSL SYNTAX
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42, 25.5
- References start with @: @symbol_name
- Use :as @name to capture results

## EXAMPLES

Adding product to EXISTING CBU (after lookup confirms it exists):
```
(cbu.add-product :cbu-id "Apex Capital" :product "CUSTODY")
```

Creating NEW CBU and adding product:
```
(cbu.ensure :name "Pacific Growth" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.add-product :cbu-id @fund :product "CUSTODY")
```

## PRODUCTS (use exact CODES)
- CUSTODY
- FUND_ACCOUNTING
- TRANSFER_AGENCY
- MIDDLE_OFFICE
- COLLATERAL_MGMT
- MARKETS_FX
- ALTS

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    )
}

/// Extract DSL from agent response text
///
/// Handles common agent output formats:
/// 1. Raw DSL text (ideal)
/// 2. Markdown code fences (```dsl ... ``` or ``` ... ```)
/// 3. Text with DSL embedded (extracts S-expressions)
fn extract_dsl_from_text(text: &str) -> String {
    let trimmed = text.trim();

    // If it already parses as DSL, return as-is
    if trimmed.starts_with('(') && parse_program(trimmed).is_ok() {
        return trimmed.to_string();
    }

    // Try to extract from markdown code fence
    // Matches ```dsl, ```clojure, ```lisp, or just ```
    let fence_patterns = ["```dsl", "```clojure", "```lisp", "```"];
    for pattern in fence_patterns {
        if let Some(start_idx) = trimmed.find(pattern) {
            let after_fence = &trimmed[start_idx + pattern.len()..];
            // Skip to newline after opening fence
            let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
            let content = &after_fence[content_start..];
            // Find closing fence
            if let Some(end_idx) = content.find("```") {
                let extracted = content[..end_idx].trim();
                if parse_program(extracted).is_ok() {
                    return extracted.to_string();
                }
            }
        }
    }

    // Try to find S-expression block in text
    // Look for opening paren and find matching close
    if let Some(start) = trimmed.find('(') {
        let mut depth = 0;
        let mut end = start;
        for (i, c) in trimmed[start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        // Check if there are more statements
                        let remaining = trimmed[end..].trim();
                        if remaining.starts_with('(') {
                            // Multiple statements - find the last closing paren
                            continue;
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        // Find last balanced paren for multi-statement DSL
        let mut last_end = end;
        let mut search_start = end;
        while let Some(next_start) = trimmed[search_start..].find('(') {
            let abs_start = search_start + next_start;
            let mut d = 0;
            for (i, c) in trimmed[abs_start..].char_indices() {
                match c {
                    '(' => d += 1,
                    ')' => {
                        d -= 1;
                        if d == 0 {
                            last_end = abs_start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            search_start = last_end;
        }
        let extracted = trimmed[start..last_end].trim();
        if parse_program(extracted).is_ok() {
            return extracted.to_string();
        }
    }

    // Fall back to original text
    trimmed.to_string()
}

/// Build vocabulary prompt for a domain
fn build_vocab_prompt(domain: Option<&str>) -> String {
    let mut lines = Vec::new();
    let reg = registry();

    let domain_list: Vec<String> = if let Some(d) = domain {
        vec![d.to_string()]
    } else {
        reg.domains().to_vec()
    };

    for domain_name in domain_list {
        for verb in reg.verbs_for_domain(&domain_name) {
            let required = verb.required_arg_names().join(", ");
            let optional = verb.optional_arg_names().join(", ");
            lines.push(format!(
                "{}.{}: {} [required: {}] [optional: {}]",
                verb.domain, verb.verb, verb.description, required, optional
            ));
        }
    }

    lines.join("\n")
}

/// POST /api/agent/validate - Validate DSL syntax and semantics (including dataflow)
async fn validate_dsl(
    State(state): State<AgentState>,
    Json(req): Json<ValidateDslRequest>,
) -> Result<Json<ValidationResult>, StatusCode> {
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};

    // First parse
    if let Err(e) = parse_program(&req.dsl) {
        return Ok(Json(ValidationResult {
            valid: false,
            errors: vec![ValidationError {
                line: None,
                column: None,
                message: e,
                suggestion: None,
            }],
            warnings: vec![],
        }));
    }

    // Then run full semantic validation with CSG linter (includes dataflow)
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    match validator_result {
        Ok(mut validator) => {
            let request = ValidationRequest {
                source: req.dsl.clone(),
                context: ValidationContext::default(),
            };
            match validator.validate(&request).await {
                crate::dsl_v2::validation::ValidationResult::Ok(_) => Ok(Json(ValidationResult {
                    valid: true,
                    errors: vec![],
                    warnings: vec![],
                })),
                crate::dsl_v2::validation::ValidationResult::Err(diagnostics) => {
                    let errors: Vec<ValidationError> = diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .map(|d| ValidationError {
                            line: Some(d.span.line as usize),
                            column: Some(d.span.column as usize),
                            message: format!("[{}] {}", d.code.as_str(), d.message),
                            suggestion: d.suggestions.first().map(|s| s.message.clone()),
                        })
                        .collect();
                    let warnings: Vec<String> = diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Warning)
                        .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                        .collect();
                    Ok(Json(ValidationResult {
                        valid: errors.is_empty(),
                        errors,
                        warnings,
                    }))
                }
            }
        }
        Err(_) => {
            // If validator fails to initialize, fall back to parse-only validation
            Ok(Json(ValidationResult {
                valid: true,
                errors: vec![],
                warnings: vec![],
            }))
        }
    }
}

/// GET /api/agent/domains - List available domains
async fn list_domains() -> Json<DomainsResponse> {
    let reg = registry();
    let domain_list = reg.domains();
    let domains: Vec<DomainInfo> = domain_list
        .iter()
        .map(|name| {
            let verbs = reg.verbs_for_domain(name);
            DomainInfo {
                name: name.to_string(),
                description: get_domain_description(name),
                verb_count: verbs.len(),
            }
        })
        .collect();

    Json(DomainsResponse {
        total_verbs: reg.len(),
        domains,
    })
}

/// GET /api/agent/vocabulary - Get vocabulary
async fn get_vocabulary(Query(query): Query<VocabQuery>) -> Json<VocabResponse> {
    let reg = registry();
    let verbs: Vec<VerbInfo> = if let Some(domain) = &query.domain {
        reg.verbs_for_domain(domain)
            .iter()
            .map(|v| VerbInfo {
                domain: v.domain.to_string(),
                name: v.verb.to_string(),
                full_name: format!("{}.{}", v.domain, v.verb),
                description: v.description.to_string(),
                required_args: v
                    .required_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                optional_args: v
                    .optional_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            })
            .collect()
    } else {
        // Return all verbs
        reg.domains()
            .iter()
            .flat_map(|d| {
                reg.verbs_for_domain(d)
                    .iter()
                    .map(|v| VerbInfo {
                        domain: v.domain.to_string(),
                        name: v.verb.to_string(),
                        full_name: format!("{}.{}", v.domain, v.verb),
                        description: v.description.to_string(),
                        required_args: v
                            .required_arg_names()
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                        optional_args: v
                            .optional_arg_names()
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };

    Json(VocabResponse { verbs })
}

/// GET /api/agent/health - Health check
async fn health_check() -> Json<HealthResponse> {
    let reg = registry();
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        verb_count: reg.len(),
        domain_count: reg.domains().len(),
    })
}

// ============================================================================
// Completion Handler (LSP-style via EntityGateway)
// ============================================================================

/// POST /api/agent/complete - Get completions for entities
///
/// Provides LSP-style autocomplete for CBUs, entities, products, roles,
/// jurisdictions, and other reference data via EntityGateway.
async fn complete_entity(
    Json(req): Json<CompleteRequest>,
) -> Result<Json<CompleteResponse>, StatusCode> {
    // Map entity_type to EntityGateway nickname
    let nickname = match req.entity_type.to_lowercase().as_str() {
        "cbu" => "CBU",
        "entity" | "person" | "company" => "ENTITY",
        "product" => "PRODUCT",
        "role" => "ROLE",
        "jurisdiction" => "JURISDICTION",
        "currency" => "CURRENCY",
        "client_type" | "clienttype" => "CLIENT_TYPE",
        "instrument_class" | "instrumentclass" => "INSTRUMENT_CLASS",
        "market" => "MARKET",
        _ => {
            // Unknown type - return empty
            return Ok(Json(CompleteResponse {
                items: vec![],
                total: 0,
            }));
        }
    };

    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to connect to EntityGateway at {}: {}", addr, e);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    // Search via EntityGateway
    use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

    let search_request = SearchRequest {
        nickname: nickname.to_string(),
        values: vec![req.query.clone()],
        search_key: None,
        mode: SearchMode::Fuzzy as i32,
        limit: Some(req.limit),
        discriminators: std::collections::HashMap::new(),
    };

    let response = match client.search(search_request).await {
        Ok(r) => r.into_inner(),
        Err(e) => {
            tracing::error!("EntityGateway search failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert to completion items
    let items: Vec<CompletionItem> = response
        .matches
        .into_iter()
        .map(|m| CompletionItem {
            value: m.token,
            label: m.display.clone(),
            detail: if m.display.contains('(') {
                // Extract detail from display if present, e.g., "Apex Fund (LU)"
                None
            } else {
                Some(nickname.to_string())
            },
            score: m.score,
        })
        .collect();

    let total = items.len();
    Ok(Json(CompleteResponse { items, total }))
}

// ============================================================================
// Onboarding Handlers
// ============================================================================

/// POST /api/agent/onboard - Generate onboarding DSL from natural language
///
/// Uses the enhanced system prompt with onboarding context to generate
/// complete onboarding workflows from natural language descriptions.
async fn generate_onboarding_dsl(
    State(state): State<AgentState>,
    Json(req): Json<OnboardingRequest>,
) -> Json<OnboardingResponse> {
    // Use the existing generate_dsl logic with onboarding-focused instruction
    let generate_req = GenerateDslRequest {
        instruction: req.description.clone(),
        domain: None, // Let it use all domains including resource and delivery
    };

    let gen_response = generate_dsl(Json(generate_req)).await;

    match (gen_response.dsl.clone(), gen_response.error.clone()) {
        (Some(dsl), None) => {
            // Validate the generated DSL
            let validation = match parse_program(&dsl) {
                Ok(_) => ValidationResult {
                    valid: true,
                    errors: vec![],
                    warnings: vec![],
                },
                Err(e) => ValidationResult {
                    valid: false,
                    errors: vec![ValidationError {
                        line: None,
                        column: None,
                        message: e,
                        suggestion: None,
                    }],
                    warnings: vec![],
                },
            };

            // Execute if requested and valid
            let execution = if req.execute && validation.valid {
                match execute_onboarding_dsl(&state, &dsl).await {
                    Ok(result) => Some(result),
                    Err(e) => Some(OnboardingExecutionResult {
                        success: false,
                        cbu_id: None,
                        resource_count: 0,
                        delivery_count: 0,
                        errors: vec![e],
                    }),
                }
            } else {
                None
            };

            Json(OnboardingResponse {
                dsl: Some(dsl),
                explanation: Some("Onboarding DSL generated successfully".to_string()),
                validation: Some(validation),
                execution,
                error: None,
            })
        }
        (_, Some(error)) => Json(OnboardingResponse {
            dsl: None,
            explanation: None,
            validation: None,
            execution: None,
            error: Some(error),
        }),
        _ => Json(OnboardingResponse {
            dsl: None,
            explanation: None,
            validation: None,
            execution: None,
            error: Some("Unknown error generating DSL".to_string()),
        }),
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Helper: Execute onboarding DSL and count results
async fn execute_onboarding_dsl(
    state: &AgentState,
    dsl: &str,
) -> Result<OnboardingExecutionResult, String> {
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};

    let program = parse_program(dsl).map_err(|e| format!("Parse error: {}", e))?;

    // CSG validation (includes dataflow)
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    if let Ok(mut validator) = validator_result {
        let request = ValidationRequest {
            source: dsl.to_string(),
            context: ValidationContext::default(),
        };
        if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
            validator.validate(&request).await
        {
            let errors: Vec<String> = diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                .collect();
            if !errors.is_empty() {
                return Err(format!("Validation errors: {}", errors.join("; ")));
            }
        }
    }

    let plan = compile(&program).map_err(|e| format!("Compile error: {}", e))?;

    let mut ctx = ExecutionContext::new();
    state
        .dsl_v2_executor
        .execute_plan(&plan, &mut ctx)
        .await
        .map_err(|e| format!("Execution error: {}", e))?;

    // Count resources and deliveries from bindings
    let cbu_id = ctx
        .symbols
        .get("cbu_id")
        .or_else(|| ctx.symbols.get("client"))
        .copied();

    let resource_count = ctx
        .symbols
        .keys()
        .filter(|k| {
            k.contains("custody")
                || k.contains("settle")
                || k.contains("swift")
                || k.contains("nav")
                || k.contains("ibor")
                || k.contains("pnl")
                || k.contains("ledger")
        })
        .count();

    let delivery_count = ctx
        .symbols
        .keys()
        .filter(|k| k.contains("delivery"))
        .count();

    Ok(OnboardingExecutionResult {
        success: true,
        cbu_id,
        resource_count,
        delivery_count,
        errors: vec![],
    })
}

fn get_domain_description(domain: &str) -> String {
    match domain {
        "cbu" => "Client Business Unit lifecycle management".to_string(),
        "entity" => "Legal entity creation and management".to_string(),
        "document" => "Document management and verification".to_string(),
        "kyc" => "KYC investigation and risk assessment".to_string(),
        "screening" => "PEP, sanctions, and adverse media screening".to_string(),
        "decision" => "Approval workflow and decision management".to_string(),
        "monitoring" => "Ongoing monitoring and periodic reviews".to_string(),
        "attribute" => "Attribute value management".to_string(),
        "resource" => "Resource instance management for onboarding".to_string(),
        "delivery" => "Service delivery tracking for onboarding".to_string(),
        _ => format!("{} domain operations", domain),
    }
}

// ============================================================================
// Additional Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ExecuteDslRequest {
    /// DSL source to execute. If None/missing, uses session's assembled_dsl.
    #[serde(default)]
    pub dsl: Option<String>,
}

// ============================================================================
// Direct Execute Handler (no session required)
// ============================================================================

/// Request for direct DSL execution
#[derive(Debug, Deserialize)]
pub struct DirectExecuteRequest {
    pub dsl: String,
    #[serde(default)]
    pub bindings: Option<std::collections::HashMap<String, Uuid>>,
}

/// Primary key returned from DSL execution - derived from verb's `produces` metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryKey {
    /// The entity type produced (from verb YAML `produces.type`: cbu, entity, case, workstream, etc.)
    pub entity_type: String,
    /// Optional subtype for entities (from verb YAML `produces.subtype`: proper_person, limited_company, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    /// The primary key UUID
    pub id: Uuid,
    /// Display name (from :name argument if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Pipeline stage indicating where processing stopped
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PipelineStage {
    /// DSL source received but parse failed
    Draft,
    /// Parse succeeded - AST exists, tokens valid
    /// May have unresolved EntityRefs
    Parsed,
    /// AST has unresolved EntityRefs requiring user/agent resolution
    /// UI should show search popup for each unresolved ref
    Resolving,
    /// All EntityRefs resolved - ready for lint
    Resolved,
    /// CSG linter passed - dataflow valid
    Linted,
    /// Compile succeeded - execution plan ready
    Compiled,
    /// Execute succeeded - DB mutations committed
    Executed,
}

/// An unresolved EntityRef that needs user/agent resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    /// Statement index in AST (0-based)
    pub statement_index: usize,
    /// Argument key containing the EntityRef
    pub arg_key: String,
    /// Entity type for search (e.g., "cbu", "entity", "product")
    pub entity_type: String,
    /// The search text entered by user
    pub search_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectExecuteResponse {
    pub success: bool,
    /// Pipeline stage reached (determines what data is available)
    pub stage: PipelineStage,
    pub results: Vec<ExecutionResult>,
    pub bindings: std::collections::HashMap<String, Uuid>,
    pub errors: Vec<String>,
    /// The AST - available once stage >= Parsed
    /// DSL source + AST are a tuple pair, persisted together
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<Vec<crate::dsl_v2::ast::Statement>>,
    /// Unresolved EntityRefs requiring user/agent resolution
    /// UI should show search popup for each; use /api/dsl/resolve-ref to update
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unresolved_refs: Vec<UnresolvedRef>,
    /// Primary key(s) created/updated by this execution
    /// Each domain returns its primary entity type PK
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub primary_keys: Vec<PrimaryKey>,
    /// Whether statements were reordered by the planner
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reordered: bool,
    /// Synthetic steps injected by the planner (e.g., implicit entity creates)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub synthetic_steps: Vec<SyntheticStepInfo>,
    /// Planner diagnostics (warnings, lifecycle violations)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub planner_diagnostics: Vec<PlannerDiagnosticInfo>,
}

/// Information about a synthetic step injected by the planner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticStepInfo {
    pub binding: String,
    pub verb: String,
    pub entity_type: String,
}

/// Planner diagnostic information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerDiagnosticInfo {
    pub kind: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stmt_index: Option<usize>,
}

/// POST /execute - Direct DSL execution without session
/// Used by Go UI and egui WASM for executing DSL directly
async fn direct_execute_dsl(
    State(state): State<AgentState>,
    Json(req): Json<DirectExecuteRequest>,
) -> Json<DirectExecuteResponse> {
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};
    use crate::dsl_v2::{enrich_program, runtime_registry};

    // Parse - if this fails, we're in Draft stage (no AST)
    let raw_program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(DirectExecuteResponse {
                success: false,
                stage: PipelineStage::Draft,
                results: vec![],
                bindings: std::collections::HashMap::new(),
                errors: vec![format!("Parse error: {}", e)],
                ast: None,
                unresolved_refs: vec![],
                primary_keys: vec![],
                reordered: false,
                synthetic_steps: vec![],
                planner_diagnostics: vec![],
            });
        }
    };

    // Enrich: convert string literals to EntityRefs based on YAML verb config
    let registry = runtime_registry();
    let enrichment_result = enrich_program(raw_program, registry);
    let mut program = enrichment_result.program;

    // Auto-resolve EntityRefs with exact matches via EntityGateway
    // This handles cases like :jurisdiction "LU" or :umbrella-id "Fund Name"
    program = auto_resolve_entity_refs(program).await;

    // Validate with CSG linter (includes dataflow validation)
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    if let Ok(mut validator) = validator_result {
        let request = ValidationRequest {
            source: req.dsl.clone(),
            context: ValidationContext::default(),
        };
        if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
            validator.validate(&request).await
        {
            let errors: Vec<String> = diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                .collect();
            if !errors.is_empty() {
                // Parsed but lint failed - AST exists, stage is Parsed
                return Json(DirectExecuteResponse {
                    success: false,
                    stage: PipelineStage::Parsed,
                    results: vec![],
                    bindings: std::collections::HashMap::new(),
                    errors,
                    ast: Some(program.statements.clone()),
                    primary_keys: vec![],
                    reordered: false,
                    synthetic_steps: vec![],
                    planner_diagnostics: vec![],
                    unresolved_refs: vec![],
                });
            }
        }
    }

    // Check if planner is enabled via environment variable
    let planner_enabled = std::env::var("PLANNER_ENABLED")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    // Track planner results for response
    let mut reordered = false;
    let mut synthetic_steps: Vec<SyntheticStepInfo> = vec![];
    let mut planner_diagnostics: Vec<PlannerDiagnosticInfo> = vec![];

    // Compile (with or without planning)
    let plan = if planner_enabled {
        // Create planning context from request bindings
        let mut planning_context = crate::dsl_v2::PlanningContext::new();
        if let Some(bindings) = &req.bindings {
            for name in bindings.keys() {
                // Add binding with unknown type (we don't track types in simple bindings)
                planning_context.add_binding(name, "unknown");
            }
        }

        match crate::dsl_v2::compile_with_planning(&program, &planning_context) {
            Ok(result) => {
                reordered = result.reordered;

                // Convert synthetic steps to response format
                synthetic_steps = result
                    .synthetic_steps
                    .iter()
                    .map(|s| SyntheticStepInfo {
                        binding: s.binding.clone(),
                        verb: s.verb.clone(),
                        entity_type: s.entity_type.clone(),
                    })
                    .collect();

                // Convert diagnostics to response format
                planner_diagnostics = result
                    .diagnostics
                    .iter()
                    .map(|d| match d {
                        crate::dsl_v2::PlannerDiagnostic::SyntheticStepInjected {
                            binding,
                            verb: _,
                            entity_type,
                            before_stmt,
                        } => PlannerDiagnosticInfo {
                            kind: "synthetic_step".to_string(),
                            message: format!(
                                "Injected synthetic {}.create for @{}",
                                entity_type, binding
                            ),
                            binding: Some(binding.clone()),
                            stmt_index: Some(*before_stmt),
                        },
                        crate::dsl_v2::PlannerDiagnostic::MissingProducer {
                            binding,
                            entity_type,
                            required_by_stmt,
                            reason,
                        } => PlannerDiagnosticInfo {
                            kind: "missing_producer".to_string(),
                            message: format!(
                                "Missing producer for @{} ({}): {}",
                                binding, entity_type, reason
                            ),
                            binding: Some(binding.clone()),
                            stmt_index: Some(*required_by_stmt),
                        },
                        crate::dsl_v2::PlannerDiagnostic::LifecycleViolation {
                            binding,
                            verb,
                            current_state,
                            required_states,
                            stmt_index,
                        } => PlannerDiagnosticInfo {
                            kind: "lifecycle_violation".to_string(),
                            message: format!(
                                "{} requires @{} in state {:?}, but current state is '{}'",
                                verb, binding, required_states, current_state
                            ),
                            binding: Some(binding.clone()),
                            stmt_index: Some(*stmt_index),
                        },
                        crate::dsl_v2::PlannerDiagnostic::StatementsReordered {
                            original_order,
                            new_order,
                            reason,
                        } => PlannerDiagnosticInfo {
                            kind: "reordered".to_string(),
                            message: format!(
                                "Statements reordered: {:?} -> {:?} ({})",
                                original_order, new_order, reason
                            ),
                            binding: None,
                            stmt_index: None,
                        },
                        crate::dsl_v2::PlannerDiagnostic::Warning {
                            message,
                            stmt_index,
                        } => PlannerDiagnosticInfo {
                            kind: "warning".to_string(),
                            message: message.clone(),
                            binding: None,
                            stmt_index: *stmt_index,
                        },
                    })
                    .collect();

                result.plan
            }
            Err(e) => {
                // Linted but compile failed - stage is Linted
                return Json(DirectExecuteResponse {
                    success: false,
                    stage: PipelineStage::Linted,
                    results: vec![],
                    bindings: std::collections::HashMap::new(),
                    errors: vec![format!("Compile error: {}", e)],
                    ast: Some(program.statements.clone()),
                    primary_keys: vec![],
                    reordered: false,
                    synthetic_steps: vec![],
                    planner_diagnostics: vec![],
                    unresolved_refs: vec![],
                });
            }
        }
    } else {
        // Standard compile without planning
        match compile(&program) {
            Ok(p) => p,
            Err(e) => {
                // Linted but compile failed - stage is Linted
                return Json(DirectExecuteResponse {
                    success: false,
                    stage: PipelineStage::Linted,
                    results: vec![],
                    bindings: std::collections::HashMap::new(),
                    errors: vec![format!("Compile error: {}", e)],
                    ast: Some(program.statements.clone()),
                    primary_keys: vec![],
                    reordered: false,
                    synthetic_steps: vec![],
                    planner_diagnostics: vec![],
                    unresolved_refs: vec![],
                });
            }
        }
    };

    // Execute
    let mut ctx = ExecutionContext::new().with_audit_user("direct_execute");

    // Pre-bind any symbols passed from previous executions
    if let Some(bindings) = &req.bindings {
        for (name, id) in bindings {
            ctx.bind(name, *id);
        }
    }

    match state.dsl_v2_executor.execute_plan(&plan, &mut ctx).await {
        Ok(exec_results) => {
            let results: Vec<ExecutionResult> = exec_results
                .iter()
                .enumerate()
                .map(|(idx, r)| {
                    let (entity_id, result_data) = match r {
                        DslV2Result::Uuid(id) => (Some(*id), None),
                        DslV2Result::Record(json) => (None, Some(json.clone())),
                        DslV2Result::RecordSet(records) => {
                            (None, Some(serde_json::Value::Array(records.clone())))
                        }
                        _ => (None, None),
                    };
                    ExecutionResult {
                        statement_index: idx,
                        dsl: req.dsl.clone(),
                        success: true,
                        message: format!("{:?}", r),
                        entity_id,
                        entity_type: None,
                        result: result_data,
                    }
                })
                .collect();

            // Extract primary keys using verb's `produces` metadata from YAML config
            // This is the same metadata used by entity resolution / dataflow validation
            let mut bindings = ctx.symbols.clone();
            let mut primary_keys: Vec<PrimaryKey> = Vec::new();
            let runtime_reg = crate::dsl_v2::runtime_registry();

            for (idx, r) in exec_results.iter().enumerate() {
                if let DslV2Result::Uuid(id) = r {
                    if let Some(step) = plan.steps.get(idx) {
                        let domain = &step.verb_call.domain;
                        let verb = &step.verb_call.verb;

                        // Get the entity type from verb's `produces` metadata
                        // This is authoritative - defined in verbs/*.yaml (same as entity resolution)
                        let (entity_type, subtype) =
                            if let Some(produces) = runtime_reg.get_produces(domain, verb) {
                                (produces.produced_type.clone(), produces.subtype.clone())
                            } else {
                                // Fallback to domain if no produces defined
                                (domain.clone(), None)
                            };

                        // Add to bindings with entity_type-specific key (e.g., cbu_id, entity_id)
                        let binding_key = format!("{}_id", entity_type.replace('-', "_"));
                        bindings.insert(binding_key, *id);

                        // Extract display name from verb arguments
                        // Look at the verb's args config to find the primary name field
                        let name = if let Some(verb_def) = runtime_reg.get(domain, verb) {
                            // Find the first string arg that looks like a name
                            verb_def
                                .args
                                .iter()
                                .find(|a| {
                                    a.name == "name"
                                        || a.name == "first-name"
                                        || a.name.ends_with("-name")
                                })
                                .and_then(|name_arg| {
                                    step.verb_call
                                        .arguments
                                        .iter()
                                        .find(|arg| arg.key == name_arg.name)
                                        .and_then(|arg| {
                                            if let crate::dsl_v2::ast::AstNode::Literal(
                                                crate::dsl_v2::ast::Literal::String(s),
                                            ) = &arg.value
                                            {
                                                Some(s.clone())
                                            } else {
                                                None
                                            }
                                        })
                                })
                        } else {
                            None
                        };

                        primary_keys.push(PrimaryKey {
                            entity_type: entity_type.clone(),
                            subtype,
                            id: *id,
                            name,
                        });
                    }
                }
            }

            // Full pipeline complete - stage is Executed
            Json(DirectExecuteResponse {
                success: true,
                stage: PipelineStage::Executed,
                results,
                bindings,
                errors: vec![],
                ast: Some(program.statements.clone()),
                primary_keys,
                reordered,
                synthetic_steps,
                planner_diagnostics,
                unresolved_refs: vec![],
            })
        }
        Err(e) => {
            // Compiled but execute failed - stage is Compiled
            Json(DirectExecuteResponse {
                success: false,
                stage: PipelineStage::Compiled,
                results: vec![],
                bindings: ctx.symbols.clone(),
                errors: vec![e.to_string()],
                ast: Some(program.statements.clone()),
                primary_keys: vec![],
                reordered,
                synthetic_steps,
                planner_diagnostics,
                unresolved_refs: vec![],
            })
        }
    }
}

// ============================================================================
// Batch Operations Handlers
// ============================================================================

/// POST /api/batch/add-products - Add products to multiple CBUs
/// Server-side DSL generation and execution (no LLM needed)
async fn batch_add_products(
    State(state): State<AgentState>,
    Json(req): Json<BatchAddProductsRequest>,
) -> Json<BatchAddProductsResponse> {
    use std::time::Instant;

    let start = Instant::now();
    let mut results = Vec::new();
    let mut success_count = 0;
    let mut failure_count = 0;

    let executor = DslExecutor::new(state.pool.clone());

    // Process each CBU × product combination
    for cbu_id in &req.cbu_ids {
        for product in &req.products {
            // Generate DSL server-side (deterministic, no LLM)
            let dsl = format!(
                r#"(cbu.add-product :cbu-id "{}" :product "{}")"#,
                cbu_id, product
            );

            // Parse and execute
            match parse_program(&dsl) {
                Ok(program) => {
                    let plan = match compile(&program) {
                        Ok(p) => p,
                        Err(e) => {
                            failure_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: false,
                                error: Some(format!("Compile error: {}", e)),
                                services_added: None,
                            });
                            continue;
                        }
                    };

                    let mut ctx = ExecutionContext::new();
                    match executor.execute_plan(&plan, &mut ctx).await {
                        Ok(exec_results) => {
                            // Count services added from the result
                            let services_added = exec_results
                                .iter()
                                .filter_map(|r| {
                                    if let DslV2Result::Affected(n) = r {
                                        Some(*n as i32)
                                    } else {
                                        None
                                    }
                                })
                                .sum();

                            success_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: true,
                                error: None,
                                services_added: Some(services_added),
                            });
                        }
                        Err(e) => {
                            failure_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: false,
                                error: Some(format!("Execution error: {}", e)),
                                services_added: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(BatchProductResult {
                        cbu_id: *cbu_id,
                        product: product.clone(),
                        success: false,
                        error: Some(format!("Parse error: {:?}", e)),
                        services_added: None,
                    });
                }
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Json(BatchAddProductsResponse {
        total_operations: results.len(),
        success_count,
        failure_count,
        duration_ms,
        results,
    })
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

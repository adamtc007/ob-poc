//! REST API routes for DSL v2 execution
//!
//! Session endpoints:
//! - POST   /api/session           - Create new session
//! - GET    /api/session/:id       - Get session state
//! - DELETE /api/session/:id       - Delete session
//! - POST   /api/session/:id/execute - Execute DSL
//! - POST   /api/session/:id/clear - Clear session
//!
//! Vocabulary endpoints:
//! - GET    /api/agent/domains      - List available DSL domains
//! - GET    /api/agent/vocabulary   - Get vocabulary for a domain
//! - GET    /api/agent/health       - Health check

use crate::api::session::{
    create_session_store, ChatRequest, ChatResponse, CreateSessionRequest, CreateSessionResponse,
    ExecuteResponse, ExecutionResult, SessionState, SessionStateResponse, SessionStore,
};
use crate::dsl_v2::{
    compile, domains as dsl_domains, parse_program, verb_count, verbs_for_domain, DslExecutor,
    ExecutionContext, ExecutionResult as DslV2Result,
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
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ValidateDslRequest {
    pub dsl: String,
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
// State
// ============================================================================

#[derive(Clone)]
pub struct AgentState {
    pub pool: PgPool,
    pub dsl_v2_executor: Arc<DslExecutor>,
    pub sessions: SessionStore,
}

impl AgentState {
    pub fn new(pool: PgPool) -> Self {
        let dsl_v2_executor = Arc::new(DslExecutor::new(pool.clone()));
        let sessions = create_session_store();
        Self {
            pool,
            dsl_v2_executor,
            sessions,
        }
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn create_agent_router(pool: PgPool) -> Router {
    let state = AgentState::new(pool);

    Router::new()
        // Session management
        .route("/api/session", post(create_session))
        .route("/api/session/{id}", get(get_session))
        .route("/api/session/{id}", delete(delete_session))
        .route("/api/session/{id}/chat", post(chat_session))
        .route("/api/session/{id}/execute", post(execute_session_dsl))
        .route("/api/session/{id}/clear", post(clear_session_dsl))
        // Vocabulary and metadata
        .route("/api/agent/validate", post(validate_dsl))
        .route("/api/agent/domains", get(list_domains))
        .route("/api/agent/vocabulary", get(get_vocabulary))
        .route("/api/agent/health", get(health_check))
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
    let session = crate::api::session::AgentSession::new(req.domain_hint.clone());
    let session_id = session.id;
    let created_at = session.created_at;

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
    }

    Ok(Json(CreateSessionResponse {
        session_id,
        created_at,
        state: SessionState::New,
    }))
}

/// GET /api/session/:id - Get session state
async fn get_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionStateResponse {
        session_id,
        state: session.state.clone(),
        message_count: session.messages.len(),
        pending_intents: session.pending_intents.clone(),
        assembled_dsl: session.assembled_dsl.clone(),
        combined_dsl: session.assembled_dsl.join("\n"),
        context: session.context.clone(),
        messages: session.messages.clone(),
        can_execute: !session.assembled_dsl.is_empty(),
    }))
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

/// POST /api/session/:id/chat - Process chat message
/// For now, this just stores the message. LLM integration can be re-added later.
async fn chat_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Store the user message
    session.add_user_message(req.message.clone());

    // For now, respond with a placeholder - LLM integration can be added later
    session.add_agent_message(
        "DSL chat processing is being upgraded. Please use the execute endpoint directly with DSL."
            .to_string(),
        None,
        None,
    );

    Ok(Json(ChatResponse {
        message: "Chat received. Direct DSL execution is available via /execute endpoint."
            .to_string(),
        intents: vec![],
        assembled_dsl: None,
        validation_results: vec![],
        session_state: session.state.clone(),
        can_execute: false,
    }))
}

/// POST /api/session/:id/execute - Execute DSL
async fn execute_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<Option<ExecuteDslRequest>>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Get or create execution context
    let (mut context, current_state) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        (session.context.clone(), session.state.clone())
    };

    // Get DSL to execute - either from request or pending in session
    let dsl = if let Some(req) = req {
        req.dsl
    } else {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.assembled_dsl.join("\n")
    };

    if dsl.is_empty() {
        return Ok(Json(ExecuteResponse {
            success: false,
            results: Vec::new(),
            errors: vec!["No DSL to execute".to_string()],
            new_state: current_state,
        }));
    }

    // Create execution context
    let mut exec_ctx = ExecutionContext::new().with_audit_user(&format!("session-{}", session_id));

    // Pre-bind symbols from session context
    if let Some(id) = context.last_cbu_id {
        exec_ctx.bind("last_cbu", id);
    }
    if let Some(id) = context.last_entity_id {
        exec_ctx.bind("last_entity", id);
    }

    // Parse DSL
    let program = match parse_program(&dsl) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Json(ExecuteResponse {
                success: false,
                results: Vec::new(),
                errors: vec![format!("Parse error: {}", e)],
                new_state: current_state,
            }));
        }
    };

    // Compile to execution plan (handles dependency ordering)
    let plan = match compile(&program) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Json(ExecuteResponse {
                success: false,
                results: Vec::new(),
                errors: vec![format!("Compile error: {}", e)],
                new_state: current_state,
            }));
        }
    };

    // Execute
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
                if let DslV2Result::Uuid(uuid) = exec_result {
                    entity_id = Some(*uuid);
                    // Update context
                    context.last_cbu_id = Some(*uuid);
                    context.cbu_ids.push(*uuid);
                }

                results.push(ExecutionResult {
                    statement_index: idx,
                    dsl: dsl.clone(),
                    success: true,
                    message: "Executed successfully".to_string(),
                    entity_id,
                    entity_type: None,
                });
            }
        }
        Err(e) => {
            all_success = false;
            let error_msg = format!("Execution error: {}", e);
            errors.push(error_msg.clone());
            results.push(ExecutionResult {
                statement_index: 0,
                dsl: dsl.clone(),
                success: false,
                message: error_msg,
                entity_id: None,
                entity_type: None,
            });
        }
    }

    // Update session
    let new_state = {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.context = context;
            let session_results: Vec<crate::api::session::ExecutionResult> = results
                .iter()
                .map(|r| crate::api::session::ExecutionResult {
                    statement_index: r.statement_index,
                    dsl: r.dsl.clone(),
                    success: r.success,
                    message: r.message.clone(),
                    entity_id: r.entity_id,
                    entity_type: r.entity_type.clone(),
                })
                .collect();
            session.record_execution(session_results);
            session.state.clone()
        } else {
            SessionState::New
        }
    };

    Ok(Json(ExecuteResponse {
        success: all_success,
        results,
        errors,
        new_state,
    }))
}

/// POST /api/session/:id/clear - Clear session DSL
async fn clear_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    session.pending_intents.clear();
    session.assembled_dsl.clear();

    Ok(Json(SessionStateResponse {
        session_id,
        state: session.state.clone(),
        message_count: session.messages.len(),
        pending_intents: session.pending_intents.clone(),
        assembled_dsl: session.assembled_dsl.clone(),
        combined_dsl: String::new(),
        context: session.context.clone(),
        messages: session.messages.clone(),
        can_execute: false,
    }))
}

// ============================================================================
// Vocabulary and Metadata Handlers
// ============================================================================

/// POST /api/agent/validate - Validate DSL
async fn validate_dsl(
    Json(req): Json<ValidateDslRequest>,
) -> Result<Json<ValidationResult>, StatusCode> {
    match parse_program(&req.dsl) {
        Ok(_) => Ok(Json(ValidationResult {
            valid: true,
            errors: vec![],
            warnings: vec![],
        })),
        Err(e) => Ok(Json(ValidationResult {
            valid: false,
            errors: vec![ValidationError {
                line: None,
                column: None,
                message: e,
                suggestion: None,
            }],
            warnings: vec![],
        })),
    }
}

/// GET /api/agent/domains - List available domains
async fn list_domains() -> Json<DomainsResponse> {
    let domain_list = dsl_domains();
    let domains: Vec<DomainInfo> = domain_list
        .iter()
        .map(|name| {
            let verbs = verbs_for_domain(name);
            DomainInfo {
                name: name.to_string(),
                description: get_domain_description(name),
                verb_count: verbs.len(),
            }
        })
        .collect();

    Json(DomainsResponse {
        total_verbs: verb_count(),
        domains,
    })
}

/// GET /api/agent/vocabulary - Get vocabulary
async fn get_vocabulary(Query(query): Query<VocabQuery>) -> Json<VocabResponse> {
    let verbs: Vec<VerbInfo> = if let Some(domain) = &query.domain {
        verbs_for_domain(domain)
            .iter()
            .map(|v| VerbInfo {
                domain: v.domain.to_string(),
                name: v.verb.to_string(),
                full_name: format!("{}.{}", v.domain, v.verb),
                description: v.description.to_string(),
                required_args: v.required_args.iter().map(|s| s.to_string()).collect(),
                optional_args: v.optional_args.iter().map(|s| s.to_string()).collect(),
            })
            .collect()
    } else {
        // Return all verbs
        dsl_domains()
            .iter()
            .flat_map(|d| {
                verbs_for_domain(d)
                    .iter()
                    .map(|v| VerbInfo {
                        domain: v.domain.to_string(),
                        name: v.verb.to_string(),
                        full_name: format!("{}.{}", v.domain, v.verb),
                        description: v.description.to_string(),
                        required_args: v.required_args.iter().map(|s| s.to_string()).collect(),
                        optional_args: v.optional_args.iter().map(|s| s.to_string()).collect(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };

    Json(VocabResponse { verbs })
}

/// GET /api/agent/health - Health check
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        verb_count: verb_count(),
        domain_count: dsl_domains().len(),
    })
}

// ============================================================================
// Helpers
// ============================================================================

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
        _ => format!("{} domain operations", domain),
    }
}

// ============================================================================
// Additional Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ExecuteDslRequest {
    pub dsl: String,
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
        let domains = dsl_domains();
        assert!(!domains.is_empty());
        assert!(domains.contains(&"cbu"));
        assert!(domains.contains(&"entity"));
    }
}

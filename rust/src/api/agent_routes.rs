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
//!
//! Onboarding endpoints:
//! - POST   /api/agent/onboard           - Generate onboarding DSL from natural language
//! - GET    /api/agent/onboard/templates - List available onboarding templates
//! - POST   /api/agent/onboard/render    - Render an onboarding template with parameters

use crate::api::session::{
    create_session_store, ChatRequest, ChatResponse, CreateSessionRequest, CreateSessionResponse,
    ExecuteResponse, ExecutionResult, MessageRole, SessionState, SessionStateResponse,
    SessionStore,
};
use crate::database::generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
use crate::dsl_v2::{
    compile, parse_program, verb_registry::registry, DslExecutor, ExecutionContext,
    ExecutionResult as DslV2Result,
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
    pub generation_log: Arc<GenerationLogRepository>,
    pub session_repo: Arc<crate::database::SessionRepository>,
}

impl AgentState {
    pub fn new(pool: PgPool) -> Self {
        let dsl_v2_executor = Arc::new(DslExecutor::new(pool.clone()));
        let sessions = create_session_store();
        let generation_log = Arc::new(GenerationLogRepository::new(pool.clone()));
        let session_repo = Arc::new(crate::database::SessionRepository::new(pool.clone()));
        Self {
            pool,
            dsl_v2_executor,
            sessions,
            generation_log,
            session_repo,
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
        .route("/api/session/:id", get(get_session))
        .route("/api/session/:id", delete(delete_session))
        .route("/api/session/:id/chat", post(chat_session))
        .route("/api/session/:id/execute", post(execute_session_dsl))
        .route("/api/session/:id/clear", post(clear_session_dsl))
        // Vocabulary and metadata
        .route("/api/agent/generate", post(generate_dsl))
        .route("/api/agent/validate", post(validate_dsl))
        .route("/api/agent/domains", get(list_domains))
        .route("/api/agent/vocabulary", get(get_vocabulary))
        .route("/api/agent/health", get(health_check))
        // Onboarding
        .route("/api/agent/onboard", post(generate_onboarding_dsl))
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

    // Store in memory
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
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

/// POST /api/session/:id/chat - Process chat message and generate DSL via Claude
async fn chat_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    // Store the user message first
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.add_user_message(req.message.clone());
    }

    // Call Claude API to generate DSL
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            let mut sessions = state.sessions.write().await;
            let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
            session.add_agent_message(
                "Error: ANTHROPIC_API_KEY not configured".to_string(),
                None,
                None,
            );
            return Ok(Json(ChatResponse {
                message: "Error: ANTHROPIC_API_KEY not configured".to_string(),
                intents: vec![],
                assembled_dsl: None,
                validation_results: vec![],
                session_state: session.state.clone(),
                can_execute: false,
            }));
        }
    };

    // Build vocabulary prompt and system prompt
    let vocab = build_vocab_prompt(None);
    let system_prompt = build_chat_system_prompt(&vocab);

    // Call Claude API
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 2048,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": req.message}
            ]
        }))
        .send()
        .await;

    let (dsl_result, error_msg) = match response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let content = json["content"][0]["text"]
                            .as_str()
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        if content.starts_with("ERROR:") {
                            (None, Some(content))
                        } else {
                            // Validate the generated DSL
                            match parse_program(&content) {
                                Ok(_) => (Some(content), None),
                                Err(e) => (Some(content), Some(format!("Syntax error: {}", e))),
                            }
                        }
                    }
                    Err(e) => (None, Some(format!("Failed to parse API response: {}", e))),
                }
            } else {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                (None, Some(format!("API error {}: {}", status, body)))
            }
        }
        Err(e) => (None, Some(format!("Request failed: {}", e))),
    };

    // Update session with results
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    let (response_msg, can_execute, assembled) = match (&dsl_result, &error_msg) {
        (Some(dsl), None) => {
            // Success - store DSL in session
            session.assembled_dsl = vec![dsl.clone()];
            session.state = SessionState::ReadyToExecute;
            session.add_agent_message(
                "DSL generated successfully. Ready to execute.".to_string(),
                None,
                Some(dsl.clone()),
            );
            (
                "DSL generated successfully. Ready to execute.".to_string(),
                true,
                Some(crate::api::intent::AssembledDsl {
                    statements: vec![dsl.clone()],
                    combined: dsl.clone(),
                    intent_count: 1,
                }),
            )
        }
        (Some(dsl), Some(err)) => {
            // DSL generated but has validation errors
            session.assembled_dsl = vec![dsl.clone()];
            session.state = SessionState::PendingValidation;
            session.add_agent_message(
                format!("DSL generated with warnings: {}", err),
                None,
                Some(dsl.clone()),
            );
            (
                format!("DSL generated with warnings: {}", err),
                false,
                Some(crate::api::intent::AssembledDsl {
                    statements: vec![dsl.clone()],
                    combined: dsl.clone(),
                    intent_count: 1,
                }),
            )
        }
        (None, Some(err)) => {
            // Generation failed
            session.add_agent_message(format!("Error: {}", err), None, None);
            (format!("Error: {}", err), false, None)
        }
        (None, None) => {
            session.add_agent_message("Unknown error occurred".to_string(), None, None);
            ("Unknown error occurred".to_string(), false, None)
        }
    };

    Ok(Json(ChatResponse {
        message: response_msg,
        intents: vec![],
        assembled_dsl: assembled,
        validation_results: vec![],
        session_state: session.state.clone(),
        can_execute,
    }))
}

/// POST /api/session/:id/execute - Execute DSL
async fn execute_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<Option<ExecuteDslRequest>>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Get or create execution context
    let (mut context, current_state, user_intent) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

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
    }
    if let Some(id) = context.last_entity_id {
        exec_ctx.bind("last_entity", id);
    }
    // Pre-bind all named references from previous executions
    for (name, id) in &context.named_refs {
        exec_ctx.bind(name, *id);
    }

    // =========================================================================
    // PARSE DSL
    // =========================================================================
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

    // =========================================================================
    // COMPILE (includes lint)
    // =========================================================================
    let plan = match compile(&program) {
        Ok(p) => p,
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
                if let DslV2Result::Uuid(uuid) = exec_result {
                    entity_id = Some(*uuid);

                    // Only set last_cbu_id if this was a cbu.* verb
                    if let Some(step) = plan.steps.get(idx) {
                        if step.verb_call.domain == "cbu" {
                            context.last_cbu_id = Some(*uuid);
                            context.cbu_ids.push(*uuid);
                        }
                    }
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

            // =========================================================================
            // PERSIST SYMBOLS TO SESSION CONTEXT
            // =========================================================================
            // Copy all symbols from execution context back to session's named_refs
            // This allows @cbu, @entity, etc. to be referenced in subsequent messages
            for (name, id) in &exec_ctx.symbols {
                context.named_refs.insert(name.clone(), *id);
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
        let dsl_clone = dsl.clone();
        let bindings_clone = bindings_map.clone();
        let cbu_id = context.last_cbu_id;
        let domains = crate::database::extract_domains(&dsl_clone);
        let primary_domain = crate::database::detect_domain(&dsl_clone);
        let execution_ms = start_time.elapsed().as_millis() as i32;

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
        bindings: if bindings_map.is_empty() {
            None
        } else {
            Some(bindings_map)
        },
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
(cbu.add-product :cbu-id "Aviva" :product "Custody")
```

User: "Add Fund Accounting to Apex Capital"
```
(cbu.add-product :cbu-id "Apex Capital" :product "Fund Accounting")
```

### EXAMPLE: Creating NEW CBU and adding product
User: "Create a new fund called Pacific Growth in Luxembourg and add Custody"
```
(cbu.ensure :name "Pacific Growth" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.add-product :cbu-id @fund :product "Custody")
```

## Available Products (use exact names)

| Product Name | Description |
|--------------|-------------|
| `Custody` | Asset safekeeping, settlement, corporate actions |
| `Fund Accounting` | NAV calculation, investor accounting, reporting |
| `Transfer Agency` | Investor registry, subscriptions, redemptions |
| `Middle Office` | Position management, trade capture, P&L |
| `Collateral Management` | Collateral optimization and margin |
| `Markets FX` | Foreign exchange services |
| `Alternatives` | Alternative investment administration |

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

Assign roles:
(cbu.assign-role :cbu-id @cbu :entity-id @john :role "DIRECTOR")
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")

List CBUs:
(cbu.list)

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    );

    // Try to call Claude API
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

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": req.instruction}
            ]
        }))
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let content = json["content"][0]["text"]
                            .as_str()
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        if content.starts_with("ERROR:") {
                            Json(GenerateDslResponse {
                                dsl: None,
                                explanation: None,
                                error: Some(content),
                            })
                        } else {
                            // Validate the generated DSL
                            match parse_program(&content) {
                                Ok(_) => Json(GenerateDslResponse {
                                    dsl: Some(content),
                                    explanation: Some("DSL generated successfully".to_string()),
                                    error: None,
                                }),
                                Err(e) => Json(GenerateDslResponse {
                                    dsl: Some(content),
                                    explanation: None,
                                    error: Some(format!("Generated DSL has syntax error: {}", e)),
                                }),
                            }
                        }
                    }
                    Err(e) => Json(GenerateDslResponse {
                        dsl: None,
                        explanation: None,
                        error: Some(format!("Failed to parse API response: {}", e)),
                    }),
                }
            } else {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("API error {}: {}", status, body)),
                })
            }
        }
        Err(e) => Json(GenerateDslResponse {
            dsl: None,
            explanation: None,
            error: Some(format!("Request failed: {}", e)),
        }),
    }
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

/// Build system prompt for chat-based DSL generation
fn build_chat_system_prompt(vocab: &str) -> String {
    format!(
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

EXAMPLES:

;; Create a CBU (Client Business Unit)
(cbu.ensure :name "Acme Corp" :jurisdiction "GB" :client-type "corporate" :as @cbu)

;; Create a person entity
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)

;; Create a company entity
(entity.create-limited-company :name "Holdings Ltd" :jurisdiction "GB" :as @company)

;; Assign a role to an entity within a CBU
(cbu.assign-role :cbu-id @cbu :entity-id @john :role "DIRECTOR")

;; Create a fund with multiple entities
(cbu.ensure :name "Growth Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
(entity.create-proper-person :first-name "Jane" :last-name "Doe" :as @jane)
(entity.create-limited-company :name "Fund Manager SA" :jurisdiction "LU" :as @manager)
(cbu.assign-role :cbu-id @fund :entity-id @jane :role "BENEFICIAL_OWNER")
(cbu.assign-role :cbu-id @fund :entity-id @manager :role "INVESTMENT_MANAGER")

;; Record service delivery (product and service names MUST be quoted strings)
(delivery.record :cbu-id @cbu :product "CUSTODY" :service "ASSET_SAFEKEEPING")
(delivery.record :cbu-id @cbu :product "FUND_ADMIN" :service "NAV_CALCULATION")

IMPORTANT: All string values with spaces or special characters MUST be quoted.
Product codes are typically uppercase with underscores: "CUSTODY", "FUND_ADMIN", "PRIME_BROKERAGE"

Respond with ONLY the DSL code, no explanation or markdown. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    )
}

/// POST /api/agent/validate - Validate DSL syntax
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
    let program = parse_program(dsl).map_err(|e| format!("Parse error: {}", e))?;
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
        let reg = registry();
        let domains = reg.domains();
        assert!(!domains.is_empty());
        assert!(domains.iter().any(|d| d == "cbu"));
        assert!(domains.iter().any(|d| d == "entity"));
    }
}

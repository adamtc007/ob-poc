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
        // Completions (LSP-style lookup via EntityGateway)
        .route("/api/agent/complete", post(complete_entity))
        // Onboarding
        .route("/api/agent/onboard", post(generate_onboarding_dsl))
        // Enhanced generation with tool use
        .route(
            "/api/agent/generate-with-tools",
            post(generate_dsl_with_tools),
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
///
/// Pipeline: User message → Intent extraction (tool call) → DSL builder → Linter → Feedback loop
///
/// This mirrors the Claude Code workflow:
/// 1. Extract structured intents from natural language (tool call)
/// 2. Build DSL from intents (deterministic Rust code)
/// 3. Validate with CSG linter (same as Zed/LSP)
/// 4. If errors, feed back to agent and retry
/// 5. Write valid DSL to session file
async fn chat_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    use crate::api::dsl_builder::{build_dsl_program, validate_intent};
    use crate::api::dsl_session_file::DslSessionFileManager;
    use crate::api::intent::{IntentValidation, VerbIntent};
    use crate::dsl_v2::semantic_validator::SemanticValidator;
    use crate::dsl_v2::validation::ValidationContext;

    const MAX_RETRIES: usize = 3;

    // Initialize file-based session storage
    let file_manager = DslSessionFileManager::new();

    // Ensure session file exists (create if needed)
    if !file_manager.session_exists(session_id).await {
        let domain_hint = {
            let sessions = state.sessions.read().await;
            sessions
                .get(&session_id)
                .and_then(|s| s.context.domain_hint.clone())
        };
        if let Err(e) = file_manager.create_session(session_id, domain_hint).await {
            tracing::error!("Failed to create session file: {}", e);
        }
    }

    // Store the user message first
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.add_user_message(req.message.clone());
    }

    // Get API key
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

    // Build system prompt for intent extraction
    let vocab = build_vocab_prompt(None);
    let system_prompt = build_intent_extraction_prompt(&vocab);

    let client = reqwest::Client::new();
    let mut feedback_context = String::new();
    let mut final_dsl: Option<String> = None;
    let mut final_explanation = String::new();
    let mut all_intents: Vec<VerbIntent> = Vec::new();
    let mut validation_results: Vec<IntentValidation> = Vec::new();

    // Retry loop with linter feedback
    for attempt in 0..MAX_RETRIES {
        // Build message with optional feedback from previous attempt
        let user_message = if feedback_context.is_empty() {
            req.message.clone()
        } else {
            format!(
                "{}\n\n[LINTER FEEDBACK - Please fix these issues]\n{}",
                req.message, feedback_context
            )
        };

        // Call Claude with tool use for structured intent extraction
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
                    {"role": "user", "content": user_message}
                ],
                "tools": [{
                    "name": "generate_dsl_intents",
                    "description": "Generate structured DSL intents from user request. Each intent represents a single DSL verb call.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "intents": {
                                "type": "array",
                                "description": "List of DSL verb intents",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "verb": {
                                            "type": "string",
                                            "description": "The DSL verb, e.g., 'cbu.ensure', 'entity.create-proper-person'"
                                        },
                                        "params": {
                                            "type": "object",
                                            "description": "Parameters with literal values",
                                            "additionalProperties": true
                                        },
                                        "refs": {
                                            "type": "object",
                                            "description": "References to previous results, e.g., {\"cbu-id\": \"@result_1\"}",
                                            "additionalProperties": {"type": "string"}
                                        }
                                    },
                                    "required": ["verb", "params"]
                                }
                            },
                            "explanation": {
                                "type": "string",
                                "description": "Brief explanation of what the DSL will do"
                            }
                        },
                        "required": ["intents", "explanation"]
                    }
                }],
                "tool_choice": {"type": "tool", "name": "generate_dsl_intents"}
            }))
            .send()
            .await;

        // Parse response
        let (intents, explanation) = match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::error!(
                        "Claude API error: {} - {}",
                        resp.status(),
                        resp.text().await.unwrap_or_default()
                    );
                    return Err(StatusCode::BAD_GATEWAY);
                }

                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        // Extract tool use result
                        let tool_input = &json["content"][0]["input"];
                        let intents_json = &tool_input["intents"];
                        let explanation =
                            tool_input["explanation"].as_str().unwrap_or("").to_string();

                        // Parse intents
                        let intents: Vec<VerbIntent> =
                            serde_json::from_value(intents_json.clone()).unwrap_or_default();

                        (intents, explanation)
                    }
                    Err(_) => (Vec::new(), String::new()),
                }
            }
            Err(_) => (Vec::new(), String::new()),
        };

        if intents.is_empty() {
            // No intents extracted - try one more time or give up
            if attempt < MAX_RETRIES - 1 {
                feedback_context = "Could not extract any DSL intents. Please try again with clearer verb and parameter names.".to_string();
                continue;
            }
            break;
        }

        // Validate intents against registry (uses same registry as LSP)
        validation_results.clear();
        let mut has_errors = false;
        let mut error_feedback = Vec::new();

        for intent in &intents {
            let validation = validate_intent(intent);
            if !validation.valid {
                has_errors = true;
                for err in &validation.errors {
                    error_feedback.push(format!(
                        "Verb '{}': {} {}",
                        intent.verb,
                        err.message,
                        err.param
                            .as_deref()
                            .map(|p| format!("(param: {})", p))
                            .unwrap_or_default()
                    ));
                }
            }
            validation_results.push(validation);
        }

        // Build DSL from intents (deterministic - no LLM involved)
        let dsl = build_dsl_program(&intents);

        // Run SemanticValidator with EntityGateway (same pipeline as Zed/LSP)
        // This validates embedded data values (products, roles, jurisdictions, etc.)
        match SemanticValidator::new(state.pool.clone()).await {
            Ok(mut validator) => {
                let request = crate::dsl_v2::validation::ValidationRequest {
                    source: dsl.clone(),
                    context: ValidationContext::default(),
                };
                match validator.validate(&request).await {
                    crate::dsl_v2::validation::ValidationResult::Err(diagnostics) => {
                        has_errors = true;
                        for diag in diagnostics {
                            if diag.severity == crate::dsl_v2::validation::Severity::Error {
                                error_feedback.push(format!("Validation: {}", diag.message));
                            }
                        }
                    }
                    crate::dsl_v2::validation::ValidationResult::Ok(_) => {
                        // Validation passed
                    }
                }
            }
            Err(e) => {
                // EntityGateway not available - log warning but don't fail
                tracing::warn!("EntityGateway validation unavailable: {}", e);
            }
        }

        // If no errors, we're done
        if !has_errors {
            final_dsl = Some(dsl);
            final_explanation = explanation;
            all_intents = intents;
            break;
        }

        // Build feedback for next attempt
        if attempt < MAX_RETRIES - 1 {
            feedback_context = error_feedback.join("\n");
        } else {
            // Last attempt - return what we have with errors
            final_dsl = Some(dsl);
            final_explanation = format!(
                "{}\n\nNote: DSL has validation issues:\n{}",
                explanation,
                error_feedback.join("\n")
            );
            all_intents = intents;
        }
    }

    // Update session with results
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    let (response_msg, can_execute, assembled) = match final_dsl {
        Some(ref dsl) => {
            let has_errors = validation_results.iter().any(|v| !v.valid);
            session.assembled_dsl = vec![dsl.clone()];
            session.state = if has_errors {
                SessionState::PendingValidation
            } else {
                SessionState::ReadyToExecute
            };

            let msg = if final_explanation.is_empty() {
                "DSL generated successfully.".to_string()
            } else {
                final_explanation.clone()
            };

            session.add_agent_message(msg.clone(), None, Some(dsl.clone()));

            // Write DSL to session file (like Claude Code writes to source files)
            let description = req.message.chars().take(50).collect::<String>();
            if let Err(e) = file_manager.append_dsl(session_id, dsl, &description).await {
                tracing::error!("Failed to write DSL to session file: {}", e);
            }

            (
                msg,
                !has_errors,
                Some(crate::api::intent::AssembledDsl {
                    statements: vec![dsl.clone()],
                    combined: dsl.clone(),
                    intent_count: all_intents.len(),
                }),
            )
        }
        None => {
            let msg =
                "Could not generate valid DSL. Please try rephrasing your request.".to_string();
            session.add_agent_message(msg.clone(), None, None);
            (msg, false, None)
        }
    };

    Ok(Json(ChatResponse {
        message: response_msg,
        intents: all_intents,
        assembled_dsl: assembled,
        validation_results,
        session_state: session.state.clone(),
        can_execute,
    }))
}

/// Build system prompt for intent extraction (tool-based)
fn build_intent_extraction_prompt(vocab: &str) -> String {
    format!(
        r#"You are a DSL intent extraction assistant. Your job is to convert natural language requests into structured DSL intents.

IMPORTANT: You MUST use the generate_dsl_intents tool to return your response. Do NOT return plain text.

## Available DSL Verbs

{}

## Intent Structure

Each intent represents a single DSL verb call with:
- verb: The verb name (e.g., "cbu.ensure", "entity.create-proper-person")
- params: Literal parameter values (e.g., {{"name": "Acme Corp", "jurisdiction": "LU"}})
- refs: References to previous results (e.g., {{"cbu-id": "@result_1"}})

## Rules

1. Use exact verb names from the vocabulary
2. Use exact parameter names (with hyphens, e.g., "client-type" not "clientType")
3. For sequences, use @result_N references where N is the sequence number
4. Common client types: "fund", "corporate", "individual"
5. Use ISO codes for jurisdictions: "LU", "US", "GB", "IE", etc.

## Product Codes (MUST use exact uppercase codes)

| User Says | Product Code |
|-----------|--------------|
| custody, safekeeping | CUSTODY |
| fund accounting, fund admin, NAV | FUND_ACCOUNTING |
| transfer agency, TA, investor registry | TRANSFER_AGENCY |
| middle office, trade capture | MIDDLE_OFFICE |
| collateral, margin | COLLATERAL_MGMT |
| FX, foreign exchange | MARKETS_FX |
| alternatives, alts, hedge fund admin | ALTS |

IMPORTANT: Always use the UPPERCASE product codes, never the display names.

## Examples

User: "Create a fund called Test Fund in Luxembourg"
Intent: {{
  "verb": "cbu.ensure",
  "params": {{"name": "Test Fund", "jurisdiction": "LU", "client-type": "fund"}},
  "refs": {{}}
}}

User: "Add custody product to the fund"
Intent: {{
  "verb": "cbu.add-product",
  "params": {{"product": "CUSTODY"}},
  "refs": {{"cbu-id": "@result_1"}}
}}

User: "Add fund accounting and transfer agency"
Intents: [
  {{"verb": "cbu.add-product", "params": {{"product": "FUND_ACCOUNTING"}}, "refs": {{"cbu-id": "@result_1"}}}},
  {{"verb": "cbu.add-product", "params": {{"product": "TRANSFER_AGENCY"}}, "refs": {{"cbu-id": "@result_1"}}}}
]"#,
        vocab
    )
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

    // Call Claude API - request JSON response in system prompt
    let client = reqwest::Client::new();
    let json_system_prompt = format!(
        "{}\n\nIMPORTANT: Always respond with valid JSON in this exact format:\n{{\n  \"dsl\": \"(verb.name :arg value ...)\",\n  \"explanation\": \"Brief explanation of what the DSL does\"\n}}\n\nIf you cannot generate DSL, respond with:\n{{\n  \"dsl\": null,\n  \"explanation\": null,\n  \"error\": \"Error message explaining why\"\n}}",
        system_prompt
    );
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "system": json_system_prompt,
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
                        // Structured output: response is guaranteed valid JSON in content[0].text
                        let content = json["content"][0]["text"].as_str().unwrap_or("{}");

                        match serde_json::from_str::<serde_json::Value>(content) {
                            Ok(structured) => {
                                let dsl = structured["dsl"].as_str().map(|s| s.to_string());
                                let explanation =
                                    structured["explanation"].as_str().map(|s| s.to_string());
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

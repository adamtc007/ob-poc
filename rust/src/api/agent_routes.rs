//! REST API routes for Agent-driven DSL generation
//!
//! Provides endpoints for the intelligent DSL generation frontend:
//! - POST /api/agent/generate - Generate DSL from natural language
//! - POST /api/agent/validate - Validate generated DSL
//! - POST /api/agent/execute - Generate, validate, and execute DSL
//! - GET  /api/agent/domains - List available DSL domains
//! - GET  /api/agent/templates - List available templates
//! - GET  /api/agent/vocabulary - Get vocabulary for a domain
//!
//! Session endpoints (Intent-based pipeline):
//! - POST   /api/session           - Create new session
//! - GET    /api/session/:id       - Get session state
//! - DELETE /api/session/:id       - Delete session
//! - POST   /api/session/:id/chat  - Send chat, extract intents, assemble DSL
//! - POST   /api/session/:id/execute - Execute accumulated DSL
//! - POST   /api/session/:id/clear - Clear accumulated DSL

use crate::api::dsl_assembler::DslAssembler;
use crate::api::intent_extractor::IntentExtractor;
use crate::api::session::{
    create_session_store, AgentSession, ChatRequest, ChatResponse, CreateSessionRequest,
    CreateSessionResponse, ExecuteResponse, ExecutionResult, SessionState, SessionStateResponse,
    SessionStore,
};
use crate::dsl_source::agentic::{LlmDslGenerator, RagContextProvider};
use crate::forth_engine::ast::DslParser;
use crate::forth_engine::env::{OnboardingRequestId, RuntimeEnv};
use crate::forth_engine::parser_nom::NomDslParser;
use crate::forth_engine::vocab_registry::create_standard_runtime;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct GenerateDslRequest {
    /// Natural language instruction
    pub instruction: String,
    /// Operation type hint: CREATE, READ, UPDATE, DELETE
    #[serde(default = "default_operation")]
    pub operation_type: String,
    /// Optional domain hint: cbu, entity, document, kyc, screening, decision, monitoring
    pub domain: Option<String>,
    /// Optional: include validation in response
    #[serde(default)]
    pub validate: bool,
}

fn default_operation() -> String {
    "CREATE".to_string()
}

#[derive(Debug, Serialize)]
pub struct GenerateDslResponse {
    pub success: bool,
    pub dsl_text: Option<String>,
    pub confidence: f64,
    pub reasoning: String,
    pub validation: Option<ValidationResult>,
    pub generation_time_ms: u64,
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
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateDslRequest {
    pub dsl: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateDslResponse {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub parsed_verbs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DomainInfo {
    pub name: String,
    pub description: String,
    pub verb_count: usize,
    pub example_verbs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DomainsResponse {
    pub domains: Vec<DomainInfo>,
}

#[derive(Debug, Serialize)]
pub struct VocabularyEntry {
    pub verb: String,
    pub signature: String,
    pub description: String,
    pub domain: String,
}

#[derive(Debug, Serialize)]
pub struct VocabularyResponse {
    pub domain: Option<String>,
    pub entries: Vec<VocabularyEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub description: String,
    pub required_vars: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TemplatesResponse {
    pub templates: Vec<TemplateInfo>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub llm_configured: bool,
    pub database_connected: bool,
    pub vocabulary_loaded: bool,
    pub verb_count: usize,
}

// ============================================================================
// Shared State
// ============================================================================

#[derive(Clone)]
pub struct AgentState {
    pub pool: PgPool,
    pub rag_provider: Arc<RagContextProvider>,
    pub runtime: Arc<crate::forth_engine::runtime::Runtime>,
    pub sessions: SessionStore,
    pub extractor: Arc<IntentExtractor>,
    pub assembler: Arc<DslAssembler>,
}

impl AgentState {
    pub fn new(pool: PgPool) -> Self {
        let rag_provider = Arc::new(RagContextProvider::new(pool.clone()));
        let runtime = Arc::new(create_standard_runtime());
        let sessions = create_session_store();
        let extractor = Arc::new(IntentExtractor::new(rag_provider.clone(), runtime.clone()));
        let assembler = Arc::new(DslAssembler::new(runtime.clone()));
        Self {
            pool,
            rag_provider,
            runtime,
            sessions,
            extractor,
            assembler,
        }
    }
}

// ============================================================================
// Route Handlers - Agent DSL Generation
// ============================================================================

/// POST /api/agent/generate
/// Generate DSL from natural language instruction
async fn generate_dsl(
    State(state): State<AgentState>,
    Json(req): Json<GenerateDslRequest>,
) -> Result<Json<GenerateDslResponse>, StatusCode> {
    let start = Instant::now();

    // Create LLM generator
    let generator = match LlmDslGenerator::from_env_with_runtime(
        state.rag_provider.clone(),
        state.runtime.clone(),
    ) {
        Ok(g) => g,
        Err(e) => {
            return Ok(Json(GenerateDslResponse {
                success: false,
                dsl_text: None,
                confidence: 0.0,
                reasoning: String::new(),
                validation: None,
                generation_time_ms: start.elapsed().as_millis() as u64,
                error: Some(format!("Failed to initialize LLM generator: {}", e)),
            }));
        }
    };

    // Generate DSL
    let result = generator
        .generate(&req.instruction, &req.operation_type, req.domain.as_deref())
        .await;

    match result {
        Ok(generated) => {
            let validation = if req.validate {
                Some(validate_dsl_internal(&generated.dsl_text, &state.runtime))
            } else {
                None
            };

            Ok(Json(GenerateDslResponse {
                success: true,
                dsl_text: Some(generated.dsl_text),
                confidence: generated.confidence,
                reasoning: generated.reasoning,
                validation,
                generation_time_ms: start.elapsed().as_millis() as u64,
                error: None,
            }))
        }
        Err(e) => Ok(Json(GenerateDslResponse {
            success: false,
            dsl_text: None,
            confidence: 0.0,
            reasoning: String::new(),
            validation: None,
            generation_time_ms: start.elapsed().as_millis() as u64,
            error: Some(format!("Generation failed: {}", e)),
        })),
    }
}

/// POST /api/agent/validate
/// Validate DSL syntax and semantics
async fn validate_dsl(
    State(state): State<AgentState>,
    Json(req): Json<ValidateDslRequest>,
) -> Result<Json<ValidateDslResponse>, StatusCode> {
    let result = validate_dsl_internal(&req.dsl, &state.runtime);

    Ok(Json(ValidateDslResponse {
        valid: result.valid,
        errors: result.errors,
        warnings: result.warnings,
        parsed_verbs: extract_verbs(&req.dsl),
    }))
}

/// GET /api/agent/domains
/// List available DSL domains
async fn list_domains(
    State(state): State<AgentState>,
) -> Result<Json<DomainsResponse>, StatusCode> {
    let runtime = &state.runtime;

    // Get domains from the runtime vocabulary
    let domains = vec![
        DomainInfo {
            name: "cbu".to_string(),
            description: "Client Business Unit operations".to_string(),
            verb_count: count_domain_verbs(runtime, "cbu"),
            example_verbs: vec!["cbu.ensure".to_string(), "cbu.attach-entity".to_string()],
        },
        DomainInfo {
            name: "entity".to_string(),
            description: "Entity creation and management".to_string(),
            verb_count: count_domain_verbs(runtime, "entity"),
            example_verbs: vec![
                "entity.create-limited-company".to_string(),
                "entity.create-proper-person".to_string(),
            ],
        },
        DomainInfo {
            name: "document".to_string(),
            description: "Document handling and extraction".to_string(),
            verb_count: count_domain_verbs(runtime, "document"),
            example_verbs: vec!["document.request".to_string(), "document.link".to_string()],
        },
        DomainInfo {
            name: "kyc".to_string(),
            description: "KYC investigation and risk assessment".to_string(),
            verb_count: count_domain_verbs(runtime, "investigation")
                + count_domain_verbs(runtime, "risk"),
            example_verbs: vec![
                "investigation.open".to_string(),
                "risk.assess-cbu".to_string(),
            ],
        },
        DomainInfo {
            name: "screening".to_string(),
            description: "Screening operations (PEP, sanctions, adverse media)".to_string(),
            verb_count: count_domain_verbs(runtime, "screening"),
            example_verbs: vec![
                "screening.pep".to_string(),
                "screening.sanctions".to_string(),
            ],
        },
        DomainInfo {
            name: "decision".to_string(),
            description: "Decision and approval workflows".to_string(),
            verb_count: count_domain_verbs(runtime, "decision"),
            example_verbs: vec![
                "decision.approve".to_string(),
                "decision.reject".to_string(),
            ],
        },
        DomainInfo {
            name: "monitoring".to_string(),
            description: "Ongoing monitoring and reviews".to_string(),
            verb_count: count_domain_verbs(runtime, "monitoring"),
            example_verbs: vec![
                "monitoring.schedule-review".to_string(),
                "monitoring.add-alert-rule".to_string(),
            ],
        },
    ];

    Ok(Json(DomainsResponse { domains }))
}

/// GET /api/agent/vocabulary
/// Get vocabulary entries, optionally filtered by domain
async fn get_vocabulary(
    State(state): State<AgentState>,
    axum::extract::Query(params): axum::extract::Query<VocabQueryParams>,
) -> Result<Json<VocabularyResponse>, StatusCode> {
    let runtime = &state.runtime;
    let mut entries = Vec::new();

    // Get words based on domain filter
    if let Some(ref filter_domain) = params.domain {
        for word in runtime.get_domain_words(filter_domain) {
            entries.push(VocabularyEntry {
                verb: word.name.to_string(),
                signature: word.signature.to_string(),
                description: word.description.to_string(),
                domain: word.domain.to_string(),
            });
        }
    } else {
        // Get all words
        for name in runtime.get_all_word_names() {
            if let Some(word) = runtime.get_word(name) {
                entries.push(VocabularyEntry {
                    verb: word.name.to_string(),
                    signature: word.signature.to_string(),
                    description: word.description.to_string(),
                    domain: word.domain.to_string(),
                });
            }
        }
    }

    let total = entries.len();

    Ok(Json(VocabularyResponse {
        domain: params.domain,
        entries,
        total,
    }))
}

#[derive(Debug, Deserialize)]
pub struct VocabQueryParams {
    pub domain: Option<String>,
}

/// GET /api/agent/health
/// Health check for agent subsystem
async fn agent_health(State(state): State<AgentState>) -> Result<Json<HealthResponse>, StatusCode> {
    // Check LLM configuration
    let llm_configured =
        std::env::var("ANTHROPIC_API_KEY").is_ok() || std::env::var("OPENAI_API_KEY").is_ok();

    // Check database
    let database_connected = sqlx::query("SELECT 1").fetch_one(&state.pool).await.is_ok();

    // Check vocabulary
    let verb_count = state.runtime.get_all_word_names().len();
    let vocabulary_loaded = verb_count > 0;

    Ok(Json(HealthResponse {
        status: if llm_configured && database_connected && vocabulary_loaded {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        llm_configured,
        database_connected,
        vocabulary_loaded,
        verb_count,
    }))
}

// ============================================================================
// Route Handlers - Session Management
// ============================================================================

/// POST /api/session
/// Create a new agent session
async fn create_session(
    State(state): State<AgentState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    let session = AgentSession::new(req.domain_hint);
    let session_id = session.id;
    let created_at = session.created_at;
    let session_state = session.state.clone();

    // Store the session
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
    }

    Ok(Json(CreateSessionResponse {
        session_id,
        created_at,
        state: session_state,
    }))
}

/// GET /api/session/:id
/// Get session state
async fn get_session_state(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionStateResponse {
        session_id: session.id,
        state: session.state.clone(),
        message_count: session.messages.len(),
        pending_intents: session.pending_intents.clone(),
        assembled_dsl: session.assembled_dsl.clone(),
        combined_dsl: session.combined_dsl(),
        context: session.context.clone(),
        messages: session.messages.clone(),
        can_execute: session.can_execute(),
    }))
}

/// DELETE /api/session/:id
/// Delete a session
async fn delete_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut sessions = state.sessions.write().await;
    if sessions.remove(&session_id).is_some() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// POST /api/session/:id/chat
/// Send a chat message - extracts intents via LLM, validates, and assembles DSL
async fn chat_in_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    // Get session domain hint and add user message
    let domain_hint = {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.add_user_message(req.message.clone());
        session.context.domain_hint.clone()
    };

    // Step 1: Extract intents via LLM
    let extraction_result = state
        .extractor
        .extract(&req.message, domain_hint.as_deref())
        .await;

    let (intents, reasoning, _confidence) = match extraction_result {
        Ok(seq) => (seq.intents, seq.reasoning, seq.confidence),
        Err(e) => {
            // Add error message to session
            let session_state = {
                let mut sessions = state.sessions.write().await;
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.add_agent_message(
                        format!("Failed to extract intents: {}", e),
                        None,
                        None,
                    );
                    session.state.clone()
                } else {
                    SessionState::New
                }
            };

            return Ok(Json(ChatResponse {
                message: format!("Failed to extract intents: {}", e),
                intents: vec![],
                validation_results: vec![],
                assembled_dsl: None,
                session_state,
                can_execute: false,
            }));
        }
    };

    // Step 2: Validate all intents
    let validations = state.assembler.validate_all(&intents);
    let all_valid = validations.iter().all(|v| v.valid);

    // Step 3: Assemble DSL if all valid
    let (assembled, session_state, can_execute) = {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        let assembled = if all_valid && !intents.is_empty() {
            match state.assembler.assemble(&intents, &session.context) {
                Ok(dsl) => {
                    session.set_assembled_dsl(dsl.statements.clone());
                    Some(dsl)
                }
                Err(_) => {
                    session.add_intents(intents.clone());
                    None
                }
            }
        } else if !intents.is_empty() {
            // Store intents even if invalid for debugging
            session.add_intents(intents.clone());
            None
        } else {
            None
        };

        // Build agent response message
        let agent_content = reasoning.clone().unwrap_or_else(|| {
            if all_valid && !intents.is_empty() {
                format!(
                    "Extracted {} operation(s). DSL ready to execute.",
                    intents.len()
                )
            } else if !intents.is_empty() {
                "Some operations could not be validated. Please review errors.".to_string()
            } else {
                "No operations were extracted from your request.".to_string()
            }
        });

        session.add_agent_message(
            agent_content,
            Some(intents.clone()),
            assembled.as_ref().map(|a| a.combined.clone()),
        );

        let state = session.state.clone();
        let can_exec = session.can_execute();
        (assembled, state, can_exec)
    };

    Ok(Json(ChatResponse {
        message: reasoning.unwrap_or_else(|| {
            if all_valid && !intents.is_empty() {
                format!(
                    "Extracted {} operation(s). DSL ready to execute.",
                    intents.len()
                )
            } else if !intents.is_empty() {
                "Some operations could not be validated.".to_string()
            } else {
                "No operations extracted.".to_string()
            }
        }),
        intents,
        validation_results: validations,
        assembled_dsl: assembled,
        session_state,
        can_execute,
    }))
}

/// POST /api/session/:id/execute
/// Execute all accumulated DSL in the session using the real Runtime
async fn execute_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Get accumulated DSL and session context
    let (accumulated_dsl, cbu_id, current_state) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        (
            session.assembled_dsl.clone(),
            session.context.last_cbu_id,
            session.state.clone(),
        )
    };

    if accumulated_dsl.is_empty() {
        return Ok(Json(ExecuteResponse {
            success: false,
            results: Vec::new(),
            errors: vec!["No DSL to execute".to_string()],
            new_state: current_state,
        }));
    }

    let parser = NomDslParser::new();
    let mut results = Vec::new();
    let mut all_success = true;
    let mut errors = Vec::new();

    // Create a RuntimeEnv with database connection for real execution
    let request_id = OnboardingRequestId(format!("session-{}", session_id));
    let mut env = RuntimeEnv::with_pool(request_id, state.pool.clone());

    // Set CBU ID if we have one from previous executions
    if let Some(id) = cbu_id {
        env.set_cbu_id(id);
    }

    // Execute each DSL statement
    for (idx, dsl) in accumulated_dsl.iter().enumerate() {
        // First validate
        let validation = validate_dsl_internal(dsl, &state.runtime);

        if !validation.valid {
            all_success = false;
            let error_msg = validation
                .errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            errors.push(error_msg.clone());
            results.push(ExecutionResult {
                statement_index: idx,
                dsl: dsl.clone(),
                success: false,
                message: format!("Validation failed: {}", error_msg),
                entity_id: None,
                entity_type: None,
            });
            continue;
        }

        // Parse DSL
        match parser.parse(dsl) {
            Ok(exprs) => {
                // Execute using the real Runtime
                match state.runtime.execute_sheet(&exprs, &mut env) {
                    Ok(()) => {
                        // Check if a new entity was created
                        let entity_id = env.entity_id;
                        // Determine entity type from verb
                        let entity_type = if dsl.contains("cbu.") {
                            Some("CBU".to_string())
                        } else if dsl.contains("entity.") {
                            Some("ENTITY".to_string())
                        } else {
                            None
                        };

                        results.push(ExecutionResult {
                            statement_index: idx,
                            dsl: dsl.clone(),
                            success: true,
                            message: "Executed successfully".to_string(),
                            entity_id,
                            entity_type,
                        });
                    }
                    Err(e) => {
                        all_success = false;
                        let error_msg = format!("Execution error: {}", e);
                        errors.push(error_msg.clone());
                        results.push(ExecutionResult {
                            statement_index: idx,
                            dsl: dsl.clone(),
                            success: false,
                            message: error_msg,
                            entity_id: None,
                            entity_type: None,
                        });
                    }
                }
            }
            Err(e) => {
                all_success = false;
                let error_msg = format!("Parse error: {}", e);
                errors.push(error_msg.clone());
                results.push(ExecutionResult {
                    statement_index: idx,
                    dsl: dsl.clone(),
                    success: false,
                    message: error_msg,
                    entity_id: None,
                    entity_type: None,
                });
            }
        }
    }

    // Update session state and get new state
    let new_state = {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            // Convert results to session format
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

            if all_success {
                session.add_agent_message(
                    format!("Successfully executed {} DSL statement(s)", results.len()),
                    None,
                    None,
                );
            } else {
                session.add_agent_message(
                    format!("Execution completed with errors: {}", errors.join("; ")),
                    None,
                    None,
                );
            }

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

/// POST /api/session/:id/clear
/// Clear accumulated DSL but keep the session
async fn clear_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    session.clear_assembled_dsl();
    session.add_agent_message("Accumulated DSL cleared.".to_string(), None, None);

    Ok(Json(SessionStateResponse {
        session_id: session.id,
        state: session.state.clone(),
        message_count: session.messages.len(),
        pending_intents: session.pending_intents.clone(),
        assembled_dsl: session.assembled_dsl.clone(),
        combined_dsl: session.combined_dsl(),
        context: session.context.clone(),
        messages: session.messages.clone(),
        can_execute: session.can_execute(),
    }))
}

// ============================================================================
// Helper Functions
// ============================================================================

fn validate_dsl_internal(
    dsl: &str,
    runtime: &crate::forth_engine::runtime::Runtime,
) -> ValidationResult {
    let parser = NomDslParser::new();

    // Try to parse
    match parser.parse(dsl) {
        Ok(exprs) => {
            let mut errors = Vec::new();
            let warnings = Vec::new();

            // Validate verbs exist in vocabulary
            for expr in &exprs {
                if let Some(verb) = extract_verb_from_expr(expr) {
                    if runtime.get_word(&verb).is_none() {
                        errors.push(ValidationError {
                            code: "E001".to_string(),
                            message: format!("Unknown verb: {}", verb),
                            line: None,
                            column: None,
                        });
                    }
                }
            }

            ValidationResult {
                valid: errors.is_empty(),
                errors,
                warnings,
            }
        }
        Err(e) => ValidationResult {
            valid: false,
            errors: vec![ValidationError {
                code: "E000".to_string(),
                message: format!("Parse error: {}", e),
                line: None,
                column: None,
            }],
            warnings: vec![],
        },
    }
}

fn extract_verb_from_expr(expr: &crate::forth_engine::ast::Expr) -> Option<String> {
    use crate::forth_engine::ast::Expr;

    match expr {
        Expr::WordCall { name, .. } => Some(name.clone()),
        _ => None,
    }
}

fn extract_verbs(dsl: &str) -> Vec<String> {
    let parser = NomDslParser::new();
    let mut verbs = Vec::new();

    if let Ok(exprs) = parser.parse(dsl) {
        for expr in &exprs {
            if let Some(verb) = extract_verb_from_expr(expr) {
                verbs.push(verb);
            }
        }
    }
    verbs
}

fn count_domain_verbs(
    runtime: &crate::forth_engine::runtime::Runtime,
    domain_prefix: &str,
) -> usize {
    runtime.get_domain_words(domain_prefix).len()
}

// ============================================================================
// Router Creation
// ============================================================================

/// Create the agent router with all endpoints
pub fn create_agent_router(pool: PgPool) -> Router {
    let state = AgentState::new(pool);

    Router::new()
        // Agent DSL Generation
        .route("/api/agent/generate", post(generate_dsl))
        .route("/api/agent/validate", post(validate_dsl))
        .route("/api/agent/domains", get(list_domains))
        .route("/api/agent/vocabulary", get(get_vocabulary))
        .route("/api/agent/health", get(agent_health))
        // Session Management (Intent-based pipeline)
        .route("/api/session", post(create_session))
        .route("/api/session/{id}", get(get_session_state))
        .route("/api/session/{id}", delete(delete_session))
        .route("/api/session/{id}/chat", post(chat_in_session))
        .route("/api/session/{id}/execute", post(execute_session_dsl))
        .route("/api/session/{id}/clear", post(clear_session_dsl))
        .with_state(state)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_verbs() {
        let dsl = r#"
            (cbu.ensure :cbu-name "Test")
            (entity.create-limited-company :name "TestCo")
        "#;
        let verbs = extract_verbs(dsl);
        assert_eq!(verbs.len(), 2);
        assert!(verbs.contains(&"cbu.ensure".to_string()));
        assert!(verbs.contains(&"entity.create-limited-company".to_string()));
    }
}

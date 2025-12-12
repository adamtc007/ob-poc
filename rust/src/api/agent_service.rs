//! Agent Service - Centralized agent conversation logic
//!
//! This module provides a single service for all agent chat operations.
//! It implements a **deterministic pipeline** that constrains LLM output
//! to valid, executable DSL.
//!
//! ## Two DSL Generation Tools - Same Foundation
//!
//! The system has TWO tools for generating DSL, both sharing the same core infrastructure:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                        DSL GENERATION TOOLS                              │
//! ├─────────────────────────────────┬───────────────────────────────────────┤
//! │   LSP (Zed/VS Code Editor)      │   AgentService (Chat UI)              │
//! │   dsl-lsp/src/handlers/         │   api/agent_service.rs                │
//! │   - Autocomplete as you type    │   - Natural language → DSL            │
//! │   - Diagnostics (red squiggles) │   - Disambiguation dialogs            │
//! │   - Hover documentation         │   - Multi-turn conversation           │
//! └─────────────────────────────────┴───────────────────────────────────────┘
//!                        │                           │
//!                        └─────────────┬─────────────┘
//!                                      ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                     SHARED FOUNDATION                                    │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  EntityGateway (gRPC :50051)                                            │
//! │  - Fuzzy search → exact UUID resolution                                 │
//! │  - In-memory Tantivy indexes for sub-ms response                        │
//! │  - Same entity lookups for LSP autocomplete AND agent resolution        │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  Verb Registry (config/verbs/*.yaml)                                    │
//! │  - Single source of truth for all DSL verbs                             │
//! │  - LSP uses it for keyword completion                                   │
//! │  - Agent uses it to constrain LLM output                                │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  CSG Linter (csg_rules.yaml)                                            │
//! │  - Context-sensitive grammar validation                                 │
//! │  - LSP shows diagnostics in editor                                      │
//! │  - Agent uses errors to retry with LLM feedback                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Architecture: Constrained Agent Pipeline
//!
//! ```text
//! User Message (natural language)
//!       │
//!       ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  LLM Intent Extraction (with tool_use)                      │
//! │  - Constrained to DSL verbs from YAML registry              │
//! │  - Returns structured VerbIntent, NOT raw DSL               │
//! │  - Entity references go to "lookups" field                  │
//! └─────────────────────────────────────────────────────────────┘
//!       │
//!       ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  EntityGateway Resolution (DETERMINISTIC)                   │
//! │  *** SAME SERVICE USED BY LSP AUTOCOMPLETE ***              │
//! │  - Fuzzy search → exact UUID resolution                     │
//! │  - Single match → auto-resolve                              │
//! │  - Multiple matches → disambiguation UI                     │
//! │  - No match → error or create new entity                    │
//! └─────────────────────────────────────────────────────────────┘
//!       │
//!       ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  DSL Builder (DETERMINISTIC Rust code)                      │
//! │  - VerbIntent + resolved UUIDs → DSL source                 │
//! │  - No LLM involved - pure Rust code                         │
//! └─────────────────────────────────────────────────────────────┘
//!       │
//!       ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Semantic Validation (CSG Linter)                           │
//! │  *** SAME RULES AS LSP DIAGNOSTICS ***                      │
//! │  - If errors → feed back to LLM and retry (max 3x)          │
//! └─────────────────────────────────────────────────────────────┘
//!       │
//!       ▼
//! Valid DSL ready for execution
//! ```
//!
//! ## Key Design Decisions
//!
//! 1. **LLM outputs structured intents, not DSL text** - Prevents syntax errors
//! 2. **EntityGateway resolves all entity references** - Same as LSP, prevents UUID hallucination
//! 3. **Disambiguation is user-driven** - No guessing when multiple matches exist
//! 4. **DSL builder is pure Rust** - Deterministic, testable
//! 5. **Retry loop with linter feedback** - Self-healing for semantic errors
//!
//! ## Integration Points
//!
//! | Component | LSP Usage | Agent Usage |
//! |-----------|-----------|-------------|
//! | EntityGateway | `complete_keyword_values()` autocomplete | `resolve_lookups()` entity resolution |
//! | Verb Registry | `complete_verb_names()`, `complete_keywords()` | LLM prompt vocabulary, intent validation |
//! | CSG Linter | `diagnostics.rs` red squiggles | `run_semantic_validation()` retry feedback |
//! | Parser | Real-time syntax check | Post-generation validation |
//!
//! Both `agentic_server` and `ob-poc-web` should use this service.

use crate::agentic::llm_client::{LlmClient, ToolDefinition};
use crate::api::dsl_builder::{build_dsl_program, validate_intent};
use crate::api::intent::{IntentValidation, ParamValue, VerbIntent};
use crate::api::session::{
    AgentSession, DisambiguationItem, DisambiguationRequest, EntityMatchOption, SessionState,
};
use crate::dsl_v2::gateway_resolver::{gateway_addr, GatewayRefResolver};
use crate::dsl_v2::ref_resolver::ResolveResult;
use crate::dsl_v2::semantic_validator::SemanticValidator;
use crate::dsl_v2::validation::{RefType, Severity, ValidationContext, ValidationRequest};
use crate::dsl_v2::verb_registry::registry;
use crate::dsl_v2::{parse_program, Statement};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Service Types
// ============================================================================

/// Lookup info extracted from LLM intent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityLookup {
    pub search_text: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub jurisdiction_hint: Option<String>,
}

/// Result of resolving entity lookups
#[derive(Debug)]
pub enum LookupResolution {
    /// All lookups resolved to exactly one entity
    Resolved(HashMap<String, Uuid>),
    /// Some lookups are ambiguous - need disambiguation
    Ambiguous(Vec<DisambiguationItem>),
    /// Error during lookup
    Error(String),
}

/// Chat request with optional disambiguation response
#[derive(Debug, Clone, Deserialize)]
pub struct AgentChatRequest {
    /// User's message
    pub message: String,
    /// Optional CBU context
    #[serde(default)]
    pub cbu_id: Option<Uuid>,
    /// Optional disambiguation response (if responding to disambiguation request)
    #[serde(default)]
    pub disambiguation_response: Option<crate::api::session::DisambiguationResponse>,
}

/// Extended chat response that includes disambiguation status
#[derive(Debug, Serialize)]
pub struct AgentChatResponse {
    /// Agent's response message
    pub message: String,
    /// Extracted intents
    pub intents: Vec<VerbIntent>,
    /// Validation results for each intent
    pub validation_results: Vec<IntentValidation>,
    /// Current session state
    pub session_state: SessionState,
    /// Whether the session can execute
    pub can_execute: bool,
    /// DSL source rendered from AST (for display in UI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl_source: Option<String>,
    /// The full AST for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<Vec<Statement>>,
    /// Disambiguation request if needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<DisambiguationRequest>,
    /// UI commands (show CBU, highlight entity, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<AgentCommand>>,
}

/// Commands the agent can issue to the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentCommand {
    /// Show a specific CBU in the graph
    ShowCbu { cbu_id: String },
    /// Highlight an entity in the graph
    HighlightEntity { entity_id: String },
    /// Navigate to a line in the DSL panel
    NavigateDsl { line: u32 },
}

/// Configuration for the agent service
#[derive(Debug, Clone)]
pub struct AgentServiceConfig {
    /// Maximum retries for DSL generation with validation
    pub max_retries: usize,
    /// EntityGateway address
    pub gateway_addr: String,
}

impl Default for AgentServiceConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            gateway_addr: gateway_addr(),
        }
    }
}

// ============================================================================
// Agent Service
// ============================================================================

/// Centralized agent service for all conversation logic
///
/// This service handles:
/// - Intent extraction from natural language via LLM
/// - Entity resolution via EntityGateway (with disambiguation)
/// - DSL generation with semantic validation
/// - Retry loop for fixing validation errors
///
/// Usage:
/// ```ignore
/// let service = AgentService::new(pool);
/// let response = service.process_chat(&mut session, &request, llm_client).await?;
/// ```
pub struct AgentService {
    /// Database pool for semantic validation
    pool: Option<PgPool>,
    /// Configuration
    config: AgentServiceConfig,
}

impl AgentService {
    /// Create a new agent service without database support
    pub fn new() -> Self {
        Self {
            pool: None,
            config: AgentServiceConfig::default(),
        }
    }

    /// Create with database pool for semantic validation
    pub fn with_pool(pool: PgPool) -> Self {
        Self {
            pool: Some(pool),
            config: AgentServiceConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(pool: Option<PgPool>, config: AgentServiceConfig) -> Self {
        Self { pool, config }
    }

    /// Process a chat message and return response (with disambiguation if needed)
    ///
    /// This is the main entry point for agent chat. It:
    /// 1. Checks for special commands (show/load/select CBU)
    /// 2. Handles disambiguation responses
    /// 3. Extracts intents from natural language via LLM
    /// 4. Resolves entity references via EntityGateway
    /// 5. Validates and generates DSL with retry loop
    pub async fn process_chat(
        &self,
        session: &mut AgentSession,
        request: &AgentChatRequest,
        llm_client: Arc<dyn LlmClient>,
    ) -> Result<AgentChatResponse, String> {
        // Check for "show CBU" command first
        if let Some(cmd_response) = self.handle_show_command(&request.message).await? {
            return Ok(cmd_response);
        }

        // If this is a disambiguation response, handle it
        if let Some(disambig_response) = &request.disambiguation_response {
            return self
                .handle_disambiguation_response(session, disambig_response, llm_client)
                .await;
        }

        // Store user message
        session.add_user_message(request.message.clone());

        // Get session context for LLM
        let session_bindings = session.context.bindings_for_llm();
        let session_named_refs = session.context.named_refs.clone();

        // Build prompts
        let vocab = self.build_vocab_prompt(None);
        let system_prompt = self.build_intent_extraction_prompt(&vocab);

        let bindings_context = if !session_bindings.is_empty() {
            format!(
                "\n\n[SESSION CONTEXT: Available references from previous commands: {}. Use these exact @names in the refs field when referring to these entities.]",
                session_bindings.join(", ")
            )
        } else {
            String::new()
        };

        let user_message = format!("{}{}", request.message, bindings_context);

        // Define tool for intent extraction with entity lookups
        let tool = self.build_intent_tool();

        // Retry loop with validation feedback
        let mut feedback_context = String::new();
        let mut final_dsl: Option<String> = None;
        let mut final_explanation = String::new();
        let mut all_intents: Vec<VerbIntent> = Vec::new();
        let mut validation_results: Vec<IntentValidation> = Vec::new();

        for attempt in 0..self.config.max_retries {
            // Build message with optional feedback from previous attempt
            let attempt_message = if feedback_context.is_empty() {
                user_message.clone()
            } else {
                format!(
                    "{}\n\n[LINTER FEEDBACK - Please fix these issues]\n{}",
                    user_message, feedback_context
                )
            };

            // Call LLM with tool use for structured intent extraction
            let tool_result = match llm_client
                .chat_with_tool(&system_prompt, &attempt_message, &tool)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    tracing::error!("LLM API error (attempt {}): {}", attempt + 1, e);
                    if attempt == self.config.max_retries - 1 {
                        return Err(format!("LLM API error: {}", e));
                    }
                    continue;
                }
            };

            // Parse intents and explanation
            let intents: Vec<VerbIntent> =
                serde_json::from_value(tool_result.arguments["intents"].clone())
                    .unwrap_or_default();

            let explanation = tool_result.arguments["explanation"]
                .as_str()
                .unwrap_or("")
                .to_string();

            if intents.is_empty() {
                if attempt < self.config.max_retries - 1 {
                    feedback_context = "Could not extract any DSL intents. Please try again with clearer verb and parameter names.".to_string();
                    continue;
                }
                break;
            }

            // Collect all lookups from intents
            let all_lookups = self.collect_lookups(&intents);

            // Resolve lookups via EntityGateway
            let resolution = self.resolve_lookups(&all_lookups).await;

            match resolution {
                LookupResolution::Ambiguous(items) => {
                    // Need disambiguation - store intents in session for retrieval after user selection
                    // This is critical: the session persists intents so disambiguation response can use them
                    session.pending_intents = intents.clone();

                    let disambig = DisambiguationRequest {
                        request_id: Uuid::new_v4(),
                        items,
                        prompt: "Please select the correct entities:".to_string(),
                        original_intents: Some(intents), // Also include in response for stateless clients
                    };

                    session.add_agent_message(explanation.clone(), None, None);
                    session.state = SessionState::PendingValidation;

                    return Ok(AgentChatResponse {
                        message: explanation,
                        intents: vec![],
                        validation_results: vec![],
                        session_state: SessionState::PendingValidation,
                        can_execute: false,
                        dsl_source: None,
                        ast: None,
                        disambiguation: Some(disambig),
                        commands: None,
                    });
                }
                LookupResolution::Error(msg) => {
                    return Err(msg);
                }
                LookupResolution::Resolved(resolved_ids) => {
                    // All resolved - inject UUIDs and build DSL
                    let modified_intents = self.inject_resolved_ids(intents, &resolved_ids);

                    // Validate intents against registry
                    validation_results.clear();
                    let mut has_errors = false;
                    let mut error_feedback = Vec::new();

                    for intent in &modified_intents {
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

                    // Build DSL from intents
                    let dsl = build_dsl_program(&modified_intents);
                    tracing::debug!("[CHAT] Built DSL from intents: {}", dsl);

                    // Run semantic validation if we have a database pool
                    if let Some(ref pool) = self.pool {
                        if let Some(errors) = self
                            .run_semantic_validation(pool, &dsl, &session_named_refs)
                            .await
                        {
                            has_errors = true;
                            error_feedback.extend(errors);
                        }
                    }

                    // If no errors, we're done
                    if !has_errors {
                        tracing::info!(
                            "Validation passed on attempt {}, DSL length: {}",
                            attempt + 1,
                            dsl.len()
                        );
                        final_dsl = Some(dsl);
                        final_explanation = explanation;
                        all_intents = modified_intents;
                        break;
                    } else {
                        tracing::warn!(
                            "Validation failed on attempt {}: {:?}",
                            attempt + 1,
                            error_feedback
                        );
                    }

                    // Build feedback for next attempt
                    if attempt < self.config.max_retries - 1 {
                        feedback_context = error_feedback.join("\n");
                    } else {
                        // Last attempt - return what we have with errors
                        final_dsl = Some(dsl);
                        final_explanation = format!(
                            "{}\n\nNote: DSL has validation issues:\n{}",
                            explanation,
                            error_feedback.join("\n")
                        );
                        all_intents = modified_intents;
                    }
                }
            }
        }

        // Build final response
        self.build_response(
            session,
            all_intents,
            validation_results,
            final_dsl,
            final_explanation,
        )
        .await
    }

    /// Run semantic validation on DSL
    async fn run_semantic_validation(
        &self,
        pool: &PgPool,
        dsl: &str,
        known_symbols: &HashMap<String, Uuid>,
    ) -> Option<Vec<String>> {
        let validator_result = async {
            let v = SemanticValidator::new(pool.clone()).await?;
            v.with_csg_linter().await
        }
        .await;

        match validator_result {
            Ok(mut validator) => {
                let request = ValidationRequest {
                    source: dsl.to_string(),
                    context: ValidationContext::default().with_known_symbols(known_symbols.clone()),
                };

                match validator.validate(&request).await {
                    crate::dsl_v2::validation::ValidationResult::Err(diagnostics) => {
                        let errors: Vec<String> = diagnostics
                            .iter()
                            .filter(|d| {
                                // Skip EntityGateway connection/config errors
                                d.severity == Severity::Error
                                    && !d.message.contains("EntityGateway")
                                    && !d.message.contains("Unknown entity type")
                            })
                            .map(|d| format!("Validation: {}", d.message))
                            .collect();

                        if errors.is_empty() {
                            None
                        } else {
                            Some(errors)
                        }
                    }
                    crate::dsl_v2::validation::ValidationResult::Ok(_) => None,
                }
            }
            Err(e) => {
                tracing::warn!("Semantic validation unavailable: {}", e);
                None
            }
        }
    }

    /// Handle disambiguation response from user
    async fn handle_disambiguation_response(
        &self,
        session: &mut AgentSession,
        response: &crate::api::session::DisambiguationResponse,
        _llm_client: Arc<dyn LlmClient>,
    ) -> Result<AgentChatResponse, String> {
        use crate::api::session::DisambiguationSelection;

        // Build resolved IDs from user's selections
        let mut resolved_ids: HashMap<String, Uuid> = HashMap::new();
        for selection in &response.selections {
            match selection {
                DisambiguationSelection::Entity { param, entity_id } => {
                    resolved_ids.insert(param.clone(), *entity_id);
                }
                DisambiguationSelection::Interpretation { .. } => {
                    // Handle interpretation selections if needed
                }
            }
        }

        // Get original intents from session's pending disambiguation
        let original_intents = session.pending_intents.clone();

        if original_intents.is_empty() {
            return Err("No original intents available for disambiguation".to_string());
        }

        // Inject resolved IDs and build response
        let modified_intents = self.inject_resolved_ids(original_intents, &resolved_ids);

        // Validate
        let validation_results: Vec<IntentValidation> =
            modified_intents.iter().map(validate_intent).collect();

        let all_valid = validation_results.iter().all(|v| v.valid);

        // Build DSL
        let dsl = if !modified_intents.is_empty() && all_valid {
            Some(build_dsl_program(&modified_intents))
        } else {
            None
        };

        self.build_response(
            session,
            modified_intents,
            validation_results,
            dsl,
            "Entities resolved. DSL ready for execution.".to_string(),
        )
        .await
    }

    /// Handle "show/load/select CBU" commands
    async fn handle_show_command(
        &self,
        message: &str,
    ) -> Result<Option<AgentChatResponse>, String> {
        let lower_msg = message.to_lowercase();
        if !lower_msg.starts_with("show ")
            && !lower_msg.starts_with("load ")
            && !lower_msg.starts_with("select ")
        {
            return Ok(None);
        }

        let search_term = message
            .split_whitespace()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ");

        if search_term.is_empty() {
            return Ok(None);
        }

        // Use EntityGateway for CBU search
        let mut resolver = match GatewayRefResolver::connect(&self.config.gateway_addr).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Gateway connection failed: {}", e);
                return Ok(Some(AgentChatResponse {
                    message: "Could not search for CBU: gateway unavailable".to_string(),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: SessionState::New,
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: None,
                }));
            }
        };

        // Search for CBU
        let result = resolver
            .resolve(RefType::Cbu, &search_term)
            .await
            .map_err(|e| format!("CBU search failed: {}", e))?;

        match result {
            ResolveResult::Found { id, display } => Ok(Some(AgentChatResponse {
                message: format!("Showing {}", display),
                intents: vec![],
                validation_results: vec![],
                session_state: SessionState::New,
                can_execute: false,
                dsl_source: None,
                ast: None,
                disambiguation: None,
                commands: Some(vec![AgentCommand::ShowCbu {
                    cbu_id: id.to_string(),
                }]),
            })),
            ResolveResult::FoundByCode { display, uuid, .. } => {
                let cbu_id = uuid.map(|u| u.to_string()).unwrap_or_default();
                Ok(Some(AgentChatResponse {
                    message: format!("Showing {}", display),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: SessionState::New,
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: if !cbu_id.is_empty() {
                        Some(vec![AgentCommand::ShowCbu { cbu_id }])
                    } else {
                        None
                    },
                }))
            }
            ResolveResult::NotFound { suggestions } if !suggestions.is_empty() => {
                // Multiple suggestions - return disambiguation
                let matches: Vec<EntityMatchOption> = suggestions
                    .into_iter()
                    .filter_map(|s| {
                        Uuid::parse_str(&s.value).ok().map(|id| EntityMatchOption {
                            entity_id: id,
                            name: s.display,
                            entity_type: "cbu".to_string(),
                            jurisdiction: None,
                            context: None,
                            score: Some(s.score),
                        })
                    })
                    .collect();

                if matches.is_empty() {
                    Ok(Some(AgentChatResponse {
                        message: format!("No CBU found matching '{}'", search_term),
                        intents: vec![],
                        validation_results: vec![],
                        session_state: SessionState::New,
                        can_execute: false,
                        dsl_source: None,
                        ast: None,
                        disambiguation: None,
                        commands: None,
                    }))
                } else {
                    let disambig = DisambiguationRequest {
                        request_id: Uuid::new_v4(),
                        items: vec![DisambiguationItem::EntityMatch {
                            param: "cbu_id".to_string(),
                            search_text: search_term.clone(),
                            matches,
                        }],
                        prompt: format!("Which CBU did you mean by '{}'?", search_term),
                        original_intents: None,
                    };

                    Ok(Some(AgentChatResponse {
                        message: format!(
                            "Multiple CBUs match '{}'. Please select one:",
                            search_term
                        ),
                        intents: vec![],
                        validation_results: vec![],
                        session_state: SessionState::PendingValidation,
                        can_execute: false,
                        dsl_source: None,
                        ast: None,
                        disambiguation: Some(disambig),
                        commands: None,
                    }))
                }
            }
            ResolveResult::NotFound { .. } => Ok(Some(AgentChatResponse {
                message: format!("No CBU found matching '{}'", search_term),
                intents: vec![],
                validation_results: vec![],
                session_state: SessionState::New,
                can_execute: false,
                dsl_source: None,
                ast: None,
                disambiguation: None,
                commands: None,
            })),
        }
    }

    /// Collect all entity lookups from intents
    fn collect_lookups(&self, intents: &[VerbIntent]) -> HashMap<String, EntityLookup> {
        let mut lookups = HashMap::new();
        for intent in intents {
            if let Some(intent_lookups) = &intent.lookups {
                for (param, lookup_val) in intent_lookups {
                    if let Ok(info) = serde_json::from_value::<EntityLookup>(lookup_val.clone()) {
                        lookups.insert(param.clone(), info);
                    }
                }
            }
        }
        lookups
    }

    /// Resolve entity lookups via EntityGateway
    async fn resolve_lookups(&self, lookups: &HashMap<String, EntityLookup>) -> LookupResolution {
        if lookups.is_empty() {
            return LookupResolution::Resolved(HashMap::new());
        }

        let mut resolver = match GatewayRefResolver::connect(&self.config.gateway_addr).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("EntityGateway not available: {}", e);
                return LookupResolution::Resolved(HashMap::new());
            }
        };

        let mut resolved: HashMap<String, Uuid> = HashMap::new();
        let mut ambiguous: Vec<DisambiguationItem> = Vec::new();

        for (param, lookup) in lookups {
            // Determine ref type from entity_type hint
            let ref_type = match lookup.entity_type.as_deref() {
                Some("person") | Some("proper_person") => RefType::Entity,
                Some("company") | Some("limited_company") | Some("legal_entity") => RefType::Entity,
                Some("cbu") => RefType::Cbu,
                Some("product") => RefType::Product,
                Some("role") => RefType::Role,
                Some("jurisdiction") => RefType::Jurisdiction,
                _ => RefType::Entity,
            };

            match resolver.resolve(ref_type, &lookup.search_text).await {
                Ok(ResolveResult::Found { id, .. }) => {
                    resolved.insert(param.clone(), id);
                }
                Ok(ResolveResult::FoundByCode { uuid: Some(id), .. }) => {
                    resolved.insert(param.clone(), id);
                }
                Ok(ResolveResult::FoundByCode { uuid: None, .. }) => {
                    tracing::debug!(
                        "Found by code but no UUID for '{}' (param: {})",
                        lookup.search_text,
                        param
                    );
                }
                Ok(ResolveResult::NotFound { suggestions }) if !suggestions.is_empty() => {
                    let matches: Vec<EntityMatchOption> = suggestions
                        .into_iter()
                        .filter_map(|s| {
                            Uuid::parse_str(&s.value).ok().map(|id| EntityMatchOption {
                                entity_id: id,
                                name: s.display,
                                entity_type: lookup
                                    .entity_type
                                    .clone()
                                    .unwrap_or_else(|| "entity".to_string()),
                                jurisdiction: None,
                                context: None,
                                score: Some(s.score),
                            })
                        })
                        .collect();

                    if !matches.is_empty() {
                        ambiguous.push(DisambiguationItem::EntityMatch {
                            param: param.clone(),
                            search_text: lookup.search_text.clone(),
                            matches,
                        });
                    }
                }
                Ok(ResolveResult::NotFound { .. }) => {
                    tracing::debug!("No matches for '{}' (param: {})", lookup.search_text, param);
                }
                Err(e) => {
                    return LookupResolution::Error(format!("Lookup failed: {}", e));
                }
            }
        }

        if ambiguous.is_empty() {
            LookupResolution::Resolved(resolved)
        } else {
            LookupResolution::Ambiguous(ambiguous)
        }
    }

    /// Inject resolved entity IDs into intents
    fn inject_resolved_ids(
        &self,
        mut intents: Vec<VerbIntent>,
        resolved_ids: &HashMap<String, Uuid>,
    ) -> Vec<VerbIntent> {
        for intent in &mut intents {
            for (param, entity_id) in resolved_ids {
                intent
                    .params
                    .insert(param.clone(), ParamValue::Uuid(*entity_id));
            }
            intent.lookups = None;
        }
        intents
    }

    /// Build final response with DSL
    async fn build_response(
        &self,
        session: &mut AgentSession,
        intents: Vec<VerbIntent>,
        validation_results: Vec<IntentValidation>,
        dsl: Option<String>,
        explanation: String,
    ) -> Result<AgentChatResponse, String> {
        let all_valid = validation_results.iter().all(|v| v.valid);

        // Parse to AST
        let ast: Option<Vec<Statement>> = dsl
            .as_ref()
            .and_then(|src| parse_program(src).ok().map(|prog| prog.statements));

        // Update session
        if let Some(ref dsl_source) = dsl {
            if let Some(ref ast_statements) = ast {
                session.set_pending_dsl(dsl_source.clone(), ast_statements.clone());
            }
            session.assembled_dsl = vec![dsl_source.clone()];

            if all_valid {
                session.state = SessionState::ReadyToExecute;
            } else {
                session.state = SessionState::PendingValidation;
            }
        }

        session.add_agent_message(explanation.clone(), None, dsl.clone());

        let combined_dsl = if session.assembled_dsl.is_empty() {
            None
        } else {
            Some(session.assembled_dsl.join("\n\n"))
        };

        Ok(AgentChatResponse {
            message: explanation,
            intents,
            validation_results,
            session_state: session.state.clone(),
            can_execute: session.can_execute() && all_valid,
            dsl_source: combined_dsl,
            ast,
            disambiguation: None,
            commands: None,
        })
    }

    /// Build vocabulary prompt from verb registry
    fn build_vocab_prompt(&self, domain_filter: Option<&str>) -> String {
        let reg = registry();
        let mut lines = Vec::new();

        for verb in reg.all_verbs() {
            if let Some(domain) = domain_filter {
                if verb.domain != domain {
                    continue;
                }
            }

            let required: Vec<_> = verb
                .args
                .iter()
                .filter(|a| a.required)
                .map(|a| a.name.as_str())
                .collect();
            let optional: Vec<_> = verb
                .args
                .iter()
                .filter(|a| !a.required)
                .map(|a| a.name.as_str())
                .collect();

            lines.push(format!(
                "- {}.{}: {} [required: {:?}] [optional: {:?}]",
                verb.domain, verb.verb, verb.description, required, optional
            ));
        }

        lines.join("\n")
    }

    /// Build system prompt for intent extraction
    fn build_intent_extraction_prompt(&self, vocab: &str) -> String {
        format!(
            r#"You are a DSL intent extraction assistant. Convert natural language to structured DSL intents.

IMPORTANT: You MUST use the generate_dsl_intents tool to return your response. Do NOT return plain text.

## Available DSL Verbs

{}

## Intent Structure

Each intent represents a single DSL verb call with:
- verb: The verb name (e.g., "cbu.ensure", "entity.create-proper-person")
- params: Literal parameter values (e.g., {{"name": "Acme Corp", "jurisdiction": "LU"}})
- refs: References to previous results (e.g., {{"cbu-id": "@result_1"}})
- lookups: Entity references needing database resolution

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

## Entity Lookups

When the user mentions existing entities by name:
- Use the "lookups" field to request entity resolution
- Provide search_text (the name) and entity_type (person, company, cbu, entity)
- If jurisdiction is mentioned, include jurisdiction_hint

Example: "Add John Smith as director of Apex Capital"
- For "John Smith": lookups.entity-id = {{search_text: "John Smith", entity_type: "person"}}
- For "Apex Capital": lookups.cbu-id = {{search_text: "Apex Capital", entity_type: "cbu"}}

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
}}"#,
            vocab
        )
    }

    /// Build tool definition for intent extraction
    fn build_intent_tool(&self) -> ToolDefinition {
        ToolDefinition {
            name: "generate_dsl_intents".to_string(),
            description: "Generate structured DSL intents from user request. Use 'lookups' for entity references that need database resolution.".to_string(),
            parameters: serde_json::json!({
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
                                },
                                "lookups": {
                                    "type": "object",
                                    "description": "Entity lookups needing resolution. Key is param name, value is {search_text, entity_type, jurisdiction_hint}",
                                    "additionalProperties": {
                                        "type": "object",
                                        "properties": {
                                            "search_text": { "type": "string", "description": "The name/text to search for" },
                                            "entity_type": { "type": "string", "description": "Type: person, company, cbu, entity, product, role" },
                                            "jurisdiction_hint": { "type": "string", "description": "Optional jurisdiction filter (e.g., 'UK', 'US')" }
                                        },
                                        "required": ["search_text"]
                                    }
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
            }),
        }
    }

    // ========================================================================
    // Public Entity Resolution API
    // ========================================================================
    // These methods expose EntityGateway functionality directly for UI components
    // that need entity search/autocomplete without going through the LLM.

    /// Search for entities by type and query string
    ///
    /// This is a direct passthrough to EntityGateway for UI autocomplete.
    /// Returns up to `limit` matches with fuzzy search.
    pub async fn search_entities(
        &self,
        entity_type: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatchOption>, String> {
        let ref_type = match entity_type {
            "cbu" => RefType::Cbu,
            "entity" | "person" | "company" => RefType::Entity,
            "product" => RefType::Product,
            "role" => RefType::Role,
            "jurisdiction" => RefType::Jurisdiction,
            "currency" => RefType::Currency,
            _ => RefType::Entity,
        };

        let mut resolver = GatewayRefResolver::connect(&self.config.gateway_addr)
            .await
            .map_err(|e| format!("Gateway connection failed: {}", e))?;

        let matches = resolver
            .search_fuzzy(ref_type, query, limit)
            .await
            .map_err(|e| format!("Search failed: {}", e))?;

        Ok(matches
            .into_iter()
            .filter_map(|m| {
                Uuid::parse_str(&m.value).ok().map(|id| EntityMatchOption {
                    entity_id: id,
                    name: m.display,
                    entity_type: entity_type.to_string(),
                    jurisdiction: None,
                    context: None,
                    score: Some(m.score),
                })
            })
            .collect())
    }

    /// Resolve a single entity by exact name match
    ///
    /// Returns the entity if exactly one match is found,
    /// or a list of suggestions if multiple/no matches.
    pub async fn resolve_entity(
        &self,
        entity_type: &str,
        name: &str,
    ) -> Result<ResolveResult, String> {
        let ref_type = match entity_type {
            "cbu" => RefType::Cbu,
            "entity" | "person" | "company" => RefType::Entity,
            "product" => RefType::Product,
            "role" => RefType::Role,
            "jurisdiction" => RefType::Jurisdiction,
            "currency" => RefType::Currency,
            _ => RefType::Entity,
        };

        let mut resolver = GatewayRefResolver::connect(&self.config.gateway_addr)
            .await
            .map_err(|e| format!("Gateway connection failed: {}", e))?;

        resolver
            .resolve(ref_type, name)
            .await
            .map_err(|e| format!("Resolution failed: {}", e))
    }

    /// Get all available DSL verbs (for UI verb picker / autocomplete)
    pub fn get_available_verbs(&self) -> Vec<VerbInfo> {
        let reg = registry();
        reg.all_verbs()
            .map(|v| VerbInfo {
                domain: v.domain.clone(),
                verb: v.verb.clone(),
                full_name: format!("{}.{}", v.domain, v.verb),
                description: v.description.clone(),
                required_args: v
                    .args
                    .iter()
                    .filter(|a| a.required)
                    .map(|a| a.name.clone())
                    .collect(),
                optional_args: v
                    .args
                    .iter()
                    .filter(|a| !a.required)
                    .map(|a| a.name.clone())
                    .collect(),
            })
            .collect()
    }
}

/// Information about a DSL verb (for UI display)
#[derive(Debug, Clone, Serialize)]
pub struct VerbInfo {
    pub domain: String,
    pub verb: String,
    pub full_name: String,
    pub description: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

impl Default for AgentService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_lookups() {
        let service = AgentService::new();

        let mut lookups_map = HashMap::new();
        lookups_map.insert(
            "entity-id".to_string(),
            serde_json::json!({
                "search_text": "John Smith",
                "entity_type": "person"
            }),
        );

        let intent = VerbIntent {
            verb: "cbu.assign-role".to_string(),
            params: HashMap::new(),
            refs: HashMap::new(),
            lookups: Some(lookups_map),
            sequence: None,
        };

        let collected = service.collect_lookups(&[intent]);
        assert_eq!(collected.len(), 1);
        assert!(collected.contains_key("entity-id"));
        assert_eq!(collected["entity-id"].search_text, "John Smith");
    }

    #[test]
    fn test_inject_resolved_ids() {
        let service = AgentService::new();

        let intent = VerbIntent {
            verb: "cbu.assign-role".to_string(),
            params: HashMap::new(),
            refs: HashMap::new(),
            lookups: Some(HashMap::new()),
            sequence: None,
        };

        let mut resolved = HashMap::new();
        let id = Uuid::new_v4();
        resolved.insert("entity-id".to_string(), id);

        let modified = service.inject_resolved_ids(vec![intent], &resolved);

        assert_eq!(modified.len(), 1);
        assert!(modified[0].params.contains_key("entity-id"));
        assert!(modified[0].lookups.is_none());
    }
}

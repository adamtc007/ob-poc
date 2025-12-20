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
use crate::dsl_v2::Statement;
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

// Re-export AgentCommand from ob_poc_types as the single source of truth
pub use ob_poc_types::AgentCommand;

/// Configuration for the agent service
#[derive(Debug, Clone)]
pub struct AgentServiceConfig {
    /// Maximum retries for DSL generation with validation
    pub max_retries: usize,
    /// EntityGateway address
    pub gateway_addr: String,
    /// Enable pre-resolution: query EntityGateway before LLM to provide available entities
    pub enable_pre_resolution: bool,
    /// Maximum entities to pre-fetch per type for context injection
    pub pre_resolution_limit: usize,
}

impl Default for AgentServiceConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            gateway_addr: gateway_addr(),
            enable_pre_resolution: true,
            pre_resolution_limit: 20,
        }
    }
}

/// Pre-resolved entities available for the LLM to reference
#[derive(Debug, Clone, Default)]
pub struct PreResolvedContext {
    /// Available CBUs (name -> UUID)
    pub cbus: Vec<(String, Uuid)>,
    /// Available entities (name -> UUID, type)
    pub entities: Vec<(String, Uuid, String)>,
    /// Available products (code -> name)
    pub products: Vec<(String, String)>,
    /// Available roles (code)
    pub roles: Vec<String>,
    /// Available jurisdictions (code -> name)
    pub jurisdictions: Vec<(String, String)>,
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

    /// Pre-resolve available entities from EntityGateway before LLM call
    ///
    /// This is Enhancement #1: Query EntityGateway upfront and inject available
    /// entities into the LLM prompt. The LLM can then only reference entities
    /// that actually exist, eliminating "entity not found" retries.
    async fn pre_resolve_entities(&self) -> PreResolvedContext {
        let mut context = PreResolvedContext::default();

        let mut resolver = match GatewayRefResolver::connect(&self.config.gateway_addr).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Pre-resolution: EntityGateway not available: {}", e);
                return context;
            }
        };

        let limit = self.config.pre_resolution_limit;

        // Fetch CBUs - most commonly referenced
        if let Ok(matches) = resolver.search_fuzzy(RefType::Cbu, "", limit).await {
            context.cbus = matches
                .into_iter()
                .filter_map(|m| Uuid::parse_str(&m.value).ok().map(|id| (m.display, id)))
                .collect();
            tracing::debug!("Pre-resolved {} CBUs", context.cbus.len());
        }

        // Fetch entities (persons, companies)
        if let Ok(matches) = resolver.search_fuzzy(RefType::Entity, "", limit).await {
            context.entities = matches
                .into_iter()
                .filter_map(|m| {
                    Uuid::parse_str(&m.value)
                        .ok()
                        .map(|id| (m.display.clone(), id, "entity".to_string()))
                })
                .collect();
            tracing::debug!("Pre-resolved {} entities", context.entities.len());
        }

        // Fetch products
        if let Ok(matches) = resolver.search_fuzzy(RefType::Product, "", limit).await {
            context.products = matches.into_iter().map(|m| (m.value, m.display)).collect();
            tracing::debug!("Pre-resolved {} products", context.products.len());
        }

        // Fetch roles
        if let Ok(matches) = resolver.search_fuzzy(RefType::Role, "", limit).await {
            context.roles = matches.into_iter().map(|m| m.value).collect();
            tracing::debug!("Pre-resolved {} roles", context.roles.len());
        }

        // Fetch jurisdictions
        if let Ok(matches) = resolver
            .search_fuzzy(RefType::Jurisdiction, "", limit)
            .await
        {
            context.jurisdictions = matches.into_iter().map(|m| (m.value, m.display)).collect();
            tracing::debug!("Pre-resolved {} jurisdictions", context.jurisdictions.len());
        }

        context
    }

    /// Format pre-resolved entities for injection into LLM prompt
    fn format_pre_resolved_context(&self, ctx: &PreResolvedContext) -> String {
        let mut sections = Vec::new();

        if !ctx.cbus.is_empty() {
            let cbu_list: Vec<String> = ctx
                .cbus
                .iter()
                .map(|(name, id)| format!("  - \"{}\" (id: {})", name, id))
                .collect();
            sections.push(format!(
                "## Existing CBUs (use exact names or IDs)\n{}",
                cbu_list.join("\n")
            ));
        }

        if !ctx.entities.is_empty() {
            let entity_list: Vec<String> = ctx
                .entities
                .iter()
                .take(15) // Limit to avoid prompt bloat
                .map(|(name, id, _)| format!("  - \"{}\" (id: {})", name, id))
                .collect();
            sections.push(format!("## Existing Entities\n{}", entity_list.join("\n")));
        }

        if !ctx.products.is_empty() {
            let product_list: Vec<String> = ctx
                .products
                .iter()
                .map(|(code, name)| format!("  - {} ({})", code, name))
                .collect();
            sections.push(format!(
                "## Available Products (use CODE)\n{}",
                product_list.join("\n")
            ));
        }

        if !ctx.roles.is_empty() {
            sections.push(format!("## Available Roles\n  {}", ctx.roles.join(", ")));
        }

        if !ctx.jurisdictions.is_empty() {
            let juris_list: Vec<String> = ctx
                .jurisdictions
                .iter()
                .take(20)
                .map(|(code, name)| format!("{} ({})", code, name))
                .collect();
            sections.push(format!("## Jurisdictions\n  {}", juris_list.join(", ")));
        }

        if sections.is_empty() {
            String::new()
        } else {
            format!(
                "\n\n# AVAILABLE DATA (use these exact values)\n\n{}",
                sections.join("\n\n")
            )
        }
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
        tracing::info!("=== AGENT SERVICE process_chat START ===");
        tracing::info!("User message: {:?}", request.message);
        tracing::info!("CBU ID: {:?}", request.cbu_id);
        tracing::info!("Session ID: {}", session.id);

        // Check for "show CBU" command first
        if let Some(cmd_response) = self.handle_show_command(&request.message).await? {
            tracing::info!("Handled as show command");
            return Ok(cmd_response);
        }

        // Check for DSL management commands (delete, undo, clear, execute)
        if let Some(cmd_response) = self.handle_dsl_command(session, &request.message).await? {
            tracing::info!("Handled as DSL command");
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

        // Enhancement #1: Pre-resolve available entities before LLM call
        // This injects available CBUs, entities, products, etc. into the prompt
        // so the LLM can only reference things that actually exist
        let pre_resolved = if self.config.enable_pre_resolution {
            self.pre_resolve_entities().await
        } else {
            PreResolvedContext::default()
        };
        let pre_resolved_context = self.format_pre_resolved_context(&pre_resolved);

        // Build prompts with pre-resolved data
        let vocab = self.build_vocab_prompt(None);
        let system_prompt = format!(
            "{}{}",
            self.build_intent_extraction_prompt(&vocab),
            pre_resolved_context
        );

        // Build session context for LLM - include active CBU and bindings
        let active_cbu_context = session.context.active_cbu_for_llm();
        let bindings_context = if !session_bindings.is_empty() || active_cbu_context.is_some() {
            let mut parts = Vec::new();
            if let Some(cbu) = active_cbu_context {
                parts.push(cbu);
            }
            if !session_bindings.is_empty() {
                parts.push(format!(
                    "Available references: {}",
                    session_bindings.join(", ")
                ));
            }
            format!(
                "\n\n[SESSION CONTEXT: {}. Use the active CBU for operations that need a CBU. Use exact @names in the refs field when referring to entities.]",
                parts.join(". ")
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

            // Check for clarification request first
            let needs_clarification = tool_result.arguments["needs_clarification"]
                .as_bool()
                .unwrap_or(false);

            let explanation = tool_result.arguments["explanation"]
                .as_str()
                .unwrap_or("")
                .to_string();

            if needs_clarification {
                // LLM detected ambiguity and needs clarification from user
                let clarification = &tool_result.arguments["clarification"];
                let question = clarification["question"]
                    .as_str()
                    .unwrap_or("Could you please clarify your request?");
                let ambiguity_type = clarification["ambiguity_type"]
                    .as_str()
                    .unwrap_or("unknown");

                tracing::debug!(
                    "Clarification needed: type={}, question={}",
                    ambiguity_type,
                    question
                );

                // Build a user-friendly clarification message
                let clarification_message =
                    if let Some(interpretations) = clarification["interpretations"].as_array() {
                        let options: Vec<String> = interpretations
                            .iter()
                            .filter_map(|i| {
                                let opt = i["option"].as_i64().unwrap_or(0);
                                let desc = i["description"].as_str().unwrap_or("");
                                if !desc.is_empty() {
                                    Some(format!("  {}. {}", opt, desc))
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if options.is_empty() {
                            question.to_string()
                        } else {
                            format!("{}\n\nOptions:\n{}", question, options.join("\n"))
                        }
                    } else {
                        question.to_string()
                    };

                session.add_agent_message(clarification_message.clone(), None, None);
                // Use PendingValidation to indicate we're waiting for user clarification
                session.state = SessionState::PendingValidation;

                return Ok(AgentChatResponse {
                    message: clarification_message,
                    intents: vec![],
                    validation_results: vec![],
                    session_state: SessionState::PendingValidation,
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: None,
                });
            }

            // Parse intents
            let intents: Vec<VerbIntent> =
                serde_json::from_value(tool_result.arguments["intents"].clone())
                    .unwrap_or_default();

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

    /// Handle DSL management commands: delete, undo, clear, execute
    ///
    /// Recognized patterns:
    /// - "delete <search>" / "remove <search>" - removes statement containing search term
    /// - "undo" / "undo last" - removes last statement
    /// - "clear" / "clear all" / "reset" - clears all DSL
    /// - "execute" / "run" / "go" - executes accumulated DSL
    async fn handle_dsl_command(
        &self,
        session: &mut AgentSession,
        message: &str,
    ) -> Result<Option<AgentChatResponse>, String> {
        let lower_msg = message.to_lowercase().trim().to_string();
        let words: Vec<&str> = lower_msg.split_whitespace().collect();

        tracing::debug!("[DSL_CMD] message='{}' words={:?}", message, words);

        if words.is_empty() {
            return Ok(None);
        }

        // Execute command - handles: run, execute, go, do it, run it, execute it, etc.
        let is_execute = matches!(words[0], "execute" | "run" | "go" | "do")
            || (words.len() >= 2
                && matches!(
                    (words[0], words[1]),
                    ("run", "it")
                        | ("do", "it")
                        | ("execute", "it")
                        | ("run", "that")
                        | ("execute", "that")
                ));
        if is_execute {
            tracing::info!("[DSL_CMD] Matched execute/run/go command");
            if session.assembled_dsl.is_empty() {
                return Ok(Some(AgentChatResponse {
                    message: "No DSL to execute. Add some statements first.".to_string(),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: session.state.clone(),
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: Some(vec![AgentCommand::Execute]),
                }));
            }
            return Ok(Some(AgentChatResponse {
                message: format!(
                    "Executing {} DSL statement(s)...",
                    session.assembled_dsl.len()
                ),
                intents: vec![],
                validation_results: vec![],
                session_state: SessionState::ReadyToExecute,
                can_execute: true,
                dsl_source: Some(session.assembled_dsl.join("\n\n")),
                ast: None,
                disambiguation: None,
                commands: Some(vec![AgentCommand::Execute]),
            }));
        }

        // Undo command
        if words[0] == "undo" {
            if session.assembled_dsl.is_empty() {
                return Ok(Some(AgentChatResponse {
                    message: "Nothing to undo.".to_string(),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: session.state.clone(),
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: None,
                }));
            }
            let removed = session.assembled_dsl.pop().unwrap_or_default();
            // Extract a short description from the removed DSL
            let desc = removed.lines().next().unwrap_or(&removed);
            let desc_short = if desc.len() > 60 {
                format!("{}...", &desc[..60])
            } else {
                desc.to_string()
            };
            session.state = if session.assembled_dsl.is_empty() {
                SessionState::New
            } else {
                SessionState::ReadyToExecute
            };
            return Ok(Some(AgentChatResponse {
                message: format!(
                    "Removed: {}\n{} statement(s) remaining.",
                    desc_short,
                    session.assembled_dsl.len()
                ),
                intents: vec![],
                validation_results: vec![],
                session_state: session.state.clone(),
                can_execute: !session.assembled_dsl.is_empty(),
                dsl_source: if session.assembled_dsl.is_empty() {
                    None
                } else {
                    Some(session.assembled_dsl.join("\n\n"))
                },
                ast: None,
                disambiguation: None,
                commands: None,
            }));
        }

        // Clear command
        if matches!(words[0], "clear" | "reset") {
            let count = session.assembled_dsl.len();
            session.assembled_dsl.clear();
            session.pending_intents.clear();
            session.state = SessionState::New;
            return Ok(Some(AgentChatResponse {
                message: format!("Cleared {} DSL statement(s).", count),
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

        // Delete/remove command - search for matching statement in buffer
        // Only intercept if there ARE statements in the buffer to remove
        // Otherwise, pass through to LLM for DSL generation (e.g., "remove product from cbu")
        if matches!(words[0], "delete" | "remove")
            && words.len() > 1
            && !session.assembled_dsl.is_empty()
        {
            let search_term = words[1..].join(" ");

            // Find statement containing the search term (case-insensitive)
            let idx = session
                .assembled_dsl
                .iter()
                .position(|stmt| stmt.to_lowercase().contains(&search_term));

            if let Some(idx) = idx {
                let removed = session.assembled_dsl.remove(idx);
                let desc = removed.lines().next().unwrap_or(&removed);
                let desc_short = if desc.len() > 60 {
                    format!("{}...", &desc[..60])
                } else {
                    desc.to_string()
                };
                session.state = if session.assembled_dsl.is_empty() {
                    SessionState::New
                } else {
                    SessionState::ReadyToExecute
                };
                return Ok(Some(AgentChatResponse {
                    message: format!(
                        "Removed statement {}: {}\n{} statement(s) remaining.",
                        idx + 1,
                        desc_short,
                        session.assembled_dsl.len()
                    ),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: session.state.clone(),
                    can_execute: !session.assembled_dsl.is_empty(),
                    dsl_source: if session.assembled_dsl.is_empty() {
                        None
                    } else {
                        Some(session.assembled_dsl.join("\n\n"))
                    },
                    ast: None,
                    disambiguation: None,
                    commands: None,
                }));
            }
            // No match found in buffer - fall through to LLM for DSL generation
            // This allows "remove fund accounting" to generate cbu.remove-product DSL
        }

        // Not a DSL command
        Ok(None)
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

        // Parse to AST and compile to ExecutionPlan (includes DAG toposort)
        // Single pipeline: Parse → Compile (with toposort) → Ready for execution
        let (ast, plan, was_reordered): (
            Option<Vec<Statement>>,
            Option<crate::dsl_v2::execution_plan::ExecutionPlan>,
            bool,
        ) = if let Some(ref src) = dsl {
            use crate::dsl_v2::{compile, parse_program};
            match parse_program(src) {
                Ok(program) => {
                    let statements = program.statements.clone();
                    match compile(&program) {
                        Ok(exec_plan) => {
                            // Check if reordering occurred by comparing statement order
                            let was_reordered = exec_plan.steps.len() > 1
                                && exec_plan.steps.windows(2).any(|w| {
                                    // If step N references step N+1's binding, reorder happened
                                    w[0].injections.iter().any(|inj| inj.from_step > 0)
                                });
                            (Some(statements), Some(exec_plan), was_reordered)
                        }
                        Err(e) => {
                            tracing::warn!("Compile error (will retry at execution): {}", e);
                            (Some(statements), None, false)
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Parse error (will retry at execution): {}", e);
                    (None, None, false)
                }
            }
        } else {
            (None, None, false)
        };

        // Update session - accumulate DSL incrementally
        // Generate user-friendly DSL from AST (shows entity names, not UUIDs)
        let user_dsl: Option<String> = if let Some(ref ast_statements) = ast {
            Some(
                ast_statements
                    .iter()
                    .map(|s| s.to_user_dsl_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        } else {
            dsl.clone() // Fallback to raw DSL if no AST
        };

        if let Some(ref dsl_source) = dsl {
            if let Some(ref ast_statements) = ast {
                session.set_pending_dsl(
                    dsl_source.clone(),
                    ast_statements.clone(),
                    plan.clone(),
                    was_reordered,
                );
            }
            // Push user-friendly DSL to accumulated DSL (for chat display)
            if let Some(ref user_dsl_str) = user_dsl {
                session.assembled_dsl.push(user_dsl_str.clone());
            }

            if all_valid {
                session.state = SessionState::ReadyToExecute;
            } else {
                session.state = SessionState::PendingValidation;
            }
        }

        session.add_agent_message(explanation.clone(), None, user_dsl.clone());

        let combined_dsl = if session.assembled_dsl.is_empty() {
            None
        } else {
            Some(session.assembled_dsl.join("\n\n"))
        };

        // Include active CBU name in response message for clarity
        let message = if let Some(ref cbu) = session.context.active_cbu {
            format!("[{}] {}", cbu.display_name, explanation)
        } else {
            explanation
        };

        // Set can_execute flag but do NOT auto-execute
        // User must explicitly type "run"/"execute" or click Execute button
        let can_execute = session.can_execute() && all_valid;
        let commands: Option<Vec<AgentCommand>> = None;

        Ok(AgentChatResponse {
            message,
            intents,
            validation_results,
            session_state: session.state.clone(),
            can_execute,
            dsl_source: combined_dsl,
            ast,
            disambiguation: None,
            commands,
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
        // Include domain knowledge and ambiguity detection rules
        let domain_knowledge = include_str!("prompts/domain_knowledge.md");
        let ambiguity_rules = include_str!("prompts/ambiguity_detection.md");

        format!(
            r#"You are a KYC/AML onboarding DSL assistant. Convert natural language to structured DSL intents.

IMPORTANT: You MUST use the generate_dsl_intents tool to return your response. Do NOT return plain text.

## Available DSL Verbs

{vocab}

## Intent Structure

Each intent represents a single DSL verb call with:
- verb: The verb name (e.g., "cbu.ensure", "entity.create-proper-person")
- params: Literal parameter values (e.g., {{"name": "Acme Corp", "jurisdiction": "LU"}})
- refs: References to previous results or session bindings (e.g., {{"cbu-id": "@cbu"}} or {{"cbu-id": "@result_1"}})
- lookups: Entity references needing database resolution

## Rules

1. Use exact verb names from the vocabulary
2. Use exact parameter names (with hyphens, e.g., "client-type" not "clientType")
3. If @cbu is available in session context, use it for cbu-id parameters
4. For sequences of new entities, use @result_N references where N is the sequence number
5. Check for ambiguity before generating intents - ask for clarification if needed
6. Recognize REMOVAL intent: words like "remove", "delete", "drop", "unlink", "take off", "unassign" indicate removal operations
   - "remove [product]" / "delete [product]" / "unlink [product]" → cbu.remove-product
   - "remove [entity] as [role]" / "unassign [role]" → cbu.remove-role
   - "delete [entity]" → entity.delete
   - "end ownership" → ubo.end-ownership

{domain_knowledge}

{ambiguity_rules}

## Entity Lookups

When the user mentions existing entities by name:
- Use the "lookups" field to request entity resolution
- Provide search_text (the name) and entity_type (person, company, cbu, entity, product, role)
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

User: "Add custody product to the fund" (with @cbu in session)
Intent: {{
  "verb": "cbu.add-product",
  "params": {{"product": "CUSTODY"}},
  "refs": {{"cbu-id": "@cbu"}}
}}

User: "Remove fund accounting" (with @cbu in session)
Intent: {{
  "verb": "cbu.remove-product",
  "params": {{"product": "FUND_ACCOUNTING"}},
  "refs": {{"cbu-id": "@cbu"}}
}}

User: "Delete the custody product" (with @cbu in session)
Intent: {{
  "verb": "cbu.remove-product",
  "params": {{"product": "CUSTODY"}},
  "refs": {{"cbu-id": "@cbu"}}
}}

User: "Add John Smith as director"  (with @cbu in session)
Intent 1: {{
  "verb": "entity.create-proper-person",
  "params": {{"first-name": "John", "last-name": "Smith"}},
  "refs": {{}}
}}
Intent 2: {{
  "verb": "cbu.assign-role",
  "params": {{"role": "DIRECTOR"}},
  "refs": {{"cbu-id": "@cbu", "entity-id": "@result_1"}}
}}"#
        )
    }

    /// Build tool definition for intent extraction
    ///
    /// Enhancement #2: The verb field uses an enum of all valid verb names from
    /// the registry. This constrains LLM output to only valid verbs, eliminating
    /// "unknown verb" errors entirely.
    fn build_intent_tool(&self) -> ToolDefinition {
        // Get all valid verb names from registry for constrained output
        let reg = registry();
        let verb_names: Vec<String> = reg.all_verbs().map(|v| v.full_name()).collect();

        ToolDefinition {
            name: "generate_dsl_intents".to_string(),
            description: "Generate structured DSL intents from user request. Use 'lookups' for entity references that need database resolution. Set needs_clarification=true if the request is ambiguous.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "needs_clarification": {
                        "type": "boolean",
                        "description": "Set to true if the request is ambiguous and needs clarification before proceeding",
                        "default": false
                    },
                    "clarification": {
                        "type": "object",
                        "description": "Required when needs_clarification is true",
                        "properties": {
                            "ambiguity_type": {
                                "type": "string",
                                "enum": ["name_parsing", "entity_match", "missing_context", "multiple_interpretations"],
                                "description": "Type of ambiguity detected"
                            },
                            "original_text": {
                                "type": "string",
                                "description": "The ambiguous part of the input"
                            },
                            "interpretations": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "option": { "type": "integer" },
                                        "interpretation": { "type": "string" },
                                        "description": { "type": "string" }
                                    }
                                },
                                "description": "Possible interpretations of the ambiguous input"
                            },
                            "question": {
                                "type": "string",
                                "description": "Clear question to ask the user for clarification"
                            }
                        },
                        "required": ["ambiguity_type", "question"]
                    },
                    "intents": {
                        "type": "array",
                        "description": "List of DSL verb intents (empty if needs_clarification is true)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "verb": {
                                    "type": "string",
                                    "enum": verb_names,
                                    "description": "The DSL verb - MUST be one of the allowed values"
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
                        "description": "Brief explanation of what the DSL will do, or why clarification is needed"
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

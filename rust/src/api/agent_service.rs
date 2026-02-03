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
//! | EntityGateway | `complete_keyword_values()` autocomplete | `resolve_all()` unified reference resolution |
//! | Verb Registry | `complete_verb_names()`, `complete_keywords()` | LLM prompt vocabulary, intent validation |
//! | CSG Linter | `diagnostics.rs` red squiggles | `run_semantic_validation()` retry feedback |
//! | Parser | Real-time syntax check | Post-generation validation |
//!
//! Both `agentic_server` and `ob-poc-web` should use this service.

use crate::agent::learning::embedder::CandleEmbedder;
use crate::agentic::llm_client::LlmClient;
use crate::api::client_group_adapter::ClientGroupEmbedderAdapter;
use crate::api::dsl_builder::{build_dsl_program, build_user_dsl_program, validate_intent};
use crate::api::intent::{IntentValidation, ParamValue, VerbIntent};
use crate::api::session::{DisambiguationItem, DisambiguationRequest, EntityMatchOption};
use crate::database::derive_semantic_state;
use crate::dsl_v2::ast::AstNode;
use crate::dsl_v2::gateway_resolver::{gateway_addr, GatewayRefResolver};
use crate::dsl_v2::ref_resolver::ResolveResult;
use crate::dsl_v2::semantic_validator::SemanticValidator;
use crate::dsl_v2::validation::{RefType, Severity, ValidationContext, ValidationRequest};
use crate::dsl_v2::verb_registry::registry;
use crate::dsl_v2::{enrich_program, parse_program, runtime_registry, Statement};
use crate::graph::GraphScope;
use crate::macros::OperatorMacroRegistry;
use crate::mcp::intent_pipeline::{compute_dsl_hash, IntentArgValue, IntentPipeline};
use crate::mcp::utterance::segment_utterance;

use crate::ontology::SemanticStageRegistry;
use crate::session::SessionScope;
use crate::session::{ResolutionSubSession, SessionState, UnifiedSession, UnresolvedRefInfo};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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

/// A resolved entity with both display name and UUID
#[derive(Debug, Clone)]
pub struct ResolvedEntityLookup {
    pub display_name: String,
    pub entity_id: Uuid,
}

/// Result of resolving entity lookups (LEGACY - replaced by UnifiedResolution)
#[derive(Debug)]
pub enum LookupResolution {
    /// All lookups resolved to exactly one entity
    /// Maps param name -> (display_name, entity_id)
    Resolved(HashMap<String, ResolvedEntityLookup>),
    /// Some lookups are ambiguous - need disambiguation
    Ambiguous(Vec<DisambiguationItem>),
    /// Error during lookup
    Error(String),
}

/// Parameters that should be resolved as codes (not raw strings) via EntityGateway.
/// These are reference data lookups where user input needs fuzzy matching to canonical codes.
/// UUID-based entity lookups (CBU, Entity, Document) are handled separately.
const CODE_PARAMS: &[(&str, RefType)] = &[
    // Core reference codes
    ("product", RefType::Product),
    ("service", RefType::Service),
    ("role", RefType::Role),
    ("jurisdiction", RefType::Jurisdiction),
    ("currency", RefType::Currency),
    ("client-type", RefType::ClientType),
    ("entity-type", RefType::EntityType),
    // Document and attribute references
    ("document-type", RefType::DocumentType),
    ("doc-type", RefType::DocumentType),
    ("attribute-id", RefType::AttributeId),
    ("attribute", RefType::AttributeId),
    // Screening and compliance
    ("screening-type", RefType::ScreeningType),
];

/// Convert IntentArgValue to ParamValue for VerbIntent construction
fn intent_arg_to_param_value(value: &IntentArgValue) -> ParamValue {
    match value {
        IntentArgValue::String(s) => ParamValue::String(s.clone()),
        IntentArgValue::Number(n) => ParamValue::Number(*n),
        IntentArgValue::Boolean(b) => ParamValue::Boolean(*b),
        IntentArgValue::Reference(r) => ParamValue::String(format!("@{}", r)),
        IntentArgValue::Uuid(u) => ParamValue::String(u.clone()),
        IntentArgValue::Unresolved { value, .. } => ParamValue::String(format!("<{}>", value)),
        IntentArgValue::Missing { arg_name } => {
            ParamValue::String(format!("<missing:{}>", arg_name))
        }
        IntentArgValue::List(items) => {
            let converted: Vec<ParamValue> = items.iter().map(intent_arg_to_param_value).collect();
            ParamValue::List(converted)
        }
        IntentArgValue::Map(entries) => {
            let converted: HashMap<String, ParamValue> = entries
                .iter()
                .map(|(k, v)| (k.clone(), intent_arg_to_param_value(v)))
                .collect();
            ParamValue::Object(converted)
        }
    }
}

/// Unified result of resolving ALL references (entities + codes) in intents.
/// Replaces the old 3-method approach (collect_lookups, resolve_lookups, inject_resolved_ids).
#[derive(Debug)]
pub enum UnifiedResolution {
    /// All references resolved - intents are ready for DSL building
    Resolved {
        /// Modified intents with resolved entity IDs and canonical codes
        intents: Vec<VerbIntent>,
        /// Code corrections applied (param, original, canonical)
        corrections: Vec<(String, String, String)>,
    },
    /// Some entity lookups are ambiguous - need user disambiguation
    NeedsDisambiguation {
        /// Disambiguation items to present to user
        items: Vec<DisambiguationItem>,
        /// Partially resolved intents (to resume after disambiguation)
        partial_intents: Vec<VerbIntent>,
    },
    /// Error during resolution (invalid code with no good fuzzy match)
    Error(String),
}

// ChatRequest is now the SINGLE source of truth - imported from ob-poc-types
pub use ob_poc_types::ChatRequest;

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
    /// Disambiguation request if needed (LEGACY - use unresolved_refs instead)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<DisambiguationRequest>,
    /// UI commands (show CBU, highlight entity, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<AgentCommand>>,
    /// Unresolved entity references needing resolution (post-DSL parsing)
    /// When present, UI should show resolution modal for each ref
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_refs: Option<Vec<UnresolvedRefInfo>>,
    /// Index of current ref being resolved (if in resolution state)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_ref_index: Option<usize>,
    /// Hash of current DSL for resolution commit verification (Issue K)
    /// UI must pass this back to /resolve-by-ref-id to prevent stale commits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl_hash: Option<String>,
    /// Verb disambiguation request (when multiple verbs match with similar confidence)
    /// UI should render these as clickable buttons, not text
    /// User selection triggers POST /api/session/:id/select-verb
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb_disambiguation: Option<ob_poc_types::VerbDisambiguationRequest>,
    /// Intent tier clarification request (when candidates span multiple intents)
    /// Shown BEFORE verb disambiguation to reduce cognitive load
    /// User selection triggers POST /api/session/:id/select-intent-tier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_tier: Option<ob_poc_types::IntentTierRequest>,
    /// Debug information (only populated when OB_CHAT_DEBUG=1)
    /// Contains verb matching details, candidates, and selection policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<ob_poc_types::ChatDebugInfo>,
    /// Verb candidates from semantic search (for feedback capture)
    /// Not serialized to API - internal use only
    #[serde(skip)]
    pub verb_candidates: Vec<crate::mcp::verb_search::VerbSearchResult>,
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

// ============================================================================
// Client Scope (for client portal)
// ============================================================================

/// Client scope - restricts what a client can see and do
#[derive(Debug, Clone)]
pub struct ClientScope {
    /// Client identity
    pub client_id: Uuid,
    /// CBUs this client has access to
    pub accessible_cbus: Vec<Uuid>,
    /// Client display name (for personalization)
    pub client_name: Option<String>,
}

impl ClientScope {
    /// Create a new client scope
    pub fn new(client_id: Uuid, accessible_cbus: Vec<Uuid>) -> Self {
        Self {
            client_id,
            accessible_cbus,
            client_name: None,
        }
    }

    /// Check if this client can access a specific CBU
    pub fn can_access_cbu(&self, cbu_id: &Uuid) -> bool {
        self.accessible_cbus.contains(cbu_id)
    }

    /// Get the default CBU for this client (first accessible)
    pub fn default_cbu(&self) -> Option<Uuid> {
        self.accessible_cbus.first().copied()
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
// GRACEFUL RESPONSE HELPERS - For ambiguous/vague/nonsense input
// ============================================================================

use crate::mcp::intent_pipeline::{ConfidenceTier, InputQuality};

/// Build a graceful response for various input quality levels
pub fn build_graceful_response(
    quality: &InputQuality,
    has_scope: bool,
    original_input: &str,
) -> String {
    match quality {
        InputQuality::Clear => {
            // Should not be called for Clear - handled normally
            String::new()
        }
        InputQuality::Ambiguous { candidates } => {
            let verb_list = candidates
                .iter()
                .map(|c| {
                    let desc = c.description.as_deref().unwrap_or("No description");
                    format!("• **{}**: {}", c.verb, desc)
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "I'm not sure which action you meant. Did you mean:\n\n{}\n\nPlease clarify.",
                verb_list
            )
        }
        InputQuality::TooVague { best_guess } => {
            let suggestion = if has_scope {
                "Try 'show CBUs' or 'list products'"
            } else {
                "Try 'work on [client name]' to set context first"
            };
            if let Some(guess) = best_guess {
                format!(
                    "I'm not sure what you meant by \"{}\". Did you mean **{}**?\n\n{}",
                    original_input, guess, suggestion
                )
            } else {
                format!(
                    "I couldn't understand \"{}\". {}\n\nExamples: 'show Allianz CBUs', 'add custody product', 'create a new fund'",
                    original_input, suggestion
                )
            }
        }
        InputQuality::Nonsense => {
            format!(
                "I couldn't understand \"{}\". Try a command like:\n\n\
                 • 'show Allianz CBUs'\n\
                 • 'add custody product'\n\
                 • 'create a new fund for Blackrock'\n\
                 • 'work on Allianz' (to set client context)\n\n\
                 Type /commands for a full list of available commands.",
                original_input
            )
        }
    }
}

/// Get confidence tier from pipeline result
pub fn get_confidence_tier(
    candidates: &[crate::mcp::verb_search::VerbSearchResult],
) -> ConfidenceTier {
    candidates
        .first()
        .map(|c| ConfidenceTier::from(c.score))
        .unwrap_or(ConfidenceTier::VeryLow)
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
///
/// Usage:
/// ```ignore
/// let service = AgentService::new(pool, Some(embedder));
/// let response = service.process_chat(&mut session, &request, llm_client).await?;
/// ```
pub struct AgentService {
    pool: PgPool,
    config: AgentServiceConfig,
    /// Embedder for semantic verb search - REQUIRED, no fallback path
    embedder: Arc<CandleEmbedder>,
    /// Learned data for exact phrase matching (invocation_phrases, entity_aliases)
    /// Loaded at startup via warmup - enables step 2 (global learned exact match)
    learned_data: Option<crate::agent::learning::warmup::SharedLearnedData>,
}

#[allow(dead_code)]
impl AgentService {
    /// Create agent service with pool, embedder, and learned data
    ///
    /// The embedder is REQUIRED for semantic verb search. All prompts go through
    /// the Candle intent pipeline - there is no fallback path.
    ///
    /// The learned_data enables step 2 (global learned exact match) for phrases
    /// like "spin up a fund" → cbu.create. Without it, only semantic search is used.
    pub fn new(
        pool: PgPool,
        embedder: Arc<CandleEmbedder>,
        learned_data: Option<crate::agent::learning::warmup::SharedLearnedData>,
    ) -> Self {
        Self {
            pool,
            config: AgentServiceConfig::default(),
            embedder,
            learned_data,
        }
    }

    /// Create IntentPipeline for processing user input
    fn get_intent_pipeline(&self) -> IntentPipeline {
        use crate::mcp::verb_search_factory::VerbSearcherFactory;

        let dyn_embedder: Arc<dyn crate::agent::learning::embedder::Embedder> =
            self.embedder.clone() as Arc<dyn crate::agent::learning::embedder::Embedder>;

        // Build verb searcher with macro registry for operator vocabulary
        let macro_dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/verb_schemas/macros");
        let macro_reg = OperatorMacroRegistry::load_from_dir(&macro_dir).unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to load operator macros: {}, using empty registry",
                e
            );
            OperatorMacroRegistry::new()
        });

        // Use factory for consistent configuration across all call sites
        let searcher = VerbSearcherFactory::build(
            &self.pool,
            dyn_embedder,
            self.learned_data.clone(),
            Arc::new(macro_reg),
        );

        IntentPipeline::with_pool(searcher, self.pool.clone())
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

    /// Derive semantic state for the active CBU and format it for the agent prompt.
    /// Returns a formatted string showing onboarding journey progress.
    ///
    /// This helps the agent understand:
    /// - What stages are complete, in progress, or blocked
    /// - What entities are missing
    /// - What the next actionable steps are
    async fn derive_semantic_context(&self, active_cbu_id: Uuid) -> Option<String> {
        let pool = &self.pool;

        // Load the semantic stage registry
        let registry = match SemanticStageRegistry::load_default() {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to load semantic stage registry: {}", e);
                return None;
            }
        };

        // Derive semantic state for this CBU
        match derive_semantic_state(pool, &registry, active_cbu_id).await {
            Ok(state) => {
                // Use the built-in to_prompt_context method
                Some(state.to_prompt_context())
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to derive semantic state for CBU {}: {}",
                    active_cbu_id,
                    e
                );
                None
            }
        }
    }

    /// Derive taxonomy navigation context for the agent prompt.
    /// Returns a formatted string showing the current navigation state.
    ///
    /// This helps the agent understand:
    /// - Current position in the taxonomy (breadcrumb trail)
    /// - Available navigation actions (drill down, zoom out)
    /// - Visible entities at this level
    fn derive_taxonomy_context(&self, session: &UnifiedSession) -> Option<String> {
        let stack = &session.context.taxonomy_stack;

        if stack.is_empty() {
            return None;
        }

        let breadcrumbs = stack.breadcrumbs();
        let depth = stack.depth();
        let can_zoom_out = stack.can_zoom_out();
        let can_zoom_in = stack.can_zoom_in();

        let current_frame = stack.current()?;
        let tree = &current_frame.tree;

        // Build node summary (top-level children)
        let node_summary: Vec<String> = tree
            .children
            .iter()
            .take(10)
            .map(|child| {
                let count_info = if child.children.is_empty() {
                    String::new()
                } else {
                    format!(" ({} children)", child.children.len())
                };
                format!("  - {} [{:?}]{}", child.label, child.node_type, count_info)
            })
            .collect();

        let more_indicator = if tree.children.len() > 10 {
            format!("\n  ... and {} more", tree.children.len() - 10)
        } else {
            String::new()
        };

        // Build navigation hints
        let mut nav_hints = Vec::new();
        if can_zoom_out {
            nav_hints.push("'zoom out' or 'go back' to parent level");
        }
        if can_zoom_in && !tree.children.is_empty() {
            nav_hints.push("'drill into <name>' to explore a child");
        }
        nav_hints.push("'show taxonomy' to see current position");

        let context = format!(
            r#"# TAXONOMY NAVIGATION

Current Position: {} (depth {})
Breadcrumb: {}

## Visible Nodes:
{}{}

## Navigation:
{}
"#,
            current_frame.label,
            depth,
            breadcrumbs.join(" → "),
            node_summary.join("\n"),
            more_indicator,
            nav_hints.join("\n")
        );

        Some(context)
    }

    /// Derive KYC case context when a case is active in the session.
    /// Returns a formatted string showing case state with embedded workstream requests.
    ///
    /// This implements the "Domain Coherence" principle: requests appear as child
    /// nodes of workstreams in `awaiting` arrays, not as a separate list.
    async fn derive_kyc_case_context(&self, kyc_case_id: Uuid) -> Option<String> {
        let pool = &self.pool;

        // Query case state with workstreams and embedded awaiting requests
        let case_row = sqlx::query!(
            r#"
            SELECT
                c.case_id,
                c.status,
                c.risk_rating,
                c.case_type,
                c.escalation_level,
                cb.name as cbu_name
            FROM kyc.cases c
            JOIN "ob-poc".cbus cb ON c.cbu_id = cb.cbu_id
            WHERE c.case_id = $1
            "#,
            kyc_case_id
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()?;

        // Get workstreams with awaiting counts
        let workstreams = sqlx::query!(
            r#"
            SELECT
                w.workstream_id,
                w.status,
                w.blocker_type,
                e.name as entity_name,
                (SELECT COUNT(*) FROM kyc.outstanding_requests r
                 WHERE r.workstream_id = w.workstream_id
                 AND r.status IN ('PENDING', 'ESCALATED')) as awaiting_count,
                (SELECT COUNT(*) FROM kyc.outstanding_requests r
                 WHERE r.workstream_id = w.workstream_id
                 AND r.status IN ('PENDING', 'ESCALATED')
                 AND r.due_date < CURRENT_DATE) as overdue_count
            FROM kyc.entity_workstreams w
            JOIN "ob-poc".entities e ON w.entity_id = e.entity_id
            WHERE w.case_id = $1
            ORDER BY w.created_at
            "#,
            kyc_case_id
        )
        .fetch_all(pool)
        .await
        .ok()?;

        // Build workstream summary lines
        let mut ws_lines = Vec::new();
        let mut total_awaiting = 0i64;
        let mut total_overdue = 0i64;

        for ws in &workstreams {
            let awaiting = ws.awaiting_count.unwrap_or(0);
            let overdue = ws.overdue_count.unwrap_or(0);
            total_awaiting += awaiting;
            total_overdue += overdue;

            let status_icon = match ws.status.as_str() {
                "COMPLETE" => "✓",
                "BLOCKED" => "⛔",
                _ if overdue > 0 => "⚠️",
                _ if awaiting > 0 => "⏳",
                _ => "→",
            };

            let awaiting_info = if awaiting > 0 {
                if overdue > 0 {
                    format!(" [{} awaiting, {} OVERDUE]", awaiting, overdue)
                } else {
                    format!(" [{} awaiting]", awaiting)
                }
            } else {
                String::new()
            };

            let blocker_info = ws
                .blocker_type
                .as_ref()
                .map(|b| format!(" BLOCKED: {}", b))
                .unwrap_or_default();

            ws_lines.push(format!(
                "  {} {} ({}){}{}",
                status_icon, ws.entity_name, ws.status, awaiting_info, blocker_info
            ));
        }

        // Build attention section if there are overdue items
        let attention_section = if total_overdue > 0 {
            format!(
                "\n\n⚠️ ATTENTION: {} overdue request(s) need action. Use `(kyc-case.state :case-id @case)` for details.",
                total_overdue
            )
        } else {
            String::new()
        };

        let context = format!(
            r#"# KYC CASE CONTEXT

Case: {} ({})
Status: {} | Risk: {} | Type: {}

## Workstreams ({} total, {} awaiting, {} overdue):
{}{}

Use `(kyc-case.state :case-id @case)` to get full state with embedded awaiting requests."#,
            case_row.cbu_name,
            kyc_case_id,
            case_row.status,
            case_row.risk_rating.as_deref().unwrap_or("unrated"),
            case_row.case_type.as_deref().unwrap_or("unknown"),
            workstreams.len(),
            total_awaiting,
            total_overdue,
            ws_lines.join("\n"),
            attention_section
        );

        Some(context)
    }

    /// ONE PATH - all user prompts:
    /// 1. "run"/"execute"/"do it" → execute staged runbook
    /// 2. Intent pipeline → DSL
    ///    - Session/view verbs → execute immediately (navigation)
    ///    - Data mutation verbs → stage for user confirmation
    /// 3. Ambiguous? → "Did you mean X or Y?"
    pub async fn process_chat(
        &self,
        session: &mut UnifiedSession,
        request: &ChatRequest,
        _llm_client: Arc<dyn LlmClient>,
    ) -> Result<AgentChatResponse, String> {
        use crate::dsl_v2::parse_program;
        use crate::mcp::intent_pipeline::PipelineOutcome;
        session.add_user_message(request.message.clone());

        let input = request.message.trim().to_lowercase();

        // 1. Check for RUN command - execute staged runbook
        if matches!(
            input.as_str(),
            "run" | "execute" | "do it" | "go" | "run it" | "execute it"
        ) {
            return self.execute_runbook(session).await;
        }

        // 2. Check for pending verb disambiguation - numeric input selects an option
        if let Some(ref pending) = session.pending_verb_disambiguation {
            // Check if input is a number (1, 2, 3, etc.)
            if let Ok(selection) = input.trim().parse::<usize>() {
                if selection >= 1 && selection <= pending.options.len() {
                    let option = &pending.options[selection - 1];
                    let selected_verb = option.verb_fqn.clone();
                    let original_input = pending.original_input.clone();
                    let all_candidates: Vec<crate::session::unified::VerbCandidate> = pending
                        .options
                        .iter()
                        .map(|o| crate::session::unified::VerbCandidate {
                            verb: o.verb_fqn.clone(),
                            score: o.score,
                        })
                        .collect();

                    // Clear the pending state
                    session.pending_verb_disambiguation = None;

                    tracing::info!(
                        selected_verb = %selected_verb,
                        original_input = %original_input,
                        selection = selection,
                        "User selected verb from disambiguation"
                    );

                    // Record learning signal and continue with selected verb
                    return self
                        .handle_verb_selection(
                            session,
                            &original_input,
                            &selected_verb,
                            &all_candidates,
                        )
                        .await;
                } else {
                    // Invalid selection number
                    let msg = format!(
                        "Please select a number between 1 and {}.",
                        pending.options.len()
                    );
                    session.add_agent_message(msg.clone(), None, None);
                    return Ok(AgentChatResponse {
                        message: msg,
                        intents: vec![],
                        validation_results: vec![],
                        session_state: SessionState::PendingValidation,
                        can_execute: false,
                        dsl_source: None,
                        ast: None,
                        disambiguation: None,
                        commands: None,
                        unresolved_refs: None,
                        current_ref_index: None,
                        dsl_hash: None,
                        verb_disambiguation: None,
                        intent_tier: None,
                        debug: None,
                        verb_candidates: vec![],
                    });
                }
            }
            // Not a number - clear pending and process as new input
            session.pending_verb_disambiguation = None;
        }

        // Clear pending intent tier if user typed something new
        session.pending_intent_tier = None;

        // STAGE 0: UTTERANCE SEGMENTATION
        // Segment input into verb_phrase, group_phrase, scope_phrase, residual_terms
        // This runs BEFORE the pipeline to protect entity context from verb matching
        let segmentation = segment_utterance(&request.message, &self.pool).await;

        tracing::debug!(
            original = %segmentation.original,
            verb_phrase = %segmentation.verb_phrase.text,
            verb_confidence = segmentation.verb_phrase.confidence,
            group_phrase = ?segmentation.group_phrase.as_ref().map(|g| &g.text),
            group_confidence = ?segmentation.group_phrase.as_ref().map(|g| g.confidence),
            scope_phrase = ?segmentation.scope_phrase.as_ref().map(|s| &s.text),
            is_likely_typo = segmentation.is_likely_typo(),
            is_likely_garbage = segmentation.is_likely_garbage(),
            "Utterance segmentation complete"
        );

        // ONE PIPELINE - generate/validate DSL
        // Wrap session for macro expansion (macros need session state for prereqs/context)
        let session_arc = Arc::new(RwLock::new(session.clone()));

        // Use segmented verb phrase for verb search (improves matching accuracy)
        // Only use if verb phrase has reasonable confidence
        let verb_phrase_for_search = if segmentation.verb_phrase.confidence >= 0.5 {
            Some(segmentation.verb_phrase.text.as_str())
        } else {
            None
        };

        let result = self
            .get_intent_pipeline()
            .with_session(session_arc)
            .process_with_segmentation(
                &request.message,
                None,
                session.context.client_scope.clone(),
                verb_phrase_for_search,
            )
            .await;

        match result {
            Ok(r) => {
                // Handle macro expansion with explicit feedback
                if let PipelineOutcome::MacroExpanded {
                    ref macro_verb,
                    ref unlocks,
                } = r.outcome
                {
                    tracing::info!(
                        macro_verb = %macro_verb,
                        expanded_dsl = %r.dsl,
                        unlocks = ?unlocks,
                        "Macro expanded to primitive DSL"
                    );
                    // Stage the expanded primitive DSL
                    let ast = parse_program(&r.dsl)
                        .map(|p| p.statements)
                        .unwrap_or_default();
                    session.set_pending_dsl(r.dsl.clone(), ast, None, false);

                    // Macro verbs that are structure/case/mandate operations auto-run
                    let is_setup_macro = macro_verb.ends_with(".setup")
                        || macro_verb.ends_with(".select")
                        || macro_verb.ends_with(".list");

                    if is_setup_macro {
                        tracing::debug!(macro_verb = %macro_verb, "Auto-running setup macro");
                        return self.execute_runbook(session).await;
                    }

                    let msg = format!(
                        "Macro '{}' expanded to:\n{}\n\nSay 'run' to execute.",
                        macro_verb, r.dsl
                    );
                    session.add_agent_message(msg.clone(), None, Some(r.dsl.clone()));
                    return Ok(self.staged_response(r.dsl, msg, r.verb_candidates));
                }

                // Got valid DSL?
                if r.valid && !r.dsl.is_empty() {
                    // Stage in runbook (SINGLE LOOP - all DSL goes through here)
                    let ast = parse_program(&r.dsl)
                        .map(|p| p.statements)
                        .unwrap_or_default();

                    session.set_pending_dsl(r.dsl.clone(), ast, None, false);

                    // Check if this is a session/view verb (navigation)
                    let verb = &r.intent.verb;
                    let is_navigation = Self::is_navigation_verb(verb);

                    if is_navigation {
                        // Auto-trigger run for navigation verbs (goes through runbook)
                        tracing::debug!(verb = %verb, dsl = %r.dsl, "Auto-running navigation verb");
                        return self.execute_runbook(session).await;
                    }

                    // Data mutation - wait for user to say "run"
                    let msg = format!("Staged: {}\n\nSay 'run' to execute.", r.dsl);
                    session.add_agent_message(msg.clone(), None, Some(r.dsl.clone()));
                    return Ok(self.staged_response(r.dsl, msg, r.verb_candidates));
                }

                // Ambiguous? Check if we should show intent tiers or direct verb disambiguation
                // Intent tiers reduce cognitive load when candidates span multiple intents
                if matches!(r.outcome, PipelineOutcome::NeedsClarification)
                    && r.verb_candidates.len() >= 2
                {
                    // Analyze which intent tiers are represented
                    let intent_taxonomy = crate::dsl_v2::intent_tiers::intent_tier_taxonomy();
                    let verbs: Vec<&str> =
                        r.verb_candidates.iter().map(|c| c.verb.as_str()).collect();
                    let analysis = intent_taxonomy.analyze_candidates(&verbs);

                    // Get top score for threshold check
                    let top_score = r.verb_candidates.first().map(|c| c.score).unwrap_or(0.0);

                    // Should we show intent tiers first?
                    if intent_taxonomy.should_use_tiers(&analysis, top_score) {
                        return Ok(self.build_intent_tier_response(
                            &request.message,
                            &r.verb_candidates,
                            &analysis,
                            session,
                        ));
                    }

                    // Otherwise show direct verb disambiguation
                    return Ok(self.build_verb_disambiguation_response(
                        &request.message,
                        &r.verb_candidates,
                        session,
                    ));
                }

                // Pipeline gave an error message? Return it
                if let Some(err) = r.validation_error {
                    return Ok(self.fail(&err, session));
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Pipeline error");
            }
        }

        // STAGE 1: TYPO CHECK using segmentation
        // If group resolved but verb weak, this is likely a typo (not garbage)
        if segmentation.is_likely_typo() {
            let group_name = segmentation
                .group_phrase
                .as_ref()
                .map(|g| g.text.as_str())
                .unwrap_or("unknown");
            let verb_text = &segmentation.verb_phrase.text;

            tracing::info!(
                verb_phrase = %verb_text,
                group_phrase = %group_name,
                verb_confidence = segmentation.verb_phrase.confidence,
                "Detected likely typo - group resolved but verb weak"
            );

            // Provide helpful feedback with the resolved group
            let msg = format!(
                "I understood you want to work with '{}', but I couldn't parse the command '{}'.\n\n\
                 Did you mean one of these?\n\
                 - \"work on {}\" (set client scope)\n\
                 - \"load {}\" (load client book)\n\
                 - \"show {} cbus\" (list CBUs)\n\n\
                 Try /commands for help.",
                group_name, verb_text, group_name, group_name, group_name
            );
            session.add_agent_message(msg.clone(), None, None);
            return Ok(self.fail(&msg, session));
        }

        // STAGE 2: GARBAGE CHECK
        if segmentation.is_likely_garbage() {
            tracing::debug!(
                original = %segmentation.original,
                "Input appears to be garbage (no meaningful segments)"
            );
            return Ok(self.fail("I don't understand. Try /commands for help.", session));
        }

        // Fallback - something else went wrong
        Ok(self.fail("I don't understand. Try /commands for help.", session))
    }

    /// Check if a verb is a navigation/session verb that should auto-run
    fn is_navigation_verb(verb: &str) -> bool {
        // Session verbs - scope/navigation
        if verb.starts_with("session.") {
            return true;
        }
        // View verbs - viewport navigation
        if verb.starts_with("view.") {
            return true;
        }
        false
    }

    /// Handle verb selection from disambiguation (either numeric input or API call)
    ///
    /// Records learning signal and re-runs pipeline with selected verb
    async fn handle_verb_selection(
        &self,
        session: &mut UnifiedSession,
        original_input: &str,
        selected_verb: &str,
        all_candidates: &[crate::session::unified::VerbCandidate],
    ) -> Result<AgentChatResponse, String> {
        use crate::dsl_v2::parse_program;

        // Record learning signal (gold-standard training data)
        // Convert candidates to verb strings for the recording function
        let candidate_verbs: Vec<String> = all_candidates.iter().map(|c| c.verb.clone()).collect();
        if let Err(e) = crate::api::agent_routes::record_verb_selection_signal(
            &self.pool,
            original_input,
            selected_verb,
            &candidate_verbs,
        )
        .await
        {
            tracing::warn!("Failed to record verb selection signal: {}", e);
            // Continue anyway - don't block the user
        }

        // Re-run intent pipeline with selected verb as domain hint
        // The verb is now known, so we generate DSL for it
        let domain = selected_verb.split('.').next();
        let session_arc = std::sync::Arc::new(std::sync::RwLock::new(session.clone()));
        let result = self
            .get_intent_pipeline()
            .with_session(session_arc)
            .process_with_scope(original_input, domain, session.context.client_scope.clone())
            .await;

        match result {
            Ok(r) => {
                // Got valid DSL - stage it
                if r.valid && !r.dsl.is_empty() {
                    let ast = parse_program(&r.dsl)
                        .map(|p| p.statements)
                        .unwrap_or_default();

                    // Check if navigation verb (auto-execute)
                    let is_navigation =
                        selected_verb.starts_with("session.") || selected_verb.starts_with("view.");

                    if is_navigation {
                        session.set_pending_dsl(r.dsl.clone(), ast, None, false);
                        return self.execute_runbook(session).await;
                    }

                    // Stage for confirmation
                    session.set_pending_dsl(r.dsl.clone(), ast, None, false);
                    let msg = format!(
                        "Selected **{}**.\n\nStaged: {}\n\nSay 'run' to execute.",
                        selected_verb, r.dsl
                    );
                    session.add_agent_message(msg.clone(), None, Some(r.dsl.clone()));
                    return Ok(self.staged_response(r.dsl, msg, r.verb_candidates));
                }

                // Pipeline gave an error
                if let Some(err) = r.validation_error {
                    return Ok(self.fail(&err, session));
                }

                // Fallback
                Ok(self.fail("Failed to generate DSL for selected verb", session))
            }
            Err(e) => Ok(self.fail(&format!("Pipeline error: {}", e), session)),
        }
    }

    /// Execute all pending DSL in the session runbook
    ///
    /// Pipeline: Parse → Enrich → Resolve EntityRefs → Execute
    async fn execute_runbook(
        &self,
        session: &mut UnifiedSession,
    ) -> Result<AgentChatResponse, String> {
        use crate::dsl_v2::{DslExecutor, ExecutionContext};

        // Check if there's anything to run
        if !session.run_sheet.has_runnable() {
            return Ok(self.fail("Nothing staged to run. Send a command first.", session));
        }

        // Get all pending DSL
        let dsl = match session.run_sheet.combined_dsl() {
            Some(d) if !d.is_empty() => d,
            _ => return Ok(self.fail("No DSL to execute.", session)),
        };

        // 1. Parse DSL
        let raw_program = match parse_program(&dsl) {
            Ok(p) => p,
            Err(e) => return Ok(self.fail(&format!("Parse error: {}", e), session)),
        };

        // 2. Enrich: convert string literals to EntityRefs based on YAML verb config
        let registry = runtime_registry();
        let enrichment_result = enrich_program(raw_program, registry);
        let mut program = enrichment_result.program;

        // 3. Resolve all EntityRefs before execution
        // This is where we look up "Allianz" → client_group UUID
        for stmt in &mut program.statements {
            if let Statement::VerbCall(vc) = stmt {
                for arg in &mut vc.arguments {
                    self.resolve_ast_node(&mut arg.value).await;
                }
            }
        }

        // 4. Check for any remaining unresolved refs
        let mut unresolved = Vec::new();
        for stmt in &program.statements {
            if let Statement::VerbCall(vc) = stmt {
                for arg in &vc.arguments {
                    Self::collect_unresolved(&arg.value, &mut unresolved);
                }
            }
        }

        if !unresolved.is_empty() {
            let details: Vec<String> = unresolved
                .iter()
                .map(|(et, val)| format!("{}: '{}'", et, val))
                .collect();
            let msg = format!(
                "Cannot execute: {} unresolved reference(s):\n  - {}",
                unresolved.len(),
                details.join("\n  - ")
            );
            return Ok(self.fail(&msg, session));
        }

        // 5. Convert resolved AST back to DSL string for execution
        let resolved_dsl = program.to_dsl_string();
        tracing::debug!(resolved_dsl = %resolved_dsl, "Executing resolved DSL");

        // 6. Execute
        let executor = DslExecutor::new(self.pool.clone());
        let mut exec_ctx = ExecutionContext::new();
        match executor.execute_dsl(&resolved_dsl, &mut exec_ctx).await {
            Ok(results) => {
                // Check if any result is a macro that returned combined_dsl to stage
                // This handles verbs like cbu.create-from-client-group that generate DSL batches
                for result in &results {
                    if let crate::dsl_v2::ExecutionResult::Record(json) = result {
                        if let Some(combined_dsl) =
                            json.get("combined_dsl").and_then(|v| v.as_str())
                        {
                            if !combined_dsl.is_empty() {
                                // Macro returned DSL to stage - clear current runsheet and stage the new DSL
                                session.run_sheet.entries.clear();

                                let ast = parse_program(combined_dsl)
                                    .map(|p| p.statements)
                                    .unwrap_or_default();
                                session.set_pending_dsl(combined_dsl.to_string(), ast, None, false);

                                let entities_found = json
                                    .get("entities_found")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                let msg = json
                                    .get("message")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("DSL batch generated");

                                let response_msg = format!(
                                    "{}\n\nStaged {} cbu.create statements. Say 'run' to execute.",
                                    msg, entities_found
                                );
                                session.add_agent_message(
                                    response_msg.clone(),
                                    None,
                                    Some(combined_dsl.to_string()),
                                );

                                return Ok(AgentChatResponse {
                                    message: response_msg,
                                    dsl_source: Some(combined_dsl.to_string()),
                                    can_execute: true, // Ready to run
                                    session_state: SessionState::ReadyToExecute,
                                    intents: vec![],
                                    validation_results: vec![],
                                    ast: None,
                                    disambiguation: None,
                                    commands: None,
                                    unresolved_refs: None,
                                    current_ref_index: None,
                                    dsl_hash: None,
                                    verb_disambiguation: None,
                                    intent_tier: None,
                                    debug: None,
                                    verb_candidates: vec![],
                                });
                            }
                        }
                    }
                }

                // Normal execution - mark as executed
                session.run_sheet.mark_all_executed();

                // Sync unified session if any CBUs were loaded
                // This propagates scope to session.context so the watch endpoint
                // returns scope_type, which triggers UI viewport refresh
                if let Some(unified_session) = exec_ctx.take_pending_session() {
                    let loaded = unified_session.cbu_ids_vec();
                    let cbu_count = loaded.len();

                    // Merge loaded CBUs into context
                    for cbu_id in &loaded {
                        if !session.context.cbu_ids.contains(cbu_id) {
                            session.context.cbu_ids.push(*cbu_id);
                        }
                    }

                    // Set scope definition so UI knows to trigger scope_graph refetch
                    // Use Custom scope for multi-CBU loads, SingleCbu for single CBU
                    let scope_def = if cbu_count == 1 {
                        GraphScope::SingleCbu {
                            cbu_id: loaded[0],
                            cbu_name: unified_session.name.clone().unwrap_or_default(),
                        }
                    } else if cbu_count > 1 {
                        // Multi-CBU scope - use Custom with session name or description
                        GraphScope::Custom {
                            description: unified_session
                                .name
                                .clone()
                                .unwrap_or_else(|| format!("{} CBUs", cbu_count)),
                        }
                    } else {
                        GraphScope::Empty
                    };

                    session.context.scope = Some(SessionScope::from_graph_scope(scope_def));
                    tracing::info!(
                        "[EXEC] Set context.scope with {} CBUs, scope_type={:?}",
                        cbu_count,
                        session.context.scope.as_ref().map(|s| &s.definition)
                    );
                }

                let msg = format!(
                    "Executed {} statement(s). {} CBUs in scope.",
                    results.len(),
                    session.context.cbu_ids.len()
                );
                session.add_agent_message(msg.clone(), None, None);
                Ok(AgentChatResponse {
                    message: msg,
                    dsl_source: Some(resolved_dsl),
                    can_execute: false, // Already executed
                    session_state: SessionState::Executed,
                    intents: vec![],
                    validation_results: vec![],
                    ast: None,
                    disambiguation: None,
                    commands: None,
                    unresolved_refs: None,
                    current_ref_index: None,
                    dsl_hash: None,
                    verb_disambiguation: None,
                    intent_tier: None,
                    debug: None,
                    verb_candidates: vec![],
                })
            }
            Err(e) => {
                let msg = format!("Execution failed: {}", e);
                session.add_agent_message(msg.clone(), None, None);
                Ok(self.fail(&msg, session))
            }
        }
    }

    /// Recursively resolve EntityRefs in an AST node
    async fn resolve_ast_node(&self, node: &mut AstNode) {
        match node {
            AstNode::EntityRef {
                entity_type,
                value,
                resolved_key,
                ..
            } => {
                // Skip if already resolved
                if resolved_key.is_some() {
                    return;
                }

                // Resolve using AgentService.resolve_entity (handles client_group specially)
                match self.resolve_entity(entity_type, value).await {
                    Ok(ResolveResult::Found {
                        id,
                        display: display_name,
                    }) => {
                        tracing::debug!(
                            entity_type = %entity_type,
                            value = %value,
                            resolved_id = %id,
                            display_name = %display_name,
                            "Resolved EntityRef"
                        );
                        *resolved_key = Some(id.to_string());
                    }
                    Ok(ResolveResult::FoundByCode {
                        code,
                        uuid,
                        display: display_name,
                    }) => {
                        // For code-based PKs, use UUID if available, otherwise the code
                        let resolved = uuid.map(|u| u.to_string()).unwrap_or_else(|| code.clone());
                        tracing::debug!(
                            entity_type = %entity_type,
                            value = %value,
                            resolved_key = %resolved,
                            display_name = %display_name,
                            "Resolved EntityRef by code"
                        );
                        *resolved_key = Some(resolved);
                    }
                    Ok(ResolveResult::NotFound { suggestions }) => {
                        if !suggestions.is_empty() {
                            tracing::warn!(
                                entity_type = %entity_type,
                                value = %value,
                                suggestions = ?suggestions.iter().map(|s| &s.display).collect::<Vec<_>>(),
                                "Ambiguous EntityRef - suggestions available"
                            );
                        } else {
                            tracing::warn!(
                                entity_type = %entity_type,
                                value = %value,
                                "EntityRef not found"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            entity_type = %entity_type,
                            value = %value,
                            error = %e,
                            "EntityRef resolution error"
                        );
                    }
                }
            }
            AstNode::List { items, .. } => {
                for item in items {
                    Box::pin(self.resolve_ast_node(item)).await;
                }
            }
            AstNode::Map { entries, .. } => {
                for (_, v) in entries {
                    Box::pin(self.resolve_ast_node(v)).await;
                }
            }
            AstNode::Nested(vc) => {
                for arg in &mut vc.arguments {
                    Box::pin(self.resolve_ast_node(&mut arg.value)).await;
                }
            }
            // Literals and SymbolRefs don't need resolution
            AstNode::Literal(_, _) | AstNode::SymbolRef { .. } => {}
        }
    }

    /// Collect unresolved EntityRefs from an AST node
    fn collect_unresolved(node: &AstNode, unresolved: &mut Vec<(String, String)>) {
        use crate::dsl_v2::ast::AstNode;

        match node {
            AstNode::EntityRef {
                entity_type,
                value,
                resolved_key,
                ..
            } => {
                if resolved_key.is_none() {
                    unresolved.push((entity_type.clone(), value.clone()));
                }
            }
            AstNode::List { items, .. } => {
                for item in items {
                    Self::collect_unresolved(item, unresolved);
                }
            }
            AstNode::Map { entries, .. } => {
                for (_, v) in entries {
                    Self::collect_unresolved(v, unresolved);
                }
            }
            AstNode::Nested(vc) => {
                for arg in &vc.arguments {
                    Self::collect_unresolved(&arg.value, unresolved);
                }
            }
            AstNode::Literal(_, _) | AstNode::SymbolRef { .. } => {}
        }
    }

    fn staged_response(
        &self,
        dsl: String,
        msg: String,
        candidates: Vec<crate::mcp::verb_search::VerbSearchResult>,
    ) -> AgentChatResponse {
        AgentChatResponse {
            message: msg,
            dsl_source: Some(dsl),
            can_execute: true,
            session_state: SessionState::ReadyToExecute,
            intents: vec![],
            validation_results: vec![],
            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            debug: None,
            verb_candidates: candidates,
        }
    }

    fn ok_response(&self, dsl: String) -> AgentChatResponse {
        AgentChatResponse {
            message: dsl.clone(),
            dsl_source: Some(dsl),
            can_execute: true,
            session_state: SessionState::ReadyToExecute,
            intents: vec![],
            validation_results: vec![],
            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            debug: None,
            verb_candidates: vec![],
        }
    }

    /// Fail: return message to user
    fn fail(&self, msg: &str, session: &mut UnifiedSession) -> AgentChatResponse {
        session.add_agent_message(msg.to_string(), None, None);
        AgentChatResponse {
            message: msg.to_string(),
            intents: vec![],
            validation_results: vec![],
            session_state: SessionState::New,
            can_execute: false,
            dsl_source: None,
            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            debug: None,
            verb_candidates: vec![],
        }
    }

    /// Build a structured verb disambiguation response for the UI
    ///
    /// When verb search returns ambiguous results (multiple verbs with similar scores),
    /// this method creates a response with clickable options instead of just text.
    ///
    /// The UI will render these as buttons. When clicked, the selection is sent to
    /// `/api/session/:id/select-verb` which records the learning signal and executes.
    fn build_verb_disambiguation_response(
        &self,
        original_input: &str,
        candidates: &[crate::mcp::verb_search::VerbSearchResult],
        session: &mut UnifiedSession,
    ) -> AgentChatResponse {
        use ob_poc_types::{VerbDisambiguationRequest, VerbOption};

        // Build verb options from candidates (top 5 max)
        // Include domain/category context from taxonomy for better UX
        let taxonomy = crate::dsl_v2::verb_taxonomy::verb_taxonomy();
        let options: Vec<VerbOption> = candidates
            .iter()
            .take(5)
            .map(|c| {
                let description = c
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Execute {}", c.verb));

                // Get domain context from taxonomy
                let location = taxonomy.location_for_verb(&c.verb);

                VerbOption {
                    verb_fqn: c.verb.clone(),
                    description,
                    example: format!("({})", c.verb),
                    score: c.score,
                    matched_phrase: Some(c.matched_phrase.clone()),
                    domain_label: location.as_ref().map(|l| l.domain_label.clone()),
                    category_label: location.as_ref().map(|l| l.category_label.clone()),
                }
            })
            .collect();

        let request_id = Uuid::now_v7().to_string();

        let disambiguation_request = VerbDisambiguationRequest {
            request_id: request_id.clone(),
            original_input: original_input.to_string(),
            options,
            prompt: "Which action did you mean?".to_string(),
        };

        // Build message for display (also shown in chat history)
        let options_text: Vec<String> = candidates
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, c)| {
                let desc = c.description.as_deref().unwrap_or("No description");
                format!("{}. **{}**: {}", i + 1, c.verb, desc)
            })
            .collect();

        let message = format!(
            "I found multiple matching actions for \"{}\":\n\n{}\n\nType a number to select, or enter a new command.",
            original_input,
            options_text.join("\n")
        );

        session.add_agent_message(message.clone(), None, None);

        // Store pending disambiguation state for numeric selection handling
        use crate::session::unified::{
            PendingVerbDisambiguation, VerbCandidate, VerbDisambiguationOption,
        };
        let pending_options: Vec<VerbDisambiguationOption> = candidates
            .iter()
            .take(5)
            .map(|c| VerbDisambiguationOption {
                verb_fqn: c.verb.clone(),
                description: c
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Execute {}", c.verb)),
                score: c.score,
                matched_phrase: c.matched_phrase.clone(),
                all_candidates: candidates
                    .iter()
                    .map(|cand| VerbCandidate {
                        verb: cand.verb.clone(),
                        score: cand.score,
                    })
                    .collect(),
            })
            .collect();

        session.pending_verb_disambiguation = Some(PendingVerbDisambiguation {
            original_input: original_input.to_string(),
            options: pending_options,
            created_at: chrono::Utc::now(),
        });

        // Return response with verb_disambiguation field populated
        // The UI should check for this field and render clickable buttons
        AgentChatResponse {
            message,
            intents: vec![],
            validation_results: vec![],
            session_state: SessionState::PendingValidation,
            can_execute: false,
            dsl_source: None,
            ast: None,
            disambiguation: None, // Legacy entity disambiguation
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: Some(disambiguation_request),
            intent_tier: None,
            debug: None,
            verb_candidates: vec![],
        }
    }

    /// Build an intent tier clarification response
    ///
    /// When verb candidates span multiple intents (navigate vs create vs modify),
    /// we first ask the user to clarify their intent before showing specific verbs.
    /// This reduces cognitive load and creates richer learning signals.
    fn build_intent_tier_response(
        &self,
        original_input: &str,
        candidates: &[crate::mcp::verb_search::VerbSearchResult],
        analysis: &crate::dsl_v2::intent_tiers::TierAnalysis,
        session: &mut UnifiedSession,
    ) -> AgentChatResponse {
        let intent_taxonomy = crate::dsl_v2::intent_tiers::intent_tier_taxonomy();

        // Build tier 1 request
        let tier_request = intent_taxonomy.build_tier1_request(original_input, analysis);

        // Build message for display
        let options_text: Vec<String> = tier_request
            .options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                format!(
                    "{}. **{}**: {} ({} options)",
                    i + 1,
                    opt.label,
                    opt.description,
                    opt.verb_count
                )
            })
            .collect();

        let message = format!(
            "I'm not sure what you mean by \"{}\". What are you trying to do?\n\n{}\n\nType a number to select.",
            original_input,
            options_text.join("\n")
        );

        session.add_agent_message(message.clone(), None, None);

        // Store pending intent tier state for selection handling
        use crate::session::unified::{PendingIntentTier, VerbCandidate};
        session.pending_intent_tier = Some(PendingIntentTier {
            request_id: tier_request.request_id.clone(),
            tier_number: 1,
            original_input: original_input.to_string(),
            candidates: candidates
                .iter()
                .map(|c| VerbCandidate {
                    verb: c.verb.clone(),
                    score: c.score,
                })
                .collect(),
            selected_path: vec![],
            created_at: chrono::Utc::now(),
        });

        AgentChatResponse {
            message,
            intents: vec![],
            validation_results: vec![],
            session_state: SessionState::PendingValidation,
            can_execute: false,
            dsl_source: None,
            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: Some(tier_request),
            debug: None,
            verb_candidates: vec![],
        }
    }

    /// Build VerbIntent from pipeline result
    fn build_intent(&self, result: &crate::mcp::intent_pipeline::PipelineResult) -> VerbIntent {
        let params = result
            .intent
            .arguments
            .iter()
            .map(|arg| (arg.name.clone(), intent_arg_to_param_value(&arg.value)))
            .collect();
        VerbIntent {
            verb: result.intent.verb.clone(),
            params,
            refs: HashMap::new(),
            lookups: None,
            sequence: None,
        }
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
        session: &mut UnifiedSession,
        response: &crate::api::session::DisambiguationResponse,
        _llm_client: Arc<dyn LlmClient>,
    ) -> Result<AgentChatResponse, String> {
        use crate::api::session::DisambiguationSelection;

        // Build resolved lookups from user's selections
        let mut resolved: HashMap<String, ResolvedEntityLookup> = HashMap::new();
        for selection in &response.selections {
            match selection {
                DisambiguationSelection::Entity {
                    param,
                    entity_id,
                    display_name,
                } => {
                    resolved.insert(
                        param.clone(),
                        ResolvedEntityLookup {
                            // Use display_name if provided, otherwise fall back to UUID string
                            display_name: display_name
                                .clone()
                                .unwrap_or_else(|| entity_id.to_string()),
                            entity_id: *entity_id,
                        },
                    );
                }
                DisambiguationSelection::Interpretation { .. } => {
                    // Handle interpretation selections if needed
                }
            }
        }

        // Get original intents from session's pending disambiguation
        let mut original_intents = session.pending_intents.clone();

        if original_intents.is_empty() {
            return Err("No original intents available for disambiguation".to_string());
        }

        // Inject resolved entity lookups into intents
        for intent in &mut original_intents {
            for (param, lookup) in &resolved {
                intent.params.insert(
                    param.clone(),
                    ParamValue::ResolvedEntity {
                        display_name: lookup.display_name.clone(),
                        resolved_id: lookup.entity_id,
                    },
                );
            }
            intent.lookups = None;
        }

        // Also resolve code values (products, roles, jurisdictions)
        let modified_intents = match self.resolve_codes_only(&mut original_intents).await {
            Ok(corrections) => {
                for (param, from, to) in &corrections {
                    tracing::info!("Resolved {}: '{}' → '{}'", param, from, to);
                }
                original_intents
            }
            Err(e) => return Err(e),
        };

        // Validate
        let validation_results: Vec<IntentValidation> =
            modified_intents.iter().map(validate_intent).collect();

        let all_valid = validation_results.iter().all(|v| v.valid);

        // Build DSL - both execution and user versions
        let (exec_dsl, user_dsl) = if !modified_intents.is_empty() && all_valid {
            (
                Some(build_dsl_program(&modified_intents)),
                Some(build_user_dsl_program(&modified_intents)),
            )
        } else {
            (None, None)
        };

        self.build_response(
            session,
            modified_intents,
            validation_results,
            exec_dsl,
            user_dsl,
            "Entities resolved. DSL ready for execution.".to_string(),
        )
        .await
    }

    // =============================================================================
    // UNIFIED DSL PIPELINE - One Path, Same Path
    // =============================================================================
    // ALL user input goes through the semantic intent pipeline (Candle embeddings).
    // Navigation phrases ("enhance", "zoom in", "drill") are matched to view.* and
    // session.* verbs just like any other DSL verb.
    //
    // The LLM handles all cases. Whether the user types:
    // - "add custody to Allianz" (natural language)
    // - "zoom in on that" (navigation)
    // - "(cbu.add-product :product CUSTODY)" (direct DSL)
    //
    // The result is always: valid DSL ready for execution.
    // One path. Same path. Quality design.
    // =============================================================================

    /// Handle REPL control commands (run, execute, undo, clear, etc.)
    ///
    /// These are NOT DSL-generating commands - they control the REPL session state.
    /// They bypass the LLM entirely because they're instant control commands.
    ///
    /// Returns Some(response) if the message is a REPL command, None otherwise.
    fn handle_repl_command(
        &self,
        message: &str,
        session: &UnifiedSession,
    ) -> Option<AgentChatResponse> {
        let msg = message.trim().to_lowercase();

        // Execute/Run commands - trigger execution of pending DSL
        if msg == "run" || msg == "execute" || msg == "go" || msg == "do it" {
            // Check if there's pending DSL to execute
            if session.can_execute() {
                return Some(AgentChatResponse {
                    message: "Executing...".to_string(),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: session.state.clone(),
                    can_execute: true,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: Some(vec![AgentCommand::Execute]),
                    unresolved_refs: None,
                    current_ref_index: None,
                    dsl_hash: None,
                    verb_disambiguation: None,
                    intent_tier: None,
                    debug: None,
                    verb_candidates: vec![],
                });
            } else {
                return Some(AgentChatResponse {
                    message: "Nothing to execute. Generate some DSL first.".to_string(),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: session.state.clone(),
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: None,
                    unresolved_refs: None,
                    current_ref_index: None,
                    dsl_hash: None,
                    verb_disambiguation: None,
                    intent_tier: None,
                    debug: None,
                    verb_candidates: vec![],
                });
            }
        }

        // Undo command
        if msg == "undo" {
            return Some(AgentChatResponse {
                message: "Undoing last action...".to_string(),
                intents: vec![],
                validation_results: vec![],
                session_state: session.state.clone(),
                can_execute: false,
                dsl_source: None,
                ast: None,
                disambiguation: None,
                commands: Some(vec![AgentCommand::Undo]),
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
                verb_disambiguation: None,
                intent_tier: None,
                debug: None,
                verb_candidates: vec![],
            });
        }

        // Clear command
        if msg == "clear" || msg == "reset" {
            return Some(AgentChatResponse {
                message: "Clearing session...".to_string(),
                intents: vec![],
                validation_results: vec![],
                session_state: session.state.clone(),
                can_execute: false,
                dsl_source: None,
                ast: None,
                disambiguation: None,
                commands: Some(vec![AgentCommand::Clear]),
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
                verb_disambiguation: None,
                intent_tier: None,
                debug: None,
                verb_candidates: vec![],
            });
        }

        // Not a REPL command - let it flow to LLM
        None
    }

    /// Collect all entity lookups from intents
    /// Unified reference resolution: resolves ALL references (entities + codes) in a single pass.
    ///
    /// This replaces the old 3-method approach:
    /// - collect_lookups() → absorbed
    /// - resolve_lookups() → absorbed
    /// - inject_resolved_ids() → resolution modifies intents in place
    ///
    /// One method. One Gateway connection. One pattern.
    async fn resolve_all(&self, mut intents: Vec<VerbIntent>) -> UnifiedResolution {
        let mut resolver = match GatewayRefResolver::connect(&self.config.gateway_addr).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("EntityGateway not available: {}", e);
                // Return intents as-is - CSG linter will catch issues later
                return UnifiedResolution::Resolved {
                    intents,
                    corrections: vec![],
                };
            }
        };

        let mut disambiguations: Vec<DisambiguationItem> = Vec::new();
        let mut corrections: Vec<(String, String, String)> = Vec::new();

        for intent in &mut intents {
            // 1. Resolve entity lookups → UUIDs
            if let Some(lookups) = intent.lookups.take() {
                for (param, lookup_val) in lookups {
                    let lookup: EntityLookup = match serde_json::from_value(lookup_val) {
                        Ok(l) => l,
                        Err(_) => continue,
                    };

                    // Determine ref type from entity_type hint
                    let ref_type = match lookup.entity_type.as_deref() {
                        Some("person") | Some("proper_person") => RefType::Entity,
                        Some("company") | Some("limited_company") | Some("legal_entity") => {
                            RefType::Entity
                        }
                        // "group" = apex entity in ownership hierarchy (e.g., Allianz SE, BlackRock Inc)
                        // These are regular entities used as roots of control_edges
                        Some("group") | Some("apex") | Some("holding") => RefType::Entity,
                        // "client_group" = virtual client brand/nickname (e.g., "Allianz", "BlackRock")
                        // Resolution: two-stage - nickname → group_id → anchor_entity_id
                        Some("client_group") | Some("client") => RefType::ClientGroup,
                        Some("cbu") => RefType::Cbu,
                        Some("product") => RefType::Product,
                        Some("role") => RefType::Role,
                        Some("jurisdiction") => RefType::Jurisdiction,
                        _ => RefType::Entity,
                    };

                    match resolver.resolve(ref_type, &lookup.search_text).await {
                        Ok(ResolveResult::Found { id, .. }) => {
                            intent.params.insert(
                                param,
                                ParamValue::ResolvedEntity {
                                    display_name: lookup.search_text,
                                    resolved_id: id,
                                },
                            );
                        }
                        Ok(ResolveResult::FoundByCode { uuid: Some(id), .. }) => {
                            intent.params.insert(
                                param,
                                ParamValue::ResolvedEntity {
                                    display_name: lookup.search_text,
                                    resolved_id: id,
                                },
                            );
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
                                disambiguations.push(DisambiguationItem::EntityMatch {
                                    param,
                                    search_text: lookup.search_text.clone(),
                                    matches,
                                    entity_type: lookup.entity_type.clone(),
                                    search_column: None, // Legacy path doesn't have search_column
                                    ref_id: None,        // Legacy path doesn't have ref_id
                                });
                            }
                        }
                        Ok(ResolveResult::NotFound { .. }) => {
                            tracing::debug!(
                                "No matches for '{}' (param: {})",
                                lookup.search_text,
                                param
                            );
                        }
                        Err(e) => {
                            return UnifiedResolution::Error(format!("Lookup failed: {}", e));
                        }
                    }
                }
            }

            // 2. Resolve code params → canonical codes
            for (param_name, ref_type) in CODE_PARAMS {
                let raw_value = match intent.params.get(*param_name) {
                    Some(ParamValue::String(s)) if !s.trim().is_empty() => s.clone(),
                    _ => continue,
                };

                match resolver.resolve(*ref_type, &raw_value).await {
                    Ok(ResolveResult::Found { id, .. }) => {
                        let canonical = id.to_string();
                        if canonical != raw_value {
                            corrections.push((
                                param_name.to_string(),
                                raw_value,
                                canonical.clone(),
                            ));
                        }
                        intent
                            .params
                            .insert(param_name.to_string(), ParamValue::String(canonical));
                    }
                    Ok(ResolveResult::FoundByCode { code, .. }) => {
                        if code != raw_value {
                            corrections.push((param_name.to_string(), raw_value, code.clone()));
                        }
                        intent
                            .params
                            .insert(param_name.to_string(), ParamValue::String(code));
                    }
                    Ok(ResolveResult::NotFound { suggestions })
                        if suggestions.first().map(|s| s.score > 0.7).unwrap_or(false) =>
                    {
                        // High confidence fuzzy match - auto-correct
                        let best = &suggestions[0];
                        corrections.push((param_name.to_string(), raw_value, best.value.clone()));
                        intent.params.insert(
                            param_name.to_string(),
                            ParamValue::String(best.value.clone()),
                        );
                    }
                    Ok(ResolveResult::NotFound { suggestions }) if !suggestions.is_empty() => {
                        // Low confidence - return error with suggestions
                        let suggestion_list: Vec<&str> = suggestions
                            .iter()
                            .take(3)
                            .map(|s| s.value.as_str())
                            .collect();
                        return UnifiedResolution::Error(format!(
                            "Unknown {}: '{}'. Try: {}",
                            param_name,
                            raw_value,
                            suggestion_list.join(", ")
                        ));
                    }
                    Ok(ResolveResult::NotFound { .. }) => {
                        return UnifiedResolution::Error(format!(
                            "Unknown {} code: '{}'. Check available codes.",
                            param_name, raw_value
                        ));
                    }
                    Err(e) => {
                        // Resolution error - log but continue (CSG linter will catch)
                        tracing::warn!(
                            "Code resolution failed for {} '{}': {}",
                            param_name,
                            raw_value,
                            e
                        );
                    }
                }
            }
        }

        if disambiguations.is_empty() {
            UnifiedResolution::Resolved {
                intents,
                corrections,
            }
        } else {
            UnifiedResolution::NeedsDisambiguation {
                items: disambiguations,
                partial_intents: intents,
            }
        }
    }

    /// Resolve only code values (products, roles, jurisdictions, etc.) in intents.
    /// Used after disambiguation when entities are already resolved.
    async fn resolve_codes_only(
        &self,
        intents: &mut [VerbIntent],
    ) -> Result<Vec<(String, String, String)>, String> {
        let mut corrections: Vec<(String, String, String)> = Vec::new();

        let mut resolver = match GatewayRefResolver::connect(&self.config.gateway_addr).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("EntityGateway not available for code resolution: {}", e);
                return Ok(corrections);
            }
        };

        for intent in intents.iter_mut() {
            for (param_name, ref_type) in CODE_PARAMS {
                let raw_value = match intent.params.get(*param_name) {
                    Some(ParamValue::String(s)) if !s.trim().is_empty() => s.clone(),
                    _ => continue,
                };

                match resolver.resolve(*ref_type, &raw_value).await {
                    Ok(ResolveResult::Found { id, .. }) => {
                        let canonical = id.to_string();
                        if canonical != raw_value {
                            corrections.push((
                                param_name.to_string(),
                                raw_value,
                                canonical.clone(),
                            ));
                        }
                        intent
                            .params
                            .insert(param_name.to_string(), ParamValue::String(canonical));
                    }
                    Ok(ResolveResult::FoundByCode { code, .. }) => {
                        if code != raw_value {
                            corrections.push((param_name.to_string(), raw_value, code.clone()));
                        }
                        intent
                            .params
                            .insert(param_name.to_string(), ParamValue::String(code));
                    }
                    Ok(ResolveResult::NotFound { suggestions })
                        if suggestions.first().map(|s| s.score > 0.7).unwrap_or(false) =>
                    {
                        let best = &suggestions[0];
                        corrections.push((param_name.to_string(), raw_value, best.value.clone()));
                        intent.params.insert(
                            param_name.to_string(),
                            ParamValue::String(best.value.clone()),
                        );
                    }
                    Ok(ResolveResult::NotFound { suggestions }) if !suggestions.is_empty() => {
                        let suggestion_list: Vec<&str> = suggestions
                            .iter()
                            .take(3)
                            .map(|s| s.value.as_str())
                            .collect();
                        return Err(format!(
                            "Unknown {}: '{}'. Try: {}",
                            param_name,
                            raw_value,
                            suggestion_list.join(", ")
                        ));
                    }
                    Ok(ResolveResult::NotFound { .. }) => {
                        return Err(format!(
                            "Unknown {} code: '{}'. Check available codes.",
                            param_name, raw_value
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Code resolution failed for {} '{}': {}",
                            param_name,
                            raw_value,
                            e
                        );
                    }
                }
            }
        }

        Ok(corrections)
    }

    /// Build final response with DSL
    /// Takes both execution DSL (with UUIDs for DB) and user DSL (with display names for chat)
    async fn build_response(
        &self,
        session: &mut UnifiedSession,
        intents: Vec<VerbIntent>,
        validation_results: Vec<IntentValidation>,
        exec_dsl: Option<String>,
        user_dsl: Option<String>,
        explanation: String,
    ) -> Result<AgentChatResponse, String> {
        let all_valid = validation_results.iter().all(|v| v.valid);

        // Parse to AST and compile to ExecutionPlan (includes DAG toposort)
        // Single pipeline: Parse → Compile (with toposort) → Ready for execution
        // NOTE: We parse the EXECUTION DSL (with UUIDs) for actual execution
        let (ast, plan, was_reordered): (
            Option<Vec<Statement>>,
            Option<crate::dsl_v2::execution_plan::ExecutionPlan>,
            bool,
        ) = if let Some(ref src) = exec_dsl {
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

        // Store execution DSL in session for actual execution
        if let Some(ref exec_dsl_source) = exec_dsl {
            if let Some(ref ast_statements) = ast {
                session.set_pending_dsl(
                    exec_dsl_source.clone(),
                    ast_statements.clone(),
                    plan.clone(),
                    was_reordered,
                );
            }
            if all_valid {
                session.state = SessionState::ReadyToExecute;
            } else {
                session.state = SessionState::PendingValidation;
            }
        }

        session.add_agent_message(explanation.clone(), None, user_dsl.clone());

        let combined_dsl = session.run_sheet.combined_dsl();

        // Include active CBU name in response message for clarity
        let message = if let Some(ref cbu) = session.context.active_cbu {
            format!("[{}] {}", cbu.display_name, explanation)
        } else {
            explanation
        };

        // Check for unresolved entity references in the AST
        // This enables the 3-stage compiler model: Syntax → Semantics → Resolution
        let (unresolved_refs, current_ref_index) = if let Some(ref ast_statements) = ast {
            let resolution = ResolutionSubSession::from_statements(ast_statements);
            if !resolution.unresolved_refs.is_empty() {
                tracing::info!(
                    "Found {} unresolved refs in AST, triggering resolution",
                    resolution.unresolved_refs.len()
                );
                // Mark session as needing resolution
                session.state = SessionState::PendingValidation;
                (Some(resolution.unresolved_refs), Some(0usize))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Set can_execute flag
        // Can't execute if there are unresolved refs
        let can_execute = session.can_execute() && all_valid && unresolved_refs.is_none();

        // Auto-execute safe navigation commands (session.*, view.* verbs)
        // These don't modify data, just change what the user is viewing
        let should_auto_execute = can_execute
            && intents.iter().all(|intent| {
                let domain = intent.verb.split('.').next().unwrap_or("");
                matches!(domain, "session" | "view")
            })
            && !intents.is_empty();

        let commands: Option<Vec<AgentCommand>> = if should_auto_execute {
            Some(vec![AgentCommand::Execute])
        } else {
            None
        };

        // Compute dsl_hash for resolution commits (Issue K)
        // Only needed when there are unresolved refs
        let dsl_hash = if unresolved_refs.is_some() {
            combined_dsl.as_ref().map(|dsl| compute_dsl_hash(dsl))
        } else {
            None
        };

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
            unresolved_refs,
            current_ref_index,
            dsl_hash,
            verb_disambiguation: None,
            intent_tier: None,
            debug: None,
            verb_candidates: vec![],
        })
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
    ///
    /// For client_group type, uses PgClientGroupResolver with semantic search.
    pub async fn search_entities(
        &self,
        entity_type: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatchOption>, String> {
        self.search_entities_in_scope(entity_type, query, limit, None)
            .await
    }

    /// Search for entities within a client group scope
    ///
    /// When `client_group_id` is provided, results are filtered to entities
    /// that belong to that client group. This prevents cross-client entity
    /// leakage and improves disambiguation accuracy.
    pub async fn search_entities_in_scope(
        &self,
        entity_type: &str,
        query: &str,
        limit: usize,
        client_group_id: Option<Uuid>,
    ) -> Result<Vec<EntityMatchOption>, String> {
        // Special handling for client_group - uses PgClientGroupResolver
        if entity_type == "client_group" || entity_type == "client" {
            return self.search_client_groups(query, limit).await;
        }

        // If we have a client_group_id, use the scoped search function
        if let Some(group_id) = client_group_id {
            return self
                .search_entities_by_client_group(entity_type, query, limit, group_id)
                .await;
        }

        // No scope - fall through to global search
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

    /// Search for entities within a specific client group
    ///
    /// Uses typed search dispatch: CBU slots search CBUs, entity slots search entities.
    /// This is THE core fix for "Allianz means different things depending on context".
    async fn search_entities_by_client_group(
        &self,
        entity_type: &str,
        query: &str,
        limit: usize,
        client_group_id: Uuid,
    ) -> Result<Vec<EntityMatchOption>, String> {
        use crate::mcp::scope_resolution::{search_in_scope, ScopeContext};

        // Build scope context with the client group
        let scope = ScopeContext::new().with_client_group(client_group_id, String::new());

        // TYPE-DISPATCHED SEARCH: The key fix
        // When entity_type is "cbu", we search CBUs, not entities
        let matches = search_in_scope(&self.pool, &scope, entity_type, query, limit)
            .await
            .map_err(|e| format!("Scoped search failed: {}", e))?;

        // Map to EntityMatchOption with typed metadata
        Ok(matches
            .into_iter()
            .map(|m| EntityMatchOption {
                entity_id: m.id,
                name: m.name,
                entity_type: m.result_type, // Preserve actual type from search
                jurisdiction: None,
                context: Some(format!("{} ({})", m.display_kind, m.match_type)),
                score: Some(m.confidence as f32),
            })
            .collect())
    }

    /// Search client groups using PgClientGroupResolver with semantic search
    async fn search_client_groups(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatchOption>, String> {
        use ob_semantic_matcher::client_group_resolver::ClientGroupAliasResolver;

        let adapter = ClientGroupEmbedderAdapter(self.embedder.clone());
        let resolver = ob_semantic_matcher::client_group_resolver::PgClientGroupResolver::new(
            self.pool.clone(),
            Arc::new(adapter),
            "BAAI/bge-small-en-v1.5".to_string(),
        );

        let matches = resolver
            .search_aliases(query, limit)
            .await
            .map_err(|e| format!("Client group search failed: {}", e))?;

        Ok(matches
            .into_iter()
            .map(|m| EntityMatchOption {
                entity_id: m.group_id,
                name: m.canonical_name,
                entity_type: "client_group".to_string(),
                jurisdiction: None,
                context: Some(format!("Matched: {}", m.matched_alias)),
                score: Some(m.similarity_score),
            })
            .collect())
    }

    /// Resolve a single entity by exact name match
    ///
    /// Returns the entity if exactly one match is found,
    /// or a list of suggestions if multiple/no matches.
    ///
    /// For client_group type, uses PgClientGroupResolver with semantic search.
    pub async fn resolve_entity(
        &self,
        entity_type: &str,
        name: &str,
    ) -> Result<ResolveResult, String> {
        // Special handling for client_group - uses PgClientGroupResolver
        if entity_type == "client_group" || entity_type == "client" {
            return self.resolve_client_group(name).await;
        }

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

    /// Resolve a client group by name using PgClientGroupResolver
    async fn resolve_client_group(&self, name: &str) -> Result<ResolveResult, String> {
        use crate::dsl_v2::ref_resolver::SuggestedMatch;
        use ob_semantic_matcher::client_group_resolver::{
            ClientGroupAliasResolver, ClientGroupResolveError, ResolutionConfig,
        };

        let adapter = ClientGroupEmbedderAdapter(self.embedder.clone());
        let resolver = ob_semantic_matcher::client_group_resolver::PgClientGroupResolver::new(
            self.pool.clone(),
            Arc::new(adapter),
            "BAAI/bge-small-en-v1.5".to_string(),
        );

        let config = ResolutionConfig::default();

        match resolver.resolve_alias(name, &config).await {
            Ok(m) => {
                // Single confident match
                Ok(ResolveResult::Found {
                    id: m.group_id,
                    display: m.canonical_name,
                })
            }
            Err(ClientGroupResolveError::Ambiguous { candidates, .. }) => {
                // Multiple candidates - return suggestions
                let suggestions = candidates
                    .into_iter()
                    .map(|c| SuggestedMatch {
                        value: c.group_id.to_string(),
                        display: c.canonical_name,
                        score: c.similarity_score,
                    })
                    .collect();
                Ok(ResolveResult::NotFound { suggestions })
            }
            Err(ClientGroupResolveError::NoMatch(_)) => {
                // No match - return empty suggestions
                Ok(ResolveResult::NotFound {
                    suggestions: vec![],
                })
            }
            Err(e) => Err(format!("Client group resolution failed: {}", e)),
        }
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

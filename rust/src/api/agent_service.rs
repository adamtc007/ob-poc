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
use crate::api::session::{
    AgentSession, DisambiguationItem, DisambiguationRequest, EntityMatchOption,
    ResolutionSubSession, SessionState, UnresolvedRefInfo,
};
use crate::database::{derive_semantic_state, VerbService};
use crate::dsl_v2::gateway_resolver::{gateway_addr, GatewayRefResolver};
use crate::dsl_v2::ref_resolver::ResolveResult;
use crate::dsl_v2::semantic_validator::SemanticValidator;
use crate::dsl_v2::validation::{RefType, Severity, ValidationContext, ValidationRequest};
use crate::dsl_v2::verb_registry::registry;
use crate::dsl_v2::Statement;
use crate::mcp::intent_pipeline::{compute_dsl_hash, IntentArgValue, IntentPipeline};
use crate::mcp::verb_search::HybridVerbSearcher;
use crate::ontology::SemanticStageRegistry;
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
    client_scope: Option<ClientScope>,
    /// Embedder for semantic verb search (None = exact match only)
    embedder: Option<Arc<CandleEmbedder>>,
}

#[allow(dead_code)]
impl AgentService {
    /// Create agent service with pool and optional embedder
    pub fn new(pool: PgPool, embedder: Option<Arc<CandleEmbedder>>) -> Self {
        Self {
            pool,
            config: AgentServiceConfig::default(),
            client_scope: None,
            embedder,
        }
    }

    /// Create with database pool only (no semantic search)
    pub fn with_pool(pool: PgPool) -> Self {
        Self::new(pool, None)
    }

    /// Check if semantic search is available
    pub fn has_semantic_search(&self) -> bool {
        self.embedder.is_some()
    }

    /// Create IntentPipeline for processing user input
    fn get_intent_pipeline(&self) -> Result<IntentPipeline, String> {
        let verb_service = Arc::new(VerbService::new(self.pool.clone()));
        let mut searcher = HybridVerbSearcher::new(verb_service, None);

        if let Some(ref embedder) = self.embedder {
            // Cast Arc<CandleEmbedder> to Arc<dyn Embedder>
            let dyn_embedder: Arc<dyn crate::agent::learning::embedder::Embedder> =
                embedder.clone() as Arc<dyn crate::agent::learning::embedder::Embedder>;
            searcher = searcher.with_embedder(dyn_embedder);
        }

        Ok(IntentPipeline::new(searcher))
    }

    /// Create a client-scoped agent service
    pub fn for_client(pool: PgPool, client_id: Uuid, accessible_cbus: Vec<Uuid>) -> Self {
        Self {
            pool,
            config: AgentServiceConfig::default(),
            client_scope: Some(ClientScope::new(client_id, accessible_cbus)),
            embedder: None,
        }
    }

    /// Create a client-scoped agent service with name
    pub fn for_client_named(
        pool: PgPool,
        client_id: Uuid,
        accessible_cbus: Vec<Uuid>,
        client_name: String,
    ) -> Self {
        let mut scope = ClientScope::new(client_id, accessible_cbus);
        scope.client_name = Some(client_name);
        Self {
            pool,
            config: AgentServiceConfig::default(),
            client_scope: Some(scope),
            embedder: None,
        }
    }

    /// Check if operating in client mode
    pub fn is_client_mode(&self) -> bool {
        self.client_scope.is_some()
    }

    /// Get the client scope (if in client mode)
    pub fn client_scope(&self) -> Option<&ClientScope> {
        self.client_scope.as_ref()
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
    fn derive_taxonomy_context(&self, session: &AgentSession) -> Option<String> {
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

        // If this is a disambiguation response, handle it
        if let Some(disambig_response) = &request.disambiguation_response {
            return self
                .handle_disambiguation_response(session, disambig_response, llm_client.clone())
                .await;
        }

        // Process via IntentPipeline: user input → semantic verb search → LLM arg extraction → DSL
        if let Ok(pipeline) = self.get_intent_pipeline() {
            // Pass existing scope context from session (for subsequent commands after scope is set)
            let existing_scope = session.context.client_scope.clone();
            let pipeline_result = pipeline
                .process_with_scope(&request.message, None, existing_scope)
                .await;

            match pipeline_result {
                Ok(result) => {
                    // =========================================================
                    // STAGE 0: Handle Scope Resolution (before verb processing)
                    // =========================================================
                    // If scope was resolved, store it in session and return success
                    // This consumes the input - no verb processing happens
                    use crate::mcp::intent_pipeline::PipelineOutcome;

                    if let PipelineOutcome::ScopeResolved {
                        group_id,
                        group_name,
                        entity_count,
                    } = &result.outcome
                    {
                        tracing::info!(
                            "Scope resolved: {} ({}, {} entities)",
                            group_name,
                            group_id,
                            entity_count
                        );

                        // Store scope context in session
                        if let Some(scope_ctx) = result.scope_context.clone() {
                            session.context.set_client_scope(scope_ctx);
                        }

                        session.add_user_message(request.message.clone());
                        let response_msg = format!(
                            "Now working on **{}** ({} entities)",
                            group_name, entity_count
                        );
                        session.add_agent_message(response_msg.clone(), None, None);

                        // Transition to scoped state
                        session.transition(crate::api::session::SessionEvent::ScopeSet);

                        return Ok(AgentChatResponse {
                            message: response_msg,
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
                        });
                    }

                    // Handle scope candidates (user needs to pick)
                    if let PipelineOutcome::ScopeCandidates = &result.outcome {
                        session.add_user_message(request.message.clone());

                        // Build disambiguation for client group selection
                        let error_msg = result
                            .validation_error
                            .clone()
                            .unwrap_or_else(|| "Multiple clients match".to_string());
                        session.add_agent_message(error_msg.clone(), None, None);

                        return Ok(AgentChatResponse {
                            message: error_msg,
                            intents: vec![],
                            validation_results: vec![],
                            session_state: SessionState::PendingValidation,
                            can_execute: false,
                            dsl_source: None,
                            ast: None,
                            disambiguation: None, // TODO: Build client group picker
                            commands: None,
                            unresolved_refs: None,
                            current_ref_index: None,
                            dsl_hash: None,
                        });
                    }

                    // =========================================================
                    // Normal verb processing (scope context passed through)
                    // =========================================================

                    // Store scope context in session if present (for subsequent operations)
                    if let Some(scope_ctx) = result.scope_context.clone() {
                        if scope_ctx.has_scope() {
                            session.context.set_client_scope(scope_ctx);
                        }
                    }

                    tracing::info!(
                        "IntentPipeline matched verb: {} (score: {:.2})",
                        result.intent.verb,
                        result
                            .verb_candidates
                            .first()
                            .map(|c| c.score)
                            .unwrap_or(0.0)
                    );

                    // If we have unresolved entity refs, trigger disambiguation using pipeline result
                    if !result.unresolved_refs.is_empty() {
                        tracing::info!(
                            "Pipeline has {} unresolved refs, triggering disambiguation",
                            result.unresolved_refs.len()
                        );

                        session.add_user_message(request.message.clone());

                        // Convert pipeline unresolved refs to disambiguation items (Fix K)
                        // Pre-populate matches with actual entity search results
                        let mut disambig_items: Vec<DisambiguationItem> = Vec::new();
                        // Get client_group_id from session scope for scoped entity search
                        let scope_group_id = session.context.client_group_id();
                        for r in &result.unresolved_refs {
                            // Search for matching entities within scope (if scope is set)
                            let entity_type_str = r.entity_type.as_deref().unwrap_or("entity");
                            let matches = self
                                .search_entities_in_scope(
                                    entity_type_str,
                                    &r.search_value,
                                    10,
                                    scope_group_id,
                                )
                                .await
                                .unwrap_or_default();

                            tracing::info!(
                                "Pre-populated {} matches for '{}' (type: {}, scope: {:?})",
                                matches.len(),
                                r.search_value,
                                entity_type_str,
                                scope_group_id
                            );

                            disambig_items.push(DisambiguationItem::EntityMatch {
                                param: r.param_name.clone(),
                                search_text: r.search_value.clone(),
                                matches,
                                entity_type: r.entity_type.clone(),
                                search_column: r.search_column.clone(),
                                ref_id: r.ref_id.clone(),
                            });
                        }

                        // Build VerbIntent from pipeline result
                        let mut params: HashMap<String, ParamValue> = HashMap::new();
                        for arg in &result.intent.arguments {
                            let value = intent_arg_to_param_value(&arg.value);
                            params.insert(arg.name.clone(), value);
                        }

                        let intent = VerbIntent {
                            verb: result.intent.verb.clone(),
                            params,
                            refs: HashMap::new(),
                            lookups: None,
                            sequence: None,
                        };

                        // Store intent in session for after disambiguation
                        session.pending_intents = vec![intent.clone()];

                        // Parse and ENRICH DSL to AST - enrichment converts strings to EntityRef
                        // based on verb arg lookup config. Without enrichment, resolve-by-ref-id
                        // won't find any EntityRef nodes to update.
                        let dsl_hash = if let Ok(program) =
                            dsl_core::parser::parse_program(&result.dsl)
                        {
                            // Enrich to convert String literals to EntityRef where lookup config exists
                            let registry = crate::dsl_v2::runtime_registry::runtime_registry_arc();
                            let enriched =
                                crate::dsl_v2::enrichment::enrich_program(program, &registry);
                            session.context.ast = enriched.program.statements;
                            tracing::info!(
                                    "Stored {} statements in session.context.ast for disambiguation (enriched)",
                                    session.context.ast.len()
                                );
                            // Hash the re-serialized DSL to ensure consistency
                            Some(compute_dsl_hash(&session.context.to_dsl_source()))
                        } else {
                            // Fallback to pipeline DSL if parsing fails
                            Some(compute_dsl_hash(&result.dsl))
                        };

                        let disambig = DisambiguationRequest {
                            request_id: Uuid::new_v4(),
                            items: disambig_items,
                            prompt: format!(
                                "Please select the correct entities for {}:",
                                result.intent.verb
                            ),
                            original_intents: Some(vec![intent.clone()]),
                        };

                        let response_msg = format!(
                            "I need to resolve some entities for `{}`",
                            result.intent.verb
                        );
                        session.add_agent_message(response_msg.clone(), None, None);
                        session.state = SessionState::PendingValidation;

                        return Ok(AgentChatResponse {
                            message: response_msg,
                            intents: vec![intent],
                            validation_results: vec![],
                            session_state: SessionState::PendingValidation,
                            can_execute: false,
                            dsl_source: Some(result.dsl),
                            ast: None,
                            disambiguation: Some(disambig),
                            commands: None,
                            unresolved_refs: None,
                            current_ref_index: None,
                            dsl_hash,
                        });
                    } else if result.valid {
                        // Valid DSL with all refs resolved - ready to execute
                        session.add_user_message(request.message.clone());

                        // Parse to AST for response - extract statements
                        let ast = dsl_core::parser::parse_program(&result.dsl)
                            .ok()
                            .map(|p| p.statements);

                        // Build response with the pipeline result
                        let response_msg = if result.intent.notes.is_empty() {
                            format!("Generated: {}", result.dsl)
                        } else {
                            result.intent.notes.join("\n")
                        };
                        // Build VerbIntent with proper params HashMap
                        let mut params: HashMap<String, ParamValue> = HashMap::new();
                        for arg in &result.intent.arguments {
                            let value = intent_arg_to_param_value(&arg.value);
                            params.insert(arg.name.clone(), value);
                        }

                        let intent = VerbIntent {
                            verb: result.intent.verb.clone(),
                            params,
                            refs: HashMap::new(),
                            lookups: None,
                            sequence: None,
                        };

                        session.add_agent_message(
                            response_msg.clone(),
                            Some(vec![intent.clone()]),
                            Some(result.dsl.clone()),
                        );

                        // Add DSL to run_sheet so /execute can find it
                        session.set_pending_dsl(
                            result.dsl.clone(),
                            ast.clone().unwrap_or_default(),
                            None, // No pre-compiled plan
                            false,
                        );

                        // Auto-execute safe navigation commands (session.*, view.* verbs)
                        // These don't modify data, just change what the user is viewing
                        let domain = result.intent.verb.split('.').next().unwrap_or("");
                        let should_auto_execute = matches!(domain, "session" | "view");
                        let commands = if should_auto_execute {
                            Some(vec![AgentCommand::Execute])
                        } else {
                            None
                        };

                        return Ok(AgentChatResponse {
                            message: response_msg,
                            intents: vec![intent.clone()],
                            validation_results: vec![IntentValidation {
                                valid: true,
                                intent,
                                errors: vec![],
                                warnings: vec![],
                            }],
                            session_state: SessionState::ReadyToExecute,
                            can_execute: true,
                            dsl_source: Some(result.dsl),
                            ast,
                            disambiguation: None,
                            commands,
                            unresolved_refs: None,
                            current_ref_index: None,
                            dsl_hash: None,
                        });
                    } else if !result.missing_required.is_empty() {
                        // Missing required arguments - ask user to clarify
                        let verb = &result.intent.verb;
                        let missing = result.missing_required.join(", ");

                        tracing::info!(
                            "Verb {} matched but missing required args: {}",
                            verb,
                            missing
                        );

                        session.add_user_message(request.message.clone());

                        let clarification_msg = format!(
                            "I understood you want to use `{}`, but I need more information.\n\nPlease specify: {}",
                            verb, missing
                        );
                        session.add_agent_message(clarification_msg.clone(), None, None);

                        return Ok(AgentChatResponse {
                            message: clarification_msg,
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
                        });
                    } else {
                        // DSL validation failed - return error with details
                        let error_msg = result
                            .validation_error
                            .unwrap_or_else(|| "DSL validation failed".to_string());
                        tracing::warn!("Pipeline DSL validation failed: {}", error_msg);

                        session.add_user_message(request.message.clone());
                        session.add_agent_message(error_msg.clone(), None, None);

                        return Ok(AgentChatResponse {
                            message: format!(
                                "I understood `{}` but the DSL is invalid: {}",
                                result.intent.verb, error_msg
                            ),
                            intents: vec![],
                            validation_results: vec![],
                            session_state: SessionState::New,
                            can_execute: false,
                            dsl_source: Some(result.dsl),
                            ast: None,
                            disambiguation: None,
                            commands: None,
                            unresolved_refs: None,
                            current_ref_index: None,
                            dsl_hash: None,
                        });
                    }
                }
                Err(e) => {
                    // Pipeline failed - return error, don't fall through
                    tracing::error!("IntentPipeline error: {}", e);

                    session.add_user_message(request.message.clone());
                    let error_msg = format!("I couldn't understand that request: {}", e);
                    session.add_agent_message(error_msg.clone(), None, None);

                    return Ok(AgentChatResponse {
                        message: error_msg,
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
                    });
                }
            }
        }

        // =========================================================================
        // ERROR: IntentPipeline unavailable - system is down
        // =========================================================================
        // If we reach here, the database pool is unavailable. This is a critical
        // system failure - return error rather than pretending to work.
        tracing::error!("IntentPipeline unavailable - database pool not configured");
        Err("Service unavailable: database connection required".to_string())
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
        session: &AgentSession,
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
        session: &mut AgentSession,
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
    /// Uses the client_group_entity table to filter results to entities
    /// that belong to the specified client group.
    async fn search_entities_by_client_group(
        &self,
        entity_type: &str,
        query: &str,
        limit: usize,
        client_group_id: Uuid,
    ) -> Result<Vec<EntityMatchOption>, String> {
        use crate::mcp::scope_resolution::search_entities_in_scope;
        use crate::mcp::scope_resolution::ScopeContext;

        // Build scope context with just the client group
        let scope = ScopeContext::new().with_client_group(client_group_id, String::new());

        // Use the scope_resolution module's search function
        let matches = search_entities_in_scope(&self.pool, &scope, query, limit)
            .await
            .map_err(|e| format!("Scoped entity search failed: {}", e))?;

        // Map to EntityMatchOption
        Ok(matches
            .into_iter()
            .map(|m| EntityMatchOption {
                entity_id: m.entity_id,
                name: m.entity_name,
                entity_type: entity_type.to_string(),
                jurisdiction: None,
                context: Some(format!("Matched: {} ({})", m.matched_tag, m.match_type)),
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

        // Need embedder for semantic search
        let embedder = match &self.embedder {
            Some(e) => e.clone(),
            None => {
                return Err("Client group search requires embedder (semantic search)".to_string())
            }
        };

        let adapter = ClientGroupEmbedderAdapter(embedder);
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

        let embedder = match &self.embedder {
            Some(e) => e.clone(),
            None => {
                return Err("Client group resolution requires embedder".to_string());
            }
        };

        let adapter = ClientGroupEmbedderAdapter(embedder);
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

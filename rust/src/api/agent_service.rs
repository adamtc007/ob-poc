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

use crate::agent::esper::{EsperCommandRegistry, LookupResult};
use crate::agent::learning::embedder::CandleEmbedder;
use crate::agentic::llm_client::{LlmClient, ToolDefinition};
use crate::api::dsl_builder::{build_dsl_program, build_user_dsl_program, validate_intent};
use crate::api::intent::{IntentValidation, ParamValue, VerbIntent};
use crate::api::session::{
    AgentSession, DisambiguationItem, DisambiguationRequest, EntityMatchOption,
    ResolutionSubSession, SessionState, UnresolvedRefInfo,
};
use crate::database::derive_semantic_state;
use crate::dsl_v2::gateway_resolver::{gateway_addr, GatewayRefResolver};
use crate::dsl_v2::ref_resolver::ResolveResult;
use crate::dsl_v2::semantic_validator::SemanticValidator;
use crate::dsl_v2::validation::{RefType, Severity, ValidationContext, ValidationRequest};
use crate::dsl_v2::verb_registry::registry;
use crate::dsl_v2::Statement;
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
    /// Client scope (if operating in client portal mode)
    client_scope: Option<ClientScope>,
    /// ESPER command registry for instant navigation (optional)
    esper_registry: Option<Arc<EsperCommandRegistry>>,
    /// Embedder for ESPER semantic fallback (optional)
    embedder: Option<Arc<CandleEmbedder>>,
}

#[allow(dead_code)]
impl AgentService {
    /// Create a new agent service without database support
    pub fn new() -> Self {
        Self {
            pool: None,
            config: AgentServiceConfig::default(),
            client_scope: None,
            esper_registry: None,
            embedder: None,
        }
    }

    /// Create with database pool for semantic validation
    pub fn with_pool(pool: PgPool) -> Self {
        Self {
            pool: Some(pool),
            config: AgentServiceConfig::default(),
            client_scope: None,
            esper_registry: None,
            embedder: None,
        }
    }

    /// Create with custom configuration
    pub fn with_config(pool: Option<PgPool>, config: AgentServiceConfig) -> Self {
        Self {
            pool,
            config,
            client_scope: None,
            esper_registry: None,
            embedder: None,
        }
    }

    /// Set ESPER registry for instant navigation commands
    pub fn with_esper_registry(mut self, registry: Arc<EsperCommandRegistry>) -> Self {
        self.esper_registry = Some(registry);
        self
    }

    /// Set embedder for ESPER semantic fallback
    pub fn with_embedder(mut self, embedder: Arc<CandleEmbedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Create a client-scoped agent service for the client portal
    ///
    /// Client mode restricts:
    /// - Only accessible CBUs are visible
    /// - Limited verb palette (read + respond operations only)
    /// - Different system prompt (client-friendly, explains WHY)
    pub fn for_client(pool: PgPool, client_id: Uuid, accessible_cbus: Vec<Uuid>) -> Self {
        Self {
            pool: Some(pool),
            config: AgentServiceConfig::default(),
            client_scope: Some(ClientScope::new(client_id, accessible_cbus)),
            esper_registry: None,
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
            pool: Some(pool),
            config: AgentServiceConfig::default(),
            client_scope: Some(scope),
            esper_registry: None,
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
        let pool = self.pool.as_ref()?;

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
        let pool = self.pool.as_ref()?;

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

        // Handle ESPER navigation commands (bypass LLM)
        // These are instant UI commands like "enhance", "zoom in 2x", "universe"
        // ESPER is kept as fast-path because it's UI navigation, not DSL generation
        if let Some(response) = self.handle_esper_command(&request.message) {
            return Ok(response);
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

        // Get relevant verbs filter from stage focus (if set)
        let stage_verb_filter: Option<Vec<String>> =
            if let Some(ref stage_code) = session.context.stage_focus {
                // Load the semantic stage registry to get relevant verbs
                match SemanticStageRegistry::load_default() {
                    Ok(registry) => registry
                        .get_stage(stage_code)
                        .and_then(|s| s.relevant_verbs.clone()),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to load semantic stage registry for verb filtering: {}",
                            e
                        );
                        None
                    }
                }
            } else {
                None
            };

        // Build prompts with pre-resolved data and optional verb filtering
        let vocab = self.build_vocab_prompt(None, stage_verb_filter.as_deref());

        // Add stage focus context to the prompt if filtering is active
        let stage_focus_context = if let Some(ref stage_code) = session.context.stage_focus {
            format!(
                "\n\n## FOCUS: {} Stage\nYou are focused on the {} stage. Prioritize verbs relevant to this stage.",
                stage_code, stage_code
            )
        } else {
            String::new()
        };

        let system_prompt = format!(
            "{}{}{}",
            self.build_intent_extraction_prompt(&vocab),
            pre_resolved_context,
            stage_focus_context
        );

        // Build session context for LLM - include active CBU, scope, bindings, and semantic state
        let active_cbu_context = session.context.active_cbu_for_llm();
        let scope_context = session.context.scope_context_for_llm();

        // Derive semantic state if we have an active CBU
        let semantic_context = if let Some(ref cbu) = session.context.active_cbu {
            self.derive_semantic_context(cbu.id).await
        } else {
            None
        };

        // Derive KYC case context if a case is active in the session
        let kyc_case_context = if let Some(case_id) = session.context.primary_keys.kyc_case_id {
            self.derive_kyc_case_context(case_id).await
        } else {
            None
        };

        // Derive taxonomy navigation context if active
        let taxonomy_context = self.derive_taxonomy_context(session);

        let bindings_context = if !session_bindings.is_empty()
            || active_cbu_context.is_some()
            || scope_context.is_some()
            || semantic_context.is_some()
            || kyc_case_context.is_some()
            || taxonomy_context.is_some()
        {
            let mut parts = Vec::new();
            if let Some(cbu) = active_cbu_context {
                parts.push(cbu);
            }
            // Add multi-CBU scope context for bulk operations
            if let Some(scope) = scope_context {
                parts.push(scope);
            }
            if !session_bindings.is_empty() {
                parts.push(format!(
                    "Available references: {}",
                    session_bindings.join(", ")
                ));
            }

            // Add semantic stage context if available
            let semantic_section = if let Some(ref sem_ctx) = semantic_context {
                format!("\n\n{}", sem_ctx)
            } else {
                String::new()
            };

            // Add KYC case context if available (domain-coherent view with embedded requests)
            let kyc_section = if let Some(ref kyc_ctx) = kyc_case_context {
                format!("\n\n{}", kyc_ctx)
            } else {
                String::new()
            };

            // Add taxonomy navigation context if available
            let taxonomy_section = if let Some(ref tax_ctx) = taxonomy_context {
                format!("\n\n{}", tax_ctx)
            } else {
                String::new()
            };

            format!(
                "\n\n[SESSION CONTEXT: {}. Use the active CBU for operations that need a CBU. Use exact @names in the refs field when referring to entities.]{}{}{}",
                parts.join(". "),
                semantic_section,
                kyc_section,
                taxonomy_section
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
        let mut final_user_dsl: Option<String> = None;
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
                    unresolved_refs: None,
                    current_ref_index: None,
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

            // Unified resolution: entities + codes in one pass
            let resolution = self.resolve_all(intents).await;

            match resolution {
                UnifiedResolution::NeedsDisambiguation {
                    items,
                    partial_intents,
                } => {
                    // Need disambiguation - store intents in session for retrieval after user selection
                    session.pending_intents = partial_intents.clone();

                    let disambig = DisambiguationRequest {
                        request_id: Uuid::new_v4(),
                        items,
                        prompt: "Please select the correct entities:".to_string(),
                        original_intents: Some(partial_intents),
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
                        unresolved_refs: None,
                        current_ref_index: None,
                    });
                }
                UnifiedResolution::Error(msg) => {
                    // Could be entity lookup failure OR invalid code
                    return Err(msg);
                }
                UnifiedResolution::Resolved {
                    intents: modified_intents,
                    corrections,
                } => {
                    // Log any code corrections
                    for (param, from, to) in &corrections {
                        tracing::info!("Resolved {}: '{}' → '{}'", param, from, to);
                    }

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

                    // Build DSL from intents - both execution (UUIDs) and user (display names)
                    let dsl = build_dsl_program(&modified_intents);
                    let user_dsl = build_user_dsl_program(&modified_intents);
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
                        final_user_dsl = Some(user_dsl);
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
                        final_user_dsl = Some(user_dsl);
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
            final_user_dsl,
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

    /// Handle ESPER navigation commands (Blade Runner-style voice/chat navigation).
    ///
    /// Flow:
    /// ```text
    /// User phrase → Trie lookup → EsperMatch → AgentChatResponse
    ///                    ↓
    ///              Miss? → Semantic fallback (if ready)
    ///                    ↓
    ///              Miss? → Return None → Falls through to DSL pipeline
    /// ```
    fn handle_esper_command(&self, message: &str) -> Option<AgentChatResponse> {
        let registry = self.esper_registry.as_ref()?;

        // Try fast path first (trie lookup)
        // Only compute embedding on trie miss + semantic index ready
        let query_embedding = if registry.lookup(message).is_none() && registry.semantic_ready() {
            // Slow path: compute embedding for semantic search (~5-15ms)
            self.embedder.as_ref().and_then(|e| {
                e.embed_blocking(message)
                    .map_err(|err| {
                        tracing::warn!("ESPER semantic embed failed: {}", err);
                        err
                    })
                    .ok()
            })
        } else {
            None
        };

        // Lookup with semantic fallback
        match registry.lookup_with_semantic(message, query_embedding.as_deref()) {
            LookupResult::Matched(esper_match) => {
                // Fast path hit - trie match
                Some(AgentChatResponse {
                    message: esper_match.response.clone(),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: SessionState::New,
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: Some(vec![esper_match.command.clone()]),
                    unresolved_refs: None,
                    current_ref_index: None,
                })
            }
            LookupResult::SemanticMatch {
                esper_match,
                semantic,
                should_learn,
            } => {
                // Semantic match with high confidence - auto-execute
                tracing::info!(
                    "ESPER semantic match: '{}' → {} (confidence: {:.2}, learn: {})",
                    message,
                    semantic.command_key,
                    semantic.confidence,
                    should_learn
                );

                // Self-healing: persist learned alias to DB (fire-and-forget)
                if should_learn {
                    if let Some(pool) = &self.pool {
                        let pool = pool.clone();
                        let phrase = message.to_lowercase();
                        let command_key = semantic.command_key.clone();
                        let confidence = semantic.confidence;

                        tokio::spawn(async move {
                            if let Err(e) = persist_learned_esper_alias(
                                &pool,
                                &phrase,
                                &command_key,
                                confidence,
                            )
                            .await
                            {
                                tracing::warn!("Failed to persist ESPER learned alias: {}", e);
                            }
                        });
                    }
                }

                Some(AgentChatResponse {
                    message: esper_match.response.clone(),
                    intents: vec![],
                    validation_results: vec![],
                    session_state: SessionState::New,
                    can_execute: false,
                    dsl_source: None,
                    ast: None,
                    disambiguation: None,
                    commands: Some(vec![esper_match.command.clone()]),
                    unresolved_refs: None,
                    current_ref_index: None,
                })
            }
            LookupResult::NeedsDisambiguation {
                candidates,
                original_phrase,
            } => {
                // Medium confidence - show disambiguation UI
                tracing::info!(
                    "ESPER needs disambiguation for '{}': {:?}",
                    original_phrase,
                    candidates
                        .iter()
                        .map(|c| format!("{} ({:.2})", c.command_key, c.confidence))
                        .collect::<Vec<_>>()
                );

                let options: Vec<String> = candidates
                    .iter()
                    .map(|c| format!("{} (matched: '{}')", c.command_key, c.matched_alias))
                    .collect();

                Some(AgentChatResponse {
                    message: format!(
                        "I'm not sure what you mean by '{}'. Did you mean one of these?\n{}",
                        original_phrase,
                        options.join("\n")
                    ),
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
                })
            }
            LookupResult::NoMatch => None,
        }
    }

    // =============================================================================
    // DEAD CODE REMOVED - One Path, Same Path
    // =============================================================================
    // The following special-case handlers were removed because ALL user input
    // now goes through LLM intent extraction:
    //
    // - handle_filter_command: "highlight shells", "clear filter" etc.
    // - parse_view_mode: view mode parsing
    // - parse_entity_types: entity type parsing for filters
    // - handle_show_command: "show cbu Allianz" etc.
    // - handle_dsl_command: "run", "undo", "clear" etc.
    // - try_direct_dsl: direct DSL input parsing
    //
    // The LLM handles all these cases. Whether the user types:
    // - "add custody to Allianz" (natural language)
    // - "(product add custody)" (malformed DSL)
    // - "(cbu.add-product :product CUSTODY)" (valid DSL)
    //
    // The result is always: valid DSL ready for execution.
    // One path. Same path. Quality design.
    // =============================================================================

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
                                    search_text: lookup.search_text,
                                    matches,
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

        // Set can_execute flag but do NOT auto-execute
        // User must explicitly type "run"/"execute" or click Execute button
        // Can't execute if there are unresolved refs
        let can_execute = session.can_execute() && all_valid && unresolved_refs.is_none();
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
            unresolved_refs,
            current_ref_index,
        })
    }

    /// Build vocabulary prompt from verb registry
    ///
    /// If `verb_filter` is provided, only include those specific verbs.
    /// Otherwise, if `domain_filter` is provided, filter by domain.
    fn build_vocab_prompt(
        &self,
        domain_filter: Option<&str>,
        verb_filter: Option<&[String]>,
    ) -> String {
        let reg = registry();
        let mut lines = Vec::new();

        for verb in reg.all_verbs() {
            // First check verb filter (most specific)
            if let Some(verbs) = verb_filter {
                let full_name = format!("{}.{}", verb.domain, verb.verb);
                if !verbs.iter().any(|v| v == &full_name) {
                    continue;
                }
            } else if let Some(domain) = domain_filter {
                // Fall back to domain filter
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
    ///
    /// Uses a 10-layer architecture for maintainability (see prompts/INTEGRATION.md):
    /// 1. Role definition and constraints
    /// 2. Structure rules (output format)
    /// 3. Verb vocabulary (from registry)
    /// 4. DAG dependencies (@result_N semantics)
    /// 5. Domain knowledge (code mappings)
    /// 6. Entity context (pre-resolved - injected separately)
    /// 7. Session state (injected separately)
    /// 8. Ambiguity detection rules
    /// 9. Few-shot examples
    /// 10. Error context (if retrying - injected separately)
    fn build_intent_extraction_prompt(&self, vocab: &str) -> String {
        // Layer 1: Role and constraints
        let role_prompt = r#"You are a KYC/AML onboarding DSL assistant. Convert natural language to structured DSL intents.

IMPORTANT: You MUST use the generate_dsl_intents tool to return your response. Do NOT return plain text.

## Your Role
- Translate natural language requests into structured DSL operations
- Identify entity references that need database resolution
- Ask for clarification when requests are ambiguous (set needs_clarification=true)
- Rate your confidence in each interpretation (0.0-1.0)
- Never execute anything - only generate structured intents

## Constraints
- Only use verbs from the AVAILABLE VERBS list below
- Never invent verbs, parameters, or entity types
- Express uncertainty in confidence scores
- If unsure, ask rather than guess"#;

        // Layer 2: Intent structure rules
        let structure_rules = r#"
## Intent Structure

Each intent represents a single DSL verb call with:
- verb: The verb name (e.g., "cbu.ensure", "entity.create-proper-person")
- params: Literal parameter values (e.g., {"name": "Acme Corp", "jurisdiction": "LU"})
- refs: References to previous results or session bindings (e.g., {"cbu-id": "@cbu"} or {"cbu-id": "@result_1"})
- lookups: Entity references needing database resolution

## Rules

1. Use exact verb names from the vocabulary
2. Use exact parameter names (with hyphens, e.g., "client-type" not "clientType")
3. If @cbu is available in session context, use it for cbu-id parameters
4. For sequences of new entities, use @result_N references where N is the sequence number
5. Check for ambiguity before generating intents - ask for clarification if needed
6. Recognize REMOVAL intent: words like "remove", "delete", "drop", "unlink", "take off", "unassign" indicate removal operations
   - "remove [product]" / "delete [product]" → cbu.remove-product
   - "remove [entity] as [role]" / "unassign [role]" → cbu.remove-role
   - "delete [entity]" → entity.delete
   - "end ownership" → ubo.end-ownership

## Entity Lookups

When the user mentions existing entities by name:
- Use the "lookups" field to request entity resolution
- Provide search_text (the name) and entity_type (person, company, cbu, entity, product, role)
- If jurisdiction is mentioned, include jurisdiction_hint

## Confidence Scoring

Rate your confidence in each interpretation:
- 0.95-1.0: Unambiguous request with all required info
- 0.85-0.94: Clear intent but requires entity lookup
- 0.70-0.84: Some inference required, minor assumptions
- 0.50-0.69: Significant assumptions, consider asking
- 0.30-0.49: Multiple interpretations, ASK for clarification
- 0.0-0.29: Very unclear, MUST ask for clarification"#;

        // Layer 4: DAG dependencies - teaches @result_N semantics
        let dag_dependencies = include_str!("prompts/dag_dependencies.md");

        // Layer 5: Domain knowledge (code mappings)
        let domain_knowledge = include_str!("prompts/domain_knowledge.md");

        // Layer 5b: KYC async patterns (fire-and-forget, domain coherence)
        let kyc_async_patterns = include_str!("prompts/kyc_async_patterns.md");

        // Layer 8: Ambiguity detection rules
        let ambiguity_rules = include_str!("prompts/ambiguity_detection.md");

        // Layer 9: Few-shot examples
        let few_shot_examples = include_str!("prompts/few_shot_examples.md");

        format!(
            r#"{role_prompt}

## Available DSL Verbs

{vocab}

{structure_rules}

{dag_dependencies}

{domain_knowledge}

{kyc_async_patterns}

{ambiguity_rules}

{few_shot_examples}"#
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
                    },
                    "confidence": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "description": "Confidence in interpretation: 0.95-1.0=certain, 0.85-0.94=high, 0.70-0.84=good, 0.50-0.69=medium (consider asking), <0.50=ask for clarification"
                    }
                },
                "required": ["intents", "explanation", "confidence"]
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
// ESPER Learning Persistence
// ============================================================================

/// Persist a learned ESPER alias to the database (fire-and-forget)
///
/// Uses upsert to increment occurrence_count if alias already exists.
/// Auto-approves aliases with confidence >= 0.80.
async fn persist_learned_esper_alias(
    pool: &PgPool,
    phrase: &str,
    command_key: &str,
    confidence: f32,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO agent.esper_aliases (phrase, command_key, occurrence_count, confidence, auto_approved, source)
        VALUES ($1, $2, 1, $3, $4, 'semantic_match')
        ON CONFLICT (phrase, command_key) DO UPDATE SET
            occurrence_count = agent.esper_aliases.occurrence_count + 1,
            confidence = GREATEST(agent.esper_aliases.confidence, $3),
            auto_approved = agent.esper_aliases.auto_approved OR $4,
            updated_at = NOW()
        "#,
        phrase,
        command_key,
        confidence as f64,
        confidence >= 0.80
    )
    .execute(pool)
    .await?;

    tracing::debug!(
        "Persisted ESPER learned alias: '{}' → {} (confidence: {:.2})",
        phrase,
        command_key,
        confidence
    );

    Ok(())
}

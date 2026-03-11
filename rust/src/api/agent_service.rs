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
//! │  - Returns structured intent via PipelineResult             │
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
//! │  - Intent + resolved UUIDs → DSL source                     │
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
use crate::api::client_group_adapter::ClientGroupEmbedderAdapter;

use crate::api::session::DisambiguationRequest;
use crate::dsl_v2::ast::AstNode;
use crate::dsl_v2::gateway_resolver::{gateway_addr, GatewayRefResolver};
use crate::dsl_v2::macros::{load_macro_registry_from_dir, MacroRegistry};
use crate::dsl_v2::ref_resolver::ResolveResult;
use crate::dsl_v2::validation::RefType;
use crate::dsl_v2::{enrich_program, parse_program, runtime_registry, Statement};
#[cfg(not(feature = "runbook-gate-vnext"))]
use crate::graph::GraphScope;
use crate::mcp::macro_index::MacroIndex;
use crate::mcp::noun_index::NounIndex;
use crate::mcp::scenario_index::ScenarioIndex;
use crate::mcp::verb_search_factory::VerbSearcherFactory;
use crate::sage::SageEngine;
use crate::semtaxonomy::{
    extract_entity_candidates, hydrate_sage_session, DomainStateSummary, EntityCandidate,
    EntityRef as SemtaxEntityRef, IntentHint, SageSession as SemtaxSession, VerbSurfaceEntry,
};
use crate::semtaxonomy_v2::{
    build_generic_state, introduces_entity_reference, step1_entity_scope, step2_entity_state,
    step3_select_verb, EntityScope, EntityScopeOutcome, EntitySource,
};
#[cfg(not(feature = "runbook-gate-vnext"))]
use crate::session::SessionScope;
use crate::session::{SessionEvent, SessionState, UnifiedSession, UnresolvedRefInfo};
use crate::domain_ops::CustomOperationRegistry;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
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

#[cfg(test)]
mod tests {
    use super::AgentService;
    use crate::session::UnifiedSession;
    use crate::semtaxonomy::EntityCandidate;
    use crate::semtaxonomy_v2::{step1_entity_scope, EntityScopeOutcome};
    use uuid::Uuid;

    #[test]
    fn read_only_pivot_detection_catches_show_queries() {
        assert!(AgentService::is_read_only_pivot_request(
            "show me the cbus instead"
        ));
        assert!(AgentService::is_read_only_pivot_request(
            "what deals does Allianz have?"
        ));
    }

    #[test]
    fn read_only_pivot_detection_excludes_write_queries() {
        assert!(!AgentService::is_read_only_pivot_request(
            "create a new cbu for Allianz"
        ));
        assert!(!AgentService::is_read_only_pivot_request("update the cbu"));
    }

    #[test]
    fn reclassify_before_pending_for_fresh_utterances() {
        assert!(AgentService::should_reclassify_before_pending(
            "what deals does Allianz have?"
        ));
        assert!(AgentService::should_reclassify_before_pending(
            "create a new cbu"
        ));
        assert!(!AgentService::should_reclassify_before_pending("2"));
        assert!(!AgentService::should_reclassify_before_pending("NEW"));
        assert!(!AgentService::should_reclassify_before_pending(
            "data management"
        ));
    }

    #[test]
    fn confirmation_without_pending_mutation_is_safe_noop() {
        assert!(crate::agent::orchestrator::is_confirmation("yes"));
        assert!(!AgentService::should_reclassify_before_pending("yes"));
    }

    #[test]
    fn relationship_relevant_detects_ownership_queries() {
        assert!(AgentService::relationship_relevant("who owns this?"));
        assert!(AgentService::relationship_relevant(
            "show me the relationship between Allianz and Deutsche Bank"
        ));
        assert!(!AgentService::relationship_relevant("show me the cbus"));
    }

    #[test]
    fn signal_driven_domains_from_active_deal() {
        let state = serde_json::json!({
            "signals": {
                "has_active_deal": true,
                "has_active_onboarding": false,
                "has_active_kyc": false,
                "has_incomplete_ubo": false,
                "has_pending_documentation": false
            }
        });
        let domains = AgentService::signal_driven_domains(Some(&state));
        assert!(domains.contains(&"deal".to_string()));
    }

    #[test]
    fn signal_driven_domains_from_incomplete_ubo() {
        let state = serde_json::json!({
            "signals": {
                "has_active_deal": false,
                "has_active_onboarding": false,
                "has_active_kyc": false,
                "has_incomplete_ubo": true,
                "has_pending_documentation": false
            }
        });
        let domains = AgentService::signal_driven_domains(Some(&state));
        assert!(domains.contains(&"ubo".to_string()));
        assert!(domains.contains(&"ownership".to_string()));
    }

    #[test]
    fn signal_driven_domains_empty_for_stale_entity() {
        let state = serde_json::json!({
            "signals": {
                "has_active_deal": false,
                "has_active_onboarding": false,
                "has_active_kyc": false,
                "has_incomplete_ubo": false,
                "has_pending_documentation": false,
                "stale": true
            }
        });
        let domains = AgentService::signal_driven_domains(Some(&state));
        assert!(domains.is_empty());
    }

    #[test]
    fn lifecycle_populated_entity_preferred_over_hollow() {
        let hollow = EntityCandidate {
            entity_id: Uuid::now_v7(),
            entity_type: "entity".to_string(),
            name: "Allianz Holdings".to_string(),
            match_score: 0.91,
            match_field: Some("name".to_string()),
            summary: None,
            source_kind: Some("db_search".to_string()),
            linked_cbu_ids: Vec::new(),
            is_onboarding_member: false,
            candidate_for_cbu: true,
            lifecycle_populated: false,
            linked_entity_count: 0,
            has_active_workflow: false,
        };
        let rich = EntityCandidate {
            entity_id: Uuid::now_v7(),
            entity_type: "cbu".to_string(),
            name: "Allianz Holdings".to_string(),
            match_score: 0.89,
            match_field: Some("name".to_string()),
            summary: None,
            source_kind: Some("db_search".to_string()),
            linked_cbu_ids: vec![Uuid::now_v7()],
            is_onboarding_member: true,
            candidate_for_cbu: false,
            lifecycle_populated: true,
            linked_entity_count: 3,
            has_active_workflow: true,
        };

        let outcome = step1_entity_scope("show me Allianz", None, &[hollow, rich.clone()]);
        match outcome {
            EntityScopeOutcome::Resolved(selected) => assert_eq!(selected.entity_id, rich.entity_id),
            other => panic!("expected resolved outcome, got {other:?}"),
        }
    }

    #[test]
    fn infer_discovery_domains_includes_entity_for_company() {
        let session = UnifiedSession::new();
        let domains = AgentService::infer_discovery_domains(&session, "add a new company");
        assert!(domains.contains(&"entity".to_string()));
    }

    #[test]
    fn infer_discovery_domains_includes_entity_for_person() {
        let session = UnifiedSession::new();
        let domains =
            AgentService::infer_discovery_domains(&session, "register John Smith as a person");
        assert!(domains.contains(&"entity".to_string()));
    }

    #[test]
    fn infer_discovery_domains_includes_entity_for_trust() {
        let session = UnifiedSession::new();
        let domains = AgentService::infer_discovery_domains(
            &session,
            "add a trust -- Cayman Islands discretionary trust",
        );
        assert!(domains.contains(&"entity".to_string()));
    }
}

/// Parameters that should be resolved as codes (not raw strings) via EntityGateway.
/// These are reference data lookups where user input needs fuzzy matching to canonical codes.
/// UUID-based entity lookups (CBU, Entity, Document) are handled separately.
// ChatRequest is now the SINGLE source of truth - imported from ob-poc-types
pub use ob_poc_types::ChatRequest;

/// Extended chat response that includes disambiguation status
#[derive(Debug, Serialize)]
pub struct AgentChatResponse {
    /// Agent's response message
    pub message: String,
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
    /// Unified decision packet (NEW - wraps all clarification types)
    /// When present, UI should render a decision card with choices
    /// User selection triggers POST /api/session/:id/decision/reply
    /// This will eventually replace verb_disambiguation and intent_tier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<ob_poc_types::DecisionPacket>,
    /// Typed Sage explanation payload for UI rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sage_explain: Option<ob_poc_types::chat::SageExplainPayload>,
    /// Typed Coder/REPL proposal payload for UI rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coder_proposal: Option<ob_poc_types::chat::CoderProposalPayload>,
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
/// let response = service.process_chat(&mut session, &request, actor).await?;
/// ```
pub struct AgentService {
    pool: PgPool,
    config: AgentServiceConfig,
    /// Embedder for semantic verb search - REQUIRED, no fallback path
    embedder: Arc<CandleEmbedder>,
    /// Learned data for exact phrase matching (invocation_phrases, entity_aliases)
    /// Loaded at startup via warmup - enables step 2 (global learned exact match)
    learned_data: Option<crate::agent::learning::warmup::SharedLearnedData>,
    /// Lexicon service for fast in-memory lexical verb search
    /// Runs BEFORE semantic search for exact/token matches (Phase A of 072)
    lexicon: Option<crate::mcp::verb_search::SharedLexicon>,
    /// Entity linking service for in-memory entity mention extraction and resolution
    /// Used by LookupService for verb-first entity resolution
    entity_linker: Option<Arc<dyn crate::entity_linking::EntityLinkingService>>,
    /// Server-side policy enforcement for single-pipeline invariants
    policy_gate: Arc<crate::policy::PolicyGate>,
    /// Semantic OS client — when set, routes sem_reg calls through DI boundary
    sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
    /// NounIndex for deterministic Tier -1 ECIR noun→verb resolution
    noun_index: Option<Arc<NounIndex>>,
    /// MacroIndex for deterministic Tier -2B macro search parity
    macro_index: Option<Arc<MacroIndex>>,
    /// ScenarioIndex for journey-level Tier -2A compound intent resolution
    scenario_index: Option<Arc<ScenarioIndex>>,
    /// Cached MacroRegistry to avoid reloading from disk on every verb search
    macro_registry: Option<Arc<MacroRegistry>>,
    /// Optional Sage engine for Stage 1.5 shadow classification.
    sage_engine: Option<Arc<dyn SageEngine>>,
}

impl AgentService {
    fn push_discovery_domain(domains: &mut Vec<String>, domain: &str) {
        if !domains.iter().any(|existing| existing == domain) {
            domains.push(domain.to_string());
        }
    }

    fn semtaxonomy_enabled() -> bool {
        !matches!(
            std::env::var("SEMTAXONOMY_ENABLED").ok().as_deref(),
            Some("0" | "false" | "FALSE" | "no" | "NO")
        )
    }

    fn classify_action(input: &str) -> Option<&'static str> {
        let lower = input.to_ascii_lowercase();

        if ["delete", "remove", "drop", "destroy", "kill", "purge"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            return Some("delete");
        }
        if [
            "create",
            "add",
            "new",
            "register",
            "set up",
            "spin up",
            "open",
            "onboard",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
        {
            return Some("create");
        }
        if [
            "update", "change", "modify", "edit", "rename", "set ", "amend", "correct",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
        {
            return Some("update");
        }
        if ["reject", "deny", "decline", "refuse"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            return Some("reject");
        }
        if ["verify", "approve", "confirm", "accept", "validate"]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            return Some("verify");
        }
        let read_only = [
            "show", "list", "what", "which", "read", "view", "inspect", "describe", "display",
            "tell", "who", "where",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let write_intent = [
            "create", "add", "update", "change", "delete", "remove", "assign", "set", "open",
            "run", "check", "verify", "approve", "reject", "complete", "close", "rename",
            "amend", "register", "onboard", "spin up",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        if read_only && !write_intent {
            return Some("read");
        }

        None
    }

    fn infer_discovery_domains(session: &UnifiedSession, message: &str) -> Vec<String> {
        let normalized = message.to_ascii_lowercase();
        let mut domains = Vec::new();

        if normalized.contains("deal") || normalized.contains("mandate") || normalized.contains("rate card") {
            Self::push_discovery_domain(&mut domains, "deal");
        }
        if normalized.contains("cbu")
            || normalized.contains("client onboarding")
            || normalized.contains("onboard")
            || normalized.contains("client ")
        {
            Self::push_discovery_domain(&mut domains, "cbu");
        }
        if normalized.contains("document") || normalized.contains("doc pack") || normalized.contains("evidence") {
            Self::push_discovery_domain(&mut domains, "document");
        }
        if normalized.contains("ubo")
            || normalized.contains("ownership")
            || normalized.contains("beneficial owner")
            || normalized.contains("who owns")
            || normalized.contains("who controls")
        {
            Self::push_discovery_domain(&mut domains, "ubo");
            Self::push_discovery_domain(&mut domains, "ownership");
        }
        if normalized.contains("relationship") || normalized.contains("graph") {
            Self::push_discovery_domain(&mut domains, "entity");
        }
        if normalized.contains("entity")
            || normalized.contains("legal entity")
            || normalized.contains("company")
            || normalized.contains("person")
            || normalized.contains("individual")
            || normalized.contains("trust")
            || normalized.contains("partnership")
            || normalized.contains("register")
            || normalized.contains("placeholder")
            || (normalized.contains("add")
                && (normalized.contains("company")
                    || normalized.contains("person")
                    || normalized.contains("trust")
                    || normalized.contains("limited")))
        {
            Self::push_discovery_domain(&mut domains, "entity");
        }
        if normalized.contains("screening")
            || normalized.contains("sanctions")
            || normalized.contains("pep")
            || normalized.contains("adverse media")
        {
            Self::push_discovery_domain(&mut domains, "screening");
        }
        if normalized.contains("fund")
            || normalized.contains("subfund")
            || normalized.contains("share class")
            || normalized.contains("umbrella")
        {
            Self::push_discovery_domain(&mut domains, "fund");
            if normalized.contains("structure") || normalized.contains("subfund") || normalized.contains("share class") {
                Self::push_discovery_domain(&mut domains, "struct");
            }
        }
        if normalized.contains("case") {
            Self::push_discovery_domain(&mut domains, "case");
        }
        if normalized.contains("kyc") {
            Self::push_discovery_domain(&mut domains, "kyc");
        }
        if normalized.contains("group") || normalized.contains("party") {
            Self::push_discovery_domain(&mut domains, "client-group");
        }
        if normalized == "undo" || normalized.contains("session") || normalized.contains("resume") {
            Self::push_discovery_domain(&mut domains, "session");
        }

        if domains.is_empty() {
            if let Some(stage_focus) = session.context.stage_focus.as_deref() {
                match stage_focus {
                    "semos-data-management" | "semos-data" => Self::push_discovery_domain(&mut domains, "deal"),
                    "semos-kyc" => Self::push_discovery_domain(&mut domains, "kyc"),
                    "semos-onboarding" => Self::push_discovery_domain(&mut domains, "cbu"),
                    "semos-stewardship" => Self::push_discovery_domain(&mut domains, "registry"),
                    _ => {}
                }
            }
        }

        if let Some(state) = session.semtaxonomy_session.as_ref() {
            if let Some(domain_scope) = state.domain_scope.as_ref() {
                Self::push_discovery_domain(&mut domains, domain_scope);
            }
        }

        domains
    }

    fn parse_entity_candidates(payload: &serde_json::Value) -> Vec<EntityCandidate> {
        payload["results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let entity_id = item["entity_id"]
                    .as_str()
                    .and_then(|raw| Uuid::parse_str(raw).ok())?;
                Some(EntityCandidate {
                    entity_id,
                    entity_type: item["entity_type"].as_str().unwrap_or("entity").to_string(),
                    name: item["name"].as_str().unwrap_or("unknown").to_string(),
                    match_score: item["match_score"].as_f64().unwrap_or_default(),
                    match_field: item["match_field"].as_str().map(str::to_string),
                    summary: item.get("summary").cloned(),
                    source_kind: item["source_kind"].as_str().map(str::to_string),
                    linked_cbu_ids: item["linked_cbu_ids"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|value| value.as_str().and_then(|raw| Uuid::parse_str(raw).ok()))
                        .collect(),
                    is_onboarding_member: item["is_onboarding_member"].as_bool().unwrap_or(false),
                    candidate_for_cbu: item["candidate_for_cbu"].as_bool().unwrap_or(false),
                    lifecycle_populated: item["lifecycle_populated"].as_bool().unwrap_or(false),
                    linked_entity_count: item["linked_entity_count"]
                        .as_u64()
                        .map(|value| value as usize)
                        .unwrap_or(0),
                    has_active_workflow: item["has_active_workflow"].as_bool().unwrap_or(false),
                })
            })
            .collect()
    }

    fn is_vague_status_utterance(request: &str) -> bool {
        let normalized = request.trim().to_ascii_lowercase();
        [
            "what is the status",
            "what's the status",
            "status",
            "where are we",
            "where do we stand",
            "what now",
            "what next",
            "next step",
            "next steps",
            "move forward",
        ]
        .iter()
        .any(|needle| normalized == *needle || normalized.contains(needle))
    }

    fn signal_driven_domains(entity_state: Option<&serde_json::Value>) -> Vec<String> {
        let mut domains = Vec::new();
        let Some(signals) = entity_state
            .and_then(|state| state.get("signals"))
            .and_then(serde_json::Value::as_object)
        else {
            return domains;
        };

        if signals
            .get("has_active_deal")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Self::push_discovery_domain(&mut domains, "deal");
        }
        if signals
            .get("has_active_onboarding")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Self::push_discovery_domain(&mut domains, "cbu");
        }
        if signals
            .get("has_active_kyc")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Self::push_discovery_domain(&mut domains, "kyc");
            Self::push_discovery_domain(&mut domains, "case");
        }
        if signals
            .get("has_pending_documentation")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Self::push_discovery_domain(&mut domains, "document");
        }
        if signals
            .get("has_incomplete_ubo")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Self::push_discovery_domain(&mut domains, "ubo");
            Self::push_discovery_domain(&mut domains, "ownership");
        }
        domains
    }

    fn build_domain_state_summaries(
        entity_state: Option<&serde_json::Value>,
        domains: &[String],
    ) -> Vec<DomainStateSummary> {
        let activities = entity_state
            .and_then(|state| state.get("activities"))
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let signals = entity_state
            .and_then(|state| state.get("signals"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        domains
            .iter()
            .map(|domain| {
                let mut active_count = 0usize;
                let mut blocked_count = 0usize;
                let mut notable_gaps = Vec::new();
                for activity in &activities {
                    let activity_domain = activity["domain"].as_str().unwrap_or_default();
                    if activity_domain.eq_ignore_ascii_case(domain) {
                        active_count += 1;
                        if activity["status"].as_str() == Some("blocked") {
                            blocked_count += 1;
                        }
                    }
                }
                match domain.as_str() {
                    "deal" if signals["has_active_deal"].as_bool().unwrap_or(false) => active_count += 1,
                    "kyc" if signals["has_active_kyc"].as_bool().unwrap_or(false) => active_count += 1,
                    "document" if signals["has_pending_documentation"].as_bool().unwrap_or(false) => {
                        notable_gaps.push("pending_documentation".to_string())
                    }
                    "ubo" | "ownership" if signals["has_incomplete_ubo"].as_bool().unwrap_or(false) => {
                        notable_gaps.push("incomplete_ubo".to_string())
                    }
                    _ => {}
                }
                let next_action_candidates = match domain.as_str() {
                        "document" if blocked_count > 0 || active_count > 0 => {
                            vec!["document.list-requests-by-workstream".to_string()]
                        }
                        "screening" if active_count > 0 || blocked_count > 0 => {
                            vec!["screening.sanctions".to_string()]
                        }
                        "kyc" if active_count > 0 => vec!["case.list".to_string()],
                        "ubo" | "ownership" if !notable_gaps.is_empty() => {
                            vec!["ubo.list-owners".to_string(), "ubo.trace-chains".to_string()]
                        }
                        "deal" if active_count > 0 => vec!["deal.list".to_string(), "deal.read-timeline".to_string()],
                        "cbu" if active_count > 0 || domain == "cbu" => {
                            vec!["cbu.list".to_string(), "cbu.create".to_string(), "cbu.parties".to_string()]
                        }
                        _ => Vec::new(),
                    };
                DomainStateSummary {
                    domain: domain.clone(),
                    active_count,
                    blocked_count,
                    notable_gaps,
                    next_action_candidates,
                }
            })
            .collect()
    }

    fn merge_entity_candidates(mut candidates: Vec<EntityCandidate>) -> Vec<EntityCandidate> {
        candidates.sort_by(|left, right| {
            right
                .has_active_workflow
                .cmp(&left.has_active_workflow)
                .then(right.lifecycle_populated.cmp(&left.lifecycle_populated))
                .then(right.linked_entity_count.cmp(&left.linked_entity_count))
                .then(
                    right
                        .match_score
                        .partial_cmp(&left.match_score)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(left.name.cmp(&right.name))
        });
        candidates.dedup_by(|left, right| left.entity_id == right.entity_id);
        candidates
    }

    fn make_synthetic_arg(
        key: &str,
        value: serde_json::Value,
    ) -> Result<crate::dsl_v2::Argument, String> {
        use crate::dsl_v2::{AstNode, Literal, Span};

        fn node_from_json(value: serde_json::Value) -> Result<AstNode, String> {
            Ok(match value {
                serde_json::Value::String(s) => {
                    if let Ok(uuid) = uuid::Uuid::parse_str(&s) {
                        AstNode::Literal(Literal::Uuid(uuid), Span::synthetic())
                    } else {
                        AstNode::Literal(Literal::String(s), Span::synthetic())
                    }
                }
                serde_json::Value::Bool(b) => {
                    AstNode::Literal(Literal::Boolean(b), Span::synthetic())
                }
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        AstNode::Literal(Literal::Integer(i), Span::synthetic())
                    } else if let Some(f) = n.as_f64() {
                        let dec = rust_decimal::Decimal::from_f64_retain(f)
                            .ok_or_else(|| "Invalid decimal literal".to_string())?;
                        AstNode::Literal(Literal::Decimal(dec), Span::synthetic())
                    } else {
                        return Err("Unsupported numeric argument".to_string());
                    }
                }
                serde_json::Value::Array(items) => AstNode::List {
                    items: items
                        .into_iter()
                        .map(node_from_json)
                        .collect::<Result<Vec<_>, _>>()?,
                    span: Span::synthetic(),
                },
                serde_json::Value::Null => {
                    AstNode::Literal(Literal::String(String::new()), Span::synthetic())
                }
                serde_json::Value::Object(_) => {
                    return Err(
                        "Object args are not yet supported in SemTaxonomy synthetic calls"
                            .to_string(),
                    );
                }
            })
        }

        Ok(crate::dsl_v2::Argument {
            key: key.to_string(),
            value: node_from_json(value)?,
            span: crate::dsl_v2::Span::synthetic(),
        })
    }

    async fn run_discovery_op(
        &self,
        domain: &str,
        verb: &str,
        args: Vec<(&str, serde_json::Value)>,
    ) -> Result<serde_json::Value, String> {
        let registry = CustomOperationRegistry::new();
        let op = registry
            .get(domain, verb)
            .ok_or_else(|| format!("SemTaxonomy op not registered: {}.{}", domain, verb))?;
        let arguments = args
            .into_iter()
            .map(|(k, v)| Self::make_synthetic_arg(k, v))
            .collect::<Result<Vec<_>, _>>()?;
        let call = crate::dsl_v2::VerbCall {
            domain: domain.to_string(),
            verb: verb.to_string(),
            arguments,
            binding: None,
            span: crate::dsl_v2::Span::synthetic(),
        };
        let mut exec_ctx = crate::dsl_v2::ExecutionContext::new();
        let result = op
            .execute(&call, &mut exec_ctx, &self.pool)
            .await
            .map_err(|e| e.to_string())?;

        match result {
            crate::dsl_v2::ExecutionResult::Record(record) => Ok(record),
            crate::dsl_v2::ExecutionResult::RecordSet(records) => {
                Ok(serde_json::json!({ "results": records }))
            }
            other => Ok(serde_json::json!({ "result_kind": format!("{:?}", other) })),
        }
    }

    async fn try_semtaxonomy_path(
        &self,
        session: &mut UnifiedSession,
        request: &ChatRequest,
        dominant_entity_id: Option<Uuid>,
        resolved_kinds: &[String],
    ) -> Result<Option<AgentChatResponse>, String> {
        tracing::info!(
            semtaxonomy_enabled = Self::semtaxonomy_enabled(),
            session_id = %session.id,
            message = %request.message,
            "try_semtaxonomy_path invoked"
        );
        if !Self::semtaxonomy_enabled() {
            return Ok(None);
        }

        let extracted_names = extract_entity_candidates(&request.message);
        let mut state = session
            .semtaxonomy_session
            .clone()
            .unwrap_or_else(|| SemtaxSession {
                session_id: session.id,
                started_at: chrono::Utc::now(),
                ..Default::default()
            });
        state.utterance_history.push(request.message.clone());
        if state.utterance_history.len() > 20 {
            let keep_from = state.utterance_history.len() - 20;
            state.utterance_history.drain(0..keep_from);
        }

        let inferred_domains = Self::infer_discovery_domains(session, &request.message);
        let domain_scope = inferred_domains.first().cloned();
        let active_entity_id = if extracted_names.is_empty() {
            dominant_entity_id
                .or(session.context.dominant_entity_id)
                .or_else(|| state.active_entity.as_ref().map(|entity| entity.entity_id))
        } else {
            None
        };

        let previous_active = state.active_entity.clone();
        let (cascade_result, entity_candidates, active_entity, entity_state, intent_hints, grounding_strategy, grounding_confidence) =
            if let Some(entity_id) = active_entity_id {
                let context = self
                    .run_discovery_op(
                        "discovery",
                        "entity-context",
                        vec![("entity-id", serde_json::json!(entity_id))],
                    )
                    .await?;
                let active_entity = Some(SemtaxEntityRef {
                    entity_id,
                    entity_type: context["entity_type"]
                        .as_str()
                        .unwrap_or("entity")
                        .to_string(),
                    name: context["name"].as_str().unwrap_or("unknown").to_string(),
                });
                let hints = vec![IntentHint {
                    intent: "grounded-entity-context".to_string(),
                    confidence: "high".to_string(),
                    reason: "Existing entity context was available for this turn".to_string(),
                }];
                let entity_candidates = active_entity
                    .as_ref()
                    .map(|entity| {
                        vec![EntityCandidate {
                            entity_id: entity.entity_id,
                            entity_type: entity.entity_type.clone(),
                            name: entity.name.clone(),
                            match_score: 1.0,
                            match_field: Some("session_scope".to_string()),
                            summary: None,
                            source_kind: Some("session_scope".to_string()),
                            linked_cbu_ids: Vec::new(),
                            is_onboarding_member: entity.entity_type.eq_ignore_ascii_case("cbu"),
                            candidate_for_cbu: !entity.entity_type.eq_ignore_ascii_case("cbu"),
                            lifecycle_populated: true,
                            linked_entity_count: 0,
                            has_active_workflow: true,
                        }]
                    })
                    .unwrap_or_default();
                (
                    None,
                    entity_candidates,
                    active_entity,
                    Some(context),
                    hints,
                    Some("existing_scope".to_string()),
                    Some("high".to_string()),
                )
            } else {
                let search_queries = if extracted_names.is_empty() {
                    vec![request.message.clone()]
                } else {
                    extracted_names.clone()
                };
                let mut merged_candidates = Vec::new();
                let mut first_search_payload = None;
                for query in &search_queries {
                    let search = self
                        .run_discovery_op(
                            "discovery",
                            "search-entities",
                            vec![
                                ("query", serde_json::json!(query)),
                                ("entity-types", serde_json::json!(resolved_kinds)),
                            ],
                        )
                        .await?;
                    if first_search_payload.is_none() {
                        first_search_payload = Some(search.clone());
                    }
                    merged_candidates.extend(Self::parse_entity_candidates(&search));
                }
                if merged_candidates.is_empty() && !extracted_names.is_empty() {
                    let fallback = self
                        .run_discovery_op(
                            "discovery",
                            "search-entities",
                            vec![
                                ("query", serde_json::json!(request.message.clone())),
                                ("entity-types", serde_json::json!(resolved_kinds)),
                            ],
                        )
                        .await?;
                    if first_search_payload.is_none() {
                        first_search_payload = Some(fallback.clone());
                    }
                    merged_candidates.extend(Self::parse_entity_candidates(&fallback));
                }
                let search = first_search_payload
                    .unwrap_or_else(|| serde_json::json!({ "results": [] }));
                let mut entity_candidates = Self::merge_entity_candidates(merged_candidates);
                let mut cascade_result = None;
                if entity_candidates.len() > 1 || domain_scope.is_none() {
                    let include_relationships = Self::relationship_relevant(&request.message);
                    let cascade_query = extracted_names
                        .first()
                        .cloned()
                        .unwrap_or_else(|| request.message.clone());
                    if let Ok(cascade) = self
                        .run_discovery_op(
                            "discovery",
                            "cascade-research",
                            vec![
                                ("query", serde_json::json!(cascade_query)),
                                ("top-n", serde_json::json!(3)),
                                (
                                    "include-relationships",
                                    serde_json::json!(include_relationships),
                                ),
                            ],
                        )
                        .await
                    {
                        if let Some(entities) = cascade["entities"].as_array() {
                            entity_candidates = entities
                                .iter()
                                .filter_map(|item| {
                                    let entity_id = item["entity_id"]
                                        .as_str()
                                        .and_then(|raw| Uuid::parse_str(raw).ok())?;
                                    Some(EntityCandidate {
                                        entity_id,
                                        entity_type: item["entity_type"]
                                            .as_str()
                                            .unwrap_or("entity")
                                            .to_string(),
                                        name: item["name"].as_str().unwrap_or("unknown").to_string(),
                                        match_score: item["match_score"].as_f64().unwrap_or_default(),
                                        match_field: item["match_field"].as_str().map(str::to_string),
                                        summary: item.get("signals").cloned(),
                                        source_kind: Some("cascade_research".to_string()),
                                        linked_cbu_ids: item["linked_cbu_ids"]
                                            .as_array()
                                            .cloned()
                                            .unwrap_or_default()
                                            .into_iter()
                                            .filter_map(|value| {
                                                value.as_str().and_then(|raw| Uuid::parse_str(raw).ok())
                                            })
                                            .collect(),
                                        is_onboarding_member: item["is_onboarding_member"]
                                            .as_bool()
                                            .unwrap_or(false),
                                        candidate_for_cbu: item["candidate_for_cbu"]
                                            .as_bool()
                                            .unwrap_or(false),
                                        lifecycle_populated: item["lifecycle_populated"]
                                            .as_bool()
                                            .unwrap_or(false),
                                        linked_entity_count: item["linked_entity_count"]
                                            .as_u64()
                                            .map(|value| value as usize)
                                            .unwrap_or(0),
                                        has_active_workflow: item["has_active_workflow"]
                                            .as_bool()
                                            .unwrap_or(false),
                                    })
                                })
                                .collect();
                        }
                        cascade_result = Some(cascade);
                    }
                }
                entity_candidates = Self::merge_entity_candidates(entity_candidates);
                let previous_scope = previous_active.as_ref().map(|entity| EntityScope {
                    entity_id: entity.entity_id,
                    entity_type: entity.entity_type.clone(),
                    name: entity.name.clone(),
                    confidence: 1.0,
                    source: EntitySource::SessionCarry,
                });
                let fresh_entity_reference = introduces_entity_reference(&request.message);
                let scope_outcome = step1_entity_scope(
                    &request.message,
                    previous_scope.as_ref(),
                    &entity_candidates,
                );
                let active_entity = match &scope_outcome {
                    EntityScopeOutcome::Resolved(scope) => Some(SemtaxEntityRef {
                        entity_id: scope.entity_id,
                        entity_type: scope.entity_type.clone(),
                        name: scope.name.clone(),
                    }),
                    EntityScopeOutcome::Ambiguous(_) | EntityScopeOutcome::Unresolved => None,
                };
                match &scope_outcome {
                    EntityScopeOutcome::Ambiguous(scopes) => {
                        hydrate_sage_session(
                            &mut state,
                            cascade_result.clone().or(Some(search.clone())),
                            None,
                            entity_candidates.clone(),
                            domain_scope.clone(),
                            session.context.stage_focus.clone(),
                            Vec::new(),
                            None,
                            Vec::new(),
                            vec![IntentHint {
                                intent: "entity-scope-ambiguous".to_string(),
                                confidence: "low".to_string(),
                                reason: "Multiple entity candidates remained after search".to_string(),
                            }],
                            Some("scope_ambiguity".to_string()),
                            Some("low".to_string()),
                        );
                        session.semtaxonomy_session = Some(state);
                        let options = scopes
                            .iter()
                            .take(5)
                            .map(|scope| format!("{} ({})", scope.name, scope.entity_type))
                            .collect::<Vec<_>>();
                        let message = format!(
                            "I found multiple possible matches for this entity reference.\n\n{}",
                            options
                                .iter()
                                .enumerate()
                                .map(|(idx, item)| format!("{}. {}", idx + 1, item))
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                        let sage_explain = ob_poc_types::chat::SageExplainPayload {
                            understanding: "I found multiple plausible entity matches and need you to confirm which one you mean before I continue.".to_string(),
                            mode: "scope_clarification".to_string(),
                            scope_summary: None,
                            confidence: "low".to_string(),
                            clarifications: options,
                        };
                        return Ok(Some(Self::scope_feedback_response(
                            session,
                            message,
                            sage_explain,
                        )));
                    }
                    EntityScopeOutcome::Unresolved if fresh_entity_reference => {
                        hydrate_sage_session(
                            &mut state,
                            Some(search.clone()),
                            None,
                            entity_candidates.clone(),
                            domain_scope.clone(),
                            session.context.stage_focus.clone(),
                            Vec::new(),
                            None,
                            Vec::new(),
                            vec![IntentHint {
                                intent: "entity-scope-unresolved".to_string(),
                                confidence: "low".to_string(),
                                reason: "Entity search did not resolve a usable scope".to_string(),
                            }],
                            Some("scope_unresolved".to_string()),
                            Some("low".to_string()),
                        );
                        session.semtaxonomy_session = Some(state);
                        let message = "I could not identify the entity from that utterance. Give me the full name or a more specific reference.".to_string();
                        let sage_explain = ob_poc_types::chat::SageExplainPayload {
                            understanding: "I could not resolve a single entity scope from that utterance, so I am stopping before state and transition selection.".to_string(),
                            mode: "scope_clarification".to_string(),
                            scope_summary: None,
                            confidence: "low".to_string(),
                            clarifications: vec![
                                "Try the full legal or onboarding name".to_string(),
                                "If you mean a different entity, name it explicitly".to_string(),
                            ],
                        };
                        return Ok(Some(Self::scope_feedback_response(
                            session,
                            message,
                            sage_explain,
                        )));
                    }
                    _ => {}
                }
                let entity_state_from_cascade = cascade_result
                    .as_ref()
                    .and_then(|cascade| cascade["entities"].as_array())
                    .and_then(|entities| {
                        active_entity.as_ref().and_then(|entity| {
                            entities.iter().find(|item| {
                                item["entity_id"].as_str()
                                    == Some(entity.entity_id.to_string().as_str())
                            })
                        })
                    })
                    .and_then(|entity| entity.get("context").cloned());
                let entity_state = match (active_entity.as_ref(), entity_state_from_cascade) {
                    (_, Some(context)) => Some(context),
                    (Some(entity), None) => Some(
                        self.run_discovery_op(
                            "discovery",
                            "entity-context",
                            vec![("entity-id", serde_json::json!(entity.entity_id))],
                        )
                        .await?,
                    ),
                    (None, None) => None,
                };
                let hints = vec![IntentHint {
                    intent: "entity-discovery".to_string(),
                    confidence: if active_entity.is_some() {
                        "medium".to_string()
                    } else {
                        "low".to_string()
                    },
                    reason: "Turn grounded through entity search before composition".to_string(),
                }];
                let used_cascade = cascade_result.is_some();
                let grounding_confidence = if entity_candidates.len() <= 1 {
                    "high".to_string()
                } else if active_entity.is_some() {
                    "medium".to_string()
                } else {
                    "low".to_string()
                };
                (
                    cascade_result.or(Some(search)),
                    entity_candidates,
                    active_entity,
                    entity_state,
                    hints,
                    Some(if used_cascade {
                        "cascade_research".to_string()
                    } else {
                        "search_entities".to_string()
                    }),
                    Some(grounding_confidence),
                )
            };

        if let Some(entity) = active_entity.as_ref() {
            if entity.entity_type.eq_ignore_ascii_case("client-group") {
                let scope = crate::mcp::scope_resolution::ScopeContext::new()
                    .with_client_group(entity.entity_id, entity.name.clone());
                session.context.set_client_scope(scope);
                session.context.deal_id = None;
                session.context.deal_gate_skipped = false;
            }
        }

        let mut inferred_domains = inferred_domains;
        let action_class = Self::classify_action(&request.message);
        let signal_domains = Self::signal_driven_domains(entity_state.as_ref());
        for domain in &signal_domains {
            Self::push_discovery_domain(&mut inferred_domains, &domain);
        }
        if matches!(action_class, Some("create")) {
            let lower = request.message.to_ascii_lowercase();
            let entity_creation_nouns = [
                "entity",
                "company",
                "person",
                "individual",
                "trust",
                "partnership",
                "limited",
                "plc",
                "ltd",
                "ag",
                "sa",
                "legal entity",
                "holding",
                "holdco",
            ];
            if entity_creation_nouns
                .iter()
                .any(|noun| lower.contains(noun))
            {
                Self::push_discovery_domain(&mut inferred_domains, "entity");
            }
        }
        if Self::is_vague_status_utterance(&request.message) && !signal_domains.is_empty() {
            let mut reordered = signal_domains;
            for domain in inferred_domains {
                if !reordered.iter().any(|existing| existing == &domain) {
                    reordered.push(domain);
                }
            }
            inferred_domains = reordered;
        }

        let domain_scope = inferred_domains.first().cloned();

        let entity_type_for_actions = active_entity
            .as_ref()
            .map(|entity| entity.entity_type.clone())
            .or_else(|| resolved_kinds.first().cloned())
            .unwrap_or_else(|| "entity".to_string());

        let mut action_surface = if let Some(active_entity) = active_entity.as_ref() {
            let transitions = self
                .run_discovery_op(
                    "discovery",
                    "graph-walk",
                    vec![
                        ("entity-id", serde_json::json!(active_entity.entity_id)),
                        ("include-blocked", serde_json::json!(true)),
                    ],
                )
                .await
                .unwrap_or_else(|_| serde_json::json!({}));
            state
                .research_cache
                .insert("graph-walk".to_string(), transitions.clone());

            transitions["valid_verbs"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|verb| {
                    let verb_id = verb["verb_id"].as_str().unwrap_or_default().to_string();
                    let domain = verb_id
                        .split_once('.')
                        .map(|(domain, _)| domain.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let invocation_phrases = verb["invocation_phrases"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect::<Vec<_>>();
                    let description = if invocation_phrases.is_empty() {
                        verb["description"].as_str().unwrap_or_default().to_string()
                    } else {
                        format!(
                            "{} {}",
                            verb["description"].as_str().unwrap_or_default(),
                            invocation_phrases.join(" ")
                        )
                    };
                    VerbSurfaceEntry {
                        verb_id: verb_id.clone(),
                        domain,
                        name: verb_id
                            .split_once('.')
                            .map(|(_, name)| name.to_string())
                            .unwrap_or_else(|| verb_id.clone()),
                        description,
                        polarity: verb["polarity"].as_str().unwrap_or("read").to_string(),
                        phase_tags: vec![verb["lane"].as_str().unwrap_or("general").to_string()],
                        subject_kinds: vec![entity_type_for_actions.clone()],
                        parameters: verb["parameters"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|parameter| crate::semtaxonomy::VerbParameter {
                                name: parameter["name"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                required: parameter["required"].as_bool().unwrap_or(false),
                                description: parameter["description"]
                                    .as_str()
                                    .map(str::to_string),
                            })
                            .collect(),
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        if action_surface.is_empty() {
            let mut action_domains_and_types: Vec<(String, String)> = inferred_domains
                .iter()
                .cloned()
                .map(|domain| (domain, entity_type_for_actions.clone()))
                .collect();
            if inferred_domains.iter().any(|domain| domain == "entity")
                && !action_domains_and_types
                    .iter()
                    .any(|(domain, entity_type)| domain == "entity" && entity_type == "entity")
            {
                action_domains_and_types.push(("entity".to_string(), "entity".to_string()));
            }

            for (domain, entity_type) in &action_domains_and_types {
                let surface = self
                    .run_discovery_op(
                        "discovery",
                        "available-actions",
                        vec![
                            ("domain", serde_json::json!(domain.clone())),
                            ("entity-type", serde_json::json!(entity_type.clone())),
                            ("polarity", serde_json::json!("all")),
                        ],
                    )
                    .await?;
                let mut entries = surface["groups"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .flat_map(|group| {
                        let phase = group["aspect"].as_str().unwrap_or("general").to_string();
                        let domain_name = domain.clone();
                        let subject_kind = entity_type.clone();
                        group["verbs"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(move |verb| VerbSurfaceEntry {
                                verb_id: verb["verb_id"].as_str().unwrap_or_default().to_string(),
                                domain: domain_name.clone(),
                                name: verb["name"].as_str().unwrap_or_default().to_string(),
                                description: verb["description"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                polarity: verb["polarity"].as_str().unwrap_or("read").to_string(),
                                phase_tags: vec![phase.clone()],
                                subject_kinds: vec![subject_kind.clone()],
                                parameters: verb["parameters"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|parameter| crate::semtaxonomy::VerbParameter {
                                        name: parameter["name"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                        required: parameter["required"].as_bool().unwrap_or(false),
                                        description: parameter["description"]
                                            .as_str()
                                            .map(str::to_string),
                                    })
                                    .collect(),
                            })
                    })
                    .collect::<Vec<_>>();
                action_surface.append(&mut entries);
            }
        }
        action_surface.sort_by(|left, right| left.verb_id.cmp(&right.verb_id));
        action_surface.dedup_by(|left, right| left.verb_id == right.verb_id);
        let action_surface = (!action_surface.is_empty()).then_some(action_surface);
        let valid_transitions = state
            .research_cache
            .get("graph-walk")
            .cloned()
            .or_else(|| state.research_cache.get("valid-transitions").cloned());

        if let Some(entries) = action_surface.as_ref() {
            for entry in entries.iter().take(12) {
                if let Ok(detail) = self
                    .run_discovery_op(
                        "discovery",
                        "verb-detail",
                        vec![("verb-id", serde_json::json!(entry.verb_id.clone()))],
                    )
                    .await
                {
                    state
                        .research_cache
                        .insert(format!("verb-detail:{}", entry.verb_id), detail);
                }
            }
        }

        let domain_state_summaries =
            Self::build_domain_state_summaries(entity_state.as_ref(), &inferred_domains);

        hydrate_sage_session(
            &mut state,
            cascade_result.clone(),
            active_entity.clone(),
            entity_candidates,
            domain_scope.clone(),
            session.context.stage_focus.clone(),
            action_surface.clone().unwrap_or_default(),
            entity_state.clone(),
            domain_state_summaries,
            intent_hints,
            grounding_strategy,
            grounding_confidence,
        );
        session.semtaxonomy_session = Some(state);

        let history_depth = session.messages.len();
        let visible_action_count = action_surface.as_ref().map(|verbs| verbs.len()).unwrap_or(0);
        let selected_verb = if let Some(entity) = active_entity.as_ref() {
            let scope = EntityScope {
                entity_id: entity.entity_id,
                entity_type: entity.entity_type.clone(),
                name: entity.name.clone(),
                confidence: 1.0,
                source: EntitySource::SearchHit,
            };
            let state_model = step2_entity_state(
                scope,
                entity_state.as_ref(),
                valid_transitions.as_ref(),
            );
            step3_select_verb(&request.message, &state_model)
        } else {
            action_surface
                .as_ref()
                .map(|surface| {
                    let state_model = build_generic_state(&entity_type_for_actions, surface);
                    step3_select_verb(&request.message, &state_model)
                })
                .flatten()
        };
        let dsl = selected_verb
            .as_ref()
            .and_then(Self::render_selected_verb_dsl);
        let ready_to_execute = selected_verb
            .as_ref()
            .map(|selected| !selected.partial && !selected.verb_id.is_empty())
            .unwrap_or(false);
        let action_preview = action_surface
            .as_ref()
            .map(|verbs| {
                verbs.iter()
                    .take(3)
                    .map(|verb| verb.verb_id.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|preview| !preview.is_empty());
        let entity_summary = active_entity
            .as_ref()
            .map(|entity| format!("{} ({})", entity.name, entity.entity_type))
            .unwrap_or_else(|| "no entity grounded yet".to_string());
        let message = format!(
            "SemTaxonomy grounded this turn.\n\nEntity: {}\nDomain scope: {}\nHistory depth: {}{}\n{}",
            entity_summary,
            domain_scope.clone().unwrap_or_else(|| "unspecified".to_string()),
            history_depth,
            action_preview
                .map(|preview| format!("\nAvailable actions: {}", preview))
                .unwrap_or_default(),
            selected_verb
                .as_ref()
                .map(|selected| {
                    let missing = if selected.missing_args.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "\nMissing required inputs: {}",
                            selected
                                .missing_args
                                .iter()
                                .map(|arg| arg.name.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    format!("\n{}{}", selected.explanation, missing)
                })
                .unwrap_or_else(|| "\nNo deterministic runbook could be composed yet.".to_string())
        );

        let sage_explain = Some(ob_poc_types::chat::SageExplainPayload {
            understanding: selected_verb
                .as_ref()
                .map(|selected| selected.explanation.clone())
                .unwrap_or_else(|| {
                    format!(
                        "So you want to work from a grounded discovery context for: {}",
                        request.message
                    )
                }),
            mode: "semtaxonomy_discovery".to_string(),
            scope_summary: Some(format!(
                "entity={}, domain={}",
                entity_summary,
                domain_scope.unwrap_or_else(|| "unspecified".to_string())
            )),
            confidence: if active_entity.is_some() {
                "medium".to_string()
            } else {
                "low".to_string()
            },
            clarifications: Vec::new(),
        });
        let coder_proposal = Some(ob_poc_types::chat::CoderProposalPayload {
            verb_fqn: selected_verb
                .as_ref()
                .map(|selected| selected.verb_id.clone())
                .filter(|verb_id| !verb_id.is_empty()),
            dsl: dsl.clone(),
            change_summary: selected_verb
                .as_ref()
                .map(|selected| {
                    let mut summary = vec![selected.explanation.clone()];
                    if !selected.missing_args.is_empty() {
                        summary.push(format!(
                            "Missing required inputs: {}",
                            selected
                                .missing_args
                                .iter()
                                .map(|arg| arg.name.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    summary
                })
                .unwrap_or_else(|| {
                    vec![format!(
                        "CompositionRequest prepared with {} prior messages and {} visible actions",
                        history_depth,
                        visible_action_count
                    )]
                }),
            requires_confirmation: selected_verb
                .as_ref()
                .map(|selected| selected.requires_confirmation && !selected.partial)
                .unwrap_or(false),
            ready_to_execute,
        });

        if let (Some(selected), Some(dsl_source), Some(verb_fqn)) = (
            selected_verb.as_ref(),
            dsl.as_deref(),
            selected_verb
                .as_ref()
                .map(|candidate| candidate.verb_id.as_str()),
        ) {
            if selected.requires_confirmation && !selected.partial {
                let pending = Self::build_semtaxonomy_pending_mutation(
                    request,
                    session,
                    verb_fqn,
                    dsl_source,
                    &selected.explanation,
                );
                session.pending_decision = None;
                session.pending_intent_tier = None;
                session.pending_verb_disambiguation = None;
                if session.has_pending() {
                    session.cancel_pending();
                }
                session.pending_mutation = Some(pending.clone());
                session.transition(SessionEvent::DslPendingValidation);

                let bullets = if pending.change_summary.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n\nThis will:\n{}",
                        pending
                            .change_summary
                            .iter()
                            .map(|item| format!("• {}", item))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                };
                let message = format!(
                    "This would change state.\n\nPending change: {}{}\n\nReply 'yes' to confirm or ask a read-only question to cancel.",
                    pending.confirmation_text, bullets
                );
                let sage_explain = Some(Self::to_sage_explain_payload(&pending.intent.explain));
                let coder_proposal = Self::to_coder_proposal_payload(
                    Some(&pending),
                    Some(&pending.coder_result.dsl),
                    Some(&pending.coder_result.verb_fqn),
                    false,
                );

                Self::add_agent_message_with_payloads(
                    session,
                    message.clone(),
                    None,
                    sage_explain.clone(),
                    coder_proposal.clone(),
                );

                return Ok(Some(AgentChatResponse {
                    message,
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
                    decision: None,
                    sage_explain,
                    coder_proposal,
                }));
            }
        }

        Self::add_agent_message_with_payloads(
            session,
            message.clone(),
            dsl,
            sage_explain.clone(),
            coder_proposal.clone(),
        );

        Ok(Some(AgentChatResponse {
            message,
            session_state: SessionState::New,
            can_execute: ready_to_execute,
            dsl_source: if ready_to_execute {
                coder_proposal.as_ref().and_then(|proposal| proposal.dsl.clone())
            } else {
                None
            },
            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            decision: None,
            sage_explain,
            coder_proposal,
        }))
    }

    /// Return true when session context checks should not enforce client/deal gating.
    ///
    /// Semantic OS workflows are registry-scoped and should not force
    /// client-group/deal prompts before intent routing.
    fn skips_client_scope_gate(stage_focus: Option<&str>) -> bool {
        matches!(stage_focus, Some(s) if s.starts_with("semos-"))
    }

    /// Best-effort NL mapping for Semantic OS workflow selection prompts.
    ///
    /// Returns workflow choice ID expected by the pending decision packet.
    fn infer_semos_workflow_choice(input_lower: &str) -> Option<&'static str> {
        let normalized = input_lower.trim();
        if normalized.is_empty() {
            return None;
        }

        // Order matters: prefer explicit data-management phrasing first.
        if normalized.contains("data management")
            || normalized.contains("manage data")
            || normalized.contains("data entity")
            || normalized.contains("entity data")
            || normalized.contains("data entities")
            || normalized.contains("entity management")
            || normalized.contains("taxonomy")
            || normalized.contains("data governance")
        {
            return Some("3");
        }

        if normalized.contains("onboarding") || normalized.contains("onboard") {
            return Some("1");
        }

        if normalized.contains("kyc")
            || normalized.contains("know your customer")
            || normalized.contains("screening")
            || normalized.contains("due diligence")
        {
            return Some("2");
        }

        if normalized.contains("stewardship")
            || normalized.contains("publish")
            || normalized.contains("changeset")
            || normalized.contains("change set")
        {
            return Some("4");
        }

        None
    }

    /// Map Semantic OS stage focus to SemReg phase-tag goals.
    ///
    /// `semos-data-management` is intentionally expanded to include:
    /// - `data` (registry/data stewardship verbs)
    /// - `deal` (commercial data records)
    /// - `onboarding` (CBU-tagged data records)
    /// - `kyc` (document-tagged records)
    /// - `navigation` (session/view navigation verbs)
    fn stage_focus_goals(stage_focus: Option<&str>) -> Vec<String> {
        match stage_focus {
            Some("semos-data-management") | Some("semos-data") => vec![
                "data-management".to_string(),
                "data".to_string(),
                "deal".to_string(),
                "onboarding".to_string(),
                "kyc".to_string(),
                "navigation".to_string(),
            ],
            Some(s) if s.starts_with("semos-") => {
                vec![s.strip_prefix("semos-").unwrap_or_default().to_string()]
            }
            _ => vec![],
        }
    }

    /// Whether a pending decision is a hard gate that must be resolved first.
    fn is_mandatory_pending_decision(packet: &ob_poc_types::DecisionPacket) -> bool {
        use ob_poc_types::DecisionKind;
        matches!(packet.kind, DecisionKind::ClarifyGroup)
            || (matches!(packet.kind, DecisionKind::ClarifyScope)
                && packet.trace.decision_reason == "semos_workflow_selection")
    }

    /// Build and return a retry response while restoring pending decision state.
    fn reprompt_pending_decision(
        session: &mut UnifiedSession,
        packet: ob_poc_types::DecisionPacket,
        message: String,
    ) -> AgentChatResponse {
        session.pending_decision = Some(packet.clone());
        session.add_agent_message(message.clone(), None, None);
        AgentChatResponse {
            message,
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
            decision: Some(packet),
            sage_explain: None,
            coder_proposal: None,
        }
    }

    fn utterance_requires_deal_context(message: &str) -> bool {
        let normalized = message.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return false;
        }

        let explicit_deal_listing = [
            "what deals",
            "show deals",
            "list deals",
            "show me the deals",
            "show me deals",
        ];
        if explicit_deal_listing
            .iter()
            .any(|phrase| normalized.starts_with(phrase))
        {
            return false;
        }

        let current_deal_markers = [
            "this deal",
            "that deal",
            "current deal",
            "selected deal",
            "deal details",
            "deal status",
            "deal documents",
            "deal products",
            "deal parties",
            "deal timeline",
            "deal workflow",
            "load deal",
            "open deal",
            "switch deal",
        ];

        current_deal_markers
            .iter()
            .any(|marker| normalized.contains(marker))
    }

    /// Create agent service with pool and embedder
    ///
    /// The embedder is REQUIRED for semantic verb search. All prompts go through
    /// the Candle intent pipeline - there is no fallback path.
    ///
    /// The learned_data enables step 2 (global learned exact match) for phrases
    /// like "spin up a fund" → cbu.create. Without it, only semantic search is used.
    ///
    /// The lexicon enables step 0 (fast lexical matching) for exact label and
    /// token overlap matches. Runs BEFORE semantic embedding computation.
    ///
    /// The entity_linker enables entity mention extraction from utterances for
    /// context enrichment and disambiguation.
    pub fn new(
        pool: PgPool,
        embedder: Arc<CandleEmbedder>,
        learned_data: Option<crate::agent::learning::warmup::SharedLearnedData>,
        lexicon: Option<crate::mcp::verb_search::SharedLexicon>,
    ) -> Self {
        Self {
            pool,
            config: AgentServiceConfig::default(),
            embedder,
            learned_data,
            lexicon,
            entity_linker: None,
            policy_gate: Arc::new(crate::policy::PolicyGate::from_env()),
            sem_os_client: None,
            noun_index: None,
            macro_index: None,
            scenario_index: None,
            macro_registry: None,
            sage_engine: None,
        }
    }

    /// Set entity linker for in-memory entity resolution
    pub fn with_entity_linker(
        mut self,
        entity_linker: Arc<dyn crate::entity_linking::EntityLinkingService>,
    ) -> Self {
        self.entity_linker = Some(entity_linker);
        self
    }

    /// Set Semantic OS client for routing sem_reg calls through DI boundary
    pub fn with_sem_os_client(mut self, client: Arc<dyn sem_os_client::SemOsClient>) -> Self {
        self.sem_os_client = Some(client);
        self
    }

    /// Set NounIndex for deterministic Tier -1 ECIR noun→verb resolution
    pub fn with_noun_index(mut self, ni: Arc<NounIndex>) -> Self {
        self.noun_index = Some(ni);
        self
    }

    /// Set MacroIndex for deterministic Tier -2B macro search parity
    pub fn with_macro_index(mut self, mi: Arc<MacroIndex>) -> Self {
        self.macro_index = Some(mi);
        self
    }

    /// Set ScenarioIndex for journey-level Tier -2A compound intent resolution
    pub fn with_scenario_index(mut self, si: Arc<ScenarioIndex>) -> Self {
        self.scenario_index = Some(si);
        self
    }

    /// Set cached MacroRegistry (avoids reloading from disk on every verb search)
    pub fn with_macro_registry(mut self, mr: Arc<MacroRegistry>) -> Self {
        self.macro_registry = Some(mr);
        self
    }

    /// Set Sage engine for Stage 1.5 shadow classification.
    ///
    /// # Examples
    /// ```ignore
    /// use std::sync::Arc;
    /// use ob_poc::sage::DeterministicSage;
    ///
    /// let service = service.with_sage_engine(Arc::new(DeterministicSage));
    /// ```
    pub fn with_sage_engine(mut self, sage_engine: Arc<dyn SageEngine>) -> Self {
        self.sage_engine = Some(sage_engine);
        self
    }

    /// Extract entity mentions from utterance and build debug info
    ///
    /// Returns (entity_resolution_debug, dominant_entity_id, expected_kinds)
    /// The dominant_entity_id can be used to constrain verb argument resolution.
    fn extract_entity_mentions(
        &self,
        utterance: &str,
        expected_kinds: Option<&[String]>,
    ) -> (
        Option<ob_poc_types::EntityResolutionDebug>,
        Option<uuid::Uuid>,
        Vec<String>,
    ) {
        let Some(linker) = &self.entity_linker else {
            return (None, None, vec![]);
        };

        // Extract entity mentions from utterance
        let resolutions = linker.resolve_mentions(
            utterance,
            expected_kinds,
            None, // No context concepts for now
            5,    // Top 5 candidates per mention
        );

        if resolutions.is_empty() {
            // No mentions found - still return debug info showing snapshot was checked
            let debug = ob_poc_types::EntityResolutionDebug {
                snapshot_hash: linker.snapshot_hash().to_string(),
                entity_count: linker.entity_count(),
                mentions: vec![],
                dominant_entity: None,
                expected_kinds: expected_kinds.map(|k| k.to_vec()).unwrap_or_default(),
            };
            return (Some(debug), None, vec![]);
        }

        // Build debug info
        let mentions: Vec<ob_poc_types::EntityMentionDebug> = resolutions
            .iter()
            .map(|r| {
                let candidates: Vec<ob_poc_types::EntityCandidateDebug> = r
                    .candidates
                    .iter()
                    .take(3)
                    .map(|c| ob_poc_types::EntityCandidateDebug {
                        entity_id: c.entity_id.to_string(),
                        entity_kind: c.entity_kind.clone(),
                        canonical_name: c.canonical_name.clone(),
                        score: c.score,
                        evidence: c.evidence.iter().map(|e| format!("{:?}", e)).collect(),
                    })
                    .collect();

                ob_poc_types::EntityMentionDebug {
                    span: r.mention_span,
                    text: r.mention_text.clone(),
                    candidates,
                    selected_id: r.selected.map(|id| id.to_string()),
                    confidence: r.confidence,
                }
            })
            .collect();

        // Find dominant entity (highest confidence with selection)
        let dominant = resolutions
            .iter()
            .filter(|r| r.selected.is_some() && r.confidence > 0.5)
            .max_by(|a, b| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let dominant_debug = dominant.and_then(|r| {
            r.candidates
                .first()
                .map(|c| ob_poc_types::EntityCandidateDebug {
                    entity_id: c.entity_id.to_string(),
                    entity_kind: c.entity_kind.clone(),
                    canonical_name: c.canonical_name.clone(),
                    score: c.score,
                    evidence: c.evidence.iter().map(|e| format!("{:?}", e)).collect(),
                })
        });

        let dominant_id = dominant.and_then(|r| r.selected);

        // Collect entity kinds from resolved mentions for verb search hints
        let resolved_kinds: Vec<String> = resolutions
            .iter()
            .filter(|r| r.selected.is_some())
            .filter_map(|r| r.candidates.first())
            .map(|c| c.entity_kind.clone())
            .collect();

        let debug = ob_poc_types::EntityResolutionDebug {
            snapshot_hash: linker.snapshot_hash().to_string(),
            entity_count: linker.entity_count(),
            mentions,
            dominant_entity: dominant_debug,
            expected_kinds: expected_kinds.map(|k| k.to_vec()).unwrap_or_default(),
        };

        (Some(debug), dominant_id, resolved_kinds)
    }

    /// Build the verb searcher with all search indices.
    ///
    /// Uses cached MacroRegistry, NounIndex, MacroIndex, and ScenarioIndex
    /// (loaded once at startup) instead of reloading from disk on every call.
    fn build_verb_searcher(&self) -> crate::mcp::verb_search::HybridVerbSearcher {
        let dyn_embedder: Arc<dyn crate::agent::learning::embedder::Embedder> =
            self.embedder.clone() as Arc<dyn crate::agent::learning::embedder::Embedder>;

        // Use cached macro registry, falling back to disk load if not provided
        let macro_reg = self.macro_registry.clone().unwrap_or_else(|| {
            let macro_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("config/verb_schemas/macros");
            let reg = load_macro_registry_from_dir(&macro_dir).unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to load operator macros: {}, using empty registry",
                    e
                );
                MacroRegistry::new()
            });
            Arc::new(reg)
        });

        // Use factory for consistent configuration across all call sites
        VerbSearcherFactory::build(
            &self.pool,
            dyn_embedder,
            self.learned_data.clone(),
            macro_reg,
            self.lexicon.clone(),
            self.noun_index.clone(),
            self.macro_index.clone(),
            self.scenario_index.clone(),
        )
    }

    /// Create IntentPipeline for processing user input
    /// Build an OrchestratorContext for the unified intent pipeline.
    fn build_orchestrator_context(
        &self,
        session: &crate::session::UnifiedSession,
        actor: crate::sem_reg::abac::ActorContext,
        source: crate::agent::orchestrator::UtteranceSource,
    ) -> crate::agent::orchestrator::OrchestratorContext {
        use crate::agent::orchestrator::OrchestratorContext;

        // Map stage_focus to SemReg goals for verb phase_tag filtering.
        let goals = Self::stage_focus_goals(session.context.stage_focus.as_deref());

        OrchestratorContext {
            actor,
            session_id: Some(session.id),
            case_id: session.current_case.as_ref().map(|c| c.case_id),
            dominant_entity_id: session.context.dominant_entity_id,
            scope: session.context.client_scope.clone(),
            pool: self.pool.clone(),
            verb_searcher: std::sync::Arc::new(self.build_verb_searcher()),
            lookup_service: self.get_lookup_service(),
            policy_gate: self.policy_gate.clone(),
            source,
            sem_os_client: self.sem_os_client.clone(),
            agent_mode: sem_os_core::authoring::agent_mode::AgentMode::default(),
            goals,
            stage_focus: session.context.stage_focus.clone(),
            sage_engine: self.sage_engine.clone(),
            pre_sage_entity_kind: Self::current_sage_entity_kind(session),
            pre_sage_entity_name: Self::current_sage_entity_name(session),
            recent_sage_intents: session.recent_sage_intents.clone(),
        }
    }

    fn current_sage_entity_kind(session: &crate::session::UnifiedSession) -> Option<String> {
        session
            .context
            .active_cbu
            .as_ref()
            .map(|entity| entity.entity_type.clone())
            .or_else(|| session.current_structure.as_ref().map(|_| "structure".to_string()))
            .or_else(|| session.current_case.as_ref().map(|_| "kyc-case".to_string()))
            .or_else(|| session.current_mandate.as_ref().map(|_| "mandate".to_string()))
            .or_else(|| session.domain_hint.clone())
            .or_else(|| (!session.entity_type.is_empty()).then(|| session.entity_type.clone()))
    }

    fn current_sage_entity_name(session: &crate::session::UnifiedSession) -> Option<String> {
        session
            .context
            .active_cbu
            .as_ref()
            .map(|entity| entity.display_name.clone())
            .or_else(|| session.current_structure.as_ref().map(|item| item.display_name.clone()))
            .or_else(|| session.current_case.as_ref().map(|item| item.display_name.clone()))
            .or_else(|| session.current_mandate.as_ref().map(|item| item.display_name.clone()))
            .or_else(|| session.client.as_ref().map(|item| item.display_name.clone()))
    }

    fn push_recent_sage_intent(
        session: &mut crate::session::UnifiedSession,
        intent: &crate::sage::OutcomeIntent,
    ) {
        session.recent_sage_intents.push(crate::sage::RecentIntent {
            plane: intent.plane.as_str().to_string(),
            domain_concept: intent.domain_concept.clone(),
            action: intent.action.as_str().to_string(),
            confidence: intent.confidence.as_str().to_string(),
        });
        if session.recent_sage_intents.len() > 5 {
            let keep_from = session.recent_sage_intents.len() - 5;
            session.recent_sage_intents.drain(0..keep_from);
        }
    }

    fn to_sage_explain_payload(
        explain: &crate::sage::SageExplain,
    ) -> ob_poc_types::chat::SageExplainPayload {
        ob_poc_types::chat::SageExplainPayload {
            understanding: explain.understanding.clone(),
            mode: explain.mode.clone(),
            scope_summary: explain.scope_summary.clone(),
            confidence: explain.confidence.clone(),
            clarifications: explain.clarifications.clone(),
        }
    }

    fn to_coder_proposal_payload(
        pending_mutation: Option<&crate::sage::PendingMutation>,
        dsl: Option<&str>,
        final_verb: Option<&str>,
        can_execute: bool,
    ) -> Option<ob_poc_types::chat::CoderProposalPayload> {
        if pending_mutation.is_none() && dsl.is_none() && final_verb.is_none() {
            return None;
        }
        Some(ob_poc_types::chat::CoderProposalPayload {
            verb_fqn: pending_mutation
                .map(|pending| pending.coder_result.verb_fqn.clone())
                .or_else(|| final_verb.map(str::to_string)),
            dsl: pending_mutation
                .map(|pending| pending.coder_result.dsl.clone())
                .or_else(|| dsl.map(str::to_string)),
            change_summary: pending_mutation
                .map(|pending| pending.change_summary.clone())
                .unwrap_or_default(),
            requires_confirmation: pending_mutation.is_some(),
            ready_to_execute: can_execute,
        })
    }

    fn build_semtaxonomy_pending_mutation(
        request: &ChatRequest,
        session: &UnifiedSession,
        verb_fqn: &str,
        dsl: &str,
        explanation: &str,
    ) -> crate::sage::PendingMutation {
        let mut intent = crate::sage::OutcomeIntent::stub(
            &request.message,
            crate::sage::ObservationPlane::Instance,
            crate::sage::IntentPolarity::Write,
        );
        intent.summary = explanation.to_string();
        intent.domain_concept = verb_fqn
            .split('.')
            .next()
            .unwrap_or_default()
            .to_string();
        intent.action = crate::sage::OutcomeAction::from_first_word(&request.message);
        intent.subject = session
            .semtaxonomy_session
            .as_ref()
            .and_then(|state| state.active_entity.as_ref())
            .map(|entity| crate::sage::EntityRef {
                mention: entity.name.clone(),
                kind_hint: Some(entity.entity_type.clone()),
                uuid: Some(entity.entity_id),
            });
        intent.explain = crate::sage::SageExplain {
            understanding: explanation.to_string(),
            mode: "confirmation_required".to_string(),
            scope_summary: session
                .semtaxonomy_session
                .as_ref()
                .and_then(|state| state.active_entity.as_ref())
                .map(|entity| format!("entity={} ({})", entity.name, entity.entity_type)),
            confidence: "medium".to_string(),
            clarifications: Vec::new(),
        };
        intent.coder_handoff.goal = "compose_runbook".to_string();
        intent.coder_handoff.intent_summary = explanation.to_string();
        intent.coder_handoff.required_outcome = format!("execute {verb_fqn}");
        intent.coder_handoff.constraints = vec!["no_mutation_without_confirmation".to_string()];
        intent.coder_handoff.serve_safe = false;
        intent.coder_handoff.requires_confirmation = true;

        let coder_result = crate::sage::CoderResult {
            verb_fqn: verb_fqn.to_string(),
            dsl: dsl.to_string(),
            resolution: crate::sage::coder::CoderResolution::Proposed,
            missing_args: Vec::new(),
            unresolved_refs: Vec::new(),
            diagnostics: None,
        };

        let subject_name = intent
            .subject
            .as_ref()
            .map(|subject| subject.mention.as_str())
            .unwrap_or("this");
        let action_word = match intent.action {
            crate::sage::OutcomeAction::Create => "create",
            crate::sage::OutcomeAction::Update => "update",
            crate::sage::OutcomeAction::Delete => "delete",
            crate::sage::OutcomeAction::Assign => "assign",
            crate::sage::OutcomeAction::Import => "import",
            crate::sage::OutcomeAction::Publish => "publish",
            _ => "change",
        };

        crate::sage::PendingMutation {
            confirmation_text: format!("So you want to {action_word} {subject_name}?"),
            change_summary: vec![
                format!("Resolved action: {verb_fqn}"),
                explanation.to_string(),
            ],
            coder_result,
            intent,
        }
    }

    fn add_agent_message_with_payloads(
        session: &mut UnifiedSession,
        content: String,
        dsl: Option<String>,
        sage_explain: Option<ob_poc_types::chat::SageExplainPayload>,
        coder_proposal: Option<ob_poc_types::chat::CoderProposalPayload>,
    ) {
        session.add_agent_message(content, None, dsl);
        if let Some(last) = session.messages.last_mut() {
            last.sage_explain = sage_explain;
            last.coder_proposal = coder_proposal;
        }
    }

    fn scope_feedback_response(
        session: &mut UnifiedSession,
        message: String,
        sage_explain: ob_poc_types::chat::SageExplainPayload,
    ) -> AgentChatResponse {
        Self::add_agent_message_with_payloads(
            session,
            message.clone(),
            None,
            Some(sage_explain.clone()),
            None,
        );
        AgentChatResponse {
            message,
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
            decision: None,
            sage_explain: Some(sage_explain),
            coder_proposal: None,
        }
    }

    /// Get or build the LookupService for unified verb + entity discovery
    ///
    /// Returns None if entity_linker is not configured (graceful degradation).
    /// Builds on-demand using existing components (entity_linker, verb_searcher, lexicon).
    fn get_lookup_service(&self) -> Option<crate::lookup::LookupService> {
        // Build on demand if we have entity_linker
        let entity_linker = self.entity_linker.clone()?;
        let verb_searcher = Arc::new(self.build_verb_searcher());

        let mut lookup_svc = crate::lookup::LookupService::new(entity_linker);
        lookup_svc = lookup_svc.with_verb_searcher(verb_searcher);

        if let Some(ref lexicon) = self.lexicon {
            lookup_svc = lookup_svc.with_lexicon(lexicon.clone());
        }

        Some(lookup_svc)
    }

    /// Resolve the allowed verb set for the current session context.
    ///
    /// Uses the **same** `build_orchestrator_context()` + `resolve_sem_reg_verbs()`
    /// path as `process_chat()` / `handle_utterance()`, guaranteeing the returned
    /// `ContextEnvelope` carries the identical verb set the agent pipeline would use.
    #[cfg(feature = "database")]
    pub async fn resolve_options(
        &self,
        session: &crate::session::UnifiedSession,
        actor: crate::sem_reg::abac::ActorContext,
    ) -> Result<crate::agent::context_envelope::ContextEnvelope, String> {
        let ctx = self.build_orchestrator_context(
            session,
            actor,
            crate::agent::orchestrator::UtteranceSource::Chat,
        );
        let envelope =
            crate::agent::orchestrator::resolve_sem_reg_verbs(&ctx, None, false).await;
        Ok(envelope)
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
        actor: crate::sem_reg::abac::ActorContext,
    ) -> Result<AgentChatResponse, String> {
        use crate::dsl_v2::parse_program;
        use crate::mcp::intent_pipeline::PipelineOutcome;
        session.add_user_message(request.message.clone());

        let input = request.message.trim().to_lowercase();
        let mut pivot_feedback: Option<String> = None;

        // 1. Check for pending mutation confirmation before anything else.
        if let Some(pending) = session.pending_mutation.clone() {
            if crate::agent::orchestrator::is_confirmation(&request.message) {
                session.pending_mutation = None;
                session.pending_decision = None;
                session.pending_intent_tier = None;
                session.pending_verb_disambiguation = None;
                if session.has_pending() {
                    session.cancel_pending();
                }
                let ast = parse_program(&pending.coder_result.dsl)
                    .map(|p| p.statements)
                    .unwrap_or_default();
                session.set_pending_dsl(pending.coder_result.dsl.clone(), ast, None, false);
                let mut response = self.execute_runbook(session).await?;
                response.dsl_source = None;
                if let Some(last) = session.messages.last_mut() {
                    last.sage_explain =
                        Some(Self::to_sage_explain_payload(&pending.intent.explain));
                    last.coder_proposal = Self::to_coder_proposal_payload(
                        Some(&pending),
                        Some(&pending.coder_result.dsl),
                        Some(&pending.coder_result.verb_fqn),
                        false,
                    );
                }
                return Ok(response);
            }
            session.pending_mutation = None;
            session.pending_decision = None;
            session.pending_intent_tier = None;
            session.pending_verb_disambiguation = None;
            if session.has_pending() {
                session.cancel_pending();
            }
            if Self::is_read_only_pivot_request(&request.message) {
                pivot_feedback = Some(
                    "Cancelled the pending change and switched back to read-only mode."
                        .to_string(),
                );
            }
        }

        if crate::agent::orchestrator::is_confirmation(&request.message) {
            session.pending_decision = None;
            session.pending_intent_tier = None;
            session.pending_verb_disambiguation = None;
            let msg = Self::with_pivot_feedback(
                &pivot_feedback,
                "There is no pending change to confirm. I am still in read-only mode.",
            );
            return Ok(self.fail(&msg, session));
        }

        // 2. Check for RUN command - execute staged runbook
        if matches!(
            input.as_str(),
            "run" | "execute" | "do it" | "go" | "run it" | "execute it"
        ) {
            return self.execute_runbook(session).await;
        }

        if Self::semtaxonomy_enabled() {
            if session.pending_verb_disambiguation.take().is_some() {
                pivot_feedback = Some(
                    "Discarded the legacy pending verb choice and re-routed through SemTaxonomy."
                        .to_string(),
                );
            }
            if session.pending_intent_tier.take().is_some() {
                pivot_feedback = Some(
                    "Discarded the legacy pending intent choice and re-routed through SemTaxonomy."
                        .to_string(),
                );
            }
            if session.pending_decision.take().is_some() {
                pivot_feedback = Some(
                    "Discarded the legacy pending choice and re-routed through SemTaxonomy."
                        .to_string(),
                );
            }

            if let Some(mut response) = self
                .try_semtaxonomy_path(session, request, session.context.dominant_entity_id, &[])
                .await?
            {
                response.message = Self::with_pivot_feedback(&pivot_feedback, response.message);
                if let Some(last) = session.messages.last_mut() {
                    last.content = response.message.clone();
                    last.sage_explain = response.sage_explain.clone();
                    last.coder_proposal = response.coder_proposal.clone();
                }
                return Ok(response);
            }

            let msg = Self::with_pivot_feedback(
                &pivot_feedback,
                "SemTaxonomy could not ground this utterance yet.",
            );
            return Ok(self.fail(&msg, session));
        }

        // 3. Check for pending verb disambiguation - numeric input selects an option
        if session.pending_verb_disambiguation.is_some()
            && Self::should_reclassify_before_pending(&request.message)
        {
            session.pending_verb_disambiguation = None;
            pivot_feedback = Some(
                "Discarded the pending verb choice and re-routed from your new instruction."
                    .to_string(),
            );
        }
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
                            actor.clone(),
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
                        decision: None,
                        sage_explain: None,
                        coder_proposal: None,
                    });
                }
            }
            // Not a number - clear pending and process as new input
            session.pending_verb_disambiguation = None;
        }

        // 4. Check for pending decision (client group or deal selection)
        if let Some(pending) = session.pending_decision.take() {
            if Self::semtaxonomy_enabled() {
                pivot_feedback = Some(
                    "Discarded the legacy pending choice and re-routed through SemTaxonomy."
                        .to_string(),
                );
            } else if Self::should_reclassify_before_pending(&request.message) {
                pivot_feedback = Some(
                    "Discarded the pending choice and re-routed from your new instruction."
                        .to_string(),
                );
            } else {
            // Check if input is a number (1, 2, 3, etc.) or special keyword
            let input_upper = input.trim().to_uppercase();
            let input_lower = input.trim().to_lowercase();
            if input_upper == "NEW"
                || input_upper == "SKIP"
                || input.trim().parse::<usize>().is_ok()
            {
                // User is responding to the decision prompt
                let choice_id = if input_upper == "NEW" {
                    "NEW".to_string()
                } else if input_upper == "SKIP" {
                    "SKIP".to_string()
                } else {
                    input.trim().to_string()
                };

                // Find the matching choice
                if let Some(choice) = pending.choices.iter().find(|c| c.id == choice_id) {
                    let choice = choice.clone();
                    return self
                        .handle_decision_selection(session, &pending, &choice)
                        .await;
                } else if let Ok(idx) = input.trim().parse::<usize>() {
                    // Try index-based selection
                    if idx >= 1 && idx <= pending.choices.len() {
                        let choice = pending.choices[idx - 1].clone();
                        return self
                            .handle_decision_selection(session, &pending, &choice)
                            .await;
                    }
                }

                // Invalid selection — restore pending so user can retry
                let msg = if pending
                    .choices
                    .iter()
                    .any(|c| c.id == "NEW" || c.id == "SKIP")
                {
                    format!(
                        "Please select a valid option (1-{}) or type NEW/SKIP.",
                        pending.choices.len()
                    )
                } else {
                    format!(
                        "Please select a valid option (1-{}).",
                        pending.choices.len()
                    )
                };
                return Ok(Self::reprompt_pending_decision(session, pending, msg));
            }
            // Not a number/keyword - try fuzzy match against choice labels
            // This handles cases like typing "aviva" when the choices list
            // contains "Aviva Investors"
            if let Some(matched_idx) = pending
                .choices
                .iter()
                .position(|c| c.label.to_lowercase().contains(&input_lower))
            {
                let choice = pending.choices[matched_idx].clone();
                return self
                    .handle_decision_selection(session, &pending, &choice)
                    .await;
            }

            // Semantic OS workflow selection accepts natural language intents,
            // e.g. "I want to manage data" should map to Data Management.
            if pending.trace.decision_reason == "semos_workflow_selection" {
                if let Some(choice_id) = Self::infer_semos_workflow_choice(&input_lower) {
                    if let Some(choice) = pending.choices.iter().find(|c| c.id == choice_id) {
                        return self
                            .handle_decision_selection(session, &pending, choice)
                            .await;
                    }
                }
            }

            // No match found against choice labels.
            // For deal selection, treat unmatched input as implicit SKIP
            // so the user can proceed to the intent pipeline (e.g. "show me lux cbu").
            // For client group, the gate is mandatory so we re-prompt.
            if matches!(pending.kind, ob_poc_types::DecisionKind::ClarifyDeal) {
                let skip_choice = pending
                    .choices
                    .iter()
                    .find(|c| c.id == "SKIP")
                    .cloned()
                    .unwrap_or_else(|| ob_poc_types::UserChoice {
                        id: "SKIP".to_string(),
                        label: "Skip for now".to_string(),
                        description: String::new(),
                        is_escape: true,
                    });

                // Auto-skip deal selection, then process the original input
                let _ = self
                    .handle_decision_selection(session, &pending, &skip_choice)
                    .await;
                // Fall through to process original input via intent pipeline
            }
            // Mandatory decisions must remain active until user picks an option.
            if Self::is_mandatory_pending_decision(&pending) {
                let msg = pending.prompt.clone();
                return Ok(Self::reprompt_pending_decision(session, pending, msg));
            }
            // Optional decisions can fall through to the normal intent pipeline.
            }
        }

        // 5. Check for pending intent tier selection
        if session.pending_intent_tier.is_some()
            && Self::should_reclassify_before_pending(&request.message)
        {
            session.pending_intent_tier = None;
            pivot_feedback = Some(
                "Discarded the pending intent choice and re-routed from your new instruction."
                    .to_string(),
            );
        }
        if let Some(ref pending_tier) = session.pending_intent_tier.clone() {
            // Check if input is a number selecting a tier option
            if let Ok(selection) = input.trim().parse::<usize>() {
                let intent_taxonomy = crate::dsl_v2::intent_tiers::intent_tier_taxonomy();

                // Rebuild the tier options to check bounds
                let tier_options = {
                    let verbs: Vec<&str> = pending_tier
                        .candidates
                        .iter()
                        .map(|c| c.verb.as_str())
                        .collect();
                    let analysis = intent_taxonomy.analyze_candidates(&verbs);
                    intent_taxonomy.build_tier1_request(&pending_tier.original_input, &analysis)
                };

                if selection >= 1 && selection <= tier_options.options.len() {
                    let selected_tier = &tier_options.options[selection - 1];
                    let selected_id = selected_tier.id.clone();

                    tracing::info!(
                        selected_tier = %selected_id,
                        original_input = %pending_tier.original_input,
                        "User selected intent tier"
                    );

                    // Filter candidates to selected tier's verbs
                    let filtered: Vec<crate::session::unified::VerbCandidate> = pending_tier
                        .candidates
                        .iter()
                        .filter(|c| {
                            intent_taxonomy
                                .get_verb_tiers(&c.verb)
                                .map(|(t1, _)| t1 == selected_id)
                                .unwrap_or(false)
                        })
                        .cloned()
                        .collect();

                    session.pending_intent_tier = None;

                    if filtered.len() == 1 {
                        // Single verb in this tier — proceed directly
                        let selected_verb = filtered[0].verb.clone();
                        let original_input = pending_tier.original_input.clone();
                        let all_candidates = pending_tier.candidates.clone();
                        return self
                            .handle_verb_selection(
                                session,
                                &original_input,
                                &selected_verb,
                                &all_candidates
                                    .iter()
                                    .map(|c| crate::session::unified::VerbCandidate {
                                        verb: c.verb.clone(),
                                        score: c.score,
                                    })
                                    .collect::<Vec<_>>(),
                                actor.clone(),
                            )
                            .await;
                    } else if !filtered.is_empty() {
                        // Multiple verbs in this tier — show verb disambiguation
                        let search_results: Vec<crate::mcp::verb_search::VerbSearchResult> =
                            filtered
                                .iter()
                                .map(|c| crate::mcp::verb_search::VerbSearchResult {
                                    verb: c.verb.clone(),
                                    score: c.score,
                                    source:
                                        crate::mcp::verb_search::VerbSearchSource::PatternEmbedding,
                                    matched_phrase: pending_tier.original_input.clone(),
                                    description: None,
                                    journey: None,
                                })
                                .collect();
                        return Ok(self.build_verb_disambiguation_response(
                            &pending_tier.original_input,
                            &search_results,
                            session,
                        ));
                    }
                    // No verbs matched tier — fall through to pipeline
                }
            }
            // Not a number or invalid — clear and process as new input
            session.pending_intent_tier = None;
        }

        // =====================================================================
        // SESSION CONTEXT CHECK - Client Group → Deal flow
        // =====================================================================
        // At the start of a session, we need:
        // 1. Client group set (who are we working with?)
        // 2. Deal context (which deal are we working on?)
        //
        // If client group is set but no deal, check for existing deals and prompt.
        // This makes "deal" a first-class concept the agent understands.
        // =====================================================================
        tracing::info!(
            semtaxonomy_enabled = Self::semtaxonomy_enabled(),
            session_id = %session.id,
            message = %request.message,
            "process_chat about to evaluate SemTaxonomy branch"
        );
        if let Some(response) = self
            .try_semtaxonomy_path(session, request, session.context.dominant_entity_id, &[])
            .await?
        {
            return Ok(response);
        }

        if let Some(decision) = self.check_session_context(session, Some(&request.message)).await {
            return Ok(decision);
        }

        // UNIFIED LOOKUP - Verb-first dual search
        // If LookupService is available (entity_linker configured), use it for combined
        // verb + entity discovery. Otherwise fall back to separate entity linking.
        let (entity_resolution_debug, dominant_entity_id, resolved_kinds) =
            if let Some(lookup_service) = self.get_lookup_service() {
                // Unified path: verb-first ordering
                let lookup_result = lookup_service.analyze(&request.message, 5).await;

                tracing::debug!(
                    verb_matched = lookup_result.verb_matched,
                    entities_resolved = lookup_result.entities_resolved,
                    verb_count = lookup_result.verbs.len(),
                    entity_count = lookup_result.entities.len(),
                    expected_kinds = ?lookup_result.expected_kinds,
                    "LookupService analysis completed"
                );

                // Build debug info from lookup result
                let er_debug = if !lookup_result.entities.is_empty()
                    || lookup_result.dominant_entity.is_some()
                {
                    let mentions: Vec<ob_poc_types::EntityMentionDebug> = lookup_result
                        .entities
                        .iter()
                        .map(|r| {
                            let candidates: Vec<ob_poc_types::EntityCandidateDebug> = r
                                .candidates
                                .iter()
                                .take(3)
                                .map(|c| ob_poc_types::EntityCandidateDebug {
                                    entity_id: c.entity_id.to_string(),
                                    entity_kind: c.entity_kind.clone(),
                                    canonical_name: c.canonical_name.clone(),
                                    score: c.score,
                                    evidence: c
                                        .evidence
                                        .iter()
                                        .map(|e| format!("{:?}", e))
                                        .collect(),
                                })
                                .collect();

                            ob_poc_types::EntityMentionDebug {
                                span: r.mention_span,
                                text: r.mention_text.clone(),
                                candidates,
                                selected_id: r.selected.map(|id| id.to_string()),
                                confidence: r.confidence,
                            }
                        })
                        .collect();

                    let dominant_debug = lookup_result.dominant_entity.as_ref().map(|d| {
                        ob_poc_types::EntityCandidateDebug {
                            entity_id: d.entity_id.to_string(),
                            entity_kind: d.entity_kind.clone(),
                            canonical_name: d.canonical_name.clone(),
                            score: d.confidence,
                            evidence: vec![],
                        }
                    });

                    Some(ob_poc_types::EntityResolutionDebug {
                        snapshot_hash: self
                            .entity_linker
                            .as_ref()
                            .map(|l| l.snapshot_hash().to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                        entity_count: self
                            .entity_linker
                            .as_ref()
                            .map(|l| l.entity_count())
                            .unwrap_or(0),
                        mentions,
                        dominant_entity: dominant_debug,
                        expected_kinds: lookup_result.expected_kinds.clone(),
                    })
                } else {
                    None
                };

                let dominant_id = lookup_result.dominant_entity.as_ref().map(|d| d.entity_id);
                let kinds: Vec<String> = lookup_result
                    .entities
                    .iter()
                    .filter(|r| r.selected.is_some())
                    .filter_map(|r| r.candidates.first())
                    .map(|c| c.entity_kind.clone())
                    .collect();

                (er_debug, dominant_id, kinds)
            } else {
                // Legacy path: separate entity linking
                self.extract_entity_mentions(&request.message, None)
            };

        if let Some(ref er_debug) = entity_resolution_debug {
            tracing::debug!(
                snapshot_hash = %er_debug.snapshot_hash,
                entity_count = er_debug.entity_count,
                mention_count = er_debug.mentions.len(),
                dominant = ?er_debug.dominant_entity.as_ref().map(|e| &e.canonical_name),
                resolved_kinds = ?resolved_kinds,
                "Entity resolution completed"
            );
        }

        // Store dominant entity in session context for downstream resolution
        if let Some(entity_id) = dominant_entity_id {
            session.context.dominant_entity_id = Some(entity_id);
        }

        // ONE PIPELINE - generate/validate DSL via unified orchestrator
        let orch_ctx = self.build_orchestrator_context(
            session,
            actor.clone(),
            crate::agent::orchestrator::UtteranceSource::Chat,
        );
        let orch_outcome =
            crate::agent::orchestrator::handle_utterance(&orch_ctx, &request.message).await;
        let (result, journey_match, journey_decision, pending_mutation, auto_execute, sage_intent) = match orch_outcome {
            Ok(o) => (
                Ok(o.pipeline_result),
                o.trace.journey_match,
                o.journey_decision,
                o.pending_mutation,
                o.auto_execute,
                o.sage_intent,
            ),
            Err(e) => (Err(e), None, None, None, false, None),
        };
        if let Some(intent) = sage_intent.as_ref() {
            Self::push_recent_sage_intent(session, intent);
        }
        let sage_explain_payload = sage_intent
            .as_ref()
            .map(|intent| Self::to_sage_explain_payload(&intent.explain));

        match result {
            Ok(r) => {
                if let Some(pending) = pending_mutation {
                    session.pending_decision = None;
                    session.pending_intent_tier = None;
                    session.pending_verb_disambiguation = None;
                    if session.has_pending() {
                        session.cancel_pending();
                    }
                    session.pending_mutation = Some(pending.clone());
                    let bullets = if pending.change_summary.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "\n\nThis will:\n{}",
                            pending
                                .change_summary
                                .iter()
                                .map(|item| format!("• {}", item))
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                    };
                    let msg = format!(
                        "This would change state.\n\nPending change: {}{}\n\nReply 'yes' to confirm or ask a read-only question to cancel.",
                        pending.confirmation_text, bullets
                    );
                    let msg = Self::with_pivot_feedback(&pivot_feedback, msg);
                    let coder_proposal = Self::to_coder_proposal_payload(
                        Some(&pending),
                        Some(&pending.coder_result.dsl),
                        Some(&pending.coder_result.verb_fqn),
                        false,
                    );
                    Self::add_agent_message_with_payloads(
                        session,
                        msg.clone(),
                        None,
                        sage_explain_payload.clone(),
                        coder_proposal.clone(),
                    );
                    return Ok(AgentChatResponse {
                        message: msg,
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
                        decision: None,
                        sage_explain: sage_explain_payload.clone(),
                        coder_proposal,
                    });
                }

                // Handle scope resolution - "work on allianz", "switch to blackrock"
                if let PipelineOutcome::ScopeResolved {
                    ref group_id,
                    ref group_name,
                    entity_count,
                } = r.outcome
                {
                    if Self::semtaxonomy_enabled()
                        && session
                            .semtaxonomy_session
                            .as_ref()
                            .and_then(|state| state.active_entity.as_ref())
                            .map(|entity| entity.entity_type.eq_ignore_ascii_case("client-group"))
                            .unwrap_or(false)
                    {
                        tracing::info!(
                            group_id = %group_id,
                            group_name = %group_name,
                            entity_count = entity_count,
                            "Suppressing legacy scope response because SemTaxonomy already grounded client scope"
                        );
                    } else {
                    tracing::info!(
                        group_id = %group_id,
                        group_name = %group_name,
                        entity_count = entity_count,
                        "Scope resolved via pipeline, updating session context"
                    );

                    // Update session context with resolved scope
                    if let Ok(uuid) = group_id.parse::<uuid::Uuid>() {
                        let scope = crate::mcp::scope_resolution::ScopeContext::new()
                            .with_client_group(uuid, group_name.clone());
                        session.context.set_client_scope(scope);
                        // Reset deal context when switching clients
                        session.context.deal_id = None;
                        session.context.deal_gate_skipped = false;
                    }

                    let msg = format!(
                        "Now working with client: {} ({} entities in scope).",
                        group_name, entity_count
                    );
                    let msg = Self::with_pivot_feedback(&pivot_feedback, msg);
                    Self::add_agent_message_with_payloads(
                        session,
                        msg.clone(),
                        None,
                        sage_explain_payload.clone(),
                        None,
                    );
                    return Ok(AgentChatResponse {
                        message: msg,

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
                        decision: None,
                        sage_explain: sage_explain_payload.clone(),
                        coder_proposal: None,
                    });
                    }
                }

                // Handle scope candidates - multiple client matches
                if matches!(r.outcome, PipelineOutcome::ScopeCandidates) {
                    if let Some(err) = r.validation_error {
                        let err = Self::with_pivot_feedback(&pivot_feedback, err);
                        return Ok(self.fail(&err, session));
                    }
                }

                // Got valid DSL?
                if r.valid && !r.dsl.is_empty() {
                    if auto_execute {
                        let ast = parse_program(&r.dsl)
                            .map(|p| p.statements)
                            .unwrap_or_default();
                        session.set_pending_dsl_with_labels(
                            r.dsl.clone(),
                            ast,
                            None,
                            false,
                            Self::build_journey_labels(&journey_match, &r.intent.verb),
                        );
                        let mut response = self.execute_runbook(session).await?;
                        response.dsl_source = None;
                        response.message =
                            Self::with_pivot_feedback(&pivot_feedback, response.message);
                        response.sage_explain = sage_explain_payload.clone();
                        response.coder_proposal = Self::to_coder_proposal_payload(
                            None,
                            Some(&r.dsl),
                            Some(&r.intent.verb),
                            false,
                        );
                        if let Some(last) = session.messages.last_mut() {
                            last.sage_explain = response.sage_explain.clone();
                            last.coder_proposal = response.coder_proposal.clone();
                        }
                        return Ok(response);
                    }

                    // Stage in runbook (SINGLE LOOP - all DSL goes through here)
                    let ast = parse_program(&r.dsl)
                        .map(|p| p.statements)
                        .unwrap_or_default();

                    // Build provenance labels from journey metadata (Tier -2 match)
                    let labels = Self::build_journey_labels(&journey_match, &r.intent.verb);
                    session.set_pending_dsl_with_labels(r.dsl.clone(), ast, None, false, labels);

                    // Check if this is a session/view verb (navigation)
                    let verb = &r.intent.verb;
                    let is_navigation = Self::is_navigation_verb(verb);

                    if is_navigation {
                        // Auto-trigger run for navigation verbs (goes through runbook)
                        tracing::debug!(verb = %verb, dsl = %r.dsl, "Auto-running navigation verb");
                        let mut response = self.execute_runbook(session).await?;
                        response.sage_explain = sage_explain_payload.clone();
                        response.coder_proposal = Self::to_coder_proposal_payload(
                            None,
                            Some(&r.dsl),
                            Some(verb),
                            false,
                        );
                        if let Some(last) = session.messages.last_mut() {
                            last.sage_explain = response.sage_explain.clone();
                            last.coder_proposal = response.coder_proposal.clone();
                        }
                        return Ok(response);
                    }

                    // Data mutation - wait for user to say "run"
                    // Enrich message with journey context when Tier -2 matched
                    let msg = if let Some(ref jm) = journey_match {
                        let title = jm.scenario_title.as_deref().unwrap_or(&r.intent.verb);
                        format!(
                            "**{}**\n\n```\n{}\n```\n\nSay 'run' to execute.",
                            title, r.dsl
                        )
                    } else {
                        format!("Staged: {}\n\nSay 'run' to execute.", r.dsl)
                    };
                    let msg = Self::with_pivot_feedback(&pivot_feedback, msg);
                    let coder_proposal = Self::to_coder_proposal_payload(
                        None,
                        Some(&r.dsl),
                        Some(&r.intent.verb),
                        true,
                    );
                    Self::add_agent_message_with_payloads(
                        session,
                        msg.clone(),
                        Some(r.dsl.clone()),
                        sage_explain_payload.clone(),
                        coder_proposal.clone(),
                    );
                    let mut response = self.staged_response(r.dsl, msg);
                    response.sage_explain = sage_explain_payload.clone();
                    response.coder_proposal = coder_proposal;
                    return Ok(response);
                }

                // Journey-level disambiguation (e.g., macro_selector needs jurisdiction pick)
                // Takes priority over generic verb disambiguation since the journey is already
                // matched — we just need a parameter to resolve the specific macro.
                if matches!(r.outcome, PipelineOutcome::NeedsClarification) {
                    if let Some(jd) = journey_decision {
                        let msg = Self::with_pivot_feedback(&pivot_feedback, jd.prompt.clone());
                        Self::add_agent_message_with_payloads(
                            session,
                            msg.clone(),
                            None,
                            sage_explain_payload.clone(),
                            None,
                        );
                        return Ok(AgentChatResponse {
                            message: msg,
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
                            decision: Some(jd),
                            sage_explain: sage_explain_payload.clone(),
                            coder_proposal: None,
                        });
                    }
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
                        let mut response = self.build_intent_tier_response(
                            &request.message,
                            &r.verb_candidates,
                            &analysis,
                            session,
                        );
                        response.message =
                            Self::with_pivot_feedback(&pivot_feedback, response.message);
                        return Ok(response);
                    }

                    // Otherwise show direct verb disambiguation
                    let mut response = self.build_verb_disambiguation_response(
                        &request.message,
                        &r.verb_candidates,
                        session,
                    );
                    response.message =
                        Self::with_pivot_feedback(&pivot_feedback, response.message);
                    return Ok(response);
                }

                // Pipeline gave an error message? Return it
                if let Some(err) = r.validation_error {
                    let err = Self::with_pivot_feedback(&pivot_feedback, err);
                    return Ok(self.fail(&err, session));
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Pipeline error");
            }
        }

        // Fallback
        let msg = Self::with_pivot_feedback(
            &pivot_feedback,
            "I don't understand. Try /commands for help.",
        );
        Ok(self.fail(&msg, session))
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

    fn is_read_only_pivot_request(input: &str) -> bool {
        let normalized = input.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return false;
        }

        normalized.starts_with("what ")
            || normalized.starts_with("show ")
            || normalized.starts_with("show me ")
            || normalized.starts_with("list ")
            || normalized.starts_with("read ")
            || normalized.starts_with("get ")
            || normalized.starts_with("describe ")
            || normalized.starts_with("which ")
            || normalized.starts_with("who ")
            || normalized.starts_with("where ")
            || normalized.starts_with("how many ")
            || normalized.starts_with("status")
            || normalized.starts_with("view ")
    }

    fn is_write_request(input: &str) -> bool {
        let normalized = input.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return false;
        }

        normalized.starts_with("create ")
            || normalized.starts_with("add ")
            || normalized.starts_with("update ")
            || normalized.starts_with("change ")
            || normalized.starts_with("delete ")
            || normalized.starts_with("remove ")
            || normalized.starts_with("assign ")
            || normalized.starts_with("set ")
            || normalized.starts_with("run ")
            || normalized.starts_with("execute ")
            || normalized.starts_with("publish ")
    }

    fn relationship_relevant(input: &str) -> bool {
        let normalized = input.trim().to_ascii_lowercase();
        [
            "relationship",
            "relationships",
            "ownership",
            "owner",
            "owners",
            "who owns",
            "who controls",
            "graph",
            "party",
            "parties",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
    }

    fn render_selected_verb_dsl(
        selected: &crate::semtaxonomy_v2::SelectedVerb,
    ) -> Option<String> {
        if selected.verb_id.is_empty() {
            return None;
        }
        let mut parts = vec![format!("({}", selected.verb_id)];
        if let serde_json::Value::Object(args) = &selected.args {
            let mut keys = args.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(value) = args.get(&key) {
                    parts.push(format!(" :{} {}", key, Self::render_selected_arg_value(value)));
                }
            }
        }
        parts.push(")".to_string());
        Some(parts.join(""))
    }

    fn render_selected_arg_value(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(value) => Self::quote_dsl_string(value),
            serde_json::Value::Bool(value) => value.to_string(),
            serde_json::Value::Number(value) => value.to_string(),
            serde_json::Value::Array(items) => {
                let rendered = items
                    .iter()
                    .map(Self::render_selected_arg_value)
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("(list {})", rendered)
            }
            serde_json::Value::Null => "nil".to_string(),
            other => Self::quote_dsl_string(&other.to_string()),
        }
    }

    fn quote_dsl_string(value: &str) -> String {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }

    fn should_reclassify_before_pending(input: &str) -> bool {
        let normalized = input.trim().to_ascii_lowercase();
        if normalized.is_empty()
            || normalized == "new"
            || normalized == "skip"
            || normalized.parse::<usize>().is_ok()
        {
            return false;
        }

        Self::is_read_only_pivot_request(&normalized) || Self::is_write_request(&normalized)
    }

    fn with_pivot_feedback(pivot_feedback: &Option<String>, message: impl Into<String>) -> String {
        let message = message.into();
        match pivot_feedback {
            Some(note) => format!("{note}\n\n{message}"),
            None => message,
        }
    }

    /// Build provenance labels from journey metadata (Tier -2 match).
    ///
    /// Returns an empty map when no journey match is present, so the caller
    /// can unconditionally pass labels to `set_pending_dsl_with_labels`.
    fn build_journey_labels(
        journey_match: &Option<crate::mcp::verb_search::JourneyMetadata>,
        verb_fqn: &str,
    ) -> std::collections::HashMap<String, String> {
        use crate::mcp::verb_search::JourneyRoute;

        let jm = match journey_match {
            Some(jm) => jm,
            None => return std::collections::HashMap::new(),
        };

        let mut labels = std::collections::HashMap::new();

        // origin_kind: "scenario" if scenario-triggered, "macro" otherwise
        let kind = if jm.scenario_id.is_some() {
            "scenario"
        } else {
            "macro"
        };
        labels.insert("origin_kind".to_string(), kind.to_string());

        // origin_macro_fqn: the primary macro FQN from the route
        let macro_fqn = match &jm.route {
            JourneyRoute::Macro { macro_fqn } => macro_fqn.clone(),
            JourneyRoute::MacroSequence { macros } => macros
                .first()
                .cloned()
                .unwrap_or_else(|| verb_fqn.to_string()),
            JourneyRoute::NeedsSelection { .. } => verb_fqn.to_string(),
        };
        labels.insert("origin_macro_fqn".to_string(), macro_fqn);

        // origin_scenario_id (only for Tier -2A scenario matches)
        if let Some(ref sid) = jm.scenario_id {
            labels.insert("origin_scenario_id".to_string(), sid.clone());
        }

        // origin_title (for progress narration)
        if let Some(ref title) = jm.scenario_title {
            labels.insert("origin_title".to_string(), title.clone());
        }

        labels
    }

    /// Build an execution result message enriched with journey narration.
    ///
    /// When the run sheet entries carry `origin_title` labels (from Tier -2
    /// journey matches), the message includes the journey title:
    ///   "Lux UCITS SICAV Setup — Executed 13 statement(s). 50 CBUs in scope."
    ///
    /// Falls back to the plain "Executed N statement(s)." format otherwise.
    fn narrate_execution(
        run_sheet: &crate::session::unified::RunSheet,
        executed_count: usize,
        cbu_count: usize,
    ) -> String {
        // Look for a journey title on the most recent executed entry
        let title = run_sheet
            .entries
            .iter()
            .rev()
            .find_map(|e| e.labels.get("origin_title"));

        match title {
            Some(t) => format!(
                "**{}** — Executed {} statement(s). {} CBUs in scope.",
                t, executed_count, cbu_count
            ),
            None => format!(
                "Executed {} statement(s). {} CBUs in scope.",
                executed_count, cbu_count
            ),
        }
    }

    /// Check session context and prompt for client group or deal if needed
    ///
    /// Returns Some(response) if context needs to be set, None to continue processing
    async fn check_session_context(
        &self,
        session: &mut UnifiedSession,
        message: Option<&str>,
    ) -> Option<AgentChatResponse> {
        use crate::database::DealRepository;
        use ob_poc_types::{
            ClarificationPayload, DealClarificationPayload, DealOption, DecisionKind,
            DecisionPacket, DecisionTrace, SessionStateView, UserChoice,
        };

        // Semantic OS workflow sessions are registry-scoped and should not be
        // blocked on client/deal context collection.
        if Self::skips_client_scope_gate(session.context.stage_focus.as_deref()) {
            return None;
        }

        // Skip context check if session already has deal context or gate was skipped
        if session.context.deal_id.is_some() || session.context.deal_gate_skipped {
            return None;
        }

        // Check if client group is set - if not, prompt for it first
        let client_group_id = match session.context.client_group_id() {
            Some(id) => id,
            None => {
                // No client group - prompt user to select one
                return self.prompt_for_client_group(session).await;
            }
        };

        let client_group_name = session
            .context
            .client_group_name()
            .unwrap_or("Unknown")
            .to_string();

        if let Some(message) = message {
            if !Self::utterance_requires_deal_context(message) {
                return None;
            }
        } else {
            return None;
        }

        // Client group is set but no deal - check for existing deals
        let deals =
            match DealRepository::get_deals_for_client_group(&self.pool, client_group_id).await {
                Ok(deals) => deals,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to fetch deals for client group");
                    return None; // Continue without deal context
                }
            };

        // Build deal options
        let deal_options: Vec<DealOption> = deals
            .iter()
            .map(|d| DealOption {
                deal_id: d.deal_id.to_string(),
                deal_name: d.deal_name.clone(),
                deal_status: d.deal_status.clone(),
                product_count: d.product_count,
                summary: Some(format!(
                    "{} products, {}",
                    d.product_count,
                    d.deal_status.to_lowercase()
                )),
            })
            .collect();

        // Build choices for UI
        let mut choices: Vec<UserChoice> = deal_options
            .iter()
            .enumerate()
            .map(|(i, d)| UserChoice {
                id: format!("{}", i + 1),
                label: d.deal_name.clone(),
                description: d.summary.clone().unwrap_or_default(),
                is_escape: false,
            })
            .collect();

        // Add "Create new deal" option
        choices.push(UserChoice {
            id: "NEW".to_string(),
            label: "Create new deal".to_string(),
            description: format!("Start a new deal for {}", client_group_name),
            is_escape: true,
        });

        // Add "Skip" option to work without deal context
        choices.push(UserChoice {
            id: "SKIP".to_string(),
            label: "Skip for now".to_string(),
            description: "Continue without deal context".to_string(),
            is_escape: true,
        });

        let prompt = if deals.is_empty() {
            format!(
                "No deals found for {}. Would you like to create one?",
                client_group_name
            )
        } else {
            format!(
                "Found {} deal(s) for {}. Which one would you like to work on?",
                deals.len(),
                client_group_name
            )
        };

        let payload = DealClarificationPayload {
            client_group_id: client_group_id.to_string(),
            client_group_name: client_group_name.clone(),
            deals: deal_options,
            can_create: true,
        };

        let packet = DecisionPacket {
            packet_id: uuid::Uuid::new_v4().to_string(),
            kind: DecisionKind::ClarifyDeal,
            session: SessionStateView {
                session_id: Some(session.id),
                client_group_anchor: Some(client_group_id.to_string()),
                client_group_name: Some(client_group_name.clone()),
                persona: None,
                last_confirmed_verb: None,
            },
            utterance: String::new(),
            payload: ClarificationPayload::Deal(payload),
            prompt: prompt.clone(),
            choices,
            best_plan: None,
            alternatives: vec![],
            requires_confirm: false,
            confirm_token: None,
            trace: DecisionTrace {
                config_version: "1.0".to_string(),
                entity_snapshot_hash: None,
                lexicon_snapshot_hash: None,
                semantic_lane_enabled: false,
                embedding_model_id: None,
                verb_margin: 0.0,
                scope_margin: 0.0,
                kind_margin: 0.0,
                decision_reason: "session_context_check".to_string(),
            },
        };

        // Store pending decision in session
        session.pending_decision = Some(packet.clone());

        let message = prompt;
        session.add_agent_message(message.clone(), None, None);

        Some(AgentChatResponse {
            message,

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
            decision: Some(packet),
            sage_explain: None,
            coder_proposal: None,
        })
    }

    /// Prompt user to select a client group at session start
    ///
    /// This is the first step in the context flow:
    /// Client Group → Deal → CBU/Entity
    async fn prompt_for_client_group(
        &self,
        session: &mut UnifiedSession,
    ) -> Option<AgentChatResponse> {
        use crate::database::DealRepository;
        use ob_poc_types::{
            ClarificationPayload, DecisionKind, DecisionPacket, DecisionTrace,
            GroupClarificationPayload, GroupOption, SessionStateView, UserChoice,
        };

        // Fetch all client groups
        let client_groups = match DealRepository::get_all_client_groups(&self.pool).await {
            Ok(groups) => groups,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch client groups");
                return None; // Continue without context
            }
        };

        if client_groups.is_empty() {
            tracing::info!("No client groups found in database");
            return None;
        }

        // Build group options for UI
        let group_options: Vec<GroupOption> = client_groups
            .iter()
            .map(|g| GroupOption {
                id: g.id.to_string(),
                alias: g.canonical_name.clone(),
                score: 1.0,
                method: "list".to_string(),
            })
            .collect();

        // Build choices for UI
        let choices: Vec<UserChoice> = client_groups
            .iter()
            .enumerate()
            .map(|(i, g)| UserChoice {
                id: format!("{}", i + 1),
                label: g.canonical_name.clone(),
                description: format!("{} active deal(s)", g.deal_count),
                is_escape: false,
            })
            .collect();

        let prompt = "Welcome! Which client would you like to work with today?".to_string();

        let payload = GroupClarificationPayload {
            options: group_options,
        };

        let packet = DecisionPacket {
            packet_id: uuid::Uuid::new_v4().to_string(),
            kind: DecisionKind::ClarifyGroup,
            session: SessionStateView {
                session_id: Some(session.id),
                client_group_anchor: None,
                client_group_name: None,
                persona: None,
                last_confirmed_verb: None,
            },
            utterance: String::new(),
            payload: ClarificationPayload::Group(payload),
            prompt: prompt.clone(),
            choices,
            best_plan: None,
            alternatives: vec![],
            requires_confirm: false,
            confirm_token: None,
            trace: DecisionTrace {
                config_version: "1.0".to_string(),
                entity_snapshot_hash: None,
                lexicon_snapshot_hash: None,
                semantic_lane_enabled: false,
                embedding_model_id: None,
                verb_margin: 0.0,
                scope_margin: 0.0,
                kind_margin: 0.0,
                decision_reason: "session_start_client_group".to_string(),
            },
        };

        // Store pending decision in session
        session.pending_decision = Some(packet.clone());

        session.add_agent_message(prompt.clone(), None, None);

        Some(AgentChatResponse {
            message: prompt,

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
            decision: Some(packet),
            sage_explain: None,
            coder_proposal: None,
        })
    }

    /// Handle a decision selection (client group or deal selection from pending_decision)
    async fn handle_decision_selection(
        &self,
        session: &mut UnifiedSession,
        packet: &ob_poc_types::DecisionPacket,
        choice: &ob_poc_types::UserChoice,
    ) -> Result<AgentChatResponse, String> {
        use ob_poc_types::{ClarificationPayload, DecisionKind};

        let message = match &packet.kind {
            DecisionKind::Proposal => format!("Selected: {}", choice.label),
            DecisionKind::ClarifyGroup => {
                // Handle client group selection
                if let ClarificationPayload::Group(group_payload) = &packet.payload {
                    // Find the selected group by index
                    if let Ok(idx) = choice.id.parse::<usize>() {
                        if let Some(group) = group_payload.options.get(idx.saturating_sub(1)) {
                            // Set client group context in session
                            if let Ok(group_uuid) = uuid::Uuid::parse_str(&group.id) {
                                let scope = crate::mcp::scope_resolution::ScopeContext::new()
                                    .with_client_group(group_uuid, group.alias.clone());
                                session.context.set_client_scope(scope);
                                format!("Now working with client: {}. How can I help you today?", group.alias)
                            } else {
                                "Invalid group ID".to_string()
                            }
                        } else {
                            format!("Selected client: {}", choice.label)
                        }
                    } else {
                        format!("Selected client: {}", choice.label)
                    }
                } else {
                    format!("Selected client: {}", choice.label)
                }
            }
            DecisionKind::ClarifyDeal => {
                // Handle deal selection
                if choice.id == "NEW" {
                    "Let's create a new deal. What would you like to name it?".to_string()
                } else if choice.id == "SKIP" {
                    session.context.deal_id = None;
                    session.context.deal_name = None;
                    session.context.deal_gate_skipped = true;
                    "Continuing without deal context. You can set one later with 'load deal'."
                        .to_string()
                } else {
                    // User selected an existing deal
                    if let ClarificationPayload::Deal(deal_payload) = &packet.payload {
                        if let Ok(idx) = choice.id.parse::<usize>() {
                            if let Some(deal) = deal_payload.deals.get(idx.saturating_sub(1)) {
                                if let Ok(deal_uuid) = uuid::Uuid::parse_str(&deal.deal_id) {
                                    session.context.deal_id = Some(deal_uuid);
                                    session.context.deal_name = Some(deal.deal_name.clone());
                                    format!(
                                        "Now working on deal: {}. How can I help you today?",
                                        deal.deal_name
                                    )
                                } else {
                                    "Invalid deal ID".to_string()
                                }
                            } else {
                                format!("Selected deal: {}", choice.label)
                            }
                        } else {
                            format!("Selected deal: {}", choice.label)
                        }
                    } else {
                        format!("Selected deal: {}", choice.label)
                    }
                }
            }
            DecisionKind::ClarifyScope => {
                let is_semos = packet.trace.decision_reason == "semos_workflow_selection";
                if is_semos {
                    let stage_focus = match choice.id.as_str() {
                        "1" => "semos-onboarding",
                        "2" => "semos-kyc",
                        "3" => "semos-data-management",
                        "4" => "semos-stewardship",
                        _ => "semos-data-management",
                    };
                    session.context.stage_focus = Some(stage_focus.to_string());
                    format!(
                        "Great, let's work on {}. I'll scope to that workflow.",
                        choice.label
                    )
                } else {
                    format!("Selected scope: {}", choice.label)
                }
            }
            DecisionKind::ClarifyVerb => format!("Selected verb: {}", choice.label),
            DecisionKind::ClarifyEntity => format!("Selected entity: {}", choice.label),
            DecisionKind::Refuse => format!("Selected: {}", choice.label),
        };

        session.add_agent_message(message.clone(), None, None);
        Ok(AgentChatResponse {
            message,

            session_state: SessionState::Scoped,
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
            decision: None,
            sage_explain: None,
            coder_proposal: None,
        })
    }

    /// Public entry point for forced verb selection via decision reply.
    ///
    /// Called by `handle_decision_reply` when `ClarifyVerb` selection is made.
    /// Delegates to `handle_verb_selection` which routes through orchestrator.
    pub async fn process_forced_verb_selection(
        &self,
        session: &mut crate::session::UnifiedSession,
        original_utterance: &str,
        forced_verb_fqn: &str,
        actor: crate::sem_reg::abac::ActorContext,
    ) -> Result<AgentChatResponse, String> {
        self.handle_verb_selection(
            session,
            original_utterance,
            forced_verb_fqn,
            &[], // No candidates list needed for forced selection
            actor,
        )
        .await
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
        actor: crate::sem_reg::abac::ActorContext,
    ) -> Result<AgentChatResponse, String> {
        use crate::dsl_v2::parse_program;

        // Record learning signal (gold-standard training data)
        // Convert candidates to verb strings for the recording function
        let candidate_verbs: Vec<String> = all_candidates.iter().map(|c| c.verb.clone()).collect();
        if let Err(e) = crate::api::agent_learning_routes::record_verb_selection_signal(
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

        // Binding disambiguation: use forced-verb to ensure user's selection is honoured
        let orch_ctx = self.build_orchestrator_context(
            session,
            actor,
            crate::agent::orchestrator::UtteranceSource::Chat,
        );
        let orch_outcome = crate::agent::orchestrator::handle_utterance_with_forced_verb(
            &orch_ctx,
            original_input,
            selected_verb,
        )
        .await;
        let result = orch_outcome.map(|o| o.pipeline_result);

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
                    return Ok(self.staged_response(r.dsl, msg));
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
        // Check if there's anything to run
        if !session.run_sheet.has_runnable() {
            return Ok(self.fail("Nothing staged to run. Send a command first.", session));
        }

        // Get all pending DSL
        let dsl = match session.run_sheet.runnable_dsl() {
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

        // 6. Execute — gated path (INV-1, INV-11)
        //
        // When `runbook-gate-vnext` is enabled, ALL execution goes through
        // compile_invocation() + execute_runbook(). When disabled, the legacy
        // DslExecutor::execute_dsl() path is used.
        self.execute_resolved_dsl(session, resolved_dsl, program)
            .await
    }

    /// Legacy execution path — bypasses runbook compilation gate.
    ///
    /// Retained under `#[cfg(not(feature = "runbook-gate-vnext"))]` as fallback.
    /// When `runbook-gate-vnext` is enabled, this is dead code and `execute_via_runbook_gate`
    /// is used instead.
    #[cfg(not(feature = "runbook-gate-vnext"))]
    async fn execute_resolved_dsl(
        &self,
        session: &mut UnifiedSession,
        resolved_dsl: String,
        _program: crate::dsl_v2::ast::Program,
    ) -> Result<AgentChatResponse, String> {
        use crate::dsl_v2::{DslExecutor, ExecutionContext};

        let executor = DslExecutor::new(self.pool.clone());
        let mut exec_ctx = ExecutionContext::new();
        match executor.execute_dsl(&resolved_dsl, &mut exec_ctx).await {
            Ok(results) => {
                // Check if any result is a macro that returned combined_dsl to stage
                for result in &results {
                    if let crate::dsl_v2::ExecutionResult::Record(json) = result {
                        if let Some(combined_dsl) =
                            json.get("combined_dsl").and_then(|v| v.as_str())
                        {
                            if !combined_dsl.is_empty() {
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
                                    can_execute: true,
                                    session_state: SessionState::ReadyToExecute,

                                    ast: None,
                                    disambiguation: None,
                                    commands: None,
                                    unresolved_refs: None,
                                    current_ref_index: None,
                                    dsl_hash: None,
                                    verb_disambiguation: None,
                                    intent_tier: None,
                                    decision: None,
                                    sage_explain: None,
                                    coder_proposal: None,
                                });
                            }
                        }
                    }
                }

                // Normal execution - mark as executed
                session.run_sheet.mark_all_executed();
                self.sync_scope_from_exec_ctx(session, &mut exec_ctx);

                let msg = Self::narrate_execution(
                    &session.run_sheet,
                    results.len(),
                    session.context.cbu_ids.len(),
                );
                session.add_agent_message(msg.clone(), None, None);
                Ok(AgentChatResponse {
                    message: msg,
                    dsl_source: Some(resolved_dsl),
                    can_execute: false,
                    session_state: SessionState::Executed,

                    ast: None,
                    disambiguation: None,
                    commands: None,
                    unresolved_refs: None,
                    current_ref_index: None,
                    dsl_hash: None,
                    verb_disambiguation: None,
                    intent_tier: None,
                    decision: None,
                    sage_explain: None,
                    coder_proposal: None,
                })
            }
            Err(e) => {
                let msg = format!("Execution failed: {}", e);
                Ok(self.fail(&msg, session))
            }
        }
    }

    /// Runbook-gated execution path (INV-1, INV-11).
    ///
    /// Routes all Chat API execution through `compile_invocation()` + `execute_runbook()`.
    /// No raw DSL execution — the only executable truth is the compiled runbook.
    ///
    /// INV-1: Every verb call is wrapped in a `CompiledRunbook` and executed
    /// through `execute_runbook()`. No raw `DslExecutor::execute_dsl()` call.
    #[cfg(feature = "runbook-gate-vnext")]
    async fn execute_resolved_dsl(
        &self,
        session: &mut UnifiedSession,
        resolved_dsl: String,
        program: crate::dsl_v2::ast::Program,
    ) -> Result<AgentChatResponse, String> {
        use crate::repl::executor_bridge::RealDslExecutor;
        use crate::runbook::executor::RunbookStoreBackend;
        use crate::runbook::step_executor_bridge::DslStepExecutor;
        use crate::runbook::{
            envelope::ReplayEnvelope,
            execute_runbook,
            types::{CompiledStep, ExecutionMode},
            write_set::derive_write_set_heuristic,
            CompiledRunbook,
        };

        // Phase D: use Postgres store when pool available for event emission.
        #[cfg(feature = "database")]
        let pg_store = crate::runbook::executor::PostgresRunbookStore::new(self.pool.clone());
        #[cfg(feature = "database")]
        let store: &dyn RunbookStoreBackend = &pg_store;

        #[cfg(not(feature = "database"))]
        let mem_store = RunbookStore::new();
        #[cfg(not(feature = "database"))]
        let store: &dyn RunbookStoreBackend = &mem_store;
        let session_id = session.id;
        let mut executed_count = 0usize;

        for stmt in &program.statements {
            if let crate::dsl_v2::ast::Statement::VerbCall(vc) = stmt {
                let verb_fqn = vc.full_name();
                let args: std::collections::BTreeMap<String, String> = vc
                    .arguments
                    .iter()
                    .map(|a| (a.key.clone(), a.value.to_dsl_string()))
                    .collect();
                let dsl_source = vc.to_dsl_string();

                // Derive write_set from args (heuristic UUID extraction)
                let write_set: Vec<uuid::Uuid> =
                    derive_write_set_heuristic(&args).into_iter().collect();

                let step = CompiledStep {
                    step_id: uuid::Uuid::new_v4(),
                    sentence: dsl_source.clone(),
                    verb: verb_fqn.clone(),
                    dsl: dsl_source,
                    args: args.clone(),
                    depends_on: vec![],
                    execution_mode: ExecutionMode::Sync,
                    write_set: write_set.clone(),
                    verb_contract_snapshot_id: None,
                };

                let envelope = ReplayEnvelope {
                    core: crate::runbook::envelope::EnvelopeCore {
                        session_cursor: 0,
                        entity_bindings: std::collections::BTreeMap::new(),
                        external_lookup_digests: vec![],
                        macro_audit_digests: vec![],
                        snapshot_manifest: std::collections::HashMap::new(),
                    },
                    external_lookups: vec![],
                    macro_audits: vec![],
                    sealed_at: chrono::Utc::now(),
                };

                let runbook_version = session.messages.len() as u64
                    + session.run_sheet.entries.len() as u64
                    + 1;
                let runbook = CompiledRunbook::new(
                    session_id,
                    runbook_version,
                    vec![step],
                    envelope,
                );
                let runbook_id = runbook.id;
                if let Err(e) = store.insert(&runbook).await {
                    let msg = format!("Failed to store compiled runbook: {}", e);
                    return Ok(self.fail(&msg, session));
                }

                // Execute through the gate (INV-1)
                let real_executor = RealDslExecutor::new(self.pool.clone());
                let step_executor = DslStepExecutor::new(std::sync::Arc::new(real_executor));
                match execute_runbook(store, runbook_id, None, &step_executor).await {
                    Ok(_result) => {
                        executed_count += 1;
                    }
                    Err(e) => {
                        let msg = format!("Runbook execution failed: {}", e);
                        return Ok(self.fail(&msg, session));
                    }
                }
            }
        }

        // Mark as executed
        session.run_sheet.mark_all_executed();

        let msg = Self::narrate_execution(
            &session.run_sheet,
            executed_count,
            session.context.cbu_ids.len(),
        );
        session.add_agent_message(msg.clone(), None, None);
        Ok(AgentChatResponse {
            message: msg,
            dsl_source: Some(resolved_dsl),
            can_execute: false,
            session_state: SessionState::Executed,

            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            decision: None,
            sage_explain: None,
            coder_proposal: None,
        })
    }

    /// Sync scope from execution context into session (legacy path only).
    #[cfg(not(feature = "runbook-gate-vnext"))]
    fn sync_scope_from_exec_ctx(
        &self,
        session: &mut UnifiedSession,
        exec_ctx: &mut crate::dsl_v2::ExecutionContext,
    ) {
        if let Some(unified_session) = exec_ctx.take_pending_session() {
            let loaded = unified_session.cbu_ids_vec();
            let cbu_count = loaded.len();

            for cbu_id in &loaded {
                if !session.context.cbu_ids.contains(cbu_id) {
                    session.context.cbu_ids.push(*cbu_id);
                }
            }

            let scope_def = if cbu_count == 1 {
                GraphScope::SingleCbu {
                    cbu_id: loaded[0],
                    cbu_name: unified_session.name.clone().unwrap_or_default(),
                }
            } else if cbu_count > 1 {
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

    fn staged_response(&self, dsl: String, msg: String) -> AgentChatResponse {
        AgentChatResponse {
            message: msg,
            dsl_source: Some(dsl),
            can_execute: true,
            session_state: SessionState::ReadyToExecute,

            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            decision: None,
            sage_explain: None,
            coder_proposal: None,
        }
    }

    /// Fail: return message to user
    fn fail(&self, msg: &str, session: &mut UnifiedSession) -> AgentChatResponse {
        session.add_agent_message(msg.to_string(), None, None);
        AgentChatResponse {
            message: msg.to_string(),

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
            decision: None,
            sage_explain: None,
            coder_proposal: None,
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

        let request_id = Uuid::new_v4().to_string();

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
            decision: None,
            sage_explain: None,
            coder_proposal: None,
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
            decision: None,
            sage_explain: None,
            coder_proposal: None,
        }
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

    // ========================================================================
    // Public Entity Resolution API
    // ========================================================================

    /// Resolve a single entity by exact name match
    ///
    /// Returns the entity if exactly one match is found,
    /// or a list of suggestions if multiple/no matches.
    ///
    /// For client_group type, uses PgClientGroupResolver with semantic search.
    async fn resolve_entity(&self, entity_type: &str, name: &str) -> Result<ResolveResult, String> {
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
}

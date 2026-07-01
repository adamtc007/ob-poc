//! Agent Service - Centralized agent conversation logic
//!
//! This module provides a single service for all agent chat operations.
//! It implements a **deterministic pipeline** that constrains LLM output
//! to valid, executable DSL.
//!
//! ## Legacy `execute_runbook` gate (INV-11)
//!
//! Earlier revisions defined an `execute_runbook` method on `AgentService` that
//! gated the Chat API through the compiled-runbook pipeline. After convergence
//! onto `ReplOrchestratorV2` (`rust/src/sequencer.rs`), the methods along that
//! path went unused and were removed. If a non-orchestrator path is ever
//! reactivated here, it MUST route through `execute_runbook` in the canonical
//! gate (see invariants `runbook::invariant_tests::test_execution_gate_source_invariants`
//! and `runbook::executor::tests::test_runbook_gate_chat_and_repl`).
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

use crate::api::session::DisambiguationRequest;
use crate::dsl_v2::gateway_resolver::gateway_addr;
use crate::dsl_v2::macros::{load_macro_registry_from_dir, MacroRegistry};
use crate::dsl_v2::Statement;
use crate::mcp::macro_index::MacroIndex;
use crate::mcp::scenario_index::ScenarioIndex;
use crate::mcp::verb_search_factory::VerbSearcherFactory;
use crate::sage::SageEngine;
use crate::session::{SessionState, UnresolvedRefInfo};
// Phase2Service: removed with process_chat (TOCTOU recheck in REPL orchestrator)
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

    #[test]
    fn semos_calibration_focus_emits_no_goals() {
        assert!(AgentService::stage_focus_goals(Some("semos-calibration")).is_empty());
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
    /// Typed Drafter/REPL proposal payload for UI rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drafter_proposal: Option<ob_poc_types::chat::DraftProposalPayload>,
    /// Typed Sem OS discovery/bootstrap payload for onboarding-stage sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_bootstrap: Option<ob_poc_types::chat::DiscoveryBootstrapPayload>,
    /// Typed parked-runbook payload for long-running or gated execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parked_entries: Option<Vec<ob_poc_types::chat::ParkedEntryPayload>>,
    /// Onboarding state view — "where am I + what can I do" contextual verb picker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarding_state: Option<ob_poc_types::onboarding_state::OnboardingStateView>,
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
    policy_gate: Arc<ob_poc_boundary::policy::PolicyGate>,
    /// Semantic OS client — when set, routes sem_reg calls through DI boundary
    sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
    /// MacroIndex for deterministic Tier -2B macro search parity
    macro_index: Option<Arc<MacroIndex>>,
    /// ScenarioIndex for journey-level Tier -2A compound intent resolution
    scenario_index: Option<Arc<ScenarioIndex>>,
    /// Cached MacroRegistry to avoid reloading from disk on every verb search
    macro_registry: Option<Arc<MacroRegistry>>,
    /// Optional Sage engine for Stage 1.5 shadow classification.
    sage_engine: Option<Arc<dyn SageEngine>>,
    /// Canonical SemOS plugin op registry. Threaded into every inner
    /// `DslExecutor` / `RealDslExecutor` constructed by this service so
    /// plugin verbs dispatch correctly post-Phase-5c-migrate slice #80.
    sem_os_ops: Option<Arc<sem_os_postgres::ops::SemOsVerbOpRegistry>>,
    /// Platform service registry. Threaded into every inner executor this
    /// service constructs so ops that consume platform traits via
    /// `VerbExecutionContext::service::<dyn T>()` resolve correctly.
    service_registry: Option<Arc<dsl_runtime::ServiceRegistry>>,
}

impl AgentService {
    /// Map Semantic OS stage focus to Sem OS phase-tag goals.
    ///
    /// `semos-data-management` is intentionally expanded to include:
    /// - `data` (registry/data stewardship verbs)
    /// - `deal` (commercial data records)
    /// - `onboarding` (CBU-tagged data records)
    /// - `kyc` (document-tagged records)
    /// - `navigation` (session/view navigation verbs)
    fn stage_focus_goals(stage_focus: Option<&str>) -> Vec<String> {
        match stage_focus {
            Some("semos-calibration") => vec![],
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
            embedder,
            learned_data,
            lexicon,
            entity_linker: None,
            policy_gate: Arc::new(ob_poc_boundary::policy::PolicyGate::from_env()),
            sem_os_client: None,
            macro_index: None,
            scenario_index: None,
            macro_registry: None,
            sage_engine: None,
            sem_os_ops: None,
            service_registry: None,
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

    /// Install the canonical SemOS plugin op registry. Threaded into every
    /// inner `DslExecutor` / `RealDslExecutor` this service constructs so
    /// plugin verbs dispatch correctly (post-Phase-5c-migrate slice #80).
    pub fn with_sem_os_ops(mut self, ops: Arc<sem_os_postgres::ops::SemOsVerbOpRegistry>) -> Self {
        self.sem_os_ops = Some(ops);
        self
    }

    /// Install the platform service registry. Threaded into every inner
    /// executor this service constructs so ops that consume platform traits
    /// via `VerbExecutionContext::service::<dyn T>()` resolve correctly.
    pub fn with_service_registry(mut self, services: Arc<dsl_runtime::ServiceRegistry>) -> Self {
        self.service_registry = Some(services);
        self
    }

    /// Build the verb searcher with all search indices.
    ///
    /// Uses cached MacroRegistry, MacroIndex, and ScenarioIndex
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

        // Map stage_focus to Sem OS goals for verb phase_tag filtering.
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
            agent_mode: sem_os_types::agent_mode::AgentMode::default(),
            goals,
            stage_focus: session.context.stage_focus.clone(),
            sage_engine: self.sage_engine.clone(),
            pre_sage_entity_kind: Self::current_sage_entity_kind(session),
            pre_sage_entity_name: Self::current_sage_entity_name(session),
            pre_sage_entity_confidence: None,
            recent_sage_intents: session.recent_sage_intents.clone(),
            nlci_compiler: Some(crate::semtaxonomy_v2::build_minimal_cbu_compiler()),
            discovery_selected_domain: session.context.discovery_selected_domain.clone(),
            discovery_selected_family: session.context.discovery_selected_family.clone(),
            discovery_selected_constellation: session
                .context
                .discovery_selected_constellation
                .clone(),
            discovery_answers: session.context.discovery_answers.clone(),
            session_cbu_ids: if session.context.cbu_ids.is_empty() {
                None
            } else {
                Some(session.context.cbu_ids.clone())
            },
        }
    }

    fn current_sage_entity_kind(session: &crate::session::UnifiedSession) -> Option<String> {
        session
            .context
            .active_cbu
            .as_ref()
            .map(|entity| entity.entity_type.clone())
            .or_else(|| {
                session
                    .current_structure
                    .as_ref()
                    .map(|_| "structure".to_string())
            })
            .or_else(|| {
                session
                    .current_case
                    .as_ref()
                    .map(|_| "kyc-case".to_string())
            })
            .or_else(|| {
                session
                    .current_mandate
                    .as_ref()
                    .map(|_| "trading-profile".to_string())
            })
            .or_else(|| session.domain_hint.clone())
            .or_else(|| (!session.entity_type.is_empty()).then(|| session.entity_type.clone()))
    }

    fn current_sage_entity_name(session: &crate::session::UnifiedSession) -> Option<String> {
        session
            .context
            .active_cbu
            .as_ref()
            .map(|entity| entity.display_name.clone())
            .or_else(|| {
                session
                    .current_structure
                    .as_ref()
                    .map(|item| item.display_name.clone())
            })
            .or_else(|| {
                session
                    .current_case
                    .as_ref()
                    .map(|item| item.display_name.clone())
            })
            .or_else(|| {
                session
                    .current_mandate
                    .as_ref()
                    .map(|item| item.display_name.clone())
            })
            .or_else(|| {
                session
                    .client
                    .as_ref()
                    .map(|item| item.display_name.clone())
            })
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
    /// `SemOsContextEnvelope` carries the identical verb set the agent pipeline would use.
    #[cfg(feature = "database")]
    pub async fn resolve_options(
        &self,
        session: &crate::session::UnifiedSession,
        actor: crate::sem_reg::abac::ActorContext,
    ) -> Result<crate::agent::sem_os_context_envelope::SemOsContextEnvelope, String> {
        let ctx = self.build_orchestrator_context(
            session,
            actor,
            crate::agent::orchestrator::UtteranceSource::Chat,
        );
        let envelope = crate::agent::orchestrator::resolve_sem_reg_verbs(
            &ctx,
            "",
            None,
            ctx.pre_sage_entity_kind.as_deref(),
            false,
        )
        .await;
        Ok(envelope)
    }
}

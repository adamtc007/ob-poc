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
// VerbSearchResult/VerbSearchSource: removed with process_chat
use crate::mcp::verb_search_factory::VerbSearcherFactory;
use crate::sage::SageEngine;
#[cfg(not(feature = "runbook-gate-vnext"))]
use crate::session::SessionScope;
use crate::session::{SessionEvent, SessionState, UnifiedSession, UnresolvedRefInfo};
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
    /// Typed Coder/REPL proposal payload for UI rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coder_proposal: Option<ob_poc_types::chat::CoderProposalPayload>,
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

#[cfg(feature = "runbook-gate-vnext")]
fn agent_phase5_recheck_record(
    verb_fqn: &str,
    dsl_source: &str,
    envelope: &crate::agent::sem_os_context_envelope::SemOsContextEnvelope,
) -> serde_json::Value {
    use crate::traceability::Phase2Service;
    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    let status = Phase2Service::runtime_gate_status(&phase2.artifacts, verb_fqn);
    let primary_block = phase2.primary_constellation_block();

    serde_json::json!({
        "verb": verb_fqn,
        "dsl_command": dsl_source,
        "status": status,
        "sem_os_label": phase2.policy_label,
        "allowed_verb_count": phase2.legal_verb_count(),
        "pruned_verb_count": phase2.pruned_verb_count(),
        "fingerprint": phase2.fingerprint(),
        "snapshot_set_id": envelope.snapshot_set_id,
        "blocking_entity": primary_block.as_ref().and_then(|block| block.blocking_entity.clone()),
        "blocking_state": primary_block.as_ref().and_then(|block| block.blocking_state.clone()),
        "blocking_predicate": primary_block.as_ref().map(|block| block.predicate.clone()),
        "resolution_hint": primary_block.as_ref().map(|block| block.resolution_hint.clone()),
    })
}

/// TOCTOU recheck: verify verb is still allowed in current SemOS envelope before execution.
#[cfg(feature = "runbook-gate-vnext")]
fn agent_phase5_recheck_failure(
    verb_fqn: &str,
    envelope: &crate::agent::sem_os_context_envelope::SemOsContextEnvelope,
) -> Option<String> {
    use crate::traceability::Phase2Service;
    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    Phase2Service::runtime_gate_failure(&phase2.artifacts, verb_fqn)
}

#[cfg(feature = "runbook-gate-vnext")]
fn agent_execution_artifact(
    runbook_id: crate::runbook::types::CompiledRunbookId,
    step: &crate::runbook::executor::StepExecutionResult,
    final_status: &crate::runbook::types::CompiledRunbookStatus,
) -> serde_json::Value {
    let (status, result) = match &step.outcome {
        crate::runbook::executor::StepOutcome::Completed { result } => ("completed", Some(result)),
        crate::runbook::executor::StepOutcome::Parked { .. } => ("parked", None),
        crate::runbook::executor::StepOutcome::Failed { .. } => ("failed", None),
        crate::runbook::executor::StepOutcome::Skipped { .. } => ("skipped", None),
    };

    serde_json::json!({
        "runbook_id": runbook_id.to_string(),
        "step_id": step.step_id,
        "verb": step.verb,
        "status": status,
        "final_status": final_status,
        "result": result,
    })
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

#[allow(dead_code)] // Remaining helpers pending further decomposition
impl AgentService {

    fn response_needs_follow_up(response: &AgentChatResponse) -> bool {
        response.decision.is_some()
            || response.verb_disambiguation.is_some()
            || response.intent_tier.is_some()
            || response
                .coder_proposal
                .as_ref()
                .is_some_and(|proposal| proposal.requires_confirmation)
            || response.discovery_bootstrap.is_some()
            || response.can_execute
    }

    fn trace_sage_context(session: &UnifiedSession) -> crate::sage::SageContext {
        crate::sage::SageContext {
            session_id: Some(session.id),
            stage_focus: session.context.stage_focus.clone(),
            goals: Vec::new(),
            entity_kind: (!session.entity_type.is_empty()).then(|| session.entity_type.clone()),
            dominant_entity_name: session
                .context
                .active_cbu
                .as_ref()
                .map(|cbu| cbu.display_name.clone()),
            last_intents: session.recent_sage_intents.clone(),
        }
    }


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
            agent_mode: sem_os_core::authoring::agent_mode::AgentMode::default(),
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

    fn add_agent_message_with_payloads(
        session: &mut UnifiedSession,
        content: String,
        dsl: Option<String>,
        sage_explain: Option<ob_poc_types::chat::SageExplainPayload>,
        coder_proposal: Option<ob_poc_types::chat::CoderProposalPayload>,
        discovery_bootstrap: Option<ob_poc_types::chat::DiscoveryBootstrapPayload>,
        parked_entries: Option<Vec<ob_poc_types::chat::ParkedEntryPayload>>,
    ) {
        session.add_agent_message(content, None, dsl);
        if let Some(last) = session.messages.last_mut() {
            last.sage_explain = sage_explain;
            last.coder_proposal = coder_proposal;
            last.discovery_bootstrap = discovery_bootstrap;
            last.parked_entries = parked_entries;
        }
    }

    #[cfg(feature = "database")]
    fn to_discovery_bootstrap_payload(
        envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
    ) -> Option<ob_poc_types::chat::DiscoveryBootstrapPayload> {
        let surface = envelope?.discovery_surface.as_ref()?;

        Some(ob_poc_types::chat::DiscoveryBootstrapPayload {
            grounding_readiness: match surface.grounding_readiness {
                sem_os_core::context_resolution::GroundingReadiness::NotReady => "not_ready",
                sem_os_core::context_resolution::GroundingReadiness::FamilyReady => "family_ready",
                sem_os_core::context_resolution::GroundingReadiness::ConstellationReady => {
                    "constellation_ready"
                }
                sem_os_core::context_resolution::GroundingReadiness::Grounded => "grounded",
            }
            .to_string(),
            matched_universes: surface
                .matched_universes
                .iter()
                .map(|item| ob_poc_types::chat::DiscoveryUniverseOption {
                    universe_id: item.universe_id.clone(),
                    name: item.name.clone(),
                    score: item.score,
                })
                .collect(),
            matched_domains: surface
                .matched_domains
                .iter()
                .map(|item| ob_poc_types::chat::DiscoveryDomainOption {
                    domain_id: item.domain_id.clone(),
                    label: item.label.clone(),
                    score: item.score,
                })
                .collect(),
            matched_families: surface
                .matched_families
                .iter()
                .map(|item| ob_poc_types::chat::DiscoveryFamilyOption {
                    family_id: item.family_id.clone(),
                    label: item.label.clone(),
                    domain_id: item.domain_id.clone(),
                    score: item.score,
                })
                .collect(),
            matched_constellations: surface
                .matched_constellations
                .iter()
                .map(|item| ob_poc_types::chat::DiscoveryConstellationOption {
                    constellation_id: item.constellation_id.clone(),
                    label: item.label.clone(),
                    score: item.score,
                })
                .collect(),
            missing_inputs: surface
                .missing_inputs
                .iter()
                .map(|input| ob_poc_types::chat::DiscoveryInputPrompt {
                    key: input.key.clone(),
                    label: input.label.clone(),
                    required: input.required,
                    input_type: input.input_type.clone(),
                })
                .collect(),
            entry_questions: surface
                .entry_questions
                .iter()
                .map(|question| ob_poc_types::chat::DiscoveryQuestionPrompt {
                    question_id: question.question_id.clone(),
                    prompt: question.prompt.clone(),
                    maps_to: question.maps_to.clone(),
                    priority: question.priority,
                })
                .collect(),
        })
    }

    #[cfg(not(feature = "database"))]
    fn to_discovery_bootstrap_payload(
        _envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
    ) -> Option<ob_poc_types::chat::DiscoveryBootstrapPayload> {
        None
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
        program: crate::dsl_v2::ast::Program,
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
                                    discovery_bootstrap: None,
                                    parked_entries: None,
                                    onboarding_state: None,
                                });
                            }
                        }
                    }
                }

                // Normal execution - mark as executed
                session.run_sheet.mark_all_executed();
                self.sync_scope_from_exec_ctx(session, &mut exec_ctx);

                // Record positive learning signal (non-vnext path)
                if !results.is_empty() {
                    let original_utterance = session
                        .messages
                        .iter()
                        .rev()
                        .find(|m| m.role == crate::session::unified::MessageRole::User)
                        .map(|m| m.content.clone());

                    if let Some(utterance) = original_utterance {
                        let executed_verbs: Vec<String> = program
                            .statements
                            .iter()
                            .filter_map(|stmt| {
                                if let crate::dsl_v2::ast::Statement::VerbCall(vc) = stmt {
                                    Some(vc.full_name())
                                } else {
                                    None
                                }
                            })
                            .collect();

        // Verb selection signal recording removed — learning routes deleted
        let _ = (&executed_verbs, &utterance);
                    }
                }

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
                    discovery_bootstrap: None,
                    parked_entries: None,
                    onboarding_state: None,
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
        session.pending_execution_rechecks.clear();
        session.pending_execution_artifacts.clear();

        // Fetch envelope ONCE before the execution loop — not per statement.
        // Using a fresh envelope per statement (the old pattern) creates a TOCTOU
        // window where SemOS policy can change mid-batch, causing inconsistent
        // governance within a single DSL execution.
        let recheck_envelope = {
            let actor = crate::policy::ActorResolver::from_env();
            match self.resolve_options(session, actor).await {
                Ok(env) => env,
                Err(_) => {
                    crate::agent::sem_os_context_envelope::SemOsContextEnvelope::unavailable()
                }
            }
        };

        for stmt in &program.statements {
            if let crate::dsl_v2::ast::Statement::VerbCall(vc) = stmt {
                let verb_fqn = vc.full_name();
                let args: std::collections::BTreeMap<String, String> = vc
                    .arguments
                    .iter()
                    .map(|a| (a.key.clone(), a.value.to_dsl_string()))
                    .collect();
                let dsl_source = vc.to_dsl_string();

                // Phase 5 recheck: validate each verb against the SINGLE envelope
                session
                    .pending_execution_rechecks
                    .push(agent_phase5_recheck_record(
                        &verb_fqn,
                        &dsl_source,
                        &recheck_envelope,
                    ));
                if let Some(error) = agent_phase5_recheck_failure(&verb_fqn, &recheck_envelope) {
                    return Ok(self.fail(&error, session));
                }

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
                        snapshot_manifest: std::collections::BTreeMap::new(),
                    },
                    external_lookups: vec![],
                    macro_audits: vec![],
                    sealed_at: chrono::Utc::now(),
                };

                let runbook_version =
                    session.messages.len() as u64 + session.run_sheet.entries.len() as u64 + 1;
                let runbook =
                    CompiledRunbook::new(session_id, runbook_version, vec![step], envelope);
                let runbook_id = runbook.id;
                if let Err(e) = store.insert(&runbook).await {
                    let msg = format!("Failed to store compiled runbook: {}", e);
                    return Ok(self.fail(&msg, session));
                }

                // Execute through the gate (INV-1)
                let real_executor = RealDslExecutor::new(self.pool.clone());
                let step_executor = DslStepExecutor::new(std::sync::Arc::new(real_executor));
                match execute_runbook(store, runbook_id, None, &step_executor).await {
                    Ok(result) => {
                        session
                            .pending_execution_artifacts
                            .extend(result.step_results.iter().map(|step| {
                                agent_execution_artifact(runbook_id, step, &result.final_status)
                            }));
                        let parked_entries = match &result.final_status {
                            crate::runbook::CompiledRunbookStatus::Parked { reason, cursor } => {
                                result
                                    .step_results
                                    .iter()
                                    .find(|step| step.step_id == cursor.step_id)
                                    .map(|step| match reason {
                                        crate::runbook::ParkReason::AwaitingCallback {
                                            correlation_key,
                                        } => {
                                            vec![ob_poc_types::chat::ParkedEntryPayload {
                                                step_id: step.step_id.to_string(),
                                                verb: step.verb.clone(),
                                                park_reason: "awaiting_callback".to_string(),
                                                correlation_key: Some(correlation_key.clone()),
                                                resource: None,
                                                gate_entry_id: None,
                                                message: match &step.outcome {
                                                    crate::runbook::StepOutcome::Parked {
                                                        message,
                                                        ..
                                                    } => Some(message.clone()),
                                                    _ => None,
                                                },
                                            }]
                                        }
                                        crate::runbook::ParkReason::UserPaused => {
                                            vec![ob_poc_types::chat::ParkedEntryPayload {
                                                step_id: step.step_id.to_string(),
                                                verb: step.verb.clone(),
                                                park_reason: "user_paused".to_string(),
                                                correlation_key: None,
                                                resource: None,
                                                gate_entry_id: None,
                                                message: None,
                                            }]
                                        }
                                        crate::runbook::ParkReason::ResourceUnavailable {
                                            resource,
                                        } => {
                                            vec![ob_poc_types::chat::ParkedEntryPayload {
                                                step_id: step.step_id.to_string(),
                                                verb: step.verb.clone(),
                                                park_reason: "resource_unavailable".to_string(),
                                                correlation_key: None,
                                                resource: Some(resource.clone()),
                                                gate_entry_id: None,
                                                message: None,
                                            }]
                                        }
                                        crate::runbook::ParkReason::HumanGate { entry_id } => {
                                            vec![ob_poc_types::chat::ParkedEntryPayload {
                                                step_id: step.step_id.to_string(),
                                                verb: step.verb.clone(),
                                                park_reason: "human_gate".to_string(),
                                                correlation_key: None,
                                                resource: None,
                                                gate_entry_id: Some(entry_id.to_string()),
                                                message: None,
                                            }]
                                        }
                                    })
                            }
                            _ => None,
                        };
                        if let Some(parked_entries) = parked_entries {
                            let msg = if let Some(first) = parked_entries.first() {
                                match first.park_reason.as_str() {
                                    "awaiting_callback" => format!(
                                        "Execution parked while waiting for an external callback for `{}`.",
                                        first.verb
                                    ),
                                    "human_gate" => format!(
                                        "Execution parked and is waiting for human approval for `{}`.",
                                        first.verb
                                    ),
                                    "resource_unavailable" => format!(
                                        "Execution parked because a required resource is unavailable for `{}`.",
                                        first.verb
                                    ),
                                    "user_paused" => format!(
                                        "Execution is paused for `{}`.",
                                        first.verb
                                    ),
                                    _ => format!("Execution parked for `{}`.", first.verb),
                                }
                            } else {
                                "Execution parked.".to_string()
                            };
                            session.add_agent_message(msg.clone(), None, None);
                            if let Some(last) = session.messages.last_mut() {
                                last.parked_entries = Some(parked_entries.clone());
                            }
                            return Ok(AgentChatResponse {
                                message: msg,
                                dsl_source: Some(resolved_dsl),
                                can_execute: false,
                                session_state: SessionState::Executing,
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
                                discovery_bootstrap: None,
                                parked_entries: Some(parked_entries),
                                onboarding_state: None,
                            });
                        }
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

        // Record positive learning signal: utterance → verb → executed successfully.
        // This feeds the promotion pipeline so successful phrases strengthen over time.
        if executed_count > 0 {
            let original_utterance = session
                .messages
                .iter()
                .rev()
                .find(|m| m.role == crate::session::unified::MessageRole::User)
                .map(|m| m.content.clone());

            if let Some(utterance) = original_utterance {
                // Extract verb FQNs from the executed program
                let executed_verbs: Vec<String> = program
                    .statements
                    .iter()
                    .filter_map(|stmt| {
                        if let crate::dsl_v2::ast::Statement::VerbCall(vc) = stmt {
                            Some(vc.full_name())
                        } else {
                            None
                        }
                    })
                    .collect();

                // Verb selection signal recording removed — learning routes deleted
                let _ = (&executed_verbs, &utterance);
                {
                }
            }
        }

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
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
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
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
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
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
        }
    }

    fn fail_closed_session(&self, msg: &str, session: &mut UnifiedSession) -> AgentChatResponse {
        session.transition(SessionEvent::Close);
        session.add_agent_message(msg.to_string(), None, None);
        AgentChatResponse {
            message: msg.to_string(),

            session_state: SessionState::Closed,
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
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
        }
    }

    fn is_fatal_semos_error(msg: &str) -> bool {
        msg.contains("Sem OS is unavailable")
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
        // Load verb metadata for differentiation context
        let registry = crate::dsl_v2::runtime_registry::runtime_registry();

        // Deduplicate candidates: if two verbs have the same description
        // (same operation under different FQNs), keep only the first.
        // This prevents offering "cbu.assign-role" and "cbu.role.assign"
        // as two separate options when they're the same operation.
        let mut seen_descriptions = std::collections::HashSet::new();
        let deduped: Vec<&crate::mcp::verb_search::VerbSearchResult> = candidates
            .iter()
            .filter(|c| {
                let desc = c.description.as_deref().unwrap_or(&c.verb);
                seen_descriptions.insert(desc.to_string())
            })
            .collect();

        let options: Vec<VerbOption> = deduped
            .iter()
            .take(5)
            .map(|c| {
                let description = c
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Execute {}", c.verb));

                // Get domain context from taxonomy
                let location = taxonomy.location_for_verb(&c.verb);

                // Build a suggested utterance the user can say to unambiguously
                // select this verb. The phrase must be specific enough to resolve
                // back through the pipeline to THIS verb and no other.
                //
                // Requirements:
                // - 4+ words (3-word phrases like "show all in" are too generic)
                // - No dots (verb FQN is not an utterance)
                // - Contains at least one domain-specific noun (not just stop words)
                //
                // If the matched phrase is too generic, fall back to the description
                // which is unique per verb by definition.
                let suggested = {
                    let phrase = &c.matched_phrase;
                    let word_count = phrase.split_whitespace().count();
                    let is_specific = word_count >= 4 && !phrase.contains('.') && phrase.len() > 15;
                    let base = if is_specific {
                        phrase.clone()
                    } else {
                        description.clone()
                    };

                    // Interpolate entity name if available — the utterance should
                    // be plain English that resolves through entity linking.
                    // "Open a KYC case" → "Open a KYC case for Allianz Dynamic Commodities"
                    let entity_name: Option<&str> = session
                        .context
                        .client_group_name()
                        .or(session.context.deal_name.as_deref());

                    let enriched = if let Some(name) = entity_name {
                        // Only append if the phrase doesn't already contain the name
                        if !base.to_lowercase().contains(&name.to_lowercase()) {
                            format!("{} for {}", base, name)
                        } else {
                            base
                        }
                    } else {
                        base
                    };
                    Some(enriched)
                };

                // ── Differentiation context ───────────────────────────
                // Determine verb kind from search source + registry behavior
                use crate::mcp::verb_search::VerbSearchSource;

                let parts: Vec<&str> = c.verb.splitn(2, '.').collect();
                let rv = if parts.len() == 2 {
                    registry.get(parts[0], parts[1])
                } else {
                    None
                };

                let verb_kind =
                    match c.source {
                        VerbSearchSource::MacroIndex | VerbSearchSource::ScenarioIndex => "macro",
                        _ => match rv {
                            Some(v) => match &v.behavior {
                                crate::dsl_v2::runtime_registry::RuntimeBehavior::Crud(crud) => {
                                    match crud.operation {
                                        crate::dsl_v2::config::types::CrudOperation::Select => {
                                            "query"
                                        }
                                        _ => "primitive",
                                    }
                                }
                                crate::dsl_v2::runtime_registry::RuntimeBehavior::Plugin(_) => {
                                    if v.produces.is_none() && v.harm_class.as_ref().map(|h| {
                                    matches!(h, crate::dsl_v2::config::types::HarmClass::ReadOnly)
                                }).unwrap_or(false) {
                                    "query"
                                } else {
                                    "primitive"
                                }
                                }
                                _ => "primitive",
                            },
                            None => "primitive",
                        },
                    };

                let step_count: Option<u32> = None; // Populated when macro metadata available

                // Build differentiation text explaining WHY this option
                // differs from the others
                let differentiation = Some(match verb_kind {
                    "macro" => "Multi-step workflow — executes a sequence of operations".into(),
                    "query" => "Read-only — does not change any state".into(),
                    "workflow" => "Template — expands to multiple DSL statements".into(),
                    _ => description.clone(),
                });

                // ── Entity & constellation context ────────────────────
                // Derive what entity type this verb targets and where
                // it sits in the constellation from verb metadata.
                let target_entity_kind =
                    rv.and_then(|v| v.subject_kinds.first().cloned())
                        .or_else(|| {
                            rv.and_then(|v| v.produces.as_ref().map(|p| p.produced_type.clone()))
                        });

                // Map domain → constellation slot name for context
                let constellation_slot = match parts.first().copied() {
                    Some("kyc-case" | "kyc") => Some("kyc_case"),
                    Some("screening") => Some("screening"),
                    Some("document" | "requirement") => Some("evidence"),
                    Some("ubo" | "ownership" | "control") => Some("ubo_discovery"),
                    Some("cbu") => Some("cbu"),
                    Some("entity") => Some("entity"),
                    Some("sla") => Some("sla"),
                    Some("deal") => Some("deal"),
                    Some("tollgate") => Some("tollgate"),
                    _ => None,
                };

                // Build human-readable entity context
                let entity_context = match (constellation_slot, target_entity_kind.as_deref()) {
                    (Some("kyc_case"), _) => Some("Operates on the KYC case for this CBU".into()),
                    (Some("screening"), _) => {
                        Some("Compliance screening on entities in this workstream".into())
                    }
                    (Some("evidence"), _) => {
                        Some("Document/evidence requirement for an entity".into())
                    }
                    (Some("ubo_discovery"), _) => {
                        Some("Group-level ownership and control discovery".into())
                    }
                    (Some("cbu"), _) => {
                        Some("Operates on a Client Business Unit (structure)".into())
                    }
                    (Some("entity"), Some(kind)) => Some(format!("Operates on a {} entity", kind)),
                    (Some("tollgate"), _) => Some("KYC approval tollgate evaluation".into()),
                    (Some(slot), _) => Some(format!("Operates in the {} context", slot)),
                    _ => None,
                };

                // Get the dominant entity name from session context if available
                let target_entity_name =
                    session.context.dominant_entity_id.map(|id| id.to_string());

                VerbOption {
                    verb_fqn: c.verb.clone(),
                    description,
                    example: format!("({})", c.verb),
                    score: c.score,
                    matched_phrase: Some(c.matched_phrase.clone()),
                    domain_label: location.as_ref().map(|l| l.domain_label.clone()),
                    category_label: location.as_ref().map(|l| l.category_label.clone()),
                    suggested_utterance: suggested,
                    verb_kind: Some(verb_kind.to_string()),
                    differentiation,
                    requires_state: None,
                    produces_state: None,
                    scope: None,
                    step_count,
                    target_entity_kind,
                    constellation_slot: constellation_slot.map(String::from),
                    entity_context,
                    target_entity_name,
                }
            })
            .collect();

        let request_id = Uuid::new_v4().to_string();

        // Build message for display with differentiation + entity context
        let options_text: Vec<String> = options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                let utterance = opt
                    .suggested_utterance
                    .as_deref()
                    .unwrap_or(&opt.description);
                let reason = opt.differentiation.as_deref().unwrap_or(&opt.description);
                let context = opt
                    .entity_context
                    .as_deref()
                    .map(|ctx| format!(" [{}]", ctx))
                    .unwrap_or_default();
                format!("{}. \"{}\" — {}{}", i + 1, utterance, reason, context)
            })
            .collect();

        let message = format!(
            "I'm not sure which you meant:\n\n{}\n\nYou can type a number, or say one of the phrases above.",
            options_text.join("\n")
        );

        let disambiguation_request = VerbDisambiguationRequest {
            request_id: request_id.clone(),
            original_input: original_input.to_string(),
            options,
            prompt: "Which action did you mean?".to_string(),
        };

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
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
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
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
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


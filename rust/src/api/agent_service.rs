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
use crate::dsl_v2::ref_resolver::ResolveResult;
use crate::dsl_v2::validation::RefType;
use crate::dsl_v2::{enrich_program, parse_program, runtime_registry, Statement};
use crate::graph::GraphScope;
use crate::macros::OperatorMacroRegistry;
use crate::mcp::verb_search_factory::VerbSearcherFactory;
use crate::session::SessionScope;
use crate::session::{SessionState, UnifiedSession, UnresolvedRefInfo};
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
}

impl AgentService {
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

    /// Build the verb searcher with macro registry
    fn build_verb_searcher(&self) -> crate::mcp::verb_search::HybridVerbSearcher {
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
        VerbSearcherFactory::build(
            &self.pool,
            dyn_embedder,
            self.learned_data.clone(),
            Arc::new(macro_reg),
            self.lexicon.clone(),
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

        OrchestratorContext {
            actor,
            session_id: Some(session.id),
            case_id: Some(session.id), // Use session ID as case ID
            dominant_entity_id: session.context.dominant_entity_id,
            scope: session.context.client_scope.clone(),
            pool: self.pool.clone(),
            verb_searcher: std::sync::Arc::new(self.build_verb_searcher()),
            lookup_service: self.get_lookup_service(),
            policy_gate: self.policy_gate.clone(),
            source,
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
                    });
                }
            }
            // Not a number - clear pending and process as new input
            session.pending_verb_disambiguation = None;
        }

        // 3. Check for pending decision (client group or deal selection)
        if let Some(ref pending) = session.pending_decision.clone() {
            // Check if input is a number (1, 2, 3, etc.) or special keyword
            let input_upper = input.trim().to_uppercase();
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
                    let packet = pending.clone();
                    session.pending_decision = None;

                    // Handle the selection based on decision kind
                    return self
                        .handle_decision_selection(session, &packet, &choice)
                        .await;
                } else if let Ok(idx) = input.trim().parse::<usize>() {
                    // Try index-based selection
                    if idx >= 1 && idx <= pending.choices.len() {
                        let choice = pending.choices[idx - 1].clone();
                        let packet = pending.clone();
                        session.pending_decision = None;

                        return self
                            .handle_decision_selection(session, &packet, &choice)
                            .await;
                    }
                }

                // Invalid selection
                let msg = format!(
                    "Please select a valid option (1-{}) or type NEW/SKIP.",
                    pending.choices.len()
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
                    decision: Some(pending.clone()),
                });
            }
            // Not a number/keyword - try fuzzy match against choice labels
            // This handles cases like typing "aviva" when the choices list
            // contains "Aviva Investors"
            let input_lower = input.trim().to_lowercase();
            if let Some(matched) = pending
                .choices
                .iter()
                .find(|c| c.label.to_lowercase().contains(&input_lower))
            {
                let choice = matched.clone();
                let packet = pending.clone();
                session.pending_decision = None;

                return self
                    .handle_decision_selection(session, &packet, &choice)
                    .await;
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
                let packet = pending.clone();
                session.pending_decision = None;

                // Auto-skip deal selection, then process the original input
                let _ = self
                    .handle_decision_selection(session, &packet, &skip_choice)
                    .await;
                // Fall through to process original input via intent pipeline
            } else {
                session.pending_decision = None;
            }
        }

        // 4. Check for pending intent tier selection
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
        if let Some(decision) = self.check_session_context(session).await {
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
        let result = orch_outcome.map(|o| o.pipeline_result);

        match result {
            Ok(r) => {
                // Handle scope resolution - "work on allianz", "switch to blackrock"
                if let PipelineOutcome::ScopeResolved {
                    ref group_id,
                    ref group_name,
                    entity_count,
                } = r.outcome
                {
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
                    session.add_agent_message(msg.clone(), None, None);
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
                    });
                }

                // Handle scope candidates - multiple client matches
                if matches!(r.outcome, PipelineOutcome::ScopeCandidates) {
                    if let Some(err) = r.validation_error {
                        return Ok(self.fail(&err, session));
                    }
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
                    return Ok(self.staged_response(r.dsl, msg));
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

        // Fallback
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

    /// Check session context and prompt for client group or deal if needed
    ///
    /// Returns Some(response) if context needs to be set, None to continue processing
    async fn check_session_context(
        &self,
        session: &mut UnifiedSession,
    ) -> Option<AgentChatResponse> {
        use crate::database::DealRepository;
        use ob_poc_types::{
            ClarificationPayload, DealClarificationPayload, DealOption, DecisionKind,
            DecisionPacket, DecisionTrace, SessionStateView, UserChoice,
        };

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
                                format!("Now working with client: {}. Let me check for any existing deals...", group.alias)
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
            _ => format!("Selected: {}", choice.label),
        };

        session.add_agent_message(message.clone(), None, None);

        // After setting client group, check for deals
        if matches!(packet.kind, DecisionKind::ClarifyGroup) {
            if let Some(deal_decision) = self.check_session_context(session).await {
                return Ok(deal_decision);
            }
        }

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
        use crate::dsl_v2::{DslExecutor, ExecutionContext};

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

                                    ast: None,
                                    disambiguation: None,
                                    commands: None,
                                    unresolved_refs: None,
                                    current_ref_index: None,
                                    dsl_hash: None,
                                    verb_disambiguation: None,
                                    intent_tier: None,
                                    decision: None,
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

                    ast: None,
                    disambiguation: None,
                    commands: None,
                    unresolved_refs: None,
                    current_ref_index: None,
                    dsl_hash: None,
                    verb_disambiguation: None,
                    intent_tier: None,
                    decision: None,
                })
            }
            Err(e) => {
                let msg = format!("Execution failed: {}", e);
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

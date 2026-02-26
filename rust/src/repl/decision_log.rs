//! DecisionLog — Structured logging for offline replay and tuning.
//!
//! Every agent turn produces a `DecisionLog` snapshot capturing:
//!
//! - Raw verb candidates (pre-scoring)
//! - Reranked verb candidates (post-scoring)
//! - Entity candidates and resolution method
//! - Arg extraction method (deterministic vs LLM)
//! - Context stack summary at the time of decision
//! - Final DSL proposed
//!
//! The log enables:
//!
//! 1. **Golden corpus CI** — replay saved sessions with current scoring
//!    constants and verify accuracy doesn't regress.
//! 2. **Replay-tuner CLI** — sweep scoring constants offline and compare
//!    accuracy across configurations.
//! 3. **Production diagnostics** — explain why a particular verb/entity
//!    was chosen (or rejected) for a given turn.
//!
//! # Privacy (Invariant I-10)
//!
//! In operational mode, `raw_input` is redacted (replaced with a hash).
//! Only `input_hash` is kept for replay keying. The scoring decisions
//! and verb/entity candidates are always retained.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::scoring;

// ============================================================================
// ScoringConfig — Tunable scoring constants
// ============================================================================

/// All scoring constants in one struct for serialization and replay.
///
/// The replay-tuner CLI varies these values to find optimal settings.
/// In production, these are read from the `scoring` module constants.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoringConfig {
    // Pack scoring (scoring.rs)
    pub pack_verb_boost: f32,
    pub pack_verb_penalty: f32,
    pub template_step_boost: f32,
    pub domain_affinity_boost: f32,
    pub absolute_floor: f32,
    pub threshold: f32,
    pub margin: f32,
    pub strong_threshold: f32,

    // Proposal engine (proposal_engine.rs)
    pub template_score_threshold: f32,
    pub template_confidence_boost: f32,
    pub auto_advance_threshold: f32,

    // Entity resolver (resolver.rs)
    pub entity_high_confidence: f64,
    pub entity_min_confidence: f64,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            pack_verb_boost: scoring::PACK_VERB_BOOST,
            pack_verb_penalty: scoring::PACK_VERB_PENALTY,
            template_step_boost: scoring::TEMPLATE_STEP_BOOST,
            domain_affinity_boost: scoring::DOMAIN_AFFINITY_BOOST,
            absolute_floor: scoring::ABSOLUTE_FLOOR,
            threshold: scoring::THRESHOLD,
            margin: scoring::MARGIN,
            strong_threshold: scoring::STRONG_THRESHOLD,
            template_score_threshold: 0.3,
            template_confidence_boost: 0.1,
            auto_advance_threshold: 0.85,
            entity_high_confidence: 0.7,
            entity_min_confidence: 0.3,
        }
    }
}

impl ScoringConfig {
    /// Load from the current compiled constants.
    pub fn from_current() -> Self {
        Self::default()
    }
}

// ============================================================================
// DecisionLog — Full turn snapshot
// ============================================================================

/// Complete decision snapshot for a single REPL turn.
///
/// Captures everything needed to replay or diagnose a decision:
/// - What the user said (or its hash)
/// - What verbs were considered and why
/// - What entities were resolved and how
/// - What args were extracted and by what method
/// - What DSL was proposed
/// - What scoring config was active
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionLog {
    /// Unique log entry ID.
    pub id: Uuid,

    /// Session this turn belongs to.
    pub session_id: Uuid,

    /// Turn number (0-based, sequential within session).
    pub turn: u32,

    /// Timestamp of the decision.
    pub timestamp: DateTime<Utc>,

    /// SHA-256 hash of the raw input (always present, even when redacted).
    pub input_hash: String,

    /// Raw user input (redacted to empty string in operational mode per I-10).
    pub raw_input: String,

    /// Classification of what kind of turn this was.
    pub turn_type: TurnType,

    /// Verb decision details.
    pub verb_decision: VerbDecision,

    /// Entity resolution details (empty if no entities in this turn).
    pub entity_decisions: Vec<EntityDecision>,

    /// Arg extraction details.
    pub extraction_decision: ExtractionDecision,

    /// Context summary at the time of this decision.
    pub context_summary: ContextSummary,

    /// Final DSL proposed (if any).
    pub proposed_dsl: Option<String>,

    /// Scoring config used for this decision.
    pub scoring_config: ScoringConfig,
}

/// What kind of interaction this turn represented.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnType {
    /// Normal user message → verb match → DSL.
    IntentMatch,
    /// User selected from disambiguation options.
    VerbSelection,
    /// User answered a pack question.
    PackAnswer,
    /// User confirmed/rejected a proposal.
    Confirmation,
    /// User issued a fast command (undo, show runbook, etc.).
    FastCommand,
    /// Entity resolution disambiguation.
    EntitySelection,
    /// Scope/client-group selection.
    ScopeSelection,
}

// ============================================================================
// Verb Decision
// ============================================================================

/// Full record of verb matching for this turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbDecision {
    /// Raw candidates from semantic search (before pack scoring).
    pub raw_candidates: Vec<VerbCandidateSnapshot>,

    /// Reranked candidates (after pack scoring, exclusions, floor filter).
    pub reranked_candidates: Vec<VerbCandidateSnapshot>,

    /// Ambiguity outcome from the policy engine.
    pub ambiguity_outcome: String,

    /// Final selected verb (if any).
    pub selected_verb: Option<String>,

    /// Selection confidence.
    pub confidence: f32,

    /// Whether a template fast-path was used.
    pub used_template_path: bool,

    /// Template ID if template path was used.
    pub template_id: Option<String>,

    /// Precondition filter stats (if filter was applied).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precondition_filter: Option<PreconditionFilterLog>,

    /// SemReg ContextEnvelope fingerprint (SHA-256 of sorted allowed verb FQNs).
    /// Populated when SemOS client is available and ContextEnvelope is resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_envelope_fingerprint: Option<String>,

    /// Number of verbs pruned by SemReg context resolution.
    /// Zero when SemOS unavailable or when no pruning occurred.
    #[serde(default)]
    pub pruned_verbs_count: usize,
}

/// Log entry for precondition filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionFilterLog {
    /// Candidates before filtering.
    pub before_count: usize,
    /// Candidates after filtering.
    pub after_count: usize,
    /// Verbs removed with reasons.
    pub removed: Vec<PreconditionRemovedVerb>,
}

/// A verb removed by precondition filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionRemovedVerb {
    pub verb_fqn: String,
    pub reasons: Vec<String>,
    pub suggested_verb: Option<String>,
}

/// Snapshot of a verb candidate at a point in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidateSnapshot {
    pub verb_fqn: String,
    pub score: f32,
    pub domain: Option<String>,
    /// Tags explaining score adjustments.
    pub adjustments: Vec<String>,
}

impl Default for VerbDecision {
    fn default() -> Self {
        Self {
            raw_candidates: Vec::new(),
            reranked_candidates: Vec::new(),
            ambiguity_outcome: "none".to_string(),
            selected_verb: None,
            confidence: 0.0,
            used_template_path: false,
            template_id: None,
            precondition_filter: None,
            context_envelope_fingerprint: None,
            pruned_verbs_count: 0,
        }
    }
}

// ============================================================================
// Entity Decision
// ============================================================================

/// Record of entity resolution for a single arg slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDecision {
    /// Arg name that needed entity resolution (e.g. "entity-id").
    pub arg_name: String,

    /// Raw input mention (e.g. "the Irish fund").
    pub mention: String,

    /// Resolution method used.
    pub method: EntityResolutionMethod,

    /// Candidates considered (for search-based resolution).
    pub candidates: Vec<EntityCandidateSnapshot>,

    /// Final resolved entity ID (if resolved).
    pub resolved_entity_id: Option<Uuid>,

    /// Resolved display name.
    pub resolved_name: Option<String>,
}

/// How an entity was resolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityResolutionMethod {
    /// Resolved via pronoun/focus (zero cost).
    Focus,
    /// Resolved via accumulated Q&A answers.
    AccumulatedAnswer,
    /// Resolved via canonicalization.
    Canonicalization,
    /// Resolved via search against candidate universe.
    ScopedSearch,
    /// Resolved by user picking from disambiguation.
    UserSelection,
    /// Not resolved.
    Unresolved,
}

/// Snapshot of an entity candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidateSnapshot {
    pub entity_id: Uuid,
    pub display_name: String,
    pub entity_type: Option<String>,
    pub score: f32,
}

// ============================================================================
// Extraction Decision
// ============================================================================

/// Record of how arguments were extracted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionDecision {
    /// Method used for extraction.
    pub method: ExtractionMethod,

    /// Args that were filled.
    pub filled_args: HashMap<String, String>,

    /// Args that remain missing.
    pub missing_args: Vec<String>,

    /// Per-slot provenance.
    pub slot_provenance: HashMap<String, String>,

    /// LLM model ID (if LLM was used).
    pub model_id: Option<String>,

    /// LLM confidence (if LLM was used).
    pub llm_confidence: Option<f64>,
}

/// How args were extracted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMethod {
    /// All args filled deterministically (no LLM).
    Deterministic,
    /// LLM was used for arg extraction.
    Llm,
    /// Mix of deterministic + LLM.
    Hybrid,
    /// Fast command — no extraction needed.
    FastCommand,
    /// No extraction attempted (e.g. no-match turn).
    None,
}

impl Default for ExtractionDecision {
    fn default() -> Self {
        Self {
            method: ExtractionMethod::None,
            filled_args: HashMap::new(),
            missing_args: Vec::new(),
            slot_provenance: HashMap::new(),
            model_id: None,
            llm_confidence: None,
        }
    }
}

// ============================================================================
// Context Summary
// ============================================================================

/// Lightweight snapshot of the ContextStack at decision time.
///
/// Captures enough to understand the decision without the full
/// ContextStack (which contains large collections).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    /// Client group name (if in scope).
    pub client_group: Option<String>,

    /// Number of CBUs in scope.
    pub cbu_count: usize,

    /// Active pack ID (staged or executed).
    pub active_pack_id: Option<String>,

    /// Active pack's dominant domain.
    pub active_pack_domain: Option<String>,

    /// Number of allowed verbs in active pack.
    pub pack_allowed_verb_count: usize,

    /// Number of forbidden verbs in active pack.
    pub pack_forbidden_verb_count: usize,

    /// Whether a template hint is active.
    pub has_template_hint: bool,

    /// Expected next verb from template hint (if any).
    pub template_expected_verb: Option<String>,

    /// Template progress label (e.g. "Step 3 of 8").
    pub template_progress: Option<String>,

    /// Number of active exclusions.
    pub exclusion_count: usize,

    /// Number of accumulated Q&A answers.
    pub answer_count: usize,

    /// Focus entity display name (if set).
    pub focus_entity: Option<String>,

    /// Focus CBU display name (if set).
    pub focus_cbu: Option<String>,

    /// Number of recent entity mentions.
    pub recent_mention_count: usize,

    /// Number of completed runbook entries.
    pub completed_entry_count: usize,
}

// ============================================================================
// Builder
// ============================================================================

impl DecisionLog {
    /// Create a new DecisionLog for a turn.
    pub fn new(session_id: Uuid, turn: u32, raw_input: &str) -> Self {
        let input_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(raw_input.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        Self {
            id: Uuid::new_v4(),
            session_id,
            turn,
            timestamp: Utc::now(),
            input_hash,
            raw_input: raw_input.to_string(),
            turn_type: TurnType::IntentMatch,
            verb_decision: VerbDecision::default(),
            entity_decisions: Vec::new(),
            extraction_decision: ExtractionDecision::default(),
            context_summary: ContextSummary::empty(),
            proposed_dsl: None,
            scoring_config: ScoringConfig::from_current(),
        }
    }

    /// Redact raw input for operational mode (Invariant I-10).
    pub fn redact(&mut self) {
        self.raw_input = String::new();
    }

    /// Set turn type.
    pub fn with_turn_type(mut self, turn_type: TurnType) -> Self {
        self.turn_type = turn_type;
        self
    }

    /// Set verb decision.
    pub fn with_verb_decision(mut self, decision: VerbDecision) -> Self {
        self.verb_decision = decision;
        self
    }

    /// Set entity decisions.
    pub fn with_entity_decisions(mut self, decisions: Vec<EntityDecision>) -> Self {
        self.entity_decisions = decisions;
        self
    }

    /// Set extraction decision.
    pub fn with_extraction_decision(mut self, decision: ExtractionDecision) -> Self {
        self.extraction_decision = decision;
        self
    }

    /// Set context summary.
    pub fn with_context_summary(mut self, summary: ContextSummary) -> Self {
        self.context_summary = summary;
        self
    }

    /// Set proposed DSL.
    pub fn with_proposed_dsl(mut self, dsl: String) -> Self {
        self.proposed_dsl = Some(dsl);
        self
    }
}

impl ContextSummary {
    pub fn empty() -> Self {
        Self {
            client_group: None,
            cbu_count: 0,
            active_pack_id: None,
            active_pack_domain: None,
            pack_allowed_verb_count: 0,
            pack_forbidden_verb_count: 0,
            has_template_hint: false,
            template_expected_verb: None,
            template_progress: None,
            exclusion_count: 0,
            answer_count: 0,
            focus_entity: None,
            focus_cbu: None,
            recent_mention_count: 0,
            completed_entry_count: 0,
        }
    }

    /// Build from a ContextStack.
    pub fn from_context(ctx: &super::context_stack::ContextStack) -> Self {
        let (active_pack_id, active_pack_domain, pack_allowed, pack_forbidden) =
            if let Some(pack) = ctx.active_pack() {
                (
                    Some(pack.pack_id.clone()),
                    pack.dominant_domain.clone(),
                    pack.allowed_verbs.len(),
                    pack.forbidden_verbs.len(),
                )
            } else {
                (None, None, 0, 0)
            };

        let (template_expected_verb, template_progress) = if let Some(ref hint) = ctx.template_hint
        {
            (
                Some(hint.expected_verb.clone()),
                Some(hint.progress_label()),
            )
        } else {
            (None, None)
        };

        let completed = ctx.outcomes.outcomes.len();

        Self {
            client_group: ctx.derived_scope.client_group_name.clone(),
            cbu_count: ctx.derived_scope.loaded_cbu_ids.len(),
            active_pack_id,
            active_pack_domain,
            pack_allowed_verb_count: pack_allowed,
            pack_forbidden_verb_count: pack_forbidden,
            has_template_hint: ctx.template_hint.is_some(),
            template_expected_verb,
            template_progress,
            exclusion_count: ctx.exclusions.exclusions.len(),
            answer_count: ctx.accumulated_answers.len(),
            focus_entity: ctx.focus.entity.as_ref().map(|f| f.display_name.clone()),
            focus_cbu: ctx.focus.cbu.as_ref().map(|f| f.display_name.clone()),
            recent_mention_count: ctx.recent.mentions.len(),
            completed_entry_count: completed,
        }
    }
}

// ============================================================================
// Session Decision Log (collection across turns)
// ============================================================================

/// Accumulates DecisionLogs across an entire session for replay.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionDecisionLog {
    pub session_id: Uuid,
    pub entries: Vec<DecisionLog>,
}

impl SessionDecisionLog {
    pub fn new(session_id: Uuid) -> Self {
        Self {
            session_id,
            entries: Vec::new(),
        }
    }

    /// Append a turn decision.
    pub fn push(&mut self, log: DecisionLog) {
        self.entries.push(log);
    }

    /// Number of logged turns.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Export to JSON for replay-tuner consumption.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Import from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ============================================================================
// Golden Corpus Types
// ============================================================================

/// A single golden corpus test case for CI regression testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenTestCase {
    /// Unique test ID.
    pub id: String,

    /// Category for grouping (e.g. "kyc", "bootstrap", "book-setup").
    pub category: String,

    /// User input text.
    pub input: String,

    /// Active pack ID at the time of this input (if any).
    pub pack_id: Option<String>,

    /// Expected verb FQN.
    pub expected_verb: String,

    /// Whether the verb must be an exact match or top-3 is acceptable.
    pub match_mode: GoldenMatchMode,

    /// Optional: expected args (key → value).
    pub expected_args: HashMap<String, String>,

    /// Optional: expected entity resolution.
    pub expected_entities: Vec<GoldenEntityExpectation>,

    /// Tags for filtering (e.g. "pronoun", "multi-intent", "edge-case").
    pub tags: Vec<String>,
}

/// How strict the verb match must be.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoldenMatchMode {
    /// Must be the top-1 result.
    Exact,
    /// Must be in top-3 results.
    TopThree,
    /// Either exact match or acceptable ambiguity (for safety-critical verbs).
    MatchOrAmbiguous,
}

/// Expected entity resolution for a golden test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenEntityExpectation {
    pub arg_name: String,
    pub expected_method: EntityResolutionMethod,
    pub expected_value: Option<String>,
}

/// Result of running a golden test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenTestResult {
    pub test_id: String,
    pub passed: bool,
    pub actual_verb: Option<String>,
    pub actual_score: f32,
    pub verb_rank: Option<usize>,
    pub failure_reason: Option<String>,
    pub scoring_config: ScoringConfig,
}

/// Aggregated results for a golden corpus run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenCorpusReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub accuracy_pct: f32,
    pub by_category: HashMap<String, CategoryResult>,
    pub scoring_config: ScoringConfig,
    pub results: Vec<GoldenTestResult>,
}

/// Per-category result summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryResult {
    pub total: usize,
    pub passed: usize,
    pub accuracy_pct: f32,
}

impl GoldenCorpusReport {
    /// Build from a list of test results.
    pub fn from_results(results: Vec<GoldenTestResult>, config: ScoringConfig) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let accuracy_pct = if total > 0 {
            (passed as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let by_category: HashMap<String, (usize, usize)> = HashMap::new();
        // Category breakdown is built by callers who have access to the
        // GoldenTestCase category field. Leave empty here.

        let category_results = by_category
            .into_iter()
            .map(|(cat, (total, passed))| {
                let acc = if total > 0 {
                    (passed as f32 / total as f32) * 100.0
                } else {
                    0.0
                };
                (
                    cat,
                    CategoryResult {
                        total,
                        passed,
                        accuracy_pct: acc,
                    },
                )
            })
            .collect();

        Self {
            total,
            passed,
            failed: total - passed,
            accuracy_pct,
            by_category: category_results,
            scoring_config: config,
            results,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::context_stack::ContextStack;
    use crate::repl::runbook::Runbook;

    fn empty_context() -> ContextStack {
        let rb = Runbook::new(Uuid::new_v4());
        ContextStack::from_runbook(&rb, None, 0)
    }

    #[test]
    fn test_scoring_config_default_matches_constants() {
        let config = ScoringConfig::default();
        assert!((config.pack_verb_boost - scoring::PACK_VERB_BOOST).abs() < f32::EPSILON);
        assert!((config.pack_verb_penalty - scoring::PACK_VERB_PENALTY).abs() < f32::EPSILON);
        assert!((config.template_step_boost - scoring::TEMPLATE_STEP_BOOST).abs() < f32::EPSILON);
        assert!(
            (config.domain_affinity_boost - scoring::DOMAIN_AFFINITY_BOOST).abs() < f32::EPSILON
        );
        assert!((config.absolute_floor - scoring::ABSOLUTE_FLOOR).abs() < f32::EPSILON);
        assert!((config.threshold - scoring::THRESHOLD).abs() < f32::EPSILON);
        assert!((config.margin - scoring::MARGIN).abs() < f32::EPSILON);
        assert!((config.strong_threshold - scoring::STRONG_THRESHOLD).abs() < f32::EPSILON);
    }

    #[test]
    fn test_decision_log_creation() {
        let session_id = Uuid::new_v4();
        let log = DecisionLog::new(session_id, 0, "load the allianz book");

        assert_eq!(log.session_id, session_id);
        assert_eq!(log.turn, 0);
        assert_eq!(log.raw_input, "load the allianz book");
        assert!(!log.input_hash.is_empty());
        assert_eq!(log.turn_type, TurnType::IntentMatch);
        assert!(log.proposed_dsl.is_none());
    }

    #[test]
    fn test_decision_log_redact() {
        let mut log = DecisionLog::new(Uuid::new_v4(), 0, "sensitive input");
        let hash_before = log.input_hash.clone();

        log.redact();

        assert!(log.raw_input.is_empty());
        assert_eq!(log.input_hash, hash_before); // Hash preserved
    }

    #[test]
    fn test_decision_log_builder_chain() {
        let log = DecisionLog::new(Uuid::new_v4(), 1, "add the Irish fund")
            .with_turn_type(TurnType::IntentMatch)
            .with_verb_decision(VerbDecision {
                raw_candidates: vec![VerbCandidateSnapshot {
                    verb_fqn: "kyc.add-entity".to_string(),
                    score: 0.82,
                    domain: Some("kyc".to_string()),
                    adjustments: vec!["pack_boost +0.10".to_string()],
                }],
                reranked_candidates: vec![VerbCandidateSnapshot {
                    verb_fqn: "kyc.add-entity".to_string(),
                    score: 0.92,
                    domain: Some("kyc".to_string()),
                    adjustments: vec!["pack_boost +0.10".to_string()],
                }],
                ambiguity_outcome: "confident".to_string(),
                selected_verb: Some("kyc.add-entity".to_string()),
                confidence: 0.92,
                used_template_path: false,
                template_id: None,
                precondition_filter: None,
                context_envelope_fingerprint: Some("abc123".to_string()),
                pruned_verbs_count: 42,
            })
            .with_proposed_dsl("(kyc.add-entity :entity-name \"Irish Fund\")".to_string());

        assert_eq!(
            log.verb_decision.selected_verb.as_deref(),
            Some("kyc.add-entity")
        );
        assert!(log.proposed_dsl.is_some());
        assert_eq!(log.verb_decision.reranked_candidates.len(), 1);
        assert!((log.verb_decision.reranked_candidates[0].score - 0.92).abs() < f32::EPSILON);
    }

    #[test]
    fn test_context_summary_from_empty_context() {
        let ctx = empty_context();
        let summary = ContextSummary::from_context(&ctx);

        assert!(summary.client_group.is_none());
        assert_eq!(summary.cbu_count, 0);
        assert!(summary.active_pack_id.is_none());
        assert!(!summary.has_template_hint);
        assert_eq!(summary.exclusion_count, 0);
        assert_eq!(summary.answer_count, 0);
        assert!(summary.focus_entity.is_none());
    }

    #[test]
    fn test_context_summary_from_scoped_context() {
        let mut rb = Runbook::new(Uuid::new_v4());
        rb.client_group_id = Some(Uuid::new_v4());

        let mut entry = super::super::runbook::RunbookEntry::new(
            "session.load-cluster".to_string(),
            "Load Allianz".to_string(),
            "(session.load-cluster :client <Allianz>)".to_string(),
        );
        entry
            .args
            .insert("client".to_string(), "Allianz".to_string());
        entry.status = super::super::runbook::EntryStatus::Completed;
        let cbu_id = Uuid::new_v4();
        entry.result = Some(serde_json::json!({
            "cbu_ids": [cbu_id.to_string()]
        }));
        rb.add_entry(entry);

        let ctx = ContextStack::from_runbook(&rb, None, 1);
        let summary = ContextSummary::from_context(&ctx);

        assert_eq!(summary.client_group.as_deref(), Some("Allianz"));
        assert_eq!(summary.cbu_count, 1);
        assert_eq!(summary.completed_entry_count, 1);
    }

    #[test]
    fn test_session_decision_log() {
        let session_id = Uuid::new_v4();
        let mut session_log = SessionDecisionLog::new(session_id);

        assert!(session_log.is_empty());

        session_log.push(DecisionLog::new(session_id, 0, "hello"));
        session_log.push(DecisionLog::new(session_id, 1, "load allianz"));

        assert_eq!(session_log.len(), 2);
        assert!(!session_log.is_empty());
    }

    #[test]
    fn test_session_decision_log_json_roundtrip() {
        let session_id = Uuid::new_v4();
        let mut session_log = SessionDecisionLog::new(session_id);
        session_log.push(
            DecisionLog::new(session_id, 0, "load the book")
                .with_turn_type(TurnType::IntentMatch)
                .with_proposed_dsl("(session.load-galaxy :apex-name \"Allianz\")".to_string()),
        );

        let json = session_log.to_json().expect("serialize");
        let restored = SessionDecisionLog::from_json(&json).expect("deserialize");

        assert_eq!(restored.len(), 1);
        assert_eq!(restored.entries[0].turn, 0);
        assert_eq!(restored.entries[0].turn_type, TurnType::IntentMatch);
        assert!(restored.entries[0].proposed_dsl.is_some());
    }

    #[test]
    fn test_golden_test_case_serde() {
        let test_case = GoldenTestCase {
            id: "kyc-001".to_string(),
            category: "kyc".to_string(),
            input: "add the Irish fund".to_string(),
            pack_id: Some("kyc-case".to_string()),
            expected_verb: "kyc.add-entity".to_string(),
            match_mode: GoldenMatchMode::Exact,
            expected_args: HashMap::from([("entity-name".to_string(), "Irish fund".to_string())]),
            expected_entities: vec![],
            tags: vec!["pronoun".to_string()],
        };

        let json = serde_json::to_string(&test_case).expect("serialize");
        let restored: GoldenTestCase = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, "kyc-001");
        assert_eq!(restored.match_mode, GoldenMatchMode::Exact);
    }

    #[test]
    fn test_golden_corpus_report() {
        let results = vec![
            GoldenTestResult {
                test_id: "t1".to_string(),
                passed: true,
                actual_verb: Some("kyc.add-entity".to_string()),
                actual_score: 0.90,
                verb_rank: Some(1),
                failure_reason: None,
                scoring_config: ScoringConfig::default(),
            },
            GoldenTestResult {
                test_id: "t2".to_string(),
                passed: false,
                actual_verb: Some("cbu.create".to_string()),
                actual_score: 0.55,
                verb_rank: Some(3),
                failure_reason: Some("expected kyc.create-case".to_string()),
                scoring_config: ScoringConfig::default(),
            },
        ];

        let report = GoldenCorpusReport::from_results(results, ScoringConfig::default());
        assert_eq!(report.total, 2);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 1);
        assert!((report.accuracy_pct - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_verb_candidate_snapshot_adjustments() {
        let snap = VerbCandidateSnapshot {
            verb_fqn: "kyc.add-entity".to_string(),
            score: 0.92,
            domain: Some("kyc".to_string()),
            adjustments: vec![
                "pack_boost +0.10".to_string(),
                "template_step +0.15".to_string(),
                "domain_affinity +0.03".to_string(),
            ],
        };
        assert_eq!(snap.adjustments.len(), 3);
    }

    #[test]
    fn test_extraction_decision_deterministic() {
        let decision = ExtractionDecision {
            method: ExtractionMethod::Deterministic,
            filled_args: HashMap::from([("cbu-id".to_string(), "some-uuid".to_string())]),
            missing_args: vec![],
            slot_provenance: HashMap::from([(
                "cbu-id".to_string(),
                "copied_from_previous".to_string(),
            )]),
            model_id: Some("deterministic".to_string()),
            llm_confidence: None,
        };

        assert_eq!(decision.method, ExtractionMethod::Deterministic);
        assert!(decision.missing_args.is_empty());
    }

    #[test]
    fn test_entity_decision_focus_resolution() {
        let decision = EntityDecision {
            arg_name: "entity-id".to_string(),
            mention: "it".to_string(),
            method: EntityResolutionMethod::Focus,
            candidates: vec![],
            resolved_entity_id: Some(Uuid::new_v4()),
            resolved_name: Some("Allianz SE".to_string()),
        };

        assert_eq!(decision.method, EntityResolutionMethod::Focus);
        assert!(decision.candidates.is_empty());
    }

    #[test]
    fn test_scoring_config_serde_roundtrip() {
        let config = ScoringConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: ScoringConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, restored);
    }

    #[test]
    fn test_input_hash_deterministic() {
        let log1 = DecisionLog::new(Uuid::new_v4(), 0, "load allianz");
        let log2 = DecisionLog::new(Uuid::new_v4(), 0, "load allianz");
        assert_eq!(log1.input_hash, log2.input_hash);

        let log3 = DecisionLog::new(Uuid::new_v4(), 0, "load blackrock");
        assert_ne!(log1.input_hash, log3.input_hash);
    }
}

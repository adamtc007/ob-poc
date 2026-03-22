//! Core data types for loopback calibration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Calibration mode for a generated or curated utterance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationMode {
    Positive,
    Negative,
    Boundary,
}

/// Subtype for negative utterances.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NegativeType {
    TypeA,
    TypeB,
}

/// Governance state for a calibration scenario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceStatus {
    Draft,
    Reviewed,
    Admitted,
    Deprecated,
    Superseded { by: Uuid },
}

/// Relative ambiguity risk between target and neighbour verb.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfusionRisk {
    High,
    Medium,
    Low,
}

/// Expected outcome for a calibration utterance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ExpectedOutcome {
    ResolvesTo(String),
    ResolvesToOneOf(Vec<String>),
    HaltsWithReason(ExpectedHaltReason),
    HaltsAtPhase(u8),
    TriggersClarification,
    FallsToSage,
}

/// Expected halt reason for negative / boundary calibration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedHaltReason {
    NoViableVerb,
    StateConflict,
    ConstellationBlock,
    AmbiguousResolution,
    BelowConfidenceThreshold,
    DagOrderingConflict,
    ExclusionMakesPlanInfeasible,
    MidPlanConstellationBlock,
    MissingReferentialContext,
    NoParsableIntent,
    SemanticNotReady,
    NoAllowedVerbs,
    NoMatch,
}

/// Outcome verdict for one utterance executed against one scenario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalibrationVerdict {
    Pass,
    WrongVerb {
        expected: String,
        actual: String,
    },
    FalseNegative {
        expected: String,
        actual_halt: String,
    },
    FalsePositive {
        unexpected_verb: String,
        expected_halt: ExpectedHaltReason,
    },
    PassWithFragileMargin {
        margin: f32,
        threshold: f32,
    },
    CorrectPhaseWrongReason {
        expected: ExpectedHaltReason,
        actual: String,
    },
    WrongPhase {
        expected_phase: u8,
        actual_phase: u8,
    },
    UnnecessaryFallback,
}

/// Embedding pre-screen classification band.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreScreenStratum {
    ClearMatch {
        distance: f32,
    },
    BoundaryCase {
        margin: f32,
    },
    ClearNonMatch {
        distance: f32,
    },
    NeighbourPreferred {
        preferred_verb: String,
        preferred_distance: f32,
    },
}

/// Execution shape expected for a scenario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalibrationExecutionShape {
    Singleton,
    Batch {
        filter_expression: String,
        expected_entity_count: usize,
    },
    CrossEntityPlan {
        plan_nodes: Vec<CalibrationPlanNode>,
        expected_dag3_edges: Vec<String>,
        exclusion_predicates: Vec<String>,
    },
}

/// One node in a cross-entity plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalibrationPlanNode {
    pub entity_type: String,
    pub entity_state: String,
    pub target_verb: String,
}

/// Describes a semantically adjacent neighbour verb.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NearNeighbourVerb {
    pub verb_id: String,
    pub expected_embedding_distance: f32,
    pub confusion_risk: ConfusionRisk,
    pub distinguishing_signals: Vec<String>,
}

/// A deliberately excluded neighbour plus rationale.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExcludedNeighbour {
    pub verb_id: String,
    pub reason: String,
}

/// Curated or admitted gold utterance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoldUtterance {
    pub text: String,
    pub expected_outcome: ExpectedOutcome,
    pub authored_by: String,
    pub admitted_at: DateTime<Utc>,
}

/// Scenario seed persisted for calibration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationScenario {
    pub scenario_id: Uuid,
    pub scenario_name: String,
    pub created_by: String,
    pub governance_status: GovernanceStatus,
    pub constellation_template_id: String,
    pub constellation_template_version: String,
    pub situation_signature: String,
    pub situation_signature_hash: Option<i64>,
    pub operational_phase: String,
    pub target_entity_type: String,
    pub target_entity_state: String,
    pub linked_entity_states: Vec<(String, String)>,
    pub target_verb: String,
    pub legal_verb_set_snapshot: Vec<String>,
    pub verb_taxonomy_tag: String,
    pub excluded_neighbours: Vec<ExcludedNeighbour>,
    pub near_neighbour_verbs: Vec<NearNeighbourVerb>,
    pub expected_margin_threshold: f32,
    pub execution_shape: CalibrationExecutionShape,
    pub gold_utterances: Vec<GoldUtterance>,
    pub admitted_synthetic_set_id: Option<Uuid>,
}

/// Generated utterance candidate before execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneratedUtterance {
    pub text: String,
    pub calibration_mode: CalibrationMode,
    pub negative_type: Option<NegativeType>,
    pub expected_outcome: ExpectedOutcome,
    pub generation_rationale: String,
}

/// Embedding pre-screen output for one utterance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingPreScreen {
    pub utterance: String,
    pub target_verb_distance: f32,
    pub nearest_neighbour_distance: f32,
    pub nearest_neighbour_verb: String,
    pub margin: f32,
    pub stratum: PreScreenStratum,
}

/// Classified outcome for one calibration utterance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationOutcome {
    pub utterance_id: Uuid,
    pub utterance_text: String,
    pub calibration_mode: CalibrationMode,
    pub negative_type: Option<NegativeType>,
    pub pre_screen: Option<EmbeddingPreScreen>,
    pub expected_outcome: ExpectedOutcome,
    pub trace_id: Uuid,
    pub actual_resolved_verb: Option<String>,
    pub actual_halt_reason: Option<String>,
    pub verdict: CalibrationVerdict,
    pub failure_phase: Option<u8>,
    pub failure_detail: Option<serde_json::Value>,
    pub top1_score: Option<f32>,
    pub top2_score: Option<f32>,
    pub margin: Option<f32>,
    pub margin_stable: Option<bool>,
    pub latency_total_ms: Option<i64>,
    pub latency_per_phase: Option<Vec<(u8, i64)>>,
}

/// One persisted fixture-state row captured after a calibration utterance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureStateSnapshot {
    pub binding_key: String,
    pub entity_id: Uuid,
    pub entity_type: String,
    pub current_state: String,
}

/// Aggregated metrics for one run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CalibrationMetrics {
    pub positive_hit_rate: f32,
    pub negative_type_a_rejection_rate: f32,
    pub negative_type_b_rejection_rate: f32,
    pub boundary_correct_rate: f32,
    pub overall_accuracy: f32,
    pub phase4_fallback_rate: f32,
    pub phase4_avg_margin: Option<f32>,
    pub fragile_boundary_count: usize,
    pub avg_total_latency_ms: Option<f32>,
}

/// Drift summary between two runs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CalibrationDrift {
    pub prior_run_id: Uuid,
    pub current_run_id: Uuid,
    pub overall_accuracy_delta: f32,
    pub fallback_rate_delta: f32,
    pub avg_margin_delta: Option<f32>,
    pub newly_failing_utterances: Vec<Uuid>,
    pub newly_passing_utterances: Vec<Uuid>,
}

/// Persisted run record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRun {
    pub run_id: Uuid,
    pub scenario_id: Uuid,
    pub triggered_by: String,
    pub surface_versions: crate::traceability::SurfaceVersions,
    pub utterance_count: usize,
    pub positive_count: usize,
    pub negative_count: usize,
    pub boundary_count: usize,
    pub metrics: CalibrationMetrics,
    pub outcomes: Vec<CalibrationOutcome>,
    pub prior_run_id: Option<Uuid>,
    pub drift: Option<CalibrationDrift>,
    pub trace_ids: Vec<Uuid>,
    pub run_start: DateTime<Utc>,
    pub run_end: Option<DateTime<Utc>>,
}

/// Review-facing utterance row with lifecycle metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationUtteranceReviewRow {
    pub utterance_id: Uuid,
    pub scenario_id: Uuid,
    pub text: String,
    pub calibration_mode: CalibrationMode,
    pub negative_type: Option<NegativeType>,
    pub lifecycle_status: String,
    pub expected_outcome: ExpectedOutcome,
    pub pre_screen: Option<EmbeddingPreScreen>,
    pub generation_rationale: Option<String>,
    pub reviewed_by: Option<String>,
    pub admitted_at: Option<DateTime<Utc>>,
    pub deprecated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Portfolio summary row for one scenario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationPortfolioEntry {
    pub scenario_id: Uuid,
    pub scenario_name: String,
    pub target_verb: String,
    pub governance_status: GovernanceStatus,
    pub admitted_utterance_count: usize,
    pub last_run_id: Option<Uuid>,
    pub overall_accuracy: Option<f32>,
    pub fallback_rate: Option<f32>,
    pub fragile_boundary_count: Option<usize>,
    pub last_run_start: Option<DateTime<Utc>>,
}

/// Persisted fixture transition snapshot for one utterance within a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalibrationFixtureTransition {
    pub utterance_id: Uuid,
    pub trace_id: Uuid,
    pub fixture_state: Vec<FixtureStateSnapshot>,
}

/// Draft Loop 1 gap proposal derived from a failed calibration outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposedGapEntry {
    pub code: String,
    pub source: String,
    pub utterance: String,
    pub entity_type: String,
    pub entity_state: String,
    pub target_verb: String,
    pub actual_halt_reason: Option<String>,
}

/// Draft Loop 2 clarification suggestion derived from an ambiguous outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuggestedClarification {
    pub trigger_phrase: String,
    pub verb_a: String,
    pub verb_b: String,
    pub suggested_prompt: String,
}

/// Portable bundle for one scenario and its curated/generated utterances.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationScenarioBundle {
    pub exported_at: DateTime<Utc>,
    pub scenario: CalibrationScenario,
    pub utterances: Vec<CalibrationUtteranceReviewRow>,
}

/// Summary of calibration write-through into live learning stores.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CalibrationWriteThroughSummary {
    pub loop1_candidates_upserted: usize,
    pub loop2_phrases_upserted: usize,
    pub loop2_blocklist_upserts: usize,
}

//! Intent telemetry — append-only event stream from the orchestrator.
//!
//! Only the orchestrator writes `intent_events`. Telemetry is best-effort
//! and never drives execution.

pub mod redaction;
pub mod store;

pub use redaction::{normalize_utterance, preview_redacted, utterance_hash};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Read-only state-reachability observation for one candidate verb (eval mode).
///
/// Produced by `verb_surface::observe_state_reachability` over the ranked /
/// allowed set. NON-MUTATING: it never removes a candidate or changes ranking —
/// it only tags whether Step-5 lifecycle reachability *would* admit the verb at
/// the current entity state. This is the counterfactual the Option A vs B fork
/// is decided on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateObservation {
    pub verb: String,
    pub state_reachable: bool,
    /// The lifecycle predicate that fails when `state_reachable == false`
    /// (e.g. "requires_states [\"open\"], current 'closed'").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failing_predicate: Option<String>,
}

/// Soft-stage candidate flow derived from the FINAL search results — the count
/// of surviving candidates bucketed by search source and ordinal tier.
///
/// Derived from `Vec<VerbSearchResult>` (see `verb_search::soft_stage_flow`),
/// so it observes search output without instrumenting (and risking) the search
/// body itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SoftStageFlow {
    /// Surviving candidate count by `VerbSearchSource` label.
    pub by_source: BTreeMap<String, usize>,
    /// Surviving candidate count by ordinal `Tier` label.
    pub by_tier: BTreeMap<String, usize>,
    pub total: usize,
}

/// Row model mirroring `"ob-poc".intent_events`.
#[derive(Debug, Clone)]
pub struct IntentEventRow {
    pub event_id: Uuid,
    pub session_id: Uuid,
    pub actor_id: String,
    pub entrypoint: String,

    pub utterance_hash: String,
    pub utterance_preview: Option<String>,
    pub scope: Option<String>,

    pub subject_ref_type: Option<String>,
    pub subject_ref_id: Option<Uuid>,

    pub semreg_mode: String,
    pub semreg_denied_verbs: Option<JsonValue>,

    pub verb_candidates_pre: Option<JsonValue>,
    pub verb_candidates_post: Option<JsonValue>,

    pub chosen_verb_fqn: Option<String>,
    pub selection_source: Option<String>,
    pub forced_verb_fqn: Option<String>,

    pub outcome: String,
    pub dsl_hash: Option<String>,
    pub run_sheet_entry_id: Option<Uuid>,

    pub macro_semreg_checked: bool,
    pub macro_denied_verbs: Option<JsonValue>,

    pub prompt_version: Option<String>,
    pub error_code: Option<String>,

    pub dominant_entity_id: Option<Uuid>,
    pub dominant_entity_kind: Option<String>,
    pub entity_kind_filtered: bool,

    pub allowed_verbs_fingerprint: Option<String>,
    pub pruned_verbs_count: i32,

    pub toctou_recheck_performed: bool,
    pub toctou_result: Option<String>,
    pub toctou_new_fingerprint: Option<String>,

    // ── Intent Trace evidence (Option C) ───────────────────────────
    /// Verb registry size before pack-scope collapse (FilterSummary.total_registry).
    pub surface_full_count: Option<i32>,
    /// Verb count after pack-scope collapse (FilterSummary.after_semreg).
    pub surface_pack_scoped_count: Option<i32>,
    /// Candidate flow bucketed by source/tier among the final results.
    pub soft_stage_flow: Option<JsonValue>,
    /// Eval-mode read-only state reachability tags over ranked/allowed.
    pub state_observer: Option<JsonValue>,
    /// Entity/context resolution confidence.
    pub entity_confidence: Option<f32>,
}

/// Map a PipelineOutcome to its telemetry string label.
pub fn outcome_label(outcome: &crate::mcp::intent_pipeline::PipelineOutcome) -> &'static str {
    use crate::mcp::intent_pipeline::PipelineOutcome;
    match outcome {
        PipelineOutcome::Ready => "ready",
        PipelineOutcome::NeedsUserInput => "needs_user_input",
        PipelineOutcome::NeedsClarification => "needs_clarification",
        PipelineOutcome::NoMatch => "no_match",
        PipelineOutcome::SemanticNotReady => "semantic_not_ready",
        PipelineOutcome::ScopeResolved { .. } => "scope_resolved",
        PipelineOutcome::ScopeCandidates => "scope_candidates",
        PipelineOutcome::NoAllowedVerbs => "no_allowed_verbs",
    }
}

/// Convert verb candidates to a compact JSON array: [["verb", score], ...]
pub fn candidates_to_json(candidates: &[(String, f32)]) -> Option<JsonValue> {
    if candidates.is_empty() {
        return None;
    }
    let arr: Vec<JsonValue> = candidates
        .iter()
        .take(10) // cap at top 10 for storage
        .map(|(v, s)| serde_json::json!([v, s]))
        .collect();
    Some(JsonValue::Array(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidates_to_json_empty() {
        assert!(candidates_to_json(&[]).is_none());
    }

    #[test]
    fn test_candidates_to_json_basic() {
        let cands = vec![("cbu.create".to_string(), 0.95f32)];
        let json = candidates_to_json(&cands).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0][0], "cbu.create");
    }

    #[test]
    fn test_candidates_to_json_caps_at_10() {
        let cands: Vec<(String, f32)> = (0..20).map(|i| (format!("v.{}", i), 0.5)).collect();
        let json = candidates_to_json(&cands).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 10);
    }

    #[test]
    fn test_outcome_label_coverage() {
        use crate::mcp::intent_pipeline::PipelineOutcome;
        assert_eq!(outcome_label(&PipelineOutcome::Ready), "ready");
        assert_eq!(
            outcome_label(&PipelineOutcome::NeedsClarification),
            "needs_clarification"
        );
        assert_eq!(
            outcome_label(&PipelineOutcome::NoAllowedVerbs),
            "no_allowed_verbs"
        );
    }
}

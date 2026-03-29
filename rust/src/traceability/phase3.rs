//! Phase 3 candidate-set contract helpers.

use std::collections::HashSet;

use crate::mcp::verb_search::VerbSearchResult;
use crate::traceability::Phase2Evaluation;

/// Result of enforcing the Phase 2 legality ceiling on a Phase 3 candidate set.
#[derive(Debug, Clone)]
pub struct Phase3SubsetResult {
    pub retained_candidates: Vec<VerbSearchResult>,
    pub eliminated_candidates: Vec<VerbSearchResult>,
}

impl Phase3SubsetResult {
    /// True when at least one candidate had to be removed for violating the Phase 2 ceiling.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase3SubsetResult;
    ///
    /// let result = Phase3SubsetResult {
    ///     retained_candidates: vec![],
    ///     eliminated_candidates: vec![],
    /// };
    /// assert!(!result.had_violation());
    /// ```
    pub fn had_violation(&self) -> bool {
        !self.eliminated_candidates.is_empty()
    }
}

/// Evaluated Phase 3 result for a single turn.
#[derive(Debug, Clone)]
pub struct Phase3Evaluation {
    pub subset_result: Phase3SubsetResult,
    pub filter_name: &'static str,
}

impl Phase3Evaluation {
    /// Create a new Phase 3 evaluation wrapper.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase3Evaluation, Phase3SubsetResult};
    ///
    /// let evaluation = Phase3Evaluation::new(
    ///     Phase3SubsetResult {
    ///         retained_candidates: vec![],
    ///         eliminated_candidates: vec![],
    ///     },
    ///     "phase2_legal_ceiling",
    /// );
    /// assert_eq!(evaluation.filter_name, "phase2_legal_ceiling");
    /// ```
    pub fn new(subset_result: Phase3SubsetResult, filter_name: &'static str) -> Self {
        Self {
            subset_result,
            filter_name,
        }
    }

    /// Build the persisted Phase 3 payload from this evaluated result.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Service, Phase3Evaluation};
    ///
    /// let evaluation = Phase3Evaluation::new(
    ///     ob_poc::traceability::enforce_phase2_evaluation_subset(
    ///         vec![],
    ///         &Phase2Service::evaluate(None, None),
    ///     ),
    ///     "phase2_legal_ceiling",
    /// );
    /// assert_eq!(evaluation.payload()["status"], "available");
    /// ```
    pub fn payload(&self) -> serde_json::Value {
        build_phase3_payload(&self.subset_result, self.filter_name)
    }

    /// Build the persisted Phase 3 payload or an explicit unavailable placeholder.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Service, Phase3Evaluation};
    ///
    /// let evaluation = Phase3Evaluation::new(
    ///     ob_poc::traceability::enforce_phase2_evaluation_subset(
    ///         vec![],
    ///         &Phase2Service::evaluate(None, None),
    ///     ),
    ///     "phase2_legal_ceiling",
    /// );
    /// assert_eq!(
    ///     evaluation.payload_or_unavailable("example")["status"],
    ///     "available"
    /// );
    /// ```
    pub fn payload_or_unavailable(&self, entrypoint: &str) -> serde_json::Value {
        let _ = entrypoint;
        self.payload()
    }

    /// Return whether Phase 3 removed any candidates for legality reasons.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Service, Phase3Evaluation};
    ///
    /// let evaluation = Phase3Evaluation::new(
    ///     ob_poc::traceability::enforce_phase2_evaluation_subset(
    ///         vec![],
    ///         &Phase2Service::evaluate(None, None),
    ///     ),
    ///     "phase2_legal_ceiling",
    /// );
    /// assert!(!evaluation.had_violation());
    /// ```
    pub fn had_violation(&self) -> bool {
        self.subset_result.had_violation()
    }
}

/// Build a persisted Phase 3 trace payload from the subset-enforcement result.
///
/// # Examples
/// ```rust
/// use ob_poc::mcp::verb_search::{VerbSearchResult, VerbSearchSource};
/// use ob_poc::traceability::{build_phase3_payload, Phase3SubsetResult};
///
/// let result = Phase3SubsetResult {
///     retained_candidates: vec![VerbSearchResult {
///         verb: "kyc-case.create".to_string(),
///         score: 0.9,
///         source: VerbSearchSource::LearnedExact,
///         matched_phrase: "open case".to_string(),
///         description: None,
///         journey: None,
///     }],
///     eliminated_candidates: vec![],
/// };
/// let payload = build_phase3_payload(&result, "action_surface");
/// assert_eq!(payload["phase4_candidate_set"][0], "kyc-case.create");
/// ```
pub fn build_phase3_payload(result: &Phase3SubsetResult, filter_name: &str) -> serde_json::Value {
    serde_json::json!({
        "status": "available",
        "legal_set_in_count": result.retained_candidates.len() + result.eliminated_candidates.len(),
        "after_action_filter_count": result.retained_candidates.len(),
        "filter_chain": [{
            "filter_name": filter_name,
            "input_count": result.retained_candidates.len() + result.eliminated_candidates.len(),
            "output_count": result.retained_candidates.len(),
            "eliminated": result.eliminated_candidates.iter().map(|candidate| candidate.verb.clone()).collect::<Vec<_>>(),
            "filter_type": "hard_prune",
        }],
        "eliminated_candidates": result
            .eliminated_candidates
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "verb_id": candidate.verb,
                    "eliminated_by": "phase2_legal_ceiling",
                    "reason": "candidate outside phase2 legal set",
                })
            })
            .collect::<Vec<_>>(),
        "demoted_candidates": [],
        "retained_candidates": result
            .retained_candidates
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "verb_id": candidate.verb,
                    "composite_score": candidate.score,
                    "was_demoted": false,
                    "matched_phrase": candidate.matched_phrase,
                    "source": format!("{:?}", candidate.source),
                })
            })
            .collect::<Vec<_>>(),
        "phase4_candidate_set": result
            .retained_candidates
            .iter()
            .map(|candidate| candidate.verb.clone())
            .collect::<Vec<_>>(),
        "deterministic_resolution": !result.had_violation(),
    })
}

/// Build an unavailable Phase 3 placeholder for paths without explicit narrowing details.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::build_phase3_unavailable_payload;
///
/// let payload = build_phase3_unavailable_payload("repl_v2");
/// assert_eq!(payload["status"], "unavailable");
/// ```
pub fn build_phase3_unavailable_payload(entrypoint: &str) -> serde_json::Value {
    serde_json::json!({
        "status": "unavailable",
        "entrypoint": entrypoint,
        "legal_set_in_count": 0,
        "after_action_filter_count": 0,
        "filter_chain": [],
        "eliminated_candidates": [],
        "demoted_candidates": [],
        "retained_candidates": [],
        "phase4_candidate_set": [],
        "deterministic_resolution": false,
    })
}

/// Enforce the Phase 2 legal-verb ceiling on Phase 3 candidates.
///
/// If `legal_verbs` is `None`, this function preserves the input set unchanged.
///
/// # Examples
/// ```rust
/// use std::collections::HashSet;
/// use ob_poc::mcp::verb_search::{VerbSearchResult, VerbSearchSource};
/// use ob_poc::traceability::enforce_phase2_legal_subset;
///
/// let candidates = vec![VerbSearchResult {
///     verb: "kyc-case.create".to_string(),
///     score: 0.9,
///     source: VerbSearchSource::LearnedExact,
///     matched_phrase: "open case".to_string(),
///     description: None,
///     journey: None,
/// }];
/// let legal = HashSet::from(["kyc-case.create".to_string()]);
/// let result = enforce_phase2_legal_subset(candidates, Some(&legal));
/// assert_eq!(result.retained_candidates.len(), 1);
/// ```
pub fn enforce_phase2_legal_subset(
    candidates: Vec<VerbSearchResult>,
    legal_verbs: Option<&HashSet<String>>,
) -> Phase3SubsetResult {
    let Some(legal_verbs) = legal_verbs else {
        return Phase3SubsetResult {
            retained_candidates: candidates,
            eliminated_candidates: vec![],
        };
    };

    let (retained_candidates, eliminated_candidates): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|candidate| legal_verbs.contains(&candidate.verb));

    Phase3SubsetResult {
        retained_candidates,
        eliminated_candidates,
    }
}

/// Enforce the Phase 2 legal-verb ceiling using the evaluated Phase 2 result.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::{enforce_phase2_evaluation_subset, Phase2Service};
///
/// let evaluation = Phase2Service::evaluate(None, None);
/// let result = enforce_phase2_evaluation_subset(vec![], &evaluation);
/// assert_eq!(result.retained_candidates.len(), 0);
/// ```
pub fn enforce_phase2_evaluation_subset(
    candidates: Vec<VerbSearchResult>,
    phase2: &Phase2Evaluation,
) -> Phase3SubsetResult {
    enforce_phase2_legal_subset(candidates, phase2.legal_verbs_if_usable.as_ref())
}

/// Evaluate Phase 3 narrowing against the evaluated Phase 2 legality ceiling.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::{evaluate_phase3_against_phase2, Phase2Service};
///
/// let evaluation = evaluate_phase3_against_phase2(vec![], &Phase2Service::evaluate(None, None));
/// assert_eq!(evaluation.filter_name, "phase2_legal_ceiling");
/// ```
pub fn evaluate_phase3_against_phase2(
    candidates: Vec<VerbSearchResult>,
    phase2: &Phase2Evaluation,
) -> Phase3Evaluation {
    Phase3Evaluation::new(
        enforce_phase2_evaluation_subset(candidates, phase2),
        "phase2_legal_ceiling",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_phase3_payload, build_phase3_unavailable_payload, enforce_phase2_legal_subset,
        evaluate_phase3_against_phase2,
    };
    use crate::mcp::verb_search::{VerbSearchResult, VerbSearchSource};
    use crate::traceability::Phase2Service;
    use std::collections::HashSet;

    #[test]
    fn test_phase3_subset_prunes_out_of_ceiling_candidates() {
        let candidates = vec![
            VerbSearchResult {
                verb: "kyc-case.create".to_string(),
                score: 0.9,
                source: VerbSearchSource::LearnedExact,
                matched_phrase: "open case".to_string(),
                description: None,
                journey: None,
            },
            VerbSearchResult {
                verb: "deal.create".to_string(),
                score: 0.8,
                source: VerbSearchSource::LearnedExact,
                matched_phrase: "create deal".to_string(),
                description: None,
                journey: None,
            },
        ];
        let legal = HashSet::from(["kyc-case.create".to_string()]);

        let result = enforce_phase2_legal_subset(candidates, Some(&legal));

        assert_eq!(result.retained_candidates.len(), 1);
        assert_eq!(result.retained_candidates[0].verb, "kyc-case.create");
        assert_eq!(result.eliminated_candidates.len(), 1);
        assert_eq!(result.eliminated_candidates[0].verb, "deal.create");
    }

    #[test]
    fn test_phase3_payload_records_pruned_candidates() {
        let candidates = vec![
            VerbSearchResult {
                verb: "kyc-case.create".to_string(),
                score: 0.9,
                source: VerbSearchSource::LearnedExact,
                matched_phrase: "open case".to_string(),
                description: None,
                journey: None,
            },
            VerbSearchResult {
                verb: "deal.create".to_string(),
                score: 0.8,
                source: VerbSearchSource::LearnedExact,
                matched_phrase: "create deal".to_string(),
                description: None,
                journey: None,
            },
        ];
        let legal = HashSet::from(["kyc-case.create".to_string()]);
        let result = enforce_phase2_legal_subset(candidates, Some(&legal));
        let payload = build_phase3_payload(&result, "action_surface");

        assert_eq!(payload["phase4_candidate_set"][0], "kyc-case.create");
        assert_eq!(
            payload["eliminated_candidates"][0]["verb_id"],
            "deal.create"
        );
    }

    #[test]
    fn test_phase3_unavailable_payload_marks_status() {
        let payload = build_phase3_unavailable_payload("agent_service_direct");
        assert_eq!(payload["status"], "unavailable");
    }

    #[test]
    fn test_phase3_evaluation_wraps_subset_payload() {
        let evaluation =
            evaluate_phase3_against_phase2(vec![], &Phase2Service::evaluate(None, None));

        assert_eq!(evaluation.filter_name, "phase2_legal_ceiling");
        assert_eq!(evaluation.payload()["status"], "available");
        assert!(!evaluation.had_violation());
    }
}

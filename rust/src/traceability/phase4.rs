//! Phase 4 resolution and fallback helpers.

use std::collections::HashSet;

use serde_json::json;

use crate::traceability::Phase2Evaluation;

/// Evaluated Phase 4 result for a single turn.
#[derive(Debug, Clone)]
pub struct Phase4Evaluation {
    pub resolved_verb: Option<String>,
    pub candidates_in: Vec<String>,
    pub resolution_strategy: String,
    pub confidence: f32,
    pub fallback_reason: Option<String>,
    pub legality_violation: Option<&'static str>,
}

impl Phase4Evaluation {
    /// Create a new Phase 4 evaluation wrapper.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase4Evaluation;
    ///
    /// let evaluation = Phase4Evaluation::new(None, vec![], "failed", 0.0, None, None);
    /// assert_eq!(evaluation.resolution_strategy, "failed");
    /// ```
    pub fn new(
        resolved_verb: Option<String>,
        candidates_in: Vec<String>,
        resolution_strategy: impl Into<String>,
        confidence: f32,
        fallback_reason: Option<String>,
        legality_violation: Option<&'static str>,
    ) -> Self {
        Self {
            resolved_verb,
            candidates_in,
            resolution_strategy: resolution_strategy.into(),
            confidence,
            fallback_reason,
            legality_violation,
        }
    }

    /// Build the persisted Phase 4 payload from this evaluated result.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase4Evaluation;
    ///
    /// let evaluation = Phase4Evaluation::new(
    ///     Some("kyc.open-case".to_string()),
    ///     vec!["kyc.open-case".to_string()],
    ///     "exact_match",
    ///     0.9,
    ///     None,
    ///     None,
    /// );
    /// assert_eq!(evaluation.payload()["resolved_verb"], "kyc.open-case");
    /// ```
    pub fn payload(&self) -> serde_json::Value {
        build_phase4_payload(
            self.resolved_verb.as_deref(),
            &self.candidates_in,
            &self.resolution_strategy,
            self.confidence,
            self.fallback_reason.as_deref(),
        )
    }

    /// Build the persisted Phase 4 payload or an explicit unavailable placeholder.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase4Evaluation;
    ///
    /// let evaluation = Phase4Evaluation::new(None, vec![], "failed", 0.0, None, None);
    /// assert_eq!(
    ///     evaluation.payload_or_unavailable("example")["status"],
    ///     "unavailable"
    /// );
    /// ```
    pub fn payload_or_unavailable(&self, entrypoint: &str) -> serde_json::Value {
        if self.is_unavailable() {
            build_phase4_unavailable_payload(entrypoint)
        } else {
            self.payload()
        }
    }

    /// Return whether Phase 4 invoked fallback widening.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase4Evaluation;
    ///
    /// let evaluation = Phase4Evaluation::new(None, vec![], "failed", 0.0, None, None);
    /// assert!(!evaluation.fallback_invoked());
    /// ```
    pub fn fallback_invoked(&self) -> bool {
        self.fallback_reason.is_some()
    }

    /// Return the normalized trace reason code for Phase 4 fallback.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase4Evaluation;
    ///
    /// let evaluation = Phase4Evaluation::new(
    ///     None,
    ///     vec![],
    ///     "failed",
    ///     0.0,
    ///     Some("pattern mismatch".to_string()),
    ///     None,
    /// );
    /// assert_eq!(
    ///     evaluation.fallback_reason_code_for_trace(),
    ///     Some("pattern_mismatch".to_string())
    /// );
    /// ```
    pub fn fallback_reason_code_for_trace(&self) -> Option<String> {
        fallback_reason_code_for_trace(self.fallback_reason.as_deref())
    }

    /// Return whether the Phase 4 evaluation has no candidate or resolved verb data.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase4Evaluation;
    ///
    /// let evaluation = Phase4Evaluation::new(None, vec![], "failed", 0.0, None, None);
    /// assert!(evaluation.is_unavailable());
    /// ```
    pub fn is_unavailable(&self) -> bool {
        self.candidates_in.is_empty() && self.resolved_verb.is_none()
    }
}

/// Build a Phase 4 trace payload from the resolved verb and candidate set.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::build_phase4_payload;
///
/// let payload = build_phase4_payload(
///     Some("kyc.open-case"),
///     &["kyc.open-case".to_string()],
///     "exact_match",
///     0.9,
///     None,
/// );
/// assert_eq!(payload["resolved_verb"], "kyc.open-case");
/// ```
pub fn build_phase4_payload(
    resolved_verb: Option<&str>,
    candidates_in: &[String],
    resolution_strategy: &str,
    confidence: f32,
    fallback_reason: Option<&str>,
) -> serde_json::Value {
    let fallback_invoked = fallback_reason.is_some();
    let alternatives = candidates_in
        .iter()
        .filter(|candidate| Some(candidate.as_str()) != resolved_verb)
        .cloned()
        .collect::<Vec<_>>();

    json!({
        "status": if resolved_verb.is_some() { "resolved" } else { "failed" },
        "candidates_in": candidates_in,
        "candidate_count_in": candidates_in.len(),
        "resolution_strategy": normalized_resolution_strategy(
            resolution_strategy,
            fallback_invoked,
            resolved_verb,
        ),
        "resolution_strategy_detail": resolution_strategy,
        "resolved_verb": resolved_verb,
        "alternative_verbs": alternatives,
        "confidence": confidence,
        "fallback_invoked": fallback_invoked,
        "dsl_command": serde_json::Value::Null,
        "requires_confirmation": serde_json::Value::Null,
        "confirmation_reason": serde_json::Value::Null,
        "fallback_escape": fallback_reason.map(|reason| {
            json!({
                "reason": reason,
                "reason_code": fallback_reason_code(reason),
                "source_phase": 4,
                "widened_to": "phase2_legal_set",
                "resolution_from_widened": resolved_verb,
                "widened_strategy": widened_strategy_label(resolution_strategy),
            })
        }),
    })
}

/// Build an unavailable placeholder for paths without Phase 4 resolution details.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::build_phase4_unavailable_payload;
///
/// let payload = build_phase4_unavailable_payload("repl_v2");
/// assert_eq!(payload["status"], "unavailable");
/// ```
pub fn build_phase4_unavailable_payload(entrypoint: &str) -> serde_json::Value {
    json!({
        "status": "unavailable",
        "entrypoint": entrypoint,
        "candidates_in": [],
        "candidate_count_in": 0,
        "resolution_strategy": "unavailable",
        "resolution_strategy_detail": "unavailable",
        "resolved_verb": serde_json::Value::Null,
        "alternative_verbs": [],
        "confidence": serde_json::Value::Null,
        "fallback_invoked": false,
        "fallback_escape": serde_json::Value::Null,
    })
}

/// Enforce that a resolved Phase 4 verb remains within the Phase 2 legal set.
///
/// # Examples
/// ```rust
/// use std::collections::HashSet;
/// use ob_poc::traceability::enforce_phase4_resolution_within_phase2;
///
/// let legal = HashSet::from(["kyc.open-case".to_string()]);
/// assert_eq!(
///     enforce_phase4_resolution_within_phase2(Some("deal.create"), Some(&legal)),
///     Some("phase4_widened_outside_phase2")
/// );
/// ```
pub fn enforce_phase4_resolution_within_phase2(
    resolved_verb: Option<&str>,
    legal_verbs: Option<&HashSet<String>>,
) -> Option<&'static str> {
    let resolved_verb = resolved_verb?;
    let legal_verbs = legal_verbs?;

    if legal_verbs.contains(resolved_verb) {
        None
    } else {
        Some("phase4_widened_outside_phase2")
    }
}

/// Enforce that a resolved Phase 4 verb remains within the evaluated Phase 2 legal set.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::{enforce_phase4_resolution_within_evaluation, Phase2Service};
///
/// let evaluation = Phase2Service::evaluate(None, None);
/// assert_eq!(
///     enforce_phase4_resolution_within_evaluation(Some("deal.create"), &evaluation),
///     None
/// );
/// ```
pub fn enforce_phase4_resolution_within_evaluation(
    resolved_verb: Option<&str>,
    phase2: &Phase2Evaluation,
) -> Option<&'static str> {
    enforce_phase4_resolution_within_phase2(resolved_verb, phase2.legal_verbs_if_usable.as_ref())
}

/// Evaluate Phase 4 resolution against the Phase 2 legality ceiling.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::{evaluate_phase4_within_phase2, Phase2Service};
///
/// let evaluation = evaluate_phase4_within_phase2(
///     None,
///     vec![],
///     "failed",
///     0.0,
///     None,
///     &Phase2Service::evaluate(None, None),
/// );
/// assert_eq!(evaluation.resolved_verb, None);
/// ```
pub fn evaluate_phase4_within_phase2(
    resolved_verb: Option<String>,
    candidates_in: Vec<String>,
    resolution_strategy: impl Into<String>,
    confidence: f32,
    fallback_reason: Option<String>,
    phase2: &Phase2Evaluation,
) -> Phase4Evaluation {
    let legality_violation =
        enforce_phase4_resolution_within_evaluation(resolved_verb.as_deref(), phase2);
    Phase4Evaluation::new(
        resolved_verb,
        candidates_in,
        resolution_strategy,
        confidence,
        fallback_reason,
        legality_violation,
    )
}

/// Return the normalized fallback reason code used by persisted traces.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::fallback_reason_code_for_trace;
///
/// assert_eq!(
///     fallback_reason_code_for_trace(Some("pattern mismatch forced fallback")),
///     Some("pattern_mismatch")
/// );
/// ```
pub fn fallback_reason_code_for_trace(reason: Option<&str>) -> Option<String> {
    reason.map(|value| fallback_reason_code(value).to_string())
}

fn fallback_reason_code(reason: &str) -> &'static str {
    let reason = reason.to_ascii_lowercase();
    if reason.contains("pattern") {
        "pattern_mismatch"
    } else if reason.contains("taxonomy") {
        "taxonomy_over_demotion"
    } else if reason.contains("concept") {
        "concept_coverage_gap"
    } else if reason.contains("embedding") {
        "embedding_collision"
    } else if reason.contains("prune") {
        "action_category_over_prune"
    } else {
        "unknown"
    }
}

fn normalized_resolution_strategy(
    strategy: &str,
    fallback_invoked: bool,
    resolved_verb: Option<&str>,
) -> &'static str {
    if fallback_invoked {
        return "fallback_widened";
    }

    let strategy = strategy.to_ascii_lowercase();
    if strategy.contains("exact") || strategy.contains("user_choice") || strategy == "semreg" {
        "exact_match"
    } else if strategy.contains("concept") {
        "concept_match"
    } else if strategy.contains("embedding") {
        "embedding_similarity"
    } else if strategy.contains("llm")
        || strategy.contains("coder")
        || strategy.contains("sage")
        || strategy.contains("delegate")
    {
        "llm_disambiguation"
    } else if resolved_verb.is_some() {
        "exact_match"
    } else {
        "failed"
    }
}

fn widened_strategy_label(strategy: &str) -> &'static str {
    let strategy = strategy.to_ascii_lowercase();
    if strategy.contains("coder") || strategy.contains("sage") || strategy.contains("llm") {
        "llm_disambiguation"
    } else if strategy.contains("embedding") {
        "embedding_similarity"
    } else if strategy.contains("concept") {
        "concept_match"
    } else {
        "legacy_pipeline"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_phase4_payload, enforce_phase4_resolution_within_phase2,
        evaluate_phase4_within_phase2, fallback_reason_code_for_trace,
    };
    use crate::traceability::Phase2Service;
    use std::collections::HashSet;

    #[test]
    fn test_phase4_payload_captures_fallback() {
        let payload = build_phase4_payload(
            Some("kyc.open-case"),
            &["kyc.open-case".to_string(), "deal.create".to_string()],
            "fallback_widened",
            0.82,
            Some("pattern mismatch forced fallback"),
        );

        assert_eq!(payload["resolved_verb"], "kyc.open-case");
        assert_eq!(payload["fallback_invoked"], true);
        assert_eq!(payload["resolution_strategy"], "fallback_widened");
        assert_eq!(payload["resolution_strategy_detail"], "fallback_widened");
        assert_eq!(
            payload["fallback_escape"]["reason_code"],
            "pattern_mismatch"
        );
    }

    #[test]
    fn test_phase4_payload_normalizes_llm_resolution() {
        let payload = build_phase4_payload(
            Some("kyc.open-case"),
            &["kyc.open-case".to_string()],
            "sage_serve_coder",
            0.91,
            None,
        );

        assert_eq!(payload["resolution_strategy"], "llm_disambiguation");
        assert_eq!(payload["candidate_count_in"], 1);
        assert_eq!(payload["fallback_invoked"], false);
    }

    #[test]
    fn test_phase4_guard_rejects_outside_phase2() {
        let legal = HashSet::from(["kyc.open-case".to_string()]);
        assert_eq!(
            enforce_phase4_resolution_within_phase2(Some("deal.create"), Some(&legal)),
            Some("phase4_widened_outside_phase2")
        );
    }

    #[test]
    fn test_phase4_reason_code_helper() {
        assert_eq!(
            fallback_reason_code_for_trace(Some("concept coverage gap")),
            Some("concept_coverage_gap".to_string())
        );
        assert_eq!(fallback_reason_code_for_trace(None), None);
    }

    #[test]
    fn test_phase4_evaluation_exposes_fallback_metadata() {
        let evaluation = evaluate_phase4_within_phase2(
            Some("kyc.open-case".to_string()),
            vec!["kyc.open-case".to_string()],
            "fallback_widened",
            0.82,
            Some("pattern mismatch forced fallback".to_string()),
            &Phase2Service::evaluate(None, None),
        );

        assert!(evaluation.fallback_invoked());
        assert_eq!(
            evaluation.fallback_reason_code_for_trace(),
            Some("pattern_mismatch".to_string())
        );
        assert!(!evaluation.is_unavailable());
    }
}

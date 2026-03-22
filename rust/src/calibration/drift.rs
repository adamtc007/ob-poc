//! Run-over-run drift computation.

use std::collections::HashMap;

use super::types::{CalibrationDrift, CalibrationOutcome, CalibrationRun, CalibrationVerdict};

/// Compute drift between a prior run and a current run.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::calibration::{compute_drift, CalibrationRun};
///
/// # fn demo(prior: &CalibrationRun, current: &CalibrationRun) {
/// let _ = compute_drift(prior, current);
/// # }
/// ```
pub fn compute_drift(prior: &CalibrationRun, current: &CalibrationRun) -> CalibrationDrift {
    let prior_by_utterance = prior
        .outcomes
        .iter()
        .map(|outcome| (outcome.utterance_id, outcome))
        .collect::<HashMap<_, _>>();
    let current_by_utterance = current
        .outcomes
        .iter()
        .map(|outcome| (outcome.utterance_id, outcome))
        .collect::<HashMap<_, _>>();

    let mut newly_failing_utterances = Vec::new();
    let mut newly_passing_utterances = Vec::new();
    for (utterance_id, current_outcome) in &current_by_utterance {
        let Some(prior_outcome) = prior_by_utterance.get(utterance_id) else {
            continue;
        };
        let prior_pass = is_pass(prior_outcome);
        let current_pass = is_pass(current_outcome);
        if prior_pass && !current_pass {
            newly_failing_utterances.push(*utterance_id);
        } else if !prior_pass && current_pass {
            newly_passing_utterances.push(*utterance_id);
        }
    }

    CalibrationDrift {
        prior_run_id: prior.run_id,
        current_run_id: current.run_id,
        overall_accuracy_delta: current.metrics.overall_accuracy - prior.metrics.overall_accuracy,
        fallback_rate_delta: current.metrics.phase4_fallback_rate
            - prior.metrics.phase4_fallback_rate,
        avg_margin_delta: match (
            prior.metrics.phase4_avg_margin,
            current.metrics.phase4_avg_margin,
        ) {
            (Some(prior_margin), Some(current_margin)) => Some(current_margin - prior_margin),
            _ => None,
        },
        newly_failing_utterances,
        newly_passing_utterances,
    }
}

fn is_pass(outcome: &CalibrationOutcome) -> bool {
    matches!(
        outcome.verdict,
        CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }
    )
}

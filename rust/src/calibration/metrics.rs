//! Aggregate metrics for calibration runs.

use super::types::{
    CalibrationMetrics, CalibrationMode, CalibrationOutcome, CalibrationVerdict, NegativeType,
};

/// Compute aggregate metrics from classified outcomes.
///
/// # Examples
/// ```rust
/// use ob_poc::calibration::compute_metrics;
///
/// let metrics = compute_metrics(&[]);
/// assert_eq!(metrics.overall_accuracy, 0.0);
/// ```
pub fn compute_metrics(outcomes: &[CalibrationOutcome]) -> CalibrationMetrics {
    let total = outcomes.len() as f32;
    if total == 0.0 {
        return CalibrationMetrics::default();
    }

    let positives = outcomes
        .iter()
        .filter(|outcome| outcome.calibration_mode == CalibrationMode::Positive)
        .collect::<Vec<_>>();
    let negative_a = outcomes
        .iter()
        .filter(|outcome| outcome.negative_type == Some(NegativeType::TypeA))
        .collect::<Vec<_>>();
    let negative_b = outcomes
        .iter()
        .filter(|outcome| outcome.negative_type == Some(NegativeType::TypeB))
        .collect::<Vec<_>>();
    let boundaries = outcomes
        .iter()
        .filter(|outcome| outcome.calibration_mode == CalibrationMode::Boundary)
        .collect::<Vec<_>>();
    let passes = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.verdict,
                CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }
            )
        })
        .count() as f32;
    let fallback_count = outcomes
        .iter()
        .filter(|outcome| matches!(outcome.verdict, CalibrationVerdict::UnnecessaryFallback))
        .count() as f32;
    let fragile_boundary_count = boundaries
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.verdict,
                CalibrationVerdict::PassWithFragileMargin { .. }
            )
        })
        .count();
    let margins = outcomes
        .iter()
        .filter_map(|outcome| outcome.margin)
        .collect::<Vec<_>>();
    let avg_margin =
        (!margins.is_empty()).then(|| margins.iter().sum::<f32>() / margins.len() as f32);
    let latencies = outcomes
        .iter()
        .filter_map(|outcome| outcome.latency_total_ms)
        .collect::<Vec<_>>();
    let avg_latency = (!latencies.is_empty())
        .then(|| latencies.iter().sum::<i64>() as f32 / latencies.len() as f32);

    CalibrationMetrics {
        positive_hit_rate: ratio(
            positives
                .iter()
                .filter(|outcome| {
                    matches!(
                        outcome.verdict,
                        CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }
                    )
                })
                .count(),
            positives.len(),
        ),
        negative_type_a_rejection_rate: ratio(
            negative_a
                .iter()
                .filter(|outcome| {
                    !matches!(outcome.verdict, CalibrationVerdict::FalsePositive { .. })
                })
                .count(),
            negative_a.len(),
        ),
        negative_type_b_rejection_rate: ratio(
            negative_b
                .iter()
                .filter(|outcome| {
                    !matches!(outcome.verdict, CalibrationVerdict::FalsePositive { .. })
                })
                .count(),
            negative_b.len(),
        ),
        boundary_correct_rate: ratio(
            boundaries
                .iter()
                .filter(|outcome| {
                    matches!(
                        outcome.verdict,
                        CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }
                    )
                })
                .count(),
            boundaries.len(),
        ),
        overall_accuracy: passes / total,
        phase4_fallback_rate: fallback_count / total,
        phase4_avg_margin: avg_margin,
        fragile_boundary_count,
        avg_total_latency_ms: avg_latency,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::{
        CalibrationOutcome, ExpectedHaltReason, ExpectedOutcome, NegativeType,
    };
    use uuid::Uuid;

    fn sample_outcome(
        mode: CalibrationMode,
        negative_type: Option<NegativeType>,
        verdict: CalibrationVerdict,
        margin: Option<f32>,
    ) -> CalibrationOutcome {
        CalibrationOutcome {
            utterance_id: Uuid::new_v4(),
            utterance_text: "demo".into(),
            calibration_mode: mode,
            negative_type,
            pre_screen: None,
            expected_outcome: ExpectedOutcome::ResolvesTo("entity.read".into()),
            trace_id: Uuid::new_v4(),
            actual_resolved_verb: Some("entity.read".into()),
            actual_halt_reason: None,
            verdict,
            failure_phase: None,
            failure_detail: None,
            top1_score: Some(0.9),
            top2_score: None,
            margin,
            margin_stable: margin.map(|value| value >= 0.08),
            latency_total_ms: Some(10),
            latency_per_phase: None,
        }
    }

    #[test]
    fn compute_metrics_handles_mixed_outcomes() {
        let outcomes = vec![
            sample_outcome(
                CalibrationMode::Positive,
                None,
                CalibrationVerdict::Pass,
                Some(0.2),
            ),
            sample_outcome(
                CalibrationMode::Negative,
                Some(NegativeType::TypeA),
                CalibrationVerdict::Pass,
                Some(0.1),
            ),
            sample_outcome(
                CalibrationMode::Negative,
                Some(NegativeType::TypeB),
                CalibrationVerdict::FalsePositive {
                    unexpected_verb: "entity.update".into(),
                    expected_halt: ExpectedHaltReason::NoViableVerb,
                },
                Some(0.01),
            ),
            sample_outcome(
                CalibrationMode::Boundary,
                None,
                CalibrationVerdict::PassWithFragileMargin {
                    margin: 0.01,
                    threshold: 0.08,
                },
                Some(0.01),
            ),
        ];

        let metrics = compute_metrics(&outcomes);
        assert_eq!(metrics.fragile_boundary_count, 1);
        assert!(metrics.overall_accuracy > 0.0);
        assert!(metrics.phase4_avg_margin.is_some());
    }
}

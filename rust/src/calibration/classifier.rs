//! Outcome classification for persisted utterance traces.

use crate::traceability::{TraceOutcome, UtteranceTraceRecord};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{
    CalibrationMode, CalibrationOutcome, CalibrationScenario, CalibrationVerdict,
    EmbeddingPreScreen, ExpectedHaltReason, ExpectedOutcome, NegativeType,
};

/// Minimal row needed to classify one calibration utterance execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationUtteranceRow {
    pub utterance_id: Uuid,
    pub text: String,
    pub calibration_mode: CalibrationMode,
    pub negative_type: Option<NegativeType>,
    pub expected_outcome: ExpectedOutcome,
    pub pre_screen: Option<EmbeddingPreScreen>,
}

/// Classify one persisted trace against the expected calibration outcome.
///
/// # Examples
/// ```rust
/// use ob_poc::calibration::{classify_outcome, CalibrationExecutionShape, CalibrationMode, CalibrationScenario, CalibrationUtteranceRow, ExpectedOutcome, GovernanceStatus};
/// use ob_poc::traceability::{SurfaceVersions, TraceKind, TraceOutcome, UtteranceTraceRecord};
/// use chrono::Utc;
/// use uuid::Uuid;
///
/// let scenario = CalibrationScenario {
///     scenario_id: Uuid::nil(),
///     scenario_name: "demo".into(),
///     created_by: "test".into(),
///     governance_status: GovernanceStatus::Draft,
///     constellation_template_id: "struct.demo".into(),
///     constellation_template_version: "v1".into(),
///     situation_signature: "entity:ACTIVE".into(),
///     situation_signature_hash: Some(1),
///     operational_phase: "Active".into(),
///     target_entity_type: "entity".into(),
///     target_entity_state: "ACTIVE".into(),
///     linked_entity_states: vec![],
///     target_verb: "entity.read".into(),
///     legal_verb_set_snapshot: vec!["entity.read".into()],
///     verb_taxonomy_tag: "read".into(),
///     excluded_neighbours: vec![],
///     near_neighbour_verbs: vec![],
///     expected_margin_threshold: 0.1,
///     execution_shape: CalibrationExecutionShape::Singleton,
///     gold_utterances: vec![],
///     admitted_synthetic_set_id: None,
/// };
/// let trace = UtteranceTraceRecord {
///     trace_id: Uuid::nil(),
///     utterance_id: Uuid::nil(),
///     session_id: Uuid::nil(),
///     correlation_id: None,
///     trace_kind: TraceKind::Original,
///     parent_trace_id: None,
///     timestamp: Utc::now(),
///     raw_utterance: "show entity".into(),
///     is_synthetic: true,
///     outcome: TraceOutcome::ExecutedSuccessfully,
///     halt_reason_code: None,
///     halt_phase: None,
///     resolved_verb: Some("entity.read".into()),
///     plane: None,
///     polarity: None,
///     execution_shape_kind: None,
///     fallback_invoked: false,
///     fallback_reason_code: None,
///     situation_signature_hash: Some(1),
///     template_id: None,
///     template_version: None,
///     surface_versions: SurfaceVersions::default(),
///     trace_payload: serde_json::json!({"phase_4":{"confidence":0.9}}),
/// };
/// let utterance = CalibrationUtteranceRow {
///     utterance_id: Uuid::nil(),
///     text: "show entity".into(),
///     calibration_mode: CalibrationMode::Positive,
///     negative_type: None,
///     expected_outcome: ExpectedOutcome::ResolvesTo("entity.read".into()),
///     pre_screen: None,
/// };
/// let outcome = classify_outcome(&trace, &utterance, &scenario);
/// assert!(matches!(outcome.verdict, CalibrationVerdict::Pass));
/// ```
pub fn classify_outcome(
    trace: &UtteranceTraceRecord,
    utterance: &CalibrationUtteranceRow,
    scenario: &CalibrationScenario,
) -> CalibrationOutcome {
    let actual_verb = trace.resolved_verb.clone();
    let actual_halt = trace.halt_reason_code.clone();
    let halt_phase = trace.halt_phase.map(|value| value as u8);
    let top1_score = trace
        .trace_payload
        .pointer("/phase_4/confidence")
        .and_then(|value| value.as_f64())
        .map(|value| value as f32);
    let top2_score = None;
    let margin = match (top1_score, utterance.pre_screen.as_ref()) {
        (Some(top1), Some(pre_screen)) => Some(pre_screen.nearest_neighbour_distance - top1),
        _ => utterance
            .pre_screen
            .as_ref()
            .map(|pre_screen| pre_screen.margin),
    };

    let verdict = if trace.outcome == TraceOutcome::InProgress {
        CalibrationVerdict::FalseNegative {
            expected: "trace_incomplete".to_string(),
            actual_halt: "in_progress".to_string(),
        }
    } else {
        classify_expected_outcome(
            &utterance.expected_outcome,
            trace.outcome,
            actual_verb.as_deref(),
            actual_halt.as_deref(),
            halt_phase,
            trace.fallback_invoked,
            margin,
            scenario.expected_margin_threshold,
        )
    };

    CalibrationOutcome {
        utterance_id: utterance.utterance_id,
        utterance_text: utterance.text.clone(),
        calibration_mode: utterance.calibration_mode,
        negative_type: utterance.negative_type,
        pre_screen: utterance.pre_screen.clone(),
        expected_outcome: utterance.expected_outcome.clone(),
        trace_id: trace.trace_id,
        actual_resolved_verb: actual_verb,
        actual_halt_reason: actual_halt,
        verdict,
        failure_phase: halt_phase,
        failure_detail: None,
        top1_score,
        top2_score,
        margin,
        margin_stable: margin.map(|value| value >= scenario.expected_margin_threshold),
        latency_total_ms: extract_latency_ms(trace),
        latency_per_phase: extract_per_phase_latency(trace),
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_expected_outcome(
    expected: &ExpectedOutcome,
    actual_outcome: TraceOutcome,
    actual_verb: Option<&str>,
    actual_halt: Option<&str>,
    halt_phase: Option<u8>,
    fallback_invoked: bool,
    margin: Option<f32>,
    threshold: f32,
) -> CalibrationVerdict {
    let base = match expected {
        ExpectedOutcome::ResolvesTo(target) => match actual_verb {
            Some(verb) if verb == target => pass_or_fragile(margin, threshold),
            Some(verb) => CalibrationVerdict::WrongVerb {
                expected: target.clone(),
                actual: verb.to_string(),
            },
            None => CalibrationVerdict::FalseNegative {
                expected: target.clone(),
                actual_halt: actual_halt.unwrap_or_default().to_string(),
            },
        },
        ExpectedOutcome::ResolvesToOneOf(targets) => match actual_verb {
            Some(verb) if targets.iter().any(|target| target == verb) => {
                pass_or_fragile(margin, threshold)
            }
            Some(verb) => CalibrationVerdict::WrongVerb {
                expected: targets.join("|"),
                actual: verb.to_string(),
            },
            None => CalibrationVerdict::FalseNegative {
                expected: targets.join("|"),
                actual_halt: actual_halt.unwrap_or_default().to_string(),
            },
        },
        ExpectedOutcome::HaltsWithReason(expected_reason) => match actual_verb {
            Some(verb) => CalibrationVerdict::FalsePositive {
                unexpected_verb: verb.to_string(),
                expected_halt: *expected_reason,
            },
            None if halt_reason_matches(actual_halt, *expected_reason) => CalibrationVerdict::Pass,
            None => CalibrationVerdict::CorrectPhaseWrongReason {
                expected: *expected_reason,
                actual: actual_halt.unwrap_or_default().to_string(),
            },
        },
        ExpectedOutcome::HaltsAtPhase(expected_phase) => match halt_phase {
            Some(actual_phase) if actual_phase == *expected_phase => CalibrationVerdict::Pass,
            Some(actual_phase) => CalibrationVerdict::WrongPhase {
                expected_phase: *expected_phase,
                actual_phase,
            },
            None if actual_verb.is_some() => CalibrationVerdict::FalsePositive {
                unexpected_verb: actual_verb.unwrap_or_default().to_string(),
                expected_halt: ExpectedHaltReason::NoViableVerb,
            },
            None => CalibrationVerdict::Pass,
        },
        ExpectedOutcome::TriggersClarification => {
            if actual_outcome == TraceOutcome::ClarificationTriggered {
                CalibrationVerdict::Pass
            } else if let Some(verb) = actual_verb {
                CalibrationVerdict::FalsePositive {
                    unexpected_verb: verb.to_string(),
                    expected_halt: ExpectedHaltReason::AmbiguousResolution,
                }
            } else {
                CalibrationVerdict::CorrectPhaseWrongReason {
                    expected: ExpectedHaltReason::AmbiguousResolution,
                    actual: actual_halt.unwrap_or_default().to_string(),
                }
            }
        }
        ExpectedOutcome::FallsToSage => {
            if actual_verb.is_none() {
                CalibrationVerdict::Pass
            } else {
                CalibrationVerdict::FalsePositive {
                    unexpected_verb: actual_verb.unwrap_or_default().to_string(),
                    expected_halt: ExpectedHaltReason::NoParsableIntent,
                }
            }
        }
    };

    if matches!(base, CalibrationVerdict::Pass) && fallback_invoked {
        CalibrationVerdict::UnnecessaryFallback
    } else {
        base
    }
}

fn pass_or_fragile(margin: Option<f32>, threshold: f32) -> CalibrationVerdict {
    match margin {
        Some(value) if value < threshold => CalibrationVerdict::PassWithFragileMargin {
            margin: value,
            threshold,
        },
        _ => CalibrationVerdict::Pass,
    }
}

fn halt_reason_matches(actual: Option<&str>, expected: ExpectedHaltReason) -> bool {
    match expected {
        ExpectedHaltReason::AmbiguousResolution => actual == Some("ambiguous_entity"),
        ExpectedHaltReason::SemanticNotReady => actual == Some("semantic_not_ready"),
        ExpectedHaltReason::NoAllowedVerbs => actual == Some("no_allowed_verbs"),
        ExpectedHaltReason::NoMatch => actual == Some("no_match"),
        ExpectedHaltReason::NoParsableIntent => {
            actual == Some("no_match") || actual == Some("no_parsable_intent")
        }
        ExpectedHaltReason::NoViableVerb => {
            actual == Some("no_allowed_verbs") || actual == Some("no_match")
        }
        ExpectedHaltReason::StateConflict => actual == Some("state_conflict"),
        ExpectedHaltReason::ConstellationBlock => actual == Some("constellation_block"),
        ExpectedHaltReason::BelowConfidenceThreshold => {
            actual == Some("below_confidence_threshold")
        }
        ExpectedHaltReason::DagOrderingConflict => actual == Some("dag_ordering_conflict"),
        ExpectedHaltReason::ExclusionMakesPlanInfeasible => {
            actual == Some("exclusion_makes_plan_infeasible")
        }
        ExpectedHaltReason::MidPlanConstellationBlock => {
            actual == Some("mid_plan_constellation_block")
        }
        ExpectedHaltReason::MissingReferentialContext => {
            actual == Some("missing_referential_context")
        }
    }
}

fn extract_latency_ms(trace: &UtteranceTraceRecord) -> Option<i64> {
    let phase5 = trace.trace_payload.get("phase_5")?;
    let start = phase5.get("execution_start")?.as_str()?;
    let end = phase5.get("execution_end")?.as_str()?;
    let start = chrono::DateTime::parse_from_rfc3339(start).ok()?;
    let end = chrono::DateTime::parse_from_rfc3339(end).ok()?;
    Some((end - start).num_milliseconds())
}

fn extract_per_phase_latency(_trace: &UtteranceTraceRecord) -> Option<Vec<(u8, i64)>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::{CalibrationExecutionShape, CalibrationScenario, GovernanceStatus};
    use crate::traceability::{SurfaceVersions, TraceKind};
    use chrono::Utc;

    fn sample_scenario() -> CalibrationScenario {
        CalibrationScenario {
            scenario_id: Uuid::nil(),
            scenario_name: "demo".into(),
            created_by: "test".into(),
            governance_status: GovernanceStatus::Draft,
            constellation_template_id: "struct.demo".into(),
            constellation_template_version: "v1".into(),
            situation_signature: "entity:ACTIVE".into(),
            situation_signature_hash: Some(1),
            operational_phase: "Active".into(),
            target_entity_type: "entity".into(),
            target_entity_state: "ACTIVE".into(),
            linked_entity_states: vec![],
            target_verb: "entity.read".into(),
            legal_verb_set_snapshot: vec!["entity.read".into()],
            verb_taxonomy_tag: "read".into(),
            excluded_neighbours: vec![],
            near_neighbour_verbs: vec![],
            expected_margin_threshold: 0.08,
            execution_shape: CalibrationExecutionShape::Singleton,
            gold_utterances: vec![],
            admitted_synthetic_set_id: None,
        }
    }

    fn sample_trace(outcome: TraceOutcome, resolved_verb: Option<&str>) -> UtteranceTraceRecord {
        UtteranceTraceRecord {
            trace_id: Uuid::new_v4(),
            utterance_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            correlation_id: None,
            trace_kind: TraceKind::Original,
            parent_trace_id: None,
            timestamp: Utc::now(),
            raw_utterance: "show entity".into(),
            is_synthetic: true,
            outcome,
            halt_reason_code: None,
            halt_phase: None,
            resolved_verb: resolved_verb.map(ToOwned::to_owned),
            plane: None,
            polarity: None,
            execution_shape_kind: None,
            fallback_invoked: false,
            fallback_reason_code: None,
            situation_signature_hash: Some(1),
            template_id: None,
            template_version: None,
            surface_versions: SurfaceVersions::default(),
            trace_payload: serde_json::json!({"phase_4":{"confidence":0.9}}),
        }
    }

    #[test]
    fn classify_outcome_passes_matching_resolution() {
        let utterance = CalibrationUtteranceRow {
            utterance_id: Uuid::new_v4(),
            text: "show entity".into(),
            calibration_mode: CalibrationMode::Positive,
            negative_type: None,
            expected_outcome: ExpectedOutcome::ResolvesTo("entity.read".into()),
            pre_screen: None,
        };
        let outcome = classify_outcome(
            &sample_trace(TraceOutcome::ExecutedSuccessfully, Some("entity.read")),
            &utterance,
            &sample_scenario(),
        );
        assert!(matches!(outcome.verdict, CalibrationVerdict::Pass));
    }

    #[test]
    fn classify_outcome_handles_clarification_triggered() {
        let utterance = CalibrationUtteranceRow {
            utterance_id: Uuid::new_v4(),
            text: "do it".into(),
            calibration_mode: CalibrationMode::Boundary,
            negative_type: None,
            expected_outcome: ExpectedOutcome::TriggersClarification,
            pre_screen: None,
        };
        let outcome = classify_outcome(
            &sample_trace(TraceOutcome::ClarificationTriggered, None),
            &utterance,
            &sample_scenario(),
        );
        assert!(matches!(outcome.verdict, CalibrationVerdict::Pass));
    }
}

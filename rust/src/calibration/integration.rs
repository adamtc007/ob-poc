//! Loopback-calibration adapters for Loop 1 and Loop 2 outputs.

use super::types::{
    CalibrationOutcome, CalibrationScenario, CalibrationVerdict, ProposedGapEntry,
    SuggestedClarification,
};

/// Generate draft Loop 1 gap proposals from a scenario run.
///
/// # Examples
/// ```rust
/// use ob_poc::calibration::{generate_proposed_gaps, CalibrationOutcome, CalibrationScenario, CalibrationVerdict, ExpectedOutcome, GovernanceStatus, CalibrationExecutionShape};
/// use uuid::Uuid;
///
/// let scenario = CalibrationScenario {
///     scenario_id: Uuid::nil(),
///     scenario_name: "demo".into(),
///     created_by: "test".into(),
///     governance_status: GovernanceStatus::Draft,
///     constellation_template_id: "demo".into(),
///     constellation_template_version: "v1".into(),
///     situation_signature: "entity:ACTIVE".into(),
///     situation_signature_hash: Some(1),
///     operational_phase: "Active".into(),
///     target_entity_type: "entity".into(),
///     target_entity_state: "ACTIVE".into(),
///     linked_entity_states: vec![],
///     target_verb: "entity.read".into(),
///     legal_verb_set_snapshot: vec![],
///     verb_taxonomy_tag: "read".into(),
///     excluded_neighbours: vec![],
///     near_neighbour_verbs: vec![],
///     expected_margin_threshold: 0.1,
///     execution_shape: CalibrationExecutionShape::Singleton,
///     gold_utterances: vec![],
///     admitted_synthetic_set_id: None,
/// };
/// let outcomes = vec![CalibrationOutcome {
///     utterance_id: Uuid::nil(),
///     utterance_text: "show me the entity".into(),
///     calibration_mode: ob_poc::calibration::CalibrationMode::Positive,
///     negative_type: None,
///     pre_screen: None,
///     expected_outcome: ExpectedOutcome::ResolvesTo("entity.read".into()),
///     trace_id: Uuid::nil(),
///     actual_resolved_verb: None,
///     actual_halt_reason: Some("no_viable_verb".into()),
///     verdict: CalibrationVerdict::FalseNegative {
///         expected: "entity.read".into(),
///         actual_halt: "no_viable_verb".into(),
///     },
///     failure_phase: Some(4),
///     failure_detail: None,
///     top1_score: None,
///     top2_score: None,
///     margin: None,
///     margin_stable: None,
///     latency_total_ms: None,
///     latency_per_phase: None,
/// }];
/// let gaps = generate_proposed_gaps(&scenario, &outcomes);
/// assert_eq!(gaps.len(), 1);
/// ```
pub fn generate_proposed_gaps(
    scenario: &CalibrationScenario,
    outcomes: &[CalibrationOutcome],
) -> Vec<ProposedGapEntry> {
    outcomes
        .iter()
        .filter(|outcome| {
            matches!(outcome.verdict, CalibrationVerdict::FalseNegative { .. })
                && matches!(
                    outcome.actual_halt_reason.as_deref(),
                    Some("no_viable_verb" | "no_match" | "no_allowed_verbs")
                )
        })
        .map(|outcome| ProposedGapEntry {
            code: format!(
                "GAP.CAL.{}",
                scenario
                    .target_verb
                    .replace(['.', '_'], "-")
                    .to_ascii_uppercase()
            ),
            source: "loopback_calibration".to_string(),
            utterance: outcome.utterance_text.clone(),
            entity_type: scenario.target_entity_type.clone(),
            entity_state: scenario.target_entity_state.clone(),
            target_verb: scenario.target_verb.clone(),
            actual_halt_reason: outcome.actual_halt_reason.clone(),
        })
        .collect()
}

/// Generate draft Loop 2 clarification suggestions from a scenario run.
///
/// # Examples
/// ```rust
/// use ob_poc::calibration::{generate_suggested_clarifications, CalibrationOutcome, CalibrationScenario, CalibrationVerdict, ConfusionRisk, ExpectedOutcome, GovernanceStatus, CalibrationExecutionShape, NearNeighbourVerb};
/// use uuid::Uuid;
///
/// let scenario = CalibrationScenario {
///     scenario_id: Uuid::nil(),
///     scenario_name: "demo".into(),
///     created_by: "test".into(),
///     governance_status: GovernanceStatus::Draft,
///     constellation_template_id: "demo".into(),
///     constellation_template_version: "v1".into(),
///     situation_signature: "entity:ACTIVE".into(),
///     situation_signature_hash: Some(1),
///     operational_phase: "Active".into(),
///     target_entity_type: "entity".into(),
///     target_entity_state: "ACTIVE".into(),
///     linked_entity_states: vec![],
///     target_verb: "entity.read".into(),
///     legal_verb_set_snapshot: vec![],
///     verb_taxonomy_tag: "read".into(),
///     excluded_neighbours: vec![],
///     near_neighbour_verbs: vec![NearNeighbourVerb {
///         verb_id: "entity.update".into(),
///         expected_embedding_distance: 0.11,
///         confusion_risk: ConfusionRisk::High,
///         distinguishing_signals: vec!["readonly vs write".into()],
///     }],
///     expected_margin_threshold: 0.1,
///     execution_shape: CalibrationExecutionShape::Singleton,
///     gold_utterances: vec![],
///     admitted_synthetic_set_id: None,
/// };
/// let outcomes = vec![CalibrationOutcome {
///     utterance_id: Uuid::nil(),
///     utterance_text: "change the entity".into(),
///     calibration_mode: ob_poc::calibration::CalibrationMode::Boundary,
///     negative_type: None,
///     pre_screen: None,
///     expected_outcome: ExpectedOutcome::TriggersClarification,
///     trace_id: Uuid::nil(),
///     actual_resolved_verb: Some("entity.update".into()),
///     actual_halt_reason: None,
///     verdict: CalibrationVerdict::WrongVerb {
///         expected: "entity.read".into(),
///         actual: "entity.update".into(),
///     },
///     failure_phase: None,
///     failure_detail: None,
///     top1_score: None,
///     top2_score: None,
///     margin: Some(0.02),
///     margin_stable: Some(false),
///     latency_total_ms: None,
///     latency_per_phase: None,
/// }];
/// let suggestions = generate_suggested_clarifications(&scenario, &outcomes);
/// assert_eq!(suggestions.len(), 1);
/// ```
pub fn generate_suggested_clarifications(
    scenario: &CalibrationScenario,
    outcomes: &[CalibrationOutcome],
) -> Vec<SuggestedClarification> {
    let neighbour = scenario
        .near_neighbour_verbs
        .first()
        .map(|value| value.verb_id.clone())
        .unwrap_or_else(|| "unknown.neighbour".to_string());

    outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.expected_outcome,
                super::types::ExpectedOutcome::TriggersClarification
            ) || matches!(
                outcome.actual_halt_reason.as_deref(),
                Some("ambiguous_resolution")
            ) || matches!(outcome.margin_stable, Some(false))
                || matches!(outcome.verdict, CalibrationVerdict::WrongVerb { .. })
        })
        .map(|outcome| SuggestedClarification {
            trigger_phrase: outcome.utterance_text.clone(),
            verb_a: scenario.target_verb.clone(),
            verb_b: outcome
                .actual_resolved_verb
                .clone()
                .unwrap_or_else(|| neighbour.clone()),
            suggested_prompt: format!(
                "Did you want '{}' or '{}'?",
                scenario.target_verb,
                outcome
                    .actual_resolved_verb
                    .as_deref()
                    .unwrap_or(neighbour.as_str())
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{generate_proposed_gaps, generate_suggested_clarifications};
    use crate::calibration::{
        CalibrationExecutionShape, CalibrationMode, CalibrationOutcome, CalibrationScenario,
        CalibrationVerdict, ConfusionRisk, ExpectedOutcome, GovernanceStatus, NearNeighbourVerb,
    };

    fn scenario() -> CalibrationScenario {
        CalibrationScenario {
            scenario_id: Uuid::nil(),
            scenario_name: "demo".into(),
            created_by: "test".into(),
            governance_status: GovernanceStatus::Draft,
            constellation_template_id: "demo".into(),
            constellation_template_version: "v1".into(),
            situation_signature: "entity:ACTIVE".into(),
            situation_signature_hash: Some(1),
            operational_phase: "Active".into(),
            target_entity_type: "entity".into(),
            target_entity_state: "ACTIVE".into(),
            linked_entity_states: vec![],
            target_verb: "entity.read".into(),
            legal_verb_set_snapshot: vec![],
            verb_taxonomy_tag: "read".into(),
            excluded_neighbours: vec![],
            near_neighbour_verbs: vec![NearNeighbourVerb {
                verb_id: "entity.update".into(),
                expected_embedding_distance: 0.12,
                confusion_risk: ConfusionRisk::High,
                distinguishing_signals: vec!["read-vs-write".into()],
            }],
            expected_margin_threshold: 0.1,
            execution_shape: CalibrationExecutionShape::Singleton,
            gold_utterances: vec![],
            admitted_synthetic_set_id: None,
        }
    }

    #[test]
    fn generate_proposed_gaps_flags_false_negative_no_viable_verb() {
        let outcomes = vec![CalibrationOutcome {
            utterance_id: Uuid::nil(),
            utterance_text: "show entity".into(),
            calibration_mode: CalibrationMode::Positive,
            negative_type: None,
            pre_screen: None,
            expected_outcome: ExpectedOutcome::ResolvesTo("entity.read".into()),
            trace_id: Uuid::nil(),
            actual_resolved_verb: None,
            actual_halt_reason: Some("no_viable_verb".into()),
            verdict: CalibrationVerdict::FalseNegative {
                expected: "entity.read".into(),
                actual_halt: "no_viable_verb".into(),
            },
            failure_phase: Some(4),
            failure_detail: None,
            top1_score: None,
            top2_score: None,
            margin: None,
            margin_stable: None,
            latency_total_ms: None,
            latency_per_phase: None,
        }];
        assert_eq!(generate_proposed_gaps(&scenario(), &outcomes).len(), 1);
    }

    #[test]
    fn generate_suggested_clarifications_flags_unstable_boundary() {
        let outcomes = vec![CalibrationOutcome {
            utterance_id: Uuid::nil(),
            utterance_text: "change entity".into(),
            calibration_mode: CalibrationMode::Boundary,
            negative_type: None,
            pre_screen: None,
            expected_outcome: ExpectedOutcome::TriggersClarification,
            trace_id: Uuid::nil(),
            actual_resolved_verb: Some("entity.update".into()),
            actual_halt_reason: None,
            verdict: CalibrationVerdict::WrongVerb {
                expected: "entity.read".into(),
                actual: "entity.update".into(),
            },
            failure_phase: None,
            failure_detail: None,
            top1_score: None,
            top2_score: None,
            margin: Some(0.01),
            margin_stable: Some(false),
            latency_total_ms: None,
            latency_per_phase: None,
        }];
        assert_eq!(
            generate_suggested_clarifications(&scenario(), &outcomes).len(),
            1
        );
    }
}

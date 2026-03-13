//! Bridge helpers from legacy Sage output into canonical NLCI compiler input.

use uuid::Uuid;

use crate::sage::{OutcomeAction, OutcomeIntent, OutcomeStep};

use super::{
    BindingMode, CompilerInputEnvelope, IntentIdentifier, IntentParameter, IntentQualifier,
    IntentStep, IntentTarget, SemanticIr, SemanticStep, SemanticTarget, StructuredIntentPlan,
};

/// Build a canonical compiler input envelope from the current Sage outcome model.
///
/// # Examples
/// ```ignore
/// use ob_poc::sage::{IntentPolarity, ObservationPlane, OutcomeAction, OutcomeIntent, SageConfidence};
/// use ob_poc::semtaxonomy_v2::compiler_input_from_outcome_intent;
///
/// let outcome = OutcomeIntent {
///     summary: "Read the current CBU".to_string(),
///     plane: ObservationPlane::Instance,
///     polarity: IntentPolarity::Read,
///     domain_concept: "cbu".to_string(),
///     action: OutcomeAction::Read,
///     subject: None,
///     steps: vec![],
///     confidence: SageConfidence::High,
///     pending_clarifications: vec![],
///     hints: Default::default(),
///     explain: Default::default(),
///     coder_handoff: Default::default(),
/// };
/// let envelope = compiler_input_from_outcome_intent(
///     &outcome,
///     None,
///     None,
///     Some("cbu"),
///     Some("Current CBU"),
/// );
/// assert_eq!(envelope.structured_intent.steps.len(), 1);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn compiler_input_from_outcome_intent(
    outcome: &OutcomeIntent,
    session_id: Option<Uuid>,
    session_entity_id: Option<Uuid>,
    session_entity_kind: Option<&str>,
    session_entity_name: Option<&str>,
) -> CompilerInputEnvelope {
    let steps = if outcome.steps.is_empty() {
        vec![fallback_step(outcome)]
    } else {
        outcome.steps.clone()
    };

    let intent_steps = steps
        .iter()
        .map(|step| intent_step_from_legacy(step, outcome, session_entity_id))
        .collect();
    let semantic_steps = steps
        .iter()
        .map(|step| semantic_step_from_legacy(step, outcome, session_entity_id))
        .collect();

    CompilerInputEnvelope {
        structured_intent: StructuredIntentPlan {
            steps: intent_steps,
            composition: Some(if steps.len() > 1 {
                "sequential".to_string()
            } else {
                "single_step".to_string()
            }),
            data_flow: vec![],
        },
        semantic_ir: SemanticIr {
            steps: semantic_steps,
            composition: Some(if steps.len() > 1 {
                "sequential".to_string()
            } else {
                "single_step".to_string()
            }),
        },
        session_id: session_id.map(|id| id.to_string()),
        session_entity_id: session_entity_id.map(|id| id.to_string()),
        session_entity_kind: session_entity_kind.map(str::to_string),
        session_entity_name: session_entity_name.map(str::to_string),
    }
}

fn fallback_step(outcome: &OutcomeIntent) -> OutcomeStep {
    OutcomeStep {
        action: outcome.action.clone(),
        target: outcome.domain_concept.clone(),
        params: std::collections::HashMap::new(),
        notes: Some(outcome.summary.clone()),
    }
}

fn intent_step_from_legacy(
    step: &OutcomeStep,
    outcome: &OutcomeIntent,
    session_entity_id: Option<Uuid>,
) -> IntentStep {
    IntentStep {
        action: action_name(&step.action),
        entity: normalized_entity(step, outcome),
        target: target_from_legacy(outcome, session_entity_id),
        qualifiers: legacy_intent_qualifiers(step, outcome),
        parameters: step
            .params
            .iter()
            .map(|(name, value)| IntentParameter {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        confidence: outcome.confidence.as_str().to_string(),
    }
}

fn semantic_step_from_legacy(
    step: &OutcomeStep,
    outcome: &OutcomeIntent,
    session_entity_id: Option<Uuid>,
) -> SemanticStep {
    SemanticStep {
        action: action_name(&step.action),
        entity: normalized_entity(step, outcome),
        binding_mode: binding_mode_from_legacy(outcome, session_entity_id),
        target: semantic_target_from_legacy(outcome, session_entity_id),
        parameters: step
            .params
            .iter()
            .map(|(name, value)| IntentParameter {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        qualifiers: legacy_semantic_qualifiers(step, outcome),
    }
}

fn normalized_entity(step: &OutcomeStep, outcome: &OutcomeIntent) -> String {
    if !step.target.trim().is_empty() {
        step.target.clone()
    } else {
        outcome.domain_concept.clone()
    }
}

fn target_from_legacy(outcome: &OutcomeIntent, session_entity_id: Option<Uuid>) -> Option<IntentTarget> {
    if let Some(subject) = outcome.subject.as_ref() {
        return Some(IntentTarget {
        identifier: subject.uuid.map(|uuid| IntentIdentifier {
            value: uuid.to_string(),
            identifier_type: "uuid".to_string(),
        }),
        reference: Some(subject.mention.clone()),
        filter: None,
        });
    }

    session_entity_id.map(|uuid| IntentTarget {
        identifier: Some(IntentIdentifier {
            value: uuid.to_string(),
            identifier_type: "uuid".to_string(),
        }),
        reference: Some("current".to_string()),
        filter: None,
    })
}

fn semantic_target_from_legacy(
    outcome: &OutcomeIntent,
    session_entity_id: Option<Uuid>,
) -> Option<SemanticTarget> {
    if let Some(subject) = outcome.subject.as_ref() {
        return Some(SemanticTarget {
        subject_kind: subject
            .kind_hint
            .clone()
            .unwrap_or_else(|| outcome.domain_concept.clone()),
        identifier: subject.uuid.map(|uuid| uuid.to_string()),
        identifier_type: subject.uuid.map(|_| "uuid".to_string()),
        reference: Some(subject.mention.clone()),
        filter: None,
        });
    }

    session_entity_id.map(|uuid| SemanticTarget {
        subject_kind: outcome.domain_concept.clone(),
        identifier: Some(uuid.to_string()),
        identifier_type: Some("uuid".to_string()),
        reference: Some("current".to_string()),
        filter: None,
    })
}

fn binding_mode_from_legacy(outcome: &OutcomeIntent, session_entity_id: Option<Uuid>) -> BindingMode {
    if let Some(subject) = &outcome.subject {
        if subject.uuid.is_some() {
            return BindingMode::Identifier;
        }
        return BindingMode::SessionReference;
    }
    if session_entity_id.is_some() {
        return BindingMode::SessionReference;
    }
    BindingMode::Unbound
}

fn action_name(action: &OutcomeAction) -> String {
    action.as_str().to_string()
}

fn legacy_intent_qualifiers(step: &OutcomeStep, outcome: &OutcomeIntent) -> Vec<IntentQualifier> {
    let mut qualifiers = Vec::new();

    if !outcome.summary.trim().is_empty() {
        qualifiers.push(IntentQualifier {
            name: "legacy-summary".to_string(),
            value: outcome.summary.trim().to_string(),
        });
    }

    if let Some(notes) = step
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|notes| !notes.is_empty())
    {
        qualifiers.push(IntentQualifier {
            name: "legacy-notes".to_string(),
            value: notes.to_string(),
        });
    }

    qualifiers
}

fn legacy_semantic_qualifiers(
    step: &OutcomeStep,
    outcome: &OutcomeIntent,
) -> Vec<(String, String)> {
    legacy_intent_qualifiers(step, outcome)
        .into_iter()
        .map(|qualifier| (qualifier.name, qualifier.value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sage::{IntentPolarity, ObservationPlane, SageConfidence};

    #[test]
    fn bridge_builds_single_step_envelope_from_legacy_outcome() {
        let outcome = OutcomeIntent {
            summary: "Read the current CBU".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Read,
            domain_concept: "cbu".to_string(),
            action: OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::High,
            pending_clarifications: vec![],
            hints: Default::default(),
            explain: Default::default(),
            coder_handoff: Default::default(),
        };

        let envelope =
            compiler_input_from_outcome_intent(
                &outcome,
                None,
                Some(Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid uuid")),
                None,
                Some("cbu"),
                Some("Current CBU"),
            );

        assert_eq!(envelope.structured_intent.steps.len(), 1);
        assert_eq!(envelope.structured_intent.steps[0].action, "read");
        assert_eq!(
            envelope.structured_intent.steps[0].qualifiers,
            vec![
                IntentQualifier {
                    name: "legacy-summary".to_string(),
                    value: "Read the current CBU".to_string(),
                },
                IntentQualifier {
                    name: "legacy-notes".to_string(),
                    value: "Read the current CBU".to_string(),
                }
            ]
        );
        assert_eq!(
            envelope.semantic_ir.steps[0].binding_mode,
            BindingMode::SessionReference
        );
        assert_eq!(
            envelope.semantic_ir.steps[0].qualifiers,
            vec![
                (
                    "legacy-summary".to_string(),
                    "Read the current CBU".to_string()
                ),
                (
                    "legacy-notes".to_string(),
                    "Read the current CBU".to_string()
                )
            ]
        );
    }

    #[test]
    fn bridge_preserves_distinct_legacy_notes() {
        let outcome = OutcomeIntent {
            summary: "Submit the current CBU for validation".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Write,
            domain_concept: "cbu".to_string(),
            action: OutcomeAction::Update,
            subject: None,
            steps: vec![OutcomeStep {
                action: OutcomeAction::Update,
                target: "cbu".to_string(),
                params: std::collections::HashMap::new(),
                notes: Some("move lifecycle into validation review".to_string()),
            }],
            confidence: SageConfidence::High,
            pending_clarifications: vec![],
            hints: Default::default(),
            explain: Default::default(),
            coder_handoff: Default::default(),
        };

        let envelope = compiler_input_from_outcome_intent(
            &outcome,
            None,
            Some(Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid uuid")),
            Some("cbu"),
            Some("Current CBU"),
        );

        assert_eq!(
            envelope.structured_intent.steps[0].qualifiers,
            vec![
                IntentQualifier {
                    name: "legacy-summary".to_string(),
                    value: "Submit the current CBU for validation".to_string(),
                },
                IntentQualifier {
                    name: "legacy-notes".to_string(),
                    value: "move lifecycle into validation review".to_string(),
                }
            ]
        );
    }
}

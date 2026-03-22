//! Synthetic utterance generation helpers.

use anyhow::{Context, Result};

use super::types::{CalibrationScenario, GeneratedUtterance};

/// Build the LLM prompt used to generate a synthetic utterance family.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::calibration::{build_generation_prompt, CalibrationScenario};
///
/// # fn demo(scenario: &CalibrationScenario) {
/// let prompt = build_generation_prompt(scenario);
/// assert!(prompt.contains(&scenario.target_verb));
/// # }
/// ```
pub fn build_generation_prompt(scenario: &CalibrationScenario) -> String {
    let neighbour_lines = scenario
        .near_neighbour_verbs
        .iter()
        .map(|neighbour| {
            format!(
                "- {} (risk: {:?}, distance: {:.3}, signals: {})",
                neighbour.verb_id,
                neighbour.confusion_risk,
                neighbour.expected_embedding_distance,
                neighbour.distinguishing_signals.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Generate a JSON array of synthetic calibration utterances.\n\
Entity type: {entity_type}\n\
Entity state: {entity_state}\n\
Operational phase: {phase}\n\
Situation signature: {signature}\n\
Target verb: {target_verb}\n\
Legal verbs: {legal_verbs}\n\
Nearest neighbours:\n{neighbours}\n\
Return 8-10 positive, 3-4 negative type_a, 2-3 negative type_b, and 3-5 boundary utterances.\n\
Each row must contain: text, calibration_mode, negative_type, expected_outcome, generation_rationale.",
        entity_type = scenario.target_entity_type,
        entity_state = scenario.target_entity_state,
        phase = scenario.operational_phase,
        signature = scenario.situation_signature,
        target_verb = scenario.target_verb,
        legal_verbs = scenario.legal_verb_set_snapshot.join(", "),
        neighbours = neighbour_lines,
    )
}

/// Parse a model JSON response into generated utterances.
///
/// # Examples
/// ```rust
/// use ob_poc::calibration::generator::parse_generated_utterances;
///
/// let raw = r#"[{
///   "text":"show me the case",
///   "calibration_mode":"positive",
///   "negative_type":null,
///   "expected_outcome":{"type":"resolves_to","value":"case.read"},
///   "generation_rationale":"basic read"
/// }]"#;
/// let parsed = parse_generated_utterances(raw).unwrap();
/// assert_eq!(parsed.len(), 1);
/// ```
pub fn parse_generated_utterances(raw: &str) -> Result<Vec<GeneratedUtterance>> {
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str(cleaned).context("parse generated utterance JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::{
        CalibrationExecutionShape, CalibrationScenario, ConfusionRisk, GovernanceStatus,
        NearNeighbourVerb,
    };
    use uuid::Uuid;

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
            legal_verb_set_snapshot: vec!["entity.read".into(), "entity.update".into()],
            verb_taxonomy_tag: "read".into(),
            excluded_neighbours: vec![],
            near_neighbour_verbs: vec![NearNeighbourVerb {
                verb_id: "entity.update".into(),
                expected_embedding_distance: 0.12,
                confusion_risk: ConfusionRisk::High,
                distinguishing_signals: vec!["read_only".into()],
            }],
            expected_margin_threshold: 0.08,
            execution_shape: CalibrationExecutionShape::Singleton,
            gold_utterances: vec![],
            admitted_synthetic_set_id: None,
        }
    }

    #[test]
    fn build_generation_prompt_includes_target_and_neighbour() {
        let prompt = build_generation_prompt(&sample_scenario());
        assert!(prompt.contains("entity.read"));
        assert!(prompt.contains("entity.update"));
    }

    #[test]
    fn parse_generated_utterances_supports_fenced_json() {
        let raw = r#"```json
[
  {
    "text": "show me the entity",
    "calibration_mode": "positive",
    "negative_type": null,
    "expected_outcome": { "type": "resolves_to", "value": "entity.read" },
    "generation_rationale": "basic read"
  }
]
```"#;
        let parsed = parse_generated_utterances(raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "show me the entity");
    }
}

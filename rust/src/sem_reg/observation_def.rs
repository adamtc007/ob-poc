//! Observation definition types for the semantic registry.
//!
//! An observation is a structured data point obtained from an external
//! source or automated process. Examples: PEP screening result,
//! sanctions list check, credit score, adverse media hit.

use serde::{Deserialize, Serialize};

/// Body for an observation definition snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationDefBody {
    /// Fully qualified name, e.g. `"obs.pep-screening"`
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Observation type: `screening`, `verification`, `assessment`, `computation`
    pub observation_type: String,
    /// FQN of the verb that produces this observation (if automated)
    #[serde(default)]
    pub source_verb_fqn: Option<String>,
    /// Rules for extracting attributes from observation results
    #[serde(default)]
    pub extraction_rules: Vec<ExtractionRule>,
    /// Whether human review is required before the observation is accepted
    #[serde(default)]
    pub requires_human_review: bool,
}

/// A rule for extracting attribute values from observation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionRule {
    /// Attribute FQN that this rule populates
    pub target_attribute_fqn: String,
    /// JSON path or expression to extract the value from the observation payload
    pub source_path: String,
    /// Data transformation: `none`, `uppercase`, `date_parse`, `boolean_coerce`
    #[serde(default = "default_none")]
    pub transform: String,
    /// Confidence score assigned to extracted values (0.0 to 1.0)
    #[serde(default)]
    pub confidence: Option<f64>,
}

fn default_none() -> String {
    "none".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_def_serde() {
        let body = ObservationDefBody {
            fqn: "obs.pep-screening".into(),
            name: "PEP Screening".into(),
            description: "Check against politically exposed persons lists".into(),
            observation_type: "screening".into(),
            source_verb_fqn: Some("screening.run-pep-check".into()),
            extraction_rules: vec![ExtractionRule {
                target_attribute_fqn: "entity.pep-status".into(),
                source_path: "$.result.pep_match".into(),
                transform: "boolean_coerce".into(),
                confidence: Some(0.95),
            }],
            requires_human_review: true,
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: ObservationDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "obs.pep-screening");
        assert_eq!(round.observation_type, "screening");
        assert!(round.requires_human_review);
        assert_eq!(round.extraction_rules.len(), 1);
    }

    #[test]
    fn test_extraction_rule_defaults() {
        let json = serde_json::json!({
            "target_attribute_fqn": "entity.name",
            "source_path": "$.name"
        });
        let rule: ExtractionRule = serde_json::from_value(json).unwrap();
        assert_eq!(rule.transform, "none");
        assert!(rule.confidence.is_none());
    }
}

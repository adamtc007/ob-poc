//! Evidence requirement body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_one() -> u32 {
    1
}

/// Body of an `evidence_requirement` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRequirementBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub target_entity_type: String,
    #[serde(default)]
    pub trigger_context: Option<String>,
    #[serde(default)]
    pub required_documents: Vec<RequiredDocument>,
    #[serde(default)]
    pub required_observations: Vec<RequiredObservation>,
    #[serde(default = "default_true")]
    pub all_required: bool,
}

/// A document type required by an evidence requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredDocument {
    pub document_type_fqn: String,
    #[serde(default = "default_one")]
    pub min_count: u32,
    #[serde(default)]
    pub max_age_days: Option<u32>,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default = "default_true")]
    pub mandatory: bool,
}

/// An observation type required by an evidence requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredObservation {
    pub observation_def_fqn: String,
    #[serde(default)]
    pub min_confidence: Option<f64>,
    #[serde(default)]
    pub max_age_days: Option<u32>,
    #[serde(default = "default_true")]
    pub mandatory: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let val = EvidenceRequirementBody {
            fqn: "kyc.identity_evidence".into(),
            name: "Identity Evidence".into(),
            description: "Required identity docs".into(),
            target_entity_type: "natural_person".into(),
            trigger_context: Some("onboarding".into()),
            required_documents: vec![RequiredDocument {
                document_type_fqn: "doc.passport".into(),
                min_count: 1,
                max_age_days: Some(365),
                alternatives: vec!["doc.national_id".into()],
                mandatory: true,
            }],
            required_observations: vec![RequiredObservation {
                observation_def_fqn: "obs.identity_check".into(),
                min_confidence: Some(0.85),
                max_age_days: Some(90),
                mandatory: false,
            }],
            all_required: false,
        };
        let json = serde_json::to_value(&val).unwrap();
        let back: EvidenceRequirementBody = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        assert_eq!(json, json2);
    }

    #[test]
    fn defaults_all_required_and_mandatory() {
        let doc: RequiredDocument = serde_json::from_value(serde_json::json!({
            "document_type_fqn": "doc.x"
        }))
        .unwrap();
        assert!(doc.mandatory);
        assert_eq!(doc.min_count, 1);

        let body: EvidenceRequirementBody = serde_json::from_value(serde_json::json!({
            "fqn": "f", "name": "n", "description": "d", "target_entity_type": "t"
        }))
        .unwrap();
        assert!(body.all_required);
    }
}

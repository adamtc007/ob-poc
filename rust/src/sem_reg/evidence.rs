//! Evidence requirement types for the semantic registry.
//!
//! An evidence requirement specifies what documents and observations
//! must be collected to substantiate a registry claim. This is the
//! bridge between the Proof Rule (governance) and the document/observation
//! collection pipeline.
//!
//! Key invariant: if an evidence requirement references a `TrustClass::Proof`
//! attribute, the evidence requirement itself must be governed-tier.

use serde::{Deserialize, Serialize};

/// Body for an evidence requirement snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRequirementBody {
    /// Fully qualified name, e.g. `"kyc.identity-evidence"`
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Entity type this requirement applies to
    pub target_entity_type: String,
    /// Context that triggers this requirement (e.g., `onboarding`, `periodic_review`)
    #[serde(default)]
    pub trigger_context: Option<String>,
    /// Required documents
    #[serde(default)]
    pub required_documents: Vec<RequiredDocument>,
    /// Required observations (data points from external sources)
    #[serde(default)]
    pub required_observations: Vec<RequiredObservation>,
    /// Whether ALL items are required (true) or ANY (false)
    #[serde(default = "default_true")]
    pub all_required: bool,
}

fn default_true() -> bool {
    true
}

/// A document required as evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredDocument {
    /// FQN of the document type definition
    pub document_type_fqn: String,
    /// How many copies/instances are needed
    #[serde(default = "default_one")]
    pub min_count: u32,
    /// Maximum age in days (None = no expiry check)
    #[serde(default)]
    pub max_age_days: Option<u32>,
    /// Acceptable alternatives (any one suffices)
    #[serde(default)]
    pub alternatives: Vec<String>,
    /// Whether this specific document is mandatory
    #[serde(default = "default_true_doc")]
    pub mandatory: bool,
}

fn default_one() -> u32 {
    1
}

fn default_true_doc() -> bool {
    true
}

/// An observation (data point) required as evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredObservation {
    /// FQN of the observation definition
    pub observation_def_fqn: String,
    /// Minimum confidence score required (0.0 to 1.0)
    #[serde(default)]
    pub min_confidence: Option<f64>,
    /// Maximum age in days
    #[serde(default)]
    pub max_age_days: Option<u32>,
    /// Whether this observation is mandatory
    #[serde(default = "default_true_obs")]
    pub mandatory: bool,
}

fn default_true_obs() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_requirement_serde() {
        let body = EvidenceRequirementBody {
            fqn: "kyc.identity-evidence".into(),
            name: "Identity Evidence Pack".into(),
            description: "Documents and observations required for identity verification".into(),
            target_entity_type: "entity.proper-person".into(),
            trigger_context: Some("onboarding".into()),
            required_documents: vec![RequiredDocument {
                document_type_fqn: "doc.passport".into(),
                min_count: 1,
                max_age_days: Some(3650), // 10 years
                alternatives: vec!["doc.national-id".into(), "doc.driving-licence".into()],
                mandatory: true,
            }],
            required_observations: vec![RequiredObservation {
                observation_def_fqn: "obs.pep-screening".into(),
                min_confidence: Some(0.95),
                max_age_days: Some(90),
                mandatory: true,
            }],
            all_required: true,
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: EvidenceRequirementBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "kyc.identity-evidence");
        assert_eq!(round.required_documents.len(), 1);
        assert_eq!(round.required_documents[0].alternatives.len(), 2);
        assert_eq!(round.required_observations.len(), 1);
    }

    #[test]
    fn test_evidence_defaults() {
        let json = serde_json::json!({
            "fqn": "test.evidence",
            "name": "Test",
            "description": "Test evidence",
            "target_entity_type": "entity.test"
        });
        let body: EvidenceRequirementBody = serde_json::from_value(json).unwrap();
        assert!(body.all_required); // default true
        assert!(body.required_documents.is_empty());
        assert!(body.required_observations.is_empty());
        assert!(body.trigger_context.is_none());
    }
}

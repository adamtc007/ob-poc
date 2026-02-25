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

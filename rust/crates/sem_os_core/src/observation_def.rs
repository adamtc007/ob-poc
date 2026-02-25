//! Observation definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

fn default_none() -> String {
    "none".into()
}

/// Body of an `observation_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationDefBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub observation_type: String,
    #[serde(default)]
    pub source_verb_fqn: Option<String>,
    #[serde(default)]
    pub extraction_rules: Vec<ExtractionRule>,
    #[serde(default)]
    pub requires_human_review: bool,
}

/// A rule for extracting observation data into an attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionRule {
    pub target_attribute_fqn: String,
    pub source_path: String,
    #[serde(default = "default_none")]
    pub transform: String,
    #[serde(default)]
    pub confidence: Option<f64>,
}

//! Document type definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

/// Body of a `document_type_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTypeDefBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub max_age_days: Option<u32>,
    #[serde(default)]
    pub accepted_formats: Vec<String>,
    #[serde(default)]
    pub extraction_rules: Vec<DocumentExtractionRule>,
}

/// A rule for extracting data from a document into an attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentExtractionRule {
    pub target_attribute_fqn: String,
    pub method: String,
    #[serde(default)]
    pub min_confidence: Option<f64>,
    #[serde(default)]
    pub requires_review: bool,
}

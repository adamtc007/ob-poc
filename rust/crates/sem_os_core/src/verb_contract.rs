//! Verb contract body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Body of a `verb_contract` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContractBody {
    pub fqn: String,
    pub domain: String,
    pub action: String,
    pub description: String,
    pub behavior: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<VerbArgDef>,
    #[serde(default)]
    pub returns: Option<VerbReturnSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<VerbPrecondition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub postconditions: Vec<String>,
    #[serde(default)]
    pub produces: Option<VerbProducesSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invocation_phrases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subject_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phase_tags: Vec<String>,
    #[serde(default = "default_true")]
    pub requires_subject: bool,
    #[serde(default)]
    pub produces_focus: bool,
    #[serde(default)]
    pub metadata: Option<VerbContractMetadata>,
    /// CRUD table/schema/operation mapping (when behavior = "crud").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crud_mapping: Option<VerbCrudMapping>,
}

/// CRUD table/operation mapping captured from verb YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCrudMapping {
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_column: Option<String>,
}

/// Definition of a verb argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbArgDef {
    pub name: String,
    pub arg_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lookup: Option<VerbArgLookup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_values: Option<Vec<String>>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

/// Entity lookup configuration for a verb argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbArgLookup {
    pub table: String,
    pub entity_type: String,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub search_key: Option<String>,
    #[serde(default)]
    pub primary_key: Option<String>,
}

/// Return type specification for a verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbReturnSpec {
    #[serde(rename = "type")]
    pub return_type: String,
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
}

/// A precondition that must be met before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbPrecondition {
    pub kind: String,
    pub value: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// What a verb produces on success.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbProducesSpec {
    #[serde(rename = "type")]
    pub entity_type: String,
    #[serde(default)]
    pub resolved: bool,
}

/// Optional metadata attached to a verb contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContractMetadata {
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub source_of_truth: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub noun: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subject_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phase_tags: Vec<String>,
}

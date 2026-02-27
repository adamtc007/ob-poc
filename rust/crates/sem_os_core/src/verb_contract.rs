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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let val = VerbContractBody {
            fqn: "cbu.create".into(),
            domain: "cbu".into(),
            action: "create".into(),
            description: "Create a CBU".into(),
            behavior: "plugin".into(),
            args: vec![VerbArgDef {
                name: "name".into(),
                arg_type: "string".into(),
                required: true,
                description: Some("CBU name".into()),
                lookup: None,
                valid_values: None,
                default: None,
            }],
            returns: Some(VerbReturnSpec {
                return_type: "uuid".into(),
                schema: None,
            }),
            preconditions: vec![VerbPrecondition {
                kind: "requires_scope".into(),
                value: "cbu".into(),
                description: None,
            }],
            postconditions: vec![],
            produces: Some(VerbProducesSpec {
                entity_type: "cbu".into(),
                resolved: true,
            }),
            consumes: vec![],
            invocation_phrases: vec!["create cbu".into()],
            subject_kinds: vec!["cbu".into()],
            phase_tags: vec![],
            requires_subject: true,
            produces_focus: false,
            metadata: Some(VerbContractMetadata {
                tier: Some("intent".into()),
                source_of_truth: None,
                scope: None,
                noun: None,
                tags: vec![],
                subject_kinds: vec![],
                phase_tags: vec![],
            }),
            crud_mapping: Some(VerbCrudMapping {
                operation: "insert".into(),
                table: Some("cbus".into()),
                schema: Some("ob-poc".into()),
                key_column: None,
            }),
        };
        let json = serde_json::to_value(&val).unwrap();
        // Check #[serde(rename = "type")] on returns and produces
        assert_eq!(json["returns"]["type"], "uuid");
        assert_eq!(json["produces"]["type"], "cbu");
        // Check default_true(): requires_subject defaults to true
        let minimal: VerbContractBody =
            serde_json::from_str(r#"{"fqn":"x","domain":"x","action":"x","description":"x","behavior":"x"}"#).unwrap();
        assert!(minimal.requires_subject);
        // Round-trip
        let back: VerbContractBody = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        assert_eq!(json, json2);
    }
}

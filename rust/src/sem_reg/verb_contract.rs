//! Verb contract body — the typed JSONB content for `ObjectType::VerbContract`.
//!
//! A verb contract is the semantic registry's representation of a DSL verb.
//! It captures the verb's I/O surface, preconditions, and behavior metadata.

use serde::{Deserialize, Serialize};

/// The JSONB body stored in `definition` for verb contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContractBody {
    /// Fully qualified name, e.g. "cbu.create", "session.load-galaxy"
    pub fqn: String,
    /// Domain (e.g. "cbu", "session", "kyc")
    pub domain: String,
    /// Action (e.g. "create", "load-galaxy", "update-status")
    pub action: String,
    /// Human-readable description
    pub description: String,
    /// Behavior type
    pub behavior: String,
    /// Argument definitions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<VerbArgDef>,
    /// Return specification
    #[serde(default)]
    pub returns: Option<VerbReturnSpec>,
    /// Preconditions that must hold before execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<VerbPrecondition>,
    /// Postconditions guaranteed after successful execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub postconditions: Vec<String>,
    /// What this verb produces (binding type)
    #[serde(default)]
    pub produces: Option<VerbProducesSpec>,
    /// What this verb consumes (required bindings)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumes: Vec<String>,
    /// Natural language invocation phrases
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invocation_phrases: Vec<String>,
    /// Verb metadata
    #[serde(default)]
    pub metadata: Option<VerbContractMetadata>,
}

/// A single argument in a verb contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbArgDef {
    /// Argument name (kebab-case in DSL, e.g. "fund-entity-id")
    pub name: String,
    /// Argument type (string, uuid, integer, etc.)
    pub arg_type: String,
    /// Whether this argument is required
    #[serde(default)]
    pub required: bool,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// Lookup configuration for entity resolution
    #[serde(default)]
    pub lookup: Option<VerbArgLookup>,
    /// Valid values (for enum-like args)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_values: Option<Vec<String>>,
    /// Default value
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

/// Lookup configuration for resolving entity references in verb arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbArgLookup {
    /// Table to search
    pub table: String,
    /// Entity type
    pub entity_type: String,
    /// Schema
    #[serde(default)]
    pub schema: Option<String>,
    /// Column to search by
    #[serde(default)]
    pub search_key: Option<String>,
    /// Primary key column
    #[serde(default)]
    pub primary_key: Option<String>,
}

/// Return type specification for a verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbReturnSpec {
    /// Return type (uuid, record, record_set, affected, void)
    #[serde(rename = "type")]
    pub return_type: String,
    /// Schema of the returned value (for record/record_set)
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
}

/// Precondition for a verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbPrecondition {
    /// Precondition type (e.g. "requires_scope", "requires_prior", "forbids_prior")
    pub kind: String,
    /// Value (e.g. "cbu" for requires_scope, "cbu.create" for requires_prior)
    pub value: String,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
}

/// What a verb produces (binding/output type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbProducesSpec {
    /// Entity type produced
    #[serde(rename = "type")]
    pub entity_type: String,
    /// Whether the produced reference is resolved
    #[serde(default)]
    pub resolved: bool,
}

/// Metadata for a verb contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContractMetadata {
    /// Tier (intent, composite, template)
    #[serde(default)]
    pub tier: Option<String>,
    /// Source of truth (operational, external, derived)
    #[serde(default)]
    pub source_of_truth: Option<String>,
    /// Scope (global, session, cbu)
    #[serde(default)]
    pub scope: Option<String>,
    /// Display noun for UI
    #[serde(default)]
    pub noun: Option<String>,
    /// Tags for categorisation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_contract_serde() {
        let body = VerbContractBody {
            fqn: "cbu.create".into(),
            domain: "cbu".into(),
            action: "create".into(),
            description: "Create a new Client Business Unit".into(),
            behavior: "plugin".into(),
            args: vec![
                VerbArgDef {
                    name: "name".into(),
                    arg_type: "string".into(),
                    required: true,
                    description: Some("CBU name".into()),
                    lookup: None,
                    valid_values: None,
                    default: None,
                },
                VerbArgDef {
                    name: "jurisdiction".into(),
                    arg_type: "string".into(),
                    required: true,
                    description: Some("Jurisdiction code".into()),
                    lookup: Some(VerbArgLookup {
                        table: "master_jurisdictions".into(),
                        entity_type: "jurisdiction".into(),
                        schema: Some("ob-poc".into()),
                        search_key: Some("jurisdiction_code".into()),
                        primary_key: Some("jurisdiction_code".into()),
                    }),
                    valid_values: None,
                    default: None,
                },
            ],
            returns: Some(VerbReturnSpec {
                return_type: "uuid".into(),
                schema: None,
            }),
            preconditions: vec![],
            postconditions: vec!["CBU exists with given name and jurisdiction".into()],
            produces: Some(VerbProducesSpec {
                entity_type: "cbu".into(),
                resolved: false,
            }),
            consumes: vec![],
            invocation_phrases: vec!["create CBU".into(), "new CBU".into()],
            metadata: Some(VerbContractMetadata {
                tier: Some("intent".into()),
                source_of_truth: Some("operational".into()),
                scope: Some("global".into()),
                noun: Some("cbu".into()),
                tags: vec!["lifecycle".into(), "write".into()],
            }),
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: VerbContractBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "cbu.create");
        assert_eq!(back.args.len(), 2);
        assert!(back.args[1].lookup.is_some());
        assert_eq!(back.invocation_phrases.len(), 2);
    }

    #[test]
    fn test_minimal_verb_contract() {
        let body = VerbContractBody {
            fqn: "session.info".into(),
            domain: "session".into(),
            action: "info".into(),
            description: "Show session information".into(),
            behavior: "plugin".into(),
            args: vec![],
            returns: None,
            preconditions: vec![],
            postconditions: vec![],
            produces: None,
            consumes: vec![],
            invocation_phrases: vec![],
            metadata: None,
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: VerbContractBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "session.info");
        assert!(back.args.is_empty());
    }
}

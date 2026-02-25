//! Derivation spec body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Body of a `derivation_spec` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationSpecBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub output_attribute_fqn: String,
    pub inputs: Vec<DerivationInput>,
    pub expression: DerivationExpression,
    #[serde(default)]
    pub null_semantics: NullSemantics,
    #[serde(default)]
    pub freshness_rule: Option<FreshnessRule>,
    #[serde(default)]
    pub security_inheritance: SecurityInheritanceMode,
    #[serde(default)]
    pub evidence_grade: EvidenceGrade,
    #[serde(default)]
    pub tests: Vec<DerivationTestCase>,
}

/// An input attribute for a derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationInput {
    pub attribute_fqn: String,
    pub role: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

/// The expression used to compute the derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DerivationExpression {
    FunctionRef { ref_name: String },
}

/// How null inputs are handled.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullSemantics {
    #[default]
    Propagate,
    Default(serde_json::Value),
    Skip,
    Error,
}

/// How security labels are inherited from inputs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityInheritanceMode {
    #[default]
    Strict,
    DeclaredOverride,
}

/// Whether derived attributes can be used as evidence.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGrade {
    #[default]
    Prohibited,
    AllowedWithConstraints,
}

/// Freshness constraint on input data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessRule {
    pub max_age_seconds: u64,
}

/// A test case for verifying derivation logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationTestCase {
    pub inputs: serde_json::Value,
    pub expected: serde_json::Value,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let val = DerivationSpecBody {
            fqn: "derived.total_aum".into(),
            name: "Total AUM".into(),
            description: "Sum of holdings".into(),
            output_attribute_fqn: "cbu.total_aum".into(),
            inputs: vec![DerivationInput {
                attribute_fqn: "holding.amount".into(),
                role: "addend".into(),
                required: true,
            }],
            expression: DerivationExpression::FunctionRef {
                ref_name: "sum_holdings".into(),
            },
            null_semantics: NullSemantics::Skip,
            freshness_rule: Some(FreshnessRule {
                max_age_seconds: 3600,
            }),
            security_inheritance: SecurityInheritanceMode::Strict,
            evidence_grade: EvidenceGrade::AllowedWithConstraints,
            tests: vec![DerivationTestCase {
                inputs: serde_json::json!({"holding.amount": [100, 200]}),
                expected: serde_json::json!(300),
            }],
        };
        let json = serde_json::to_value(&val).unwrap();
        // Check tagged enum FunctionRef serialization
        assert_eq!(json["expression"]["type"], "function_ref");
        // Check NullSemantics::Skip
        assert_eq!(json["null_semantics"], "skip");
        // Check defaults: default_true on DerivationInput.required
        let input: DerivationInput =
            serde_json::from_str(r#"{"attribute_fqn":"x","role":"y"}"#).unwrap();
        assert!(input.required);
        // Round-trip
        let back: DerivationSpecBody = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        assert_eq!(json, json2);
    }
}

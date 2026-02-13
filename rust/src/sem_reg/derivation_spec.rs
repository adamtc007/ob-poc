//! Derivation specification body type.
//!
//! A `DerivationSpecBody` describes a recipe for computing a derived attribute
//! from one or more input attributes. Stored as JSONB in `sem_reg.snapshots`
//! with `object_type = 'derivation_spec'`.
//!
//! MVP: Only `FunctionRef` expressions — Rust function dispatch via the
//! `DerivationFunctionRegistry`. AST-based expressions can be added later.

use serde::{Deserialize, Serialize};

/// Body type for `ObjectType::DerivationSpec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationSpecBody {
    /// Fully qualified name, e.g. `"risk.composite_score"`.
    pub fqn: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the derivation.
    pub description: String,
    /// FQN of the output attribute produced by this derivation.
    pub output_attribute_fqn: String,
    /// Input attributes consumed by this derivation.
    pub inputs: Vec<DerivationInput>,
    /// The expression that computes the output from inputs.
    pub expression: DerivationExpression,
    /// How null inputs are handled.
    #[serde(default)]
    pub null_semantics: NullSemantics,
    /// Optional freshness constraint on input data.
    #[serde(default)]
    pub freshness_rule: Option<FreshnessRule>,
    /// How security labels are inherited from inputs.
    #[serde(default)]
    pub security_inheritance: SecurityInheritanceMode,
    /// Whether this derivation may be used as regulatory evidence.
    #[serde(default)]
    pub evidence_grade: EvidenceGrade,
    /// Inline test cases for validation.
    #[serde(default)]
    pub tests: Vec<DerivationTestCase>,
}

/// A single input to a derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationInput {
    /// FQN of the input attribute.
    pub attribute_fqn: String,
    /// Role of this input (e.g., `"primary"`, `"secondary"`, `"weight"`).
    pub role: String,
    /// Whether this input is required (affects null semantics).
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

/// The computation expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DerivationExpression {
    /// MVP: dispatch to a named Rust function via `DerivationFunctionRegistry`.
    FunctionRef {
        /// Name of the registered function, e.g. `"weighted_average"`.
        ref_name: String,
    },
    // Future variants:
    // ExpressionAst { ast: serde_json::Value },
    // QueryPlan { sql: String },
}

/// How null inputs are handled.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullSemantics {
    /// Null propagates: if any required input is null, output is null.
    #[default]
    Propagate,
    /// Use a default value when inputs are null.
    Default(serde_json::Value),
    /// Skip derivation entirely when inputs are null.
    Skip,
    /// Error when required inputs are null.
    Error,
}

/// How security labels are inherited from inputs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityInheritanceMode {
    /// Default: compute inherited label using `compute_inherited_label()`.
    #[default]
    Strict,
    /// Allow declared override with steward approval.
    DeclaredOverride,
}

/// Whether a derivation may be used as regulatory evidence.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGrade {
    /// This derivation MUST NOT be used as evidence (default for operational).
    #[default]
    Prohibited,
    /// May be used as evidence with constraints (governed tier only).
    AllowedWithConstraints,
}

/// Freshness constraint on input data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessRule {
    /// Maximum age of input data in seconds.
    pub max_age_seconds: u64,
}

/// Inline test case for derivation validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationTestCase {
    /// Input values keyed by attribute FQN or role.
    pub inputs: serde_json::Value,
    /// Expected output value.
    pub expected: serde_json::Value,
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> DerivationSpecBody {
        DerivationSpecBody {
            fqn: "risk.composite_score".into(),
            name: "Composite Risk Score".into(),
            description: "Weighted average of risk factors".into(),
            output_attribute_fqn: "risk.composite_score_value".into(),
            inputs: vec![
                DerivationInput {
                    attribute_fqn: "risk.credit_score".into(),
                    role: "primary".into(),
                    required: true,
                },
                DerivationInput {
                    attribute_fqn: "risk.market_volatility".into(),
                    role: "secondary".into(),
                    required: false,
                },
            ],
            expression: DerivationExpression::FunctionRef {
                ref_name: "weighted_average".into(),
            },
            null_semantics: NullSemantics::Propagate,
            freshness_rule: Some(FreshnessRule {
                max_age_seconds: 3600,
            }),
            security_inheritance: SecurityInheritanceMode::Strict,
            evidence_grade: EvidenceGrade::Prohibited,
            tests: vec![DerivationTestCase {
                inputs: serde_json::json!({"credit_score": 750, "market_volatility": 0.15}),
                expected: serde_json::json!(0.65),
            }],
        }
    }

    #[test]
    fn test_serde_round_trip() {
        let spec = sample_spec();
        let json = serde_json::to_value(&spec).unwrap();
        let back: DerivationSpecBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "risk.composite_score");
        assert_eq!(back.inputs.len(), 2);
        assert_eq!(back.tests.len(), 1);
    }

    #[test]
    fn test_null_semantics_variants() {
        // Default
        let json = serde_json::json!("propagate");
        let ns: NullSemantics = serde_json::from_value(json).unwrap();
        assert!(matches!(ns, NullSemantics::Propagate));

        // Skip
        let json = serde_json::json!("skip");
        let ns: NullSemantics = serde_json::from_value(json).unwrap();
        assert!(matches!(ns, NullSemantics::Skip));

        // Error
        let json = serde_json::json!("error");
        let ns: NullSemantics = serde_json::from_value(json).unwrap();
        assert!(matches!(ns, NullSemantics::Error));

        // Default with value
        let json = serde_json::json!({"default": 0});
        let ns: NullSemantics = serde_json::from_value(json).unwrap();
        assert!(matches!(ns, NullSemantics::Default(_)));
    }

    #[test]
    fn test_expression_function_ref() {
        let expr = DerivationExpression::FunctionRef {
            ref_name: "weighted_average".into(),
        };
        let json = serde_json::to_value(&expr).unwrap();
        assert_eq!(json["type"], "function_ref");
        assert_eq!(json["ref_name"], "weighted_average");

        let back: DerivationExpression = serde_json::from_value(json).unwrap();
        match back {
            DerivationExpression::FunctionRef { ref_name } => {
                assert_eq!(ref_name, "weighted_average");
            }
        }
    }

    #[test]
    fn test_evidence_grade_default() {
        let grade: EvidenceGrade = Default::default();
        assert!(matches!(grade, EvidenceGrade::Prohibited));
    }

    #[test]
    fn test_input_required_default() {
        let json = serde_json::json!({
            "attribute_fqn": "test.attr",
            "role": "primary"
        });
        let input: DerivationInput = serde_json::from_value(json).unwrap();
        assert!(input.required); // default_true
    }
}

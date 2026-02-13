//! Derivation evaluation engine.
//!
//! Evaluates `DerivationSpecBody` recipes by dispatching to registered Rust
//! functions. Computes inherited security labels and produces snapshot-pinnable
//! results.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::derivation_spec::{DerivationExpression, DerivationSpecBody, NullSemantics};
use super::security::compute_inherited_label;
use super::types::SecurityLabel;

// ── Derivation function trait ─────────────────────────────────

/// Trait for derivation functions that can be registered and dispatched.
pub trait DerivationFn: Send + Sync {
    /// Evaluate the function with the given inputs.
    fn evaluate(&self, inputs: &serde_json::Value) -> Result<serde_json::Value, DerivationError>;
}

/// Implement `DerivationFn` for closures.
impl<F> DerivationFn for F
where
    F: Fn(&serde_json::Value) -> Result<serde_json::Value, DerivationError> + Send + Sync,
{
    fn evaluate(&self, inputs: &serde_json::Value) -> Result<serde_json::Value, DerivationError> {
        (self)(inputs)
    }
}

// ── Errors ────────────────────────────────────────────────────

/// Errors from derivation evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivationError {
    /// Referenced function not found in registry.
    FunctionNotFound { ref_name: String },
    /// Required input is null.
    NullRequiredInput { attribute_fqn: String },
    /// Function execution failed.
    ExecutionFailed { message: String },
    /// Input data is stale (exceeds freshness rule).
    StalenessViolation { max_age_seconds: u64 },
}

impl std::fmt::Display for DerivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FunctionNotFound { ref_name } => {
                write!(
                    f,
                    "Derivation function '{}' not found in registry",
                    ref_name
                )
            }
            Self::NullRequiredInput { attribute_fqn } => {
                write!(f, "Required input '{}' is null", attribute_fqn)
            }
            Self::ExecutionFailed { message } => {
                write!(f, "Derivation execution failed: {}", message)
            }
            Self::StalenessViolation { max_age_seconds } => {
                write!(
                    f,
                    "Input data exceeds freshness rule ({} seconds)",
                    max_age_seconds
                )
            }
        }
    }
}

impl std::error::Error for DerivationError {}

// ── Derivation result ─────────────────────────────────────────

/// Result of evaluating a derivation spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct DerivationResult {
    /// The computed output value.
    pub value: serde_json::Value,
    /// Snapshot ID of the DerivationSpec used.
    pub spec_snapshot_id: Uuid,
    /// Snapshot IDs of the input data used.
    pub input_snapshot_ids: Vec<Uuid>,
    /// Inherited security label computed from input labels.
    pub inherited_label: SecurityLabel,
    /// When the derivation was evaluated.
    pub evaluated_at: DateTime<Utc>,
}

use serde::{Deserialize, Serialize};

// ── Function registry ─────────────────────────────────────────

/// Registry of named derivation functions.
pub struct DerivationFunctionRegistry {
    functions: HashMap<String, Arc<dyn DerivationFn>>,
}

impl DerivationFunctionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Register a named function.
    pub fn register(&mut self, name: &str, func: Arc<dyn DerivationFn>) {
        self.functions.insert(name.to_string(), func);
    }

    /// Look up a function by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn DerivationFn>> {
        self.functions.get(name)
    }

    /// Evaluate a derivation spec against inputs.
    ///
    /// Steps:
    /// 1. Resolve function by `ref_name`
    /// 2. Check null inputs against `null_semantics`
    /// 3. Evaluate function
    /// 4. Compute inherited security label from input labels
    pub fn evaluate(
        &self,
        spec: &DerivationSpecBody,
        inputs: &serde_json::Value,
        input_labels: &[SecurityLabel],
        spec_snapshot_id: Uuid,
        input_snapshot_ids: Vec<Uuid>,
    ) -> Result<DerivationResult, DerivationError> {
        // 1. Look up function
        let ref_name = match &spec.expression {
            DerivationExpression::FunctionRef { ref_name } => ref_name,
        };

        let func =
            self.functions
                .get(ref_name)
                .ok_or_else(|| DerivationError::FunctionNotFound {
                    ref_name: ref_name.clone(),
                })?;

        // 2. Check null inputs for required inputs
        for input_def in &spec.inputs {
            if input_def.required {
                let value = inputs
                    .get(&input_def.attribute_fqn)
                    .or_else(|| inputs.get(&input_def.role));
                match value {
                    None | Some(serde_json::Value::Null) => match &spec.null_semantics {
                        NullSemantics::Error => {
                            return Err(DerivationError::NullRequiredInput {
                                attribute_fqn: input_def.attribute_fqn.clone(),
                            });
                        }
                        NullSemantics::Propagate => {
                            return Ok(DerivationResult {
                                value: serde_json::Value::Null,
                                spec_snapshot_id,
                                input_snapshot_ids,
                                inherited_label: compute_inherited_label(input_labels),
                                evaluated_at: Utc::now(),
                            });
                        }
                        NullSemantics::Skip => {
                            return Ok(DerivationResult {
                                value: serde_json::Value::Null,
                                spec_snapshot_id,
                                input_snapshot_ids,
                                inherited_label: compute_inherited_label(input_labels),
                                evaluated_at: Utc::now(),
                            });
                        }
                        NullSemantics::Default(default_val) => {
                            // Will use default — continue to evaluation
                            // (the function should handle defaults itself)
                            let _ = default_val;
                        }
                    },
                    _ => {} // Has value — ok
                }
            }
        }

        // 3. Evaluate function
        let value = func
            .evaluate(inputs)
            .map_err(|e| DerivationError::ExecutionFailed {
                message: e.to_string(),
            })?;

        // 4. Compute inherited security label
        let inherited_label = compute_inherited_label(input_labels);

        Ok(DerivationResult {
            value,
            spec_snapshot_id,
            input_snapshot_ids,
            inherited_label,
            evaluated_at: Utc::now(),
        })
    }
}

impl Default for DerivationFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::derivation_spec::*;
    use crate::sem_reg::types::{Classification, HandlingControl};

    fn sample_spec() -> DerivationSpecBody {
        DerivationSpecBody {
            fqn: "test.sum".into(),
            name: "Sum".into(),
            description: "Sum two values".into(),
            output_attribute_fqn: "test.result".into(),
            inputs: vec![
                DerivationInput {
                    attribute_fqn: "test.a".into(),
                    role: "primary".into(),
                    required: true,
                },
                DerivationInput {
                    attribute_fqn: "test.b".into(),
                    role: "secondary".into(),
                    required: true,
                },
            ],
            expression: DerivationExpression::FunctionRef {
                ref_name: "sum".into(),
            },
            null_semantics: NullSemantics::Error,
            freshness_rule: None,
            security_inheritance: SecurityInheritanceMode::Strict,
            evidence_grade: EvidenceGrade::Prohibited,
            tests: vec![],
        }
    }

    fn sum_function(inputs: &serde_json::Value) -> Result<serde_json::Value, DerivationError> {
        let a = inputs.get("test.a").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let b = inputs.get("test.b").and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(serde_json::json!(a + b))
    }

    #[test]
    fn test_function_not_found() {
        let registry = DerivationFunctionRegistry::new();
        let spec = sample_spec();
        let inputs = serde_json::json!({"test.a": 1, "test.b": 2});
        let result = registry.evaluate(&spec, &inputs, &[], Uuid::new_v4(), vec![]);
        assert!(matches!(
            result,
            Err(DerivationError::FunctionNotFound { .. })
        ));
    }

    #[test]
    fn test_successful_evaluation() {
        let mut registry = DerivationFunctionRegistry::new();
        registry.register("sum", Arc::new(sum_function as fn(&serde_json::Value) -> _));

        let spec = sample_spec();
        let inputs = serde_json::json!({"test.a": 10, "test.b": 20});
        let result = registry
            .evaluate(&spec, &inputs, &[], Uuid::new_v4(), vec![])
            .unwrap();

        assert_eq!(result.value, serde_json::json!(30.0));
    }

    #[test]
    fn test_null_required_input_error_semantics() {
        let mut registry = DerivationFunctionRegistry::new();
        registry.register("sum", Arc::new(sum_function as fn(&serde_json::Value) -> _));

        let spec = sample_spec(); // null_semantics = Error
        let inputs = serde_json::json!({"test.a": 10}); // missing test.b
        let result = registry.evaluate(&spec, &inputs, &[], Uuid::new_v4(), vec![]);
        assert!(matches!(
            result,
            Err(DerivationError::NullRequiredInput { .. })
        ));
    }

    #[test]
    fn test_null_required_input_propagate_semantics() {
        let mut registry = DerivationFunctionRegistry::new();
        registry.register("sum", Arc::new(sum_function as fn(&serde_json::Value) -> _));

        let mut spec = sample_spec();
        spec.null_semantics = NullSemantics::Propagate;
        let inputs = serde_json::json!({"test.a": 10}); // missing test.b
        let result = registry
            .evaluate(&spec, &inputs, &[], Uuid::new_v4(), vec![])
            .unwrap();
        assert_eq!(result.value, serde_json::Value::Null);
    }

    #[test]
    fn test_security_label_inherited() {
        let mut registry = DerivationFunctionRegistry::new();
        registry.register("sum", Arc::new(sum_function as fn(&serde_json::Value) -> _));

        let label_a = SecurityLabel {
            classification: Classification::Internal,
            pii: false,
            jurisdictions: vec!["LU".into()],
            ..SecurityLabel::default()
        };
        let label_b = SecurityLabel {
            classification: Classification::Confidential,
            pii: true,
            jurisdictions: vec!["DE".into()],
            handling_controls: vec![HandlingControl::MaskByDefault],
            ..SecurityLabel::default()
        };

        let spec = sample_spec();
        let inputs = serde_json::json!({"test.a": 10, "test.b": 20});
        let result = registry
            .evaluate(&spec, &inputs, &[label_a, label_b], Uuid::new_v4(), vec![])
            .unwrap();

        // Inherited: highest classification, PII true, union jurisdictions
        assert_eq!(
            result.inherited_label.classification,
            Classification::Confidential
        );
        assert!(result.inherited_label.pii);
        assert!(result
            .inherited_label
            .jurisdictions
            .contains(&"LU".to_string()));
        assert!(result
            .inherited_label
            .jurisdictions
            .contains(&"DE".to_string()));
        assert!(result
            .inherited_label
            .handling_controls
            .contains(&HandlingControl::MaskByDefault));
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = DerivationFunctionRegistry::default();
        assert!(registry.get("anything").is_none());
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = DerivationFunctionRegistry::new();
        registry.register("sum", Arc::new(sum_function as fn(&serde_json::Value) -> _));
        assert!(registry.get("sum").is_some());
        assert!(registry.get("multiply").is_none());
    }

    #[test]
    fn test_derivation_result_has_timestamp() {
        let mut registry = DerivationFunctionRegistry::new();
        registry.register("sum", Arc::new(sum_function as fn(&serde_json::Value) -> _));

        let spec = sample_spec();
        let inputs = serde_json::json!({"test.a": 1, "test.b": 2});
        let before = Utc::now();
        let result = registry
            .evaluate(&spec, &inputs, &[], Uuid::new_v4(), vec![])
            .unwrap();
        let after = Utc::now();

        assert!(result.evaluated_at >= before);
        assert!(result.evaluated_at <= after);
    }
}

//! Derivation function registry and evaluation — pure logic, no DB dependency.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::derivation_spec::{DerivationExpression, DerivationSpecBody, NullSemantics};
use crate::security::compute_inherited_label;
use crate::types::SecurityLabel;

// ── Derivation function trait ─────────────────────────────────

/// A pure function that computes a derived attribute value from inputs.
pub trait DerivationFn: Send + Sync {
    fn evaluate(&self, inputs: &serde_json::Value) -> Result<serde_json::Value, DerivationError>;
}

impl<F> DerivationFn for F
where
    F: Fn(&serde_json::Value) -> Result<serde_json::Value, DerivationError> + Send + Sync,
{
    fn evaluate(&self, inputs: &serde_json::Value) -> Result<serde_json::Value, DerivationError> {
        (self)(inputs)
    }
}

// ── Errors ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivationError {
    FunctionNotFound { ref_name: String },
    NullRequiredInput { attribute_fqn: String },
    ExecutionFailed { message: String },
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
    pub value: serde_json::Value,
    pub spec_snapshot_id: Uuid,
    pub input_snapshot_ids: Vec<Uuid>,
    pub inherited_label: SecurityLabel,
    pub evaluated_at: DateTime<Utc>,
}

// ── Function registry ─────────────────────────────────────────

/// Registry mapping function names to evaluation implementations.
pub struct DerivationFunctionRegistry {
    functions: HashMap<String, Arc<dyn DerivationFn>>,
}

impl DerivationFunctionRegistry {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, func: Arc<dyn DerivationFn>) {
        self.functions.insert(name.to_string(), func);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn DerivationFn>> {
        self.functions.get(name)
    }

    /// Evaluate a derivation spec against provided inputs.
    pub fn evaluate(
        &self,
        spec: &DerivationSpecBody,
        inputs: &serde_json::Value,
        input_labels: &[SecurityLabel],
        spec_snapshot_id: Uuid,
        input_snapshot_ids: Vec<Uuid>,
    ) -> Result<DerivationResult, DerivationError> {
        // Check null semantics for required inputs
        for input_def in &spec.inputs {
            if input_def.required {
                let val = inputs.get(&input_def.attribute_fqn);
                if val.is_none() || val == Some(&serde_json::Value::Null) {
                    match &spec.null_semantics {
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
                            // Continue with default — would need to inject into inputs
                            // For now, proceed and let the function handle it
                            let _ = default_val;
                        }
                    }
                }
            }
        }

        // Dispatch to registered function
        let func = match &spec.expression {
            DerivationExpression::FunctionRef { ref_name } => {
                self.get(ref_name)
                    .ok_or_else(|| DerivationError::FunctionNotFound {
                        ref_name: ref_name.clone(),
                    })?
            }
        };

        let value = func.evaluate(inputs)?;
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

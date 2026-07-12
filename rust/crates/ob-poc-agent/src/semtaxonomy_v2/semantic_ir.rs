//! Canonical compiler-facing semantic IR for the NLCI pipeline.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::binding::BindingMode;
use super::intent_schema::IntentParameter;

/// Compiler-normalized target reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticTarget {
    /// Resolved subject kind expected by the compiler.
    pub subject_kind: String,
    /// User-supplied identifier if present.
    pub identifier: Option<String>,
    /// Identifier type if present.
    pub identifier_type: Option<String>,
    /// Session/reference binding if used.
    pub reference: Option<String>,
    /// Optional filter expression.
    pub filter: Option<String>,
}

/// One compiler-ready step in the semantic IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticStep {
    /// Canonical action label used for operation resolution.
    pub action: String,
    /// Canonical entity/domain label used for surface object resolution.
    pub entity: String,
    /// Binding mode selected for this step.
    pub binding_mode: BindingMode,
    /// Compiler-normalized target.
    pub target: Option<SemanticTarget>,
    /// Normalized parameters.
    #[serde(default)]
    pub parameters: Vec<IntentParameter>,
    /// Explicit qualifiers preserved into the compiler layer.
    #[serde(default)]
    pub qualifiers: Vec<(String, String)>,
}

/// Canonical semantic IR consumed by deterministic compiler phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticIr {
    /// Ordered compiler steps.
    #[serde(default)]
    pub steps: Vec<SemanticStep>,
    /// Composition label such as `single_step` or `sequential`.
    pub composition: Option<String>,
}

impl SemanticIr {
    /// Validate the compiler-facing semantic IR against NLCI invariants.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::semtaxonomy_v2::{BindingMode, SemanticIr, SemanticStep};
    ///
    /// let ir = SemanticIr {
    ///     steps: vec![SemanticStep {
    ///         action: "read".to_string(),
    ///         entity: "cbu".to_string(),
    ///         binding_mode: BindingMode::Unbound,
    ///         target: None,
    ///         parameters: vec![],
    ///         qualifiers: vec![],
    ///     }],
    ///     composition: Some("single_step".to_string()),
    /// };
    /// ir.validate_invariants()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn validate_invariants(&self) -> Result<()> {
        if self.steps.is_empty() {
            return Err(anyhow!(
                "NLCI invariant violation: semantic IR must contain at least one step"
            ));
        }

        for step in &self.steps {
            if step.action.trim().is_empty() {
                return Err(anyhow!(
                    "NLCI invariant violation: semantic IR step action must not be empty"
                ));
            }
            if step.entity.trim().is_empty() {
                return Err(anyhow!(
                    "NLCI invariant violation: semantic IR step entity must not be empty"
                ));
            }
        }

        Ok(())
    }
}

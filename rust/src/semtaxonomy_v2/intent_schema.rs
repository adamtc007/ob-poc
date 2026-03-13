//! Canonical Layer 1 structured intent schema for the NLCI pipeline.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Identifier binding supplied in the extracted intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentIdentifier {
    /// User- or system-supplied identifier value.
    pub value: String,
    /// Identifier type label such as `uuid`, `code`, or `name`.
    pub identifier_type: String,
}

/// Target object reference from the extracted intent plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentTarget {
    /// Explicit identifier if the user named a concrete target.
    pub identifier: Option<IntentIdentifier>,
    /// Relative/session reference such as `current` or `active_cbu`.
    pub reference: Option<String>,
    /// Optional filter expression captured from the utterance.
    pub filter: Option<String>,
}

/// Qualifier attached to one extracted intent step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentQualifier {
    /// Qualifier key, for example `phase`, `scope`, or `mode`.
    pub name: String,
    /// Canonical qualifier value.
    pub value: String,
}

/// Parameter captured by Layer 1 extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentParameter {
    /// Parameter name as emitted by the structured extraction layer.
    pub name: String,
    /// Parameter value as normalized text.
    pub value: String,
}

/// One extracted intent step from the natural-language plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentStep {
    /// Action label such as `create`, `read`, `rename`, or `approve`.
    pub action: String,
    /// Entity/domain label such as `cbu`, `document`, or `kyc_case`.
    pub entity: String,
    /// Optional target for the step.
    pub target: Option<IntentTarget>,
    /// Optional qualifiers constraining interpretation.
    #[serde(default)]
    pub qualifiers: Vec<IntentQualifier>,
    /// Extracted parameter assignments.
    #[serde(default)]
    pub parameters: Vec<IntentParameter>,
    /// Confidence label from Layer 1 extraction.
    pub confidence: String,
}

/// Canonical structured intent plan emitted by Layer 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredIntentPlan {
    /// Ordered extracted steps.
    #[serde(default)]
    pub steps: Vec<IntentStep>,
    /// Composition label such as `single_step` or `sequential`.
    pub composition: Option<String>,
    /// Data-flow notes connecting step outputs to later steps.
    #[serde(default)]
    pub data_flow: Vec<String>,
}

impl StructuredIntentPlan {
    /// Validate the Layer 1 structured intent against NLCI invariants.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::semtaxonomy_v2::{IntentStep, StructuredIntentPlan};
    ///
    /// let plan = StructuredIntentPlan {
    ///     steps: vec![IntentStep {
    ///         action: "read".to_string(),
    ///         entity: "cbu".to_string(),
    ///         target: None,
    ///         qualifiers: vec![],
    ///         parameters: vec![],
    ///         confidence: "high".to_string(),
    ///     }],
    ///     composition: Some("single_step".to_string()),
    ///     data_flow: vec![],
    /// };
    /// plan.validate_invariants()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn validate_invariants(&self) -> Result<()> {
        if self.steps.is_empty() {
            return Err(anyhow!(
                "NLCI invariant violation: structured intent plan must contain at least one step"
            ));
        }

        for step in &self.steps {
            if step.action.trim().is_empty() {
                return Err(anyhow!(
                    "NLCI invariant violation: extracted intent step action must not be empty"
                ));
            }
            if step.entity.trim().is_empty() {
                return Err(anyhow!(
                    "NLCI invariant violation: extracted intent step entity must not be empty"
                ));
            }
            if step.action.contains('(') || step.action.contains(':') || step.action.contains(')') {
                return Err(anyhow!(
                    "NLCI invariant violation: extraction must emit structured actions, not DSL text"
                ));
            }
            if step.entity.contains('(') || step.entity.contains(':') || step.entity.contains(')') {
                return Err(anyhow!(
                    "NLCI invariant violation: extraction must emit structured entities, not DSL text"
                ));
            }
        }

        Ok(())
    }
}

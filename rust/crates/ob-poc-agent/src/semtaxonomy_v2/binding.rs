//! Binding modes and typed compiler envelopes for NLCI.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::intent_schema::StructuredIntentPlan;
use super::semantic_ir::SemanticIr;

/// Canonical binding mode used by the deterministic compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingMode {
    /// User supplied a concrete identifier.
    Identifier,
    /// User referred to an entity through session context.
    SessionReference,
    /// User supplied a filter requiring deterministic narrowing.
    Filter,
    /// No binding was provided; creation or generic action surface is expected.
    Unbound,
}

/// Canonical compiler input envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerInputEnvelope {
    /// Layer 1 extracted intent.
    pub structured_intent: StructuredIntentPlan,
    /// Compiler-normalized IR.
    pub semantic_ir: SemanticIr,
    /// Session identifier string if present.
    pub session_id: Option<String>,
    /// Active entity identifier carried into compilation, if any.
    pub session_entity_id: Option<String>,
    /// Active entity kind carried into compilation, if any.
    pub session_entity_kind: Option<String>,
    /// Active entity name carried into compilation, if any.
    pub session_entity_name: Option<String>,
}

/// Candidate considered by the deterministic compiler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerCandidate {
    /// Candidate verb identifier.
    pub verb_id: String,
    /// Selection score for deterministic ranking.
    pub score: f64,
    /// Short explanation of why the candidate survived filtering.
    pub rationale: String,
}

/// Selected compiler outcome before execution handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerSelection {
    /// Selected verb identifier.
    pub verb_id: String,
    /// Bound arguments as canonical string pairs until runtime coercion.
    #[serde(default)]
    pub arguments: Vec<(String, String)>,
    /// Whether the selection requires confirmation.
    pub requires_confirmation: bool,
    /// Human-readable explanation of the deterministic choice.
    pub explanation: String,
}

impl CompilerInputEnvelope {
    /// Validate canonical compiler input envelope invariants.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::semtaxonomy_v2::{BindingMode, CompilerInputEnvelope, IntentStep, SemanticIr, SemanticStep, StructuredIntentPlan};
    ///
    /// let envelope = CompilerInputEnvelope {
    ///     structured_intent: StructuredIntentPlan {
    ///         steps: vec![IntentStep {
    ///             action: "read".to_string(),
    ///             entity: "cbu".to_string(),
    ///             target: None,
    ///             qualifiers: vec![],
    ///             parameters: vec![],
    ///             confidence: "high".to_string(),
    ///         }],
    ///         composition: Some("single_step".to_string()),
    ///         data_flow: vec![],
    ///     },
    ///     semantic_ir: SemanticIr {
    ///         steps: vec![SemanticStep {
    ///             action: "read".to_string(),
    ///             entity: "cbu".to_string(),
    ///             binding_mode: BindingMode::Unbound,
    ///             target: None,
    ///             parameters: vec![],
    ///             qualifiers: vec![],
    ///         }],
    ///         composition: Some("single_step".to_string()),
    ///     },
    ///     session_id: None,
    ///     session_entity_id: None,
    ///     session_entity_kind: None,
    ///     session_entity_name: None,
    /// };
    /// envelope.validate_invariants()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn validate_invariants(&self) -> Result<()> {
        self.structured_intent.validate_invariants()?;
        self.semantic_ir.validate_invariants()?;

        if self.structured_intent.steps.len() != self.semantic_ir.steps.len() {
            return Err(anyhow!(
                "NLCI invariant violation: structured intent step count must match semantic IR step count"
            ));
        }

        Ok(())
    }
}

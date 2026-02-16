//! Orchestrator response types for the compile surface.
//!
//! `process_utterance()` returns exactly one of three variants:
//!
//! 1. **Compiled** — a `CompiledRunbook` was successfully created and is ready
//!    for `execute_runbook()`.
//! 2. **Clarification** — more information is needed before compilation can
//!    proceed (missing args, ambiguous entity, etc.).
//! 3. **ConstraintViolation** — the expanded plan violates active pack
//!    constraints; includes remediation options.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::CompiledRunbookId;

// ---------------------------------------------------------------------------
// OrchestratorResponse
// ---------------------------------------------------------------------------

/// The compile-surface response from `process_utterance()`.
///
/// Exactly one variant is returned per utterance. The caller (REPL, Chat API,
/// MCP) pattern-matches to decide what to show the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OrchestratorResponse {
    /// Compilation succeeded — a runbook is ready for execution.
    Compiled(CompiledRunbookSummary),

    /// More information is needed before compilation can proceed.
    Clarification(ClarificationRequest),

    /// The expanded plan violates active pack constraints.
    ConstraintViolation(ConstraintViolationDetail),
}

impl OrchestratorResponse {
    /// Convenience: is this a successful compilation?
    pub fn is_compiled(&self) -> bool {
        matches!(self, Self::Compiled(_))
    }
}

// ---------------------------------------------------------------------------
// CompiledRunbookSummary
// ---------------------------------------------------------------------------

/// Summary returned when compilation succeeds.
///
/// Does NOT contain the full `CompiledRunbook` — the caller uses the
/// `compiled_runbook_id` to pass to `execute_runbook()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRunbookSummary {
    /// The execution handle to pass to `execute_runbook()`.
    pub compiled_runbook_id: CompiledRunbookId,

    /// Monotonic version within the session.
    pub runbook_version: u64,

    /// Number of steps in the compiled runbook.
    pub step_count: usize,

    /// Entity UUIDs referenced in the replay envelope.
    pub envelope_entity_count: usize,

    /// Human-readable preview of the first few steps.
    pub preview: Vec<StepPreview>,
}

/// Preview of a single compiled step (for UI display before execution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPreview {
    pub step_id: Uuid,
    pub verb: String,
    pub sentence: String,
}

// ---------------------------------------------------------------------------
// ClarificationRequest
// ---------------------------------------------------------------------------

/// Request for additional information before compilation can proceed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationRequest {
    /// Human-readable question to show the user.
    pub question: String,

    /// Fields that are missing or ambiguous.
    pub missing_fields: Vec<MissingField>,

    /// Context about what the system understood so far.
    pub context: ClarificationContext,
}

/// A field that needs user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingField {
    /// Argument name (e.g., `"depositary"`).
    pub field_name: String,

    /// Why it's needed.
    pub reason: String,

    /// Suggested values (if any).
    pub suggestions: Vec<String>,

    /// Whether this field is required vs nice-to-have.
    pub required: bool,
}

/// Context attached to a clarification request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationContext {
    /// Verb that was matched.
    pub verb: Option<String>,

    /// Whether this is a macro or primitive verb.
    pub is_macro: bool,

    /// Arguments that were successfully extracted.
    pub extracted_args: std::collections::HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// ConstraintViolationDetail
// ---------------------------------------------------------------------------

/// Detail returned when the expanded plan violates pack constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintViolationDetail {
    /// Human-readable explanation of the violation.
    pub explanation: String,

    /// The verbs in the expanded plan that violated constraints.
    pub violating_verbs: Vec<String>,

    /// The active constraints that were violated.
    pub active_constraints: Vec<ActiveConstraint>,

    /// Remediation options the user can choose from.
    pub remediation_options: Vec<Remediation>,
}

/// A constraint that was violated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveConstraint {
    /// Pack that imposed this constraint.
    pub pack_id: String,

    /// Pack name for display.
    pub pack_name: String,

    /// Type of constraint.
    pub constraint_type: ConstraintType,
}

/// Type of pack constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    /// Verb is not in the allowed set.
    ForbiddenVerb { verb: String },
    /// Entity kind is not in the allowed set.
    ForbiddenEntityKind { kind: String },
}

/// A remediation option presented to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Remediation {
    /// Widen the scope by activating additional packs.
    WidenScope { suggested_packs: Vec<String> },
    /// Suspend the constraining pack temporarily.
    SuspendPack { pack_id: String, pack_name: String },
    /// Use alternative verbs that are within constraints.
    AlternativeVerbs { alternatives: Vec<AlternativeVerb> },
}

/// An alternative verb that would satisfy constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeVerb {
    pub verb: String,
    pub description: String,
    pub score: f32,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_response_serde_round_trip() {
        let summary = CompiledRunbookSummary {
            compiled_runbook_id: super::super::types::CompiledRunbookId::new(),
            runbook_version: 1,
            step_count: 3,
            envelope_entity_count: 2,
            preview: vec![StepPreview {
                step_id: Uuid::new_v4(),
                verb: "cbu.create".into(),
                sentence: "Create a new fund structure".into(),
            }],
        };
        let resp = OrchestratorResponse::Compiled(summary);
        let json = serde_json::to_string(&resp).unwrap();
        let back: OrchestratorResponse = serde_json::from_str(&json).unwrap();
        assert!(back.is_compiled());
    }

    #[test]
    fn constraint_violation_serde() {
        let violation = OrchestratorResponse::ConstraintViolation(ConstraintViolationDetail {
            explanation: "cbu.delete is not allowed in kyc-case pack".into(),
            violating_verbs: vec!["cbu.delete".into()],
            active_constraints: vec![ActiveConstraint {
                pack_id: "kyc-case".into(),
                pack_name: "KYC Case Management".into(),
                constraint_type: ConstraintType::ForbiddenVerb {
                    verb: "cbu.delete".into(),
                },
            }],
            remediation_options: vec![Remediation::SuspendPack {
                pack_id: "kyc-case".into(),
                pack_name: "KYC Case Management".into(),
            }],
        });
        let json = serde_json::to_string(&violation).unwrap();
        assert!(json.contains("constraint_violation"));
        let back: OrchestratorResponse = serde_json::from_str(&json).unwrap();
        assert!(!back.is_compiled());
    }
}

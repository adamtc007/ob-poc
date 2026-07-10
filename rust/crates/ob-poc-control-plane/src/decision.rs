//! Control Plane Decision Model (V&S §9.3, §10).
//!
//! `evaluate(candidate_intent, context) -> ControlPlaneDecision` is the
//! crate's conceptual core API (§9.3). T1 defines the decision/rejection
//! shape only; the `evaluate` orchestration function itself is wired
//! incrementally as T2's gate adapters land (each adapter is a real
//! `Gate<Ctx>` implementation that `evaluate` will invoke through the
//! `gate::evaluate_collect_where_independent` scaffold).

use crate::envelope::ExecutionEnvelope;
use crate::gate::GateId;
use crate::proof::ControlPlaneProof;

/// The three-way outcome of a control-plane evaluation (§9.3).
#[derive(Debug, Clone)]
pub enum ControlPlaneDecision {
    ApprovedStp(Box<ExecutionEnvelope>),
    RequiresHumanGate(ControlPlaneProof),
    Rejected(ControlPlaneRejection),
}

/// Aggregates every gate's failure under collect-where-independent (§6.16):
/// a rejection reports *every* failed control, not just the first, so it is
/// a better work item for an operator or auditor than a fail-fast trace
/// would be.
#[derive(Debug, Clone, Default)]
pub struct ControlPlaneRejection {
    failures: Vec<GateFailure>,
}

/// One gate's contribution to a `ControlPlaneRejection`.
#[derive(Debug, Clone)]
pub enum GateFailure {
    Failed { gate: GateId, reason: String },
    /// A gate whose declared predecessors did not all succeed, so it was
    /// never evaluated (§6.16: "recorded as `not_evaluated` with the
    /// blocking predecessor named").
    NotEvaluated { gate: GateId, blocked_by: Vec<GateId> },
}

impl ControlPlaneRejection {
    pub fn new(failures: Vec<GateFailure>) -> Self {
        Self { failures }
    }

    pub fn failures(&self) -> &[GateFailure] {
        &self.failures
    }

    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_aggregates_every_failure_not_just_the_first() {
        let rejection = ControlPlaneRejection::new(vec![
            GateFailure::Failed {
                gate: GateId::IntentAdmission,
                reason: "unknown verb".to_string(),
            },
            GateFailure::NotEvaluated {
                gate: GateId::EntityBinding,
                blocked_by: vec![GateId::IntentAdmission],
            },
        ]);
        assert_eq!(rejection.failures().len(), 2);
        assert!(!rejection.is_empty());
    }
}

//! G4 — DAG and State-Slot Enforcement (V&S §6.4).
//!
//! T2.2 wires the adapter over `GateChecker::check_transition` (ledger
//! C-024, C-025, C-026); the T0.2 lifecycle check (`LifecycleGateMode`,
//! C-027) becomes a second input source and is unified here. `GateChecker`
//! reports `Vec<GateViolation>` (constraint id + severity + message), not a
//! pre-classified failure category, so this adapter does not invent a
//! finer-grained split than the underlying validator provides: any blocking
//! violation (from either source) maps to `GuardFailed`, carrying the
//! validator's own message — `IllegalFromState`/`IllegalToState`/
//! `Unreachable`/`WrongLifecycleAxis`/`TransitionUnimplemented` stay
//! reserved for a future validator that actually distinguishes them (RR-8
//! candidate, not fabricated here).

use uuid::Uuid;

use crate::gate::{Gate, GateId, GateResult};

/// `StateTransitionProof` — V&S §6.4 "Output". Variant names mirror the
/// possible outcomes listed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateTransitionOutcome {
    Legal(LegalTransition),
    IllegalFromState,
    IllegalToState,
    Unreachable,
    WrongLifecycleAxis,
    TransitionUnimplemented,
    GuardFailed { reason: String },
}

/// Success-form proof: the proposed action is legal for this entity, in
/// this state, through this governed state slot. Constructible only from
/// within this module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LegalTransition {
    entity_id: Uuid,
    from_state: String,
    to_state: String,
}

impl LegalTransition {
    // Called by the (future) T2.2 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(entity_id: Uuid, from_state: impl Into<String>, to_state: impl Into<String>) -> Self {
        Self {
            entity_id,
            from_state: from_state.into(),
            to_state: to_state.into(),
        }
    }

    pub fn entity_id(&self) -> Uuid {
        self.entity_id
    }

    pub fn from_state(&self) -> &str {
        &self.from_state
    }

    pub fn to_state(&self) -> &str {
        &self.to_state
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::LegalTransition;
    use uuid::Uuid;

    pub(crate) fn legal(entity_id: Uuid, from_state: &str, to_state: &str) -> LegalTransition {
        LegalTransition::new(entity_id, from_state, to_state)
    }
}

/// Pre-computed input for the DAG proof gate. `blocking_violations` is the
/// `severity=error` subset of `GateChecker::check_transition`'s
/// `Vec<GateViolation>` (C-025/C-026), stringified at the call site.
/// `lifecycle_fail_open_class`/`lifecycle_gate_mode_fail_closed` carry the
/// T0.2 `requires_states` precondition result (C-027) so this gate can
/// unify both semantics in one decision rather than the two independently
/// diverging as they did before T0.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagProofInput {
    pub entity_id: Uuid,
    pub from_state: String,
    pub to_state: String,
    pub blocking_violations: Vec<String>,
    pub lifecycle_fail_open_class: Option<String>,
    pub lifecycle_gate_mode_fail_closed: bool,
}

fn decide(input: &DagProofInput) -> StateTransitionOutcome {
    if !input.blocking_violations.is_empty() {
        return StateTransitionOutcome::GuardFailed {
            reason: input.blocking_violations.join("; "),
        };
    }
    if let Some(class) = &input.lifecycle_fail_open_class {
        if input.lifecycle_gate_mode_fail_closed {
            return StateTransitionOutcome::GuardFailed {
                reason: format!("lifecycle precondition fail-open class: {class}"),
            };
        }
    }
    StateTransitionOutcome::Legal(LegalTransition::new(
        input.entity_id,
        input.from_state.clone(),
        input.to_state.clone(),
    ))
}

/// T2.2 adapter: `Gate<crate::context::EvaluationContext>` impl for G4.
pub struct DagProofGate;

impl Gate<crate::context::EvaluationContext> for DagProofGate {
    fn id(&self) -> GateId {
        GateId::DagProof
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.dag_proof else {
            return GateResult::Failure("no DagProofInput supplied".to_string());
        };
        match decide(input) {
            StateTransitionOutcome::Legal(_) => GateResult::Success,
            other => GateResult::Failure(format!("{other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transition_is_constructible_within_its_own_module() {
        let transition = LegalTransition::new(Uuid::nil(), "VALIDATION_PENDING", "VALIDATED");
        assert_eq!(transition.from_state(), "VALIDATION_PENDING");
        assert_eq!(transition.to_state(), "VALIDATED");
    }

    fn base_input() -> DagProofInput {
        DagProofInput {
            entity_id: Uuid::nil(),
            from_state: "VALIDATION_PENDING".to_string(),
            to_state: "VALIDATED".to_string(),
            blocking_violations: vec![],
            lifecycle_fail_open_class: None,
            lifecycle_gate_mode_fail_closed: false,
        }
    }

    #[test]
    fn no_violations_and_no_lifecycle_class_is_legal() {
        assert_eq!(
            decide(&base_input()),
            StateTransitionOutcome::Legal(LegalTransition::new(
                Uuid::nil(),
                "VALIDATION_PENDING",
                "VALIDATED"
            ))
        );
    }

    #[test]
    fn blocking_violation_fails_guard_regardless_of_lifecycle_mode() {
        let input = DagProofInput {
            blocking_violations: vec!["cbu_operationally_active requires kyc.person.approve".to_string()],
            ..base_input()
        };
        assert!(matches!(decide(&input), StateTransitionOutcome::GuardFailed { .. }));
    }

    #[test]
    fn lifecycle_fail_open_class_passes_when_mode_is_fail_open() {
        let input = DagProofInput {
            lifecycle_fail_open_class: Some("NoSlotMapping".to_string()),
            lifecycle_gate_mode_fail_closed: false,
            ..base_input()
        };
        assert!(matches!(decide(&input), StateTransitionOutcome::Legal(_)));
    }

    #[test]
    fn lifecycle_fail_open_class_fails_when_mode_is_fail_closed() {
        let input = DagProofInput {
            lifecycle_fail_open_class: Some("NoSlotMapping".to_string()),
            lifecycle_gate_mode_fail_closed: true,
            ..base_input()
        };
        assert!(matches!(decide(&input), StateTransitionOutcome::GuardFailed { .. }));
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(DagProofGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(DagProofGate.id(), GateId::DagProof);
    }
}

//! G4 — DAG and State-Slot Enforcement (V&S §6.4).
//!
//! T2.2 wires the adapter over `GateChecker::check_transition` (ledger
//! C-025, C-026); the T0.2 lifecycle check (`LifecycleGateMode`) becomes a
//! second input source and is unified here, marking `requires_states`
//! logic RETIRE-pending in the ownership ledger.

use uuid::Uuid;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transition_is_constructible_within_its_own_module() {
        let transition = LegalTransition::new(Uuid::nil(), "VALIDATION_PENDING", "VALIDATED");
        assert_eq!(transition.from_state(), "VALIDATION_PENDING");
        assert_eq!(transition.to_state(), "VALIDATED");
    }
}

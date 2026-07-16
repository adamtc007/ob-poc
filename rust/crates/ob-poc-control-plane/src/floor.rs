//! T11.F — The Definitional Floor (EOP-PLAN-CONTROLPLANE-002 v0.1).
//!
//! The single source of truth for which gate outcomes are **definitional**
//! (unconditional — no legitimate traffic can produce them; a floor
//! rejection is dictionary-lookup failure, structurally independent of
//! `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`/shadow-vs-enforce mode) versus
//! **judgmental** (policy/authority/evidence-shaped — remain shadow-first/
//! graduated, unchanged by this module).
//!
//! # Why this lives here, in one place
//!
//! Design doc `EOP-DESIGN-CONTROLPLANE-T11.F.2-DEFINITIONAL-FLOOR-001`
//! §1.1's drift guard: the floor-eligible outcome subset must be defined
//! in exactly one place, not duplicated at each of the four ingress call
//! sites — with an exhaustive match (no wildcard arm) so a future variant
//! added to any of the three gate enums fails to compile here until
//! explicitly classified, the same discipline `GATE_DEPENDENCIES`
//! (`gate.rs`) already establishes for dependency ordering.
//!
//! # G1 is deliberately absent from this module
//!
//! G1 (Intent Admission)'s floor check does not classify an
//! `IntentAdmissionDecision` at all — `intent_admission.rs::decide()` has
//! a real, documented defect (ownership ledger, "Defect register — G1")
//! that makes it unable to discriminate "verb doesn't exist" from "verb
//! exists but is policy-denied" in production. The floor's G1 check
//! bypasses `decide()` entirely via a direct verb-registry lookup, done in
//! `ob-poc` (this crate has no access to `dsl_v2::runtime_registry`, and
//! should not gain one merely to host this classification — see B1/B8).
//! Nothing here is "the G1 floor logic"; `ob-poc`'s own call sites own it.

use crate::dag_proof::StateTransitionOutcome;
use crate::pack_resolution::PackResolutionOutcome;

/// G3 (Pack Resolution) floor eligibility.
///
/// `MissingPack`/`AmbiguousPack` are structural facts about SemOS
/// pack-resolution state (no candidate / more than one) — definitional.
/// `PackDeniesIntent`/`PackDeniesEntity` are pack-authored business rules
/// — judgmental, stay shadow. `Resolved(_)` is success, not a rejection at
/// all — `false` here, not "not applicable"; callers must not treat a
/// non-floor-eligible outcome as automatically a pass.
pub fn g3_is_floor_eligible(outcome: &PackResolutionOutcome) -> bool {
    match outcome {
        PackResolutionOutcome::Resolved(_) => false,
        PackResolutionOutcome::MissingPack => true,
        PackResolutionOutcome::AmbiguousPack => true,
        PackResolutionOutcome::PackDeniesIntent => false,
        PackResolutionOutcome::PackDeniesEntity => false,
    }
}

/// G4 (DAG Legality) floor eligibility over a fully-resolved
/// [`StateTransitionOutcome`].
///
/// The five named topological variants (`IllegalFromState`/
/// `IllegalToState`/`Unreachable`/`WrongLifecycleAxis`/
/// `TransitionUnimplemented`) are, per `dag_proof.rs`'s own module doc,
/// **reserved for a future validator that actually distinguishes them —
/// never constructed by the current `decide()`** (confirmed: `decide()`'s
/// only two real branches are `GuardFailed` and `Legal`). They are
/// included here, floor-eligible, for when that future validator lands —
/// not vacuous cowardice, but not load-bearing today either; the real
/// production floor surface for G4 is entirely `GuardFailed`, which this
/// function deliberately does NOT classify (see
/// [`g4_blocking_violations_are_floor_eligible`] below) because
/// `GuardFailed { reason }` collapses two different sources (DAG-taxonomy
/// `blocking_violations`, definitional; `lifecycle_fail_open_class`,
/// judgmental) into one untyped string with no discriminator — the same
/// conflation shape as G1's defect, manifesting post-`decide()` rather
/// than pre-. Classifying `GuardFailed` here would silently pick one side
/// or the other with no way to be right for both real callers.
pub fn g4_is_floor_eligible(outcome: &StateTransitionOutcome) -> bool {
    match outcome {
        StateTransitionOutcome::Legal(_) => false,
        StateTransitionOutcome::IllegalFromState => true,
        StateTransitionOutcome::IllegalToState => true,
        StateTransitionOutcome::Unreachable => true,
        StateTransitionOutcome::WrongLifecycleAxis => true,
        StateTransitionOutcome::TransitionUnimplemented => true,
        StateTransitionOutcome::GuardFailed { .. } => false,
    }
}

/// G4 floor eligibility for the `GuardFailed` case specifically — checked
/// on the **input**, before `decide()` collapses it, per the design doc's
/// §2 resolution.
///
/// `blocking_violations` (traced: `gate_checker.rs::check_transition` →
/// `step_executor_bridge.rs::resolve_transition_probe`, error-severity
/// `GateViolation`s from DAG-taxonomy-declared `CrossWorkspaceConstraint`s
/// — a static, YAML-authored topology fact) is definitional. A non-empty
/// `blocking_violations` list is floor-eligible regardless of whether
/// `lifecycle_fail_open_class` is also set — `decide()` checks
/// `blocking_violations` first and unconditionally, so a non-empty list
/// is what actually produces `GuardFailed` in that case; the
/// `lifecycle_fail_open_class` branch is only ever reached when
/// `blocking_violations` is empty (confirmed by reading `decide()`'s own
/// control flow, not assumed).
pub fn g4_blocking_violations_are_floor_eligible(blocking_violations: &[String]) -> bool {
    !blocking_violations.is_empty()
}

/// G3 floor check from the raw, publicly-constructible
/// [`PackResolutionInput`](crate::pack_resolution::PackResolutionInput) —
/// the entry point external callers (`ob-poc`'s `phase5_runtime_recheck`)
/// actually use, since `pack_resolution::decide` is `pub(crate)` and stays
/// that way (only this module needs to call it to classify floor
/// eligibility; widening its visibility would let callers bypass this
/// single source of truth and hand-roll their own classification).
pub fn g3_input_is_floor_eligible(input: &crate::pack_resolution::PackResolutionInput) -> bool {
    g3_is_floor_eligible(&crate::pack_resolution::decide(input))
}

/// G4 floor check from the raw, publicly-constructible
/// [`DagProofInput`](crate::dag_proof::DagProofInput) — same rationale as
/// [`g3_input_is_floor_eligible`]. Checks `blocking_violations` directly
/// (per this module's own doc: `GuardFailed` is not classified at the
/// outcome level because it conflates two sources) alongside the five
/// topological variants.
pub fn g4_input_is_floor_eligible(input: &crate::dag_proof::DagProofInput) -> bool {
    g4_blocking_violations_are_floor_eligible(&input.blocking_violations)
        || g4_is_floor_eligible(&crate::dag_proof::decide(input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // ── Drift guard: exhaustive match, no wildcard arm ──────────────────
    //
    // These tests don't assert anything beyond "the function compiles and
    // runs" — their real value is that `g3_is_floor_eligible`/
    // `g4_is_floor_eligible`'s own `match` bodies have no `_ =>` arm. If a
    // future PackResolutionOutcome/StateTransitionOutcome variant is added
    // without updating this module, the crate fails to compile here,
    // before it fails anywhere else.

    #[test]
    fn g3_missing_and_ambiguous_pack_are_floor_eligible() {
        assert!(g3_is_floor_eligible(&PackResolutionOutcome::MissingPack));
        assert!(g3_is_floor_eligible(&PackResolutionOutcome::AmbiguousPack));
    }

    #[test]
    fn g3_pack_denies_are_not_floor_eligible() {
        assert!(!g3_is_floor_eligible(&PackResolutionOutcome::PackDeniesIntent));
        assert!(!g3_is_floor_eligible(&PackResolutionOutcome::PackDeniesEntity));
    }

    #[test]
    fn g3_resolved_is_not_floor_eligible() {
        // A crate-internal construction path exists via tests_support in
        // other gate modules; PackResolutionOutcome::Resolved isn't
        // publicly constructible outside pack_resolution.rs, so this test
        // lives there instead — this module only classifies the two
        // definitional variants and the two judgmental ones, which don't
        // need `Resolved` to prove the point. Cross-referenced, not
        // duplicated.
    }

    #[test]
    fn g4_topological_variants_are_floor_eligible() {
        assert!(g4_is_floor_eligible(&StateTransitionOutcome::IllegalFromState));
        assert!(g4_is_floor_eligible(&StateTransitionOutcome::IllegalToState));
        assert!(g4_is_floor_eligible(&StateTransitionOutcome::Unreachable));
        assert!(g4_is_floor_eligible(&StateTransitionOutcome::WrongLifecycleAxis));
        assert!(g4_is_floor_eligible(&StateTransitionOutcome::TransitionUnimplemented));
    }

    #[test]
    fn g4_guard_failed_is_not_floor_eligible_at_the_outcome_level() {
        assert!(!g4_is_floor_eligible(&StateTransitionOutcome::GuardFailed {
            reason: "anything".to_string(),
        }));
    }

    #[test]
    fn g4_legal_is_not_floor_eligible() {
        let legal = crate::dag_proof::tests_support::legal(Uuid::nil(), "A", "B");
        assert!(!g4_is_floor_eligible(&StateTransitionOutcome::Legal(legal)));
    }

    #[test]
    fn g4_blocking_violations_input_check() {
        assert!(g4_blocking_violations_are_floor_eligible(&[
            "v1.3 gate violation [c1]: blocked".to_string()
        ]));
        assert!(!g4_blocking_violations_are_floor_eligible(&[]));
    }

    #[test]
    fn g3_input_missing_pack_is_floor_eligible() {
        let input = crate::pack_resolution::PackResolutionInput {
            candidate_pack_ids: vec![],
            semreg_allowed_set_available: false,
            constraint_denies_intent: false,
        };
        assert!(g3_input_is_floor_eligible(&input));
    }

    #[test]
    fn g3_input_pack_denies_intent_is_not_floor_eligible() {
        let input = crate::pack_resolution::PackResolutionInput {
            candidate_pack_ids: vec!["pack.a".to_string()],
            semreg_allowed_set_available: true,
            constraint_denies_intent: true,
        };
        assert!(!g3_input_is_floor_eligible(&input));
    }

    #[test]
    fn g4_input_blocking_violation_is_floor_eligible() {
        let input = crate::dag_proof::DagProofInput {
            entity_id: Uuid::nil(),
            from_state: "A".to_string(),
            to_state: "B".to_string(),
            blocking_violations: vec!["v1.3 gate violation [c1]: blocked".to_string()],
            lifecycle_fail_open_class: None,
            lifecycle_gate_mode_fail_closed: false,
        };
        assert!(g4_input_is_floor_eligible(&input));
    }

    #[test]
    fn g4_input_lifecycle_fail_open_is_not_floor_eligible() {
        let input = crate::dag_proof::DagProofInput {
            entity_id: Uuid::nil(),
            from_state: "A".to_string(),
            to_state: "B".to_string(),
            blocking_violations: vec![],
            lifecycle_fail_open_class: Some("some-class".to_string()),
            lifecycle_gate_mode_fail_closed: true,
        };
        assert!(!g4_input_is_floor_eligible(&input));
    }
}

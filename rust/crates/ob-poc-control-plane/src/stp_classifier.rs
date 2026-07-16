//! G8 — STP Eligibility Classification (V&S §6.8).
//!
//! No production analogue exists today. T3.3 implements the real
//! classifier as a pure function over aggregated gate results + config
//! policy; it must be deterministic — the same (intent, ctx, pins) always
//! yields the same classification (§12.8, §12.11).
//!
//! The AI must not self-certify STP eligibility (§6.8) — this module's
//! classifier is the only code path permitted to produce a
//! `StpEligibilityDecision`.
//!
//! `evaluate_collect_where_independent` (T1.3) already enforces the
//! "aggregated gate results" half of this gate's job structurally: per
//! `GATE_DEPENDENCIES`, `StpClassifier` is only ever called once
//! `IntentAdmission`, `EntityBinding`, `PackResolution`, `DagProof`,
//! `Authority`, `Evidence`, and `WriteSet` have all independently reported
//! `Success` — a gate whose `evaluate` runs never sees a failed
//! predecessor. This module's own responsibility is the residual policy
//! §6.8 layers on top of "everything upstream passed": the durable-verb
//! rule (C-028) and plan A5's unpinned-entity cap.

use crate::gate::{Gate, GateId, GateResult};

/// `StpEligibilityDecision` — V&S §6.8 "Output" / classification table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StpEligibilityDecision {
    /// Valid, authorised, evidence complete, no human gate required.
    StpExecutable,
    /// Valid plan, but approval/review required before execution.
    HumanGated,
    /// Invalid, ambiguous, unauthorised, incomplete or outside scope.
    Rejected,
}

/// Pre-computed input for the STP classifier — the residual policy facts
/// not already captured by the seven upstream gates succeeding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StpClassifierInput {
    /// `true` when the candidate verb is on the durable-verb list
    /// (`DslExecutor`'s C-028 rule: durable verbs are rejected unless
    /// direct durable execution is explicitly allowed).
    pub is_durable_verb: bool,
    pub durable_execution_explicitly_allowed: bool,
    /// Every bound entity lacking a comparable version pin
    /// (`SnapshotPins::unpinned_entities`) — plan A5: any such entity caps
    /// the classification at `HumanGated`, never `StpExecutable`.
    pub has_unpinned_entities: bool,
}

/// Pure classification function — deterministic by construction (no I/O, no
/// non-deterministic iteration: a single `bool`/`bool`/`bool` input always
/// yields the same `StpEligibilityDecision`).
pub fn classify(input: &StpClassifierInput) -> StpEligibilityDecision {
    if input.is_durable_verb && !input.durable_execution_explicitly_allowed {
        return StpEligibilityDecision::Rejected;
    }
    if input.has_unpinned_entities {
        return StpEligibilityDecision::HumanGated;
    }
    StpEligibilityDecision::StpExecutable
}

/// T3.3 adapter: `Gate<crate::context::EvaluationContext>` impl for G8.
/// `GateResult` is binary (success/failure), so `HumanGated` and `Rejected`
/// both map to `Failure` here — callers that need the 3-way distinction
/// call `classify` directly (e.g. the future `decision::evaluate`
/// orchestration assembling `ControlPlaneDecision::RequiresHumanGate` vs
/// `Rejected`); the evaluator report only needs to know "auto-executable or
/// not."
pub struct StpClassifierGate;

impl Gate<crate::context::EvaluationContext> for StpClassifierGate {
    fn id(&self) -> GateId {
        GateId::StpClassifier
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.stp_classifier else {
            return GateResult::Failure("no StpClassifierInput supplied".to_string());
        };
        match classify(input) {
            StpEligibilityDecision::StpExecutable => GateResult::Success,
            StpEligibilityDecision::HumanGated => GateResult::Failure("requires_human_gate".to_string()),
            StpEligibilityDecision::Rejected => GateResult::Failure("rejected".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input() -> StpClassifierInput {
        StpClassifierInput {
            is_durable_verb: false,
            durable_execution_explicitly_allowed: false,
            has_unpinned_entities: false,
        }
    }

    #[test]
    fn clean_input_is_stp_executable() {
        assert_eq!(classify(&base_input()), StpEligibilityDecision::StpExecutable);
    }

    #[test]
    fn durable_verb_without_explicit_allow_is_rejected() {
        let input = StpClassifierInput {
            is_durable_verb: true,
            ..base_input()
        };
        assert_eq!(classify(&input), StpEligibilityDecision::Rejected);
    }

    #[test]
    fn durable_verb_with_explicit_allow_is_not_rejected_on_that_basis() {
        let input = StpClassifierInput {
            is_durable_verb: true,
            durable_execution_explicitly_allowed: true,
            ..base_input()
        };
        assert_eq!(classify(&input), StpEligibilityDecision::StpExecutable);
    }

    #[test]
    fn unpinned_entity_caps_at_human_gated_even_when_otherwise_clean() {
        let input = StpClassifierInput {
            has_unpinned_entities: true,
            ..base_input()
        };
        assert_eq!(classify(&input), StpEligibilityDecision::HumanGated);
    }

    #[test]
    fn rejected_durable_verb_takes_precedence_over_unpinned_entity_cap() {
        let input = StpClassifierInput {
            is_durable_verb: true,
            has_unpinned_entities: true,
            ..base_input()
        };
        assert_eq!(classify(&input), StpEligibilityDecision::Rejected);
    }

    #[test]
    fn classification_is_deterministic_across_many_reevaluations() {
        // Exit criterion: "same (intent, ctx, pins) -> identical decision
        // across 1,000 randomized re-evaluations." classify() is a pure
        // function over three bools with no I/O — every fixed input must
        // produce the identical decision on every one of 1,000 calls.
        let inputs = [
            base_input(),
            StpClassifierInput {
                is_durable_verb: true,
                ..base_input()
            },
            StpClassifierInput {
                has_unpinned_entities: true,
                ..base_input()
            },
            StpClassifierInput {
                is_durable_verb: true,
                durable_execution_explicitly_allowed: true,
                has_unpinned_entities: true,
            },
        ];
        for input in &inputs {
            let first = classify(input);
            for _ in 0..1000 {
                assert_eq!(classify(input), first);
            }
        }
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(StpClassifierGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(StpClassifierGate.id(), GateId::StpClassifier);
    }

    #[test]
    fn gate_evaluate_reports_success_for_stp_executable() {
        let ctx = crate::context::EvaluationContext {
            stp_classifier: Some(base_input()),
            ..Default::default()
        };
        assert_eq!(StpClassifierGate.evaluate(&ctx), GateResult::Success);
    }

    #[test]
    fn gate_evaluate_reports_failure_for_human_gated() {
        let ctx = crate::context::EvaluationContext {
            stp_classifier: Some(StpClassifierInput {
                has_unpinned_entities: true,
                ..base_input()
            }),
            ..Default::default()
        };
        assert!(matches!(StpClassifierGate.evaluate(&ctx), GateResult::Failure(_)));
    }
}

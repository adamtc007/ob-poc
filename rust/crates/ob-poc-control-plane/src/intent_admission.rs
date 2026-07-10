//! G1 — Intent Admission (V&S §6.1).
//!
//! T2.1 wires the adapter over `SessionVerbSurface` + `SemOsContextEnvelope`
//! (ledger C-002, C-005, C-007, C-009, C-011, C-012, C-013, C-029, C-035,
//! C-040). This module never recomputes verb-surface membership or SemOS
//! CCIR/ABAC pruning — it grades an already-computed `IntentAdmissionInput`
//! (built at the call site from `SessionVerbSurface::allowed_fqns()` /
//! `contains()` and `SemOsContextEnvelope.pruned_verbs`) and, net-new per
//! V&S §6.13.1 (no prior analogue — Phase 0 RR-3 found none), requires an
//! interpretation attestation on AI-originated candidates.

use crate::gate::{Gate, GateId, GateResult};
use uuid::Uuid;

/// `IntentAdmissionDecision` — V&S §6.1 "Output". Variant names mirror the
/// possible outcomes listed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentAdmissionDecision {
    Admitted(AdmittedIntent),
    RejectedUnknownIntent,
    RejectedOutsidePack,
    RejectedDeprecated,
    RejectedUnauthorisedSurface,
    /// A candidate intent lacking a valid interpretation attestation
    /// (§6.13.1) for an AI-originated request.
    RejectedAttestationInsufficient,
}

/// Success-form proof: the intent is recognised, in-pack, current, and
/// (for AI-originated intents) carries a valid attestation.
///
/// Constructible only from within this module — the only place code can
/// obtain an `AdmittedIntent` is by matching
/// `IntentAdmissionDecision::Admitted(_)`, which in turn is only produced
/// by this gate's own (future, T2.1) evaluation logic.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AdmittedIntent {
    intent_id: Uuid,
    verb_fqn: String,
    /// `true` for AI-originated intents whose interpretation attestation
    /// (§6.13.1) was present and valid; always `true` for operator-typed
    /// intents (no attestation requirement).
    attested: bool,
}

impl AdmittedIntent {
    // Called by the (future) T2.1 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(intent_id: Uuid, verb_fqn: impl Into<String>, attested: bool) -> Self {
        Self {
            intent_id,
            verb_fqn: verb_fqn.into(),
            attested,
        }
    }

    pub fn intent_id(&self) -> Uuid {
        self.intent_id
    }

    pub fn verb_fqn(&self) -> &str {
        &self.verb_fqn
    }

    pub fn attested(&self) -> bool {
        self.attested
    }
}

/// Test-only fixture bridge. `cfg(test)`-gated (compiled out entirely in
/// non-test builds), so it does not loosen the production visibility of
/// `AdmittedIntent::new` — it exists so crate-internal integration-style
/// tests elsewhere (e.g. `envelope::tests`) can obtain a fixture without
/// duplicating this module's construction logic.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::AdmittedIntent;
    use uuid::Uuid;

    pub(crate) fn admitted(id: Uuid, verb_fqn: &str) -> AdmittedIntent {
        AdmittedIntent::new(id, verb_fqn, true)
    }
}

/// Pre-computed input for the intent admission gate. Built at the call site
/// by translating `SessionVerbSurface` (C-009: AgentMode + scope/workflow +
/// SemReg CCIR + fail policy + ranking already applied) and
/// `SemOsContextEnvelope.pruned_verbs` (C-007) — this struct is a plain
/// snapshot of their outcome, not a re-implementation of the pruning logic
/// itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntentAdmissionInput {
    pub intent_id: Uuid,
    pub verb_fqn: String,
    /// `true` iff the verb survived into `SessionVerbSurface::allowed_fqns()`
    /// (equivalently `.contains(verb_fqn)`), i.e. it was not pruned by
    /// AgentMode/WorkflowPhase/GroupScope/SemRegCcir/LifecycleState/
    /// ActorGating/FailPolicy (C-009) nor by `SemOsContextEnvelope`'s ABAC/
    /// entity-kind/agent-mode/policy prune reasons (C-007).
    pub is_admitted: bool,
    /// Human-readable prune reasons when `is_admitted` is `false` — mirrors
    /// `PruneReason`/`SurfacePrune` variant names, stringified at the call
    /// site so this crate carries no dependency on `ob-poc`'s types.
    pub exclusion_reasons: Vec<String>,
    /// `true` when the candidate intent was produced by an LLM/agent
    /// pipeline rather than typed verbatim by an operator. AI-originated
    /// intents require an interpretation attestation (§6.13.1); operator
    /// intents do not.
    pub is_ai_originated: bool,
    /// `true` iff a valid interpretation attestation (Sage pre-classification and intent telemetry) was present for this candidate; ignored unless `is_ai_originated` is `true`.
    pub interpretation_attested: bool,
}

/// Grades an already-computed `IntentAdmissionInput`. Pure function — no I/O,
/// no recomputation of surface membership.
fn decide(input: &IntentAdmissionInput) -> IntentAdmissionDecision {
    if !input.is_admitted {
        return if input.exclusion_reasons.iter().any(|r| r == "unknown_intent") {
            IntentAdmissionDecision::RejectedUnknownIntent
        } else if input.exclusion_reasons.iter().any(|r| r == "outside_pack") {
            IntentAdmissionDecision::RejectedOutsidePack
        } else if input.exclusion_reasons.iter().any(|r| r == "deprecated") {
            IntentAdmissionDecision::RejectedDeprecated
        } else {
            IntentAdmissionDecision::RejectedUnauthorisedSurface
        };
    }
    if input.is_ai_originated && !input.interpretation_attested {
        return IntentAdmissionDecision::RejectedAttestationInsufficient;
    }
    IntentAdmissionDecision::Admitted(AdmittedIntent::new(
        input.intent_id,
        input.verb_fqn.clone(),
        input.interpretation_attested || !input.is_ai_originated,
    ))
}

/// T2.1 adapter: `Gate<crate::context::EvaluationContext>` impl for G1.
pub struct IntentAdmissionGate;

impl Gate<crate::context::EvaluationContext> for IntentAdmissionGate {
    fn id(&self) -> GateId {
        GateId::IntentAdmission
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.intent_admission else {
            return GateResult::Failure("no IntentAdmissionInput supplied".to_string());
        };
        match decide(input) {
            IntentAdmissionDecision::Admitted(_) => GateResult::Success,
            other => GateResult::Failure(format!("{other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admitted_intent_is_constructible_within_its_own_module() {
        let id = Uuid::nil();
        let admitted = AdmittedIntent::new(id, "cbu.confirm", true);
        assert_eq!(admitted.intent_id(), id);
        assert_eq!(admitted.verb_fqn(), "cbu.confirm");
        assert!(admitted.attested());
    }

    fn base_input() -> IntentAdmissionInput {
        IntentAdmissionInput {
            intent_id: Uuid::nil(),
            verb_fqn: "cbu.confirm".to_string(),
            is_admitted: true,
            exclusion_reasons: vec![],
            is_ai_originated: false,
            interpretation_attested: false,
        }
    }

    #[test]
    fn operator_typed_intent_admitted_without_attestation() {
        let input = base_input();
        assert_eq!(
            decide(&input),
            IntentAdmissionDecision::Admitted(AdmittedIntent::new(Uuid::nil(), "cbu.confirm", true))
        );
    }

    #[test]
    fn ai_originated_intent_without_attestation_is_rejected() {
        let input = IntentAdmissionInput {
            is_ai_originated: true,
            interpretation_attested: false,
            ..base_input()
        };
        assert_eq!(decide(&input), IntentAdmissionDecision::RejectedAttestationInsufficient);
    }

    #[test]
    fn ai_originated_intent_with_attestation_is_admitted() {
        let input = IntentAdmissionInput {
            is_ai_originated: true,
            interpretation_attested: true,
            ..base_input()
        };
        assert_eq!(
            decide(&input),
            IntentAdmissionDecision::Admitted(AdmittedIntent::new(Uuid::nil(), "cbu.confirm", true))
        );
    }

    #[test]
    fn pruned_verb_is_rejected_unauthorised_surface_by_default() {
        let input = IntentAdmissionInput {
            is_admitted: false,
            exclusion_reasons: vec!["AbacDenied".to_string()],
            ..base_input()
        };
        assert_eq!(decide(&input), IntentAdmissionDecision::RejectedUnauthorisedSurface);
    }

    #[test]
    fn unknown_verb_is_rejected_unknown_intent() {
        let input = IntentAdmissionInput {
            is_admitted: false,
            exclusion_reasons: vec!["unknown_intent".to_string()],
            ..base_input()
        };
        assert_eq!(decide(&input), IntentAdmissionDecision::RejectedUnknownIntent);
    }

    #[test]
    fn gate_evaluate_reports_success_on_admission() {
        let ctx = crate::context::EvaluationContext {
            intent_admission: Some(base_input()),
            ..Default::default()
        };
        assert_eq!(IntentAdmissionGate.evaluate(&ctx), GateResult::Success);
        assert_eq!(IntentAdmissionGate.id(), GateId::IntentAdmission);
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(
            IntentAdmissionGate.evaluate(&ctx),
            GateResult::Failure(_)
        ));
    }
}

//! G5 — Authority and Policy Gate (V&S §6.5).
//!
//! T2.4 wires the adapter over `AccessDecision` partitioning +
//! `ActorResolver` (ledger C-002, C-005, C-006, C-007, C-008, C-016); the
//! TOCTOU recheck result (`TocTouResult`) is consumed as snapshot evidence,
//! not re-implemented. Evidence readiness is owned by the evidence gate
//! (§6.6) — this gate consumes that gate's outcome as a policy-time input,
//! not a hard dependency (see the comment on `GATE_DEPENDENCIES` in
//! `gate.rs`).
//!
//! `Authorised::actor_id` is `String` (not `Uuid`) because the wrapped
//! `sem_os_policy::abac::ActorContext.actor_id` this gate reads is itself a
//! `String` (session/env-derived, not always a UUID) — this crate does not
//! invent a stronger type than the validator it wraps actually provides.

use crate::gate::{Gate, GateId, GateResult};

/// `AuthorityDecision` — V&S §6.5 "Output". Variant names mirror the
/// possible outcomes listed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityOutcome {
    Authorised(Authorised),
    RequiresHumanApproval,
    RequiresSecondLineReview,
    RejectedUnauthorised,
    RejectedSegregationOfDuties,
    RejectedPolicy,
}

/// Success-form proof: the actor/context is authorised to execute the
/// proposed command. Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Authorised {
    actor_id: String,
    role: String,
}

impl Authorised {
    // Called by the (future) T2.4 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(actor_id: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            actor_id: actor_id.into(),
            role: role.into(),
        }
    }

    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub fn role(&self) -> &str {
        &self.role
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod tests_support {
    use super::Authorised;

    pub fn authorised(actor_id: &str, role: &str) -> Authorised {
        Authorised::new(actor_id, role)
    }
}

/// Pre-computed input for the authority gate. `access_decision` mirrors
/// `sem_os_policy::abac::AccessDecision` (Allow / Deny / AllowWithMasking)
/// stringified at the call site; `masked_fields` is populated only for the
/// `AllowWithMasking` case. `toctou_drifted` carries a `TocTouResult` that
/// is not `StillAllowed` (C-008) — this gate consumes that outcome, it does
/// not recompute the row-version comparison itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthorityInput {
    pub actor_id: String,
    pub role: String,
    pub access_decision: AccessDecisionKind,
    pub deny_reason: Option<String>,
    pub requires_human_approval: bool,
    pub requires_second_line_review: bool,
    pub segregation_of_duties_violated: bool,
    pub toctou_drifted: bool,
}

/// Mirrors `sem_os_policy::abac::AccessDecision`'s three variants without
/// depending on that crate (§9.1: no execution-tier dependency).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AccessDecisionKind {
    Allow,
    Deny,
    AllowWithMasking,
}

pub(crate) fn decide(input: &AuthorityInput) -> AuthorityOutcome {
    if input.segregation_of_duties_violated {
        return AuthorityOutcome::RejectedSegregationOfDuties;
    }
    if matches!(input.access_decision, AccessDecisionKind::Deny) {
        return AuthorityOutcome::RejectedUnauthorised;
    }
    if input.toctou_drifted {
        return AuthorityOutcome::RequiresHumanApproval;
    }
    if input.requires_second_line_review {
        return AuthorityOutcome::RequiresSecondLineReview;
    }
    if input.requires_human_approval {
        return AuthorityOutcome::RequiresHumanApproval;
    }
    AuthorityOutcome::Authorised(Authorised::new(input.actor_id.clone(), input.role.clone()))
}

/// T2.4 adapter: `Gate<crate::context::EvaluationContext>` impl for G5.
pub struct AuthorityGate;

impl Gate<crate::context::EvaluationContext> for AuthorityGate {
    fn id(&self) -> GateId {
        GateId::Authority
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.authority else {
            return GateResult::Failure("no AuthorityInput supplied".to_string());
        };
        match decide(input) {
            AuthorityOutcome::Authorised(_) => GateResult::Success,
            other => GateResult::Failure(format!("{other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorised_is_constructible_within_its_own_module() {
        let authorised = Authorised::new("actor-1", "compliance_officer");
        assert_eq!(authorised.role(), "compliance_officer");
    }

    fn base_input() -> AuthorityInput {
        AuthorityInput {
            actor_id: "actor-1".to_string(),
            role: "compliance_officer".to_string(),
            access_decision: AccessDecisionKind::Allow,
            deny_reason: None,
            requires_human_approval: false,
            requires_second_line_review: false,
            segregation_of_duties_violated: false,
            toctou_drifted: false,
        }
    }

    #[test]
    fn allow_decision_authorises() {
        assert_eq!(
            decide(&base_input()),
            AuthorityOutcome::Authorised(Authorised::new("actor-1", "compliance_officer"))
        );
    }

    #[test]
    fn deny_decision_rejects_unauthorised() {
        let input = AuthorityInput {
            access_decision: AccessDecisionKind::Deny,
            deny_reason: Some("no clearance".to_string()),
            ..base_input()
        };
        assert_eq!(decide(&input), AuthorityOutcome::RejectedUnauthorised);
    }

    #[test]
    fn sod_violation_rejects_before_anything_else() {
        let input = AuthorityInput {
            access_decision: AccessDecisionKind::Deny,
            segregation_of_duties_violated: true,
            ..base_input()
        };
        assert_eq!(decide(&input), AuthorityOutcome::RejectedSegregationOfDuties);
    }

    #[test]
    fn toctou_drift_requires_human_approval() {
        let input = AuthorityInput {
            toctou_drifted: true,
            ..base_input()
        };
        assert_eq!(decide(&input), AuthorityOutcome::RequiresHumanApproval);
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(AuthorityGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(AuthorityGate.id(), GateId::Authority);
    }
}

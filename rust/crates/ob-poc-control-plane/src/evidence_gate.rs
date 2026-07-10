//! G6 — Evidence and Obligation Check (V&S §6.6).
//!
//! T2.5 wires the adapter over SemOS governance evidence gaps
//! (`SemOsContextEnvelope.evidence_gaps`, C-007) and KYC preconditions
//! (`ob-poc-kyc-substrate::fold::control::check_control_preconditions`,
//! C-040, C-041; store-side re-check under stream lock, C-042) so
//! `ob-poc-kyc-substrate` stays the owner of precondition semantics — this
//! module only grades an already-evaluated result, it does not re-run
//! `check_control_preconditions` itself.

use crate::gate::{Gate, GateId, GateResult};

/// `EvidenceReadiness` — V&S §6.6 "Output". Variant names mirror the
/// possible outcomes listed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceOutcome {
    Sufficient(EvidenceSufficient),
    MissingRequiredEvidence,
    ExpiredEvidence,
    ConflictingEvidence,
    PendingApproval,
    ObligationOpen,
}

/// Success-form proof: required evidence and obligations are satisfied.
/// Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EvidenceSufficient {
    satisfied_obligation_ids: Vec<String>,
}

impl EvidenceSufficient {
    // Called by the (future) T2.5 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(satisfied_obligation_ids: Vec<String>) -> Self {
        Self {
            satisfied_obligation_ids,
        }
    }

    pub fn satisfied_obligation_ids(&self) -> &[String] {
        &self.satisfied_obligation_ids
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::EvidenceSufficient;

    pub(crate) fn sufficient(satisfied_obligation_ids: Vec<String>) -> EvidenceSufficient {
        EvidenceSufficient::new(satisfied_obligation_ids)
    }
}

/// Pre-computed input for the evidence gate. `evidence_gaps` mirrors
/// `SemOsContextEnvelope.evidence_gaps` (C-007); `kyc_precondition_failures`
/// mirrors the `Err` arm of `check_control_preconditions` (C-041) or the
/// store-side re-check under stream lock (C-042), stringified at the call
/// site. `satisfied_obligation_ids` lists obligations already resolved
/// (`kyc.obligation.satisfy`/`waive`) that this evaluation may cite as
/// evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceInput {
    pub evidence_gaps: Vec<String>,
    pub kyc_precondition_failures: Vec<KycPreconditionFailure>,
    pub satisfied_obligation_ids: Vec<String>,
    pub open_obligation_ids: Vec<String>,
}

/// Mirrors the precondition classes `check_control_preconditions` reports
/// (K-11 evidence-before-verify, K-14 reconcile-before-fold,
/// strategy-not-selected) without depending on `ob-poc-kyc-substrate`
/// directly (§9.1: no execution-tier dependency).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KycPreconditionFailure {
    EvidenceNotCited,
    NotReconciled,
    StrategyNotSelected,
}

fn decide(input: &EvidenceInput) -> EvidenceOutcome {
    if input
        .kyc_precondition_failures
        .iter()
        .any(|f| matches!(f, KycPreconditionFailure::NotReconciled))
    {
        return EvidenceOutcome::ConflictingEvidence;
    }
    if input
        .kyc_precondition_failures
        .iter()
        .any(|f| matches!(f, KycPreconditionFailure::EvidenceNotCited))
    {
        return EvidenceOutcome::MissingRequiredEvidence;
    }
    if !input.kyc_precondition_failures.is_empty() {
        // StrategyNotSelected and any future precondition class: treated as
        // missing evidence rather than fabricating a finer split the
        // underlying validator doesn't report.
        return EvidenceOutcome::MissingRequiredEvidence;
    }
    if !input.evidence_gaps.is_empty() {
        return EvidenceOutcome::MissingRequiredEvidence;
    }
    if !input.open_obligation_ids.is_empty() {
        return EvidenceOutcome::ObligationOpen;
    }
    EvidenceOutcome::Sufficient(EvidenceSufficient::new(input.satisfied_obligation_ids.clone()))
}

/// T2.5 adapter: `Gate<crate::context::EvaluationContext>` impl for G6.
pub struct EvidenceGate;

impl Gate<crate::context::EvaluationContext> for EvidenceGate {
    fn id(&self) -> GateId {
        GateId::Evidence
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.evidence else {
            return GateResult::Failure("no EvidenceInput supplied".to_string());
        };
        match decide(input) {
            EvidenceOutcome::Sufficient(_) => GateResult::Success,
            other => GateResult::Failure(format!("{other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_sufficient_is_constructible_within_its_own_module() {
        let evidence = EvidenceSufficient::new(vec!["obligation-1".to_string()]);
        assert_eq!(evidence.satisfied_obligation_ids(), ["obligation-1"]);
    }

    fn base_input() -> EvidenceInput {
        EvidenceInput {
            evidence_gaps: vec![],
            kyc_precondition_failures: vec![],
            satisfied_obligation_ids: vec!["obligation-1".to_string()],
            open_obligation_ids: vec![],
        }
    }

    #[test]
    fn no_gaps_no_failures_is_sufficient() {
        assert_eq!(
            decide(&base_input()),
            EvidenceOutcome::Sufficient(EvidenceSufficient::new(vec!["obligation-1".to_string()]))
        );
    }

    #[test]
    fn not_reconciled_is_conflicting_evidence() {
        let input = EvidenceInput {
            kyc_precondition_failures: vec![KycPreconditionFailure::NotReconciled],
            ..base_input()
        };
        assert_eq!(decide(&input), EvidenceOutcome::ConflictingEvidence);
    }

    #[test]
    fn evidence_not_cited_is_missing_required_evidence() {
        let input = EvidenceInput {
            kyc_precondition_failures: vec![KycPreconditionFailure::EvidenceNotCited],
            ..base_input()
        };
        assert_eq!(decide(&input), EvidenceOutcome::MissingRequiredEvidence);
    }

    #[test]
    fn open_obligation_blocks_when_no_other_gap() {
        let input = EvidenceInput {
            open_obligation_ids: vec!["obligation-2".to_string()],
            ..base_input()
        };
        assert_eq!(decide(&input), EvidenceOutcome::ObligationOpen);
    }

    #[test]
    fn semos_evidence_gap_blocks() {
        let input = EvidenceInput {
            evidence_gaps: vec!["missing certificate of incorporation".to_string()],
            ..base_input()
        };
        assert_eq!(decide(&input), EvidenceOutcome::MissingRequiredEvidence);
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(EvidenceGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(EvidenceGate.id(), GateId::Evidence);
    }
}

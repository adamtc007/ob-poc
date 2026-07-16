//! Control Plane Decision Model (V&S §9.3, §10).
//!
//! `evaluate(ctx, validity) -> ControlPlaneDecision` is the crate's
//! conceptual core API (§9.3), wired for real by T10.1 (EOP-PLAN-
//! CONTROLPLANE-001 Addendum C).
//!
//! **Why this duplicates a `decide()` call per gate instead of extending
//! `Gate<Ctx>`:** `evaluate_shadow` (`lib.rs`) already computes the
//! authoritative, dependency-aware pass/fail report via
//! `gate::evaluate_collect_where_independent` — reused here unchanged
//! (B3: no redesign of that mechanism). But `Gate<Ctx>::evaluate` returns
//! only `GateResult` (a boolean-shaped outcome), never the gate's own
//! proof-typed value (`AdmittedIntent`, `BoundEntities`, ...), by original
//! T1 design (`gate.rs`'s trait signature). Sealing an `ExecutionEnvelope`
//! needs the actual proof values. Rather than widen the `Gate` trait
//! (a redesign, forbidden by B3) or hand-roll a second dependency-blocking
//! walk (duplicating `evaluate_collect_where_independent`'s already-tested
//! logic, real bug risk), this function re-invokes each module's own
//! `decide()`/`classify()`/`build_pins()` a second time — pure, cheap,
//! deterministic functions, so calling one twice on the same input is
//! guaranteed (by construction) to reproduce the identical outcome
//! `evaluate_shadow` already proved. `report` remains the sole source of
//! truth for *whether* a gate succeeded; the second call only recovers
//! *what value* it succeeded with.

use crate::context::EvaluationContext;
use crate::envelope::{ExecutionEnvelope, ValidityWindow};
use crate::gate::{GateId, GateResult};
use crate::proof::ControlPlaneProof;

/// The three-way outcome of a control-plane evaluation (§9.3).
#[derive(Debug, Clone)]
pub enum ControlPlaneDecision {
    ApprovedStp(Box<ExecutionEnvelope>),
    RequiresHumanGate(Box<ControlPlaneProof>),
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

/// The eight gates whose proof types `ControlPlaneProof`/`ExecutionEnvelope`
/// embed (mirrors `proof::ControlPlaneProof`'s own field list and
/// `gate::GATE_DEPENDENCIES`'s `RunbookProof` predecessor edge).
const PROOF_BEARING_GATES: [GateId; 8] = [
    GateId::IntentAdmission,
    GateId::EntityBinding,
    GateId::PackResolution,
    GateId::DagProof,
    GateId::Authority,
    GateId::Evidence,
    GateId::WriteSet,
    GateId::DecisionSnapshot,
];

fn rejection_from_report(
    report: &crate::gate::EvaluationReport,
    gates: &[GateId],
) -> Option<ControlPlaneRejection> {
    let failures: Vec<GateFailure> = gates
        .iter()
        .filter_map(|id| match report.get(*id) {
            Some(GateResult::Success) => None,
            Some(GateResult::Failure(reason)) => Some(GateFailure::Failed {
                gate: *id,
                reason: reason.clone(),
            }),
            Some(GateResult::NotEvaluated { blocked_by }) => Some(GateFailure::NotEvaluated {
                gate: *id,
                blocked_by: blocked_by.clone(),
            }),
            Some(GateResult::NotImplemented) | None => Some(GateFailure::Failed {
                gate: *id,
                reason: "not implemented".to_string(),
            }),
            // G5 sweep (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 item 1):
            // this is the ONE site in the whole `NotApplicable`
            // compiler-driven sweep whose correct behaviour is genuinely
            // ambiguous, not mechanical — flagged as a disclosed
            // STOP-condition in the G5 session doc rather than guessed
            // through. `evaluate`/`evaluate_with_report` (this function's
            // only callers) serve Path A exclusively, and Path A never
            // applies a path-conditional `NotApplicable` override (see
            // `GateResult::NotApplicable`'s own doc) — so this arm is
            // reachable at compile time but not with any real Path A
            // data today. Whether a future path-aware caller of
            // `evaluate`/`evaluate_with_report` should treat
            // `NotApplicable` among `PROOF_BEARING_GATES` as vacuously
            // satisfied (like `Success`) or as a hard block is a real
            // design question this sweep does not answer: the envelope/
            // proof-assembly code immediately below `rejection_from_report`
            // unconditionally re-derives a typed proof value for every
            // proof-bearing gate, and no typed proof value exists for a
            // gate whose `Input` was never built because it was marked
            // not-applicable. Resolved here as fail-CLOSED (a defensive
            // rejection, not a silent pass) until a real path-aware caller
            // of this function exists and that design question gets its
            // own review — see G5's session doc.
            Some(GateResult::NotApplicable(reason)) => Some(GateFailure::Failed {
                gate: *id,
                reason: format!(
                    "gate marked NotApplicable ({reason}), but evaluate()/evaluate_with_report() \
                     is Path-A-only and Path A never applies a path-conditional NotApplicable \
                     override — treated as a defensive failure, not a silent pass (G5 STOP-condition)"
                ),
            }),
        })
        .collect();
    if failures.is_empty() {
        None
    } else {
        Some(ControlPlaneRejection::new(failures))
    }
}

/// T10.1: the crate's real evaluation entry point (§9.3's promised
/// `evaluate`). Runs the existing shadow report (`evaluate_shadow`,
/// unchanged) to determine pass/fail + dependency blocking, then — only
/// when every proof-bearing gate (G1, G2, G3, G4, G5, G6, G7, G13)
/// genuinely succeeded in that same report — re-derives each gate's typed
/// proof value and assembles either a sealed `ExecutionEnvelope` (STP
/// eligible) or a `ControlPlaneProof` (human-gated). Any failure among the
/// eight, or a `HumanGated`/`Rejected` STP classification with a missing
/// runbook reference, yields `Rejected` naming every contributing gate
/// (collect-where-independent, §6.16 — not just the first failure).
///
/// `validity` is supplied by the caller (this crate does no clock I/O of
/// its own, §9.1) — typically "now" plus a short shadow-sealing TTL.
///
/// Thin wrapper over [`evaluate_with_report`] — kept for callers that only
/// need the decision. Prefer `evaluate_with_report` when the caller also
/// needs the underlying [`EvaluationReport`] (e.g. to build a shadow-
/// decision audit row) — see that function's doc for why this split
/// exists (T10.2's owed-convergence item).
pub fn evaluate(ctx: &EvaluationContext, validity: ValidityWindow) -> ControlPlaneDecision {
    evaluate_with_report(ctx, validity).1
}

/// [`evaluate`], but also returns the [`crate::gate::EvaluationReport`]
/// [`crate::evaluate_shadow`] computed en route to the decision.
///
/// T10.1 landed `evaluate`/`evaluate_shadow` as two separate `pub` entry
/// points into overlapping logic — every caller wanting both a shadow-
/// decision row (built from a `report`) and a sealed-envelope-or-rejection
/// (`evaluate`'s job) had to call `evaluate_shadow` once for the row and
/// `evaluate` again for the decision, silently repeating the whole
/// dependency-aware gate walk. Flagged at T10.1's B2 ratification as owed
/// convergence, targeted at "whichever call site needs both" rather than
/// invented speculatively — `sequencer.rs`'s `phase5_runtime_recheck` is
/// that call site (T10.1's `report`/`decision` two-call pattern), closed
/// by switching it to this function.
///
/// Not a widened `evaluate` signature (which would break every existing
/// `ControlPlaneDecision`-only caller/test) — a new function, `evaluate`
/// demoted to a one-line wrapper over it. Same computation either way: no
/// behavioural change for existing `evaluate` callers.
pub fn evaluate_with_report(
    ctx: &EvaluationContext,
    validity: ValidityWindow,
) -> (crate::gate::EvaluationReport, ControlPlaneDecision) {
    let report = crate::evaluate_shadow(ctx);

    if let Some(rejection) = rejection_from_report(&report, &PROOF_BEARING_GATES) {
        return (report, ControlPlaneDecision::Rejected(rejection));
    }

    // Every proof-bearing gate succeeded in `report` — re-derive each
    // typed value. `expect`/fallback-to-Rejected here would only trigger
    // on a genuine crate-internal inconsistency (report said Success but
    // decide() disagrees), which purity guarantees cannot happen; still
    // handled as a graceful Rejected rather than a panic, since this is a
    // shadow-observation path and must never crash a caller's dispatch.
    let internal_inconsistency = |gate: GateId| {
        ControlPlaneDecision::Rejected(ControlPlaneRejection::new(vec![GateFailure::Failed {
            gate,
            reason: "internal inconsistency: report said Success but decide() disagreed"
                .to_string(),
        }]))
    };

    let Some(intent) = ctx
        .intent_admission
        .as_ref()
        .and_then(|i| match crate::intent_admission::decide(i) {
            crate::intent_admission::IntentAdmissionDecision::Admitted(a) => Some(a),
            _ => None,
        })
    else {
        return (report, internal_inconsistency(GateId::IntentAdmission));
    };

    let Some(binding) = ctx
        .entity_binding
        .as_ref()
        .and_then(|i| crate::entity_binding::decide(i).success().cloned())
    else {
        return (report, internal_inconsistency(GateId::EntityBinding));
    };

    let Some(pack) = ctx
        .pack_resolution
        .as_ref()
        .and_then(|i| match crate::pack_resolution::decide(i) {
            crate::pack_resolution::PackResolutionOutcome::Resolved(p) => Some(p),
            _ => None,
        })
    else {
        return (report, internal_inconsistency(GateId::PackResolution));
    };

    let Some(dag) = ctx
        .dag_proof
        .as_ref()
        .and_then(|i| match crate::dag_proof::decide(i) {
            crate::dag_proof::StateTransitionOutcome::Legal(l) => Some(l),
            _ => None,
        })
    else {
        return (report, internal_inconsistency(GateId::DagProof));
    };

    let Some(authority) = ctx
        .authority
        .as_ref()
        .and_then(|i| match crate::authority_gate::decide(i) {
            crate::authority_gate::AuthorityOutcome::Authorised(a) => Some(a),
            _ => None,
        })
    else {
        return (report, internal_inconsistency(GateId::Authority));
    };

    let Some(evidence) = ctx
        .evidence
        .as_ref()
        .and_then(|i| match crate::evidence_gate::decide(i) {
            crate::evidence_gate::EvidenceOutcome::Sufficient(e) => Some(e),
            _ => None,
        })
    else {
        return (report, internal_inconsistency(GateId::Evidence));
    };

    let Some(write_set) = ctx
        .write_set
        .as_ref()
        .and_then(|i| match crate::write_set::decide(i) {
            crate::write_set::WriteSetOutcome::Bounded(w) => Some(w),
            _ => None,
        })
    else {
        return (report, internal_inconsistency(GateId::WriteSet));
    };

    let Some(snapshot) = ctx.snapshot.as_ref().map(crate::snapshot::build_pins) else {
        return (report, internal_inconsistency(GateId::DecisionSnapshot));
    };

    // G9 (RunbookProof) is not in PROOF_BEARING_GATES's own upstream set
    // but is itself proof-bearing for the artefact — check it explicitly
    // rather than assume: `report` already ran it (its GATE_DEPENDENCIES
    // predecessors are exactly PROOF_BEARING_GATES, all just confirmed
    // Success above), so a Failure here means only the runbook-id fact
    // itself was absent, not a predecessor gap.
    let runbook = match report.get(GateId::RunbookProof) {
        Some(GateResult::Success) => ctx
            .runbook_proof
            .as_ref()
            .and_then(crate::proof::decide),
        _ => None,
    };

    let Some(runbook) = runbook else {
        return (
            report,
            ControlPlaneDecision::Rejected(ControlPlaneRejection::new(vec![GateFailure::Failed {
                gate: GateId::RunbookProof,
                reason: "no compiled runbook reference available".to_string(),
            }])),
        );
    };

    let proof = ControlPlaneProof::new(
        intent,
        binding,
        pack,
        dag,
        authority,
        evidence,
        write_set,
        runbook,
        snapshot,
    );

    // G8 (STP classifier) determines StpExecutable vs HumanGated vs
    // Rejected — the one gate whose 3-way distinction `GateResult`'s
    // binary shape can't carry (see stp_classifier.rs's own doc), so this
    // is the one place `evaluate` calls `classify` directly rather than
    // reading `report`.
    let stp = ctx.stp_classifier.as_ref().map(crate::stp_classifier::classify);
    let decision = match stp {
        Some(crate::stp_classifier::StpEligibilityDecision::StpExecutable) => {
            let ControlPlaneProof {
                intent,
                binding,
                pack,
                dag,
                authority,
                evidence,
                write_set,
                runbook,
                snapshot,
            } = proof;
            let envelope = ExecutionEnvelope::seal(
                intent, binding, pack, dag, authority, evidence, write_set, runbook, snapshot,
                validity,
            );
            ControlPlaneDecision::ApprovedStp(Box::new(envelope))
        }
        Some(crate::stp_classifier::StpEligibilityDecision::HumanGated) => {
            ControlPlaneDecision::RequiresHumanGate(Box::new(proof))
        }
        Some(crate::stp_classifier::StpEligibilityDecision::Rejected) | None => {
            ControlPlaneDecision::Rejected(ControlPlaneRejection::new(vec![GateFailure::Failed {
                gate: GateId::StpClassifier,
                reason: "rejected or no StpClassifierInput supplied".to_string(),
            }]))
        }
    };
    (report, decision)
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

    // ── T10.1: evaluate() ──

    use crate::authority_gate::{AccessDecisionKind, AuthorityInput};
    use crate::dag_proof::DagProofInput;
    use crate::entity_binding::{EntityBindingInput, EntityFacts};
    use crate::evidence_gate::EvidenceInput;
    use crate::intent_admission::IntentAdmissionInput;
    use crate::pack_resolution::PackResolutionInput;
    use crate::proof::RunbookProofInput;
    use crate::snapshot::SnapshotInput;
    use crate::stp_classifier::StpClassifierInput;
    use crate::write_set::WriteSetInput;
    use uuid::Uuid;

    fn now_window() -> ValidityWindow {
        let now = chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap();
        ValidityWindow::new(now, now + chrono::Duration::minutes(5))
    }

    /// Every input legal and STP-clean — mirrors `lib.rs`'s
    /// `fully_admitted_context` (module-private there, so rebuilt here
    /// rather than restructuring that fixture's visibility for one caller).
    fn sealable_context() -> EvaluationContext {
        let entity = Uuid::nil();
        EvaluationContext {
            intent_admission: Some(IntentAdmissionInput {
                intent_id: Uuid::nil(),
                verb_fqn: "cbu.confirm".to_string(),
                is_admitted: true,
                exclusion_reasons: vec![],
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            entity_binding: Some(EntityBindingInput {
                entities: vec![EntityFacts {
                    entity_id: entity,
                    exists: true,
                    expected_kind: "cbu".to_string(),
                    actual_kind: "cbu".to_string(),
                    lifecycle_state_readable: true,
                    availability_blocked: false,
                    availability_reason: None,
                    in_active_pack: true,
                }],
            }),
            pack_resolution: Some(PackResolutionInput {
                candidate_pack_ids: vec!["ob-poc.cbu".to_string()],
                semreg_allowed_set_available: true,
                constraint_denies_intent: false,
            }),
            dag_proof: Some(DagProofInput {
                entity_id: entity,
                from_state: "VALIDATION_PENDING".to_string(),
                to_state: "VALIDATED".to_string(),
                blocking_violations: vec![],
                lifecycle_fail_open_class: None,
                lifecycle_gate_mode_fail_closed: false,
            }),
            authority: Some(AuthorityInput {
                actor_id: "actor-1".to_string(),
                role: "compliance_officer".to_string(),
                access_decision: AccessDecisionKind::Allow,
                deny_reason: None,
                requires_human_approval: false,
                requires_second_line_review: false,
                segregation_of_duties_violated: false,
                toctou_drifted: false,
            }),
            evidence: Some(EvidenceInput {
                evidence_gaps: vec![],
                kyc_precondition_failures: vec![],
                satisfied_obligation_ids: vec!["obligation-1".to_string()],
                open_obligation_ids: vec![],
            }),
            write_set: Some(WriteSetInput {
                entity_ids: vec![entity],
                state_slots: vec!["validation_state".to_string()],
                tables: vec!["ob-poc.cbus".to_string()],
                allowed_columns: vec!["status".to_string()],
                idempotency_key: "idem-1".to_string(),
                contract_derived: true,
            }),
            snapshot: Some(SnapshotInput {
                sem_reg_snapshot_id: Some(Uuid::nil()),
                session_snapshot_id: None,
                kyc_manifest_hash: None,
                entity_row_versions: vec![(entity, "cbu".to_string(), 1)],
                versions: crate::snapshot::PinnedVersionSet::default(),
            }),
            stp_classifier: Some(StpClassifierInput {
                is_durable_verb: false,
                durable_execution_explicitly_allowed: false,
                has_unpinned_entities: false,
            }),
            runbook_proof: Some(RunbookProofInput {
                compiled_runbook_id: Some(Uuid::nil()),
            }),
            version_pinning: Some(crate::versioning::VersionPinningInput {
                versions: crate::snapshot::PinnedVersionSet::default(),
            }),
            write_set_attestation: None,
        }
    }

    #[test]
    fn evaluate_seals_a_real_envelope_when_everything_is_stp_clean() {
        let decision = evaluate(&sealable_context(), now_window());
        let ControlPlaneDecision::ApprovedStp(envelope) = decision else {
            panic!("expected ApprovedStp, got {decision:?}");
        };
        assert_eq!(envelope.intent().verb_fqn(), "cbu.confirm");
        assert_eq!(envelope.binding().entity_ids(), &[Uuid::nil()]);
        assert_eq!(envelope.runbook().runbook_id(), Uuid::nil());
    }

    /// T10.2 owed-convergence closure: `evaluate_with_report`'s `report`
    /// must be identical to a standalone `evaluate_shadow` call (proving
    /// the convergence didn't change what the report says), and its
    /// `decision` must be identical to `evaluate`'s own result (proving
    /// `evaluate`'s demotion to a one-line wrapper changed nothing
    /// observable for existing callers).
    #[test]
    fn evaluate_with_report_matches_the_separate_evaluate_shadow_and_evaluate_calls() {
        let ctx = sealable_context();
        let window = now_window();

        let standalone_report = crate::evaluate_shadow(&ctx);
        let standalone_decision = evaluate(&ctx, window);
        let (report, decision) = evaluate_with_report(&ctx, window);

        assert_eq!(report, standalone_report);
        match (&decision, &standalone_decision) {
            (ControlPlaneDecision::ApprovedStp(a), ControlPlaneDecision::ApprovedStp(b)) => {
                assert_eq!(a.intent().verb_fqn(), b.intent().verb_fqn());
                assert_eq!(a.binding().entity_ids(), b.binding().entity_ids());
            }
            other => panic!("expected both to seal ApprovedStp, got {other:?}"),
        }
    }

    #[test]
    fn evaluate_requires_human_gate_when_stp_classifier_says_human_gated() {
        let mut ctx = sealable_context();
        ctx.stp_classifier = Some(StpClassifierInput {
            is_durable_verb: false,
            durable_execution_explicitly_allowed: false,
            has_unpinned_entities: true,
        });
        let decision = evaluate(&ctx, now_window());
        let ControlPlaneDecision::RequiresHumanGate(proof) = decision else {
            panic!("expected RequiresHumanGate, got {decision:?}");
        };
        assert_eq!(proof.intent.verb_fqn(), "cbu.confirm");
    }

    #[test]
    fn evaluate_rejects_naming_every_failed_gate_not_just_the_first() {
        let mut ctx = sealable_context();
        ctx.intent_admission = None;
        ctx.write_set = None;
        let decision = evaluate(&ctx, now_window());
        let ControlPlaneDecision::Rejected(rejection) = decision else {
            panic!("expected Rejected, got {decision:?}");
        };
        // IntentAdmission fails directly; EntityBinding/PackResolution/
        // Authority/Evidence/DagProof/DecisionSnapshot all declare it (or
        // a downstream of it) as a predecessor and are NotEvaluated;
        // WriteSet fails directly (its own input is None) rather than
        // NotEvaluated, since its only declared predecessor is DagProof,
        // which itself is blocked (not failed) by the missing intent.
        assert!(rejection.failures().len() >= 2);
        assert!(rejection
            .failures()
            .iter()
            .any(|f| matches!(f, GateFailure::Failed { gate: GateId::IntentAdmission, .. })));
    }

    #[test]
    fn evaluate_rejects_when_no_compiled_runbook_ref_even_if_stp_clean() {
        let mut ctx = sealable_context();
        ctx.runbook_proof = Some(RunbookProofInput {
            compiled_runbook_id: None,
        });
        let decision = evaluate(&ctx, now_window());
        let ControlPlaneDecision::Rejected(rejection) = decision else {
            panic!("expected Rejected, got {decision:?}");
        };
        assert!(rejection
            .failures()
            .iter()
            .any(|f| matches!(f, GateFailure::Failed { gate: GateId::RunbookProof, .. })));
    }

    #[test]
    fn evaluate_rejects_when_stp_classifier_input_missing() {
        let mut ctx = sealable_context();
        ctx.stp_classifier = None;
        let decision = evaluate(&ctx, now_window());
        assert!(matches!(decision, ControlPlaneDecision::Rejected(_)));
    }
}

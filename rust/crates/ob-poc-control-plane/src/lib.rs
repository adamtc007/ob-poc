//! ob-poc-control-plane — owns the execution-control decision for AI-led
//! governed execution (EOP-VS-CONTROLPLANE-001 / EOP-PLAN-CONTROLPLANE-001).
//!
//! T1 landed the crate skeleton: proof-carrying types, the
//! `ControlPlaneDecision` model, the `ExecutionEnvelope` sealed constructor,
//! and a declared-dependency-graph evaluator with all 14 gates stubbed. T2
//! wired six real gate adapters (G1, G3, G4, G5, G6, G7) over existing
//! validators — see `docs/research/control-plane-ownership-ledger.md` for
//! the C-0xx disposition driving each adapter. T3 wires the three gates
//! with no production analogue at all (RR-8): G2 (entity binding), G13
//! (decision snapshot pins), and G8 (STP classifier) — plus assembles the
//! T3.4 `ControlPlaneProof` aggregate. Only the downstream artefact gates
//! (G9-G12, G14) remain stubbed, pending T4/T5.
//!
//! This crate must not depend on any execution-tier crate (§9.1 non-goals):
//! it does not own LLM prompting, DSL parsing, DAG authoring, SemOS
//! authoring, runtime state mutation, or re-implementations of validator
//! logic owned elsewhere. It owns only the decision. Gate adapters accept
//! plain, primitive-typed `*Input` structs (see `context::EvaluationContext`)
//! that callers translate from their own domain types — this keeps the
//! dependency edge one-directional (`ob-poc` depends on this crate, never
//! the reverse) even though the validators being wrapped live in `ob-poc`,
//! `dsl-runtime`, `ob-poc-boundary`, and `ob-poc-kyc-substrate`.
#![deny(unreachable_pub)]

pub mod authority_gate;
pub mod context;
pub mod dag_proof;
pub mod entity_binding;
pub mod evidence_gate;
pub mod gate;
pub mod intent_admission;
pub mod pack_resolution;
pub mod snapshot;
pub mod stp_classifier;
pub mod write_set;
pub mod write_set_attestation;

pub mod audit;
pub mod exceptions;
pub mod metrics;
pub mod versioning;

pub mod decision;
pub mod envelope;
pub mod proof;

use std::collections::BTreeMap;

use context::EvaluationContext;
use gate::{evaluate_collect_where_independent, EvaluationReport, Gate, GateId, UnimplementedGate};

/// Runs every wired gate adapter (G1-G8, G13) plus stubs for every gate
/// not yet implemented (G9-G12, G14), through the collect-where-independent
/// evaluator (T1.3). This is the crate's shadow entry point (T2.7): callers
/// persist the returned report and compare it against a legacy outcome —
/// it never gates dispatch itself.
///
/// Determinism (§12.8, §12.11 — T3 exit criterion): every gate here is a
/// pure function of `ctx` (no I/O, no clock/random reads), and
/// `evaluate_collect_where_independent`'s dependency walk is a fixed
/// `BTreeMap`/`Vec` traversal with no iteration-order sensitivity — so
/// `evaluate_shadow(&ctx)` called any number of times on an unchanged `ctx`
/// is guaranteed to return an identical `EvaluationReport` (see
/// `tests::same_context_reevaluates_identically_one_thousand_times` and
/// `tests::serialized_context_replays_to_the_identical_report`).
pub fn evaluate_shadow(ctx: &EvaluationContext) -> EvaluationReport {
    let intent_admission_gate = intent_admission::IntentAdmissionGate;
    let entity_binding_gate = entity_binding::EntityBindingGate;
    let pack_resolution_gate = pack_resolution::PackResolutionGate;
    let dag_proof_gate = dag_proof::DagProofGate;
    let authority_gate = authority_gate::AuthorityGate;
    let evidence_gate = evidence_gate::EvidenceGate;
    let write_set_gate = write_set::WriteSetGate;
    let stp_classifier_gate = stp_classifier::StpClassifierGate;
    let decision_snapshot_gate = snapshot::DecisionSnapshotGate;
    let write_set_attestation_gate = write_set_attestation::WriteSetAttestationGate;

    let stub_ids = [
        GateId::RunbookProof,
        GateId::ExecutionEnvelope,
        GateId::AuditReplay,
        GateId::VersionPinning,
    ];
    let stubs: Vec<UnimplementedGate> = stub_ids.iter().map(|id| UnimplementedGate(*id)).collect();

    let mut gates: BTreeMap<GateId, &dyn Gate<EvaluationContext>> = BTreeMap::new();
    gates.insert(GateId::IntentAdmission, &intent_admission_gate);
    gates.insert(GateId::EntityBinding, &entity_binding_gate);
    gates.insert(GateId::PackResolution, &pack_resolution_gate);
    gates.insert(GateId::DagProof, &dag_proof_gate);
    gates.insert(GateId::Authority, &authority_gate);
    gates.insert(GateId::Evidence, &evidence_gate);
    gates.insert(GateId::WriteSet, &write_set_gate);
    gates.insert(GateId::StpClassifier, &stp_classifier_gate);
    gates.insert(GateId::DecisionSnapshot, &decision_snapshot_gate);
    gates.insert(GateId::WriteSetAttestation, &write_set_attestation_gate);
    for stub in &stubs {
        gates.insert(stub.0, stub);
    }

    evaluate_collect_where_independent(&gates, ctx)
}

#[cfg(test)]
mod evaluate_shadow_tests {
    use super::*;
    use authority_gate::{AccessDecisionKind, AuthorityInput};
    use dag_proof::DagProofInput;
    use entity_binding::{EntityBindingInput, EntityFacts};
    use evidence_gate::EvidenceInput;
    use intent_admission::IntentAdmissionInput;
    use pack_resolution::PackResolutionInput;
    use snapshot::SnapshotInput;
    use stp_classifier::StpClassifierInput;
    use uuid::Uuid;
    use write_set::WriteSetInput;

    fn admitted_intent(verb_fqn: &str) -> IntentAdmissionInput {
        IntentAdmissionInput {
            intent_id: Uuid::nil(),
            verb_fqn: verb_fqn.to_string(),
            is_admitted: true,
            exclusion_reasons: vec![],
            is_ai_originated: false,
            interpretation_attested: false,
        }
    }

    /// Exit criterion: "shadow decisions visible for Path A end-to-end" —
    /// proves the composed evaluator actually calls the real T2.1 adapter
    /// (not a stub) and reports G1 success faithfully, while G3
    /// (PackResolution, which declares EntityBinding as a dependency)
    /// correctly reports `NotEvaluated` when EntityBinding's own input is
    /// missing and fails — collect-where-independent, not silently skipped.
    #[test]
    fn admitted_intent_evaluates_g1_and_blocks_downstream_on_missing_entity_binding_input() {
        let ctx = EvaluationContext {
            intent_admission: Some(admitted_intent("cbu.confirm")),
            ..EvaluationContext::default()
        };

        let report = evaluate_shadow(&ctx);

        assert_eq!(report.get(GateId::IntentAdmission), Some(&gate::GateResult::Success));
        assert!(matches!(
            report.get(GateId::EntityBinding),
            Some(&gate::GateResult::Failure(_))
        ));
        assert!(matches!(
            report.get(GateId::PackResolution),
            Some(&gate::GateResult::NotEvaluated { .. })
        ));
    }

    #[test]
    fn rejected_intent_reports_g1_failure() {
        let ctx = EvaluationContext {
            intent_admission: Some(IntentAdmissionInput {
                is_admitted: false,
                exclusion_reasons: vec!["AbacDenied".to_string()],
                ..admitted_intent("cbu.confirm")
            }),
            ..EvaluationContext::default()
        };

        let report = evaluate_shadow(&ctx);
        assert!(matches!(
            report.get(GateId::IntentAdmission),
            Some(&gate::GateResult::Failure(_))
        ));
    }

    /// A fully-populated context in which every gate through G8 succeeds —
    /// the T3 "chain now completes" proof (T2's tests could only ever show
    /// G1 succeeding in isolation, since G2 didn't exist).
    fn fully_admitted_context() -> EvaluationContext {
        let entity = Uuid::nil();
        EvaluationContext {
            intent_admission: Some(admitted_intent("cbu.confirm")),
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
                versions: snapshot::PinnedVersionSet::default(),
            }),
            stp_classifier: Some(StpClassifierInput {
                is_durable_verb: false,
                durable_execution_explicitly_allowed: false,
                has_unpinned_entities: false,
            }),
            write_set_attestation: Some(write_set_attestation::WriteSetAttestationInput {
                captured: vec![write_set_attestation::CapturedWrite {
                    table: "ob-poc.cbus".to_string(),
                    entity_id: entity,
                    columns: vec!["status".to_string()],
                }],
                expected_tables: vec!["ob-poc.cbus".to_string()],
                expected_entity_ids: vec![entity],
                expected_allowed_columns: vec!["status".to_string()],
            }),
        }
    }

    #[test]
    fn fully_admitted_context_succeeds_through_every_wired_gate() {
        let ctx = fully_admitted_context();
        let report = evaluate_shadow(&ctx);
        for id in [
            GateId::IntentAdmission,
            GateId::EntityBinding,
            GateId::PackResolution,
            GateId::DagProof,
            GateId::Authority,
            GateId::Evidence,
            GateId::WriteSet,
            GateId::StpClassifier,
            GateId::DecisionSnapshot,
            GateId::WriteSetAttestation,
        ] {
            assert_eq!(report.get(id), Some(&gate::GateResult::Success), "{id:?} did not succeed");
        }
    }

    /// Exit criterion: "same (intent, ctx, pins) -> identical decision
    /// across 1,000 randomized re-evaluations." Every gate in this crate is
    /// a pure function of `ctx` — re-running `evaluate_shadow` on the same
    /// (unmutated) context 1,000 times must yield byte-identical reports,
    /// for both a fully-succeeding context and a partially-blocked one.
    #[test]
    fn same_context_reevaluates_identically_one_thousand_times() {
        for ctx in [fully_admitted_context(), EvaluationContext::default()] {
            let first = evaluate_shadow(&ctx);
            for _ in 0..1000 {
                assert_eq!(evaluate_shadow(&ctx), first);
            }
        }
    }

    /// Exit criterion: "replay test — persisted proof re-evaluated against
    /// pinned snapshot reproduces decision." `EvaluationContext` derives
    /// `Serialize`/`Deserialize` precisely so a context can be persisted
    /// (the wire/storage boundary a real persisted-proof replay would cross)
    /// and, once reloaded, re-evaluates to the identical report.
    #[test]
    fn serialized_context_replays_to_the_identical_report() {
        let ctx = fully_admitted_context();
        let original_report = evaluate_shadow(&ctx);

        let persisted = serde_json::to_string(&ctx).expect("context serializes");
        let reloaded: EvaluationContext = serde_json::from_str(&persisted).expect("context deserializes");
        assert_eq!(reloaded, ctx, "round-trip must be lossless");

        let replayed_report = evaluate_shadow(&reloaded);
        assert_eq!(replayed_report, original_report, "replay must reproduce the original decision");
    }
}

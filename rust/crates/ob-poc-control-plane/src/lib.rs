//! ob-poc-control-plane — owns the execution-control decision for AI-led
//! governed execution (EOP-VS-CONTROLPLANE-001 / EOP-PLAN-CONTROLPLANE-001).
//!
//! T1 landed the crate skeleton: proof-carrying types, the
//! `ControlPlaneDecision` model, the `ExecutionEnvelope` sealed constructor,
//! and a declared-dependency-graph evaluator with all 14 gates stubbed. T2
//! wires six real gate adapters (G1, G3, G4, G5, G6, G7) over existing
//! validators — see `docs/research/control-plane-ownership-ledger.md` for
//! the C-0xx disposition driving each adapter. `G2` (entity binding), `G8`
//! (STP classifier), and the downstream artefact gates remain stubbed until
//! T3+.
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

/// Runs every T2-wired gate adapter (G1, G3, G4, G5, G6, G7) plus stubs for
/// every gate not yet implemented (G2, G8-G14), through the
/// collect-where-independent evaluator (T1.3). This is the crate's shadow
/// entry point (T2.7): callers persist the returned report and compare it
/// against a legacy outcome — it never gates dispatch itself.
pub fn evaluate_shadow(ctx: &EvaluationContext) -> EvaluationReport {
    let intent_admission_gate = intent_admission::IntentAdmissionGate;
    let pack_resolution_gate = pack_resolution::PackResolutionGate;
    let dag_proof_gate = dag_proof::DagProofGate;
    let authority_gate = authority_gate::AuthorityGate;
    let evidence_gate = evidence_gate::EvidenceGate;
    let write_set_gate = write_set::WriteSetGate;

    let stub_ids = [
        GateId::EntityBinding,
        GateId::StpClassifier,
        GateId::RunbookProof,
        GateId::ExecutionEnvelope,
        GateId::AuditReplay,
        GateId::VersionPinning,
        GateId::DecisionSnapshot,
        GateId::WriteSetAttestation,
    ];
    let stubs: Vec<UnimplementedGate> = stub_ids.iter().map(|id| UnimplementedGate(*id)).collect();

    let mut gates: BTreeMap<GateId, &dyn Gate<EvaluationContext>> = BTreeMap::new();
    gates.insert(GateId::IntentAdmission, &intent_admission_gate);
    gates.insert(GateId::PackResolution, &pack_resolution_gate);
    gates.insert(GateId::DagProof, &dag_proof_gate);
    gates.insert(GateId::Authority, &authority_gate);
    gates.insert(GateId::Evidence, &evidence_gate);
    gates.insert(GateId::WriteSet, &write_set_gate);
    for stub in &stubs {
        gates.insert(stub.0, stub);
    }

    evaluate_collect_where_independent(&gates, ctx)
}

#[cfg(test)]
mod evaluate_shadow_tests {
    use super::*;
    use intent_admission::IntentAdmissionInput;
    use uuid::Uuid;

    /// Exit criterion: "shadow decisions visible for Path A end-to-end" —
    /// proves the composed evaluator actually calls the real T2.1 adapter
    /// (not a stub) and reports G1 success/failure faithfully, while every
    /// gate downstream of the not-yet-implemented G2 (EntityBinding)
    /// correctly reports `NotEvaluated` rather than being silently skipped.
    #[test]
    fn admitted_intent_evaluates_g1_and_blocks_downstream_on_missing_entity_binding() {
        let ctx = EvaluationContext {
            intent_admission: Some(IntentAdmissionInput {
                intent_id: Uuid::nil(),
                verb_fqn: "cbu.confirm".to_string(),
                is_admitted: true,
                exclusion_reasons: vec![],
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            ..EvaluationContext::default()
        };

        let report = evaluate_shadow(&ctx);

        assert_eq!(report.get(GateId::IntentAdmission), Some(&gate::GateResult::Success));
        assert!(matches!(
            report.get(GateId::PackResolution),
            Some(&gate::GateResult::NotEvaluated { .. })
        ));
        assert_eq!(report.get(GateId::EntityBinding), Some(&gate::GateResult::NotImplemented));
    }

    #[test]
    fn rejected_intent_reports_g1_failure() {
        let ctx = EvaluationContext {
            intent_admission: Some(IntentAdmissionInput {
                intent_id: Uuid::nil(),
                verb_fqn: "cbu.confirm".to_string(),
                is_admitted: false,
                exclusion_reasons: vec!["AbacDenied".to_string()],
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            ..EvaluationContext::default()
        };

        let report = evaluate_shadow(&ctx);
        assert!(matches!(
            report.get(GateId::IntentAdmission),
            Some(&gate::GateResult::Failure(_))
        ));
    }
}

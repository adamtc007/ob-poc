//! G9 — Runbook Proof Generation (V&S §6.9) and the `CompiledRunbookRef`
//! placeholder consumed by `write_set` (T2.6) and `envelope::seal` (T1.2).
//!
//! No production analogue exists today for `ControlPlaneProof` itself
//! (Phase 0 inventory). T3.4 assembles the real aggregate — every gate
//! proof plus the decision snapshot pins, mirroring exactly what
//! `ExecutionEnvelope::seal` (T1.2) takes, because `ControlPlaneProof` is
//! what backs `ControlPlaneDecision::RequiresHumanGate` (`decision.rs`):
//! reaching that variant means every gate through G7 succeeded and only
//! the STP classifier (G8) capped the plan at human-gated (durable verb, or
//! plan A5's unpinned-entity rule) — the reviewer needs to see the *same*
//! full proof set an `ExecutionEnvelope` would have carried, not a lesser
//! one, to make an informed approve/reject call.
//!
//! Persistence beside `sem_reg.decision_records` (reuse `snapshot_manifest`
//! pattern, ledger C-044/RR-4) is a production call-site concern, not yet
//! wired this tranche — see the ownership ledger.

use uuid::Uuid;

use crate::authority_gate::Authorised;
use crate::dag_proof::LegalTransition;
use crate::entity_binding::BoundEntities;
use crate::evidence_gate::EvidenceSufficient;
use crate::intent_admission::AdmittedIntent;
use crate::pack_resolution::ResolvedPack;
use crate::snapshot::SnapshotPins;
use crate::write_set::WriteSetProof;

/// A reference to a compiled runbook, opaque to this crate. `ob-poc`'s
/// REPL/compiler owns runbook compilation (§8.5); the control plane only
/// ever holds a reference to the compiled artefact, never re-implements
/// compilation. The concrete linkage (compiled runbook id/hash) is wired
/// when T2.6/T3.4 land — this placeholder exists so `envelope::seal`'s
/// signature can be written now, matching V&S §9.4 exactly.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompiledRunbookRef {
    runbook_id: Uuid,
}

impl CompiledRunbookRef {
    pub fn new(runbook_id: Uuid) -> Self {
        Self { runbook_id }
    }

    pub fn runbook_id(&self) -> Uuid {
        self.runbook_id
    }
}

/// `ControlPlaneProof` — V&S §6.9 "Output". The pre-execution artefact
/// that allows the platform, operator, reviewer or auditor to understand
/// exactly what will happen: every gate's success proof (G1-G7) plus the
/// decision snapshot pins (G13) and the compiled runbook reference (G9
/// itself). Deliberately public fields and a plain public constructor
/// (unlike `ExecutionEnvelope::seal`, which is `pub(crate)`-gated): this is
/// an aggregation object assembled by the (future) orchestration entry
/// point from already-obtained proofs, not itself a tollgate a caller could
/// bypass — the tollgate is each individual proof type's own
/// module-private constructor.
#[derive(Debug, Clone)]
pub struct ControlPlaneProof {
    pub intent: AdmittedIntent,
    pub binding: BoundEntities,
    pub pack: ResolvedPack,
    pub dag: LegalTransition,
    pub authority: Authorised,
    pub evidence: EvidenceSufficient,
    pub write_set: WriteSetProof,
    pub runbook: CompiledRunbookRef,
    pub snapshot: SnapshotPins,
}

impl ControlPlaneProof {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        intent: AdmittedIntent,
        binding: BoundEntities,
        pack: ResolvedPack,
        dag: LegalTransition,
        authority: Authorised,
        evidence: EvidenceSufficient,
        write_set: WriteSetProof,
        runbook: CompiledRunbookRef,
        snapshot: SnapshotPins,
    ) -> Self {
        Self {
            intent,
            binding,
            pack,
            dag,
            authority,
            evidence,
            write_set,
            runbook,
            snapshot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_plane_proof_carries_every_gate_proof_through_g7_plus_pins() {
        let proof = ControlPlaneProof::new(
            crate::intent_admission::tests_support::admitted(Uuid::nil(), "cbu.confirm"),
            crate::entity_binding::tests_support::bound(vec![Uuid::nil()]),
            crate::pack_resolution::tests_support::resolved("ob-poc.cbu"),
            crate::dag_proof::tests_support::legal(Uuid::nil(), "VALIDATION_PENDING", "VALIDATED"),
            crate::authority_gate::tests_support::authorised("actor-1", "compliance_officer"),
            crate::evidence_gate::tests_support::sufficient(vec!["obligation-1".into()]),
            crate::write_set::tests_support::proof(
                vec![Uuid::nil()],
                vec!["validation_state".into()],
                vec!["ob-poc.cbus".into()],
                vec!["status".into()],
                "idem-1",
            ),
            CompiledRunbookRef::new(Uuid::nil()),
            crate::snapshot::tests_support::pins(Some(Uuid::nil()), None, None, vec![]),
        );

        assert_eq!(proof.intent.verb_fqn(), "cbu.confirm");
        assert_eq!(proof.runbook.runbook_id(), Uuid::nil());
    }
}

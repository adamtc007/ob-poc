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
use crate::gate::{Gate, GateId, GateResult};
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

/// T9.7 (widened T10.1): pre-computed input for the G9 shadow gate. Carries
/// the actual `compiled_runbook_id` — not just a presence flag — because
/// T10.1's sealing path needs a real `CompiledRunbookRef` to construct, not
/// merely a yes/no signal. `try_compile_entry` populates
/// `entry.compiled_runbook_id` before the execution loop reaches the shadow
/// call site (INV-3: "raw DSL execution without a CompiledRunbookId is
/// never permitted"), so this is a real, already production-enforced fact
/// — not a placeholder awaiting future infrastructure, unlike G10/G11.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct RunbookProofInput {
    pub compiled_runbook_id: Option<Uuid>,
}

/// T9.7 adapter: `Gate<crate::context::EvaluationContext>` impl for G9.
///
/// This gate's declared predecessors (`gate::GATE_DEPENDENCIES`) are the
/// eight gates whose proof types are literally the fields of
/// `ControlPlaneProof` above — `evaluate_collect_where_independent` only
/// calls `evaluate` here once all eight have genuinely succeeded, so a
/// `Success` here really does mean "every proof this artefact would embed
/// is available, and the runbook reference is real," not merely "the
/// runbook reference is real in isolation."
pub struct RunbookProofGate;

impl Gate<crate::context::EvaluationContext> for RunbookProofGate {
    fn id(&self) -> GateId {
        GateId::RunbookProof
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        match &ctx.runbook_proof {
            Some(input) if input.compiled_runbook_id.is_some() => GateResult::Success,
            Some(_) => {
                GateResult::Failure("entry has no compiled runbook reference".to_string())
            }
            None => GateResult::Failure("no RunbookProofInput supplied".to_string()),
        }
    }
}

/// T10.1: grades a `RunbookProofInput` into the real `CompiledRunbookRef`
/// proof, mirroring every other gate module's `decide()` shape (a
/// `pub(crate)` pure function decision::evaluate calls to obtain the typed
/// value once `evaluate_shadow` has already proven this gate `Success`).
pub(crate) fn decide(input: &RunbookProofInput) -> Option<CompiledRunbookRef> {
    input.compiled_runbook_id.map(CompiledRunbookRef::new)
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

    // ── T9.7: RunbookProofGate ──

    #[test]
    fn runbook_proof_gate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(
            RunbookProofGate.evaluate(&ctx),
            GateResult::Failure(_)
        ));
        assert_eq!(RunbookProofGate.id(), GateId::RunbookProof);
    }

    #[test]
    fn runbook_proof_gate_fails_when_no_compiled_runbook_ref() {
        let ctx = crate::context::EvaluationContext {
            runbook_proof: Some(RunbookProofInput {
                compiled_runbook_id: None,
            }),
            ..Default::default()
        };
        assert!(matches!(
            RunbookProofGate.evaluate(&ctx),
            GateResult::Failure(_)
        ));
    }

    #[test]
    fn runbook_proof_gate_succeeds_when_compiled_runbook_ref_present() {
        let ctx = crate::context::EvaluationContext {
            runbook_proof: Some(RunbookProofInput {
                compiled_runbook_id: Some(Uuid::nil()),
            }),
            ..Default::default()
        };
        assert_eq!(RunbookProofGate.evaluate(&ctx), GateResult::Success);
    }

    // ── T10.1: decide() ──

    #[test]
    fn decide_none_when_no_compiled_runbook_id() {
        assert!(decide(&RunbookProofInput {
            compiled_runbook_id: None
        })
        .is_none());
    }

    #[test]
    fn decide_some_ref_when_compiled_runbook_id_present() {
        let id = Uuid::new_v4();
        let runbook_ref = decide(&RunbookProofInput {
            compiled_runbook_id: Some(id),
        })
        .expect("id supplied");
        assert_eq!(runbook_ref.runbook_id(), id);
    }
}

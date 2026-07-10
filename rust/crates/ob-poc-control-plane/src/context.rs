//! T2/T3 production input context: the single `Ctx` type shared by every
//! `Gate<Ctx>` adapter wired so far (`gate::Gate<Ctx>` is monomorphic per
//! evaluation pass — one `Ctx` type covers every registered gate).
//!
//! Each field is a plain, primitive-typed translation of an existing
//! validator's *already-computed* output (see the ownership ledger for the
//! C-0xx row each input type traces back to), or — for `entity_binding`,
//! `snapshot`, and `stp_classifier` (T3.1/T3.2/T3.3, RR-8: "no full
//! production analogue") — a plain fact set the call site collects for a
//! gate that has no existing validator to wrap. This crate must not depend
//! on `ob-poc`/`dsl-runtime`/`sem_os_policy` domain types directly (§9.1
//! non-goals — no execution-tier dependency), so callers translate at the
//! call site; adapters here only grade the translated value, they never
//! recompute it.
//!
//! Every field derives `Serialize`/`Deserialize` so a full `EvaluationContext`
//! can round-trip through the wire/storage boundary — this is what makes the
//! T3 replay-determinism property test possible (§12.11: a persisted
//! decision must be reproducible by re-evaluating against the pinned
//! context).
//!
//! A `None` field means "this gate's input was not supplied for this
//! evaluation" — the corresponding gate adapter treats that as a hard
//! failure (fail-closed), never as a pass.

use crate::authority_gate::AuthorityInput;
use crate::dag_proof::DagProofInput;
use crate::entity_binding::EntityBindingInput;
use crate::evidence_gate::EvidenceInput;
use crate::intent_admission::IntentAdmissionInput;
use crate::pack_resolution::PackResolutionInput;
use crate::snapshot::SnapshotInput;
use crate::stp_classifier::StpClassifierInput;
use crate::write_set::WriteSetInput;
use crate::write_set_attestation::WriteSetAttestationInput;

/// Shared evaluation context for every gate adapter wired so far (G1-G8,
/// G13, G14). The remaining gates (G9-G12) are downstream/infrastructural
/// artefacts (runbook proof, execution envelope, audit/replay, version
/// pinning) that consume this context's *outputs* rather than being graded
/// from within it — they stay `UnimplementedGate` in `evaluate_shadow`
/// until T3.4/T4 follow-on work lands them.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvaluationContext {
    pub intent_admission: Option<IntentAdmissionInput>,
    pub entity_binding: Option<EntityBindingInput>,
    pub pack_resolution: Option<PackResolutionInput>,
    pub dag_proof: Option<DagProofInput>,
    pub authority: Option<AuthorityInput>,
    pub evidence: Option<EvidenceInput>,
    pub write_set: Option<WriteSetInput>,
    pub snapshot: Option<SnapshotInput>,
    pub stp_classifier: Option<StpClassifierInput>,
    pub write_set_attestation: Option<WriteSetAttestationInput>,
}

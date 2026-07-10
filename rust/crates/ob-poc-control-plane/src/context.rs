//! T2 production input context: the single `Ctx` type shared by every
//! `Gate<Ctx>` adapter wired so far (`gate::Gate<Ctx>` is monomorphic per
//! evaluation pass — one `Ctx` type covers every registered gate).
//!
//! Each field is a plain, primitive-typed translation of an existing
//! validator's *already-computed* output (see the ownership ledger for the
//! C-0xx row each input type traces back to). This crate must not depend on
//! `ob-poc`/`dsl-runtime`/`sem_os_policy` domain types directly (§9.1
//! non-goals — no execution-tier dependency), so callers translate at the
//! call site; adapters here only grade the translated value, they never
//! recompute it.
//!
//! A `None` field means "this gate's input was not supplied for this
//! evaluation" — the corresponding gate adapter treats that as a hard
//! failure (fail-closed), never as a pass.

use crate::authority_gate::AuthorityInput;
use crate::dag_proof::DagProofInput;
use crate::evidence_gate::EvidenceInput;
use crate::intent_admission::IntentAdmissionInput;
use crate::pack_resolution::PackResolutionInput;
use crate::write_set::WriteSetInput;

/// Shared evaluation context for the T2 gate adapters (G1, G3, G4, G5, G6,
/// G7). `EntityBinding` (G2) has no adapter until T3.1, so any gate whose
/// declared dependency graph transitively requires it will report
/// `NotEvaluated` in a full `evaluate_collect_where_independent` pass until
/// T3 lands — an honest, shadow-mode-appropriate intermediate state, not a
/// bug in T2.
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    pub intent_admission: Option<IntentAdmissionInput>,
    pub pack_resolution: Option<PackResolutionInput>,
    pub dag_proof: Option<DagProofInput>,
    pub authority: Option<AuthorityInput>,
    pub evidence: Option<EvidenceInput>,
    pub write_set: Option<WriteSetInput>,
}

//! KYC substrate error type.

use thiserror::Error;

use crate::types::{EdgeId, Hash, ObligationId, PersonId, SubjectId, VerbFqn};

#[derive(Debug, Error)]
pub enum KycError {
    #[error("precondition failed for {verb}: {reason}")]
    PreconditionFailed { verb: VerbFqn, reason: String },

    /// TS.5 R1/§5 `geometry_refusal_is_distinguishable_from_stud_refusal`:
    /// the type-geometry layer (TS.1 §1's FIRST constraint layer — "is this
    /// triple a move at all") is structurally distinct from a stud
    /// (positional) violation, and must produce a distinguishable error, not
    /// share `PreconditionFailed`'s shape. "Not a move" versus "not legal
    /// here."
    #[error("geometry refused for {verb}: {reason}")]
    GeometryRefused { verb: VerbFqn, reason: String },

    #[error("edge {0:?} not found in control graph")]
    EdgeNotFound(EdgeId),

    #[error("subject {0:?} not found")]
    SubjectNotFound(SubjectId),

    #[error("person {0:?} not found")]
    PersonNotFound(PersonId),

    #[error("obligation {0:?} not found")]
    ObligationNotFound(ObligationId),

    #[error("target binding is missing a required field: {0}")]
    MissingTarget(String),

    #[error("verb {0:?} not in lexicon")]
    UnknownVerb(VerbFqn),

    #[error("determination not present; run compute-fold before freeze")]
    DeterminationNotReady,

    #[error("edge {0:?} has status {1}; cannot verify without prior evidence")]
    VerifyWithoutEvidence(EdgeId, String),

    #[error("economic edges sum to {sum:.2}% for subject {subject:?} before reconciliation; run reconcile-conflict")]
    UnreconciledConflict { subject: SubjectId, sum: f64 },

    /// A `lexicon_hash` on an event has no registered fold implementation.
    ///
    /// This is a replay-integrity error — not a fallback case.  An event written
    /// under a lexicon version that was never registered cannot be replayed
    /// faithfully, so the fold must halt.  There is no "latest version" fallback;
    /// that would silently destroy replay-faithfulness (Q7, K-18/K-31).
    #[error(
        "unregistered lexicon hash {0} — register a FoldImpl for this version before replaying"
    )]
    UnregisteredLexiconHash(Hash),

    #[error("other error: {0}")]
    Other(#[from] anyhow::Error),
}

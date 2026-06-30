//! KYC substrate error type.

use thiserror::Error;

use crate::types::{EdgeId, ObligationId, PersonId, SubjectId, VerbFqn};

#[derive(Debug, Error)]
pub enum KycError {
    #[error("precondition failed for {verb}: {reason}")]
    PreconditionFailed { verb: VerbFqn, reason: String },

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

    #[error("other error: {0}")]
    Other(#[from] anyhow::Error),
}

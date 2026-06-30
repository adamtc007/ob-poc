//! Store error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    /// A precondition (or fold) rejected the append. The caller should roll back.
    /// Carries the substrate's domain error (e.g. `UnregisteredLexiconHash`,
    /// `VerifyWithoutEvidence`).
    #[error("append rejected: {0}")]
    Rejected(#[from] ob_poc_kyc_substrate::KycError),

    /// A database / I/O error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    /// A stored row could not be rehydrated into an `IntentEvent` (corrupt jsonb,
    /// malformed hash, etc.). This is a data-integrity error, never expected at
    /// runtime — surfaced rather than silently skipped (K-35).
    #[error("row rehydration failed for subject {subject} seq {seq}: {reason}")]
    Rehydrate { subject: uuid::Uuid, seq: i64, reason: String },
}

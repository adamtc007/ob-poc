//! KYC/UBO durable verb-stream store (membrane) — EOP-DD-KYCUBO-002.
#![deny(unreachable_pub)]
//!
//! The **only** sqlx crate in the KYC stack. Implements the §3 append protocol
//! over Postgres against the `kyc_intent_events` + `kyc_subject_streams` tables
//! (migration `20260630_kyc_intent_events.sql`).
//!
//! The substrate (`ob-poc-kyc-substrate`) stays pure: this crate hydrates rows
//! into owned `IntentEvent`s and reuses the source-agnostic folds verbatim.
//! "Same trait, no caller change" (DD-002 §1) is reconciled to "same pure
//! logic, different I/O interface": the in-memory `KycEventStore` trait is
//! sync + borrowed-returns and cannot back an async, transaction-scoped
//! Postgres store; what is actually reused is the fold/determination/precondition
//! logic, which is source-agnostic over `&[&IntentEvent]`.

pub mod cross_stream;
pub mod error;
pub mod manifest;
pub mod projection;
pub mod store;

pub use cross_stream::{
    enqueue_cross_stream_obligations, prior_freeze_persons, cross_stream_idem_key,
    CrossStreamEnqueueOutcome, CROSS_STREAM_OBLIGATION_CREATE, CROSS_STREAM_OBLIGATION_SUPERSEDE,
};
pub use error::StoreError;
pub use manifest::{publish_manifest, ManifestPublishOutcome};
pub use projection::{
    ObligationProjectionStats, PgKycObligationDrainer, PgKycObligationProjector,
    PgKycProjectionDrainer, PgKycProjector, ProjectionStats,
    CONTROL_EDGE_PROJECTION_EFFECT, OBLIGATION_PROJECTION_EFFECT,
};
pub use store::{AppendOutcome, PgKycEventStore};

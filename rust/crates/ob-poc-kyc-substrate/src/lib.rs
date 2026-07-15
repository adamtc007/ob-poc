//! KYC/UBO W1 substrate — EOP-DD-KYCUBO-001.
#![deny(unreachable_pub)]
//!
//! In-memory vertical slice proving the semantic model (§7 exit criteria):
//!   - Verb-event contract (§2)
//!   - Lexicon-entry contract (§3)
//!   - Control & determination fold (§4.1)
//!   - Obligation fold (§4.2)
//!   - Demoted ownership-prong strategy + freeze (§5)
//!
//! **No sqlx. No DB. No schema.**  The durable `kyc_intent_events` table
//! replaces `InMemoryEventStore` in W1-proper behind the same interface.

pub mod determination;
pub mod error;
pub mod event;
pub mod fold;
pub mod lexicon;
pub mod types;

// ── Convenience re-exports ────────────────────────────────────────────────────

pub use determination::{
    find_subject_entity, freeze_determination, recover_determination_at, ControlProngStrategy,
    DeterminationInProgress, DeterminationPin, DeterminationStrategy, FrozenDetermination,
    OwnershipProngStrategy, Prong, ProngCandidate, RecoveryPin, SmoResult,
};
pub use error::KycError;
pub use event::{CapturedEffect, InMemoryEventStore, IntentEvent, KycEventStore};
pub use fold::control::{
    check_control_preconditions, fold_control, natural_persons_from_events,
    reconciled_control_edges, reconciled_economic_edges, ControlState, EdgeKind, EdgeState,
    EdgeStatus, ReconciledControlEdge, StructureClass, TerminalStatus,
};
pub use fold::obligation::{
    fold_obligations, ObligationBasis, ObligationState, ObligationTracks, SubjectOverallState,
    SubjectRollup, TrackState,
};
pub use fold::registry::{
    fold_control_versioned, fold_obligations_versioned, FoldImpl, FoldRegistry, V1FoldImpl,
};
pub use lexicon::{phase1_lexicon, FoldId, LexiconEntry, LexiconManifest, Precondition, Taxonomy};
pub use types::{
    AuthorityRef, EdgeId, EntityId, EventId, Hash, IdemKey, ObligationId, PersonId, Principal,
    SubjectId, TargetBinding, VerbFqn,
};

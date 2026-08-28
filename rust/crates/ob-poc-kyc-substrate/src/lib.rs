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

pub(crate) mod determination;
pub(crate) mod error;
pub(crate) mod evaluation;
pub(crate) mod event;
pub(crate) mod fold;
pub(crate) mod geometry;
pub(crate) mod lexicon;
pub(crate) mod placement;
pub(crate) mod preview;
pub(crate) mod render;
pub(crate) mod types;

// ── Convenience re-exports ────────────────────────────────────────────────────

pub use determination::{
    compute_assurance, detect_pierces_in_chain, detect_statutory_stops, find_subject_entity,
    freeze_determination, fund_pivot_resolve, pull_smo_on_exhaustion, recover_determination_at,
    recover_determination_bitemporal, ControlProngStrategy, CooperativeMemberStrategy,
    DelegationStatus, DeterminationAssurance, DeterminationInProgress, DeterminationPin,
    DeterminationStrategy, FoundationCouncilStrategy, FrozenDetermination, FundControlStrategy,
    FundPivotResult, NomineePierceStrategy, OwnershipProngStrategy, PierceRecord, PivotCycleRecord,
    Prong, ProngCandidate, ProvisionalityReason, RecordedPivot, RecoveryPin, SmoPullRecord,
    SmoResult, StateOwnedStrategy, TraversalStop, TrustRoleStrategy, STRATEGY_DELEGATION_REGISTRY,
};
pub use error::KycError;
pub use evaluation::{
    applicability_holds, board_state_hash, evaluate_checks, in_scope_check_ids,
    work_list_from_history, ApplicabilityCondition, BoardSnapshot, Check, EvaluationCatalogue,
    EvaluationRun, Finding, ProvenTypeCheck, RiskLevel, RunPins, RunTrigger, UnevaluableReason,
    Verdict,
};
pub use event::{CapturedEffect, InMemoryEventStore, IntentEvent, KycEventStore};
pub use fold::control::{
    check_control_preconditions, check_preconditions, control_admission, dispatch_for_entity_type,
    edges_of_kind_into, fold_control, governing_mandate_edges_into, natural_persons_from_events,
    proof_kind_from_wire, reconciled_control_edges, reconciled_economic_edges,
    reconciled_trust_edges, unpierced_nominee_edges, ControlAdmission, ControlState,
    DeterminationDispatch, EdgeKind, EdgeState, EdgeStatus, ProofKind, ProofRecord,
    ReconciledControlEdge, ReconciledEconomicEdge, ReconciledTrustEdge, StructureClass,
    TrustRoleKind, EDGE_KIND_WIRE_VALUES, PROOF_KIND_WIRE_VALUES,
};
pub use fold::obligation::{
    fold_obligations, ObligationBasis, ObligationState, ObligationTracks, SubjectOverallState,
    SubjectRollup, TrackState,
};
pub use fold::registry::{
    fold_control_versioned, fold_obligations_versioned, FoldImpl, FoldRegistry, V1FoldImpl,
};
pub use fold::type_registry::{
    entity_type_from_wire, fold_type_registry, EnquiryRecord,
    EntityTypeRecord, TypeCorrectionRecord, TypeRegistryState,
    ENTITY_TYPE_WIRE_VALUES,
};
pub use geometry::{
    check_type_geometry, pipe_of, EntityType, GeometryError, LinkageSource, Pipe,
    PipeClassification, ALL_ENTITY_TYPES, ALL_PIPES,
};
pub use lexicon::{
    assembly_lexicon, evaluation_lexicon, FoldId, LexiconEntry, LexiconManifest, Precondition,
    Taxonomy,
};
pub use placement::{
    enumerate_placement_set, LegalMove, MoveId, PlacementSet, ProposedEdge, NONE_OF_THE_ABOVE,
};
pub use preview::preview;
pub use render::render_intent_event_to_sexpr;
pub use types::{
    AuthorityRef, EdgeId, EntityId, EventId, Hash, IdemKey, ObligationId, PersonId, Principal,
    SubjectId, TargetBinding, VerbFqn,
};

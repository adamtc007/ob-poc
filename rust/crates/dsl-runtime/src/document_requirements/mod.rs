//! Governed document requirements computation.
//!
//! Bridges published SemOS document-policy snapshots to the current runtime
//! document inventory. The policy module resolves matching bundles against
//! entity context; the governed module applies them to compute outstanding
//! gaps + strength matrices per entity.
//!
//! Moved from `ob-poc::database` in Phase 4 Slice B as an extended R-group
//! (both services are self-contained — they only depend on sem_os_core
//! plus sqlx/chrono/uuid/serde).

pub mod governed;
pub mod policy;

pub use governed::{
    EntityPolicyContext, GovernedComponentStatus, GovernedDocumentGap,
    GovernedDocumentRequirements, GovernedDocumentRequirementsService, GovernedObligationCategory,
    GovernedObligationStatus, GovernedRequirementMatrix, GovernedStrategyStatus,
};
pub use policy::{
    ActiveDocumentPolicyBundle, DocumentPolicyService, PublishedEvidenceStrategy,
    PublishedProofObligation, PublishedRequirementProfile,
};

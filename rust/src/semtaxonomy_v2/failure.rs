//! Canonical failure taxonomy for deterministic NLCI compilation.

use serde::{Deserialize, Serialize};

/// Normalized compiler failure kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompilerFailureKind {
    StructuredIntentInvalid,
    SurfaceObjectUnresolved,
    SurfaceObjectAmbiguous,
    OperationUnresolved,
    BindingInvalid,
    BindingAmbiguous,
    CandidateSelectionEmpty,
    DiscriminationAmbiguous,
    CompositionInvalid,
    GovernedExecutionBlocked,
}

/// Specific ambiguity category for compiler failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmbiguityReason {
    MultipleEntities,
    MultipleOperations,
    MultipleBindings,
    MultipleCandidates,
}

/// Binding-related failure detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingFailure {
    /// Binding field that failed validation or resolution.
    pub field: String,
    /// Failure detail.
    pub message: String,
}

/// Resolution-related failure detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionFailure {
    /// Compiler phase that emitted the failure.
    pub phase: String,
    /// Failure detail.
    pub message: String,
}

/// Discrimination-related failure detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscriminationFailure {
    /// Candidate identifiers involved in the failed discrimination.
    #[serde(default)]
    pub candidate_ids: Vec<String>,
    /// Failure detail.
    pub message: String,
}

/// Canonical normalized compiler failure envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerFailure {
    /// Normalized top-level failure kind.
    pub kind: CompilerFailureKind,
    /// Optional ambiguity reason when the failure is ambiguous by nature.
    pub ambiguity_reason: Option<AmbiguityReason>,
    /// Optional binding detail.
    pub binding_failure: Option<BindingFailure>,
    /// Optional resolution detail.
    pub resolution_failure: Option<ResolutionFailure>,
    /// Optional discrimination detail.
    pub discrimination_failure: Option<DiscriminationFailure>,
    /// User-safe message suitable for API projection.
    pub user_message: String,
}

//! Phase 2 typed contracts for deterministic candidate selection.

use serde::{Deserialize, Serialize};

use crate::semtaxonomy_v2::CompilerCandidate;

use super::binding_resolution::BindingResolutionOutput;

/// Input to candidate selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSelectionInput {
    /// Output from binding resolution.
    pub binding: BindingResolutionOutput,
}

/// Output from candidate selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateSelectionOutput {
    /// Preserved binding-resolution output.
    pub binding: BindingResolutionOutput,
    /// Deterministic candidates surviving ranking/filtering.
    #[serde(default)]
    pub candidates: Vec<CompilerCandidate>,
}

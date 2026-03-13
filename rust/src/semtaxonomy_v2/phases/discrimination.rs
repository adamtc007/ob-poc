//! Phase 3 typed contracts for deterministic discrimination.

use serde::{Deserialize, Serialize};

use crate::semtaxonomy_v2::{CompilerCandidate, CompilerFailure};

use super::candidate_selection::CandidateSelectionOutput;

/// Input to discrimination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscriminationInput {
    /// Output from candidate selection.
    pub candidates: CandidateSelectionOutput,
}

/// Output from discrimination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscriminationOutput {
    /// Preserved candidate-selection output.
    pub candidates: CandidateSelectionOutput,
    /// Winning candidate when discrimination succeeds.
    pub selected_candidate: Option<CompilerCandidate>,
    /// Normalized failure when discrimination cannot select deterministically.
    pub failure: Option<CompilerFailure>,
}

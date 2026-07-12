//! Phase 4 typed contracts for composition and parameter binding.

use serde::{Deserialize, Serialize};

use crate::semtaxonomy_v2::{CompilerCandidate, CompilerFailure, CompilerSelection};

use super::discrimination::DiscriminationOutput;

/// Input to composition and parameter binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionInput {
    /// Output from discrimination.
    pub discrimination: DiscriminationOutput,
}

/// Output from composition and parameter binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionOutput {
    /// Candidate set carried through the compiler.
    #[serde(default)]
    pub candidates: Vec<CompilerCandidate>,
    /// Final deterministic compiler selection.
    pub selection: Option<CompilerSelection>,
    /// Normalized failure when composition cannot complete.
    pub failure: Option<CompilerFailure>,
}

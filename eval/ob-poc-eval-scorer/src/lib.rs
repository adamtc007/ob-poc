#![forbid(unsafe_code)]
//! Gates-first scorer skeleton for the Sage Eval Harness.

use serde::{Deserialize, Serialize};

/// Scorer configuration for future scorecard generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScorerConfig {
    pub gates_first: bool,
}

impl Default for ScorerConfig {
    fn default() -> Self {
        Self { gates_first: true }
    }
}

/// Errors emitted by the scorer boundary.
#[derive(Debug, thiserror::Error)]
pub enum ScorerError {
    #[error("scorer implementation is not wired yet")]
    NotImplemented,
}

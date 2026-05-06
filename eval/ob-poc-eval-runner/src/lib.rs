#![forbid(unsafe_code)]
//! Replay runner skeleton for the Sage Eval Harness.

use ob_poc_eval_schema::FixtureStrategy;
use serde::{Deserialize, Serialize};

/// Runner configuration shared by future case replay commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub default_fixture_strategy: FixtureStrategy,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            default_fixture_strategy: FixtureStrategy::EphemeralFixture,
        }
    }
}

/// Errors emitted by the eval runner boundary.
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("runner implementation is not wired yet")]
    NotImplemented,
}

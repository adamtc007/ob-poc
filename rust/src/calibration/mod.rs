//! Loopback calibration harness for SemOS / DSL execution.

pub mod classifier;
pub mod db;
pub mod drift;
pub mod generator;
pub mod harness;
pub mod integration;
pub mod metrics;
pub mod pre_screen;
pub mod seed;
pub mod types;

// The calibration pipeline fns/types below are `pub`: driven across the crate
// boundary by `xtask` (`xtask/src/calibration.rs`).
pub use classifier::{classify_outcome};
pub use db::CalibrationStore;
pub use drift::compute_drift;
pub use generator::{build_generation_prompt};
pub use generator::parse_generated_utterances;
pub use harness::{execute_calibration_utterance, load_trace, CalibrationFixtures};
pub use integration::{generate_proposed_gaps, generate_suggested_clarifications};
pub use metrics::compute_metrics;
pub use pre_screen::pre_screen_utterances;
pub use seed::{build_scenario_seed};
pub use types::{CalibrationDrift, CalibrationExecutionShape, CalibrationFixtureTransition, CalibrationMode, CalibrationPortfolioEntry, CalibrationRun, CalibrationScenario, CalibrationScenarioBundle, CalibrationUtteranceReviewRow, CalibrationWriteThroughSummary, ProposedGapEntry, SuggestedClarification};

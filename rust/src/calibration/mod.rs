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

pub use classifier::{classify_outcome};
pub(crate) use classifier::{CalibrationUtteranceRow};
pub use db::CalibrationStore;
pub use drift::compute_drift;
pub use generator::{build_generation_prompt, parse_generated_utterances};
pub use harness::{execute_calibration_utterance, load_trace, CalibrationFixtures};
pub(crate) use harness::{FixtureEntity};
pub use integration::{generate_proposed_gaps, generate_suggested_clarifications};
pub use metrics::compute_metrics;
pub use pre_screen::pre_screen_utterances;
pub use seed::{build_scenario_seed};
pub(crate) use seed::{compute_situation_signature, derive_operational_phase};
pub use types::{CalibrationDrift, CalibrationExecutionShape, CalibrationFixtureTransition, CalibrationMode, CalibrationPortfolioEntry, CalibrationRun, CalibrationScenario, CalibrationScenarioBundle, CalibrationUtteranceReviewRow, CalibrationWriteThroughSummary, ProposedGapEntry, SuggestedClarification};
pub(crate) use types::{CalibrationOutcome, CalibrationVerdict, ConfusionRisk, EmbeddingPreScreen, ExpectedOutcome, GeneratedUtterance, GovernanceStatus, NearNeighbourVerb, PreScreenStratum};

//! Adversarial Verification Module
//!
//! Implements a game-theoretic "Trust But Verify → Distrust And Verify" model
//! where every piece of information is treated as a CLAIM that must be VERIFIED.
//!
//! The standard: "Would this process catch a sophisticated liar?"
//!
//! ## Architecture
//!
//! This module builds on existing infrastructure:
//! - `attribute_observations` - Evidence storage with confidence scores
//! - `client_allegations` - Client claims requiring verification
//! - `observation_discrepancies` - Conflict tracking and resolution
//! - `cbu_relationship_verification` - Ownership convergence model
//!
//! ## Key Components
//!
//! - [`ConfidenceCalculator`] - Aggregates confidence from multiple observations
//! - [`PatternDetector`] - Detects adversarial patterns (circular ownership, layering)
//! - [`EvasionDetector`] - Analyzes behavioral patterns for evasion signals
//! - [`VerificationAggregator`] - CBU-level verification status rollup
//!
//! ## Confidence Thresholds
//!
//! | Band | Score | Meaning |
//! |------|-------|---------|
//! | VERIFIED | ≥0.80 | High confidence, verified |
//! | PROVISIONAL | ≥0.60 | Acceptable with caveats |
//! | SUSPECT | ≥0.40 | Requires investigation |
//! | REJECTED | <0.40 | Insufficient evidence |

pub mod confidence;
pub mod evasion;
pub mod patterns;
pub mod registry;
pub mod types;

// Re-exports for convenience
pub use confidence::{
    ConfidenceBand, ConfidenceCalculator, ConfidenceResult, ConfidenceThresholds,
};
pub use evasion::{EvasionDetector, EvasionDetectorConfig, EvasionReport, EvasionSignal};
pub use patterns::{
    DetectedPattern, PatternDetector, PatternDetectorConfig, PatternSeverity, PatternType,
};
pub use registry::{RegistryCheckResult, RegistryVerifier};
pub use types::{
    Challenge, ChallengeStatus, ChallengeType, Claim, ClaimStatus, Escalation, EscalationLevel,
    EscalationStatus, Evidence, EvidenceSource, Inconsistency, InconsistencySeverity,
};

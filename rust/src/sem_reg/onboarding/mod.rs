//! Semantic Registry onboarding pipeline.
//!
//! Provides a 6-step pipeline for onboarding a new entity type into the
//! semantic registry. Each step publishes the required snapshots (entity type,
//! attributes, verb contracts, taxonomy placement, view assignment, evidence
//! requirements) using the same idempotent publish pattern as the scanner.

pub mod defaults;
pub mod pipeline;
pub mod validators;

pub use pipeline::{OnboardingPipeline, OnboardingRequest, OnboardingResult, StepResult};

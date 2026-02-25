//! Semantic Registry onboarding pipeline.
//!
//! Provides a 6-step pipeline for onboarding a new entity type into the
//! semantic registry. Each step publishes the required snapshots (entity type,
//! attributes, verb contracts, taxonomy placement, view assignment, evidence
//! requirements) using the same idempotent publish pattern as the scanner.
//!
//! ## Phase B0 — Extraction Pipeline (read-only)
//!
//! 1. `verb_extract` — parse DSL verb YAML → `Vec<VerbExtract>`
//! 2. `schema_extract` — query PostgreSQL information_schema → `Vec<TableExtract>`
//! 3. `xref` — cross-reference verbs ↔ schema → classified `Vec<AttributeCandidate>`
//! 4. `entity_infer` — group into entity types, classify FK relationships
//! 5. `manifest` — assemble `OnboardingManifest` + JSON serialization
//!
//! ## Phase B1 — Bootstrap Seed (one-time write)
//!
//! 6. `seed` — bootstrap write to `sem_reg.snapshots` with `BOOTSTRAP_SET_ID` guard
//! 7. `report` — format `BootstrapReport` output

pub mod defaults;
pub mod entity_infer;
pub mod manifest;
pub mod pipeline;
pub mod report;
pub mod schema_extract;
pub mod seed;
pub mod validators;
pub mod verb_extract;
pub mod xref;

pub use pipeline::{OnboardingPipeline, OnboardingRequest, OnboardingResult, StepResult};

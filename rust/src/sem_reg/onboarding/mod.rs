//! Semantic Registry onboarding extraction + bootstrap seed pipeline.
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

pub mod entity_infer;
pub mod manifest;
pub mod schema_extract;
pub mod seed;
pub mod verb_extract;
pub mod xref;

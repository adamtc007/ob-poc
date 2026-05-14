//! ob-poc-semtaxonomy — Semantic taxonomy entity-extraction layer.
//!
//! ## Capability claim
//!
//! Owns the entity-extraction layer: mention typing, candidate scoring,
//! the semtaxonomy snapshot consumed downstream by the Drafter. Distinct
//! from `ob-poc-taxonomy` (combinators) and `ob-poc-ontology` (lifecycle
//! stages).
//!
//! ## Anti-charter
//!
//! - NOT the higher-level semtaxonomy_v2 service (still in ob-poc because
//!   it depends on `crate::sage`).
//! - NOT taxonomy combinators or ontology lifecycle.

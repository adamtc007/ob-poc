//! ob-poc-ontology — Ontology lifecycle and semantic-stage reference data.
//!
//! ## Capability claim
//!
//! Owns the ontology lifecycle vocabulary: lifecycle-stage definitions,
//! semantic-stage definitions, the entity taxonomy YAML loader, and the
//! ontology service-level lookup helpers. Pure config + types — no
//! database coupling.
//!
//! ## Anti-charter
//!
//! - NOT taxonomy combinators (those live in `ob-poc-taxonomy`).
//! - NOT entity extraction (that's `ob-poc-semtaxonomy`).

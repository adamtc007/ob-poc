//! ob-poc-entity-linking — Entity-linking reference data and resolver shapes.
//!
//! ## Capability claim
//!
//! Owns the entity-linking data plane: mention extraction, candidate
//! resolution, lexicon snapshot, normalization, compiler — the shapes for
//! linking free-form mentions in utterances to canonical entities in the
//! registry.
//!
//! ## Anti-charter
//!
//! - NOT the LexiconService runtime singleton (that's in ob-poc itself).
//! - NOT the Drafter mention pipeline.

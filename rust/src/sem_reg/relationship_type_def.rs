//! Relationship type definition body — re-export of the canonical
//! definition in `sem_os_core::relationship_type_def`.
//!
//! Phase 4.7 audit follow-up (2026-05-13): the local parallel
//! definition that lived here was byte-identical to
//! `sem_os_core::relationship_type_def` for the production types
//! (`RelationshipTypeDefBody`, `RelationshipCardinality`,
//! `Directionality`) — only test fixtures and doc comments
//! differed. Collapsing to a re-export removes 1 entry from the
//! schema-authority drift allowlist and keeps `sem_os_core` as
//! the single schema authority per V&S §O7 / ADN §7.3.

pub use sem_os_ontology::relationship_type_def::*;

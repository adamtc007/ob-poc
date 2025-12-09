//! Ontology module for entity taxonomy and lifecycle management.
//!
//! This module loads and manages the entity taxonomy configuration from YAML,
//! providing:
//! - Entity type definitions with DB mappings
//! - Lifecycle state machines for entities
//! - FK relationship inference for the DSL planner
//! - Implicit entity creation configuration
//!
//! Two config sources drive the planner:
//! 1. `entity_taxonomy.yaml` - Entity definitions (this module)
//! 2. Verb YAML files - Verb lifecycle semantics (via runtime_registry)

mod lifecycle;
mod service;
mod taxonomy;
mod types;

pub use lifecycle::{is_valid_state, is_valid_transition, valid_next_states};
pub use service::{ontology, OntologyService};
pub use taxonomy::EntityTaxonomy;
pub use types::*;

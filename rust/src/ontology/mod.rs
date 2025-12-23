//! Ontology module for entity taxonomy and lifecycle management.
//!
//! This module loads and manages the entity taxonomy configuration from YAML,
//! providing:
//! - Entity type definitions with DB mappings
//! - Lifecycle state machines for entities
//! - FK relationship inference for the DSL planner
//! - Implicit entity creation configuration
//! - Semantic stage map for onboarding journey tracking
//!
//! Config sources:
//! 1. `entity_taxonomy.yaml` - Entity definitions
//! 2. `semantic_stage_map.yaml` - Onboarding stage definitions
//! 3. Verb YAML files - Verb lifecycle semantics (via runtime_registry)

mod lifecycle;
mod semantic_stage;
mod service;
mod taxonomy;
mod types;

pub use lifecycle::{is_valid_state, is_valid_transition, valid_next_states};
pub use semantic_stage::SemanticStageRegistry;
pub use service::{ontology, OntologyService};
pub use taxonomy::EntityTaxonomy;
pub use types::*;

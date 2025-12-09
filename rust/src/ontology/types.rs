//! Type definitions for the entity taxonomy configuration.
//!
//! These types map directly to the YAML structure in `config/ontology/entity_taxonomy.yaml`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root configuration structure for entity taxonomy.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityTaxonomyConfig {
    /// Configuration version
    pub version: String,

    /// Entity type definitions keyed by entity type name
    pub entities: HashMap<String, EntityDef>,

    /// FK relationships for planner inference
    #[serde(default)]
    pub relationships: Vec<FkRelationship>,

    /// Reference data tables (lookup tables, not DSL entities)
    #[serde(default)]
    pub reference_tables: HashMap<String, ReferenceTableDef>,
}

/// Definition of an entity type.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityDef {
    /// Human-readable description
    pub description: String,

    /// Category for grouping (subject, entity, kyc, document, etc.)
    pub category: String,

    /// Database configuration
    pub db: EntityDbConfig,

    /// Parent type for subtypes (e.g., proper_person -> entity)
    #[serde(default)]
    pub parent_type: Option<String>,

    /// Alias for another entity type
    #[serde(default)]
    pub alias_for: Option<String>,

    /// Search key definitions for lookups
    #[serde(default)]
    pub search_keys: Vec<SearchKeyDef>,

    /// Lifecycle state machine (optional - some entities have no lifecycle)
    #[serde(default)]
    pub lifecycle: Option<EntityLifecycle>,

    /// Subtype configuration (for base types like entity)
    #[serde(default)]
    pub subtypes: Option<SubtypeConfig>,

    /// Implicit creation configuration
    #[serde(default)]
    pub implicit_create: Option<ImplicitCreateConfig>,
}

/// Database configuration for an entity type.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityDbConfig {
    /// Database schema (e.g., "ob-poc", "kyc")
    pub schema: String,

    /// Table name
    pub table: String,

    /// Primary key column name
    pub pk: String,

    /// Type discriminator column (for polymorphic tables)
    #[serde(default)]
    pub type_column: Option<String>,

    /// Table for type code lookup
    #[serde(default)]
    pub type_lookup_table: Option<String>,

    /// Extension table for subtype-specific columns
    #[serde(default)]
    pub extension_table: Option<String>,

    /// FK column in extension table
    #[serde(default)]
    pub extension_fk: Option<String>,

    /// Type code value for this subtype
    #[serde(default)]
    pub type_code: Option<String>,
}

/// Search key definition for entity lookups.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SearchKeyDef {
    /// Single column search key
    Single {
        column: String,
        #[serde(default)]
        unique: bool,
        #[serde(default)]
        applies_to: Option<Vec<String>>,
    },
    /// Composite (multi-column) search key
    Composite {
        columns: Vec<String>,
        #[serde(default)]
        unique: bool,
        #[serde(default)]
        applies_to: Option<Vec<String>>,
    },
}

/// Lifecycle state machine definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityLifecycle {
    /// Column that stores the status
    pub status_column: String,

    /// Valid states
    pub states: Vec<String>,

    /// State transition rules
    pub transitions: Vec<StateTransition>,

    /// Initial state when entity is created
    pub initial_state: String,
}

/// State transition rule.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateTransition {
    /// Source state
    pub from: String,

    /// Valid target states
    pub to: Vec<String>,
}

/// Subtype configuration for polymorphic entity types.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubtypeConfig {
    /// Table containing type definitions
    pub source_table: String,

    /// Column containing type code
    pub code_column: String,

    /// Map of subtype code to extension table
    pub extensions: HashMap<String, String>,
}

/// Implicit entity creation configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImplicitCreateConfig {
    /// Whether implicit creation is allowed
    pub allowed: bool,

    /// Canonical verb to use for creation (e.g., "cbu.create")
    #[serde(default)]
    pub canonical_verb: Option<String>,

    /// Pattern for canonical verb (e.g., "entity.create-{subtype}")
    #[serde(default)]
    pub canonical_verb_pattern: Option<String>,

    /// Required arguments for implicit creation
    #[serde(default)]
    pub required_args: Vec<String>,
}

/// FK relationship for planner inference.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FkRelationship {
    /// Parent entity type or category
    pub parent: String,

    /// Child entity type or category
    pub child: String,

    /// FK argument name to inject (e.g., "cbu-id")
    pub fk_arg: String,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
}

/// Reference table definition (lookup tables).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReferenceTableDef {
    /// Table name
    pub table: String,

    /// Schema
    pub schema: String,

    /// Primary key column
    pub pk: String,

    /// Column used for search/lookup
    pub search_key: String,
}

// =============================================================================
// Verb Lifecycle Extension Types
// =============================================================================
// Note: VerbLifecycle is defined in dsl_v2::config::types and re-exported here.
// This avoids duplication while keeping the ontology module self-contained for docs.

// Re-export VerbLifecycle from dsl_v2 config
pub use crate::dsl_v2::config::types::VerbLifecycle;

/// Extended VerbProduces with lifecycle info.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct VerbProducesExt {
    /// Entity type produced (e.g., "cbu", "entity")
    #[serde(rename = "type")]
    pub produced_type: String,

    /// Subtype if applicable (e.g., "proper_person")
    #[serde(default)]
    pub subtype: Option<String>,

    /// Whether this verb can resolve existing entities
    #[serde(default)]
    pub resolved: bool,

    /// Initial state when creating new entity
    #[serde(default)]
    pub initial_state: Option<String>,
}

impl EntityLifecycle {
    /// Check if a transition from `from_state` to `to_state` is valid.
    pub fn is_valid_transition(&self, from_state: &str, to_state: &str) -> bool {
        self.transitions
            .iter()
            .any(|t| t.from == from_state && t.to.contains(&to_state.to_string()))
    }

    /// Get valid next states from a given state.
    pub fn valid_next_states(&self, from_state: &str) -> Vec<&str> {
        self.transitions
            .iter()
            .find(|t| t.from == from_state)
            .map(|t| t.to.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Check if a state is valid for this lifecycle.
    pub fn is_valid_state(&self, state: &str) -> bool {
        self.states.iter().any(|s| s == state)
    }
}

impl SearchKeyDef {
    /// Get the columns in this search key.
    pub fn columns(&self) -> Vec<&str> {
        match self {
            SearchKeyDef::Single { column, .. } => vec![column.as_str()],
            SearchKeyDef::Composite { columns, .. } => columns.iter().map(|s| s.as_str()).collect(),
        }
    }

    /// Check if this is a unique key.
    pub fn is_unique(&self) -> bool {
        match self {
            SearchKeyDef::Single { unique, .. } => *unique,
            SearchKeyDef::Composite { unique, .. } => *unique,
        }
    }
}

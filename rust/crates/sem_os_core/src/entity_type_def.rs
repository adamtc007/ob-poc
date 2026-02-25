//! Entity type definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

/// Body of an `entity_type_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeDefBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    #[serde(default)]
    pub db_table: Option<DbTableMapping>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle_states: Vec<LifecycleStateDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_attributes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_attributes: Vec<String>,
    #[serde(default)]
    pub parent_type: Option<String>,
}

/// Database table mapping for entity storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTableMapping {
    pub schema: String,
    pub table: String,
    pub primary_key: String,
    #[serde(default)]
    pub name_column: Option<String>,
}

/// A state in an entity's lifecycle state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleStateDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<LifecycleTransition>,
    #[serde(default)]
    pub terminal: bool,
}

/// A valid state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleTransition {
    pub to: String,
    #[serde(default)]
    pub trigger_verb: Option<String>,
    #[serde(default)]
    pub guard: Option<String>,
}

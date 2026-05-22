//! Pure data types shared between dsl-core and sem_os_core resolver.

use serde::{Deserialize, Serialize};

/// Structural facts extracted from a shape rule chain.
/// Defined here so both dsl-core (type definitions) and sem_os_core
/// (computation) can use the same type without circular dependencies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct StructuralFacts {
    #[serde(default)]
    pub jurisdiction: Option<String>,

    #[serde(default)]
    pub structure_type: Option<String>,

    #[serde(default)]
    pub allowed_structure_types: Vec<String>,

    #[serde(default)]
    pub document_bundles: Vec<String>,

    #[serde(default)]
    pub trading_profile_type: Option<String>,

    #[serde(default)]
    pub required_roles: Vec<String>,

    #[serde(default)]
    pub optional_roles: Vec<String>,

    #[serde(default)]
    pub deferred_roles: Vec<String>,
}

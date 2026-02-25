//! Relationship type definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

/// Body of a `relationship_type_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipTypeDefBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub source_entity_type_fqn: String,
    pub target_entity_type_fqn: String,
    pub cardinality: RelationshipCardinality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directionality: Option<Directionality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse_fqn: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
}

/// Directionality of a relationship.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Directionality {
    Forward,
    Reverse,
    Bidirectional,
}

/// Cardinality of a relationship.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipCardinality {
    OneToOne,
    OneToMany,
    ManyToMany,
}

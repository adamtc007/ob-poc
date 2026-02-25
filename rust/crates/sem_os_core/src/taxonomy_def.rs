//! Taxonomy definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Body of a `taxonomy_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyDefBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    #[serde(default)]
    pub root_node_fqn: Option<String>,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub classification_axis: Option<String>,
}

/// Body of a `taxonomy_node` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyNodeBody {
    pub fqn: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub taxonomy_fqn: String,
    #[serde(default)]
    pub parent_fqn: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

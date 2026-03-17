//! Constellation map definition body types — pure value types, no DB dependency.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Body of a `constellation_map` registry snapshot.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstellationMapDefBody {
    pub fqn: String,
    pub constellation: String,
    #[serde(default)]
    pub description: Option<String>,
    pub jurisdiction: String,
    pub slots: BTreeMap<String, SlotDef>,
    #[serde(default)]
    pub bulk_macros: Vec<String>,
}

/// Authored slot definition inside a constellation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlotDef {
    #[serde(rename = "type")]
    pub slot_type: SlotType,
    #[serde(default)]
    pub entity_kinds: Vec<String>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub pk: Option<String>,
    #[serde(default)]
    pub join: Option<JoinDef>,
    #[serde(default)]
    pub occurrence: Option<usize>,
    pub cardinality: Cardinality,
    #[serde(default)]
    pub depends_on: Vec<DependencyEntry>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub state_machine: Option<String>,
    #[serde(default)]
    pub overlays: Vec<String>,
    #[serde(default)]
    pub edge_overlays: Vec<String>,
    #[serde(default)]
    pub verbs: BTreeMap<String, VerbPaletteEntry>,
    #[serde(default)]
    pub children: BTreeMap<String, SlotDef>,
    #[serde(default)]
    pub max_depth: Option<usize>,
}

/// Supported slot classes.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlotType {
    Cbu,
    Entity,
    EntityGraph,
    Case,
    Tollgate,
    Mandate,
}

/// Supported slot cardinality semantics.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    Root,
    Mandatory,
    Optional,
    Recursive,
}

/// Join definition for non-root slots.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct JoinDef {
    pub via: String,
    pub parent_fk: String,
    pub child_fk: String,
    #[serde(default)]
    pub filter_column: Option<String>,
    #[serde(default)]
    pub filter_value: Option<String>,
}

/// Dependency declaration for a slot.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencyEntry {
    Simple(String),
    Explicit { slot: String, min_state: String },
}

impl DependencyEntry {
    pub fn slot_name(&self) -> &str {
        match self {
            Self::Simple(slot) => slot,
            Self::Explicit { slot, .. } => slot,
        }
    }

    pub fn min_state(&self) -> &str {
        match self {
            Self::Simple(_) => "filled",
            Self::Explicit { min_state, .. } => min_state,
        }
    }
}

/// Verb palette entry in simple or gated form.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum VerbPaletteEntry {
    Simple(String),
    Gated {
        verb: String,
        when: VerbAvailability,
    },
}

impl VerbPaletteEntry {
    pub fn verb_fqn(&self) -> &str {
        match self {
            Self::Simple(verb) => verb,
            Self::Gated { verb, .. } => verb,
        }
    }
}

/// Availability expression for a gated verb.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum VerbAvailability {
    One(String),
    Many(Vec<String>),
}

impl VerbAvailability {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value.clone()],
            Self::Many(values) => values.clone(),
        }
    }
}

//! Constellation map definition body types — pure value types, no DB dependency.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;

/// Body of a `constellation_map` registry snapshot.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstellationMapDefBody {
    pub fqn: String,
    pub constellation: String,
    #[serde(default)]
    pub description: Option<String>,
    pub jurisdiction: String,
    pub slots: BTreeMap<String, SlotDef>,
}

/// Closure semantics for a composite slot.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClosureType {
    Open,
    ClosedBounded,
    ClosedUnbounded,
}

/// Candidate eligibility constraint for attaching or populating a slot.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum EligibilityConstraint {
    /// v1: constrain by authored entity kinds.
    EntityKinds { entity_kinds: Vec<String> },
    /// v2: constrain by typed shape taxonomy position.
    ShapeTaxonomyPosition { shape_taxonomy_position: String },
}

/// Role guard metadata for discretionary gate enforcement.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RoleGuard {
    #[serde(default)]
    pub any_of: Vec<String>,

    #[serde(default)]
    pub all_of: Vec<String>,
}

/// Audit classification for discretionary gate outcomes.
pub type AuditClass = String;

/// Completeness assertion metadata for open slots.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletenessAssertionConfig {
    #[serde(default)]
    pub predicate: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, YamlValue>,
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
    #[serde(default)]
    pub closure: Option<ClosureType>,
    #[serde(default)]
    pub eligibility: Option<EligibilityConstraint>,
    #[serde(default)]
    pub cardinality_max: Option<u64>,
    #[serde(default)]
    pub entry_state: Option<String>,
    #[serde(default)]
    pub attachment_predicates: Vec<String>,
    #[serde(default)]
    pub addition_predicates: Vec<String>,
    #[serde(default)]
    pub aggregate_breach_checks: Vec<String>,
    #[serde(
        default,
        rename = "+attachment_predicates",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additive_attachment_predicates: Vec<String>,
    #[serde(
        default,
        rename = "+addition_predicates",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additive_addition_predicates: Vec<String>,
    #[serde(
        default,
        rename = "+aggregate_breach_checks",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additive_aggregate_breach_checks: Vec<String>,
    #[serde(default)]
    pub role_guard: Option<RoleGuard>,
    #[serde(default)]
    pub justification_required: Option<bool>,
    #[serde(default)]
    pub audit_class: Option<AuditClass>,
    #[serde(default)]
    pub completeness_assertion: Option<CompletenessAssertionConfig>,
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

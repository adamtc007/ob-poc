use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

/// Raw constellation map definition loaded from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstellationMapDef {
    pub constellation: String,
    pub description: Option<String>,
    pub jurisdiction: String,
    pub slots: BTreeMap<String, SlotDef>,
    #[serde(default)]
    pub bulk_macros: Vec<String>,
}

/// Raw slot definition loaded from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlotDef {
    #[serde(rename = "type")]
    pub slot_type: SlotType,
    #[serde(default)]
    pub entity_kinds: Vec<String>,
    pub table: Option<String>,
    pub pk: Option<String>,
    pub join: Option<JoinDef>,
    pub occurrence: Option<usize>,
    pub cardinality: Cardinality,
    #[serde(default)]
    pub depends_on: Vec<DependencyEntry>,
    #[serde(default, deserialize_with = "deserialize_placeholder")]
    pub placeholder: Option<String>,
    #[serde(default = "default_placeholder_detection")]
    pub placeholder_detection: String,
    pub state_machine: Option<String>,
    #[serde(default)]
    pub overlays: Vec<String>,
    #[serde(default)]
    pub edge_overlays: Vec<String>,
    #[serde(default)]
    pub verbs: BTreeMap<String, VerbPaletteEntry>,
    #[serde(default)]
    pub children: BTreeMap<String, SlotDef>,
    pub max_depth: Option<usize>,
}

fn default_placeholder_detection() -> String {
    String::from("name_match")
}

fn deserialize_placeholder<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PlaceholderValue {
        Bool(bool),
        Text(String),
    }

    let value = Option::<PlaceholderValue>::deserialize(deserializer)?;
    Ok(match value {
        Some(PlaceholderValue::Bool(true)) => Some(String::from("placeholder")),
        Some(PlaceholderValue::Bool(false)) | None => None,
        Some(PlaceholderValue::Text(value)) => Some(value),
    })
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

/// Supported cardinality semantics.
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
    pub filter_column: Option<String>,
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
    /// Return the referenced slot name.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::constellation::DependencyEntry;
    ///
    /// let dep = DependencyEntry::Simple(String::from("cbu"));
    /// assert_eq!(dep.slot_name(), "cbu");
    /// ```
    pub fn slot_name(&self) -> &str {
        match self {
            Self::Simple(slot) => slot,
            Self::Explicit { slot, .. } => slot,
        }
    }

    /// Return the minimum required state for the dependency.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::constellation::DependencyEntry;
    ///
    /// let dep = DependencyEntry::Simple(String::from("cbu"));
    /// assert_eq!(dep.min_state(), "filled");
    /// ```
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
    /// Return the fully qualified verb name.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::constellation::VerbPaletteEntry;
    ///
    /// let entry = VerbPaletteEntry::Simple(String::from("cbu.read"));
    /// assert_eq!(entry.verb_fqn(), "cbu.read");
    /// ```
    pub fn verb_fqn(&self) -> &str {
        match self {
            Self::Simple(verb) => verb,
            Self::Gated { verb, .. } => verb,
        }
    }

    /// Return the reducer states in which the verb is available.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::constellation::{VerbAvailability, VerbPaletteEntry};
    ///
    /// let entry = VerbPaletteEntry::Gated {
    ///     verb: String::from("entity.read"),
    ///     when: VerbAvailability::Many(vec![String::from("filled")]),
    /// };
    /// assert_eq!(entry.available_in(), vec![String::from("filled")]);
    /// ```
    pub fn available_in(&self) -> Vec<String> {
        match self {
            Self::Simple(_) => Vec::new(),
            Self::Gated { when, .. } => when.to_vec(),
        }
    }
}

/// Availability expression for gated verbs.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum VerbAvailability {
    One(String),
    Many(Vec<String>),
}

impl VerbAvailability {
    pub(crate) fn to_vec(&self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value.clone()],
            Self::Many(values) => values.clone(),
        }
    }
}

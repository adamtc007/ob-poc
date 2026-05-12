//! Constellation family definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

/// Body of a `constellation_family_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConstellationFamilyDefBody {
    pub(crate) fqn: String,
    pub(crate) family_id: String,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) domain_id: String,
    #[serde(default)]
    pub(crate) selection_rules: Vec<SelectionRule>,
    #[serde(default)]
    pub(crate) constellation_refs: Vec<ConstellationRef>,
    #[serde(default)]
    pub(crate) candidate_jurisdictions: Vec<String>,
    #[serde(default)]
    pub(crate) candidate_entity_kinds: Vec<String>,
    pub(crate) grounding_threshold: GroundingThreshold,
}

/// A concrete constellation that this family can narrow into.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConstellationRef {
    pub(crate) constellation_id: String,
    pub(crate) label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) jurisdiction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) entity_kind: Option<String>,
    #[serde(default)]
    pub(crate) triggers: Vec<String>,
}

/// Deterministic authored narrowing rule from family to constellation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SelectionRule {
    pub(crate) condition: String,
    pub(crate) target_constellation: String,
    pub(crate) priority: u16,
}

/// Inputs required before Sem OS may treat a family as grounded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GroundingThreshold {
    #[serde(default)]
    pub(crate) required_input_keys: Vec<String>,
    #[serde(default)]
    pub(crate) requires_entity_instance: bool,
    #[serde(default)]
    pub(crate) allows_draft_instance: bool,
}

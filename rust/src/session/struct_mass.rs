//! StructMass - Weighted complexity scoring for automatic view mode selection.
//!
//! Mass calculation determines how "heavy" a scope is, which drives:
//! - Automatic view mode selection (Universe → Solar System → Detail)
//! - LOD (Level of Detail) decisions
//! - Performance optimizations (windowing, virtualization)
//!
//! The mass is computed from weighted entity counts, relationship complexity,
//! and structural depth. The weights are configurable per domain.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Breakdown of mass by category.
///
/// Each field represents a weighted contribution to total mass.
/// Weights are applied during calculation, not stored here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MassBreakdown {
    /// Number of CBUs in scope
    pub cbu_count: u32,

    /// Number of entities (all types combined)
    pub entity_count: u32,

    /// Breakdown by entity type (type_code -> count)
    #[serde(default)]
    pub by_entity_type: HashMap<String, u32>,

    /// Number of relationships (edges)
    pub relationship_count: u32,

    /// Breakdown by relationship type (type -> count)
    #[serde(default)]
    pub by_relationship_type: HashMap<String, u32>,

    /// Maximum ownership chain depth
    pub max_ownership_depth: u32,

    /// Number of leaf nodes (entities with no outgoing ownership)
    pub leaf_count: u32,

    /// Number of root nodes (entities with no incoming ownership)
    pub root_count: u32,

    /// Number of document nodes
    pub document_count: u32,

    /// Number of service/product nodes
    pub service_count: u32,

    /// Number of cycles detected in ownership graph
    pub cycle_count: u32,
}

/// Computed mass with breakdown and configuration reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructMass {
    /// The computed total mass score
    pub total: f32,

    /// Breakdown by category (pre-weighted counts)
    pub breakdown: MassBreakdown,

    /// Individual contributions to total (for debugging/display)
    #[serde(default)]
    pub contributions: MassContributions,
}

/// Individual weighted contributions to total mass.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MassContributions {
    pub from_cbus: f32,
    pub from_entities: f32,
    pub from_relationships: f32,
    pub from_depth: f32,
    pub from_cycles: f32,
    pub from_documents: f32,
    pub from_services: f32,
}

/// View modes determined by mass thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MassViewMode {
    /// Low mass: Show full detail (entity-level graph)
    Detail,
    /// Medium mass: Show clustered/aggregated view (solar system)
    SolarSystem,
    /// High mass: Show highly aggregated view (universe)
    Universe,
}

impl MassViewMode {
    /// Get string representation for serialization/API
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Detail => "detail",
            Self::SolarSystem => "solar_system",
            Self::Universe => "universe",
        }
    }
}

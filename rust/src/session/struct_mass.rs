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

impl MassBreakdown {
    /// Create an empty breakdown
    pub fn empty() -> Self {
        Self::default()
    }

    /// Check if the breakdown represents an empty scope
    pub fn is_empty(&self) -> bool {
        self.cbu_count == 0 && self.entity_count == 0
    }

    /// Get total node count (CBUs + entities)
    pub fn total_nodes(&self) -> u32 {
        self.cbu_count + self.entity_count
    }

    /// Add counts from another breakdown (for aggregation)
    pub fn merge(&mut self, other: &MassBreakdown) {
        self.cbu_count += other.cbu_count;
        self.entity_count += other.entity_count;
        self.relationship_count += other.relationship_count;
        self.max_ownership_depth = self.max_ownership_depth.max(other.max_ownership_depth);
        self.leaf_count += other.leaf_count;
        self.root_count += other.root_count;
        self.document_count += other.document_count;
        self.service_count += other.service_count;
        self.cycle_count += other.cycle_count;

        for (k, v) in &other.by_entity_type {
            *self.by_entity_type.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &other.by_relationship_type {
            *self.by_relationship_type.entry(k.clone()).or_default() += v;
        }
    }

    /// Increment entity count for a specific type
    pub fn add_entity(&mut self, entity_type: &str) {
        self.entity_count += 1;
        *self
            .by_entity_type
            .entry(entity_type.to_string())
            .or_default() += 1;
    }

    /// Increment relationship count for a specific type
    pub fn add_relationship(&mut self, relationship_type: &str) {
        self.relationship_count += 1;
        *self
            .by_relationship_type
            .entry(relationship_type.to_string())
            .or_default() += 1;
    }
}

/// Configuration for mass weight calculation.
///
/// These weights determine how different components contribute to total mass.
/// Higher weights mean that component has more influence on view mode selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassWeights {
    /// Weight per CBU (default: 10.0)
    pub cbu_weight: f32,

    /// Weight per entity (default: 1.0)
    pub entity_weight: f32,

    /// Weight per relationship (default: 0.5)
    pub relationship_weight: f32,

    /// Weight per unit of depth (default: 2.0)
    pub depth_weight: f32,

    /// Weight per cycle detected (default: 5.0) - complexity indicator
    pub cycle_weight: f32,

    /// Weight per document (default: 0.2)
    pub document_weight: f32,

    /// Weight per service (default: 0.3)
    pub service_weight: f32,

    /// Per-entity-type weight overrides (type_code -> weight)
    /// If not specified, uses entity_weight
    #[serde(default)]
    pub entity_type_weights: HashMap<String, f32>,

    /// Per-relationship-type weight overrides
    #[serde(default)]
    pub relationship_type_weights: HashMap<String, f32>,
}

impl Default for MassWeights {
    fn default() -> Self {
        Self {
            cbu_weight: 10.0,
            entity_weight: 1.0,
            relationship_weight: 0.5,
            depth_weight: 2.0,
            cycle_weight: 5.0,
            document_weight: 0.2,
            service_weight: 0.3,
            entity_type_weights: HashMap::new(),
            relationship_type_weights: HashMap::new(),
        }
    }
}

impl MassWeights {
    /// Get weight for a specific entity type
    pub fn entity_weight_for(&self, entity_type: &str) -> f32 {
        self.entity_type_weights
            .get(entity_type)
            .copied()
            .unwrap_or(self.entity_weight)
    }

    /// Get weight for a specific relationship type
    pub fn relationship_weight_for(&self, rel_type: &str) -> f32 {
        self.relationship_type_weights
            .get(rel_type)
            .copied()
            .unwrap_or(self.relationship_weight)
    }
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

impl StructMass {
    /// Create a zero mass
    pub fn zero() -> Self {
        Self {
            total: 0.0,
            breakdown: MassBreakdown::empty(),
            contributions: MassContributions::default(),
        }
    }

    /// Calculate mass from breakdown using provided weights
    pub fn calculate(breakdown: MassBreakdown, weights: &MassWeights) -> Self {
        // CBU contribution
        let from_cbus = breakdown.cbu_count as f32 * weights.cbu_weight;

        // Entity contribution (with per-type weights)
        let from_entities: f32 = breakdown
            .by_entity_type
            .iter()
            .map(|(t, c)| *c as f32 * weights.entity_weight_for(t))
            .sum();

        // Relationship contribution (with per-type weights)
        let from_relationships: f32 = breakdown
            .by_relationship_type
            .iter()
            .map(|(t, c)| *c as f32 * weights.relationship_weight_for(t))
            .sum();

        // Depth contribution
        let from_depth = breakdown.max_ownership_depth as f32 * weights.depth_weight;

        // Cycle contribution
        let from_cycles = breakdown.cycle_count as f32 * weights.cycle_weight;

        // Document contribution
        let from_documents = breakdown.document_count as f32 * weights.document_weight;

        // Service contribution
        let from_services = breakdown.service_count as f32 * weights.service_weight;

        let contributions = MassContributions {
            from_cbus,
            from_entities,
            from_relationships,
            from_depth,
            from_cycles,
            from_documents,
            from_services,
        };

        // Total
        let total = from_cbus
            + from_entities
            + from_relationships
            + from_depth
            + from_cycles
            + from_documents
            + from_services;

        Self {
            total,
            breakdown,
            contributions,
        }
    }

    /// Check if mass is zero (empty scope)
    pub fn is_zero(&self) -> bool {
        self.total == 0.0
    }

    /// Check if mass is "light" (suitable for detailed view)
    pub fn is_light(&self, threshold: f32) -> bool {
        self.total < threshold
    }

    /// Check if mass is "heavy" (needs higher-level view)
    pub fn is_heavy(&self, threshold: f32) -> bool {
        self.total >= threshold
    }

    /// Suggest a view mode based on mass using default thresholds
    pub fn suggested_view_mode(&self) -> MassViewMode {
        self.suggested_view_mode_with_thresholds(&MassThresholds::default())
    }

    /// Suggest a view mode based on mass using custom thresholds
    pub fn suggested_view_mode_with_thresholds(&self, thresholds: &MassThresholds) -> MassViewMode {
        if self.total < thresholds.detail_max {
            MassViewMode::Detail
        } else if self.total < thresholds.solar_system_max {
            MassViewMode::SolarSystem
        } else {
            MassViewMode::Universe
        }
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Mass: {:.1} ({} CBUs, {} entities, {} relationships, depth {})",
            self.total,
            self.breakdown.cbu_count,
            self.breakdown.entity_count,
            self.breakdown.relationship_count,
            self.breakdown.max_ownership_depth
        )
    }
}

/// Thresholds for automatic view mode selection based on mass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassThresholds {
    /// Below this: Detail view (full entity graph)
    pub detail_max: f32,

    /// Below this: Solar System view (clustered)
    /// Above solar_system_max: Universe view (highly aggregated)
    pub solar_system_max: f32,
}

impl Default for MassThresholds {
    fn default() -> Self {
        Self {
            detail_max: 100.0,        // ~50-100 entities
            solar_system_max: 1000.0, // ~500-1000 entities
        }
    }
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
    /// Determine view mode from mass using thresholds
    pub fn from_mass(mass: &StructMass, thresholds: &MassThresholds) -> Self {
        if mass.total < thresholds.detail_max {
            Self::Detail
        } else if mass.total < thresholds.solar_system_max {
            Self::SolarSystem
        } else {
            Self::Universe
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Detail => "Detail",
            Self::SolarSystem => "Solar System",
            Self::Universe => "Universe",
        }
    }

    /// Get icon name for UI
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Detail => "🔬",
            Self::SolarSystem => "🌍",
            Self::Universe => "🌌",
        }
    }

    /// Get string representation for serialization/API
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Detail => "detail",
            Self::SolarSystem => "solar_system",
            Self::Universe => "universe",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_breakdown() {
        let breakdown = MassBreakdown::empty();
        assert!(breakdown.is_empty());
        assert_eq!(breakdown.total_nodes(), 0);
    }

    #[test]
    fn test_mass_calculation() {
        let mut breakdown = MassBreakdown::empty();
        breakdown.cbu_count = 1;
        breakdown.add_entity("proper_person");
        breakdown.add_entity("proper_person");
        breakdown.add_entity("limited_company");
        breakdown.add_relationship("ownership");
        breakdown.max_ownership_depth = 3;

        let weights = MassWeights::default();
        let mass = StructMass::calculate(breakdown, &weights);

        // 1 CBU * 10 + 3 entities * 1 + 1 rel * 0.5 + 3 depth * 2 = 10 + 3 + 0.5 + 6 = 19.5
        assert!((mass.total - 19.5).abs() < 0.01);
    }

    #[test]
    fn test_view_mode_selection() {
        let thresholds = MassThresholds::default();

        // Light mass -> Detail
        let light = StructMass {
            total: 50.0,
            breakdown: MassBreakdown::empty(),
            contributions: MassContributions::default(),
        };
        assert_eq!(
            MassViewMode::from_mass(&light, &thresholds),
            MassViewMode::Detail
        );

        // Medium mass -> Solar System
        let medium = StructMass {
            total: 500.0,
            breakdown: MassBreakdown::empty(),
            contributions: MassContributions::default(),
        };
        assert_eq!(
            MassViewMode::from_mass(&medium, &thresholds),
            MassViewMode::SolarSystem
        );

        // Heavy mass -> Universe
        let heavy = StructMass {
            total: 2000.0,
            breakdown: MassBreakdown::empty(),
            contributions: MassContributions::default(),
        };
        assert_eq!(
            MassViewMode::from_mass(&heavy, &thresholds),
            MassViewMode::Universe
        );
    }

    #[test]
    fn test_breakdown_merge() {
        let mut a = MassBreakdown::empty();
        a.cbu_count = 2;
        a.add_entity("proper_person");

        let mut b = MassBreakdown::empty();
        b.cbu_count = 3;
        b.add_entity("limited_company");
        b.add_entity("limited_company");

        a.merge(&b);
        assert_eq!(a.cbu_count, 5);
        assert_eq!(a.entity_count, 3);
        assert_eq!(a.by_entity_type.get("proper_person"), Some(&1));
        assert_eq!(a.by_entity_type.get("limited_company"), Some(&2));
    }
}

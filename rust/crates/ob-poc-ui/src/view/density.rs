//! Density-based view mode rules
//!
//! Determines view mode based on how many entities are visible in the viewport.
//! This enables automatic switching between overview and detail modes as user
//! zooms/pans.

use serde::Deserialize;
use uuid::Uuid;

// =============================================================================
// DENSITY THRESHOLD
// =============================================================================

/// Threshold conditions for matching entity counts
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DensityThreshold {
    /// Match when count is greater than threshold
    GreaterThan { gt: u32, entity_type: String },
    /// Match when count is less than threshold
    LessThan { lt: u32, entity_type: String },
    /// Match when count is within range [min, max]
    Range {
        min: u32,
        max: u32,
        entity_type: String,
    },
    /// Match when exactly one entity is visible
    Single,
}

impl DensityThreshold {
    /// Check if threshold matches given count and entity type
    pub fn matches(&self, count: u32, entity_type: &str) -> bool {
        match self {
            Self::GreaterThan {
                gt,
                entity_type: et,
            } => entity_type.eq_ignore_ascii_case(et) && count > *gt,
            Self::LessThan {
                lt,
                entity_type: et,
            } => entity_type.eq_ignore_ascii_case(et) && count < *lt,
            Self::Range {
                min,
                max,
                entity_type: et,
            } => entity_type.eq_ignore_ascii_case(et) && count >= *min && count <= *max,
            Self::Single => count == 1,
        }
    }
}

// =============================================================================
// VIEW MODE
// =============================================================================

/// High-level view modes based on density
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// Universe level - all CBUs as dots/clusters
    #[default]
    AstroOverview,
    /// Clustered by jurisdiction/client/etc
    AstroClustered,
    /// Mix of clusters and expanded CBUs
    HybridDrilldown,
    /// Multiple CBUs with some detail
    MultiCbuDetail,
    /// Single CBU with full pyramid structure
    SingleCbuPyramid,
    /// Full detail with all entities and attributes
    FullDetail,
}

// =============================================================================
// NODE RENDER MODE
// =============================================================================

/// How to render nodes at different densities
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeRenderMode {
    /// Smallest: colored dot only
    CompactDot,
    /// Small: circle with status color
    #[default]
    LabeledCircle,
    /// Medium: expandable taxonomy tree
    ExpandedTaxonomy,
    /// Full: complete pyramid with all levels
    FullTaxonomyPyramid,
}

// =============================================================================
// DENSITY RULE
// =============================================================================

/// A rule that maps density conditions to view configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DensityRule {
    /// Threshold condition to match
    pub threshold: DensityThreshold,
    /// View mode to use when matched
    pub mode: ViewMode,
    /// How to render nodes
    pub node_rendering: NodeRenderMode,
    /// Optional: expand this taxonomy level
    #[serde(default)]
    pub expand_taxonomy: Option<String>,
    /// Optional: cluster by this dimension
    #[serde(default)]
    pub cluster_by: Option<String>,
    /// Whether to show floating persons (persons not in main hierarchy)
    #[serde(default)]
    pub show_floating_persons: bool,
}

// =============================================================================
// VISIBLE ENTITIES
// =============================================================================

/// Computed visibility information for current viewport
#[derive(Debug, Clone, Default)]
pub struct VisibleEntities {
    /// CBU entity IDs visible in viewport
    pub cbus: Vec<Uuid>,
    /// Person entity IDs visible in viewport
    pub persons: Vec<Uuid>,
    /// Other entity IDs visible in viewport
    pub other: Vec<Uuid>,
    /// Total count of visible entities
    pub total_count: u32,
    /// Density (entities per unit area)
    pub density: f32,
}

impl VisibleEntities {
    /// Create empty visible entities
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute visible entities from positioned nodes
    ///
    /// # Arguments
    /// * `nodes` - Iterator of (position, size, entity_type, entity_id)
    /// * `viewport` - Visible rectangle in world coordinates
    /// * `zoom` - Current camera zoom level
    /// * `min_visible_size` - Minimum screen-space size to count as "visible"
    pub fn compute<'a, I>(
        nodes: I,
        viewport_min: (f32, f32),
        viewport_max: (f32, f32),
        zoom: f32,
        min_visible_size: f32,
    ) -> Self
    where
        I: Iterator<Item = ((f32, f32), (f32, f32), &'a str, Uuid)>,
    {
        let mut cbus = Vec::new();
        let mut persons = Vec::new();
        let mut other = Vec::new();

        for ((x, y), (w, h), entity_type, id) in nodes {
            // Check if node center is in viewport
            if x < viewport_min.0 || x > viewport_max.0 || y < viewport_min.1 || y > viewport_max.1
            {
                continue;
            }

            // Check if node is large enough to be "visible" on screen
            let screen_width = w * zoom;
            let screen_height = h * zoom;
            if screen_width < min_visible_size && screen_height < min_visible_size {
                continue;
            }

            // Categorize by entity type
            match entity_type.to_uppercase().as_str() {
                "CBU" => cbus.push(id),
                "PERSON" | "PROPER_PERSON" => persons.push(id),
                _ => other.push(id),
            }
        }

        let total_count = (cbus.len() + persons.len() + other.len()) as u32;
        let viewport_width = viewport_max.0 - viewport_min.0;
        let viewport_height = viewport_max.1 - viewport_min.1;
        let viewport_area = viewport_width * viewport_height;

        let density = if viewport_area > 0.0 {
            total_count as f32 / viewport_area
        } else {
            0.0
        };

        Self {
            cbus,
            persons,
            other,
            total_count,
            density,
        }
    }

    /// Get count of visible CBUs
    pub fn cbu_count(&self) -> u32 {
        self.cbus.len() as u32
    }

    /// Get count of visible persons
    pub fn person_count(&self) -> u32 {
        self.persons.len() as u32
    }

    /// Get count for a specific entity type string
    pub fn count_for_type(&self, entity_type: &str) -> u32 {
        match entity_type.to_lowercase().as_str() {
            "visible_cbu" | "cbu" => self.cbus.len() as u32,
            "visible_person" | "person" => self.persons.len() as u32,
            "total" | "all" => self.total_count,
            _ => self.total_count,
        }
    }
}

// =============================================================================
// RULE EVALUATION
// =============================================================================

/// Evaluate density rules to determine which rule matches
///
/// Rules are evaluated in order; first match wins.
pub fn evaluate_density_rules<'a>(
    visible: &VisibleEntities,
    rules: &'a [DensityRule],
) -> Option<&'a DensityRule> {
    for rule in rules {
        let matches = match &rule.threshold {
            DensityThreshold::GreaterThan { gt, entity_type } => {
                let count = visible.count_for_type(entity_type);
                count > *gt
            }
            DensityThreshold::LessThan { lt, entity_type } => {
                let count = visible.count_for_type(entity_type);
                count < *lt
            }
            DensityThreshold::Range {
                min,
                max,
                entity_type,
            } => {
                let count = visible.count_for_type(entity_type);
                count >= *min && count <= *max
            }
            DensityThreshold::Single => visible.total_count == 1,
        };

        if matches {
            return Some(rule);
        }
    }

    None
}

// =============================================================================
// DEFAULT RULES
// =============================================================================

impl DensityRule {
    /// Get default density rules for typical usage
    pub fn defaults() -> Vec<DensityRule> {
        vec![
            // Many CBUs visible -> overview mode
            DensityRule {
                threshold: DensityThreshold::GreaterThan {
                    gt: 20,
                    entity_type: "visible_cbu".to_string(),
                },
                mode: ViewMode::AstroOverview,
                node_rendering: NodeRenderMode::CompactDot,
                expand_taxonomy: None,
                cluster_by: Some("jurisdiction".to_string()),
                show_floating_persons: false,
            },
            // 5-20 CBUs -> clustered view
            DensityRule {
                threshold: DensityThreshold::Range {
                    min: 5,
                    max: 20,
                    entity_type: "visible_cbu".to_string(),
                },
                mode: ViewMode::AstroClustered,
                node_rendering: NodeRenderMode::LabeledCircle,
                expand_taxonomy: None,
                cluster_by: Some("client".to_string()),
                show_floating_persons: false,
            },
            // 2-4 CBUs -> multi-CBU detail
            DensityRule {
                threshold: DensityThreshold::Range {
                    min: 2,
                    max: 4,
                    entity_type: "visible_cbu".to_string(),
                },
                mode: ViewMode::MultiCbuDetail,
                node_rendering: NodeRenderMode::ExpandedTaxonomy,
                expand_taxonomy: Some("ownership".to_string()),
                cluster_by: None,
                show_floating_persons: true,
            },
            // Single CBU -> full pyramid
            DensityRule {
                threshold: DensityThreshold::Single,
                mode: ViewMode::SingleCbuPyramid,
                node_rendering: NodeRenderMode::FullTaxonomyPyramid,
                expand_taxonomy: Some("all".to_string()),
                cluster_by: None,
                show_floating_persons: true,
            },
        ]
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_density_threshold_greater_than() {
        let threshold = DensityThreshold::GreaterThan {
            gt: 20,
            entity_type: "visible_cbu".to_string(),
        };

        assert!(!threshold.matches(20, "visible_cbu"));
        assert!(threshold.matches(21, "visible_cbu"));
        assert!(!threshold.matches(21, "person"));
    }

    #[test]
    fn test_density_threshold_less_than() {
        let threshold = DensityThreshold::LessThan {
            lt: 5,
            entity_type: "cbu".to_string(),
        };

        assert!(threshold.matches(4, "cbu"));
        assert!(!threshold.matches(5, "cbu"));
        assert!(!threshold.matches(6, "cbu"));
    }

    #[test]
    fn test_density_threshold_range() {
        let threshold = DensityThreshold::Range {
            min: 5,
            max: 20,
            entity_type: "visible_cbu".to_string(),
        };

        assert!(!threshold.matches(4, "visible_cbu"));
        assert!(threshold.matches(5, "visible_cbu"));
        assert!(threshold.matches(15, "visible_cbu"));
        assert!(threshold.matches(20, "visible_cbu"));
        assert!(!threshold.matches(21, "visible_cbu"));
    }

    #[test]
    fn test_density_threshold_single() {
        let threshold = DensityThreshold::Single;

        assert!(!threshold.matches(0, "anything"));
        assert!(threshold.matches(1, "anything"));
        assert!(!threshold.matches(2, "anything"));
    }

    #[test]
    fn test_visible_entities_count() {
        let mut visible = VisibleEntities::new();
        visible.cbus = vec![Uuid::now_v7(), Uuid::now_v7()];
        visible.persons = vec![Uuid::now_v7()];
        visible.total_count = 3;

        assert_eq!(visible.cbu_count(), 2);
        assert_eq!(visible.person_count(), 1);
        assert_eq!(visible.count_for_type("cbu"), 2);
        assert_eq!(visible.count_for_type("total"), 3);
    }

    #[test]
    fn test_evaluate_density_rules() {
        let rules = DensityRule::defaults();

        // Many CBUs -> overview
        let mut visible = VisibleEntities::new();
        visible.cbus = (0..25).map(|_| Uuid::now_v7()).collect();
        visible.total_count = 25;

        let matched = evaluate_density_rules(&visible, &rules);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().mode, ViewMode::AstroOverview);

        // Single CBU -> pyramid
        visible.cbus = vec![Uuid::now_v7()];
        visible.total_count = 1;

        let matched = evaluate_density_rules(&visible, &rules);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().mode, ViewMode::SingleCbuPyramid);
    }
}

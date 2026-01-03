//! Core taxonomy types - universal structures for any hierarchical view
//!
//! The key insight: shape determines metaphor, not the other way around.
//! A taxonomy with 1500 nodes at depth 2 IS a galaxy, regardless of what
//! we call it. The visualization derives from the data.

use serde::{Deserialize, Serialize};

// =============================================================================
// ASTRO LEVELS - Derived from node position and descendant count
// =============================================================================

/// Astronomical scale level - derived from tree characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstroLevel {
    /// Root with 1000s of descendants - the entire observable space
    Universe,
    /// Client/Book with 100s of descendants - a major cluster
    Galaxy,
    /// Cluster with 10s of descendants - a local group
    SolarSystem,
    /// CBU with entities - a single system
    Planet,
    /// Entity within a CBU - an orbiting body
    Moon,
    /// Detail (document, observation) - smallest observable
    Asteroid,
}

impl AstroLevel {
    /// Derive astro level from descendant count and depth
    pub fn from_characteristics(descendant_count: usize, depth: u32) -> Self {
        match (descendant_count, depth) {
            (n, 0) if n >= 500 => AstroLevel::Universe,
            (n, _) if n >= 100 => AstroLevel::Galaxy,
            (n, _) if n >= 10 => AstroLevel::SolarSystem,
            (n, _) if n >= 1 => AstroLevel::Planet,
            (0, d) if d <= 1 => AstroLevel::Moon,
            _ => AstroLevel::Asteroid,
        }
    }

    /// Zoom level multiplier for this astro level
    pub fn zoom_factor(&self) -> f32 {
        match self {
            AstroLevel::Universe => 0.1,
            AstroLevel::Galaxy => 0.3,
            AstroLevel::SolarSystem => 0.6,
            AstroLevel::Planet => 1.0,
            AstroLevel::Moon => 1.5,
            AstroLevel::Asteroid => 2.0,
        }
    }
}

// =============================================================================
// METAPHORS - Derived from tree shape
// =============================================================================

/// Visual metaphor - derived from tree shape, not prescribed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metaphor {
    /// Massive (500+) - semantic clustering with orbits
    Galaxy,
    /// Large (50-500) - grouped stars in patterns
    Constellation,
    /// Deep (5+ levels) - ownership/control chain rising to apex
    Pyramid,
    /// Wide (10+ at any level) - force-directed layout
    Network,
    /// Default - simple hierarchical tree
    Tree,
}

impl Metaphor {
    /// Derive metaphor from tree shape
    pub fn from_shape(max_depth: u32, max_width: usize, total_count: usize) -> Self {
        if total_count >= 500 {
            Metaphor::Galaxy
        } else if total_count >= 50 {
            Metaphor::Constellation
        } else if max_depth >= 5 {
            Metaphor::Pyramid
        } else if max_width >= 10 {
            Metaphor::Network
        } else {
            Metaphor::Tree
        }
    }

    /// Layout algorithm to use for this metaphor
    pub fn layout_algorithm(&self) -> &'static str {
        match self {
            Metaphor::Galaxy => "semantic_cluster",
            Metaphor::Constellation => "grouped_radial",
            Metaphor::Pyramid => "hierarchical_up",
            Metaphor::Network => "force_directed",
            Metaphor::Tree => "hierarchical_down",
        }
    }
}

// =============================================================================
// NODE TYPES
// =============================================================================

/// Type of node in the taxonomy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Root of taxonomy (virtual)
    Root,
    /// Commercial client grouping
    Client,
    /// Grouping cluster (jurisdiction, ManCo, fund type)
    Cluster,
    /// Client Business Unit
    Cbu,
    /// Entity (company, person, trust, etc.)
    Entity,
    /// Role/position holder
    Position,
    /// Document
    Document,
    /// Observation/evidence
    Observation,
    /// Product subscription
    Product,
    /// Service instance
    Service,
}

impl NodeType {
    /// Icon identifier for this node type
    pub fn icon(&self) -> &'static str {
        match self {
            NodeType::Root => "universe",
            NodeType::Client => "building",
            NodeType::Cluster => "folder",
            NodeType::Cbu => "star",
            NodeType::Entity => "user",
            NodeType::Position => "badge",
            NodeType::Document => "file",
            NodeType::Observation => "eye",
            NodeType::Product => "package",
            NodeType::Service => "cog",
        }
    }

    /// Can this node type have children?
    pub fn can_have_children(&self) -> bool {
        matches!(
            self,
            NodeType::Root
                | NodeType::Client
                | NodeType::Cluster
                | NodeType::Cbu
                | NodeType::Entity
        )
    }
}

// =============================================================================
// DIMENSION VALUES - For filtering and coloring
// =============================================================================

/// Dimension values for a node - used for filtering, grouping, coloring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DimensionValues {
    /// Jurisdiction code (e.g., "LU", "IE", "US")
    pub jurisdiction: Option<String>,
    /// Fund type (UCITS, AIF, HEDGE_FUND, etc.)
    pub fund_type: Option<String>,
    /// Status indicator (RED, AMBER, GREEN)
    pub status: Option<Status>,
    /// AUM in millions (for size-based rendering)
    pub aum_millions: Option<f64>,
    /// KYC completion percentage (0-100)
    pub kyc_completion: Option<u8>,
    /// Entity type code
    pub entity_type: Option<String>,
    /// Role category
    pub role_category: Option<String>,
    /// Client type
    pub client_type: Option<String>,
    /// Product codes subscribed
    pub products: Option<Vec<String>>,
}

impl DimensionValues {
    /// Get a dimension value by field name (for dynamic grouping)
    pub fn get(&self, field: &str) -> Option<&String> {
        match field {
            "jurisdiction" => self.jurisdiction.as_ref(),
            "fund_type" => self.fund_type.as_ref(),
            "entity_type" => self.entity_type.as_ref(),
            "role_category" => self.role_category.as_ref(),
            "client_type" => self.client_type.as_ref(),
            _ => None,
        }
    }

    /// Set a dimension value by field name (for dynamic construction)
    pub fn set(&mut self, field: &str, value: impl Into<String>) {
        let v = value.into();
        match field {
            "jurisdiction" => self.jurisdiction = Some(v),
            "fund_type" => self.fund_type = Some(v),
            "entity_type" => self.entity_type = Some(v),
            "role_category" => self.role_category = Some(v),
            "client_type" => self.client_type = Some(v),
            _ => {}
        }
    }
}

/// Traffic light status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Red,
    Amber,
    Green,
}

impl Status {
    /// Parse status from string (case-insensitive)
    #[allow(dead_code)]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "RED" => Some(Status::Red),
            "AMBER" | "YELLOW" => Some(Status::Amber),
            "GREEN" => Some(Status::Green),
            _ => None,
        }
    }

    pub fn color_rgb(&self) -> (u8, u8, u8) {
        match self {
            Status::Red => (220, 53, 69),
            Status::Amber => (255, 193, 7),
            Status::Green => (40, 167, 69),
        }
    }
}

// =============================================================================
// ENTITY SUMMARY - Lazy-loaded detail
// =============================================================================

/// Summary of entity data for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySummary {
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub status: Option<String>,
    pub external_id: Option<String>,
}

// =============================================================================
// FILTERS
// =============================================================================

/// Filter predicate for refining selections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Filter {
    /// Filter by jurisdiction(s)
    Jurisdiction(Vec<String>),
    /// Filter by fund type(s)
    FundType(Vec<String>),
    /// Filter by status
    Status(Vec<Status>),
    /// Filter by AUM range (min, max in millions)
    AumRange(Option<f64>, Option<f64>),
    /// Filter by KYC completion range
    KycCompletion(Option<u8>, Option<u8>),
    /// Filter by entity type(s)
    EntityType(Vec<String>),
    /// Filter by client type(s)
    ClientType(Vec<String>),
    /// Filter by product subscription
    HasProduct(String),
    /// Filter by missing product
    MissingProduct(String),
    /// Needs attention (has red flags, pending items)
    NeedsAttention,
    /// Custom predicate (evaluated by name)
    Custom(String),
    /// Compound AND
    And(Vec<Filter>),
    /// Compound OR
    Or(Vec<Filter>),
    /// Negation
    Not(Box<Filter>),
}

impl Filter {
    /// Check if a node matches this filter
    pub fn matches(&self, dimensions: &DimensionValues) -> bool {
        match self {
            Filter::Jurisdiction(codes) => dimensions
                .jurisdiction
                .as_ref()
                .map(|j| codes.contains(j))
                .unwrap_or(false),

            Filter::FundType(types) => dimensions
                .fund_type
                .as_ref()
                .map(|t| types.contains(t))
                .unwrap_or(false),

            Filter::Status(statuses) => dimensions
                .status
                .map(|s| statuses.contains(&s))
                .unwrap_or(false),

            Filter::AumRange(min, max) => {
                if let Some(aum) = dimensions.aum_millions {
                    let above_min = min.map(|m| aum >= m).unwrap_or(true);
                    let below_max = max.map(|m| aum <= m).unwrap_or(true);
                    above_min && below_max
                } else {
                    false
                }
            }

            Filter::KycCompletion(min, max) => {
                if let Some(kyc) = dimensions.kyc_completion {
                    let above_min = min.map(|m| kyc >= m).unwrap_or(true);
                    let below_max = max.map(|m| kyc <= m).unwrap_or(true);
                    above_min && below_max
                } else {
                    false
                }
            }

            Filter::EntityType(types) => dimensions
                .entity_type
                .as_ref()
                .map(|t| types.contains(t))
                .unwrap_or(false),

            Filter::ClientType(types) => dimensions
                .client_type
                .as_ref()
                .map(|t| types.contains(t))
                .unwrap_or(false),

            Filter::HasProduct(product) => dimensions
                .products
                .as_ref()
                .map(|p| p.contains(product))
                .unwrap_or(false),

            Filter::MissingProduct(product) => dimensions
                .products
                .as_ref()
                .map(|p| !p.contains(product))
                .unwrap_or(true),

            Filter::NeedsAttention => {
                // Red status or low KYC completion
                dimensions.status == Some(Status::Red)
                    || dimensions.kyc_completion.map(|k| k < 50).unwrap_or(false)
            }

            Filter::Custom(_name) => {
                // Would be evaluated by external predicate
                true
            }

            Filter::And(filters) => filters.iter().all(|f| f.matches(dimensions)),
            Filter::Or(filters) => filters.iter().any(|f| f.matches(dimensions)),
            Filter::Not(filter) => !filter.matches(dimensions),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_astro_level_derivation() {
        assert_eq!(
            AstroLevel::from_characteristics(1000, 0),
            AstroLevel::Universe
        );
        assert_eq!(AstroLevel::from_characteristics(200, 1), AstroLevel::Galaxy);
        assert_eq!(
            AstroLevel::from_characteristics(25, 2),
            AstroLevel::SolarSystem
        );
        assert_eq!(AstroLevel::from_characteristics(5, 3), AstroLevel::Planet);
        assert_eq!(AstroLevel::from_characteristics(0, 1), AstroLevel::Moon);
    }

    #[test]
    fn test_metaphor_derivation() {
        // Large flat -> Galaxy
        assert_eq!(Metaphor::from_shape(2, 50, 600), Metaphor::Galaxy);
        // Medium -> Constellation
        assert_eq!(Metaphor::from_shape(3, 20, 100), Metaphor::Constellation);
        // Deep -> Pyramid
        assert_eq!(Metaphor::from_shape(7, 3, 30), Metaphor::Pyramid);
        // Wide -> Network
        assert_eq!(Metaphor::from_shape(2, 15, 30), Metaphor::Network);
        // Default -> Tree
        assert_eq!(Metaphor::from_shape(3, 5, 20), Metaphor::Tree);
    }

    #[test]
    fn test_filter_matching() {
        let dims = DimensionValues {
            jurisdiction: Some("LU".into()),
            fund_type: Some("UCITS".into()),
            status: Some(Status::Green),
            aum_millions: Some(500.0),
            kyc_completion: Some(85),
            ..Default::default()
        };

        assert!(Filter::Jurisdiction(vec!["LU".into()]).matches(&dims));
        assert!(!Filter::Jurisdiction(vec!["IE".into()]).matches(&dims));
        assert!(Filter::AumRange(Some(100.0), Some(1000.0)).matches(&dims));
        assert!(!Filter::AumRange(Some(600.0), None).matches(&dims));
        assert!(Filter::And(vec![
            Filter::Jurisdiction(vec!["LU".into()]),
            Filter::Status(vec![Status::Green]),
        ])
        .matches(&dims));
    }
}

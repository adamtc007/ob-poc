//! Hierarchical visualization types for CBU views
//!
//! Supports two view modes:
//! - ServiceDelivery: CBU → Product → Service → Resource → SSI/BookingRule
//! - KycUbo: CommercialClient → ManCo → Fund → ShareClasses + Officers

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// View mode selection - two distinct views, not layers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// Service delivery map: What does BNY provide to this client?
    ServiceDelivery,
    /// KYC/UBO structure: Who is this client? Who owns/controls it?
    #[default]
    KycUbo,
}

/// Node in the visualization tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: Uuid,
    pub node_type: TreeNodeType,
    pub label: String,
    pub sublabel: Option<String>,
    pub jurisdiction: Option<String>,
    pub children: Vec<TreeNode>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Types of nodes in the tree
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeNodeType {
    // KYC/UBO View
    Cbu,
    CommercialClient,
    ManCo,
    FundEntity,
    TrustEntity,
    Person,
    ShareClass,

    // Service Delivery View
    Product,
    Service,
    Resource,
    Ssi,
    BookingRule,
}

/// Edge connecting nodes (for non-hierarchical relationships like ownership)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEdge {
    pub from: Uuid,
    pub to: Uuid,
    pub edge_type: TreeEdgeType,
    pub label: Option<String>,
    pub weight: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeEdgeType {
    /// Structural (part of tree hierarchy)
    ChildOf,
    /// Overlay - ownership relationship
    Owns,
    /// Overlay - control relationship (non-ownership)
    Controls,
    /// Overlay - role assignment
    Role,
}

/// Complete visualization data for a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuVisualization {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub view_mode: ViewMode,
    pub root: TreeNode,
    /// Non-hierarchical edges drawn on top of tree (ownership, control)
    pub overlay_edges: Vec<TreeEdge>,
    pub stats: VisualizationStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisualizationStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub max_depth: usize,
    pub entity_count: usize,
    pub person_count: usize,
    pub share_class_count: usize,
}

impl VisualizationStats {
    /// Calculate stats from a tree
    pub fn from_tree(root: &TreeNode, edges: &[TreeEdge]) -> Self {
        let mut stats = Self {
            total_edges: edges.len(),
            ..Default::default()
        };
        Self::count_nodes(root, &mut stats, 0);
        stats
    }

    fn count_nodes(node: &TreeNode, stats: &mut Self, depth: usize) {
        stats.total_nodes += 1;
        stats.max_depth = stats.max_depth.max(depth);

        match node.node_type {
            TreeNodeType::Person => stats.person_count += 1,
            TreeNodeType::ShareClass => stats.share_class_count += 1,
            TreeNodeType::CommercialClient
            | TreeNodeType::ManCo
            | TreeNodeType::FundEntity
            | TreeNodeType::TrustEntity => stats.entity_count += 1,
            _ => {}
        }

        for child in &node.children {
            Self::count_nodes(child, stats, depth + 1);
        }
    }
}

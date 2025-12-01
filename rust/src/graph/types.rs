//! Graph types for CBU visualization
//!
//! These types define the intermediate representation for graph data
//! that is serialized to JSON and consumed by the egui WASM client.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Graph projection of a CBU for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuGraph {
    pub cbu_id: Uuid,
    pub label: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub layers: Vec<LayerInfo>,
    pub stats: GraphStats,
}

/// A node in the graph representing an entity, document, or resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub layer: LayerType,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: NodeStatus,
    pub data: serde_json::Value,
    /// Parent node ID for hierarchical grouping (e.g., market groups custody items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

/// Types of nodes in the graph
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    // Core
    Cbu,

    // Custody
    Market, // Grouping node for market
    Universe,
    Ssi,
    BookingRule,
    Isda,
    Csa,
    Subcustodian,

    // KYC
    Document,
    Attribute,
    Verification,

    // UBO
    Entity,
    OwnershipLink,

    // Services
    Product,
    Service,
    Resource,
}

/// Layer categories for organizing nodes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    Core,
    Custody,
    Kyc,
    Ubo,
    Services,
}

/// Status of a node
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Active,
    Pending,
    Suspended,
    Expired,
    Draft,
}

/// An edge connecting two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

/// Types of edges representing relationships
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    // Core
    HasRole,

    // Custody
    RoutesTo,
    Matches,
    CoveredBy,
    SecuredBy,
    SettlesAt,

    // KYC
    Requires,
    Validates,

    // UBO
    Owns,
    Controls,

    // Services
    Delivers,
    BelongsTo,
}

/// Information about a layer for UI rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    pub layer_type: LayerType,
    pub label: String,
    pub color: String,
    pub node_count: usize,
    pub visible: bool,
}

/// Statistics about the graph
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_layer: HashMap<String, usize>,
    pub nodes_by_type: HashMap<String, usize>,
}

impl CbuGraph {
    /// Create a new empty graph for a CBU
    pub fn new(cbu_id: Uuid, label: String) -> Self {
        Self {
            cbu_id,
            label,
            nodes: Vec::new(),
            edges: Vec::new(),
            layers: Vec::new(),
            stats: GraphStats::default(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.push(node);
    }

    /// Check if a node with the given ID exists
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id)
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }

    /// Compute statistics for the graph
    pub fn compute_stats(&mut self) {
        self.stats.total_nodes = self.nodes.len();
        self.stats.total_edges = self.edges.len();

        self.stats.nodes_by_layer.clear();
        self.stats.nodes_by_type.clear();

        for node in &self.nodes {
            let layer_key = format!("{:?}", node.layer).to_lowercase();
            let type_key = format!("{:?}", node.node_type).to_lowercase();

            *self.stats.nodes_by_layer.entry(layer_key).or_insert(0) += 1;
            *self.stats.nodes_by_type.entry(type_key).or_insert(0) += 1;
        }
    }

    /// Build layer information for UI rendering
    pub fn build_layer_info(&mut self) {
        self.layers = vec![
            LayerInfo {
                layer_type: LayerType::Core,
                label: "Core".to_string(),
                color: "#6B7280".to_string(), // Gray
                node_count: self.stats.nodes_by_layer.get("core").copied().unwrap_or(0),
                visible: true,
            },
            LayerInfo {
                layer_type: LayerType::Custody,
                label: "Custody".to_string(),
                color: "#3B82F6".to_string(), // Blue
                node_count: self
                    .stats
                    .nodes_by_layer
                    .get("custody")
                    .copied()
                    .unwrap_or(0),
                visible: true,
            },
            LayerInfo {
                layer_type: LayerType::Kyc,
                label: "KYC".to_string(),
                color: "#8B5CF6".to_string(), // Purple
                node_count: self.stats.nodes_by_layer.get("kyc").copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Ubo,
                label: "UBO".to_string(),
                color: "#10B981".to_string(), // Green
                node_count: self.stats.nodes_by_layer.get("ubo").copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Services,
                label: "Services".to_string(),
                color: "#F59E0B".to_string(), // Amber
                node_count: self
                    .stats
                    .nodes_by_layer
                    .get("services")
                    .copied()
                    .unwrap_or(0),
                visible: false,
            },
        ];
    }
}

/// Summary of a CBU for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

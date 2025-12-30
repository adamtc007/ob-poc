//! Graph View Model for DSL query results
//!
//! This module defines the GraphViewModel struct which is the output
//! of graph.* DSL verbs. It provides a UI-ready representation of
//! graph query results.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Use legacy types for backward compatibility during transition to EntityGraph
use super::types::{EdgeType, GraphEdge, LayerType, LegacyGraphNode, NodeType};

// Re-export as GraphNode for this module
type GraphNode = LegacyGraphNode;

// =============================================================================
// GRAPH VIEW MODEL - Output of graph.* verbs
// =============================================================================

/// GraphViewModel is the output of graph.* DSL verbs.
///
/// It provides a UI-ready representation of graph query results,
/// including nodes, edges, layout hints, and query metadata.
///
/// ## Design Principles (from EGUI-RULES.md)
///
/// 1. **Server is source of truth** - All data comes from server, UI just renders
/// 2. **No callbacks** - Actions return values, UI processes them
/// 3. **Stateless rendering** - Each frame renders from this model fresh
///
/// ## Usage Flow
///
/// ```text
/// DSL: (graph.view :cbu-id @fund :view-mode "kyc_ubo")
///                    │
///                    ▼
/// GraphQueryExecutor.execute()
///                    │
///                    ▼
/// GraphViewModel (this struct) ──► JSON ──► egui UI
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphViewModel {
    // =========================================================================
    // IDENTITY
    // =========================================================================
    /// Query ID for caching/deduplication
    pub query_id: String,

    /// Timestamp when the query was executed
    pub timestamp: chrono::DateTime<chrono::Utc>,

    // =========================================================================
    // GRAPH DATA
    // =========================================================================
    /// Root node ID (CBU or focus entity)
    pub root_id: String,

    /// All nodes in the result set
    pub nodes: Vec<GraphNode>,

    /// All edges in the result set
    pub edges: Vec<GraphEdge>,

    // =========================================================================
    // QUERY CONTEXT
    // =========================================================================
    /// The view mode used for this query
    pub view_mode: ViewModeInfo,

    /// Filter criteria applied (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<GraphFilter>,

    /// Focus node ID (for focus queries)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_id: Option<String>,

    /// Depth of traversal from root
    pub depth: u32,

    // =========================================================================
    // GROUPING & ORGANIZATION
    // =========================================================================
    /// Nodes grouped by layer
    #[serde(default)]
    pub nodes_by_layer: HashMap<LayerType, Vec<String>>,

    /// Nodes grouped by type
    #[serde(default)]
    pub nodes_by_type: HashMap<NodeType, Vec<String>>,

    /// Custom grouping (from group-by queries)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<NodeGroup>>,

    // =========================================================================
    // PATHS (for path queries)
    // =========================================================================
    /// Paths found (for path queries)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<GraphPath>,

    // =========================================================================
    // STATISTICS
    // =========================================================================
    /// Query statistics
    pub stats: GraphViewStats,

    // =========================================================================
    // LAYOUT HINTS
    // =========================================================================
    /// Suggested layout orientation
    #[serde(default)]
    pub orientation: LayoutOrientation,

    /// Canvas bounds (computed by layout engine)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<CanvasBounds>,
}

impl GraphViewModel {
    /// Create a new empty GraphViewModel
    pub fn new(root_id: String) -> Self {
        Self {
            query_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            root_id,
            nodes: Vec::new(),
            edges: Vec::new(),
            view_mode: ViewModeInfo::default(),
            filter: None,
            focus_id: None,
            depth: 0,
            nodes_by_layer: HashMap::new(),
            nodes_by_type: HashMap::new(),
            groups: None,
            paths: Vec::new(),
            stats: GraphViewStats::default(),
            orientation: LayoutOrientation::default(),
            bounds: None,
        }
    }

    /// Add a node to the view model
    pub fn add_node(&mut self, node: GraphNode) {
        // Update groupings
        self.nodes_by_layer
            .entry(node.layer)
            .or_default()
            .push(node.id.clone());
        self.nodes_by_type
            .entry(node.node_type)
            .or_default()
            .push(node.id.clone());

        self.nodes.push(node);
    }

    /// Add an edge to the view model
    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }

    /// Compute statistics from current nodes/edges
    pub fn compute_stats(&mut self) {
        self.stats.node_count = self.nodes.len();
        self.stats.edge_count = self.edges.len();

        // Count by layer
        self.stats.nodes_by_layer.clear();
        for (layer, nodes) in &self.nodes_by_layer {
            self.stats
                .nodes_by_layer
                .insert(format!("{:?}", layer).to_lowercase(), nodes.len());
        }

        // Count by type
        self.stats.nodes_by_type.clear();
        for (node_type, nodes) in &self.nodes_by_type {
            self.stats
                .nodes_by_type
                .insert(format!("{:?}", node_type).to_lowercase(), nodes.len());
        }

        // Count edge types
        self.stats.edges_by_type.clear();
        for edge in &self.edges {
            *self
                .stats
                .edges_by_type
                .entry(format!("{:?}", edge.edge_type).to_lowercase())
                .or_insert(0) += 1;
        }
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get all nodes of a specific type
    pub fn nodes_of_type(&self, node_type: NodeType) -> Vec<&GraphNode> {
        self.nodes
            .iter()
            .filter(|n| n.node_type == node_type)
            .collect()
    }

    /// Get all edges of a specific type
    pub fn edges_of_type(&self, edge_type: EdgeType) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.edge_type == edge_type)
            .collect()
    }

    /// Check if a node exists
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id)
    }

    /// Get nodes connected to a given node
    pub fn connected_nodes(&self, node_id: &str) -> Vec<&GraphNode> {
        let connected_ids: std::collections::HashSet<_> = self
            .edges
            .iter()
            .filter_map(|e| {
                if e.source == node_id {
                    Some(e.target.as_str())
                } else if e.target == node_id {
                    Some(e.source.as_str())
                } else {
                    None
                }
            })
            .collect();

        self.nodes
            .iter()
            .filter(|n| connected_ids.contains(n.id.as_str()))
            .collect()
    }
}

// =============================================================================
// VIEW MODE INFO
// =============================================================================

/// Information about the view mode used for a query
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ViewModeInfo {
    /// View mode name (KYC_UBO, SERVICE_DELIVERY, CUSTODY, PRODUCTS_ONLY)
    pub name: String,

    /// Layers included in this view
    #[serde(default)]
    pub layers: Vec<LayerType>,

    /// Edge types included in this view
    #[serde(default)]
    pub edge_types: Vec<EdgeType>,

    /// Description for UI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// =============================================================================
// GRAPH FILTER
// =============================================================================

/// Filter criteria for graph queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphFilter {
    /// Filter by node types
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_types: Vec<NodeType>,

    /// Filter by edge types
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edge_types: Vec<EdgeType>,

    /// Filter by layers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LayerType>,

    /// Filter by node status
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// Filter by role
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Filter by jurisdiction
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,

    /// Text search filter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,

    /// Custom attribute filters
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub attributes: HashMap<String, String>,
}

// =============================================================================
// NODE GROUP
// =============================================================================

/// A group of nodes (from group-by queries)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGroup {
    /// Group key (e.g., jurisdiction code, role name)
    pub key: String,

    /// Display label for the group
    pub label: String,

    /// Node IDs in this group
    pub node_ids: Vec<String>,

    /// Group color for rendering
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

// =============================================================================
// GRAPH PATH
// =============================================================================

/// A path through the graph (from path queries)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    /// Path ID
    pub id: String,

    /// Ordered list of node IDs in the path
    pub node_ids: Vec<String>,

    /// Ordered list of edge IDs connecting the nodes
    pub edge_ids: Vec<String>,

    /// Total path length (number of edges)
    pub length: usize,

    /// Path weight (sum of edge weights, if applicable)
    #[serde(default)]
    pub weight: f64,

    /// Path type (ownership, control, service, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_type: Option<String>,

    /// Aggregated percentage along path (for ownership chains)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate_percentage: Option<f64>,
}

impl GraphPath {
    /// Create a new path
    pub fn new(id: String, node_ids: Vec<String>, edge_ids: Vec<String>) -> Self {
        let length = edge_ids.len();
        Self {
            id,
            node_ids,
            edge_ids,
            length,
            weight: length as f64,
            path_type: None,
            aggregate_percentage: None,
        }
    }
}

// =============================================================================
// GRAPH VIEW STATS
// =============================================================================

/// Statistics about the graph view
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphViewStats {
    /// Total number of nodes
    pub node_count: usize,

    /// Total number of edges
    pub edge_count: usize,

    /// Nodes by layer
    #[serde(default)]
    pub nodes_by_layer: HashMap<String, usize>,

    /// Nodes by type
    #[serde(default)]
    pub nodes_by_type: HashMap<String, usize>,

    /// Edges by type
    #[serde(default)]
    pub edges_by_type: HashMap<String, usize>,

    /// Maximum depth from root
    #[serde(default)]
    pub max_depth: u32,

    /// Number of paths found (for path queries)
    #[serde(default)]
    pub path_count: usize,

    /// Query execution time in milliseconds
    #[serde(default)]
    pub execution_time_ms: u64,
}

// =============================================================================
// LAYOUT ORIENTATION
// =============================================================================

/// Layout orientation hint for the UI
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LayoutOrientation {
    /// Top-to-bottom (default for ownership/UBO)
    #[default]
    Vertical,
    /// Left-to-right (alternative layout)
    Horizontal,
}

// =============================================================================
// CANVAS BOUNDS
// =============================================================================

/// Canvas bounds for the graph layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl CanvasBounds {
    /// Calculate width of bounds
    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    /// Calculate height of bounds
    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }

    /// Get center point
    pub fn center(&self) -> (f32, f32) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }
}

// =============================================================================
// COMPARISON RESULT (for compare queries)
// =============================================================================

/// Result of comparing two graph states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphComparison {
    /// Query ID
    pub query_id: String,

    /// Timestamp of comparison
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Left side (before) snapshot ID or timestamp
    pub left_id: String,

    /// Right side (after) snapshot ID or timestamp
    pub right_id: String,

    /// Nodes added (in right but not left)
    pub nodes_added: Vec<String>,

    /// Nodes removed (in left but not right)
    pub nodes_removed: Vec<String>,

    /// Nodes with changed properties
    pub nodes_changed: Vec<NodeChange>,

    /// Edges added
    pub edges_added: Vec<String>,

    /// Edges removed
    pub edges_removed: Vec<String>,

    /// Summary statistics
    pub summary: ComparisonSummary,
}

/// A changed node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeChange {
    /// Node ID
    pub node_id: String,

    /// Changed fields with before/after values
    pub changes: Vec<FieldChange>,
}

/// A changed field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    /// Field name
    pub field: String,

    /// Previous value
    pub before: serde_json::Value,

    /// New value
    pub after: serde_json::Value,
}

/// Summary of comparison
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComparisonSummary {
    pub nodes_added_count: usize,
    pub nodes_removed_count: usize,
    pub nodes_changed_count: usize,
    pub edges_added_count: usize,
    pub edges_removed_count: usize,
    pub has_structural_changes: bool,
    pub has_ownership_changes: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_view_model_new() {
        let model = GraphViewModel::new("cbu-123".to_string());
        assert_eq!(model.root_id, "cbu-123");
        assert!(model.nodes.is_empty());
        assert!(model.edges.is_empty());
    }

    #[test]
    fn test_add_node_updates_groupings() {
        let mut model = GraphViewModel::new("root".to_string());

        let node = GraphNode {
            id: "entity-1".to_string(),
            node_type: NodeType::Entity,
            layer: LayerType::Core,
            label: "Test Entity".to_string(),
            ..Default::default()
        };

        model.add_node(node);

        assert_eq!(model.nodes.len(), 1);
        assert!(model.nodes_by_layer.contains_key(&LayerType::Core));
        assert!(model.nodes_by_type.contains_key(&NodeType::Entity));
    }

    #[test]
    fn test_compute_stats() {
        let mut model = GraphViewModel::new("root".to_string());

        model.add_node(GraphNode {
            id: "n1".to_string(),
            node_type: NodeType::Entity,
            layer: LayerType::Core,
            label: "Entity 1".to_string(),
            ..Default::default()
        });

        model.add_node(GraphNode {
            id: "n2".to_string(),
            node_type: NodeType::Entity,
            layer: LayerType::Ubo,
            label: "Entity 2".to_string(),
            ..Default::default()
        });

        model.add_edge(GraphEdge {
            id: "e1".to_string(),
            source: "n1".to_string(),
            target: "n2".to_string(),
            edge_type: EdgeType::Owns,
            label: Some("100%".to_string()),
        });

        model.compute_stats();

        assert_eq!(model.stats.node_count, 2);
        assert_eq!(model.stats.edge_count, 1);
    }

    #[test]
    fn test_connected_nodes() {
        let mut model = GraphViewModel::new("root".to_string());

        model.add_node(GraphNode {
            id: "a".to_string(),
            ..Default::default()
        });
        model.add_node(GraphNode {
            id: "b".to_string(),
            ..Default::default()
        });
        model.add_node(GraphNode {
            id: "c".to_string(),
            ..Default::default()
        });

        model.add_edge(GraphEdge {
            id: "e1".to_string(),
            source: "a".to_string(),
            target: "b".to_string(),
            edge_type: EdgeType::Owns,
            label: None,
        });

        let connected = model.connected_nodes("a");
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].id, "b");
    }

    #[test]
    fn test_graph_path() {
        let path = GraphPath::new(
            "path-1".to_string(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["e1".to_string(), "e2".to_string()],
        );

        assert_eq!(path.length, 2);
        assert_eq!(path.node_ids.len(), 3);
    }
}

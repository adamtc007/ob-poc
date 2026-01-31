//! Standard Graph Types
//!
//! Industry-standard graph data structures for API interoperability.
//! These types align with common formats (D3.js, Cytoscape, vis.js)
//! while providing conversion to/from the optimized SoA wire format.
//!
//! # Design Rationale
//!
//! The ESPER system uses two representations:
//!
//! 1. **API Layer** (this module): Standard `Node`/`Edge`/`Graph` structs
//!    for import/export, debugging, and interop with external tools.
//!
//! 2. **Wire Layer** (`ChamberSnapshot`): Structure-of-Arrays (SoA) format
//!    optimized for cache-friendly iteration and compact serialization.
//!
//! # Usage
//!
//! ```rust
//! use esper_snapshot::graph::{Graph, Node, Edge, EdgeKind};
//!
//! // Build graph with standard API
//! let graph = Graph::new()
//!     .with_node(Node::new(1, "Root").with_kind(0).with_position(0.0, 0.0))
//!     .with_node(Node::new(2, "Child A").with_kind(1).with_position(10.0, 20.0))
//!     .with_edge(Edge::parent(2, 1));
//!
//! // Convert to wire format for transmission
//! let mut string_table = Vec::new();
//! let chamber = graph.to_chamber(1, &mut string_table);
//! assert_eq!(chamber.entity_count(), 2);
//! ```

use crate::{ChamberSnapshot, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// STANDARD NODE
// =============================================================================

/// A graph node (entity) in standard API format.
///
/// Aligns with common graph visualization formats:
/// - D3.js: `{id, group, ...}`
/// - Cytoscape: `{data: {id, ...}, position: {x, y}}`
/// - vis.js: `{id, label, group, x, y, ...}`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Unique node identifier.
    pub id: u64,

    /// Display label.
    pub label: String,

    /// Node kind/type (maps to visual styling).
    #[serde(default)]
    pub kind: u16,

    /// Position in 2D space (optional, computed by layout if absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Vec2>,

    /// Detail reference for drill-down (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_ref: Option<u64>,

    /// Custom properties (for domain-specific data).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, serde_json::Value>,
}

impl Node {
    /// Create a new node with ID and label.
    pub fn new(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            kind: 0,
            position: None,
            detail_ref: None,
            properties: HashMap::new(),
        }
    }

    /// Set node kind.
    pub fn with_kind(mut self, kind: u16) -> Self {
        self.kind = kind;
        self
    }

    /// Set position.
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = Some(Vec2::new(x, y));
        self
    }

    /// Set detail reference.
    pub fn with_detail_ref(mut self, detail_ref: u64) -> Self {
        self.detail_ref = Some(detail_ref);
        self
    }

    /// Add a custom property.
    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

// =============================================================================
// STANDARD EDGE
// =============================================================================

/// An edge connecting two nodes.
///
/// Aligns with common graph formats:
/// - D3.js: `{source, target, ...}`
/// - Cytoscape: `{data: {source, target, ...}}`
/// - vis.js: `{from, to, ...}`
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Source node ID.
    pub source: u64,

    /// Target node ID.
    pub target: u64,

    /// Edge type/relationship kind.
    #[serde(default)]
    pub kind: EdgeKind,

    /// Edge weight (optional, for weighted graphs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
}

impl Edge {
    /// Create a new edge.
    pub fn new(source: u64, target: u64, kind: EdgeKind) -> Self {
        Self {
            source,
            target,
            kind,
            weight: None,
        }
    }

    /// Create a parent-child edge (child → parent).
    pub fn parent(child: u64, parent: u64) -> Self {
        Self::new(child, parent, EdgeKind::Parent)
    }

    /// Create a sibling edge.
    pub fn sibling(a: u64, b: u64) -> Self {
        Self::new(a, b, EdgeKind::Sibling)
    }

    /// Create a control/ownership edge.
    pub fn control(controller: u64, controlled: u64) -> Self {
        Self::new(controller, controlled, EdgeKind::Control)
    }

    /// Create a reference/link edge.
    pub fn reference(from: u64, to: u64) -> Self {
        Self::new(from, to, EdgeKind::Reference)
    }

    /// Create a door edge (cross-chamber link).
    pub fn door(from: u64, to: u64) -> Self {
        Self::new(from, to, EdgeKind::Door)
    }

    /// Set edge weight.
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Check if this edge involves a node.
    pub fn involves(&self, node_id: u64) -> bool {
        self.source == node_id || self.target == node_id
    }
}

// =============================================================================
// EDGE KIND
// =============================================================================

/// Edge relationship types.
///
/// These map to visual styling and navigation behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// Hierarchical parent-child (child points to parent).
    #[default]
    Parent,

    /// Sibling relationship (same hierarchical level).
    Sibling,

    /// Control/ownership relationship.
    Control,

    /// Reference/association (non-hierarchical link).
    Reference,

    /// Cross-chamber navigation link.
    Door,
}

impl EdgeKind {
    /// Is this a hierarchical edge?
    pub fn is_hierarchical(&self) -> bool {
        matches!(self, EdgeKind::Parent | EdgeKind::Sibling)
    }

    /// Is this a navigational edge?
    pub fn is_navigational(&self) -> bool {
        matches!(self, EdgeKind::Door)
    }
}

// =============================================================================
// STANDARD GRAPH
// =============================================================================

/// A complete graph with nodes and edges.
///
/// This is the standard API format for graph data. It can be:
/// - Serialized to JSON for external tools
/// - Converted to `ChamberSnapshot` for wire transmission
/// - Built incrementally with builder methods
///
/// # Example
///
/// ```rust
/// use esper_snapshot::graph::{Graph, Node, Edge};
///
/// let graph = Graph::new()
///     .with_node(Node::new(1, "CEO"))
///     .with_node(Node::new(2, "CTO"))
///     .with_node(Node::new(3, "CFO"))
///     .with_edge(Edge::parent(2, 1))
///     .with_edge(Edge::parent(3, 1));
///
/// assert_eq!(graph.node_count(), 3);
/// assert_eq!(graph.edge_count(), 2);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    /// All nodes in the graph.
    pub nodes: Vec<Node>,

    /// All edges in the graph.
    pub edges: Vec<Edge>,

    /// Graph-level metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node.
    pub fn with_node(mut self, node: Node) -> Self {
        self.nodes.push(node);
        self
    }

    /// Add an edge.
    pub fn with_edge(mut self, edge: Edge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Add metadata.
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Check if graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get node by ID.
    pub fn node(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get mutable node by ID.
    pub fn node_mut(&mut self, id: u64) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Get all edges from a node.
    pub fn edges_from(&self, node_id: u64) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |e| e.source == node_id)
    }

    /// Get all edges to a node.
    pub fn edges_to(&self, node_id: u64) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |e| e.target == node_id)
    }

    /// Get root nodes (no incoming parent edges).
    pub fn roots(&self) -> Vec<u64> {
        let children: std::collections::HashSet<u64> = self
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Parent)
            .map(|e| e.source)
            .collect();

        self.nodes
            .iter()
            .map(|n| n.id)
            .filter(|id| !children.contains(id))
            .collect()
    }

    /// Get children of a node.
    pub fn children(&self, node_id: u64) -> Vec<u64> {
        self.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Parent && e.target == node_id)
            .map(|e| e.source)
            .collect()
    }

    /// Get parent of a node.
    pub fn parent(&self, node_id: u64) -> Option<u64> {
        self.edges
            .iter()
            .find(|e| e.kind == EdgeKind::Parent && e.source == node_id)
            .map(|e| e.target)
    }

    /// Compute bounding box of all positioned nodes.
    pub fn bounds(&self) -> Option<crate::Rect> {
        let positioned: Vec<_> = self.nodes.iter().filter_map(|n| n.position).collect();
        if positioned.is_empty() {
            return None;
        }

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for pos in positioned {
            min_x = min_x.min(pos.x);
            max_x = max_x.max(pos.x);
            min_y = min_y.min(pos.y);
            max_y = max_y.max(pos.y);
        }

        Some(crate::Rect::new(min_x, min_y, max_x, max_y))
    }
}

// =============================================================================
// GRAPH BUILDER (Alternative API)
// =============================================================================

/// Builder for constructing graphs incrementally.
///
/// Useful when graph structure is built from database queries
/// or streaming data sources.
#[derive(Debug, Default)]
pub struct GraphBuilder {
    nodes: HashMap<u64, Node>,
    edges: Vec<Edge>,
    metadata: HashMap<String, serde_json::Value>,
}

impl GraphBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a node.
    pub fn node(&mut self, node: Node) -> &mut Self {
        self.nodes.insert(node.id, node);
        self
    }

    /// Add a node by ID and label.
    pub fn add_node(&mut self, id: u64, label: impl Into<String>) -> &mut Self {
        self.nodes.insert(id, Node::new(id, label));
        self
    }

    /// Add an edge.
    pub fn edge(&mut self, edge: Edge) -> &mut Self {
        self.edges.push(edge);
        self
    }

    /// Add a parent edge.
    pub fn parent(&mut self, child: u64, parent: u64) -> &mut Self {
        self.edges.push(Edge::parent(child, parent));
        self
    }

    /// Add metadata.
    pub fn metadata(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the graph.
    pub fn build(self) -> Graph {
        Graph {
            nodes: self.nodes.into_values().collect(),
            edges: self.edges,
            metadata: self.metadata,
        }
    }
}

// =============================================================================
// CONVERSION: Graph ↔ ChamberSnapshot
// =============================================================================

impl Graph {
    /// Convert to ChamberSnapshot (SoA wire format).
    ///
    /// Requires positions to be set on all nodes, or a layout config
    /// to compute them.
    pub fn to_chamber(&self, chamber_id: u32, string_table: &mut Vec<String>) -> ChamberSnapshot {
        use crate::{CameraPreset, ChamberKind, GridSnapshot, Rect, NONE_ID, NONE_IDX};

        let n = self.nodes.len();
        if n == 0 {
            return ChamberSnapshot {
                id: chamber_id,
                ..Default::default()
            };
        }

        // Build index map: node_id → array index
        let id_to_idx: HashMap<u64, u32> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id, i as u32))
            .collect();

        // Build SoA arrays
        let mut entity_ids = Vec::with_capacity(n);
        let mut kind_ids = Vec::with_capacity(n);
        let mut x = Vec::with_capacity(n);
        let mut y = Vec::with_capacity(n);
        let mut label_ids = Vec::with_capacity(n);
        let mut detail_refs = Vec::with_capacity(n);
        let mut first_child = vec![NONE_IDX; n];
        let mut next_sibling = vec![NONE_IDX; n];
        let mut prev_sibling = vec![NONE_IDX; n];

        // Intern labels into string table
        let mut label_map: HashMap<&str, u32> = HashMap::new();

        for node in &self.nodes {
            entity_ids.push(node.id);
            kind_ids.push(node.kind);

            let pos = node.position.unwrap_or(Vec2::ZERO);
            x.push(pos.x);
            y.push(pos.y);

            // Intern label
            let label_id = if let Some(&id) = label_map.get(node.label.as_str()) {
                id
            } else {
                let id = string_table.len() as u32;
                string_table.push(node.label.clone());
                label_map.insert(&node.label, id);
                id
            };
            label_ids.push(label_id);

            detail_refs.push(node.detail_ref.unwrap_or(NONE_ID));
        }

        // Build navigation indices from parent edges
        // Group children by parent
        let mut children_by_parent: HashMap<u64, Vec<u64>> = HashMap::new();
        for edge in &self.edges {
            if edge.kind == EdgeKind::Parent {
                children_by_parent
                    .entry(edge.target)
                    .or_default()
                    .push(edge.source);
            }
        }

        // Set first_child and sibling links
        for (parent_id, children) in children_by_parent {
            if children.is_empty() {
                continue;
            }

            let parent_idx = match id_to_idx.get(&parent_id) {
                Some(&idx) => idx as usize,
                None => continue,
            };

            // First child
            let first_child_id = children[0];
            if let Some(&fc_idx) = id_to_idx.get(&first_child_id) {
                first_child[parent_idx] = fc_idx;
            }

            // Sibling chain
            for window in children.windows(2) {
                let a_id = window[0];
                let b_id = window[1];
                if let (Some(&a_idx), Some(&b_idx)) = (id_to_idx.get(&a_id), id_to_idx.get(&b_id)) {
                    next_sibling[a_idx as usize] = b_idx;
                    prev_sibling[b_idx as usize] = a_idx;
                }
            }
        }

        // Compute bounds
        let bounds = self.bounds().unwrap_or(Rect::ZERO);

        ChamberSnapshot {
            id: chamber_id,
            kind: ChamberKind::Tree,
            bounds,
            default_camera: CameraPreset::new(bounds.center(), 1.0),
            entity_ids,
            kind_ids,
            x,
            y,
            label_ids,
            detail_refs,
            first_child,
            next_sibling,
            prev_sibling,
            doors: vec![],
            grid: GridSnapshot::default(),
        }
    }

    /// Create from ChamberSnapshot.
    pub fn from_chamber(chamber: &ChamberSnapshot, string_table: &[String]) -> Self {
        let n = chamber.entity_count();
        let mut nodes = Vec::with_capacity(n);

        for i in 0..n {
            let label = chamber
                .label_ids
                .get(i)
                .and_then(|&id| string_table.get(id as usize))
                .cloned()
                .unwrap_or_default();

            let node = Node {
                id: chamber.entity_ids[i],
                label,
                kind: chamber.kind_ids[i],
                position: Some(Vec2::new(chamber.x[i], chamber.y[i])),
                detail_ref: {
                    let dr = chamber.detail_refs[i];
                    if dr == crate::NONE_ID {
                        None
                    } else {
                        Some(dr)
                    }
                },
                properties: HashMap::new(),
            };
            nodes.push(node);
        }

        // Reconstruct edges from navigation indices
        let mut edges = Vec::new();
        for (parent_idx, &first_child_idx) in chamber.first_child.iter().enumerate() {
            if first_child_idx == crate::NONE_IDX {
                continue;
            }

            let parent_id = chamber.entity_ids[parent_idx];
            let mut child_idx = first_child_idx as usize;

            loop {
                let child_id = chamber.entity_ids[child_idx];
                edges.push(Edge::parent(child_id, parent_id));

                match chamber.next_sibling.get(child_idx) {
                    Some(&next) if next != crate::NONE_IDX => child_idx = next as usize,
                    _ => break,
                }
            }
        }

        Graph {
            nodes,
            edges,
            metadata: HashMap::new(),
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_builder() {
        let node = Node::new(42, "Test Node")
            .with_kind(5)
            .with_position(100.0, 200.0)
            .with_detail_ref(99)
            .with_property("foo", "bar");

        assert_eq!(node.id, 42);
        assert_eq!(node.label, "Test Node");
        assert_eq!(node.kind, 5);
        assert_eq!(node.position, Some(Vec2::new(100.0, 200.0)));
        assert_eq!(node.detail_ref, Some(99));
        assert_eq!(node.properties.get("foo").unwrap(), "bar");
    }

    #[test]
    fn graph_builder() {
        let graph = Graph::new()
            .with_node(Node::new(1, "Root").with_position(50.0, 10.0))
            .with_node(Node::new(2, "Child A").with_position(25.0, 30.0))
            .with_node(Node::new(3, "Child B").with_position(75.0, 30.0))
            .with_edge(Edge::parent(2, 1))
            .with_edge(Edge::parent(3, 1));

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.roots(), vec![1]);
        assert_eq!(graph.parent(2), Some(1));
        assert!(graph.children(1).contains(&2));
        assert!(graph.children(1).contains(&3));
    }

    #[test]
    fn graph_builder_incremental() {
        let mut builder = GraphBuilder::new();
        builder.add_node(1, "Root");
        builder.add_node(2, "Child");
        builder.parent(2, 1);

        let graph = builder.build();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.roots(), vec![1]);
    }

    #[test]
    fn graph_to_chamber_roundtrip() {
        let graph = Graph::new()
            .with_node(
                Node::new(100, "Root")
                    .with_kind(0)
                    .with_position(50.0, 10.0),
            )
            .with_node(Node::new(101, "A").with_kind(1).with_position(25.0, 30.0))
            .with_node(Node::new(102, "B").with_kind(1).with_position(75.0, 30.0))
            .with_edge(Edge::parent(101, 100))
            .with_edge(Edge::parent(102, 100));

        let mut string_table = Vec::new();
        let chamber = graph.to_chamber(1, &mut string_table);

        assert_eq!(chamber.entity_count(), 3);
        assert_eq!(string_table.len(), 3); // Root, A, B

        // Roundtrip
        let restored = Graph::from_chamber(&chamber, &string_table);
        assert_eq!(restored.node_count(), 3);
        assert_eq!(restored.edge_count(), 2);

        // Check hierarchy preserved
        let root = restored.node(100).unwrap();
        assert_eq!(root.label, "Root");
        assert_eq!(restored.children(100).len(), 2);
    }

    #[test]
    fn edge_kinds() {
        assert!(EdgeKind::Parent.is_hierarchical());
        assert!(EdgeKind::Sibling.is_hierarchical());
        assert!(!EdgeKind::Control.is_hierarchical());
        assert!(!EdgeKind::Reference.is_hierarchical());

        assert!(EdgeKind::Door.is_navigational());
        assert!(!EdgeKind::Parent.is_navigational());
    }

    #[test]
    fn graph_bounds() {
        let graph = Graph::new()
            .with_node(Node::new(1, "A").with_position(0.0, 0.0))
            .with_node(Node::new(2, "B").with_position(100.0, 50.0))
            .with_node(Node::new(3, "C").with_position(50.0, 100.0));

        let bounds = graph.bounds().unwrap();
        assert_eq!(bounds.min.x, 0.0);
        assert_eq!(bounds.min.y, 0.0);
        assert_eq!(bounds.max.x, 100.0);
        assert_eq!(bounds.max.y, 100.0);
    }

    #[test]
    fn json_serialization() {
        let graph = Graph::new()
            .with_node(Node::new(1, "Test"))
            .with_edge(Edge::parent(2, 1));

        let json = serde_json::to_string_pretty(&graph).unwrap();
        assert!(json.contains("\"id\": 1"));
        assert!(json.contains("\"source\": 2"));

        let restored: Graph = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.node_count(), 1);
    }
}

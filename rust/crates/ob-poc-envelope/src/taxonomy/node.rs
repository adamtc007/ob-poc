//! TaxonomyNode - Universal tree structure for any hierarchical view
//!
//! The node derives its visual metaphor from its shape, not the other way around.
//! A node with 1500 descendants at depth 2 IS a galaxy - the data defines the viz.
//!
//! Key innovation: Each node carries an `ExpansionRule` that defines how to
//! expand it into a child taxonomy. This enables fractal navigation where
//! every node is potentially a taxonomy waiting to be expanded.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::combinators::TaxonomyParser;
use super::rules::TaxonomyContext;
use super::types::{AstroLevel, DimensionValues, EntitySummary, Filter, Metaphor, NodeType};

// =============================================================================
// EXPANSION RULE - How a node expands into a child taxonomy
// =============================================================================

/// Defines how a node can be expanded into a child taxonomy.
/// This is the key to fractal navigation - every node knows how to become a taxonomy.
#[derive(Clone, Default)]
pub enum ExpansionRule {
    /// Use this parser when expanded (set by .each_is_taxonomy())
    Parser(Arc<dyn TaxonomyParser + Send + Sync>),

    /// Derive parser from context (e.g., CBU nodes expand to CbuTrading context)
    Context(TaxonomyContext),

    /// Already fully loaded - no expansion needed
    #[default]
    Complete,

    /// Terminal node - cannot be expanded (e.g., natural persons, documents)
    Terminal,
}

impl std::fmt::Debug for ExpansionRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpansionRule::Parser(_) => write!(f, "ExpansionRule::Parser(<parser>)"),
            ExpansionRule::Context(ctx) => write!(f, "ExpansionRule::Context({:?})", ctx),
            ExpansionRule::Complete => write!(f, "ExpansionRule::Complete"),
            ExpansionRule::Terminal => write!(f, "ExpansionRule::Terminal"),
        }
    }
}

/// Universal taxonomy node - works for CBU trees, UBO chains, entity forests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyNode {
    /// Unique identifier for this node
    pub id: Uuid,

    /// Type of node (CBU, Entity, Cluster, etc.)
    pub node_type: NodeType,

    /// Display label
    pub label: String,

    /// Short label for compact display
    pub short_label: Option<String>,

    /// Child nodes
    pub children: Vec<TaxonomyNode>,

    // =========================================================================
    // COMPUTED FIELDS - Set by builder
    // =========================================================================
    /// Depth in tree (0 = root)
    pub depth: u32,

    /// Total descendants (recursive count)
    pub descendant_count: usize,

    /// Max depth of subtree from this node
    pub subtree_depth: u32,

    /// Max width at any level in subtree
    pub subtree_max_width: usize,

    // =========================================================================
    // DIMENSION VALUES - For filtering and coloring
    // =========================================================================
    /// Dimension values for filtering/grouping
    pub dimensions: DimensionValues,

    // =========================================================================
    // OPTIONAL DATA - Lazy loaded
    // =========================================================================
    /// Entity summary (lazy loaded on expand)
    pub entity_data: Option<EntitySummary>,

    /// Whether this node has unexpanded children in DB
    pub has_more_children: bool,

    // =========================================================================
    // EXPANSION RULE - Fractal navigation
    // =========================================================================
    /// How to expand this node into a child taxonomy (fractal navigation)
    #[serde(skip)]
    pub expansion: ExpansionRule,

    // =========================================================================
    // FRACTAL ZOOM STATE - For zoom-level-dependent rendering
    // =========================================================================
    /// Whether this node is currently collapsed (children hidden).
    /// When collapsed, node shows aggregate summary instead of children.
    #[serde(default)]
    pub is_collapsed: bool,

    /// Zoom threshold at which this node auto-collapses.
    /// When camera zoom < this value, node collapses to save screen space.
    /// None = never auto-collapse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapse_at_zoom: Option<f32>,

    /// Zoom threshold at which this node auto-expands.
    /// When camera zoom > this value, node expands to show children.
    /// None = never auto-expand (requires explicit user action).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand_at_zoom: Option<f32>,

    /// Whether to show aggregate summary when collapsed.
    /// If true, shows "N items" or similar; if false, just shows the node.
    #[serde(default)]
    pub show_aggregate_when_collapsed: bool,

    /// Cached aggregate summary for collapsed display (e.g., "15 funds, 3 entities").
    /// Computed lazily when node is collapsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_summary: Option<String>,

    /// Priority for fractal navigation (higher = more important, shown first).
    /// Used when space is limited to decide which nodes to show.
    #[serde(default)]
    pub priority: i32,
}

impl TaxonomyNode {
    /// Create a new root node
    pub fn root(label: impl Into<String>) -> Self {
        Self {
            id: Uuid::nil(),
            node_type: NodeType::Root,
            label: label.into(),
            short_label: None,
            children: Vec::new(),
            depth: 0,
            descendant_count: 0,
            subtree_depth: 0,
            subtree_max_width: 0,
            dimensions: DimensionValues::default(),
            entity_data: None,
            has_more_children: false,
            expansion: ExpansionRule::Complete,
            // Fractal zoom state - defaults
            is_collapsed: false,
            collapse_at_zoom: None,
            expand_at_zoom: None,
            show_aggregate_when_collapsed: false,
            aggregate_summary: None,
            priority: 0,
        }
    }

    /// Create a new node with basic info
    pub fn new(
        id: Uuid,
        node_type: NodeType,
        label: impl Into<String>,
        dimensions: DimensionValues,
    ) -> Self {
        Self {
            id,
            node_type,
            label: label.into(),
            short_label: None,
            children: Vec::new(),
            depth: 0,
            descendant_count: 0,
            subtree_depth: 0,
            subtree_max_width: 0,
            dimensions,
            entity_data: None,
            has_more_children: false,
            expansion: ExpansionRule::Complete,
            // Fractal zoom state - defaults
            is_collapsed: false,
            collapse_at_zoom: None,
            expand_at_zoom: None,
            show_aggregate_when_collapsed: false,
            aggregate_summary: None,
            priority: 0,
        }
    }

    /// Create an empty root (for empty view state)
    pub fn empty_root() -> Self {
        Self::root("Empty")
    }

    /// Add a child node
    pub fn add_child(&mut self, child: TaxonomyNode) {
        self.children.push(child);
    }

    /// Compute derived fields after tree is built
    pub fn compute_metrics(&mut self) {
        self.compute_metrics_recursive(0);
    }

    fn compute_metrics_recursive(&mut self, depth: u32) {
        self.depth = depth;

        // Recurse to children first
        for child in &mut self.children {
            child.compute_metrics_recursive(depth + 1);
        }

        // Compute descendant count
        self.descendant_count = self.children.iter().map(|c| 1 + c.descendant_count).sum();

        // Compute subtree depth
        self.subtree_depth = self
            .children
            .iter()
            .map(|c| 1 + c.subtree_depth)
            .max()
            .unwrap_or(0);

        // Compute max width at any level
        self.subtree_max_width = self.compute_max_width();
    }

    fn compute_max_width(&self) -> usize {
        let mut max_width = self.children.len();

        // BFS to find max width at any level
        let mut current_level = self.children.iter().collect::<Vec<_>>();
        while !current_level.is_empty() {
            max_width = max_width.max(current_level.len());
            current_level = current_level
                .iter()
                .flat_map(|n| n.children.iter())
                .collect();
        }

        max_width
    }

    // =========================================================================
    // DERIVED PROPERTIES
    // =========================================================================

    /// Astronomical level derived from characteristics
    pub fn astro_level(&self) -> AstroLevel {
        AstroLevel::from_characteristics(self.descendant_count, self.depth)
    }

    /// Visual metaphor derived from tree shape
    pub fn metaphor(&self) -> Metaphor {
        Metaphor::from_shape(
            self.subtree_depth,
            self.subtree_max_width,
            self.descendant_count,
        )
    }

    /// Is this a leaf node?
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty() && !self.has_more_children
    }

    /// Can this node be expanded?
    pub fn is_expandable(&self) -> bool {
        self.has_more_children || !self.children.is_empty()
    }

    // =========================================================================
    // FRACTAL ZOOM BEHAVIOR
    // =========================================================================

    /// Check if this node should auto-collapse at the given zoom level.
    ///
    /// Returns true if:
    /// - `collapse_at_zoom` is set AND
    /// - current zoom is less than the threshold
    pub fn should_collapse_at_zoom(&self, current_zoom: f32) -> bool {
        self.collapse_at_zoom
            .map(|threshold| current_zoom < threshold)
            .unwrap_or(false)
    }

    /// Check if this node should auto-expand at the given zoom level.
    ///
    /// Returns true if:
    /// - `expand_at_zoom` is set AND
    /// - current zoom is greater than the threshold AND
    /// - node is currently collapsed
    pub fn should_expand_at_zoom(&self, current_zoom: f32) -> bool {
        self.is_collapsed
            && self
                .expand_at_zoom
                .map(|threshold| current_zoom > threshold)
                .unwrap_or(false)
    }

    /// Update collapse state based on zoom level.
    /// Returns true if state changed.
    pub fn update_collapse_for_zoom(&mut self, current_zoom: f32) -> bool {
        let was_collapsed = self.is_collapsed;

        if self.should_expand_at_zoom(current_zoom) {
            self.is_collapsed = false;
        } else if self.should_collapse_at_zoom(current_zoom) {
            self.is_collapsed = true;
        }

        self.is_collapsed != was_collapsed
    }

    /// Set zoom thresholds for this node.
    /// Typically called when loading node type config from database.
    pub fn set_zoom_thresholds(&mut self, collapse_at: Option<f32>, expand_at: Option<f32>) {
        self.collapse_at_zoom = collapse_at;
        self.expand_at_zoom = expand_at;
    }

    /// Compute aggregate summary for collapsed display.
    /// Updates `aggregate_summary` field.
    pub fn compute_aggregate_summary(&mut self) {
        if self.children.is_empty() {
            self.aggregate_summary = None;
            return;
        }

        // Count children by type
        let mut by_type: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for child in &self.children {
            let type_name = match child.node_type {
                NodeType::Cbu => "CBU",
                NodeType::Entity => "entity",
                NodeType::Client => "client",
                NodeType::Cluster => "group",
                NodeType::Product => "product",
                NodeType::Service => "service",
                NodeType::Root => "root",
                NodeType::Position => "position",
                NodeType::Document => "document",
                NodeType::Observation => "observation",
            };
            *by_type.entry(type_name).or_insert(0) += 1;
        }

        // Build summary string
        let parts: Vec<String> = by_type
            .iter()
            .map(|(t, n)| {
                if *n == 1 {
                    format!("1 {}", t)
                } else {
                    format!("{} {}s", n, t)
                }
            })
            .collect();

        self.aggregate_summary = Some(parts.join(", "));
    }

    // =========================================================================
    // TREE OPERATIONS
    // =========================================================================

    /// Collect all node IDs in subtree (including self)
    pub fn all_ids(&self) -> Vec<Uuid> {
        let mut ids = vec![self.id];
        for child in &self.children {
            ids.extend(child.all_ids());
        }
        ids
    }

    /// Collect IDs of nodes matching a filter
    pub fn matching_ids(&self, filter: &Filter) -> Vec<Uuid> {
        let mut ids = Vec::new();
        if filter.matches(&self.dimensions) {
            ids.push(self.id);
        }
        for child in &self.children {
            ids.extend(child.matching_ids(filter));
        }
        ids
    }

    /// Find a node by ID
    pub fn find(&self, id: Uuid) -> Option<&TaxonomyNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node by ID (mutable)
    pub fn find_mut(&mut self, id: Uuid) -> Option<&mut TaxonomyNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }
        None
    }

    /// Get path from root to a node
    pub fn path_to(&self, id: Uuid) -> Option<Vec<Uuid>> {
        if self.id == id {
            return Some(vec![self.id]);
        }
        for child in &self.children {
            if let Some(mut path) = child.path_to(id) {
                path.insert(0, self.id);
                return Some(path);
            }
        }
        None
    }

    /// Count nodes at a specific depth
    pub fn count_at_depth(&self, target_depth: u32) -> usize {
        if self.depth == target_depth {
            return 1;
        }
        if self.depth > target_depth {
            return 0;
        }
        self.children
            .iter()
            .map(|c| c.count_at_depth(target_depth))
            .sum()
    }

    /// Get nodes at a specific depth
    pub fn nodes_at_depth(&self, target_depth: u32) -> Vec<&TaxonomyNode> {
        if self.depth == target_depth {
            return vec![self];
        }
        if self.depth > target_depth {
            return vec![];
        }
        self.children
            .iter()
            .flat_map(|c| c.nodes_at_depth(target_depth))
            .collect()
    }

    // =========================================================================
    // SUMMARY
    // =========================================================================

    /// Generate a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "{} ({} items, {} levels, {:?} layout)",
            self.label,
            self.descendant_count,
            self.subtree_depth,
            self.metaphor()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_tree() -> TaxonomyNode {
        use crate::taxonomy::types::DimensionValues;

        let mut root = TaxonomyNode::root("Universe");

        // Add 3 clients
        for i in 0..3 {
            let mut client = TaxonomyNode::new(
                Uuid::new_v4(),
                NodeType::Client,
                format!("Client {}", i),
                DimensionValues::default(),
            );

            // Each client has 5 CBUs
            for j in 0..5 {
                let cbu = TaxonomyNode::new(
                    Uuid::new_v4(),
                    NodeType::Cbu,
                    format!("CBU {}-{}", i, j),
                    DimensionValues::default(),
                );
                client.add_child(cbu);
            }

            root.add_child(client);
        }

        root.compute_metrics();
        root
    }

    #[test]
    fn test_tree_metrics() {
        let tree = build_test_tree();

        assert_eq!(tree.depth, 0);
        assert_eq!(tree.descendant_count, 18); // 3 clients + 15 CBUs
        assert_eq!(tree.subtree_depth, 2);
        assert_eq!(tree.subtree_max_width, 15); // All CBUs at level 2 (3 clients × 5 CBUs)
    }

    #[test]
    fn test_find_node() {
        let tree = build_test_tree();
        let first_client_id = tree.children[0].id;

        let found = tree.find(first_client_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, first_client_id);
    }

    #[test]
    fn test_all_ids() {
        let tree = build_test_tree();
        let ids = tree.all_ids();

        // Root + 3 clients + 15 CBUs = 19
        assert_eq!(ids.len(), 19);
    }

    #[test]
    fn test_path_to() {
        let tree = build_test_tree();
        let cbu_id = tree.children[0].children[0].id;

        let path = tree.path_to(cbu_id).unwrap();
        assert_eq!(path.len(), 3); // root -> client -> cbu
    }

    #[test]
    fn test_metaphor_for_shapes() {
        use crate::taxonomy::types::DimensionValues;

        // Test tree has 15 CBUs at level 2 -> Network metaphor (max_width >= 10)
        let small = build_test_tree();
        assert_eq!(small.metaphor(), Metaphor::Network);

        // Build a deep tree
        let mut deep = TaxonomyNode::root("Deep");
        let mut current = &mut deep;
        for i in 0..7 {
            let child = TaxonomyNode::new(
                Uuid::new_v4(),
                NodeType::Entity,
                format!("Level {}", i),
                DimensionValues::default(),
            );
            current.add_child(child);
            current = &mut current.children[0];
        }
        deep.compute_metrics();
        assert_eq!(deep.metaphor(), Metaphor::Pyramid);
    }
}

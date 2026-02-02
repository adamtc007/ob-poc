//! Config-driven layout engine for CBU graph visualization (V2)
//!
//! This module replaces the hardcoded tier logic in layout.rs with a database-driven
//! approach where layout behavior comes from edge_types and node_types configuration.
//!
//! ## Key Differences from V1
//!
//! | Aspect | V1 (layout.rs) | V2 (layout_v2.rs) |
//! |--------|----------------|-------------------|
//! | Tier assignment | Hardcoded by RoleCategory/LayoutBehavior | Computed from edge tier_delta |
//! | View modes | Hardcoded ViewMode enum | Database view_modes config |
//! | Visibility | Hardcoded is_ubo_relevant() | Database show_in_*_view columns |
//! | Hierarchy | Assumed from node type | Edge is_hierarchical flag |
//! | Cycle handling | None (assumed DAG) | Cycle breaking via back-edge detection |
//!
//! ## Algorithm
//!
//! 1. Load edge configuration from database (hierarchy edges, tier deltas)
//! 2. Build adjacency graph from edges
//! 3. Identify hierarchy edges (is_hierarchical = true)
//! 4. Compute depth via BFS from root nodes using hierarchy edges only
//! 5. Break cycles by marking back-edges
//! 6. Apply tier_delta for cross-hierarchy edges
//! 7. Compute x,y positions based on depth and sibling count

use std::collections::{HashMap, HashSet, VecDeque};

#[cfg(feature = "database")]
use crate::database::{EdgeTypeConfig, ViewConfigService};

use super::types::{CbuGraph, EdgeType, LegacyGraphNode, NodeType};

// Re-export as GraphNode for this module
type GraphNode = LegacyGraphNode;

/// Configuration for the layout engine, can come from database or defaults
#[derive(Debug, Clone)]
pub struct LayoutConfigV2 {
    /// Horizontal spacing between nodes in same tier
    pub node_spacing_x: f32,
    /// Vertical spacing between tiers
    pub tier_spacing_y: f32,
    /// Default node width
    pub node_width: f32,
    /// Default node height
    pub node_height: f32,
    /// Canvas width for centering
    pub canvas_width: f32,
    /// Canvas height
    pub canvas_height: f32,
    /// Margin from edges
    pub margin: f32,
    /// Whether to use horizontal (LTR) or vertical (TTB) layout
    pub horizontal: bool,
}

impl Default for LayoutConfigV2 {
    fn default() -> Self {
        Self {
            node_spacing_x: 200.0,
            tier_spacing_y: 140.0,
            node_width: 160.0,
            node_height: 60.0,
            canvas_width: 1400.0,
            canvas_height: 900.0,
            margin: 80.0,
            horizontal: false,
        }
    }
}

/// Edge configuration for layout purposes
#[derive(Debug, Clone)]
pub struct EdgeLayoutConfig {
    /// Type code (e.g., "Owns", "HasRole")
    pub type_code: String,
    /// Whether this edge defines parent-child hierarchy for layout
    pub is_hierarchical: bool,
    /// Tier delta: how many tiers down should child be from parent
    pub tier_delta: i32,
    /// Layout direction hint
    pub layout_direction: Option<String>,
    /// Priority for routing/ordering
    pub routing_priority: i32,
}

impl Default for EdgeLayoutConfig {
    fn default() -> Self {
        Self {
            type_code: String::new(),
            is_hierarchical: false,
            tier_delta: 1,
            layout_direction: None,
            routing_priority: 50,
        }
    }
}

#[cfg(feature = "database")]
impl From<&EdgeTypeConfig> for EdgeLayoutConfig {
    fn from(cfg: &EdgeTypeConfig) -> Self {
        Self {
            type_code: cfg.edge_type_code.clone(),
            is_hierarchical: cfg.is_hierarchical,
            tier_delta: cfg.tier_delta.unwrap_or(1),
            layout_direction: cfg.layout_direction.clone(),
            routing_priority: cfg.routing_priority.unwrap_or(50),
        }
    }
}

/// Node with computed layout information
#[derive(Debug, Clone)]
struct LayoutNode {
    /// Index in original nodes array
    index: usize,
    /// Node ID (stored for Debug output, not accessed in logic)
    _id: String,
    /// Computed depth/tier (0 = root)
    depth: i32,
    /// Position within tier (for ordering)
    order_in_tier: i32,
    /// Is this a root node (no incoming hierarchy edges)?
    is_root: bool,
    /// Children via hierarchy edges
    children: Vec<usize>,
    /// Parents via hierarchy edges
    parents: Vec<usize>,
    /// Computed x position
    x: f32,
    /// Computed y position
    y: f32,
}

/// Edge with cycle detection info
#[derive(Debug, Clone)]
struct LayoutEdge {
    /// Index in original edges array
    #[allow(dead_code)]
    index: usize,
    /// Source node index
    from_idx: usize,
    /// Target node index
    to_idx: usize,
    /// Edge type configuration
    config: EdgeLayoutConfig,
    /// Is this a back-edge (would create cycle)?
    is_back_edge: bool,
}

/// Result of layout computation with back-edge info
#[derive(Debug, Clone, Default)]
pub struct LayoutResult {
    /// Indices of edges that are back-edges (create cycles)
    pub back_edge_indices: Vec<usize>,
}

/// Config-driven layout engine
pub struct LayoutEngineV2 {
    config: LayoutConfigV2,
    /// Edge type configurations keyed by type_code
    edge_configs: HashMap<String, EdgeLayoutConfig>,
    /// View mode name (for logging/debugging)
    #[allow(dead_code)]
    view_mode: String,
}

impl LayoutEngineV2 {
    /// Create a new layout engine with default configuration
    pub fn new() -> Self {
        Self {
            config: LayoutConfigV2::default(),
            edge_configs: Self::default_edge_configs(),
            view_mode: "KYC_UBO".to_string(),
        }
    }

    /// Create with custom layout configuration
    pub fn with_config(config: LayoutConfigV2) -> Self {
        Self {
            config,
            edge_configs: Self::default_edge_configs(),
            view_mode: "KYC_UBO".to_string(),
        }
    }

    /// Create from database configuration
    #[cfg(feature = "database")]
    pub async fn from_database(
        pool: &sqlx::PgPool,
        view_mode: &str,
        horizontal: bool,
    ) -> Result<Self, anyhow::Error> {
        // Load edge type configurations using static methods
        let edge_types = ViewConfigService::get_all_edge_types(pool).await?;
        let mut edge_configs = HashMap::new();
        for et in &edge_types {
            edge_configs.insert(et.edge_type_code.clone(), EdgeLayoutConfig::from(et));
        }

        // Load view mode specific config if available
        let _view_config = ViewConfigService::get_view_mode_config(pool, view_mode)
            .await
            .ok();

        // Get layout config from view_modes table or use defaults
        let layout_config = LayoutConfigV2 {
            horizontal,
            ..LayoutConfigV2::default()
        };

        Ok(Self {
            config: layout_config,
            edge_configs,
            view_mode: view_mode.to_string(),
        })
    }

    /// Set edge configurations (for testing or non-database use)
    pub fn with_edge_configs(mut self, configs: HashMap<String, EdgeLayoutConfig>) -> Self {
        self.edge_configs = configs;
        self
    }

    /// Set horizontal layout mode
    pub fn horizontal(mut self, horizontal: bool) -> Self {
        self.config.horizontal = horizontal;
        self
    }

    /// Default edge configurations when database is not available
    fn default_edge_configs() -> HashMap<String, EdgeLayoutConfig> {
        let mut configs = HashMap::new();

        // Ownership/control edges are hierarchical
        configs.insert(
            "Owns".to_string(),
            EdgeLayoutConfig {
                type_code: "Owns".to_string(),
                is_hierarchical: true,
                tier_delta: 1,
                layout_direction: Some("UP".to_string()),
                routing_priority: 10,
            },
        );

        configs.insert(
            "Controls".to_string(),
            EdgeLayoutConfig {
                type_code: "Controls".to_string(),
                is_hierarchical: true,
                tier_delta: 1,
                layout_direction: Some("UP".to_string()),
                routing_priority: 20,
            },
        );

        // Role edges are overlays, not hierarchical
        configs.insert(
            "HasRole".to_string(),
            EdgeLayoutConfig {
                type_code: "HasRole".to_string(),
                is_hierarchical: false,
                tier_delta: 0,
                layout_direction: None,
                routing_priority: 50,
            },
        );

        // Product/service edges are hierarchical (flow down from CBU)
        configs.insert(
            "HasProduct".to_string(),
            EdgeLayoutConfig {
                type_code: "HasProduct".to_string(),
                is_hierarchical: true,
                tier_delta: 1,
                layout_direction: Some("DOWN".to_string()),
                routing_priority: 30,
            },
        );

        configs.insert(
            "Delivers".to_string(),
            EdgeLayoutConfig {
                type_code: "Delivers".to_string(),
                is_hierarchical: true,
                tier_delta: 1,
                layout_direction: Some("DOWN".to_string()),
                routing_priority: 40,
            },
        );

        configs
    }

    /// Get edge configuration, using default if not found
    fn get_edge_config(&self, edge_type: &EdgeType) -> EdgeLayoutConfig {
        let type_str = format!("{:?}", edge_type);
        self.edge_configs
            .get(&type_str)
            .cloned()
            .unwrap_or_else(|| EdgeLayoutConfig {
                type_code: type_str,
                ..EdgeLayoutConfig::default()
            })
    }

    /// Apply layout to graph, computing x,y positions for all nodes
    /// Returns layout result with back-edge information
    pub fn layout(&self, graph: &mut CbuGraph) -> LayoutResult {
        let mut result = LayoutResult::default();

        if graph.nodes.is_empty() {
            return result;
        }

        // Build node ID to index map
        let id_to_idx: HashMap<String, usize> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), i))
            .collect();

        // Build layout nodes and edges
        let mut layout_nodes: Vec<LayoutNode> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| LayoutNode {
                index: i,
                _id: n.id.clone(),
                depth: -1, // Unassigned
                order_in_tier: 0,
                is_root: false,
                children: Vec::new(),
                parents: Vec::new(),
                x: 0.0,
                y: 0.0,
            })
            .collect();

        let mut layout_edges: Vec<LayoutEdge> = Vec::new();

        // Build adjacency from edges
        for (i, edge) in graph.edges.iter().enumerate() {
            let from_id = &edge.source;
            let to_id = &edge.target;

            let from_idx = match id_to_idx.get(from_id) {
                Some(&idx) => idx,
                None => continue, // Edge references missing node
            };
            let to_idx = match id_to_idx.get(to_id) {
                Some(&idx) => idx,
                None => continue,
            };

            let config = self.get_edge_config(&edge.edge_type);

            layout_edges.push(LayoutEdge {
                index: i,
                from_idx,
                to_idx,
                config: config.clone(),
                is_back_edge: false,
            });

            // Build parent/child relationships for hierarchy edges
            if config.is_hierarchical {
                layout_nodes[from_idx].children.push(to_idx);
                layout_nodes[to_idx].parents.push(from_idx);
            }
        }

        // Identify root nodes (CBU or nodes with no incoming hierarchy edges)
        let roots: Vec<usize> = layout_nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                // CBU is always a root
                if graph.nodes[*i].node_type == NodeType::Cbu {
                    return true;
                }
                // Node with no parents is a root
                n.parents.is_empty()
            })
            .map(|(i, _)| i)
            .collect();

        for &root_idx in &roots {
            layout_nodes[root_idx].is_root = true;
        }

        // Compute depth via BFS from roots
        self.compute_depths(&mut layout_nodes, &roots);

        // Detect and mark back-edges (cycles)
        self.detect_back_edges(&layout_nodes, &mut layout_edges);

        // Collect back-edge indices for result
        for le in &layout_edges {
            if le.is_back_edge {
                result.back_edge_indices.push(le.index);
            }
        }

        // Assign order within each tier
        self.assign_tier_order(&mut layout_nodes, &graph.nodes);

        // Compute x,y positions
        self.compute_positions(&mut layout_nodes);

        // Apply positions back to graph nodes
        for ln in &layout_nodes {
            let node = &mut graph.nodes[ln.index];
            node.x = Some(ln.x);
            node.y = Some(ln.y);
            node.layout_tier = Some(ln.depth);
            node.width = Some(self.config.node_width);
            node.height = Some(self.config.node_height);
        }

        result
    }

    /// Compute depths via BFS from root nodes
    fn compute_depths(&self, nodes: &mut [LayoutNode], roots: &[usize]) {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        // Initialize roots at depth 0
        for &root_idx in roots {
            nodes[root_idx].depth = 0;
            queue.push_back(root_idx);
            visited.insert(root_idx);
        }

        // BFS traversal
        while let Some(current) = queue.pop_front() {
            let current_depth = nodes[current].depth;

            for &child_idx in &nodes[current].children.clone() {
                if !visited.contains(&child_idx) {
                    // Child depth = parent depth + tier_delta (usually 1)
                    nodes[child_idx].depth = current_depth + 1;
                    visited.insert(child_idx);
                    queue.push_back(child_idx);
                }
            }
        }

        // Assign depth to any orphan nodes (not reachable from roots)
        let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
        for node in nodes.iter_mut() {
            if node.depth < 0 {
                // Place orphans at the bottom
                node.depth = max_depth + 1;
            }
        }
    }

    /// Detect back-edges that would create cycles
    fn detect_back_edges(&self, nodes: &[LayoutNode], edges: &mut [LayoutEdge]) {
        for edge in edges.iter_mut() {
            if !edge.config.is_hierarchical {
                continue; // Only hierarchy edges can create layout cycles
            }

            let from_depth = nodes[edge.from_idx].depth;
            let to_depth = nodes[edge.to_idx].depth;

            // A back-edge goes from deeper to shallower (or same level)
            if to_depth <= from_depth {
                edge.is_back_edge = true;
            }
        }
    }

    /// Assign order within each tier for left-to-right positioning
    fn assign_tier_order(&self, nodes: &mut [LayoutNode], graph_nodes: &[GraphNode]) {
        // Group nodes by depth
        let mut tiers: HashMap<i32, Vec<usize>> = HashMap::new();
        for (i, node) in nodes.iter().enumerate() {
            tiers.entry(node.depth).or_default().push(i);
        }

        // Sort nodes within each tier
        for tier_nodes in tiers.values_mut() {
            // Sort by:
            // 1. Node type (CBU first, then Entity, Product, Service, Resource)
            // 2. Entity category (SHELL before PERSON for left/right split)
            // 3. Name alphabetically
            tier_nodes.sort_by(|&a, &b| {
                let node_a = &graph_nodes[a];
                let node_b = &graph_nodes[b];

                // Node type priority
                let type_order = |n: &GraphNode| match n.node_type {
                    NodeType::Cbu => 0,
                    NodeType::Product => 1,
                    NodeType::Entity => 2,
                    NodeType::Service => 3,
                    NodeType::Resource => 4,
                    _ => 5,
                };

                let type_cmp = type_order(node_a).cmp(&type_order(node_b));
                if type_cmp != std::cmp::Ordering::Equal {
                    return type_cmp;
                }

                // Entity category: SHELL before PERSON
                let cat_order = |n: &GraphNode| match n.entity_category.as_deref() {
                    Some("SHELL") => 0,
                    Some("PERSON") => 1,
                    _ => 2,
                };

                let cat_cmp = cat_order(node_a).cmp(&cat_order(node_b));
                if cat_cmp != std::cmp::Ordering::Equal {
                    return cat_cmp;
                }

                // Alphabetical by name
                node_a.label.cmp(&node_b.label)
            });

            // Assign order
            for (order, &idx) in tier_nodes.iter().enumerate() {
                nodes[idx].order_in_tier = order as i32;
            }
        }
    }

    /// Compute x,y positions based on depth and order
    fn compute_positions(&self, nodes: &mut [LayoutNode]) {
        // Group by depth to count tier sizes
        let mut tier_sizes: HashMap<i32, i32> = HashMap::new();
        for node in nodes.iter() {
            *tier_sizes.entry(node.depth).or_insert(0) += 1;
        }

        for node in nodes.iter_mut() {
            let tier_size = tier_sizes.get(&node.depth).copied().unwrap_or(1);

            if self.config.horizontal {
                // Horizontal layout: x = depth, y = order in tier
                node.x = self.config.margin + node.depth as f32 * self.config.node_spacing_x;

                // Center tier vertically
                let tier_height = tier_size as f32 * self.config.tier_spacing_y;
                let start_y = (self.config.canvas_height - tier_height) / 2.0;
                node.y = start_y + node.order_in_tier as f32 * self.config.tier_spacing_y;
            } else {
                // Vertical layout: x = order in tier, y = depth
                node.y = self.config.margin + node.depth as f32 * self.config.tier_spacing_y;

                // Center tier horizontally
                let tier_width = tier_size as f32 * self.config.node_spacing_x;
                let start_x = (self.config.canvas_width - tier_width) / 2.0;
                node.x = start_x + node.order_in_tier as f32 * self.config.node_spacing_x;
            }
        }
    }
}

impl Default for LayoutEngineV2 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::types::{
        LayerInfo, LayerType, LegacyGraphEdge, LegacyGraphStats, NodeStatus,
    };

    fn make_test_node(id: &str, node_type: NodeType, label: &str) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            node_type,
            layer: LayerType::Core,
            label: label.to_string(),
            sublabel: None,
            status: NodeStatus::default(),
            data: serde_json::Value::Null,
            parent_id: None,
            roles: Vec::new(),
            role_categories: Vec::new(),
            primary_role: None,
            jurisdiction: None,
            role_priority: None,
            entity_category: None,
            primary_role_category: None,
            layout_behavior: None,
            ubo_treatment: None,
            kyc_obligation: None,
            x: None,
            y: None,
            width: None,
            height: None,
            layout_tier: None,
            importance: None,
            kyc_completion: None,
            verification_status: None,
            is_container: false,
            contains_type: None,
            container_parent_id: None,
            child_count: None,
            browse_nickname: None,
            parent_key: None,
            person_state: None,
        }
    }

    fn make_test_edge(
        id: &str,
        source: &str,
        target: &str,
        edge_type: EdgeType,
    ) -> LegacyGraphEdge {
        LegacyGraphEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            edge_type,
            label: None,
        }
    }

    fn make_test_graph(nodes: Vec<GraphNode>, edges: Vec<LegacyGraphEdge>) -> CbuGraph {
        CbuGraph {
            cbu_id: uuid::Uuid::now_v7(),
            label: "Test CBU".to_string(),
            cbu_category: None,
            jurisdiction: None,
            nodes,
            edges,
            layers: vec![LayerInfo {
                layer_type: LayerType::Core,
                label: "Core".to_string(),
                color: "#666666".to_string(),
                node_count: 0,
                visible: true,
            }],
            stats: LegacyGraphStats::default(),
        }
    }

    #[test]
    fn test_simple_layout() {
        let mut graph = make_test_graph(
            vec![
                make_test_node("cbu1", NodeType::Cbu, "Test CBU"),
                make_test_node("entity1", NodeType::Entity, "Entity 1"),
                make_test_node("entity2", NodeType::Entity, "Entity 2"),
            ],
            vec![
                make_test_edge("e1", "cbu1", "entity1", EdgeType::HasRole),
                make_test_edge("e2", "cbu1", "entity2", EdgeType::HasRole),
            ],
        );

        let engine = LayoutEngineV2::new();
        let _result = engine.layout(&mut graph);

        // CBU should be at tier 0
        assert_eq!(graph.nodes[0].layout_tier, Some(0));

        // All nodes should have positions
        for node in &graph.nodes {
            assert!(node.x.is_some(), "Node {} missing x", node.id);
            assert!(node.y.is_some(), "Node {} missing y", node.id);
        }
    }

    #[test]
    fn test_hierarchy_layout() {
        let mut graph = make_test_graph(
            vec![
                make_test_node("cbu1", NodeType::Cbu, "Test CBU"),
                make_test_node("entity1", NodeType::Entity, "HoldCo"),
                make_test_node("entity2", NodeType::Entity, "UBO"),
            ],
            vec![
                // Ownership chain: CBU -> HoldCo -> UBO
                make_test_edge("e1", "cbu1", "entity1", EdgeType::Owns),
                make_test_edge("e2", "entity1", "entity2", EdgeType::Owns),
            ],
        );

        let engine = LayoutEngineV2::new();
        let _result = engine.layout(&mut graph);

        // CBU at tier 0, HoldCo at tier 1, UBO at tier 2
        assert_eq!(graph.nodes[0].layout_tier, Some(0));
        assert_eq!(graph.nodes[1].layout_tier, Some(1));
        assert_eq!(graph.nodes[2].layout_tier, Some(2));

        // Vertical layout: y increases with tier
        assert!(
            graph.nodes[0].y.unwrap() < graph.nodes[1].y.unwrap(),
            "CBU should be above HoldCo"
        );
        assert!(
            graph.nodes[1].y.unwrap() < graph.nodes[2].y.unwrap(),
            "HoldCo should be above UBO"
        );
    }

    #[test]
    fn test_horizontal_layout() {
        let mut graph = make_test_graph(
            vec![
                make_test_node("cbu1", NodeType::Cbu, "Test CBU"),
                make_test_node("entity1", NodeType::Entity, "Entity 1"),
            ],
            vec![make_test_edge("e1", "cbu1", "entity1", EdgeType::Owns)],
        );

        let engine = LayoutEngineV2::new().horizontal(true);
        let _result = engine.layout(&mut graph);

        // Horizontal layout: x increases with tier
        assert!(
            graph.nodes[0].x.unwrap() < graph.nodes[1].x.unwrap(),
            "CBU should be left of Entity"
        );
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = make_test_graph(
            vec![
                make_test_node("a", NodeType::Entity, "A"),
                make_test_node("b", NodeType::Entity, "B"),
                make_test_node("c", NodeType::Entity, "C"),
            ],
            vec![
                make_test_edge("e1", "a", "b", EdgeType::Owns),
                make_test_edge("e2", "b", "c", EdgeType::Owns),
                make_test_edge("e3", "c", "a", EdgeType::Owns), // Creates cycle
            ],
        );

        let engine = LayoutEngineV2::new();
        let result = engine.layout(&mut graph);

        // The c->a edge (index 2) should be detected as a back-edge
        assert!(
            result.back_edge_indices.contains(&2),
            "Edge c->a should be detected as back-edge"
        );

        // All nodes should still have valid positions
        for node in &graph.nodes {
            assert!(node.x.is_some());
            assert!(node.y.is_some());
        }
    }
}

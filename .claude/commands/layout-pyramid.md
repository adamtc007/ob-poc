# Pyramid Layout Implementation

Implement the pyramid (Sugiyama-style hierarchical) layout engine for UBO structures.

## Files to Create

1. `rust/crates/ob-poc-ui/src/layout/mod.rs`
2. `rust/crates/ob-poc-ui/src/layout/engine.rs` - Trait definition
3. `rust/crates/ob-poc-ui/src/layout/pyramid.rs` - Pyramid implementation
4. `rust/crates/ob-poc-ui/src/layout/positioned.rs` - Output types

## Layout Engine Trait

### engine.rs
```rust
use egui::{Pos2, Vec2, Rect};

pub trait LayoutEngine {
    fn layout(
        &self,
        graph: &SemanticGraph,
        config: &TaxonomyLayoutConfig,
        viewport: Rect,
    ) -> PositionedGraph;
}
```

## Output Types

### positioned.rs
```rust
use egui::{Pos2, Vec2, Rect, Color32};

#[derive(Debug, Clone)]
pub struct PositionedGraph {
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<PositionedEdge>,
    pub floating_zone: Option<FloatingZoneLayout>,
    pub bounds: Rect,
}

#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub id: EntityId,
    pub name: String,
    pub entity_type: String,
    pub position: Pos2,
    pub size: Vec2,
    pub level: u32,
    pub style: NodeStyle,
    pub is_floating: bool,
    pub can_drill_down: bool,
}

#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub fill_color: Color32,
    pub stroke_color: Color32,
    pub stroke_width: f32,
    pub shape: NodeShape,
    pub font_size: f32,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            fill_color: Color32::from_rgb(74, 144, 217),
            stroke_color: Color32::from_rgb(44, 114, 187),
            stroke_width: 2.0,
            shape: NodeShape::Rectangle,
            font_size: 12.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NodeShape {
    Circle,
    Rectangle,
    Diamond,
    Hexagon,
}

#[derive(Debug, Clone)]
pub struct PositionedEdge {
    pub source: EntityId,
    pub target: EntityId,
    pub edge_type: String,
    pub path: Vec<Pos2>,  // Control points for bezier/polyline
    pub style: EdgeStyle,
}

#[derive(Debug, Clone)]
pub struct EdgeStyle {
    pub color: Color32,
    pub width: f32,
    pub dashed: bool,
    pub arrow: bool,
}

impl Default for EdgeStyle {
    fn default() -> Self {
        Self {
            color: Color32::from_rgb(100, 100, 100),
            width: 1.5,
            dashed: false,
            arrow: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FloatingZoneLayout {
    pub bounds: Rect,
    pub label: Option<String>,
    pub node_ids: Vec<EntityId>,
}
```

## Pyramid Layout Implementation

### pyramid.rs
```rust
use std::collections::{HashMap, HashSet, VecDeque};
use egui::{Pos2, Vec2, Rect, pos2, vec2};

pub struct PyramidLayout;

impl PyramidLayout {
    pub fn new() -> Self {
        Self
    }
}

impl LayoutEngine for PyramidLayout {
    fn layout(
        &self,
        graph: &SemanticGraph,
        config: &TaxonomyLayoutConfig,
        viewport: Rect,
    ) -> PositionedGraph {
        let pyramid_config = config.pyramid_config();
        
        // 1. Partition connected vs floating nodes
        let (connected, floating) = partition_by_connectivity(graph);
        
        // 2. Assign levels to connected nodes
        let levels = assign_levels(&connected, graph, &config.rank_rules);
        
        // 3. Group nodes by level
        let by_level = group_by_level(&connected, &levels);
        let max_level = by_level.keys().max().copied().unwrap_or(0);
        
        // 4. Order nodes within each level (crossing minimization)
        let ordered = order_within_levels(&by_level, graph);
        
        // 5. Compute positions
        let positioned_nodes = compute_positions(
            &ordered,
            &levels,
            graph,
            config,
            viewport,
            max_level,
        );
        
        // 6. Layout floating zone
        let (floating_zone, floating_nodes) = layout_floating_zone(
            &floating,
            graph,
            config,
            viewport,
        );
        
        // 7. Route edges
        let edges = route_edges(graph, &positioned_nodes, config);
        
        // Combine all nodes
        let mut all_nodes = positioned_nodes;
        all_nodes.extend(floating_nodes);
        
        // Compute bounds
        let bounds = compute_bounds(&all_nodes, &floating_zone);
        
        PositionedGraph {
            nodes: all_nodes,
            edges,
            floating_zone,
            bounds,
        }
    }
}

/// Separate nodes into connected (have structural edges) and floating (orphans)
fn partition_by_connectivity(graph: &SemanticGraph) -> (Vec<&Node>, Vec<&Node>) {
    let mut connected = vec![];
    let mut floating = vec![];
    
    for node in &graph.nodes {
        let has_structural = graph.edges.iter().any(|e| {
            (e.source == node.id || e.target == node.id)
                && is_structural_edge(&e.edge_type)
        });
        
        if has_structural {
            connected.push(node);
        } else {
            floating.push(node);
        }
    }
    
    (connected, floating)
}

fn is_structural_edge(edge_type: &str) -> bool {
    matches!(
        edge_type.to_uppercase().as_str(),
        "OWNS" | "CONTROLS" | "BENEFICIAL_OWNER" | "SHAREHOLDER" |
        "PARENT" | "SUBSIDIARY" | "DIRECTOR" | "SIGNATORY"
    )
}

/// Assign hierarchy levels based on rank rules
fn assign_levels(
    nodes: &[&Node],
    graph: &SemanticGraph,
    rank_rules: &HashMap<String, RankRule>,
) -> HashMap<EntityId, u32> {
    let mut levels = HashMap::new();
    
    // First pass: assign fixed ranks
    for node in nodes {
        if let Some(rule) = rank_rules.get(&node.entity_type) {
            match &rule.rank {
                RankAssignment::Fixed(level) => {
                    levels.insert(node.id, *level);
                }
                _ => {}
            }
        }
    }
    
    // Find root nodes (no incoming structural edges, or rank 0)
    let roots: Vec<EntityId> = nodes.iter()
        .filter(|n| {
            levels.get(&n.id) == Some(&0) ||
            !graph.edges.iter().any(|e| 
                e.target == n.id && is_structural_edge(&e.edge_type)
            )
        })
        .map(|n| n.id)
        .collect();
    
    // BFS from roots to assign derived levels
    let mut queue = VecDeque::new();
    for root in &roots {
        if !levels.contains_key(root) {
            levels.insert(*root, 0);
        }
        queue.push_back(*root);
    }
    
    let mut visited = HashSet::new();
    while let Some(node_id) = queue.pop_front() {
        if visited.contains(&node_id) {
            continue;
        }
        visited.insert(node_id);
        
        let current_level = levels.get(&node_id).copied().unwrap_or(0);
        
        // Find children (nodes this one points to)
        for edge in &graph.edges {
            if edge.source == node_id && is_structural_edge(&edge.edge_type) {
                let child_id = edge.target;
                
                // Only assign if not already assigned or if we found a shorter path
                let new_level = current_level + 1;
                let existing = levels.get(&child_id).copied();
                
                if existing.is_none() || existing.unwrap() > new_level {
                    levels.insert(child_id, new_level);
                }
                
                if !visited.contains(&child_id) {
                    queue.push_back(child_id);
                }
            }
        }
    }
    
    // Handle leaf rank assignment
    let max_level = levels.values().max().copied().unwrap_or(0);
    for node in nodes {
        if let Some(rule) = rank_rules.get(&node.entity_type) {
            if matches!(rule.rank, RankAssignment::Named(RankName::Leaf)) {
                levels.insert(node.id, max_level);
            }
        }
    }
    
    levels
}

/// Group nodes by their assigned level
fn group_by_level<'a>(
    nodes: &[&'a Node],
    levels: &HashMap<EntityId, u32>,
) -> HashMap<u32, Vec<&'a Node>> {
    let mut by_level: HashMap<u32, Vec<&Node>> = HashMap::new();
    
    for node in nodes {
        let level = levels.get(&node.id).copied().unwrap_or(0);
        by_level.entry(level).or_default().push(*node);
    }
    
    by_level
}

/// Order nodes within each level to minimize edge crossings
/// Uses barycenter heuristic
fn order_within_levels<'a>(
    by_level: &HashMap<u32, Vec<&'a Node>>,
    graph: &SemanticGraph,
) -> Vec<Vec<&'a Node>> {
    let max_level = by_level.keys().max().copied().unwrap_or(0);
    let mut ordered: Vec<Vec<&Node>> = vec![vec![]; (max_level + 1) as usize];
    
    // Initialize with current order
    for (level, nodes) in by_level {
        ordered[*level as usize] = nodes.clone();
    }
    
    // Barycenter method: sweep down then up, multiple times
    for _ in 0..4 {
        // Sweep down
        for level in 1..=max_level {
            let level_idx = level as usize;
            let prev_level = &ordered[level_idx - 1];
            
            // Compute barycenter for each node
            let mut barycenters: Vec<(usize, f32)> = ordered[level_idx]
                .iter()
                .enumerate()
                .map(|(i, node)| {
                    let bc = compute_barycenter(node.id, prev_level, graph);
                    (i, bc)
                })
                .collect();
            
            // Sort by barycenter
            barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            // Reorder
            let old_order = ordered[level_idx].clone();
            for (new_pos, (old_pos, _)) in barycenters.iter().enumerate() {
                ordered[level_idx][new_pos] = old_order[*old_pos];
            }
        }
        
        // Sweep up (similar, but looking at next level)
        for level in (0..max_level).rev() {
            let level_idx = level as usize;
            let next_level = &ordered[level_idx + 1];
            
            let mut barycenters: Vec<(usize, f32)> = ordered[level_idx]
                .iter()
                .enumerate()
                .map(|(i, node)| {
                    let bc = compute_barycenter_down(node.id, next_level, graph);
                    (i, bc)
                })
                .collect();
            
            barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            let old_order = ordered[level_idx].clone();
            for (new_pos, (old_pos, _)) in barycenters.iter().enumerate() {
                ordered[level_idx][new_pos] = old_order[*old_pos];
            }
        }
    }
    
    ordered
}

/// Compute barycenter (average position of connected nodes in previous level)
fn compute_barycenter(
    node_id: EntityId,
    prev_level: &[&Node],
    graph: &SemanticGraph,
) -> f32 {
    let mut positions = vec![];
    
    for (pos, prev_node) in prev_level.iter().enumerate() {
        // Check if there's an edge between prev_node and node_id
        let connected = graph.edges.iter().any(|e| {
            (e.source == prev_node.id && e.target == node_id) ||
            (e.target == prev_node.id && e.source == node_id)
        });
        
        if connected {
            positions.push(pos as f32);
        }
    }
    
    if positions.is_empty() {
        f32::MAX  // No connections, put at end
    } else {
        positions.iter().sum::<f32>() / positions.len() as f32
    }
}

fn compute_barycenter_down(
    node_id: EntityId,
    next_level: &[&Node],
    graph: &SemanticGraph,
) -> f32 {
    compute_barycenter(node_id, next_level, graph)
}

/// Compute final positions for all nodes
fn compute_positions(
    ordered: &[Vec<&Node>],
    levels: &HashMap<EntityId, u32>,
    graph: &SemanticGraph,
    config: &TaxonomyLayoutConfig,
    viewport: Rect,
    max_level: u32,
) -> Vec<PositionedNode> {
    let pyramid_config = config.pyramid_config();
    let mut positioned = vec![];
    
    let apex_x = viewport.center().x;
    let start_y = match pyramid_config.direction {
        Direction::TopDown => viewport.top() + 50.0,
        Direction::BottomUp => viewport.bottom() - 50.0,
        _ => viewport.top() + 50.0,
    };
    
    for (level_idx, nodes) in ordered.iter().enumerate() {
        if nodes.is_empty() {
            continue;
        }
        
        let level = level_idx as u32;
        let y = match pyramid_config.direction {
            Direction::TopDown => start_y + (level as f32 * pyramid_config.level_spacing),
            Direction::BottomUp => start_y - (level as f32 * pyramid_config.level_spacing),
            _ => start_y + (level as f32 * pyramid_config.level_spacing),
        };
        
        // Pyramid expansion: each level gets wider
        let expansion = pyramid_config.pyramid_expansion.powi(level as i32);
        let spacing = pyramid_config.sibling_spacing * expansion;
        let total_width = (nodes.len().saturating_sub(1)) as f32 * spacing;
        let start_x = apex_x - total_width / 2.0;
        
        for (idx, node) in nodes.iter().enumerate() {
            let x = start_x + idx as f32 * spacing;
            
            let style = config.node_styles
                .get(&node.entity_type)
                .map(|s| node_style_from_spec(s))
                .unwrap_or_default();
            
            let size = config.node_styles
                .get(&node.entity_type)
                .and_then(|s| s.size)
                .map(|[w, h]| vec2(w, h))
                .unwrap_or(vec2(120.0, 50.0));
            
            positioned.push(PositionedNode {
                id: node.id,
                name: node.name.clone(),
                entity_type: node.entity_type.clone(),
                position: pos2(x, y),
                size,
                level,
                style,
                is_floating: false,
                can_drill_down: node.entity_type == "CBU",
            });
        }
    }
    
    positioned
}

/// Layout the floating zone (right gutter)
fn layout_floating_zone(
    floating: &[&Node],
    graph: &SemanticGraph,
    config: &TaxonomyLayoutConfig,
    viewport: Rect,
) -> (Option<FloatingZoneLayout>, Vec<PositionedNode>) {
    if floating.is_empty() {
        return (None, vec![]);
    }
    
    let zone_spec = config.floating_zone.as_ref();
    let max_width = zone_spec.map(|z| z.max_width).unwrap_or(200.0);
    let label = zone_spec.and_then(|z| z.label.clone());
    
    // Position in right gutter
    let zone_x = viewport.right() - max_width - 20.0;
    let zone_top = viewport.top() + 60.0;
    
    let mut nodes = vec![];
    let mut y = zone_top + 30.0;  // Leave room for label
    
    for node in floating {
        let size = vec2(max_width - 20.0, 40.0);
        
        nodes.push(PositionedNode {
            id: node.id,
            name: node.name.clone(),
            entity_type: node.entity_type.clone(),
            position: pos2(zone_x + max_width / 2.0, y + size.y / 2.0),
            size,
            level: u32::MAX,  // No level for floating
            style: NodeStyle {
                fill_color: Color32::from_rgb(155, 89, 182),
                ..Default::default()
            },
            is_floating: true,
            can_drill_down: false,
        });
        
        y += size.y + 10.0;  // Spacing between floating nodes
    }
    
    let zone_height = y - zone_top + 20.0;
    let bounds = Rect::from_min_size(
        pos2(zone_x, zone_top),
        vec2(max_width, zone_height),
    );
    
    let zone = FloatingZoneLayout {
        bounds,
        label,
        node_ids: floating.iter().map(|n| n.id).collect(),
    };
    
    (Some(zone), nodes)
}

/// Route edges between positioned nodes
fn route_edges(
    graph: &SemanticGraph,
    nodes: &[PositionedNode],
    config: &TaxonomyLayoutConfig,
) -> Vec<PositionedEdge> {
    let node_positions: HashMap<EntityId, &PositionedNode> = nodes
        .iter()
        .map(|n| (n.id, n))
        .collect();
    
    let mut edges = vec![];
    
    for edge in &graph.edges {
        let source = match node_positions.get(&edge.source) {
            Some(n) => n,
            None => continue,
        };
        let target = match node_positions.get(&edge.target) {
            Some(n) => n,
            None => continue,
        };
        
        // Simple straight line for adjacent levels
        // Could add bezier curves for multi-level spans
        let source_bottom = pos2(source.position.x, source.position.y + source.size.y / 2.0);
        let target_top = pos2(target.position.x, target.position.y - target.size.y / 2.0);
        
        let path = if (source.level as i32 - target.level as i32).abs() <= 1 {
            // Adjacent levels: straight line
            vec![source_bottom, target_top]
        } else {
            // Multi-level span: add control point for curve
            let mid_y = (source_bottom.y + target_top.y) / 2.0;
            let mid_x = (source_bottom.x + target_top.x) / 2.0;
            vec![
                source_bottom,
                pos2(mid_x, mid_y),
                target_top,
            ]
        };
        
        let style = config.edge_topology
            .get(&edge.edge_type)
            .map(|t| EdgeStyle {
                dashed: matches!(t.direction, TopologyDirection::Horizontal),
                ..Default::default()
            })
            .unwrap_or_default();
        
        edges.push(PositionedEdge {
            source: edge.source,
            target: edge.target,
            edge_type: edge.edge_type.clone(),
            path,
            style,
        });
    }
    
    edges
}

/// Compute bounding box for entire layout
fn compute_bounds(
    nodes: &[PositionedNode],
    floating_zone: &Option<FloatingZoneLayout>,
) -> Rect {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    
    for node in nodes {
        let left = node.position.x - node.size.x / 2.0;
        let right = node.position.x + node.size.x / 2.0;
        let top = node.position.y - node.size.y / 2.0;
        let bottom = node.position.y + node.size.y / 2.0;
        
        min_x = min_x.min(left);
        max_x = max_x.max(right);
        min_y = min_y.min(top);
        max_y = max_y.max(bottom);
    }
    
    if let Some(fz) = floating_zone {
        min_x = min_x.min(fz.bounds.left());
        max_x = max_x.max(fz.bounds.right());
        min_y = min_y.min(fz.bounds.top());
        max_y = max_y.max(fz.bounds.bottom());
    }
    
    Rect::from_min_max(pos2(min_x, min_y), pos2(max_x, max_y))
}

fn node_style_from_spec(spec: &NodeStyleSpec) -> NodeStyle {
    NodeStyle {
        fill_color: spec.color
            .as_ref()
            .and_then(|c| parse_hex_color(c))
            .unwrap_or(Color32::from_rgb(74, 144, 217)),
        shape: spec.shape.clone().unwrap_or(NodeShape::Rectangle),
        ..Default::default()
    }
}

fn parse_hex_color(hex: &str) -> Option<Color32> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pyramid_layout_basic() {
        let graph = create_simple_ubo_graph();
        let config = load_ubo_config();
        let viewport = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        
        let layout = PyramidLayout::new();
        let result = layout.layout(&graph, &config, viewport);
        
        // UBO should be at top (level 0)
        let ubo = result.nodes.iter().find(|n| n.entity_type == "UBO").unwrap();
        assert_eq!(ubo.level, 0);
        
        // Subject should be at bottom
        let subject = result.nodes.iter().find(|n| n.entity_type == "SUBJECT").unwrap();
        assert!(subject.level > ubo.level);
    }
    
    #[test]
    fn test_floating_zone() {
        let mut graph = create_simple_ubo_graph();
        // Add orphan person
        graph.nodes.push(Node {
            id: EntityId(999),
            name: "Orphan Person".to_string(),
            entity_type: "PERSON".to_string(),
            ..Default::default()
        });
        
        let config = load_ubo_config();
        let viewport = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        
        let layout = PyramidLayout::new();
        let result = layout.layout(&graph, &config, viewport);
        
        assert!(result.floating_zone.is_some());
        let fz = result.floating_zone.unwrap();
        assert!(fz.node_ids.contains(&EntityId(999)));
    }
    
    #[test]
    fn test_deterministic_layout() {
        let graph = create_simple_ubo_graph();
        let config = load_ubo_config();
        let viewport = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        
        let layout = PyramidLayout::new();
        let result1 = layout.layout(&graph, &config, viewport);
        let result2 = layout.layout(&graph, &config, viewport);
        
        // Same input should produce same output
        for (n1, n2) in result1.nodes.iter().zip(result2.nodes.iter()) {
            assert_eq!(n1.position, n2.position);
        }
    }
}
```

## Acceptance Criteria

- [ ] Nodes assigned to correct levels by rank rules
- [ ] Floating nodes separated to gutter zone
- [ ] Pyramid widens at each level per expansion factor
- [ ] Edges route cleanly between nodes
- [ ] Layout is deterministic (same input → same output)
- [ ] Barycenter method reduces edge crossings

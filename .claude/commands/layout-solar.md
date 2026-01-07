# Solar System Layout Implementation

Implement the solar system (orbital/ring) layout for large entity universes.

## File to Create

`rust/crates/ob-poc-ui/src/layout/solar_system.rs`

## Implementation

```rust
use std::collections::HashMap;
use std::f32::consts::TAU;
use egui::{Pos2, Vec2, Rect, pos2, vec2};
use rand::Rng;

pub struct SolarSystemLayout;

impl SolarSystemLayout {
    pub fn new() -> Self {
        Self
    }
}

impl LayoutEngine for SolarSystemLayout {
    fn layout(
        &self,
        graph: &SemanticGraph,
        config: &TaxonomyLayoutConfig,
        viewport: Rect,
    ) -> PositionedGraph {
        let solar_config = config.solar_system_config();
        let center_pos = viewport.center();
        
        // 1. Select center node
        let center_node_id = select_center(graph, &solar_config.center);
        
        // 2. Assign nodes to rings
        let ring_assignments = assign_to_rings(graph, center_node_id, &config.rings);
        
        // 3. Position center node
        let mut positioned_nodes = vec![];
        
        if let Some(center) = graph.nodes.iter().find(|n| n.id == center_node_id) {
            positioned_nodes.push(PositionedNode {
                id: center.id,
                name: center.name.clone(),
                entity_type: center.entity_type.clone(),
                position: center_pos,
                size: vec2(80.0, 80.0),
                level: 0,
                ring: Some("center".to_string()),
                style: NodeStyle {
                    fill_color: Color32::from_rgb(241, 196, 15),  // Gold for center
                    shape: NodeShape::Circle,
                    ..Default::default()
                },
                is_floating: false,
                can_drill_down: true,
            });
        }
        
        // 4. Position nodes on each ring
        for (ring_idx, ring_def) in config.rings.iter().enumerate() {
            let nodes_in_ring = ring_assignments.get(ring_idx).cloned().unwrap_or_default();
            
            let ring_nodes = position_on_ring(
                graph,
                center_pos,
                ring_def,
                &nodes_in_ring,
                &solar_config.rotation,
            );
            positioned_nodes.extend(ring_nodes);
        }
        
        // 5. Route edges
        let edges = route_orbital_edges(graph, &positioned_nodes, center_pos);
        
        // 6. Compute bounds
        let bounds = compute_orbital_bounds(&positioned_nodes, &config.rings);
        
        PositionedGraph {
            nodes: positioned_nodes,
            edges,
            floating_zone: None,  // Asteroid belt is a ring, not a zone
            bounds,
        }
    }
}

/// Select the center node based on configuration
fn select_center(graph: &SemanticGraph, selection: &CenterSelection) -> EntityId {
    match selection {
        CenterSelection::FocalEntity => {
            // Look for a node marked as focal, or first CBU
            graph.nodes.iter()
                .find(|n| n.entity_type == "CBU")
                .map(|n| n.id)
                .unwrap_or_else(|| graph.nodes.first().map(|n| n.id).unwrap_or(EntityId(0)))
        }
        CenterSelection::HighestDegree => {
            // Find node with most edges
            let mut degree_count: HashMap<EntityId, usize> = HashMap::new();
            for edge in &graph.edges {
                *degree_count.entry(edge.source).or_default() += 1;
                *degree_count.entry(edge.target).or_default() += 1;
            }
            degree_count.into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(id, _)| id)
                .unwrap_or(EntityId(0))
        }
        CenterSelection::SelectedNode => {
            // Would come from session context focal_entity
            graph.nodes.first().map(|n| n.id).unwrap_or(EntityId(0))
        }
        CenterSelection::ByNodeType(node_type) => {
            graph.nodes.iter()
                .find(|n| &n.entity_type == node_type)
                .map(|n| n.id)
                .unwrap_or(EntityId(0))
        }
    }
}

/// Assign nodes to rings based on filters
fn assign_to_rings(
    graph: &SemanticGraph,
    center_id: EntityId,
    rings: &[RingDefinition],
) -> Vec<Vec<EntityId>> {
    let mut assignments: Vec<Vec<EntityId>> = vec![vec![]; rings.len()];
    let mut assigned: std::collections::HashSet<EntityId> = std::collections::HashSet::new();
    assigned.insert(center_id);  // Center is not on any ring
    
    for node in &graph.nodes {
        if assigned.contains(&node.id) {
            continue;
        }
        
        // Find first matching ring
        for (ring_idx, ring) in rings.iter().enumerate() {
            if matches_ring_filter(node, &ring.filter, graph, center_id) {
                assignments[ring_idx].push(node.id);
                assigned.insert(node.id);
                break;
            }
        }
    }
    
    // Any unassigned nodes go to last ring (asteroid belt)
    for node in &graph.nodes {
        if !assigned.contains(&node.id) {
            if let Some(last) = assignments.last_mut() {
                last.push(node.id);
            }
        }
    }
    
    assignments
}

/// Check if a node matches a ring's filter
fn matches_ring_filter(
    node: &Node,
    filter: &RingFilter,
    graph: &SemanticGraph,
    center_id: EntityId,
) -> bool {
    match filter {
        RingFilter::RoleIn { role_in } => {
            node.role.as_ref().map_or(false, |r| {
                role_in.iter().any(|allowed| allowed.eq_ignore_ascii_case(r))
            })
        }
        RingFilter::EdgeTypeIn { edge_type_in } => {
            graph.edges.iter().any(|e| {
                (e.source == node.id || e.target == node.id) &&
                edge_type_in.iter().any(|t| t.eq_ignore_ascii_case(&e.edge_type))
            })
        }
        RingFilter::HopDistance { hop_distance } => {
            compute_hop_distance(graph, center_id, node.id) == Some(*hop_distance)
        }
        RingFilter::Floating => {
            !graph.edges.iter().any(|e| e.source == node.id || e.target == node.id)
        }
        RingFilter::Custom(_expr) => {
            // Would need DSL evaluation
            false
        }
    }
}

/// Compute shortest path length between two nodes
fn compute_hop_distance(
    graph: &SemanticGraph,
    from: EntityId,
    to: EntityId,
) -> Option<u32> {
    if from == to {
        return Some(0);
    }
    
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((from, 0u32));
    
    while let Some((current, dist)) = queue.pop_front() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current);
        
        for edge in &graph.edges {
            let neighbor = if edge.source == current {
                edge.target
            } else if edge.target == current {
                edge.source
            } else {
                continue;
            };
            
            if neighbor == to {
                return Some(dist + 1);
            }
            
            if !visited.contains(&neighbor) {
                queue.push_back((neighbor, dist + 1));
            }
        }
    }
    
    None
}

/// Position nodes evenly around a ring
fn position_on_ring(
    graph: &SemanticGraph,
    center: Pos2,
    ring: &RingDefinition,
    node_ids: &[EntityId],
    rotation: &RotationStrategy,
) -> Vec<PositionedNode> {
    if node_ids.is_empty() {
        return vec![];
    }
    
    let node_map: HashMap<EntityId, &Node> = graph.nodes.iter()
        .map(|n| (n.id, n))
        .collect();
    
    let count = node_ids.len();
    let base_angle_step = TAU / count as f32;
    
    // Sort nodes based on rotation strategy
    let sorted_ids: Vec<EntityId> = match rotation {
        RotationStrategy::Alphabetical => {
            let mut ids = node_ids.to_vec();
            ids.sort_by(|a, b| {
                let name_a = node_map.get(a).map(|n| &n.name).unwrap_or(&String::new());
                let name_b = node_map.get(b).map(|n| &n.name).unwrap_or(&String::new());
                name_a.cmp(name_b)
            });
            ids
        }
        _ => node_ids.to_vec(),
    };
    
    sorted_ids.iter().enumerate().filter_map(|(i, &node_id)| {
        let node = node_map.get(&node_id)?;
        
        let base_angle = i as f32 * base_angle_step;
        
        // Apply style variations
        let (angle, radius) = match ring.style {
            RingStyle::Evenly => (base_angle, ring.radius),
            RingStyle::Scattered => {
                // Asteroid belt effect - randomize within band
                let mut rng = rand::thread_rng();
                let angle_jitter = (rng.gen::<f32>() - 0.5) * 0.3;
                let radius_jitter = (rng.gen::<f32>() - 0.5) * 40.0;
                (base_angle + angle_jitter, ring.radius + radius_jitter)
            }
            RingStyle::Clustered => {
                // Group by some attribute - for now just evenly
                (base_angle, ring.radius)
            }
        };
        
        let position = center + Vec2::angled(angle) * radius;
        
        let size = match &ring.style {
            RingStyle::Scattered => vec2(30.0, 30.0),  // Smaller for asteroid belt
            _ => vec2(50.0, 50.0),
        };
        
        let color = match &ring.name.as_str() {
            &"core" => Color32::from_rgb(231, 76, 60),    // Red for core
            &"inner" => Color32::from_rgb(52, 152, 219), // Blue for inner
            &"outer" => Color32::from_rgb(46, 204, 113), // Green for outer
            &"asteroid_belt" => Color32::from_rgb(149, 165, 166), // Gray for asteroids
            _ => Color32::from_rgb(155, 89, 182),
        };
        
        Some(PositionedNode {
            id: node.id,
            name: node.name.clone(),
            entity_type: node.entity_type.clone(),
            position,
            size,
            level: 0,  // Not used in solar system
            ring: Some(ring.name.clone()),
            style: NodeStyle {
                fill_color: color,
                shape: NodeShape::Circle,
                ..Default::default()
            },
            is_floating: matches!(ring.style, RingStyle::Scattered),
            can_drill_down: node.entity_type == "CBU",
        })
    }).collect()
}

/// Route edges with curves for cross-ring connections
fn route_orbital_edges(
    graph: &SemanticGraph,
    nodes: &[PositionedNode],
    center: Pos2,
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
        
        // Check if same ring
        let same_ring = source.ring == target.ring;
        
        let path = if same_ring {
            // Arc along the ring (simplified as straight line for now)
            vec![source.position, target.position]
        } else {
            // Curve through or near center
            let control = if source.ring.as_deref() == Some("center") || 
                          target.ring.as_deref() == Some("center") {
                // Direct line if one is center
                vec![source.position, target.position]
            } else {
                // Curve via control point between source, center, and target
                let mid = pos2(
                    (source.position.x + target.position.x + center.x) / 3.0,
                    (source.position.y + target.position.y + center.y) / 3.0,
                );
                vec![source.position, mid, target.position]
            };
            control
        };
        
        edges.push(PositionedEdge {
            source: edge.source,
            target: edge.target,
            edge_type: edge.edge_type.clone(),
            path,
            style: EdgeStyle {
                color: Color32::from_rgba_unmultiplied(100, 100, 100, 128),
                ..Default::default()
            },
        });
    }
    
    edges
}

/// Compute bounds including all rings
fn compute_orbital_bounds(
    nodes: &[PositionedNode],
    rings: &[RingDefinition],
) -> Rect {
    if nodes.is_empty() {
        return Rect::NOTHING;
    }
    
    // Find center (should be first node)
    let center = nodes.first().map(|n| n.position).unwrap_or(pos2(0.0, 0.0));
    
    // Max radius is outermost ring
    let max_radius = rings.iter()
        .map(|r| r.radius)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(100.0);
    
    let padding = 100.0;
    let size = (max_radius + padding) * 2.0;
    
    Rect::from_center_size(center, vec2(size, size))
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_center_selection_highest_degree() {
        let mut graph = SemanticGraph::default();
        
        // Node A: 3 connections
        // Node B: 1 connection
        // Node C: 2 connections
        graph.nodes.push(Node { id: EntityId(1), name: "A".into(), ..Default::default() });
        graph.nodes.push(Node { id: EntityId(2), name: "B".into(), ..Default::default() });
        graph.nodes.push(Node { id: EntityId(3), name: "C".into(), ..Default::default() });
        
        graph.edges.push(Edge { source: EntityId(1), target: EntityId(2), ..Default::default() });
        graph.edges.push(Edge { source: EntityId(1), target: EntityId(3), ..Default::default() });
        graph.edges.push(Edge { source: EntityId(3), target: EntityId(1), ..Default::default() });
        
        let center = select_center(&graph, &CenterSelection::HighestDegree);
        assert_eq!(center, EntityId(1));  // A has highest degree
    }
    
    #[test]
    fn test_ring_assignment() {
        let graph = create_test_graph_with_roles();
        let rings = vec![
            RingDefinition {
                name: "core".to_string(),
                filter: RingFilter::RoleIn { role_in: vec!["UBO".to_string()] },
                radius: 100.0,
                style: RingStyle::Evenly,
            },
            RingDefinition {
                name: "outer".to_string(),
                filter: RingFilter::Floating,
                radius: 200.0,
                style: RingStyle::Scattered,
            },
        ];
        
        let assignments = assign_to_rings(&graph, EntityId(0), &rings);
        
        // UBO nodes should be in core ring
        assert!(!assignments[0].is_empty());
    }
}
```

## Acceptance Criteria

- [ ] Center node positioned at viewport center
- [ ] Nodes correctly assigned to rings by filter
- [ ] Asteroid belt contains floating entities with scattered style
- [ ] Edges curve elegantly between rings
- [ ] Alphabetical rotation orders nodes correctly
- [ ] Layout scales with viewport size

//! Shared scene layout and hit geometry.
//!
//! Layout is derived once per scene update and then reused by paint,
//! hit testing, tooltips, minimap, and anchor/focus overlays.

use std::collections::HashMap;
use std::f32::consts::TAU;

use egui::{Pos2, Rect, Vec2};

use ob_poc_types::graph_scene::{
    GraphSceneModel, LayoutStrategy, SceneEdge, SceneNode, SceneNodeType,
};

/// Screen-independent geometry for a scene node.
#[derive(Debug, Clone)]
pub struct NodeGeometry {
    pub center: Pos2,
    pub hit_shape: HitShape,
}

/// Resolved edge endpoints for fast paint-time access.
#[derive(Debug, Clone, Copy)]
pub struct EdgeGeometry {
    pub source_idx: usize,
    pub target_idx: usize,
}

/// Shape used for world-space hit testing.
#[derive(Debug, Clone, Copy)]
pub enum HitShape {
    Circle { radius: f32 },
    Rect { half_size: Vec2 },
}

impl HitShape {
    fn contains(self, center: Pos2, point: Pos2) -> bool {
        match self {
            Self::Circle { radius } => center.distance(point) <= radius,
            Self::Rect { half_size } => {
                (point.x - center.x).abs() <= half_size.x
                    && (point.y - center.y).abs() <= half_size.y
            }
        }
    }

    fn distance_to(self, center: Pos2, point: Pos2) -> f32 {
        match self {
            Self::Circle { .. } => center.distance(point),
            Self::Rect { half_size } => {
                let dx = (point.x - center.x).abs() - half_size.x;
                let dy = (point.y - center.y).abs() - half_size.y;
                dx.max(0.0).hypot(dy.max(0.0))
            }
        }
    }

    fn bounds(self, center: Pos2) -> Rect {
        match self {
            Self::Circle { radius } => Rect::from_center_size(center, Vec2::splat(radius * 2.0)),
            Self::Rect { half_size } => Rect::from_center_size(center, half_size * 2.0),
        }
    }
}

/// Derived scene layout cache.
#[derive(Debug, Clone)]
pub struct LayoutCache {
    pub generation: u64,
    pub strategy: LayoutStrategy,
    pub nodes: Vec<NodeGeometry>,
    pub edges: Vec<EdgeGeometry>,
    pub node_indices: HashMap<String, usize>,
    pub world_bounds: Rect,
}

impl LayoutCache {
    /// Build a fresh cache from the scene payload.
    pub fn derive(scene: &GraphSceneModel) -> Self {
        let positions = compute_positions(scene);
        let node_indices: HashMap<String, usize> = scene
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (node.id.clone(), idx))
            .collect();
        let nodes: Vec<NodeGeometry> = scene
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| NodeGeometry {
                center: Pos2::new(positions[idx].0, positions[idx].1),
                hit_shape: hit_shape(scene.layout_strategy, node),
            })
            .collect();
        let edges = scene
            .edges
            .iter()
            .filter_map(|edge| {
                let source_idx = node_indices.get(&edge.source).copied()?;
                let target_idx = node_indices.get(&edge.target).copied()?;
                Some(EdgeGeometry {
                    source_idx,
                    target_idx,
                })
            })
            .collect();

        let world_bounds = bounds_from_nodes(&nodes);

        Self {
            generation: scene.generation,
            strategy: scene.layout_strategy,
            nodes,
            edges,
            node_indices,
            world_bounds,
        }
    }

    /// Hit-test a world-space point against the derived node geometry.
    pub fn hit_test(&self, scene: &GraphSceneModel, world_pos: Pos2) -> Option<String> {
        let mut best: Option<(usize, f32)> = None;

        for (idx, geom) in self.nodes.iter().enumerate() {
            if geom.hit_shape.contains(geom.center, world_pos) {
                let dist = geom.hit_shape.distance_to(geom.center, world_pos);
                if best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist) {
                    best = Some((idx, dist));
                }
            }
        }

        best.map(|(idx, _)| scene.nodes[idx].id.clone())
    }

    /// Find a node's cached world center.
    pub fn center_for_node(&self, _scene: &GraphSceneModel, node_id: &str) -> Option<Pos2> {
        let idx = self.node_indices.get(node_id).copied()?;
        self.nodes.get(idx).map(|geom| geom.center)
    }

    /// Find a node index by ID.
    pub fn index_for_node(&self, node_id: &str) -> Option<usize> {
        self.node_indices.get(node_id).copied()
    }
}

/// Compute the node positions for the supplied scene.
pub fn compute_positions(scene: &GraphSceneModel) -> Vec<(f32, f32)> {
    match scene.layout_strategy {
        LayoutStrategy::ForceDirected => fruchterman_reingold(&scene.nodes, &scene.edges),
        LayoutStrategy::ForceWithinBoundary => {
            bounded_fruchterman_reingold(&scene.nodes, &scene.edges)
        }
        LayoutStrategy::DeterministicOrbital => compute_orbital_positions(&scene.nodes),
        LayoutStrategy::HierarchicalGraph => compute_hierarchical_positions(&scene.nodes, &scene.edges),
        LayoutStrategy::TreeDag => compute_tree_layout(&scene.nodes, &scene.edges),
        LayoutStrategy::StructuredPanels => positions_from_hints_or_orbital(&scene.nodes),
    }
}

fn hit_shape(strategy: LayoutStrategy, node: &SceneNode) -> HitShape {
    match strategy {
        LayoutStrategy::ForceDirected => HitShape::Circle {
            radius: match node.node_type {
                SceneNodeType::Cluster | SceneNodeType::Aggregate => {
                    20.0 + (node.child_count as f32).sqrt() * 8.0 + 4.0
                }
                _ => 16.0,
            },
        },
        LayoutStrategy::ForceWithinBoundary => HitShape::Circle {
            radius: match node.node_type {
                SceneNodeType::Cbu => 22.0,
                SceneNodeType::Entity => 12.0,
                _ => 16.0,
            },
        },
        LayoutStrategy::DeterministicOrbital => HitShape::Circle {
            radius: match node.node_type {
                SceneNodeType::Cbu => 28.0,
                SceneNodeType::Aggregate => 24.0,
                _ => 18.0,
            },
        },
        LayoutStrategy::HierarchicalGraph => HitShape::Circle {
            radius: if node.depth == 0 {
                32.0
            } else {
                match node.node_type {
                    SceneNodeType::Cbu => 22.0,
                    SceneNodeType::Entity => 18.0,
                    SceneNodeType::Case => 16.0,
                    _ => 18.0,
                }
            },
        },
        LayoutStrategy::TreeDag => HitShape::Rect {
            half_size: Vec2::new(60.0, 18.0),
        },
        LayoutStrategy::StructuredPanels => HitShape::Circle { radius: 18.0 },
    }
}

fn bounds_from_nodes(nodes: &[NodeGeometry]) -> Rect {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for node in nodes {
        let bounds = node.hit_shape.bounds(node.center);
        min_x = min_x.min(bounds.min.x);
        min_y = min_y.min(bounds.min.y);
        max_x = max_x.max(bounds.max.x);
        max_y = max_y.max(bounds.max.y);
    }

    if min_x > max_x {
        return Rect::from_center_size(Pos2::ZERO, Vec2::splat(100.0));
    }

    Rect::from_min_max(
        Pos2::new(min_x - 50.0, min_y - 50.0),
        Pos2::new(max_x + 50.0, max_y + 50.0),
    )
}

fn positions_from_hints_or_orbital(nodes: &[SceneNode]) -> Vec<(f32, f32)> {
    nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| {
            node.position_hint.unwrap_or_else(|| {
                if idx == 0 {
                    (0.0, 0.0)
                } else {
                    let orbital_idx = idx - 1;
                    let ring_capacity = 12;
                    let ring = orbital_idx / ring_capacity;
                    let idx_in_ring = orbital_idx % ring_capacity;
                    let total = nodes.len().saturating_sub(1);
                    let nodes_in_ring = if (ring + 1) * ring_capacity <= total {
                        ring_capacity
                    } else {
                        total.saturating_sub(ring * ring_capacity).max(1)
                    };
                    let ring_radius = 200.0 + (ring as f32) * 150.0;
                    let angle = (idx_in_ring as f32 / nodes_in_ring as f32) * TAU - TAU / 4.0;
                    (angle.cos() * ring_radius, angle.sin() * ring_radius)
                }
            })
        })
        .collect()
}

fn compute_orbital_positions(nodes: &[SceneNode]) -> Vec<(f32, f32)> {
    let mut positions = Vec::with_capacity(nodes.len());

    if nodes.is_empty() {
        return positions;
    }

    positions.push(nodes[0].position_hint.unwrap_or((0.0, 0.0)));

    if nodes.len() == 1 {
        return positions;
    }

    let orbital_nodes = nodes.len() - 1;
    let ring_capacity = 12;
    let ring_count = orbital_nodes.div_ceil(ring_capacity);
    let mut placed = 0;

    for ring in 0..ring_count {
        let ring_radius = 200.0 + (ring as f32) * 150.0;
        let nodes_in_ring = if ring < ring_count - 1 {
            ring_capacity
        } else {
            orbital_nodes - placed
        };

        for j in 0..nodes_in_ring {
            let node_idx = placed + 1;
            if let Some(hint) = nodes[node_idx].position_hint {
                positions.push(hint);
            } else {
                let angle = (j as f32 / nodes_in_ring as f32) * TAU - TAU / 4.0;
                positions.push((angle.cos() * ring_radius, angle.sin() * ring_radius));
            }
            placed += 1;
        }
    }

    positions
}

fn compute_hierarchical_positions(nodes: &[SceneNode], edges: &[SceneEdge]) -> Vec<(f32, f32)> {
    let hinted = positions_from_hints(nodes);
    if hinted.is_some() {
        return hinted.unwrap_or_default();
    }

    let mut positions = vec![(0.0f32, 0.0f32); nodes.len()];
    if nodes.is_empty() {
        return positions;
    }

    let id_to_idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| (node.id.as_str(), idx))
        .collect();

    let mut children = vec![Vec::new(); nodes.len()];
    let mut has_parent = vec![false; nodes.len()];

    for edge in edges {
        if let (Some(&src), Some(&dst)) = (
            id_to_idx.get(edge.source.as_str()),
            id_to_idx.get(edge.target.as_str()),
        ) {
            children[src].push(dst);
            has_parent[dst] = true;
        }
    }

    let roots: Vec<usize> = (0..nodes.len()).filter(|idx| !has_parent[*idx]).collect();
    if roots.is_empty() {
        return depth_bucket_positions(nodes);
    }

    let mut visited = vec![false; nodes.len()];
    let horizontal_gap = 140.0f32;
    let vertical_gap = 150.0f32;
    let mut cursor_x = -((roots.len().saturating_sub(1) as f32) * horizontal_gap) / 2.0;

    for &root in &roots {
        assign_hierarchy_positions(
            root,
            cursor_x,
            0.0,
            &children,
            &mut positions,
            &mut visited,
            horizontal_gap,
            vertical_gap,
        );
        cursor_x += horizontal_gap;
    }

    for (idx, done) in visited.iter().enumerate() {
        if !done {
            positions[idx] = (
                cursor_x,
                nodes[idx].depth as f32 * vertical_gap,
            );
            cursor_x += horizontal_gap;
        }
    }

    positions
}

fn assign_hierarchy_positions(
    idx: usize,
    x: f32,
    y: f32,
    children: &[Vec<usize>],
    positions: &mut [(f32, f32)],
    visited: &mut [bool],
    horizontal_gap: f32,
    vertical_gap: f32,
) {
    if visited[idx] {
        return;
    }
    visited[idx] = true;
    positions[idx] = (x, y);

    let child_count = children[idx].len();
    if child_count == 0 {
        return;
    }

    let total_width = (child_count.saturating_sub(1) as f32) * horizontal_gap;
    let start_x = x - total_width / 2.0;

    for (offset, child_idx) in children[idx].iter().enumerate() {
        assign_hierarchy_positions(
            *child_idx,
            start_x + offset as f32 * horizontal_gap,
            y + vertical_gap,
            children,
            positions,
            visited,
            horizontal_gap,
            vertical_gap,
        );
    }
}

fn depth_bucket_positions(nodes: &[SceneNode]) -> Vec<(f32, f32)> {
    let mut positions = vec![(0.0f32, 0.0f32); nodes.len()];
    if nodes.is_empty() {
        return positions;
    }

    let max_depth = nodes.iter().map(|node| node.depth).max().unwrap_or(0);
    for depth in 0..=max_depth {
        let tier_nodes: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.depth == depth)
            .map(|(idx, _)| idx)
            .collect();

        if tier_nodes.is_empty() {
            continue;
        }

        let total_width = (tier_nodes.len().saturating_sub(1) as f32) * 120.0;
        let start_x = -total_width / 2.0;
        for (offset, idx) in tier_nodes.iter().enumerate() {
            positions[*idx] = (start_x + offset as f32 * 120.0, depth as f32 * 150.0);
        }
    }

    positions
}

fn positions_from_hints(nodes: &[SceneNode]) -> Option<Vec<(f32, f32)>> {
    nodes
        .iter()
        .map(|node| node.position_hint)
        .collect::<Option<Vec<_>>>()
}

fn compute_tree_layout(nodes: &[SceneNode], edges: &[SceneEdge]) -> Vec<(f32, f32)> {
    let n = nodes.len();
    if n == 0 {
        return vec![];
    }

    let id_to_idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let mut children: Vec<Vec<usize>> = vec![vec![]; n];
    let mut has_parent = vec![false; n];

    for edge in edges {
        if let (Some(&pi), Some(&ci)) = (
            id_to_idx.get(edge.source.as_str()),
            id_to_idx.get(edge.target.as_str()),
        ) {
            children[pi].push(ci);
            has_parent[ci] = true;
        }
    }

    let roots: Vec<usize> = (0..n).filter(|&i| !has_parent[i]).collect();
    let roots = if roots.is_empty() { vec![0] } else { roots };

    let mut widths = vec![1.0f32; n];
    let mut visited = vec![false; n];

    fn compute_width(
        idx: usize,
        children: &[Vec<usize>],
        widths: &mut [f32],
        visited: &mut [bool],
    ) {
        if visited[idx] {
            return;
        }
        visited[idx] = true;
        if children[idx].is_empty() {
            widths[idx] = 1.0;
            return;
        }
        let mut total = 0.0f32;
        for &c in &children[idx] {
            compute_width(c, children, widths, visited);
            total += widths[c];
        }
        widths[idx] = total.max(1.0);
    }

    for &root in &roots {
        compute_width(root, &children, &mut widths, &mut visited);
    }

    let mut positions = vec![(0.0f32, 0.0f32); n];
    let mut assigned = vec![false; n];

    fn assign_positions(
        idx: usize,
        x: f32,
        y: f32,
        children: &[Vec<usize>],
        widths: &[f32],
        positions: &mut [(f32, f32)],
        assigned: &mut [bool],
    ) {
        if assigned[idx] {
            return;
        }
        assigned[idx] = true;
        positions[idx] = (x, y);

        if children[idx].is_empty() {
            return;
        }

        let total_width = children[idx].iter().map(|&c| widths[c]).sum::<f32>();
        let mut cursor_x = x - (total_width * 140.0) / 2.0;

        for &c in &children[idx] {
            let child_center_x = cursor_x + (widths[c] * 140.0) / 2.0;
            assign_positions(
                c,
                child_center_x,
                y + 120.0,
                children,
                widths,
                positions,
                assigned,
            );
            cursor_x += widths[c] * 140.0;
        }
    }

    let total_root_width: f32 = roots.iter().map(|&r| widths[r]).sum();
    let mut cursor_x = -(total_root_width * 140.0) / 2.0;
    for &root in &roots {
        let root_center_x = cursor_x + (widths[root] * 140.0) / 2.0;
        assign_positions(
            root,
            root_center_x,
            0.0,
            &children,
            &widths,
            &mut positions,
            &mut assigned,
        );
        cursor_x += widths[root] * 140.0;
    }

    let max_y = positions.iter().map(|(_, y)| *y).fold(0.0f32, f32::max);
    let mut orphan_x = 0.0f32;
    for idx in 0..n {
        if !assigned[idx] {
            positions[idx] = (orphan_x, max_y + 120.0);
            orphan_x += 140.0;
        }
    }

    positions
}

fn fruchterman_reingold(nodes: &[SceneNode], edges: &[SceneEdge]) -> Vec<(f32, f32)> {
    let n = nodes.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![nodes[0].position_hint.unwrap_or((0.0, 0.0))];
    }

    let area = 800.0 * 800.0;
    let k = (area / n as f32).sqrt();
    let mut pos: Vec<(f32, f32)> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            node.position_hint.unwrap_or_else(|| {
                let angle = i as f32 * 2.399;
                let r = 50.0 * (i as f32).sqrt();
                (angle.cos() * r, angle.sin() * r)
            })
        })
        .collect();

    let damping = 0.9f32;
    let iterations = 150;
    let mut temperature = 200.0f32;
    let cooling = temperature / iterations as f32;

    let edge_pairs: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|edge| {
            let src = nodes.iter().position(|node| node.id == edge.source)?;
            let dst = nodes.iter().position(|node| node.id == edge.target)?;
            Some((src, dst))
        })
        .collect();

    for _ in 0..iterations {
        let mut disp = vec![(0.0f32, 0.0f32); n];

        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(0.01);
                let force = (k * k) / dist;
                let fx = (dx / dist) * force;
                let fy = (dy / dist) * force;
                disp[i].0 += fx;
                disp[i].1 += fy;
                disp[j].0 -= fx;
                disp[j].1 -= fy;
            }
        }

        for &(src, dst) in &edge_pairs {
            let dx = pos[src].0 - pos[dst].0;
            let dy = pos[src].1 - pos[dst].1;
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let force = (dist * dist) / k;
            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;
            disp[src].0 -= fx;
            disp[src].1 -= fy;
            disp[dst].0 += fx;
            disp[dst].1 += fy;
        }

        for idx in 0..n {
            let dx = disp[idx].0;
            let dy = disp[idx].1;
            let mag = (dx * dx + dy * dy).sqrt().max(0.01);
            let clamped = mag.min(temperature);
            pos[idx].0 += (dx / mag) * clamped * damping;
            pos[idx].1 += (dy / mag) * clamped * damping;
        }

        temperature -= cooling;
        if temperature < 0.1 {
            break;
        }
    }

    pos
}

fn bounded_fruchterman_reingold(nodes: &[SceneNode], edges: &[SceneEdge]) -> Vec<(f32, f32)> {
    const BOUNDARY_RADIUS: f32 = 400.0;

    let n = nodes.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![nodes[0].position_hint.unwrap_or((0.0, 0.0))];
    }

    let area = (BOUNDARY_RADIUS * 2.0).powi(2);
    let k = (area / n as f32).sqrt();
    let mut pos: Vec<(f32, f32)> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            node.position_hint.unwrap_or_else(|| {
                let angle = i as f32 * 2.399;
                let r = 30.0 * (i as f32).sqrt();
                (angle.cos() * r, angle.sin() * r)
            })
        })
        .collect();

    let edge_pairs: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|edge| {
            let src = nodes.iter().position(|node| node.id == edge.source)?;
            let dst = nodes.iter().position(|node| node.id == edge.target)?;
            Some((src, dst))
        })
        .collect();

    let damping = 0.9f32;
    let iterations = 150;
    let mut temperature = 150.0f32;
    let cooling = temperature / iterations as f32;

    for _ in 0..iterations {
        let mut disp = vec![(0.0f32, 0.0f32); n];

        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(0.01);
                let force = (k * k) / dist;
                let fx = (dx / dist) * force;
                let fy = (dy / dist) * force;
                disp[i].0 += fx;
                disp[i].1 += fy;
                disp[j].0 -= fx;
                disp[j].1 -= fy;
            }
        }

        for &(src, dst) in &edge_pairs {
            let dx = pos[src].0 - pos[dst].0;
            let dy = pos[src].1 - pos[dst].1;
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let force = (dist * dist) / k;
            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;
            disp[src].0 -= fx;
            disp[src].1 -= fy;
            disp[dst].0 += fx;
            disp[dst].1 += fy;
        }

        for idx in 0..n {
            let dx = disp[idx].0;
            let dy = disp[idx].1;
            let mag = (dx * dx + dy * dy).sqrt().max(0.01);
            let clamped = mag.min(temperature);
            pos[idx].0 += (dx / mag) * clamped * damping;
            pos[idx].1 += (dy / mag) * clamped * damping;

            let dist = (pos[idx].0 * pos[idx].0 + pos[idx].1 * pos[idx].1).sqrt();
            if dist > BOUNDARY_RADIUS {
                let scale = BOUNDARY_RADIUS / dist;
                pos[idx].0 *= scale;
                pos[idx].1 *= scale;
            }
        }

        temperature -= cooling;
        if temperature < 0.1 {
            break;
        }
    }

    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_types::galaxy::ViewLevel;
    use ob_poc_types::graph_scene::{DrillTarget, GraphSceneModel, LayoutStrategy, SceneBadge, SceneEdgeType};

    fn node(id: &str, depth: usize) -> SceneNode {
        SceneNode {
            id: id.into(),
            label: id.into(),
            node_type: SceneNodeType::Entity,
            state: None,
            progress: 0,
            blocking: false,
            depth,
            position_hint: None,
            badges: Vec::<SceneBadge>::new(),
            child_count: 0,
            group_id: None,
        }
    }

    #[test]
    fn orbital_layout_keeps_all_satellites() {
        let mut nodes = vec![SceneNode {
            id: "universe".into(),
            label: "Universe".into(),
            node_type: SceneNodeType::Aggregate,
            state: None,
            progress: 0,
            blocking: false,
            depth: 0,
            position_hint: Some((0.0, 0.0)),
            badges: vec![],
            child_count: 8,
            group_id: None,
        }];
        for idx in 0..8 {
            nodes.push(node(&format!("workspace:{idx}"), 1));
        }

        let scene = GraphSceneModel {
            generation: 1,
            level: ViewLevel::Universe,
            layout_strategy: LayoutStrategy::DeterministicOrbital,
            nodes,
            edges: vec![],
            groups: vec![],
            drill_targets: Vec::<DrillTarget>::new(),
            max_depth: 1,
        };

        let cache = LayoutCache::derive(&scene);
        assert_eq!(cache.nodes.len(), 9);
        for geom in cache.nodes.iter().skip(1) {
            assert!(geom.center.distance(Pos2::ZERO) > 0.0);
        }
    }

    #[test]
    fn tree_layout_uses_rect_hit_testing() {
        let scene = GraphSceneModel {
            generation: 2,
            level: ViewLevel::Core,
            layout_strategy: LayoutStrategy::TreeDag,
            nodes: vec![node("root", 0), node("child", 1)],
            edges: vec![SceneEdge {
                source: "root".into(),
                target: "child".into(),
                edge_type: SceneEdgeType::Ownership,
                label: None,
                weight: 1.0,
            }],
            groups: vec![],
            drill_targets: vec![],
            max_depth: 1,
        };

        let cache = LayoutCache::derive(&scene);
        let root_center = cache.nodes[0].center;
        let hit = cache.hit_test(&scene, Pos2::new(root_center.x + 50.0, root_center.y));
        assert_eq!(hit.as_deref(), Some("root"));
    }
}

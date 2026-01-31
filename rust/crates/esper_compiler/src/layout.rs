//! Layout algorithms for positioning entities.

use crate::config::LayoutConfig;
use crate::error::CompilerError;
use crate::input::GraphInput;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Position result from layout.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// Layout algorithm trait.
pub trait LayoutAlgorithm {
    /// Compute positions for all entities.
    fn layout(
        &self,
        input: &dyn GraphInput,
        config: &LayoutConfig,
    ) -> Result<HashMap<u64, Position>, CompilerError>;
}

/// Layout engine that dispatches to specific algorithms.
#[derive(Debug, Default)]
pub struct LayoutEngine;

impl LayoutEngine {
    /// Create a new layout engine.
    pub fn new() -> Self {
        Self
    }

    /// Compute layout positions for a graph.
    pub fn compute(
        &self,
        input: &dyn GraphInput,
        config: &LayoutConfig,
    ) -> Result<HashMap<u64, Position>, CompilerError> {
        if input.entity_count() == 0 {
            return Err(CompilerError::EmptyGraph);
        }

        match config.algorithm {
            crate::config::LayoutAlgorithmConfig::Tree => TreeLayout.layout(input, config),
            crate::config::LayoutAlgorithmConfig::Force => {
                ForceLayout::default().layout(input, config)
            }
            crate::config::LayoutAlgorithmConfig::Grid => GridLayout.layout(input, config),
            crate::config::LayoutAlgorithmConfig::Radial => RadialLayout.layout(input, config),
            crate::config::LayoutAlgorithmConfig::Matrix => MatrixLayout.layout(input, config),
        }
    }
}

/// Tree/hierarchical layout algorithm.
#[derive(Debug, Default)]
pub struct TreeLayout;

impl LayoutAlgorithm for TreeLayout {
    fn layout(
        &self,
        input: &dyn GraphInput,
        config: &LayoutConfig,
    ) -> Result<HashMap<u64, Position>, CompilerError> {
        let mut positions = HashMap::new();
        let roots = input.roots();

        if roots.is_empty() {
            return Err(CompilerError::LayoutFailed(
                "no root nodes found".to_string(),
            ));
        }

        // Calculate tree sizes for each subtree
        let mut subtree_widths: HashMap<u64, f32> = HashMap::new();
        for &root in &roots {
            calculate_subtree_width(input, root, config.node_spacing, &mut subtree_widths);
        }

        // Position each root and its subtree
        let mut current_x = config.padding;
        for &root in &roots {
            let width = *subtree_widths.get(&root).unwrap_or(&config.node_spacing);
            let start_x = current_x + width / 2.0;

            position_tree(
                input,
                root,
                start_x,
                config.padding,
                0,
                config,
                &subtree_widths,
                &mut positions,
            );

            current_x += width + config.node_spacing;
        }

        Ok(positions)
    }
}

fn calculate_subtree_width(
    input: &dyn GraphInput,
    entity_id: u64,
    node_spacing: f32,
    widths: &mut HashMap<u64, f32>,
) -> f32 {
    let children = input.children(entity_id);

    if children.is_empty() {
        widths.insert(entity_id, node_spacing);
        return node_spacing;
    }

    let total_width: f32 = children
        .iter()
        .map(|&child| calculate_subtree_width(input, child, node_spacing, widths))
        .sum::<f32>()
        + (children.len() as f32 - 1.0).max(0.0) * node_spacing;

    widths.insert(entity_id, total_width);
    total_width
}

fn position_tree(
    input: &dyn GraphInput,
    entity_id: u64,
    center_x: f32,
    y: f32,
    depth: usize,
    config: &LayoutConfig,
    subtree_widths: &HashMap<u64, f32>,
    positions: &mut HashMap<u64, Position>,
) {
    positions.insert(entity_id, Position::new(center_x, y));

    let children = input.children(entity_id);
    if children.is_empty() {
        return;
    }

    // Calculate total width of children
    let total_width: f32 = children
        .iter()
        .map(|&c| subtree_widths.get(&c).unwrap_or(&config.node_spacing))
        .sum::<f32>()
        + (children.len() as f32 - 1.0).max(0.0) * config.node_spacing;

    // Position children centered under parent
    let start_x = center_x - total_width / 2.0;
    let child_y = y + config.level_spacing;

    let mut current_x = start_x;
    for &child in &children {
        let child_width = *subtree_widths.get(&child).unwrap_or(&config.node_spacing);
        let child_center_x = current_x + child_width / 2.0;

        position_tree(
            input,
            child,
            child_center_x,
            child_y,
            depth + 1,
            config,
            subtree_widths,
            positions,
        );

        current_x += child_width + config.node_spacing;
    }
}

/// Force-directed layout algorithm.
#[derive(Debug)]
pub struct ForceLayout {
    /// Number of iterations.
    pub iterations: usize,
    /// Repulsion strength.
    pub repulsion: f32,
    /// Attraction strength (for edges).
    pub attraction: f32,
    /// Damping factor.
    pub damping: f32,
}

impl Default for ForceLayout {
    fn default() -> Self {
        Self {
            iterations: 100,
            repulsion: 5000.0,
            attraction: 0.1,
            damping: 0.9,
        }
    }
}

impl LayoutAlgorithm for ForceLayout {
    fn layout(
        &self,
        input: &dyn GraphInput,
        config: &LayoutConfig,
    ) -> Result<HashMap<u64, Position>, CompilerError> {
        let entity_ids = input.entity_ids();
        let n = entity_ids.len();

        // Initialize positions in a grid
        let mut positions: HashMap<u64, Position> = HashMap::new();
        let cols = (n as f32).sqrt().ceil() as usize;

        for (i, &id) in entity_ids.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = config.padding + col as f32 * config.node_spacing;
            let y = config.padding + row as f32 * config.node_spacing;
            positions.insert(id, Position::new(x, y));
        }

        // If using pre-computed positions, use those
        for &id in &entity_ids {
            if let Some(entity) = input.get_entity(id) {
                if let Some((x, y)) = entity.position {
                    positions.insert(id, Position::new(x, y));
                }
            }
        }

        let edges = input.edges();
        let mut velocities: HashMap<u64, (f32, f32)> =
            entity_ids.iter().map(|&id| (id, (0.0, 0.0))).collect();

        // Run force simulation
        for _ in 0..self.iterations {
            // Calculate repulsion forces (all pairs)
            for i in 0..n {
                for j in (i + 1)..n {
                    let id_i = entity_ids[i];
                    let id_j = entity_ids[j];

                    let pos_i = positions[&id_i];
                    let pos_j = positions[&id_j];

                    let dx = pos_j.x - pos_i.x;
                    let dy = pos_j.y - pos_i.y;
                    let dist_sq = dx * dx + dy * dy + 0.01; // Avoid division by zero
                    let dist = dist_sq.sqrt();

                    let force = self.repulsion / dist_sq;
                    let fx = force * dx / dist;
                    let fy = force * dy / dist;

                    velocities.get_mut(&id_i).unwrap().0 -= fx;
                    velocities.get_mut(&id_i).unwrap().1 -= fy;
                    velocities.get_mut(&id_j).unwrap().0 += fx;
                    velocities.get_mut(&id_j).unwrap().1 += fy;
                }
            }

            // Calculate attraction forces (edges)
            for edge in &edges {
                let pos_from = positions[&edge.from];
                let pos_to = positions[&edge.to];

                let dx = pos_to.x - pos_from.x;
                let dy = pos_to.y - pos_from.y;
                let dist = (dx * dx + dy * dy).sqrt() + 0.01;

                let force = self.attraction * dist;
                let fx = force * dx / dist;
                let fy = force * dy / dist;

                velocities.get_mut(&edge.from).unwrap().0 += fx;
                velocities.get_mut(&edge.from).unwrap().1 += fy;
                velocities.get_mut(&edge.to).unwrap().0 -= fx;
                velocities.get_mut(&edge.to).unwrap().1 -= fy;
            }

            // Apply velocities and damping
            for &id in &entity_ids {
                let vel = velocities.get_mut(&id).unwrap();
                let pos = positions.get_mut(&id).unwrap();

                pos.x += vel.0;
                pos.y += vel.1;

                vel.0 *= self.damping;
                vel.1 *= self.damping;
            }
        }

        // Normalize to viewport
        normalize_positions(&mut positions, config);

        Ok(positions)
    }
}

/// Grid layout algorithm.
#[derive(Debug, Default)]
pub struct GridLayout;

impl LayoutAlgorithm for GridLayout {
    fn layout(
        &self,
        input: &dyn GraphInput,
        config: &LayoutConfig,
    ) -> Result<HashMap<u64, Position>, CompilerError> {
        let entity_ids = input.entity_ids();

        let cols = ((config.viewport_width - 2.0 * config.padding) / config.node_spacing) as usize;
        let cols = cols.max(1);

        let mut positions = HashMap::new();

        for (i, &id) in entity_ids.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = config.padding + col as f32 * config.node_spacing;
            let y = config.padding + row as f32 * config.node_spacing;
            positions.insert(id, Position::new(x, y));
        }

        Ok(positions)
    }
}

/// Radial/orbital layout algorithm.
#[derive(Debug, Default)]
pub struct RadialLayout;

impl LayoutAlgorithm for RadialLayout {
    fn layout(
        &self,
        input: &dyn GraphInput,
        config: &LayoutConfig,
    ) -> Result<HashMap<u64, Position>, CompilerError> {
        let mut positions = HashMap::new();
        let roots = input.roots();

        let center_x = config.viewport_width / 2.0;
        let center_y = config.viewport_height / 2.0;

        // Position roots at center
        for (i, &root) in roots.iter().enumerate() {
            let angle = 2.0 * std::f32::consts::PI * i as f32 / roots.len().max(1) as f32;
            let radius = if roots.len() > 1 {
                config.node_spacing
            } else {
                0.0
            };
            positions.insert(
                root,
                Position::new(
                    center_x + radius * angle.cos(),
                    center_y + radius * angle.sin(),
                ),
            );

            // Position children in concentric rings
            position_radial_children(
                input,
                root,
                center_x,
                center_y,
                config.level_spacing,
                config,
                &mut positions,
            );
        }

        Ok(positions)
    }
}

fn position_radial_children(
    input: &dyn GraphInput,
    entity_id: u64,
    center_x: f32,
    center_y: f32,
    radius: f32,
    config: &LayoutConfig,
    positions: &mut HashMap<u64, Position>,
) {
    let children = input.children(entity_id);
    if children.is_empty() {
        return;
    }

    let angle_step = 2.0 * std::f32::consts::PI / children.len() as f32;

    for (i, &child) in children.iter().enumerate() {
        let angle = angle_step * i as f32;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        positions.insert(child, Position::new(x, y));

        position_radial_children(
            input,
            child,
            x,
            y,
            radius * 0.7, // Shrink radius for deeper levels
            config,
            positions,
        );
    }
}

/// Matrix layout for densely connected graphs.
#[derive(Debug, Default)]
pub struct MatrixLayout;

impl LayoutAlgorithm for MatrixLayout {
    fn layout(
        &self,
        input: &dyn GraphInput,
        config: &LayoutConfig,
    ) -> Result<HashMap<u64, Position>, CompilerError> {
        // Same as grid for now
        GridLayout.layout(input, config)
    }
}

/// Normalize positions to fit within viewport.
fn normalize_positions(positions: &mut HashMap<u64, Position>, config: &LayoutConfig) {
    if positions.is_empty() {
        return;
    }

    // Find bounds
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for pos in positions.values() {
        min_x = min_x.min(pos.x);
        max_x = max_x.max(pos.x);
        min_y = min_y.min(pos.y);
        max_y = max_y.max(pos.y);
    }

    let width = max_x - min_x;
    let height = max_y - min_y;

    if width < 0.01 && height < 0.01 {
        // All points at same location, center them
        for pos in positions.values_mut() {
            pos.x = config.viewport_width / 2.0;
            pos.y = config.viewport_height / 2.0;
        }
        return;
    }

    let available_width = config.viewport_width - 2.0 * config.padding;
    let available_height = config.viewport_height - 2.0 * config.padding;

    let scale_x = if width > 0.01 {
        available_width / width
    } else {
        1.0
    };
    let scale_y = if height > 0.01 {
        available_height / height
    } else {
        1.0
    };
    let scale = scale_x.min(scale_y).min(1.0); // Don't scale up

    for pos in positions.values_mut() {
        pos.x = config.padding + (pos.x - min_x) * scale;
        pos.y = config.padding + (pos.y - min_y) * scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MemoryGraphInput;

    #[test]
    fn tree_layout_basic() {
        let graph = MemoryGraphInput::simple_tree(vec![(1, "Root", 0), (2, "A", 1), (3, "B", 1)]);

        let config = LayoutConfig::default();
        let positions = TreeLayout.layout(&graph, &config).unwrap();

        assert_eq!(positions.len(), 3);

        // Root should be above children
        let root_pos = positions[&1];
        let child_a = positions[&2];
        assert!(root_pos.y < child_a.y);
    }

    #[test]
    fn grid_layout_basic() {
        let graph = MemoryGraphInput::new()
            .add_entity(crate::input::EntityInput::new(1, "A", 0))
            .add_entity(crate::input::EntityInput::new(2, "B", 0))
            .add_entity(crate::input::EntityInput::new(3, "C", 0))
            .add_entity(crate::input::EntityInput::new(4, "D", 0));

        let config = LayoutConfig::default();
        let positions = GridLayout.layout(&graph, &config).unwrap();

        assert_eq!(positions.len(), 4);
    }

    #[test]
    fn force_layout_converges() {
        let graph = MemoryGraphInput::new()
            .add_entity(crate::input::EntityInput::new(1, "A", 0))
            .add_entity(crate::input::EntityInput::new(2, "B", 0))
            .add_edge(1, 2, crate::input::EdgeKind::Reference);

        let config = LayoutConfig::default();
        let positions = ForceLayout::default().layout(&graph, &config).unwrap();

        assert_eq!(positions.len(), 2);

        // Connected nodes should be relatively close
        let pos_a = positions[&1];
        let pos_b = positions[&2];
        let dist = ((pos_a.x - pos_b.x).powi(2) + (pos_a.y - pos_b.y).powi(2)).sqrt();
        assert!(dist < config.viewport_width); // Should be within viewport
    }
}

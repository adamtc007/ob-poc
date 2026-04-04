//! System-level renderer — deterministic orbital layout.
//!
//! Central CBU node with entity satellites in orbit.
//! Slot positions encode role meaning (deterministic from slot index).
//! Phase 3 proof of concept.

use std::f32::consts::TAU;

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneEdge, SceneNode, SceneNodeType};

use crate::state::ObservatoryState;

/// Paint System-level constellation: central CBU + orbital entity slots.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    state: &ObservatoryState,
) {
    let nodes = &scene.nodes;
    let edges = &scene.edges;

    if nodes.is_empty() {
        return;
    }

    // ── Compute orbital positions (deterministic from index) ──
    let positions = compute_orbital_positions(nodes);

    // ── Paint edges first (below nodes) ──
    for edge in edges {
        paint_edge(painter, transform, edge, &positions);
    }

    // ── Paint nodes ──
    for (i, node) in nodes.iter().enumerate() {
        let (x, y) = positions[i];
        let screen_pos = transform.transform_pos(Pos2::new(x, y));
        let is_selected = state
            .interaction
            .selected_node
            .as_deref()
            == Some(&node.id);
        let is_hovered = state
            .interaction
            .hovered_node
            .as_deref()
            == Some(&node.id);

        paint_node(painter, screen_pos, node, is_selected, is_hovered);
    }
}

/// Compute deterministic orbital positions.
/// First node (CBU) at center. Remaining nodes in concentric rings.
fn compute_orbital_positions(nodes: &[SceneNode]) -> Vec<(f32, f32)> {
    let mut positions = Vec::with_capacity(nodes.len());

    if nodes.is_empty() {
        return positions;
    }

    // First node at center (root CBU)
    positions.push((0.0, 0.0));

    if nodes.len() == 1 {
        return positions;
    }

    // Remaining nodes in orbital ring(s)
    let orbital_nodes = nodes.len() - 1;
    let ring_capacity = 12; // max nodes per ring
    let ring_count = (orbital_nodes + ring_capacity - 1) / ring_capacity;

    let mut placed = 0;
    for ring in 0..ring_count {
        let ring_radius = 200.0 + (ring as f32) * 150.0;
        let nodes_in_ring = if ring < ring_count - 1 {
            ring_capacity
        } else {
            orbital_nodes - placed
        };

        for j in 0..nodes_in_ring {
            let angle = (j as f32 / nodes_in_ring as f32) * TAU - TAU / 4.0;
            let x = angle.cos() * ring_radius;
            let y = angle.sin() * ring_radius;
            positions.push((x, y));
            placed += 1;
        }
    }

    positions
}

/// Paint a single node.
fn paint_node(
    painter: &Painter,
    screen_pos: Pos2,
    node: &SceneNode,
    selected: bool,
    hovered: bool,
) {
    let base_radius = match node.node_type {
        SceneNodeType::Cbu => 24.0,
        SceneNodeType::Aggregate => 20.0,
        _ => 14.0,
    };

    let radius = if hovered {
        base_radius * 1.15
    } else {
        base_radius
    };

    // Node fill color based on state
    let fill_color = node_color(node);

    // Selection ring
    if selected {
        painter.circle_stroke(
            screen_pos,
            radius + 4.0,
            Stroke::new(2.0, Color32::from_rgb(59, 130, 246)),
        );
    }

    // Node circle
    painter.circle_filled(screen_pos, radius, fill_color);

    // Progress arc (if > 0) — simplified as full ring for now
    if node.progress > 0 && node.progress < 100 {
        let progress_color = Color32::from_rgb(34, 197, 94);
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(2.0, progress_color),
        );
    }

    // Blocking indicator
    if node.blocking {
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(2.0, Color32::from_rgb(239, 68, 68)),
        );
    }

    // Label
    painter.text(
        screen_pos + Vec2::new(0.0, radius + 10.0),
        egui::Align2::CENTER_TOP,
        &node.label,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(203, 213, 225),
    );
}

/// Paint an edge between two nodes.
fn paint_edge(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    edge: &SceneEdge,
    positions: &[(f32, f32)],
) {
    // Find source and target positions by index (simplified — in production, use a HashMap)
    // For now, edges reference node IDs but positions are by index
    // This will be improved when SceneCache stores id→position mapping
    let _ = (painter, transform, edge, positions);
    // TODO: Phase 5 — edge rendering with proper id→position lookup
}

/// Color for a node based on its type and state.
fn node_color(node: &SceneNode) -> Color32 {
    match node.state.as_deref() {
        Some("complete") => Color32::from_rgb(34, 197, 94),   // green
        Some("filled") => Color32::from_rgb(59, 130, 246),    // blue
        Some("blocked") => Color32::from_rgb(239, 68, 68),    // red
        Some("empty") => Color32::from_rgb(71, 85, 105),      // slate
        _ => match node.node_type {
            SceneNodeType::Cbu => Color32::from_rgb(139, 92, 246),       // purple
            SceneNodeType::Entity => Color32::from_rgb(59, 130, 246),    // blue
            SceneNodeType::Case => Color32::from_rgb(245, 158, 11),      // amber
            SceneNodeType::Tollgate => Color32::from_rgb(239, 68, 68),   // red
            _ => Color32::from_rgb(100, 116, 139),                       // gray
        },
    }
}

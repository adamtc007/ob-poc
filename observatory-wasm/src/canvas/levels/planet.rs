//! Planet-level renderer — hierarchical relationship graph.
//!
//! Tiered layout:
//! - Tier 0 (top): focused entity (largest, centered)
//! - Tier 1: direct relationships, evenly spaced horizontally
//! - Tier 2+: deeper relationships, placed under their parents
//! Horizontal spacing: 120px per node, centered per tier.
//! Vertical spacing: 150px between tiers.
//! Edges drawn with directional arrows. Labels on nodes, badges rendered.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneEdge, SceneNode, SceneNodeType};

use crate::state::CanvasApp;

const HORIZONTAL_SPACING: f32 = 120.0;
const VERTICAL_SPACING: f32 = 150.0;

/// Paint Planet-level: entity center + tiered relationship nodes.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    app: &CanvasApp,
) {
    let nodes = &scene.nodes;
    let edges = &scene.edges;

    if nodes.is_empty() {
        return;
    }

    // ── Compute tiered positions using node.depth ──
    let positions = compute_tiered_positions(nodes);

    // ── Paint edges with directional arrows ──
    for edge in edges {
        paint_edge(painter, transform, edge, nodes, &positions);
    }

    // ── Paint nodes ──
    for (i, node) in nodes.iter().enumerate() {
        let (x, y) = positions[i];
        let screen_pos = transform.transform_pos(Pos2::new(x, y));
        let is_selected = app.interaction.selected_node.as_deref() == Some(&node.id);
        let is_hovered = app.interaction.hovered_node.as_deref() == Some(&node.id);

        paint_node(painter, screen_pos, node, is_selected, is_hovered);
    }
}

// ── Tiered position computation ──────────────────────────────

fn compute_tiered_positions(nodes: &[SceneNode]) -> Vec<(f32, f32)> {
    let mut positions = vec![(0.0f32, 0.0f32); nodes.len()];

    if nodes.is_empty() {
        return positions;
    }

    // Group nodes by tier (depth)
    let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);

    for tier in 0..=max_depth {
        let tier_nodes: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.depth == tier)
            .map(|(i, _)| i)
            .collect();

        if tier_nodes.is_empty() {
            continue;
        }

        let count = tier_nodes.len();
        let total_width = (count as f32 - 1.0) * HORIZONTAL_SPACING;
        let start_x = -total_width / 2.0;
        let y = tier as f32 * VERTICAL_SPACING;

        for (j, &idx) in tier_nodes.iter().enumerate() {
            positions[idx] = (start_x + j as f32 * HORIZONTAL_SPACING, y);
        }
    }

    positions
}

// ── Node painting ────────────────────────────────────────────

fn paint_node(
    painter: &Painter,
    screen_pos: Pos2,
    node: &SceneNode,
    selected: bool,
    hovered: bool,
) {
    let is_focus = node.depth == 0;

    let base_radius = if is_focus {
        28.0
    } else {
        match node.node_type {
            SceneNodeType::Cbu => 18.0,
            SceneNodeType::Entity => 14.0,
            SceneNodeType::Case => 12.0,
            _ => 14.0,
        }
    };

    let radius = if hovered {
        base_radius * 1.15
    } else {
        base_radius
    };

    let fill = node_color(node, is_focus);
    painter.circle_filled(screen_pos, radius, fill);

    // Selection ring
    if selected {
        painter.circle_stroke(
            screen_pos,
            radius + 3.0,
            Stroke::new(2.0, Color32::from_rgb(245, 158, 11)),
        );
    }

    // Hover ring
    if hovered && !selected {
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(1.5, Color32::from_rgb(148, 163, 184)),
        );
    }

    // Progress indicator
    if node.progress > 0 && node.progress < 100 {
        painter.circle_stroke(
            screen_pos,
            radius + 2.0,
            Stroke::new(2.0, Color32::from_rgb(34, 197, 94)),
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

    // Badges (right of node)
    for (bi, badge) in node.badges.iter().enumerate() {
        let badge_pos =
            screen_pos + Vec2::new(radius + 4.0, -radius + (bi as f32 * 14.0));
        painter.text(
            badge_pos,
            egui::Align2::LEFT_CENTER,
            &badge.label,
            egui::FontId::proportional(8.0),
            Color32::from_rgb(148, 163, 184),
        );
    }

    // Label below node
    let font_size = if is_focus { 12.0 } else { 10.0 };
    painter.text(
        screen_pos + Vec2::new(0.0, radius + 8.0),
        egui::Align2::CENTER_TOP,
        &node.label,
        egui::FontId::proportional(font_size),
        Color32::from_rgb(226, 232, 240),
    );
}

// ── Edge painting with directional arrows ────────────────────

fn paint_edge(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    edge: &SceneEdge,
    nodes: &[SceneNode],
    positions: &[(f32, f32)],
) {
    let src_idx = nodes.iter().position(|n| n.id == edge.source);
    let tgt_idx = nodes.iter().position(|n| n.id == edge.target);

    let (Some(si), Some(ti)) = (src_idx, tgt_idx) else {
        return;
    };

    let src_pos = transform.transform_pos(Pos2::new(positions[si].0, positions[si].1));
    let tgt_pos = transform.transform_pos(Pos2::new(positions[ti].0, positions[ti].1));

    let edge_color = match edge.edge_type {
        ob_poc_types::graph_scene::SceneEdgeType::Dependency => {
            Color32::from_rgb(245, 158, 11)
        }
        ob_poc_types::graph_scene::SceneEdgeType::Ownership => {
            Color32::from_rgb(139, 92, 246)
        }
        ob_poc_types::graph_scene::SceneEdgeType::Control => {
            Color32::from_rgb(59, 130, 246)
        }
        _ => Color32::from_rgb(100, 116, 139),
    };

    let stroke_width = (edge.weight * 1.5).clamp(1.0, 3.0);
    painter.line_segment([src_pos, tgt_pos], Stroke::new(stroke_width, edge_color));

    // Directional arrow head at target
    let dir = (tgt_pos - src_pos).normalized();
    let perp = Vec2::new(-dir.y, dir.x);
    let arrow_size = 7.0;
    // Pull arrow back from target center by approximate node radius
    let target_radius = if nodes[ti].depth == 0 { 28.0 } else { 14.0 };
    let arrow_tip = tgt_pos - dir * target_radius;
    let arrow_base = arrow_tip - dir * arrow_size;

    painter.add(egui::Shape::convex_polygon(
        vec![
            arrow_tip,
            arrow_base + perp * arrow_size * 0.5,
            arrow_base - perp * arrow_size * 0.5,
        ],
        edge_color,
        Stroke::NONE,
    ));

    // Edge label at midpoint
    if let Some(ref label) = edge.label {
        let mid = Pos2::new(
            (src_pos.x + tgt_pos.x) / 2.0,
            (src_pos.y + tgt_pos.y) / 2.0,
        );
        painter.text(
            mid + Vec2::new(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(9.0),
            Color32::from_rgb(148, 163, 184),
        );
    }
}

// ── Color utility ────────────────────────────────────────────

fn node_color(node: &SceneNode, is_focus: bool) -> Color32 {
    match node.state.as_deref() {
        Some("complete") => Color32::from_rgb(34, 197, 94),
        Some("filled") => Color32::from_rgb(59, 130, 246),
        Some("blocked") => Color32::from_rgb(239, 68, 68),
        Some("empty") => Color32::from_rgb(71, 85, 105),
        _ => {
            if is_focus {
                Color32::from_rgb(139, 92, 246) // purple for focus
            } else {
                match node.node_type {
                    SceneNodeType::Cbu => Color32::from_rgb(139, 92, 246),
                    SceneNodeType::Entity => Color32::from_rgb(59, 130, 246),
                    SceneNodeType::Case => Color32::from_rgb(245, 158, 11),
                    SceneNodeType::Tollgate => Color32::from_rgb(239, 68, 68),
                    _ => Color32::from_rgb(100, 116, 139),
                }
            }
        }
    }
}

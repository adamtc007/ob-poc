//! Cluster-level renderer — force within fixed boundary.
//!
//! Same Fruchterman-Reingold force algorithm as Universe, with an additional
//! boundary constraint: nodes that exit radius 400 are projected back.
//! CBU nodes (radius 18), entity nodes (radius 8).
//! Boundary circle drawn with dashed stroke.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneEdge, SceneNode, SceneNodeType};

use crate::canvas::layout::LayoutCache;
use crate::state::CanvasApp;

const BOUNDARY_RADIUS: f32 = 400.0;

/// Paint Cluster-level: CBU nodes within a constrained boundary.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    cache: &LayoutCache,
    app: &CanvasApp,
) {
    let nodes = &scene.nodes;
    let edges = &scene.edges;

    // ── Boundary circle (dashed) ──
    paint_dashed_boundary(painter, transform);

    if nodes.is_empty() {
        return;
    }

    // ── Paint edges ──
    for (edge, geom) in edges.iter().zip(&cache.edges) {
        paint_edge(painter, transform, edge, geom, cache);
    }

    // ── Paint nodes ──
    for (i, node) in nodes.iter().enumerate() {
        let screen_pos = transform.transform_pos(cache.nodes[i].center);
        let is_selected = app.interaction.selected_node.as_deref() == Some(&node.id);
        let is_hovered = app.interaction.hovered_node.as_deref() == Some(&node.id);

        paint_node(painter, screen_pos, node, is_selected, is_hovered);
    }
}

// ── Dashed boundary circle ───────────────────────────────────

fn paint_dashed_boundary(painter: &Painter, transform: &egui::emath::RectTransform) {
    let center = transform.transform_pos(Pos2::ZERO);
    let edge_point = transform.transform_pos(Pos2::new(BOUNDARY_RADIUS, 0.0));
    let screen_radius = (edge_point.x - center.x).abs();

    // Draw dashed circle as short arc segments
    let dash_count = 48;
    let dash_fraction = 0.6; // fraction of each segment that is drawn
    let color = Color32::from_rgba_premultiplied(71, 85, 105, 60);

    for i in 0..dash_count {
        let angle_start =
            (i as f32 / dash_count as f32) * std::f32::consts::TAU;
        let angle_end = angle_start
            + (dash_fraction / dash_count as f32) * std::f32::consts::TAU;

        let p1 = Pos2::new(
            center.x + angle_start.cos() * screen_radius,
            center.y + angle_start.sin() * screen_radius,
        );
        let p2 = Pos2::new(
            center.x + angle_end.cos() * screen_radius,
            center.y + angle_end.sin() * screen_radius,
        );

        painter.line_segment([p1, p2], Stroke::new(1.0, color));
    }
}

// ── Node painting ────────────────────────────────────────────

fn paint_node(
    painter: &Painter,
    screen_pos: Pos2,
    node: &SceneNode,
    selected: bool,
    hovered: bool,
) {
    let base_radius = match node.node_type {
        SceneNodeType::Cbu => 18.0,
        SceneNodeType::Entity => 8.0,
        _ => 12.0,
    };

    let radius = if hovered {
        base_radius * 1.15
    } else {
        base_radius
    };

    let fill_color = node_color(node);
    painter.circle_filled(screen_pos, radius, fill_color);

    // Selection ring
    if selected {
        painter.circle_stroke(
            screen_pos,
            radius + 3.0,
            Stroke::new(2.0, Color32::from_rgb(59, 130, 246)),
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

    // Progress arc indicator
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

    // Label (always for CBU, on hover for entities)
    if matches!(node.node_type, SceneNodeType::Cbu) || hovered {
        painter.text(
            screen_pos + Vec2::new(0.0, radius + 6.0),
            egui::Align2::CENTER_TOP,
            &node.label,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(203, 213, 225),
        );
    }
}

// ── Edge painting ────────────────────────────────────────────

fn paint_edge(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    edge: &SceneEdge,
    geom: &crate::canvas::layout::EdgeGeometry,
    cache: &LayoutCache,
) {
    let src_pos = transform.transform_pos(cache.nodes[geom.source_idx].center);
    let tgt_pos = transform.transform_pos(cache.nodes[geom.target_idx].center);

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
        ob_poc_types::graph_scene::SceneEdgeType::SharedEntity => {
            Color32::from_rgba_premultiplied(100, 116, 139, 80)
        }
        _ => Color32::from_rgb(71, 85, 105),
    };

    let stroke_width = (edge.weight * 1.5).clamp(0.5, 3.0);
    painter.line_segment([src_pos, tgt_pos], Stroke::new(stroke_width, edge_color));

    // Edge label at midpoint
    if let Some(ref label) = edge.label {
        let mid = Pos2::new(
            (src_pos.x + tgt_pos.x) / 2.0,
            (src_pos.y + tgt_pos.y) / 2.0,
        );
        painter.text(
            mid,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.0),
            Color32::from_rgb(148, 163, 184),
        );
    }
}

// ── Color utility ────────────────────────────────────────────

fn node_color(node: &SceneNode) -> Color32 {
    match node.state.as_deref() {
        Some("complete") => Color32::from_rgb(34, 197, 94),
        Some("filled") => Color32::from_rgb(59, 130, 246),
        Some("blocked") => Color32::from_rgb(239, 68, 68),
        Some("empty") => Color32::from_rgb(71, 85, 105),
        _ => match node.node_type {
            SceneNodeType::Cbu => Color32::from_rgb(139, 92, 246),
            SceneNodeType::Entity => Color32::from_rgb(59, 130, 246),
            SceneNodeType::Case => Color32::from_rgb(245, 158, 11),
            _ => Color32::from_rgb(100, 116, 139),
        },
    }
}

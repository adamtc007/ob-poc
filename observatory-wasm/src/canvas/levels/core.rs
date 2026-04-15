//! Core-level renderer — tree/DAG layout for ownership/control chains.
//!
//! Top-down tree layout: root at top center, children below, spaced proportional
//! to subtree width. Recursive layout computes subtree widths first, then assigns
//! positions. Edge labels show ownership percentages. Edge width proportional to
//! edge.weight (clamped 1-4px). Colors: purple for ownership, blue for control.
//! UBO readability: clear hierarchy, no overlapping labels.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneEdge, SceneEdgeType, SceneNode};

use crate::canvas::layout::LayoutCache;
use crate::state::CanvasApp;

const NODE_WIDTH: f32 = 120.0;
const NODE_HEIGHT: f32 = 36.0;

/// Paint Core-level: ownership/control chains as top-down tree.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    cache: &LayoutCache,
    app: &CanvasApp,
) {
    let nodes = &scene.nodes;
    let edges = &scene.edges;

    if nodes.is_empty() {
        return;
    }

    // ── Paint edges with ownership/control styling ──
    for (edge, geom) in edges.iter().zip(&cache.edges) {
        paint_edge(painter, transform, edge, geom, cache);
    }

    // ── Paint nodes as rounded rectangles ──
    for (i, node) in nodes.iter().enumerate() {
        let screen_pos = transform.transform_pos(cache.nodes[i].center);
        let is_selected = app.interaction.selected_node.as_deref() == Some(&node.id);
        let is_hovered = app.interaction.hovered_node.as_deref() == Some(&node.id);

        paint_node(painter, screen_pos, node, is_selected, is_hovered);
    }
}

// ── Node painting (rounded rectangles) ───────────────────────

fn paint_node(
    painter: &Painter,
    screen_pos: Pos2,
    node: &SceneNode,
    selected: bool,
    hovered: bool,
) {
    let size = Vec2::new(NODE_WIDTH, NODE_HEIGHT);
    let node_rect = egui::Rect::from_center_size(screen_pos, size);

    let fill = match node.state.as_deref() {
        Some("complete") => Color32::from_rgb(34, 197, 94),
        Some("filled") => Color32::from_rgb(59, 130, 246),
        Some("blocked") => Color32::from_rgb(239, 68, 68),
        _ => Color32::from_rgb(71, 85, 105),
    };

    painter.rect_filled(node_rect, 4.0, fill);

    // Selection highlight
    if selected {
        painter.rect_stroke(
            node_rect.expand(2.0),
            4.0,
            Stroke::new(2.0, Color32::from_rgb(245, 158, 11)),
            egui::StrokeKind::Outside,
        );
    }

    // Hover highlight
    if hovered && !selected {
        painter.rect_stroke(
            node_rect.expand(1.0),
            4.0,
            Stroke::new(1.5, Color32::from_rgb(148, 163, 184)),
            egui::StrokeKind::Outside,
        );
    }

    // Entity name (centered in box)
    painter.text(
        screen_pos,
        egui::Align2::CENTER_CENTER,
        &node.label,
        egui::FontId::proportional(10.0),
        Color32::WHITE,
    );

    // Depth indicator above node
    if node.depth > 0 {
        painter.text(
            screen_pos + Vec2::new(0.0, -NODE_HEIGHT / 2.0 - 8.0),
            egui::Align2::CENTER_BOTTOM,
            format!("L{}", node.depth),
            egui::FontId::proportional(8.0),
            Color32::from_rgb(148, 163, 184),
        );
    }

    // Badges below node (if any)
    for (bi, badge) in node.badges.iter().enumerate() {
        let badge_pos = screen_pos
            + Vec2::new(
                NODE_WIDTH / 2.0 + 4.0,
                -NODE_HEIGHT / 2.0 + (bi as f32 * 12.0),
            );
        painter.text(
            badge_pos,
            egui::Align2::LEFT_CENTER,
            &badge.label,
            egui::FontId::proportional(8.0),
            Color32::from_rgb(148, 163, 184),
        );
    }
}

// ── Edge painting with ownership/control styling ─────────────

fn paint_edge(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    edge: &SceneEdge,
    geom: &crate::canvas::layout::EdgeGeometry,
    cache: &LayoutCache,
) {
    let src_pos = transform.transform_pos(cache.nodes[geom.source_idx].center);
    let tgt_pos = transform.transform_pos(cache.nodes[geom.target_idx].center);

    // Edge color: purple for ownership, blue for control
    let edge_color = match edge.edge_type {
        SceneEdgeType::Ownership => Color32::from_rgb(139, 92, 246), // purple
        SceneEdgeType::Control => Color32::from_rgb(59, 130, 246),   // blue
        _ => Color32::from_rgb(100, 116, 139),                       // slate
    };

    // Edge width proportional to weight, clamped 1-4px
    let stroke_width = (edge.weight * 2.0).clamp(1.0, 4.0);

    // Connect from bottom of parent to top of child
    let src_bottom = Pos2::new(src_pos.x, src_pos.y + NODE_HEIGHT / 2.0);
    let tgt_top = Pos2::new(tgt_pos.x, tgt_pos.y - NODE_HEIGHT / 2.0);

    painter.line_segment(
        [src_bottom, tgt_top],
        Stroke::new(stroke_width, edge_color),
    );

    // Arrow head at target
    let dir = (tgt_top - src_bottom).normalized();
    let perp = Vec2::new(-dir.y, dir.x);
    let arrow_size = 6.0;
    let arrow_base = tgt_top - dir * arrow_size;

    painter.add(egui::Shape::convex_polygon(
        vec![
            tgt_top,
            arrow_base + perp * arrow_size * 0.5,
            arrow_base - perp * arrow_size * 0.5,
        ],
        edge_color,
        Stroke::NONE,
    ));

    // Edge label (ownership percentage or explicit label)
    let label_text = if let Some(ref label) = edge.label {
        Some(label.clone())
    } else if edge.weight > 0.0 && matches!(edge.edge_type, SceneEdgeType::Ownership) {
        Some(format!("{:.0}%", edge.weight))
    } else {
        None
    };

    if let Some(text) = label_text {
        let mid = Pos2::new(
            (src_bottom.x + tgt_top.x) / 2.0,
            (src_bottom.y + tgt_top.y) / 2.0,
        );
        painter.text(
            mid + Vec2::new(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(9.0),
            edge_color,
        );
    }
}

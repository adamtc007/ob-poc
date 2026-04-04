//! Core-level renderer — tree/DAG layout for ownership/control chains.
//!
//! Renders ownership percentages, control paths, document trails.
//! Hierarchical top-down layout.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneEdgeType};

use crate::state::CanvasApp;

/// Paint Core-level: ownership/control chains as top-down tree.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    app: &CanvasApp,
) {
    // Edges with ownership percentages
    for edge in &scene.edges {
        let from = scene.nodes.iter().find(|n| n.id == edge.source);
        let to = scene.nodes.iter().find(|n| n.id == edge.target);
        if let (Some(f), Some(t)) = (from, to) {
            if let (Some(fp), Some(tp)) = (f.position_hint, t.position_hint) {
                let sf = transform.transform_pos(Pos2::new(fp.0, fp.1));
                let st = transform.transform_pos(Pos2::new(tp.0, tp.1));

                let edge_color = match edge.edge_type {
                    SceneEdgeType::Ownership => Color32::from_rgb(34, 197, 94),
                    SceneEdgeType::Control => Color32::from_rgb(245, 158, 11),
                    _ => Color32::from_rgb(100, 116, 139),
                };

                painter.line_segment([sf, st], Stroke::new(2.0, edge_color));

                // Percentage label on ownership edges
                if edge.weight > 0.0 && matches!(edge.edge_type, SceneEdgeType::Ownership) {
                    let mid = Pos2::new((sf.x + st.x) / 2.0, (sf.y + st.y) / 2.0);
                    painter.text(
                        mid + Vec2::new(8.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{:.0}%", edge.weight),
                        egui::FontId::proportional(9.0),
                        edge_color,
                    );
                }

                // Arrow head
                let dir = (st - sf).normalized();
                let perp = Vec2::new(-dir.y, dir.x);
                let arrow_size = 6.0;
                let arrow_base = st - dir * arrow_size;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        st,
                        arrow_base + perp * arrow_size * 0.5,
                        arrow_base - perp * arrow_size * 0.5,
                    ],
                    edge_color,
                    Stroke::NONE,
                ));
            }
        }
    }

    // Nodes as rounded rectangles (tree nodes)
    for node in &scene.nodes {
        let (x, y) = node.position_hint.unwrap_or((0.0, 0.0));
        let screen_pos = transform.transform_pos(Pos2::new(x, y));
        let is_selected = app.interaction.selected_node.as_deref() == Some(&node.id);

        let node_size = Vec2::new(120.0, 36.0);
        let node_rect = egui::Rect::from_center_size(screen_pos, node_size);

        let fill = match node.state.as_deref() {
            Some("complete") => Color32::from_rgb(34, 197, 94),
            Some("filled") => Color32::from_rgb(59, 130, 246),
            _ => Color32::from_rgb(71, 85, 105),
        };

        painter.rect_filled(node_rect, 4.0, fill);

        if is_selected {
            painter.rect_stroke(
                node_rect.expand(2.0),
                4.0,
                Stroke::new(2.0, Color32::from_rgb(245, 158, 11)),
                egui::StrokeKind::Outside,
            );
        }

        // Entity name
        painter.text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            &node.label,
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        // Depth indicator
        if node.depth > 0 {
            painter.text(
                screen_pos + Vec2::new(0.0, -node_size.y / 2.0 - 8.0),
                egui::Align2::CENTER_BOTTOM,
                format!("L{}", node.depth),
                egui::FontId::proportional(8.0),
                Color32::from_rgb(148, 163, 184),
            );
        }
    }
}

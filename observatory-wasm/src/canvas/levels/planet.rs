//! Planet-level renderer — one entity with relationship graph.
//!
//! Central entity node with relationship edges and role badges.
//! Hierarchical hints for layout.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::GraphSceneModel;

use crate::state::CanvasApp;

/// Paint Planet-level: entity center + relationship nodes.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    app: &CanvasApp,
) {
    // Edges
    for edge in &scene.edges {
        let from = scene.nodes.iter().find(|n| n.id == edge.source);
        let to = scene.nodes.iter().find(|n| n.id == edge.target);
        if let (Some(f), Some(t)) = (from, to) {
            if let (Some(fp), Some(tp)) = (f.position_hint, t.position_hint) {
                let sf = transform.transform_pos(Pos2::new(fp.0, fp.1));
                let st = transform.transform_pos(Pos2::new(tp.0, tp.1));

                // Edge with label
                painter.line_segment(
                    [sf, st],
                    Stroke::new(1.5, Color32::from_rgb(100, 116, 139)),
                );

                if let Some(ref label) = edge.label {
                    let mid = Pos2::new((sf.x + st.x) / 2.0, (sf.y + st.y) / 2.0);
                    painter.text(
                        mid,
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(9.0),
                        Color32::from_rgb(148, 163, 184),
                    );
                }
            }
        }
    }

    // Nodes
    for (i, node) in scene.nodes.iter().enumerate() {
        let (x, y) = node.position_hint.unwrap_or((0.0, 0.0));
        let screen_pos = transform.transform_pos(Pos2::new(x, y));

        let is_center = i == 0;
        let radius = if is_center { 28.0 } else { 14.0 };
        let is_selected = app.interaction.selected_node.as_deref() == Some(&node.id);

        let fill = if is_center {
            Color32::from_rgb(139, 92, 246)
        } else {
            Color32::from_rgb(59, 130, 246)
        };

        painter.circle_filled(screen_pos, radius, fill);

        if is_selected {
            painter.circle_stroke(screen_pos, radius + 3.0, Stroke::new(2.0, Color32::from_rgb(245, 158, 11)));
        }

        // Badges
        for (bi, badge) in node.badges.iter().enumerate() {
            let badge_pos = screen_pos + Vec2::new(radius + 4.0, -radius + (bi as f32 * 14.0));
            painter.text(
                badge_pos,
                egui::Align2::LEFT_CENTER,
                &badge.label,
                egui::FontId::proportional(8.0),
                Color32::from_rgb(148, 163, 184),
            );
        }

        // Label
        painter.text(
            screen_pos + Vec2::new(0.0, radius + 8.0),
            egui::Align2::CENTER_TOP,
            &node.label,
            egui::FontId::proportional(if is_center { 12.0 } else { 10.0 }),
            Color32::from_rgb(226, 232, 240),
        );
    }
}

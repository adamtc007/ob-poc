//! Cluster-level renderer — force within fixed boundary.
//!
//! CBU nodes within a cluster, shared entities as smaller nodes.

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneNodeType};

use crate::state::CanvasApp;

/// Paint Cluster-level: CBU nodes within a constrained boundary.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    app: &CanvasApp,
) {
    // Boundary circle
    let center = transform.transform_pos(Pos2::ZERO);
    let boundary_radius = 400.0 * app.camera.zoom;
    painter.circle_stroke(
        center,
        boundary_radius,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(71, 85, 105, 40)),
    );

    // Edges
    for edge in &scene.edges {
        let from = scene.nodes.iter().find(|n| n.id == edge.source);
        let to = scene.nodes.iter().find(|n| n.id == edge.target);
        if let (Some(f), Some(t)) = (from, to) {
            if let (Some(fp), Some(tp)) = (f.position_hint, t.position_hint) {
                let sf = transform.transform_pos(Pos2::new(fp.0, fp.1));
                let st = transform.transform_pos(Pos2::new(tp.0, tp.1));
                painter.line_segment(
                    [sf, st],
                    Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 116, 139, 60)),
                );
            }
        }
    }

    // Nodes
    for node in &scene.nodes {
        let (x, y) = node.position_hint.unwrap_or((0.0, 0.0));
        let screen_pos = transform.transform_pos(Pos2::new(x, y));

        let radius = match node.node_type {
            SceneNodeType::Cbu => 18.0,
            SceneNodeType::Entity => 8.0,
            _ => 12.0,
        };

        let is_selected = app.interaction.selected_node.as_deref() == Some(&node.id);
        let color = match node.node_type {
            SceneNodeType::Cbu => Color32::from_rgb(139, 92, 246),
            _ => Color32::from_rgb(71, 85, 105),
        };

        painter.circle_filled(screen_pos, radius, color);

        if is_selected {
            painter.circle_stroke(screen_pos, radius + 3.0, Stroke::new(2.0, Color32::from_rgb(59, 130, 246)));
        }

        // Label for CBU nodes
        if matches!(node.node_type, SceneNodeType::Cbu) {
            painter.text(
                screen_pos + Vec2::new(0.0, radius + 6.0),
                egui::Align2::CENTER_TOP,
                &node.label,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(203, 213, 225),
            );
        }
    }
}

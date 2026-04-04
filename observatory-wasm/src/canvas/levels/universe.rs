//! Universe-level renderer — force-directed cluster bubbles.
//!
//! Clusters grouped organically. CBU counts shown as bubble size.
//! Fruchterman-Reingold layout (cached, not per-frame).

use egui::{Color32, Painter, Pos2, Stroke, Vec2};

use ob_poc_types::graph_scene::{GraphSceneModel, SceneNodeType};

use crate::state::ObservatoryState;

/// Paint Universe-level: cluster bubbles with cross-cluster edges.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    state: &ObservatoryState,
) {
    // Paint edges first (below nodes)
    for edge in &scene.edges {
        let from_pos = find_node_pos(scene, &edge.source);
        let to_pos = find_node_pos(scene, &edge.target);
        if let (Some(from), Some(to)) = (from_pos, to_pos) {
            let screen_from = transform.transform_pos(Pos2::new(from.0, from.1));
            let screen_to = transform.transform_pos(Pos2::new(to.0, to.1));
            painter.line_segment(
                [screen_from, screen_to],
                Stroke::new(edge.weight * 0.5, Color32::from_rgba_premultiplied(100, 116, 139, 80)),
            );
        }
    }

    // Paint nodes (clusters as bubbles)
    for node in &scene.nodes {
        let (x, y) = node.position_hint.unwrap_or((0.0, 0.0));
        let screen_pos = transform.transform_pos(Pos2::new(x, y));

        let base_radius = match node.node_type {
            SceneNodeType::Cluster | SceneNodeType::Aggregate => {
                20.0 + (node.child_count as f32).sqrt() * 8.0
            }
            _ => 12.0,
        };

        let is_selected = state.interaction.selected_node.as_deref() == Some(&node.id);
        let is_hovered = state.interaction.hovered_node.as_deref() == Some(&node.id);
        let radius = if is_hovered { base_radius * 1.1 } else { base_radius };

        // Bubble fill
        let fill = cluster_color(node.depth, &node.id);
        painter.circle_filled(screen_pos, radius, fill);

        // Selection ring
        if is_selected {
            painter.circle_stroke(screen_pos, radius + 3.0, Stroke::new(2.0, Color32::from_rgb(59, 130, 246)));
        }

        // Label
        painter.text(
            screen_pos + Vec2::new(0.0, radius + 8.0),
            egui::Align2::CENTER_TOP,
            &node.label,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(203, 213, 225),
        );

        // Child count badge
        if node.child_count > 0 {
            painter.text(
                screen_pos,
                egui::Align2::CENTER_CENTER,
                format!("{}", node.child_count),
                egui::FontId::proportional(10.0),
                Color32::WHITE,
            );
        }
    }
}

fn find_node_pos(scene: &GraphSceneModel, node_id: &str) -> Option<(f32, f32)> {
    scene.nodes.iter().find(|n| n.id == node_id).and_then(|n| n.position_hint)
}

fn cluster_color(depth: usize, id: &str) -> Color32 {
    // Deterministic color from ID hash
    let hash = id.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (hash % 360) as f32;
    let saturation = 0.5 + (depth as f32 * 0.1).min(0.3);
    hsv_to_rgb(hue, saturation, 0.6)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 { (c, x, 0.0) }
        else if h < 120.0 { (x, c, 0.0) }
        else if h < 180.0 { (0.0, c, x) }
        else if h < 240.0 { (0.0, x, c) }
        else if h < 300.0 { (x, 0.0, c) }
        else { (c, 0.0, x) };
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

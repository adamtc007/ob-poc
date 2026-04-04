//! Observation controls — minimap, anchor indicator, zoom indicator, reset button.
//!
//! These are overlays painted on top of the canvas. They modify the observation
//! frame only — never the semantic struct.

use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

use crate::actions::ObservatoryAction;
use crate::state::CanvasApp;

/// Paint observation controls as overlays on the canvas.
/// Returns action if user interacts with a control.
pub fn paint_controls(
    _ui: &egui::Ui,
    painter: &Painter,
    canvas_rect: &Rect,
    app: &CanvasApp,
) -> Option<ObservatoryAction> {
    let action = None;

    // ── Zoom indicator (bottom-left) ──
    let zoom_pos = Pos2::new(canvas_rect.left() + 12.0, canvas_rect.bottom() - 12.0);
    painter.text(
        zoom_pos,
        egui::Align2::LEFT_BOTTOM,
        format!("{:.0}%", app.camera.zoom * 100.0),
        egui::FontId::monospace(10.0),
        Color32::from_rgb(148, 163, 184),
    );

    // ── Anchor indicator (bottom-left, above zoom) ──
    if let Some(ref anchor_id) = app.camera.anchor_node_id {
        let anchor_pos = Pos2::new(canvas_rect.left() + 12.0, canvas_rect.bottom() - 28.0);
        painter.text(
            anchor_pos,
            egui::Align2::LEFT_BOTTOM,
            format!("Anchor: {anchor_id}"),
            egui::FontId::proportional(10.0),
            Color32::from_rgb(245, 158, 11),
        );
    }

    // ── Selected node indicator (bottom-right) ──
    if let Some(ref selected) = app.interaction.selected_node {
        let sel_pos = Pos2::new(canvas_rect.right() - 12.0, canvas_rect.bottom() - 12.0);
        painter.text(
            sel_pos,
            egui::Align2::RIGHT_BOTTOM,
            format!("Selected: {selected}"),
            egui::FontId::proportional(10.0),
            Color32::from_rgb(59, 130, 246),
        );
    }

    // ── Minimap (top-right corner) ──
    let minimap_size = Vec2::new(120.0, 80.0);
    let minimap_rect = Rect::from_min_size(
        Pos2::new(
            canvas_rect.right() - minimap_size.x - 8.0,
            canvas_rect.top() + 8.0,
        ),
        minimap_size,
    );

    // Minimap background
    painter.rect_filled(
        minimap_rect,
        4.0,
        Color32::from_rgba_premultiplied(15, 23, 42, 200),
    );
    painter.rect_stroke(
        minimap_rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(51, 65, 85)),
        egui::StrokeKind::Outside,
    );

    // Minimap: show node dots
    if let Some(ref scene) = app.scene {
        let scene_bounds = compute_scene_bounds(scene);
        if scene_bounds.width() > 0.0 && scene_bounds.height() > 0.0 {
            let mini_transform =
                egui::emath::RectTransform::from_to(scene_bounds, minimap_rect.shrink(4.0));

            for node in &scene.nodes {
                if let Some((x, y)) = node.position_hint {
                    let pos = mini_transform.transform_pos(Pos2::new(x, y));
                    painter.circle_filled(pos, 2.0, Color32::from_rgb(148, 163, 184));
                }
            }

            // Viewport indicator on minimap
            let cam = &app.camera;
            let vp_size = Vec2::splat(200.0 / cam.zoom);
            let vp_rect = Rect::from_center_size(Pos2::new(cam.pan_x, cam.pan_y), vp_size);
            let mini_vp = Rect::from_min_max(
                mini_transform.transform_pos(vp_rect.min),
                mini_transform.transform_pos(vp_rect.max),
            );
            painter.rect_stroke(
                mini_vp,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(59, 130, 246)),
                egui::StrokeKind::Outside,
            );
        }
    }

    // ── Reset view label (top-left) — actual button handled in canvas interaction ──
    painter.text(
        Pos2::new(canvas_rect.left() + 12.0, canvas_rect.top() + 12.0),
        egui::Align2::LEFT_TOP,
        "[R] Reset View",
        egui::FontId::proportional(10.0),
        Color32::from_rgb(100, 116, 139),
    );

    action
}

/// Compute bounding rect of all node positions in the scene.
fn compute_scene_bounds(
    scene: &ob_poc_types::graph_scene::GraphSceneModel,
) -> Rect {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for node in &scene.nodes {
        if let Some((x, y)) = node.position_hint {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if min_x > max_x {
        return Rect::from_center_size(Pos2::ZERO, Vec2::splat(100.0));
    }

    let padding = 50.0;
    Rect::from_min_max(
        Pos2::new(min_x - padding, min_y - padding),
        Pos2::new(max_x + padding, max_y + padding),
    )
}

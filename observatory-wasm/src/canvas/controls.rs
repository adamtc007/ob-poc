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
    ui: &egui::Ui,
    painter: &Painter,
    canvas_rect: &Rect,
    app: &CanvasApp,
) -> Option<ObservatoryAction> {
    let mut action = None;
    let world_bounds = app
        .render_cache
        .as_ref()
        .map(|cache| cache.world_bounds)
        .unwrap_or_else(super::default_world_bounds);

    paint_debug_hud(painter, canvas_rect, app);

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

        // ── Anchor line: dashed line from viewport center to anchored node ──
        if let (Some(scene), Some(cache)) = (app.scene.as_ref(), app.render_cache.as_ref()) {
            if let Some(anchor_world_pos) = cache.center_for_node(scene, anchor_id) {
                let viewport_center = canvas_rect.center();
                // Convert anchor world pos to screen pos using the same transform
                let transform = super::world_to_screen(&app.camera, canvas_rect, world_bounds);
                let anchor_screen = transform.transform_pos(anchor_world_pos);

                // Draw dashed line (simulated via segments)
                draw_dashed_line(
                    painter,
                    viewport_center,
                    anchor_screen,
                    8.0,
                    4.0,
                    Stroke::new(1.0, Color32::from_rgb(245, 158, 11)),
                );
            }
        }
    }

    // ── Focus-lock ring: pulsing ring around locked node ──
    if let Some(ref focus_id) = app.camera.focus_lock_node_id {
        if let (Some(scene), Some(cache)) = (app.scene.as_ref(), app.render_cache.as_ref()) {
            if let Some(focus_world_pos) = cache.center_for_node(scene, focus_id) {
                let transform = super::world_to_screen(&app.camera, canvas_rect, world_bounds);
                let focus_screen = transform.transform_pos(focus_world_pos);

                // Pulsing effect via time
                let t = ui.input(|i| i.time) as f32;
                let pulse = 1.0 + 0.15 * (t * 3.0).sin();
                let radius = 24.0 * pulse;

                painter.circle_stroke(
                    focus_screen,
                    radius,
                    Stroke::new(2.0, Color32::from_rgb(139, 92, 246)),
                );
                // Outer glow ring
                painter.circle_stroke(
                    focus_screen,
                    radius + 4.0,
                    Stroke::new(
                        1.0,
                        Color32::from_rgba_premultiplied(139, 92, 246, 80),
                    ),
                );

                // Request continuous repaint for animation
                ui.ctx().request_repaint();
            }
        }
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

    // Minimap: show node dots + viewport indicator
    if let (Some(_scene), Some(cache)) = (app.scene.as_ref(), app.render_cache.as_ref()) {
        let scene_bounds = cache.world_bounds;
        if scene_bounds.width() > 0.0 && scene_bounds.height() > 0.0 {
            let mini_transform =
                egui::emath::RectTransform::from_to(scene_bounds, minimap_rect.shrink(4.0));

            for geom in &cache.nodes {
                let pos = mini_transform.transform_pos(geom.center);
                painter.circle_filled(pos, 2.0, Color32::from_rgb(148, 163, 184));
            }

            // Viewport indicator on minimap
            let cam = &app.camera;
            let vp_rect = super::camera_world_rect(cam, canvas_rect, scene_bounds);
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

            // ── Minimap click-to-jump ──
            // Check if the pointer clicked inside the minimap rect
            if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                let clicked = ui.input(|i| i.pointer.any_click());
                if clicked && minimap_rect.contains(pointer_pos) {
                    // Inverse transform: minimap screen pos → world pos
                    let inverse_transform = egui::emath::RectTransform::from_to(
                        minimap_rect.shrink(4.0),
                        scene_bounds,
                    );
                    let world_pos = inverse_transform.transform_pos(pointer_pos);
                    action = Some(ObservatoryAction::Pan {
                        // Pan action expects deltas, but we want to set absolute position.
                        // We compute the delta from current camera pan to target.
                        dx: -(world_pos.x - cam.pan_x) * cam.zoom,
                        dy: -(world_pos.y - cam.pan_y) * cam.zoom,
                    });
                }
            }
        }
    }

    // ── Reset view label (top-left) — actual button handled in canvas interaction ──
    painter.text(
        Pos2::new(canvas_rect.left() + 12.0, canvas_rect.top() + 110.0),
        egui::Align2::LEFT_TOP,
        "[R] Reset View  [Esc] Deselect",
        egui::FontId::proportional(10.0),
        Color32::from_rgb(100, 116, 139),
    );

    action
}

fn paint_debug_hud(painter: &Painter, canvas_rect: &Rect, app: &CanvasApp) {
    let (layout_strategy, generation, node_count, edge_count) = app
        .scene
        .as_ref()
        .map(|scene| {
            (
                format!("{:?}", scene.layout_strategy),
                scene.generation,
                scene.nodes.len(),
                scene.edges.len(),
            )
        })
        .unwrap_or_else(|| ("none".into(), 0, 0, 0));

    let hovered = app.interaction.hovered_node.as_deref().unwrap_or("—");
    let selected = app.interaction.selected_node.as_deref().unwrap_or("—");
    let focus = app.camera.focus_lock_node_id.as_deref().unwrap_or("—");
    let debug_lines = [
        format!("scene gen: {generation}"),
        format!("layout: {layout_strategy}"),
        format!("nodes/edges: {node_count}/{edge_count}"),
        format!("hovered: {hovered}"),
        format!("selected: {selected}"),
        format!("focus: {focus}"),
        format!("zoom: {:.2}", app.camera.zoom),
        format!("pan: {:.1}, {:.1}", app.camera.pan_x, app.camera.pan_y),
    ];

    let anchor = Pos2::new(canvas_rect.left() + 12.0, canvas_rect.top() + 12.0);
    let line_height = 12.0;
    let panel_height = debug_lines.len() as f32 * line_height + 10.0;
    let panel_width = 220.0;
    let panel_rect = Rect::from_min_size(anchor, Vec2::new(panel_width, panel_height));

    painter.rect_filled(
        panel_rect,
        4.0,
        Color32::from_rgba_premultiplied(15, 23, 42, 190),
    );
    painter.rect_stroke(
        panel_rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(51, 65, 85)),
        egui::StrokeKind::Outside,
    );

    for (idx, line) in debug_lines.iter().enumerate() {
        painter.text(
            Pos2::new(anchor.x + 6.0, anchor.y + 5.0 + idx as f32 * line_height),
            egui::Align2::LEFT_TOP,
            line,
            egui::FontId::monospace(10.0),
            Color32::from_rgb(148, 163, 184),
        );
    }
}

/// Draw a dashed line between two screen-space points.
fn draw_dashed_line(
    painter: &Painter,
    from: Pos2,
    to: Pos2,
    dash_len: f32,
    gap_len: f32,
    stroke: Stroke,
) {
    let dir = to - from;
    let total_len = dir.length();
    if total_len < 1.0 {
        return;
    }
    let unit = dir / total_len;
    let mut dist = 0.0;
    while dist < total_len {
        let seg_start = from + unit * dist;
        let seg_end_dist = (dist + dash_len).min(total_len);
        let seg_end = from + unit * seg_end_dist;
        painter.line_segment([seg_start, seg_end], stroke);
        dist = seg_end_dist + gap_len;
    }
}

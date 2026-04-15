//! Canvas module — painter-driven constellation renderer.
//!
//! Uses allocate_painter() + RectTransform for world-to-screen mapping.
//! Level-specific renderers in canvas/levels/.

pub mod controls;
pub mod layout;
pub mod levels;

use egui::{Ui, Vec2};

use crate::actions::ObservatoryAction;
use crate::state::CanvasApp;

/// Render the central constellation canvas. Returns action on interaction.
pub fn render(ui: &mut Ui, app: &mut CanvasApp) -> Option<ObservatoryAction> {
    let available = ui.available_size();
    let (response, painter) =
        ui.allocate_painter(available, egui::Sense::click_and_drag());

    // ── Background (depth-encoded, with transition blending) ──
    let colors = ob_poc_types::galaxy::DepthColors::default();
    if let Some(ref trans) = app.transition {
        // Blend between from_level and to_level colors during transition
        let from_t = trans.from_level as usize as f32 / 5.0;
        let to_t = trans.to_level as usize as f32 / 5.0;
        let t = trans.t();
        let (fr, fg, fb) = colors.color_at(from_t);
        let (tr, tg, tb) = colors.color_at(to_t);
        let r = (fr as f32 + (tr as f32 - fr as f32) * t) as u8;
        let g = (fg as f32 + (tg as f32 - fg as f32) * t) as u8;
        let b = (fb as f32 + (tb as f32 - fb as f32) * t) as u8;
        painter.rect_filled(response.rect, 0.0, egui::Color32::from_rgb(r, g, b));
    } else {
        let visible_level = app
            .scene
            .as_ref()
            .map(|scene| scene.level)
            .unwrap_or(app.current_level);
        let depth_t = visible_level as usize as f32 / 5.0;
        let (r, g, b) = colors.color_at(depth_t);
        painter.rect_filled(response.rect, 0.0, egui::Color32::from_rgb(r, g, b));
    }

    // ── World-to-screen transform (with transition zoom interpolation) ──
    let world_bounds = app
        .render_cache
        .as_ref()
        .map(|cache| cache.world_bounds)
        .unwrap_or_else(default_world_bounds);
    let transform = if let Some(ref trans) = app.transition {
        // Smoothly interpolate camera zoom toward the drill target during transition
        let t = trans.t();
        let from_zoom = app.camera.zoom;
        // Target zoom: scale by level distance (deeper = more zoomed in)
        let level_delta = trans.to_level as usize as f32 - trans.from_level as usize as f32;
        let zoom_factor = 1.0 + level_delta * 0.3 * t;
        let mut cam = app.camera.clone();
        cam.zoom = (from_zoom * zoom_factor).clamp(0.05, 10.0);
        world_to_screen(&cam, &response.rect, world_bounds)
    } else {
        world_to_screen(&app.camera, &response.rect, world_bounds)
    };

    // ── Hover detection (before painting so nodes get immediate visual feedback) ──
    app.interaction.hovered_node = None;
    if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
        if response.rect.contains(pointer_pos) {
            let world_pos = transform.inverse().transform_pos(pointer_pos);
            if let (Some(scene), Some(cache)) = (app.scene.as_ref(), app.render_cache.as_ref()) {
                if let Some(hovered_id) = cache.hit_test(scene, world_pos) {
                    app.interaction.hovered_node = Some(hovered_id.clone());
                }
            }
        }
    }

    // ── Paint scene by level ──
    if let Some(ref scene) = app.scene {
        levels::paint(&painter, &transform, scene, app);
    } else {
        painter.text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            "No constellation data",
            egui::FontId::proportional(16.0),
            egui::Color32::from_rgb(100, 116, 139),
        );
    }

    // ── Paint observation controls (overlays) ──
    if let Some(ctrl_action) = controls::paint_controls(ui, &painter, &response.rect, app) {
        return Some(ctrl_action);
    }

    // ── Tooltip (painted after nodes so it appears on top) ──
    if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
        if let Some(ref hovered_id) = app.interaction.hovered_node {
            if let Some(ref scene) = app.scene {
                if let Some(node) = scene.nodes.iter().find(|n| n.id == *hovered_id) {
                    paint_node_tooltip(ui, pointer_pos, node, scene);
                }
            }
        }
    }

    // ── Handle interaction ──
    let mut action = None;

    if ui.input(|i| i.key_pressed(egui::Key::R)) {
        return Some(ObservatoryAction::ResetView);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Some(ObservatoryAction::DeselectNode);
    }

    // Pan (drag)
    if response.dragged() {
        let delta = response.drag_delta();
        action = Some(ObservatoryAction::Pan {
            dx: delta.x,
            dy: delta.y,
        });
    }

    // Zoom (scroll)
    if response.hovered() {
        let scroll = response.ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            action = Some(ObservatoryAction::VisualZoom { delta: scroll });
        }
    }

    // Click → select / double-click → drill
    if response.clicked() || response.double_clicked() {
        if let Some(pointer_pos) = response.hover_pos() {
            let world_pos = transform.inverse().transform_pos(pointer_pos);
            if let (Some(scene), Some(cache)) = (app.scene.as_ref(), app.render_cache.as_ref()) {
                if let Some(node_id) = cache.hit_test(scene, world_pos) {
                    if response.double_clicked() {
                        if let Some(drill) = resolve_drill_target(scene, &node_id) {
                            action = Some(ObservatoryAction::Drill {
                                node_id: node_id.clone(),
                                target_level: drill,
                            });
                        } else {
                            action = Some(ObservatoryAction::SelectNode { node_id });
                        }
                    } else {
                        // Selection — local only, NOT semantic focus
                        action = Some(ObservatoryAction::SelectNode { node_id });
                    }
                } else if response.clicked() {
                    action = Some(ObservatoryAction::DeselectNode);
                }
            }
        }
    }

    // Middle-click → anchor / clear anchor
    if response.middle_clicked() {
        if let Some(pointer_pos) = response.hover_pos() {
            let world_pos = transform.inverse().transform_pos(pointer_pos);
            if let (Some(scene), Some(cache)) = (app.scene.as_ref(), app.render_cache.as_ref()) {
                if let Some(node_id) = cache.hit_test(scene, world_pos) {
                    // Middle-click on a node: anchor to it
                    action = Some(ObservatoryAction::AnchorNode { node_id });
                } else {
                    // Middle-click on empty space: clear anchor
                    action = Some(ObservatoryAction::ClearAnchor);
                }
            } else {
                action = Some(ObservatoryAction::ClearAnchor);
            }
        }
    }

    action
}

/// Paint a rich tooltip for a hovered node.
fn paint_node_tooltip(
    ui: &mut Ui,
    screen_pos: egui::Pos2,
    node: &ob_poc_types::graph_scene::SceneNode,
    scene: &ob_poc_types::graph_scene::GraphSceneModel,
) {
    let tooltip_id = egui::Id::new("node_tooltip");
    egui::Area::new(tooltip_id)
        .fixed_pos(egui::Pos2::new(screen_pos.x + 16.0, screen_pos.y + 16.0))
        .order(egui::Order::Tooltip)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .fill(egui::Color32::from_rgba_premultiplied(20, 24, 33, 240))
                .corner_radius(6.0)
                .inner_margin(10.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 65, 81)))
                .show(ui, |ui| {
                    ui.set_max_width(280.0);

                    // Title
                    ui.label(
                        egui::RichText::new(&node.label)
                            .strong()
                            .size(14.0)
                            .color(egui::Color32::from_rgb(229, 231, 235)),
                    );

                    ui.add_space(4.0);

                    // Node type + state
                    let type_str = format!("{:?}", node.node_type);
                    let state_str = node.state.as_deref().unwrap_or("—");
                    ui.horizontal(|ui| {
                        badge(ui, &type_str, egui::Color32::from_rgb(99, 102, 241));
                        badge(ui, state_str, if node.blocking {
                            egui::Color32::from_rgb(239, 68, 68)
                        } else {
                            egui::Color32::from_rgb(34, 197, 94)
                        });
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Fields grid
                    egui::Grid::new("tooltip_grid")
                        .num_columns(2)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            tooltip_row(ui, "ID", &node.id);
                            tooltip_row(ui, "Progress", &format!("{}%", node.progress));
                            tooltip_row(ui, "Blocking", if node.blocking { "YES" } else { "no" });
                            tooltip_row(ui, "Depth", &node.depth.to_string());
                            tooltip_row(ui, "Children", &node.child_count.to_string());
                            if let Some(ref group) = node.group_id {
                                tooltip_row(ui, "Group", group);
                            }
                        });

                    // Badges
                    if !node.badges.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            for b in &node.badges {
                                badge(ui, &b.label, egui::Color32::from_rgb(251, 191, 36));
                            }
                        });
                    }

                    // Connected edges
                    let edges: Vec<_> = scene.edges.iter().filter(|e| {
                        e.source == node.id || e.target == node.id
                    }).collect();
                    if !edges.is_empty() {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new("Connections")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(156, 163, 175)),
                        );
                        for edge in edges.iter().take(6) {
                            let peer = if edge.source == node.id { &edge.target } else { &edge.source };
                            let label = edge.label.as_deref().unwrap_or("");
                            let dir = if edge.source == node.id { "→" } else { "←" };
                            ui.label(
                                egui::RichText::new(format!("  {dir} {peer} ({label})"))
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(156, 163, 175)),
                            );
                        }
                        if edges.len() > 6 {
                            ui.label(
                                egui::RichText::new(format!("  ... +{} more", edges.len() - 6))
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(107, 114, 128)),
                            );
                        }
                    }

                    // Drill targets
                    let drills: Vec<_> = scene.drill_targets.iter().filter(|d| d.node_id == node.id).collect();
                    if !drills.is_empty() {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new("Drill Targets")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(156, 163, 175)),
                        );
                        for d in &drills {
                            ui.label(
                                egui::RichText::new(format!("  ⬇ {} → {:?}", d.drill_label, d.target_level))
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(129, 140, 248)),
                            );
                        }
                    }

                    let drill_target = resolve_drill_target(scene, &node.id);

                    // Hint
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(if drill_target.is_some() {
                            "Click to select · Double-click to drill"
                        } else {
                            "Click to select"
                        })
                            .size(9.0)
                            .italics()
                            .color(egui::Color32::from_rgb(107, 114, 128)),
                    );
                });
        });
}

fn resolve_drill_target(
    scene: &ob_poc_types::graph_scene::GraphSceneModel,
    node_id: &str,
) -> Option<ob_poc_types::galaxy::ViewLevel> {
    scene
        .drill_targets
        .iter()
        .find(|target| target.node_id == node_id)
        .map(|target| target.target_level)
}

fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let bg = egui::Color32::from_rgba_premultiplied(color.r() / 4, color.g() / 4, color.b() / 4, 200);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(5, 1))
        .stroke(egui::Stroke::new(0.5, color))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(10.0)
                    .color(color),
            );
        });
}

fn tooltip_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(10.0)
            .color(egui::Color32::from_rgb(156, 163, 175)),
    );
    ui.label(
        egui::RichText::new(value)
            .size(10.0)
            .color(egui::Color32::from_rgb(229, 231, 235)),
    );
    ui.end_row();
}

/// Compute world-to-screen transform from camera state.
/// Uses RectTransform — not manual matrix math.
pub(crate) fn world_to_screen(
    camera: &crate::state::ObservationFrame,
    canvas_rect: &egui::Rect,
    world_bounds: egui::Rect,
) -> egui::emath::RectTransform {
    let world_rect = camera_world_rect(camera, canvas_rect, world_bounds);
    egui::emath::RectTransform::from_to(world_rect, *canvas_rect)
}

pub(crate) fn camera_world_rect(
    camera: &crate::state::ObservationFrame,
    canvas_rect: &egui::Rect,
    world_bounds: egui::Rect,
) -> egui::Rect {
    let aspect = (canvas_rect.width() / canvas_rect.height().max(1.0)).max(0.1);
    let margin = 1.2;
    let base_width = world_bounds.width().max(200.0);
    let base_height = world_bounds.height().max(200.0);
    let world_width = (base_width.max(base_height * aspect) * margin) / camera.zoom.max(0.05);
    let world_height =
        (base_height.max(base_width / aspect) * margin) / camera.zoom.max(0.05);
    egui::Rect::from_center_size(
        egui::Pos2::new(camera.pan_x, camera.pan_y),
        Vec2::new(world_width, world_height),
    )
}

pub(crate) fn default_world_bounds() -> egui::Rect {
    egui::Rect::from_center_size(egui::Pos2::ZERO, Vec2::splat(400.0))
}

#[cfg(test)]
mod tests {
    use super::resolve_drill_target;
    use ob_poc_types::galaxy::ViewLevel;
    use ob_poc_types::graph_scene::{
        DrillTarget, GraphSceneModel, LayoutStrategy, SceneEdge, SceneGroup, SceneNode, SceneNodeType,
    };

    fn scene_with_drill_targets() -> GraphSceneModel {
        GraphSceneModel {
            generation: 7,
            level: ViewLevel::System,
            layout_strategy: LayoutStrategy::DeterministicOrbital,
            nodes: vec![SceneNode {
                id: "cbu".into(),
                label: "CBU".into(),
                node_type: SceneNodeType::Cbu,
                state: None,
                progress: 0,
                blocking: false,
                depth: 0,
                position_hint: Some((0.0, 0.0)),
                badges: vec![],
                child_count: 1,
                group_id: None,
            }],
            edges: Vec::<SceneEdge>::new(),
            groups: Vec::<SceneGroup>::new(),
            drill_targets: vec![DrillTarget {
                node_id: "cbu".into(),
                target_level: ViewLevel::Planet,
                drill_label: "View entities".into(),
            }],
            max_depth: 1,
        }
    }

    #[test]
    fn resolve_drill_target_returns_declared_target_level() {
        let scene = scene_with_drill_targets();
        assert_eq!(resolve_drill_target(&scene, "cbu"), Some(ViewLevel::Planet));
        assert_eq!(resolve_drill_target(&scene, "missing"), None);
    }
}

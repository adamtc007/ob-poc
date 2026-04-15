//! Canvas module — painter-driven constellation renderer.
//!
//! Uses allocate_painter() + RectTransform for world-to-screen mapping.
//! Level-specific renderers in canvas/levels/.

pub mod controls;
pub mod levels;

use egui::{Ui, Vec2};

use crate::actions::ObservatoryAction;
use crate::state::CanvasApp;

/// Render the central constellation canvas. Returns action on interaction.
pub fn render(ui: &mut Ui, app: &CanvasApp) -> Option<ObservatoryAction> {
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
        let depth_t = app.current_level as usize as f32 / 5.0;
        let (r, g, b) = colors.color_at(depth_t);
        painter.rect_filled(response.rect, 0.0, egui::Color32::from_rgb(r, g, b));
    }

    // ── World-to-screen transform (with transition zoom interpolation) ──
    let transform = if let Some(ref trans) = app.transition {
        // Smoothly interpolate camera zoom toward the drill target during transition
        let t = trans.t();
        let from_zoom = app.camera.zoom;
        // Target zoom: scale by level distance (deeper = more zoomed in)
        let level_delta = trans.to_level as usize as f32 - trans.from_level as usize as f32;
        let zoom_factor = 1.0 + level_delta * 0.3 * t;
        let mut cam = app.camera.clone();
        cam.zoom = (from_zoom * zoom_factor).clamp(0.05, 10.0);
        world_to_screen(&cam, &response.rect)
    } else {
        world_to_screen(&app.camera, &response.rect)
    };

    // ── Paint scene by level ──
    if let Some(ref scene) = app.scene {
        levels::paint(&painter, &transform, scene, app);
    } else {
        // No scene loaded — show placeholder
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

    // ── Hover detection + tooltip ──
    // Request continuous repaint when pointer is over canvas so hover detection is responsive
    if response.hovered() {
        ui.ctx().request_repaint();
    }
    if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
        if response.rect.contains(pointer_pos) {
            let world_pos = transform.inverse().transform_pos(pointer_pos);
            if let Some(ref scene) = app.scene {
                if let Some(hovered_id) = hit_test(scene, world_pos) {
                    if let Some(node) = scene.nodes.iter().find(|n| n.id == hovered_id) {
                        paint_node_tooltip(ui, pointer_pos, node, scene);
                    }
                }
            }
        }
    }

    // ── Handle interaction ──
    let mut action = None;

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
            if let Some(ref scene) = app.scene {
                if let Some(node_id) = hit_test(scene, world_pos) {
                    if response.double_clicked() {
                        // Semantic drill — requires server round-trip
                        action = Some(ObservatoryAction::Drill {
                            node_id: node_id.clone(),
                            target_level: ob_poc_types::galaxy::ViewLevel::default(),
                        });
                    } else {
                        // Selection — local only, NOT semantic focus
                        action = Some(ObservatoryAction::SelectNode { node_id });
                    }
                }
            }
        }
    }

    // Middle-click → anchor / clear anchor
    if response.middle_clicked() {
        if let Some(pointer_pos) = response.hover_pos() {
            let world_pos = transform.inverse().transform_pos(pointer_pos);
            if let Some(ref scene) = app.scene {
                if let Some(node_id) = hit_test(scene, world_pos) {
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

                    // Hint
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Click to select · Double-click to drill")
                            .size(9.0)
                            .italics()
                            .color(egui::Color32::from_rgb(107, 114, 128)),
                    );
                });
        });
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
) -> egui::emath::RectTransform {
    let world_size = Vec2::splat(2000.0 / camera.zoom);
    let world_rect = egui::Rect::from_center_size(
        egui::Pos2::new(camera.pan_x, camera.pan_y),
        world_size,
    );
    egui::emath::RectTransform::from_to(world_rect, *canvas_rect)
}

/// Hit test: find the nearest scene node to a world-space point.
/// Computes positions using the same orbital algorithm as the level renderers.
fn hit_test(
    scene: &ob_poc_types::graph_scene::GraphSceneModel,
    world_pos: egui::Pos2,
) -> Option<String> {
    let hit_radius = 30.0;
    let positions = compute_node_positions(&scene.nodes);

    let mut best: Option<(String, f32)> = None;
    for (i, node) in scene.nodes.iter().enumerate() {
        let (nx, ny) = positions[i];
        let dist = ((world_pos.x - nx).powi(2) + (world_pos.y - ny).powi(2)).sqrt();
        if dist < hit_radius {
            if best.as_ref().map_or(true, |(_, d)| dist < *d) {
                best = Some((node.id.clone(), dist));
            }
        }
    }

    best.map(|(id, _)| id)
}

/// Compute node positions using the same orbital algorithm as the level renderers.
/// First node at center, remaining in concentric rings.
pub(crate) fn compute_node_positions(nodes: &[ob_poc_types::graph_scene::SceneNode]) -> Vec<(f32, f32)> {
    use std::f32::consts::TAU;
    let mut positions = Vec::with_capacity(nodes.len());

    for (i, node) in nodes.iter().enumerate() {
        // Use server-provided position_hint if available
        if let Some(pos) = node.position_hint {
            positions.push(pos);
            continue;
        }
        // Otherwise compute orbital position (same as system.rs)
        if i == 0 {
            positions.push((0.0, 0.0));
        } else {
            let orbital_idx = i - 1;
            let ring_capacity = 12;
            let ring = orbital_idx / ring_capacity;
            let idx_in_ring = orbital_idx % ring_capacity;
            let total = nodes.len() - 1;
            let nodes_in_ring = if (ring + 1) * ring_capacity <= total {
                ring_capacity
            } else {
                total - ring * ring_capacity
            };
            let ring_radius = 200.0 + (ring as f32) * 150.0;
            let angle = (idx_in_ring as f32 / nodes_in_ring as f32) * TAU - TAU / 4.0;
            positions.push((angle.cos() * ring_radius, angle.sin() * ring_radius));
        }
    }
    positions
}

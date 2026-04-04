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

//! Canvas module — painter-driven constellation renderer.
//!
//! Uses allocate_painter() + RectTransform for world-to-screen mapping.
//! Level-specific renderers in canvas/levels/.

pub mod controls;
pub mod levels;

use egui::{Ui, Vec2};

use crate::actions::ObservatoryAction;
use crate::state::ObservatoryState;

/// Render the central constellation canvas. Returns action on interaction.
pub fn render(ui: &mut Ui, state: &ObservatoryState) -> Option<ObservatoryAction> {
    let available = ui.available_size();
    let (response, painter) =
        ui.allocate_painter(available, egui::Sense::click_and_drag());

    // ── Background ──
    painter.rect_filled(response.rect, 0.0, egui::Color32::from_rgb(15, 23, 42));

    // ── World-to-screen transform ──
    let transform = world_to_screen(&state.camera, &response.rect);

    // ── Paint scene by level ──
    if let Some(ref scene) = state.fetch.graph_scene.as_ready() {
        levels::paint(&painter, &transform, scene, state);
    } else {
        // No scene loaded — show placeholder
        painter.text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            if state.fetch.graph_scene.is_pending() {
                "Loading constellation..."
            } else {
                "No constellation data"
            },
            egui::FontId::proportional(16.0),
            egui::Color32::from_rgb(100, 116, 139),
        );
    }

    // ── Paint observation controls (overlays) ──
    if let Some(ctrl_action) = controls::paint_controls(ui, &painter, &response.rect, state) {
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
            if let Some(ref scene) = state.fetch.graph_scene.as_ready() {
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

    action
}

/// Compute world-to-screen transform from camera state.
/// Uses RectTransform — not manual matrix math.
fn world_to_screen(
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
fn hit_test(
    scene: &ob_poc_types::graph_scene::GraphSceneModel,
    world_pos: egui::Pos2,
) -> Option<String> {
    let hit_radius = 30.0; // world-space radius

    for node in &scene.nodes {
        if let Some((nx, ny)) = node.position_hint {
            let dist = ((world_pos.x - nx).powi(2) + (world_pos.y - ny).powi(2)).sqrt();
            if dist < hit_radius {
                return Some(node.id.clone());
            }
        }
    }

    None
}

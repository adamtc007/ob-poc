//! Overlay rendering for HUD elements.
//!
//! Renders minimap, breadcrumb navigation, selection info, and debug overlays.

use crate::{camera::CameraState, config::OverlayRenderConfig, style::RenderStyle};
use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Stroke, Ui, Vec2};
use esper_core::DroneState;
use esper_snapshot::{ChamberSnapshot, WorldSnapshot};

/// Overlay renderer for HUD elements.
#[derive(Debug, Clone)]
pub struct OverlayRenderer {
    /// Overlay configuration.
    config: OverlayRenderConfig,
}

impl Default for OverlayRenderer {
    fn default() -> Self {
        Self::new(OverlayRenderConfig::default())
    }
}

impl OverlayRenderer {
    /// Create a new overlay renderer.
    pub fn new(config: OverlayRenderConfig) -> Self {
        Self { config }
    }

    /// Render all overlays.
    pub fn render(
        &self,
        ui: &mut Ui,
        camera: &CameraState,
        drone: &DroneState,
        world: &WorldSnapshot,
        style: &RenderStyle,
        fps: Option<f32>,
    ) {
        let screen_rect = ui.available_rect_before_wrap();

        if self.config.show_minimap {
            self.render_minimap(ui, camera, world, style, screen_rect);
        }

        if self.config.show_breadcrumb {
            self.render_breadcrumb(ui, drone, world, style, screen_rect);
        }

        if self.config.show_selection_info {
            if let Some(selection) = drone.taxonomy.selection {
                self.render_selection_info(ui, selection, world, style, screen_rect);
            }
        }

        if self.config.show_debug {
            self.render_debug_info(ui, camera, drone, style, screen_rect, fps);
        }
    }

    /// Render minimap in corner of screen.
    fn render_minimap(
        &self,
        ui: &mut Ui,
        camera: &CameraState,
        world: &WorldSnapshot,
        style: &RenderStyle,
        screen_rect: Rect,
    ) {
        let minimap_size = screen_rect.width().min(screen_rect.height()) * self.config.minimap_size;
        let padding = 10.0;

        // Position in bottom-right corner
        let minimap_rect = Rect::from_min_size(
            Pos2::new(
                screen_rect.max.x - minimap_size - padding,
                screen_rect.max.y - minimap_size - padding,
            ),
            Vec2::splat(minimap_size),
        );

        let painter = ui.painter();

        // Background
        painter.rect_filled(
            minimap_rect,
            Rounding::same(4.0),
            Color32::from_rgba_unmultiplied(0, 0, 0, 180),
        );

        // Border
        painter.rect_stroke(
            minimap_rect,
            Rounding::same(4.0),
            Stroke::new(1.0, style.text.secondary),
        );

        // If we have a chamber, draw entity dots
        if let Some(chamber) = world.chambers.first() {
            self.draw_minimap_entities(painter, chamber, minimap_rect, style);
        }

        // Draw viewport indicator
        let visible_rect = camera.visible_world_rect();
        if let Some(chamber) = world.chambers.first() {
            self.draw_minimap_viewport(painter, visible_rect, chamber.bounds, minimap_rect, style);
        }
    }

    /// Draw entity dots on minimap.
    fn draw_minimap_entities(
        &self,
        painter: &egui::Painter,
        chamber: &ChamberSnapshot,
        minimap_rect: Rect,
        style: &RenderStyle,
    ) {
        let world_bounds = chamber.bounds;
        let scale_x = minimap_rect.width() / world_bounds.width();
        let scale_y = minimap_rect.height() / world_bounds.height();
        let scale = scale_x.min(scale_y) * 0.9; // 90% to leave margin

        let offset_x = minimap_rect.center().x - world_bounds.center().x * scale;
        let offset_y = minimap_rect.center().y - world_bounds.center().y * scale;

        for idx in 0..chamber.entity_ids.len() {
            let x = chamber.x[idx];
            let y = chamber.y[idx];

            let screen_x = x * scale + offset_x;
            let screen_y = y * scale + offset_y;

            if minimap_rect.contains(Pos2::new(screen_x, screen_y)) {
                painter.circle_filled(Pos2::new(screen_x, screen_y), 1.5, style.entity.fill);
            }
        }
    }

    /// Draw viewport indicator on minimap.
    fn draw_minimap_viewport(
        &self,
        painter: &egui::Painter,
        visible_rect: Rect,
        world_bounds: esper_snapshot::Rect,
        minimap_rect: Rect,
        style: &RenderStyle,
    ) {
        // Convert world_bounds to egui Rect
        let wb = Rect::from_min_max(
            Pos2::new(world_bounds.min.x, world_bounds.min.y),
            Pos2::new(world_bounds.max.x, world_bounds.max.y),
        );

        let scale_x = minimap_rect.width() / wb.width();
        let scale_y = minimap_rect.height() / wb.height();
        let scale = scale_x.min(scale_y) * 0.9;

        let offset_x = minimap_rect.center().x - wb.center().x * scale;
        let offset_y = minimap_rect.center().y - wb.center().y * scale;

        let viewport_min = Pos2::new(
            visible_rect.min.x * scale + offset_x,
            visible_rect.min.y * scale + offset_y,
        );
        let viewport_max = Pos2::new(
            visible_rect.max.x * scale + offset_x,
            visible_rect.max.y * scale + offset_y,
        );

        let viewport_rect = Rect::from_min_max(viewport_min, viewport_max);

        // Clamp to minimap bounds
        let clamped = viewport_rect.intersect(minimap_rect);

        painter.rect(
            clamped,
            Rounding::ZERO,
            Color32::from_rgba_unmultiplied(100, 180, 255, 40),
            Stroke::new(1.0, style.selection.stroke),
        );
    }

    /// Render breadcrumb navigation path.
    fn render_breadcrumb(
        &self,
        ui: &mut Ui,
        drone: &DroneState,
        world: &WorldSnapshot,
        style: &RenderStyle,
        screen_rect: Rect,
    ) {
        let padding = 10.0;
        let y_pos = screen_rect.min.y + padding;

        // Build breadcrumb path from context stack
        let mut path_parts: Vec<String> = vec!["Root".to_string()];

        // Add current chamber name if available
        let chamber_idx = drone.current_chamber as usize;
        if let Some(chamber) = world.chambers.get(chamber_idx) {
            path_parts.push(format!("Chamber {:?}", chamber.kind));
        }

        // Add focus path if any
        for &idx in &drone.taxonomy.focus_path {
            path_parts.push(format!("Node {}", idx));
        }

        let breadcrumb_text = path_parts.join(" > ");

        ui.painter().text(
            Pos2::new(screen_rect.min.x + padding, y_pos),
            Align2::LEFT_TOP,
            breadcrumb_text,
            FontId::proportional(14.0),
            style.text.secondary,
        );
    }

    /// Render selection info panel.
    fn render_selection_info(
        &self,
        ui: &mut Ui,
        selection: u32,
        world: &WorldSnapshot,
        style: &RenderStyle,
        screen_rect: Rect,
    ) {
        let panel_width = 200.0;
        let panel_height = 100.0;
        let padding = 10.0;

        let panel_rect = Rect::from_min_size(
            Pos2::new(
                screen_rect.min.x + padding,
                screen_rect.max.y - panel_height - padding,
            ),
            Vec2::new(panel_width, panel_height),
        );

        let painter = ui.painter();

        // Background
        painter.rect_filled(
            panel_rect,
            Rounding::same(4.0),
            Color32::from_rgba_unmultiplied(0, 0, 0, 200),
        );

        // Title
        painter.text(
            panel_rect.min + Vec2::new(10.0, 10.0),
            Align2::LEFT_TOP,
            "Selection",
            FontId::proportional(12.0),
            style.text.secondary,
        );

        // Selection details
        if let Some(chamber) = world.chambers.first() {
            let idx = selection as usize;
            if idx < chamber.entity_ids.len() {
                let entity_id = chamber.entity_ids[idx];

                painter.text(
                    panel_rect.min + Vec2::new(10.0, 30.0),
                    Align2::LEFT_TOP,
                    format!("ID: {}", entity_id),
                    FontId::monospace(11.0),
                    style.text.primary,
                );

                painter.text(
                    panel_rect.min + Vec2::new(10.0, 50.0),
                    Align2::LEFT_TOP,
                    format!("Index: {}", idx),
                    FontId::proportional(12.0),
                    style.text.primary,
                );
            }
        }
    }

    /// Render debug information overlay.
    fn render_debug_info(
        &self,
        ui: &mut Ui,
        camera: &CameraState,
        drone: &DroneState,
        style: &RenderStyle,
        screen_rect: Rect,
        fps: Option<f32>,
    ) {
        let padding = 10.0;
        let line_height = 16.0;
        let mut y = screen_rect.min.y + padding + 30.0; // Below breadcrumb

        let painter = ui.painter();

        let mut draw_line = |text: &str| {
            painter.text(
                Pos2::new(screen_rect.max.x - padding, y),
                Align2::RIGHT_TOP,
                text,
                FontId::monospace(11.0),
                style.text.secondary,
            );
            y += line_height;
        };

        // FPS
        if let Some(fps) = fps {
            draw_line(&format!("FPS: {:.1}", fps));
        }

        // Camera info
        draw_line(&format!("Zoom: {:.2}x", camera.zoom));
        draw_line(&format!(
            "Center: ({:.0}, {:.0})",
            camera.center.x, camera.center.y
        ));

        // Navigation state
        draw_line(&format!("Chamber: {}", drone.current_chamber));
        draw_line(&format!("Mode: {:?}", drone.mode));
        draw_line(&format!("Phase: {:?}", drone.phase()));

        if let Some(sel) = drone.taxonomy.selection {
            draw_line(&format!("Selection: {}", sel));
        }

        draw_line(&format!("Context depth: {}", drone.context_stack.depth()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_renderer_default() {
        let renderer = OverlayRenderer::default();
        assert!(renderer.config.show_minimap);
        assert!(renderer.config.show_breadcrumb);
    }

    #[test]
    fn overlay_config() {
        let config = OverlayRenderConfig {
            show_minimap: false,
            show_breadcrumb: true,
            show_selection_info: true,
            show_debug: false,
            ..Default::default()
        };
        let renderer = OverlayRenderer::new(config);
        assert!(!renderer.config.show_minimap);
        assert!(renderer.config.show_breadcrumb);
    }
}

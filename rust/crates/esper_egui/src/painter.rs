//! ESPER painter - low-level drawing primitives.
//!
//! Wraps egui's Painter with ESPER-specific drawing methods.

use crate::{camera::CameraState, style::RenderStyle};
use egui::{epaint::PathStroke, Color32, FontId, Painter, Pos2, Rect, Rounding, Stroke, Vec2};

/// ESPER-specific painter wrapping egui's Painter.
pub struct EsperPainter<'a> {
    /// Underlying egui painter.
    painter: &'a Painter,
    /// Camera state for coordinate transformation.
    camera: &'a CameraState,
    /// Visual style.
    style: &'a RenderStyle,
    /// Clip rectangle in screen coordinates.
    clip_rect: Rect,
}

impl<'a> EsperPainter<'a> {
    /// Create a new ESPER painter.
    pub fn new(painter: &'a Painter, camera: &'a CameraState, style: &'a RenderStyle) -> Self {
        let clip_rect = painter.clip_rect();
        Self {
            painter,
            camera,
            style,
            clip_rect,
        }
    }

    /// Get the underlying egui painter.
    pub fn egui_painter(&self) -> &Painter {
        self.painter
    }

    /// Get the camera state.
    pub fn camera(&self) -> &CameraState {
        self.camera
    }

    /// Get the render style.
    pub fn style(&self) -> &RenderStyle {
        self.style
    }

    // =========================================================================
    // COORDINATE TRANSFORMATION
    // =========================================================================

    /// Convert world coordinates to screen coordinates.
    pub fn world_to_screen(&self, world: Vec2) -> Pos2 {
        self.camera.world_to_screen(world)
    }

    /// Convert a world-space rectangle to screen coordinates.
    pub fn world_rect_to_screen(&self, world_rect: Rect) -> Rect {
        let min = self.world_to_screen(world_rect.min.to_vec2());
        let max = self.world_to_screen(world_rect.max.to_vec2());
        Rect::from_min_max(min, max)
    }

    /// Scale a world-space size to screen pixels.
    pub fn scale_to_screen(&self, world_size: f32) -> f32 {
        self.camera.scale_to_screen(world_size)
    }

    // =========================================================================
    // BASIC SHAPES (WORLD COORDINATES)
    // =========================================================================

    /// Draw a circle at world coordinates.
    pub fn circle_world(
        &self,
        center: Vec2,
        radius: f32,
        fill: Color32,
        stroke: impl Into<Stroke>,
    ) {
        let screen_center = self.world_to_screen(center);
        let screen_radius = self.scale_to_screen(radius);

        // Cull if outside viewport
        let bounds = Rect::from_center_size(screen_center, Vec2::splat(screen_radius * 2.0));
        if !bounds.intersects(self.clip_rect) {
            return;
        }

        self.painter
            .circle(screen_center, screen_radius, fill, stroke);
    }

    /// Draw a rectangle at world coordinates.
    pub fn rect_world(
        &self,
        rect: Rect,
        rounding: impl Into<Rounding>,
        fill: Color32,
        stroke: impl Into<Stroke>,
    ) {
        let screen_rect = self.world_rect_to_screen(rect);

        // Cull if outside viewport
        if !screen_rect.intersects(self.clip_rect) {
            return;
        }

        self.painter.rect(screen_rect, rounding, fill, stroke);
    }

    /// Draw a line between two world-space points.
    pub fn line_world(&self, from: Vec2, to: Vec2, stroke: Stroke) {
        let screen_from = self.world_to_screen(from);
        let screen_to = self.world_to_screen(to);

        // Simple line culling - check if line bounds intersect clip rect
        let bounds = Rect::from_two_pos(screen_from, screen_to);
        if !bounds.intersects(self.clip_rect) {
            return;
        }

        self.painter.line_segment(
            [screen_from, screen_to],
            PathStroke::new(stroke.width, stroke.color),
        );
    }

    /// Draw text at world coordinates.
    pub fn text_world(
        &self,
        pos: Vec2,
        anchor: egui::Align2,
        text: impl ToString,
        font: FontId,
        color: Color32,
    ) {
        let screen_pos = self.world_to_screen(pos);

        // Only draw if in viewport
        if !self.clip_rect.contains(screen_pos) {
            return;
        }

        self.painter.text(screen_pos, anchor, text, font, color);
    }

    // =========================================================================
    // ENTITY RENDERING
    // =========================================================================

    /// Draw an entity node at world coordinates.
    pub fn entity_node(
        &self,
        center: Vec2,
        radius: f32,
        selected: bool,
        hovered: bool,
        focused: bool,
    ) {
        let fill = if focused {
            self.style.entity.fill_focused
        } else if selected {
            self.style.entity.fill_selected
        } else if hovered {
            self.style.entity.fill_hovered
        } else {
            self.style.entity.fill
        };

        let stroke = Stroke::new(self.style.entity.stroke_width, self.style.entity.stroke);

        self.circle_world(center, radius, fill, stroke);

        // Draw focus ring if focused
        if focused {
            self.circle_world(
                center,
                radius + 4.0 / self.camera.zoom,
                Color32::TRANSPARENT,
                Stroke::new(
                    self.style.selection.focus_ring_width,
                    self.style.selection.focus_ring,
                ),
            );
        }
    }

    /// Draw an edge between two entities.
    pub fn entity_edge(&self, from: Vec2, to: Vec2, highlighted: bool, is_control_edge: bool) {
        let color = if is_control_edge {
            self.style.edge.color_control
        } else if highlighted {
            self.style.edge.color_highlight
        } else {
            self.style.edge.color
        };

        let width = if highlighted {
            self.style.edge.width * 2.0
        } else {
            self.style.edge.width
        };

        self.line_world(from, to, Stroke::new(width, color));
    }

    /// Draw an arrow from one point to another.
    pub fn arrow_world(&self, from: Vec2, to: Vec2, stroke: Stroke) {
        self.line_world(from, to, stroke);

        // Draw arrowhead
        let dir = (to - from).normalized();
        let perp = Vec2::new(-dir.y, dir.x);
        let arrow_size = self.style.edge.arrow_size / self.camera.zoom;

        let tip1 = to - dir * arrow_size + perp * arrow_size * 0.5;
        let tip2 = to - dir * arrow_size - perp * arrow_size * 0.5;

        self.line_world(to, tip1, stroke);
        self.line_world(to, tip2, stroke);
    }

    // =========================================================================
    // SELECTION & HIGHLIGHTS
    // =========================================================================

    /// Draw a selection box around a world-space rectangle.
    pub fn selection_box(&self, rect: Rect) {
        let screen_rect = self.world_rect_to_screen(rect);

        self.painter.rect(
            screen_rect,
            Rounding::ZERO,
            self.style.selection.fill,
            Stroke::new(
                self.style.selection.stroke_width,
                self.style.selection.stroke,
            ),
        );
    }

    /// Draw a dashed selection rectangle (for marquee selection).
    pub fn dashed_rect(&self, rect: Rect, _dash_length: f32) {
        let screen_rect = self.world_rect_to_screen(rect);
        let stroke = Stroke::new(1.0, self.style.selection.stroke);

        // Draw dashed edges (simplified - just draw the rect for now)
        // A proper implementation would draw individual dashed segments
        self.painter
            .rect_stroke(screen_rect, Rounding::ZERO, stroke);
    }

    // =========================================================================
    // DEBUG RENDERING
    // =========================================================================

    /// Draw a grid for debugging.
    pub fn debug_grid(&self, cell_size: f32) {
        let visible = self.camera.visible_world_rect();

        let start_x = (visible.min.x / cell_size).floor() * cell_size;
        let start_y = (visible.min.y / cell_size).floor() * cell_size;

        let stroke = Stroke::new(self.style.grid.line_width, self.style.grid.line);

        // Vertical lines
        let mut x = start_x;
        while x <= visible.max.x {
            self.line_world(
                Vec2::new(x, visible.min.y),
                Vec2::new(x, visible.max.y),
                stroke,
            );
            x += cell_size;
        }

        // Horizontal lines
        let mut y = start_y;
        while y <= visible.max.y {
            self.line_world(
                Vec2::new(visible.min.x, y),
                Vec2::new(visible.max.x, y),
                stroke,
            );
            y += cell_size;
        }
    }

    /// Draw a bounds rectangle for debugging.
    pub fn debug_bounds(&self, rect: Rect) {
        let stroke = Stroke::new(1.0, self.style.grid.bounds);
        self.rect_world(rect, Rounding::ZERO, Color32::TRANSPARENT, stroke);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Most painter tests require a real egui context,
    // so we test the coordinate math instead

    #[test]
    fn scale_calculation() {
        let camera = CameraState::new(Vec2::ZERO, 2.0);
        assert_eq!(camera.scale_to_screen(10.0), 20.0);
        assert_eq!(camera.scale_to_screen(5.0), 10.0);
    }
}

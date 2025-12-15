//! Camera2D - pan/zoom with smooth interpolation
//!
//! Provides world-to-screen and screen-to-world coordinate transforms.

#![allow(dead_code)]

use egui::{Pos2, Rect, Vec2};

/// 2D camera with pan and zoom
#[derive(Debug, Clone)]
pub struct Camera2D {
    /// Center of the view in world coordinates
    pub center: Pos2,
    /// Zoom level (1.0 = 100%)
    pub zoom: f32,
    /// Target center for smooth interpolation
    pub target_center: Pos2,
    /// Target zoom for smooth interpolation
    pub target_zoom: f32,
    /// Interpolation speed (0.0-1.0, higher = faster)
    pub lerp_speed: f32,
    /// Zoom limits
    pub min_zoom: f32,
    pub max_zoom: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            center: Pos2::ZERO,
            zoom: 1.0,
            target_center: Pos2::ZERO,
            target_zoom: 1.0,
            lerp_speed: 0.15,
            min_zoom: 0.1,
            max_zoom: 5.0,
        }
    }
}

impl Camera2D {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update camera with smooth interpolation (call every frame)
    pub fn update(&mut self, dt: f32) {
        let t = (self.lerp_speed * dt * 60.0).min(1.0);
        self.center = Pos2::new(
            lerp(self.center.x, self.target_center.x, t),
            lerp(self.center.y, self.target_center.y, t),
        );
        self.zoom = lerp(self.zoom, self.target_zoom, t);
    }

    /// Snap to target immediately (no interpolation)
    pub fn snap_to_target(&mut self) {
        self.center = self.target_center;
        self.zoom = self.target_zoom;
    }

    /// Pan by delta in screen coordinates
    pub fn pan(&mut self, screen_delta: Vec2) {
        let world_delta = screen_delta / self.zoom;
        self.target_center -= world_delta;
    }

    /// Pan to center on a world position
    pub fn pan_to(&mut self, world_pos: Pos2) {
        self.target_center = world_pos;
    }

    /// Zoom by factor, keeping screen_pos fixed
    pub fn zoom_at(&mut self, factor: f32, screen_pos: Pos2, screen_rect: Rect) {
        let old_zoom = self.target_zoom;
        self.target_zoom = (self.target_zoom * factor).clamp(self.min_zoom, self.max_zoom);

        if (self.target_zoom - old_zoom).abs() > 0.001 {
            let screen_center = screen_rect.center();
            let offset_from_center = screen_pos - screen_center;
            let world_offset_old = offset_from_center / old_zoom;
            let world_offset_new = offset_from_center / self.target_zoom;
            self.target_center += world_offset_old - world_offset_new;
        }
    }

    /// Set zoom level directly
    pub fn set_zoom(&mut self, zoom: f32) {
        self.target_zoom = zoom.clamp(self.min_zoom, self.max_zoom);
    }

    /// Fit the camera to show a world-space bounding box with padding
    pub fn fit_to_bounds(&mut self, bounds: Rect, screen_rect: Rect, padding: f32) {
        if bounds.is_negative() || bounds.width() < 1.0 || bounds.height() < 1.0 {
            return;
        }
        self.target_center = bounds.center();

        let padded_screen = Rect::from_min_max(
            screen_rect.min + Vec2::splat(padding),
            screen_rect.max - Vec2::splat(padding),
        );
        let zoom_x = padded_screen.width() / bounds.width();
        let zoom_y = padded_screen.height() / bounds.height();
        let new_zoom = zoom_x.min(zoom_y).clamp(self.min_zoom, self.max_zoom);

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&format!(
            "fit_to_bounds: bounds={:?}, screen={:?}, padded_screen={}x{}, zoom_x={:.3}, zoom_y={:.3}, final_zoom={:.3}",
            bounds, screen_rect, padded_screen.width(), padded_screen.height(), zoom_x, zoom_y, new_zoom
        ).into());

        self.target_zoom = new_zoom;
    }

    /// Transform world position to screen position
    pub fn world_to_screen(&self, world_pos: Pos2, screen_rect: Rect) -> Pos2 {
        let screen_center = screen_rect.center();
        let offset = (world_pos - self.center) * self.zoom;
        Pos2::new(screen_center.x + offset.x, screen_center.y + offset.y)
    }

    /// Transform screen position to world position
    pub fn screen_to_world(&self, screen_pos: Pos2, screen_rect: Rect) -> Pos2 {
        let screen_center = screen_rect.center();
        let offset = (screen_pos - screen_center) / self.zoom;
        Pos2::new(self.center.x + offset.x, self.center.y + offset.y)
    }

    /// Get the visible world bounds for the current screen rect
    pub fn visible_bounds(&self, screen_rect: Rect) -> Rect {
        let half_size = Vec2::new(
            screen_rect.width() / self.zoom / 2.0,
            screen_rect.height() / self.zoom / 2.0,
        );
        Rect::from_center_size(self.center, half_size * 2.0)
    }

    /// Check if a world-space rect is visible
    pub fn is_visible(&self, world_rect: Rect, screen_rect: Rect) -> bool {
        self.visible_bounds(screen_rect).intersects(world_rect)
    }

    /// Reset camera to default position
    pub fn reset(&mut self) {
        self.target_center = Pos2::ZERO;
        self.target_zoom = 1.0;
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

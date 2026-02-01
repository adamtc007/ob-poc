//! Camera2D - pan/zoom with spring-based smooth interpolation
//!
//! Provides world-to-screen and screen-to-world coordinate transforms.
//!
//! # EGUI-RULES Compliance
//! - Camera state is UI-only (not server data)
//! - No callbacks - state is polled each frame
//! - Call `update(dt)` at start of frame, then use transforms for rendering
//!
//! # Animation Methods
//! - `fly_to(pos)` - Animate camera to center on world position
//! - `fly_to_node(node_id, graph)` - Fly to a specific node
//! - `zoom_to(level)` - Animate to a specific zoom level
//! - `zoom_to_fit(bounds, screen_rect)` - Zoom to fit bounds in view
//! - `fly_to_bounds(bounds, screen_rect)` - Combined fly + zoom to show bounds

use super::animation::{SpringConfig, SpringF32, SpringVec2};
use egui::{Pos2, Rect, Vec2};

/// 2D camera with pan and zoom using spring physics
#[derive(Debug, Clone)]
pub struct Camera2D {
    /// Center of the view in world coordinates (animated)
    position: SpringVec2,
    /// Zoom level (animated) - 1.0 = 100%
    zoom: SpringF32,
    /// Zoom limits
    pub min_zoom: f32,
    pub max_zoom: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            position: SpringVec2::with_config(0.0, 0.0, SpringConfig::from_preset("medium")),
            zoom: SpringF32::with_config(1.0, SpringConfig::from_preset("medium")),
            min_zoom: 0.1,
            max_zoom: 5.0,
        }
    }
}

impl Camera2D {
    pub fn new() -> Self {
        Self::default()
    }

    // =========================================================================
    // CURRENT VALUES (for reading)
    // =========================================================================

    /// Current camera center position
    pub fn center(&self) -> Pos2 {
        self.position.get_pos2()
    }

    /// Current zoom level
    pub fn zoom(&self) -> f32 {
        self.zoom.get()
    }

    /// Target camera center position
    pub fn target_center(&self) -> Pos2 {
        let (x, y) = self.position.target();
        Pos2::new(x, y)
    }

    /// Target zoom level
    pub fn target_zoom(&self) -> f32 {
        self.zoom.target()
    }

    // =========================================================================
    // ANIMATION UPDATE
    // =========================================================================

    /// Update camera with spring interpolation (call every frame)
    pub fn update(&mut self, dt: f32) {
        self.position.tick(dt);
        self.zoom.tick(dt);
    }

    /// Snap to target immediately (no interpolation)
    pub fn snap_to_target(&mut self) {
        let (tx, ty) = self.position.target();
        self.position.set_immediate(tx, ty);
        self.zoom.set_immediate(self.zoom.target());
    }

    /// Check if camera is still animating
    pub fn is_animating(&self) -> bool {
        self.position.is_animating() || self.zoom.is_animating()
    }

    // =========================================================================
    // CAMERA CONTROLS
    // =========================================================================

    /// Pan by delta in screen coordinates
    pub fn pan(&mut self, screen_delta: Vec2) {
        let world_delta = screen_delta / self.zoom.get();
        let (tx, ty) = self.position.target();
        self.position
            .set_target(tx - world_delta.x, ty - world_delta.y);
    }

    /// Pan to center on a world position (animated)
    pub fn pan_to(&mut self, world_pos: Pos2) {
        self.position.set_target(world_pos.x, world_pos.y);
    }

    // =========================================================================
    // FLY-TO / ZOOM-TO METHODS (high-level animation API)
    // =========================================================================

    /// Fly to center on a world position with default spring
    /// This is the primary method for animated camera movement
    pub fn fly_to(&mut self, world_pos: Pos2) {
        self.position
            .set_config(SpringConfig::from_preset("medium"));
        self.position.set_target(world_pos.x, world_pos.y);
    }

    /// Fly to center on a world position with custom spring config
    pub fn fly_to_with_config(&mut self, world_pos: Pos2, config: SpringConfig) {
        self.position.set_config(config);
        self.position.set_target(world_pos.x, world_pos.y);
    }

    /// Fly to center on a world position with slow/cinematic spring
    pub fn fly_to_slow(&mut self, world_pos: Pos2) {
        self.fly_to_with_config(world_pos, SpringConfig::from_preset("slow"));
    }

    /// Fly to center on a world position with fast/snappy spring
    pub fn fly_to_fast(&mut self, world_pos: Pos2) {
        self.fly_to_with_config(world_pos, SpringConfig::from_preset("fast"));
    }

    /// Animate to a specific zoom level
    pub fn zoom_to(&mut self, zoom_level: f32) {
        self.zoom.set_config(SpringConfig::from_preset("medium"));
        self.zoom
            .set_target(zoom_level.clamp(self.min_zoom, self.max_zoom));
    }

    /// Animate to a specific zoom level with custom spring
    pub fn zoom_to_with_config(&mut self, zoom_level: f32, config: SpringConfig) {
        self.zoom.set_config(config);
        self.zoom
            .set_target(zoom_level.clamp(self.min_zoom, self.max_zoom));
    }

    /// Zoom to fit specific bounds in view (animated)
    pub fn zoom_to_fit(&mut self, bounds: Rect, screen_rect: Rect, padding: f32) {
        if bounds.is_negative() || bounds.width() < 1.0 || bounds.height() < 1.0 {
            return;
        }

        let padded_screen = Rect::from_min_max(
            screen_rect.min + Vec2::splat(padding),
            screen_rect.max - Vec2::splat(padding),
        );
        let zoom_x = padded_screen.width() / bounds.width();
        let zoom_y = padded_screen.height() / bounds.height();
        let new_zoom = zoom_x.min(zoom_y).clamp(self.min_zoom, self.max_zoom);

        self.zoom_to(new_zoom);
    }

    /// Combined fly + zoom to show bounds (animated)
    /// This is the high-level "show me this area" method
    pub fn fly_to_bounds(&mut self, bounds: Rect, screen_rect: Rect, padding: f32) {
        if bounds.is_negative() || bounds.width() < 1.0 || bounds.height() < 1.0 {
            return;
        }

        // Fly to center
        self.fly_to(bounds.center());

        // Zoom to fit
        self.zoom_to_fit(bounds, screen_rect, padding);
    }

    /// Combined fly + zoom with slow/cinematic spring (for drill-down navigation)
    pub fn fly_to_bounds_slow(&mut self, bounds: Rect, screen_rect: Rect, padding: f32) {
        if bounds.is_negative() || bounds.width() < 1.0 || bounds.height() < 1.0 {
            return;
        }

        self.fly_to_slow(bounds.center());
        self.zoom.set_config(SpringConfig::from_preset("slow"));
        self.zoom_to_fit(bounds, screen_rect, padding);
    }

    /// Zoom by factor, keeping screen_pos fixed in view
    pub fn zoom_at(&mut self, factor: f32, screen_pos: Pos2, screen_rect: Rect) {
        let old_zoom = self.zoom.target();
        let new_zoom = (old_zoom * factor).clamp(self.min_zoom, self.max_zoom);

        if (new_zoom - old_zoom).abs() > 0.001 {
            // Calculate world position under cursor before zoom
            let screen_center = screen_rect.center();
            let offset_from_center = screen_pos - screen_center;
            let world_offset_old = offset_from_center / old_zoom;
            let world_offset_new = offset_from_center / new_zoom;

            // Adjust target center to keep cursor position stable
            let (tx, ty) = self.position.target();
            let new_center_x = tx + (world_offset_old.x - world_offset_new.x);
            let new_center_y = ty + (world_offset_old.y - world_offset_new.y);

            self.position.set_target(new_center_x, new_center_y);
            self.zoom.set_target(new_zoom);
        }
    }

    /// Set zoom level directly (animated)
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom
            .set_target(zoom.clamp(self.min_zoom, self.max_zoom));
    }

    /// Set target zoom directly (for input handling)
    pub fn set_target_zoom(&mut self, zoom: f32) {
        self.zoom
            .set_target(zoom.clamp(self.min_zoom, self.max_zoom));
    }

    /// Set target center directly (for input handling)
    pub fn set_target_center(&mut self, pos: Pos2) {
        self.position.set_target(pos.x, pos.y);
    }

    /// Offset target center by delta in world coordinates
    pub fn offset_target_center(&mut self, delta: Vec2) {
        let (tx, ty) = self.position.target();
        self.position.set_target(tx + delta.x, ty + delta.y);
    }

    /// Fit the camera to show a world-space bounding box with padding
    pub fn fit_to_bounds(&mut self, bounds: Rect, screen_rect: Rect, padding: f32) {
        if bounds.is_negative() || bounds.width() < 1.0 || bounds.height() < 1.0 {
            return;
        }

        // Center on bounds
        self.position
            .set_target(bounds.center().x, bounds.center().y);

        // Calculate zoom to fit bounds in screen
        let padded_screen = Rect::from_min_max(
            screen_rect.min + Vec2::splat(padding),
            screen_rect.max - Vec2::splat(padding),
        );
        let zoom_x = padded_screen.width() / bounds.width();
        let zoom_y = padded_screen.height() / bounds.height();
        let new_zoom = zoom_x.min(zoom_y).clamp(self.min_zoom, self.max_zoom);

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "fit_to_bounds: bounds={:?}, screen={:?}, padded_screen={}x{}, zoom_x={:.3}, zoom_y={:.3}, final_zoom={:.3}",
                bounds,
                screen_rect,
                padded_screen.width(),
                padded_screen.height(),
                zoom_x,
                zoom_y,
                new_zoom
            )
            .into(),
        );

        self.zoom.set_target(new_zoom);
    }

    /// Reset camera to default position
    pub fn reset(&mut self) {
        self.position.set_target(0.0, 0.0);
        self.zoom.set_target(1.0);
    }

    // =========================================================================
    // SPRING CONFIGURATION
    // =========================================================================

    /// Set spring configuration for camera movement
    pub fn set_spring_config(&mut self, config: SpringConfig) {
        self.position.set_config(config);
        self.zoom.set_config(config);
    }

    /// Use fast spring (snappy UI response)
    pub fn use_fast_spring(&mut self) {
        self.set_spring_config(SpringConfig::from_preset("fast"));
    }

    /// Use slow spring (cinematic transitions)
    pub fn use_slow_spring(&mut self) {
        self.set_spring_config(SpringConfig::from_preset("slow"));
    }

    // =========================================================================
    // COORDINATE TRANSFORMS
    // =========================================================================

    /// Transform world position to screen position
    pub fn world_to_screen(&self, world_pos: Pos2, screen_rect: Rect) -> Pos2 {
        let screen_center = screen_rect.center();
        let center = self.center();
        let zoom = self.zoom();
        let offset = (world_pos - center) * zoom;
        Pos2::new(screen_center.x + offset.x, screen_center.y + offset.y)
    }

    /// Transform screen position to world position
    pub fn screen_to_world(&self, screen_pos: Pos2, screen_rect: Rect) -> Pos2 {
        let screen_center = screen_rect.center();
        let center = self.center();
        let zoom = self.zoom();
        let offset = (screen_pos - screen_center) / zoom;
        Pos2::new(center.x + offset.x, center.y + offset.y)
    }

    /// Get the visible world bounds for the current screen rect
    pub fn visible_bounds(&self, screen_rect: Rect) -> Rect {
        let zoom = self.zoom();
        let half_size = Vec2::new(
            screen_rect.width() / zoom / 2.0,
            screen_rect.height() / zoom / 2.0,
        );
        Rect::from_center_size(self.center(), half_size * 2.0)
    }

    /// Check if a world-space rect is visible
    pub fn is_visible(&self, world_rect: Rect, screen_rect: Rect) -> bool {
        self.visible_bounds(screen_rect).intersects(world_rect)
    }
}

//! Camera state and animation.
//!
//! The camera transforms world coordinates to screen coordinates.
//! It supports smooth animation via spring physics.

use egui::{Pos2, Rect, Vec2};

/// Camera state for viewport rendering.
#[derive(Debug, Clone)]
pub struct CameraState {
    /// Current camera center in world coordinates.
    pub center: Vec2,
    /// Current zoom level (1.0 = 100%).
    pub zoom: f32,
    /// Target center (for animation).
    pub target_center: Vec2,
    /// Target zoom (for animation).
    pub target_zoom: f32,
    /// Viewport size in screen pixels.
    pub viewport_size: Vec2,
    /// Animation speed (0-1, where 1 = instant).
    pub lerp_speed: f32,
    /// Whether the camera is currently animating.
    pub is_animating: bool,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            target_center: Vec2::ZERO,
            target_zoom: 1.0,
            viewport_size: Vec2::new(800.0, 600.0),
            lerp_speed: 0.15,
            is_animating: false,
        }
    }
}

impl CameraState {
    /// Create a new camera centered at the given position.
    pub fn new(center: Vec2, zoom: f32) -> Self {
        Self {
            center,
            zoom,
            target_center: center,
            target_zoom: zoom,
            ..Default::default()
        }
    }

    /// Set the viewport size (call when window resizes).
    pub fn set_viewport_size(&mut self, size: Vec2) {
        self.viewport_size = size;
    }

    /// Animate camera to a new center position.
    pub fn animate_to(&mut self, center: Vec2) {
        self.target_center = center;
        self.is_animating = true;
    }

    /// Animate camera to a new center and zoom.
    pub fn animate_to_zoom(&mut self, center: Vec2, zoom: f32) {
        self.target_center = center;
        self.target_zoom = zoom;
        self.is_animating = true;
    }

    /// Instantly snap camera to position (no animation).
    pub fn snap_to(&mut self, center: Vec2, zoom: f32) {
        self.center = center;
        self.zoom = zoom;
        self.target_center = center;
        self.target_zoom = zoom;
        self.is_animating = false;
    }

    /// Pan camera by delta in screen coordinates.
    pub fn pan_screen(&mut self, delta: Vec2) {
        let world_delta = delta / self.zoom;
        self.target_center -= world_delta;
        self.is_animating = true;
    }

    /// Pan camera by delta in world coordinates.
    pub fn pan_world(&mut self, delta: Vec2) {
        self.target_center -= delta;
        self.is_animating = true;
    }

    /// Zoom by factor around a screen point.
    pub fn zoom_around(&mut self, factor: f32, screen_point: Pos2) {
        let world_before = self.screen_to_world(screen_point);

        self.target_zoom *= factor;

        // Adjust center to keep world_before at the same screen position
        let new_zoom = self.target_zoom;
        let screen_center = Pos2::new(self.viewport_size.x / 2.0, self.viewport_size.y / 2.0);
        let screen_offset = screen_point - screen_center;
        let world_offset_new = Vec2::new(screen_offset.x, screen_offset.y) / new_zoom;
        self.target_center = world_before - world_offset_new;

        self.is_animating = true;
    }

    /// Zoom to fit a world-space rectangle.
    pub fn zoom_to_fit(&mut self, world_rect: Rect, padding: f32) {
        let padded_size = Vec2::new(
            world_rect.width() + padding * 2.0,
            world_rect.height() + padding * 2.0,
        );

        let zoom_x = self.viewport_size.x / padded_size.x;
        let zoom_y = self.viewport_size.y / padded_size.y;
        self.target_zoom = zoom_x.min(zoom_y);

        self.target_center = world_rect.center().to_vec2();
        self.is_animating = true;
    }

    /// Update camera animation (call each frame).
    pub fn tick(&mut self, dt: f32) {
        if !self.is_animating {
            return;
        }

        // Lerp towards target
        let t = 1.0 - (1.0 - self.lerp_speed).powf(dt * 60.0);

        self.center = lerp_vec2(self.center, self.target_center, t);
        self.zoom = lerp_f32(self.zoom, self.target_zoom, t);

        // Check if animation is complete
        let center_dist = (self.center - self.target_center).length();
        let zoom_diff = (self.zoom - self.target_zoom).abs();

        if center_dist < 0.01 && zoom_diff < 0.001 {
            self.center = self.target_center;
            self.zoom = self.target_zoom;
            self.is_animating = false;
        }
    }

    /// Convert screen coordinates to world coordinates.
    pub fn screen_to_world(&self, screen: Pos2) -> Vec2 {
        let screen_center = Pos2::new(self.viewport_size.x / 2.0, self.viewport_size.y / 2.0);
        let screen_offset = screen - screen_center;
        let world_offset = Vec2::new(screen_offset.x, screen_offset.y) / self.zoom;
        self.center + world_offset
    }

    /// Convert world coordinates to screen coordinates.
    pub fn world_to_screen(&self, world: Vec2) -> Pos2 {
        let screen_center = Pos2::new(self.viewport_size.x / 2.0, self.viewport_size.y / 2.0);
        let world_offset = world - self.center;
        let screen_offset = world_offset * self.zoom;
        screen_center + Vec2::new(screen_offset.x, screen_offset.y)
    }

    /// Get the visible world-space rectangle.
    pub fn visible_world_rect(&self) -> Rect {
        let half_size = self.viewport_size / (2.0 * self.zoom);
        Rect::from_center_size(
            Pos2::new(self.center.x, self.center.y),
            egui::vec2(half_size.x * 2.0, half_size.y * 2.0),
        )
    }

    /// Check if a world-space point is visible in the viewport.
    pub fn is_visible(&self, world_point: Vec2) -> bool {
        self.visible_world_rect()
            .contains(Pos2::new(world_point.x, world_point.y))
    }

    /// Check if a world-space rectangle intersects the viewport.
    pub fn is_rect_visible(&self, world_rect: Rect) -> bool {
        self.visible_world_rect().intersects(world_rect)
    }

    /// Scale a world-space size to screen pixels.
    pub fn scale_to_screen(&self, world_size: f32) -> f32 {
        world_size * self.zoom
    }

    /// Scale a screen pixel size to world coordinates.
    pub fn scale_to_world(&self, screen_size: f32) -> f32 {
        screen_size / self.zoom
    }
}

/// Linear interpolation for f32.
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation for Vec2.
fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_default() {
        let cam = CameraState::default();
        assert_eq!(cam.center, Vec2::ZERO);
        assert_eq!(cam.zoom, 1.0);
        assert!(!cam.is_animating);
    }

    #[test]
    fn camera_screen_to_world() {
        let mut cam = CameraState::default();
        cam.viewport_size = Vec2::new(800.0, 600.0);
        cam.center = Vec2::new(100.0, 50.0);
        cam.zoom = 2.0;

        // Screen center should map to camera center
        let screen_center = Pos2::new(400.0, 300.0);
        let world = cam.screen_to_world(screen_center);
        assert!((world.x - 100.0).abs() < 0.01);
        assert!((world.y - 50.0).abs() < 0.01);
    }

    #[test]
    fn camera_world_to_screen() {
        let mut cam = CameraState::default();
        cam.viewport_size = Vec2::new(800.0, 600.0);
        cam.center = Vec2::new(100.0, 50.0);
        cam.zoom = 2.0;

        // Camera center should map to screen center
        let screen = cam.world_to_screen(Vec2::new(100.0, 50.0));
        assert!((screen.x - 400.0).abs() < 0.01);
        assert!((screen.y - 300.0).abs() < 0.01);
    }

    #[test]
    fn camera_round_trip() {
        let mut cam = CameraState::default();
        cam.viewport_size = Vec2::new(800.0, 600.0);
        cam.center = Vec2::new(42.0, 17.0);
        cam.zoom = 1.5;

        let original = Pos2::new(200.0, 150.0);
        let world = cam.screen_to_world(original);
        let back = cam.world_to_screen(world);

        assert!((original.x - back.x).abs() < 0.01);
        assert!((original.y - back.y).abs() < 0.01);
    }

    #[test]
    fn camera_animation() {
        let mut cam = CameraState::default();
        cam.animate_to(Vec2::new(100.0, 100.0));

        assert!(cam.is_animating);
        assert_eq!(cam.target_center, Vec2::new(100.0, 100.0));

        // Tick should move towards target
        for _ in 0..100 {
            cam.tick(1.0 / 60.0);
        }

        assert!((cam.center.x - 100.0).abs() < 0.1);
        assert!((cam.center.y - 100.0).abs() < 0.1);
    }

    #[test]
    fn camera_snap() {
        let mut cam = CameraState::default();
        cam.snap_to(Vec2::new(50.0, 50.0), 2.0);

        assert_eq!(cam.center, Vec2::new(50.0, 50.0));
        assert_eq!(cam.zoom, 2.0);
        assert!(!cam.is_animating);
    }

    #[test]
    fn camera_visible_rect() {
        let mut cam = CameraState::default();
        cam.viewport_size = Vec2::new(800.0, 600.0);
        cam.center = Vec2::ZERO;
        cam.zoom = 1.0;

        let rect = cam.visible_world_rect();
        assert_eq!(rect.width(), 800.0);
        assert_eq!(rect.height(), 600.0);

        // At zoom 2.0, visible area should be half
        cam.zoom = 2.0;
        let rect = cam.visible_world_rect();
        assert_eq!(rect.width(), 400.0);
        assert_eq!(rect.height(), 300.0);
    }
}

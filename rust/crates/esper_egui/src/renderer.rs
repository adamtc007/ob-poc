//! Main ESPER renderer.
//!
//! Coordinates all rendering components and handles the update/render cycle.

use crate::{
    camera::CameraState,
    config::RenderConfig,
    entity::EntityRenderer,
    error::{RenderError, RenderResult},
    input::InputBridge,
    overlay::OverlayRenderer,
    painter::EsperPainter,
    style::RenderStyle,
};
use egui::Ui;
use esper_core::{DroneState, EffectSet, Verb};
use esper_snapshot::WorldSnapshot;
use tracing::debug;

/// Main ESPER renderer.
///
/// Handles the full update/render cycle:
/// 1. Update phase: Process input, tick animations, execute verbs
/// 2. Render phase: Draw world, entities, overlays
pub struct EsperRenderer {
    /// Render configuration.
    config: RenderConfig,
    /// Visual style.
    style: RenderStyle,
    /// Camera state.
    camera: CameraState,
    /// Input bridge.
    input: InputBridge,
    /// Entity renderer.
    entity_renderer: EntityRenderer,
    /// Overlay renderer.
    overlay_renderer: OverlayRenderer,
    /// Frame timing for FPS calculation.
    frame_times: Vec<f32>,
    /// Current FPS estimate.
    current_fps: Option<f32>,
}

impl Default for EsperRenderer {
    fn default() -> Self {
        Self::new(RenderConfig::default(), RenderStyle::default())
    }
}

impl EsperRenderer {
    /// Create a new ESPER renderer.
    pub fn new(config: RenderConfig, style: RenderStyle) -> Self {
        Self {
            camera: CameraState {
                zoom: config.default_zoom,
                lerp_speed: config.camera_lerp_speed,
                ..Default::default()
            },
            entity_renderer: EntityRenderer::new(config.entity.clone()),
            overlay_renderer: OverlayRenderer::new(config.overlay.clone()),
            config,
            style,
            input: InputBridge::default(),
            frame_times: Vec::with_capacity(60),
            current_fps: None,
        }
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &RenderConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut RenderConfig {
        &mut self.config
    }

    /// Get a reference to the style.
    pub fn style(&self) -> &RenderStyle {
        &self.style
    }

    /// Set the visual style.
    pub fn set_style(&mut self, style: RenderStyle) {
        self.style = style;
    }

    /// Get a reference to the camera state.
    pub fn camera(&self) -> &CameraState {
        &self.camera
    }

    /// Get a mutable reference to the camera state.
    pub fn camera_mut(&mut self) -> &mut CameraState {
        &mut self.camera
    }

    /// Update phase: process input, tick animations, return verbs to execute.
    ///
    /// Call this BEFORE rendering. Returns verbs that should be executed on the drone.
    pub fn update(
        &mut self,
        ui: &Ui,
        dt: f32,
        _drone: &DroneState,
        _world: &WorldSnapshot,
    ) -> Vec<Verb> {
        // Update viewport size from UI
        let rect = ui.available_rect_before_wrap();
        self.camera
            .set_viewport_size(egui::vec2(rect.width(), rect.height()));

        // Process input and collect verbs
        let input_state = ui.input(|i| i.clone());
        let verbs = self.input.process(&input_state);

        // Tick camera animation
        if self.config.smooth_camera {
            self.camera.tick(dt);
        }

        // Update FPS counter
        self.update_fps(dt);

        verbs
    }

    /// Process effects from verb execution.
    ///
    /// Call this after executing verbs on the drone to update the renderer state.
    pub fn process_effects(
        &mut self,
        effects: EffectSet,
        drone: &DroneState,
        world: &WorldSnapshot,
    ) {
        // Handle camera-related effects
        if effects.contains(EffectSet::CAMERA_CHANGED) {
            if effects.contains(EffectSet::SNAP_TRANSITION) {
                // Snap camera immediately
                if let Some(target) = self.get_camera_target(drone, world) {
                    self.camera.snap_to(target, self.camera.target_zoom);
                }
            } else {
                // Animate camera to target
                if let Some(target) = self.get_camera_target(drone, world) {
                    self.camera.animate_to(target);
                }
            }
        }

        // Handle chamber change
        if effects.contains(EffectSet::CHAMBER_CHANGED) {
            debug!("Chamber changed to {}", drone.current_chamber);
        }
    }

    /// Render phase: draw everything.
    ///
    /// Call this after update(). Renders the world, entities, and overlays.
    pub fn render(
        &self,
        ui: &mut Ui,
        drone: &DroneState,
        world: &WorldSnapshot,
    ) -> RenderResult<()> {
        let rect = ui.available_rect_before_wrap();

        // Fill background
        ui.painter()
            .rect_filled(rect, egui::Rounding::ZERO, self.style.background);

        // Get current chamber
        let chamber_idx = drone.current_chamber as usize;
        let chamber = world
            .chambers
            .get(chamber_idx)
            .ok_or(RenderError::ChamberNotLoaded(drone.current_chamber))?;

        // Create ESPER painter
        let painter = EsperPainter::new(ui.painter(), &self.camera, &self.style);

        // Draw debug grid if enabled
        if self.config.debug.show_grid {
            let cell_size = 100.0 / self.camera.zoom;
            painter.debug_grid(cell_size);
        }

        // Draw chamber bounds if debug enabled
        if self.config.debug.show_bounds {
            let bounds = egui::Rect::from_min_max(
                egui::Pos2::new(chamber.bounds.min.x, chamber.bounds.min.y),
                egui::Pos2::new(chamber.bounds.max.x, chamber.bounds.max.y),
            );
            painter.debug_bounds(bounds);
        }

        // Draw edges first (behind entities)
        self.entity_renderer
            .render_edges(&painter, chamber, drone.taxonomy.selection);

        // Draw entities
        // Note: TaxonomyState has selection and preview_target, not preview/focus
        self.entity_renderer.render(
            &painter,
            chamber,
            drone.taxonomy.selection,
            drone.taxonomy.preview_target,
            drone.taxonomy.selection, // Use selection as focus for now
        );

        // Draw overlays
        self.overlay_renderer.render(
            ui,
            &self.camera,
            drone,
            world,
            &self.style,
            self.current_fps,
        );

        Ok(())
    }

    /// Get the camera target position based on drone state.
    fn get_camera_target(&self, drone: &DroneState, world: &WorldSnapshot) -> Option<egui::Vec2> {
        let chamber_idx = drone.current_chamber as usize;
        let chamber = world.chambers.get(chamber_idx)?;

        // If there's a selection, center on it
        if let Some(sel_idx) = drone.taxonomy.selection {
            let idx = sel_idx as usize;
            if idx < chamber.entity_ids.len() {
                let x = chamber.x[idx];
                let y = chamber.y[idx];
                return Some(egui::vec2(x, y));
            }
        }

        // Otherwise, center on chamber
        Some(egui::vec2(
            chamber.bounds.min.x + chamber.bounds.width() / 2.0,
            chamber.bounds.min.y + chamber.bounds.height() / 2.0,
        ))
    }

    /// Update FPS counter.
    fn update_fps(&mut self, dt: f32) {
        self.frame_times.push(dt);
        if self.frame_times.len() > 60 {
            self.frame_times.remove(0);
        }

        if self.frame_times.len() >= 10 {
            let avg_dt: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
            self.current_fps = Some(1.0 / avg_dt);
        }
    }

    /// Zoom to fit the entire chamber in view.
    pub fn zoom_to_fit(&mut self, world: &WorldSnapshot, chamber_idx: u32) {
        if let Some(chamber) = world.chambers.get(chamber_idx as usize) {
            let bounds = egui::Rect::from_min_max(
                egui::Pos2::new(chamber.bounds.min.x, chamber.bounds.min.y),
                egui::Pos2::new(chamber.bounds.max.x, chamber.bounds.max.y),
            );
            self.camera.zoom_to_fit(bounds, 50.0);
        }
    }

    /// Zoom to fit a specific entity.
    pub fn zoom_to_entity(&mut self, world: &WorldSnapshot, chamber_idx: u32, entity_idx: u32) {
        if let Some(chamber) = world.chambers.get(chamber_idx as usize) {
            let idx = entity_idx as usize;
            if idx < chamber.entity_ids.len() {
                let x = chamber.x[idx];
                let y = chamber.y[idx];

                // Create a rect around the entity with some padding
                let padding = 50.0;
                let bounds = egui::Rect::from_center_size(
                    egui::Pos2::new(x, y),
                    egui::vec2(padding * 2.0, padding * 2.0),
                );

                self.camera.zoom_to_fit(bounds, padding);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_default() {
        let renderer = EsperRenderer::default();
        assert_eq!(renderer.config.default_zoom, 1.0);
        assert!(renderer.config.smooth_camera);
    }

    #[test]
    fn renderer_style_change() {
        let mut renderer = EsperRenderer::default();
        let light = RenderStyle::light();
        renderer.set_style(light.clone());
        assert_eq!(renderer.style().background, light.background);
    }

    #[test]
    fn renderer_fps_calculation() {
        let mut renderer = EsperRenderer::default();

        // Simulate 60 frames at 60fps
        for _ in 0..60 {
            renderer.update_fps(1.0 / 60.0);
        }

        let fps = renderer.current_fps.unwrap();
        assert!((fps - 60.0).abs() < 1.0);
    }
}

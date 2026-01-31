//! Render configuration.

use serde::{Deserialize, Serialize};

/// Configuration for the ESPER renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    /// Minimum zoom level (fully zoomed out).
    pub min_zoom: f32,
    /// Maximum zoom level (fully zoomed in).
    pub max_zoom: f32,
    /// Default zoom level.
    pub default_zoom: f32,

    /// Camera animation speed (0-1, where 1 = instant).
    pub camera_lerp_speed: f32,
    /// Whether to enable camera smoothing.
    pub smooth_camera: bool,

    /// Entity rendering options.
    pub entity: EntityRenderConfig,

    /// Overlay rendering options.
    pub overlay: OverlayRenderConfig,

    /// Debug rendering options.
    pub debug: DebugRenderConfig,
}

/// Entity rendering configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRenderConfig {
    /// Minimum entity size in screen pixels.
    pub min_size: f32,
    /// Maximum entity size in screen pixels.
    pub max_size: f32,
    /// Size at which labels become visible.
    pub label_threshold: f32,
    /// Size at which details become visible.
    pub detail_threshold: f32,
    /// Whether to show entity icons.
    pub show_icons: bool,
    /// Whether to show entity labels.
    pub show_labels: bool,
}

/// Overlay rendering configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayRenderConfig {
    /// Show minimap.
    pub show_minimap: bool,
    /// Minimap size (fraction of viewport).
    pub minimap_size: f32,
    /// Show breadcrumb navigation.
    pub show_breadcrumb: bool,
    /// Show selection info panel.
    pub show_selection_info: bool,
    /// Show debug overlay.
    pub show_debug: bool,
}

/// Debug rendering configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebugRenderConfig {
    /// Show grid lines.
    pub show_grid: bool,
    /// Show entity bounding boxes.
    pub show_bounds: bool,
    /// Show camera frustum.
    pub show_frustum: bool,
    /// Show FPS counter.
    pub show_fps: bool,
    /// Show navigation state.
    pub show_nav_state: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            min_zoom: 0.1,
            max_zoom: 10.0,
            default_zoom: 1.0,
            camera_lerp_speed: 0.15,
            smooth_camera: true,
            entity: EntityRenderConfig::default(),
            overlay: OverlayRenderConfig::default(),
            debug: DebugRenderConfig::default(),
        }
    }
}

impl Default for EntityRenderConfig {
    fn default() -> Self {
        Self {
            min_size: 4.0,
            max_size: 100.0,
            label_threshold: 20.0,
            detail_threshold: 40.0,
            show_icons: true,
            show_labels: true,
        }
    }
}

impl Default for OverlayRenderConfig {
    fn default() -> Self {
        Self {
            show_minimap: true,
            minimap_size: 0.15,
            show_breadcrumb: true,
            show_selection_info: true,
            show_debug: false,
        }
    }
}

impl RenderConfig {
    /// Create a minimal config for testing.
    pub fn minimal() -> Self {
        Self {
            overlay: OverlayRenderConfig {
                show_minimap: false,
                show_breadcrumb: false,
                show_selection_info: false,
                show_debug: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Create a debug config with all overlays enabled.
    pub fn debug() -> Self {
        Self {
            overlay: OverlayRenderConfig {
                show_minimap: true,
                show_breadcrumb: true,
                show_selection_info: true,
                show_debug: true,
                ..Default::default()
            },
            debug: DebugRenderConfig {
                show_grid: true,
                show_bounds: true,
                show_frustum: true,
                show_fps: true,
                show_nav_state: true,
            },
            ..Default::default()
        }
    }

    /// Clamp zoom to configured bounds.
    pub fn clamp_zoom(&self, zoom: f32) -> f32 {
        zoom.clamp(self.min_zoom, self.max_zoom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = RenderConfig::default();
        assert!(config.smooth_camera);
        assert!(config.overlay.show_minimap);
        assert!(!config.debug.show_grid);
    }

    #[test]
    fn config_minimal() {
        let config = RenderConfig::minimal();
        assert!(!config.overlay.show_minimap);
        assert!(!config.overlay.show_debug);
    }

    #[test]
    fn config_debug() {
        let config = RenderConfig::debug();
        assert!(config.debug.show_grid);
        assert!(config.debug.show_fps);
    }

    #[test]
    fn clamp_zoom() {
        let config = RenderConfig::default();
        assert_eq!(config.clamp_zoom(0.05), 0.1);
        assert_eq!(config.clamp_zoom(20.0), 10.0);
        assert_eq!(config.clamp_zoom(5.0), 5.0);
    }
}

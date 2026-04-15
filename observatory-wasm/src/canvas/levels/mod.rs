//! Level-specific renderers.
//!
//! Each level has a different layout strategy and visual form.
//! Dispatched by LayoutStrategy from the GraphSceneModel.

use egui::Painter;

use ob_poc_types::graph_scene::{GraphSceneModel, LayoutStrategy};

use crate::state::CanvasApp;

mod cluster;
mod core;
mod planet;
mod system;
mod universe;

/// Dispatch painting to the level-specific renderer.
pub fn paint(
    painter: &Painter,
    transform: &egui::emath::RectTransform,
    scene: &GraphSceneModel,
    app: &CanvasApp,
) {
    let Some(cache) = app.render_cache.as_ref() else {
        return;
    };

    match scene.layout_strategy {
        LayoutStrategy::ForceDirected => universe::paint(painter, transform, scene, cache, app),
        LayoutStrategy::ForceWithinBoundary => {
            cluster::paint(painter, transform, scene, cache, app)
        }
        LayoutStrategy::DeterministicOrbital => {
            system::paint(painter, transform, scene, cache, app)
        }
        LayoutStrategy::HierarchicalGraph => {
            planet::paint(painter, transform, scene, cache, app)
        }
        LayoutStrategy::TreeDag => core::paint(painter, transform, scene, cache, app),
        LayoutStrategy::StructuredPanels => {
            // Surface is structured panels, not canvas — show indicator
            let center = transform.transform_pos(egui::Pos2::ZERO);
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "Surface — see viewport panels",
                egui::FontId::proportional(14.0),
                egui::Color32::from_rgb(148, 163, 184),
            );
        }
    }
}

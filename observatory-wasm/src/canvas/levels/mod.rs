//! Level-specific renderers.
//!
//! Each level has a different layout strategy and visual form.
//! Dispatched by ViewLevel from the GraphSceneModel.

use egui::Painter;

use ob_poc_types::galaxy::ViewLevel;
use ob_poc_types::graph_scene::GraphSceneModel;

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
    match scene.level {
        ViewLevel::Universe => universe::paint(painter, transform, scene, app),
        ViewLevel::Cluster => cluster::paint(painter, transform, scene, app),
        ViewLevel::System => system::paint(painter, transform, scene, app),
        ViewLevel::Planet => planet::paint(painter, transform, scene, app),
        ViewLevel::Surface => {
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
        ViewLevel::Core => core::paint(painter, transform, scene, app),
    }
}

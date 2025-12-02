//! CBU Entity Graph Visualization Module
//!
//! This module provides a template-based graph visualization for CBU entities.
//!
//! # Architecture
//!
//! ```text
//! CbuGraphData (from server)
//!        │
//!        ▼
//! LayoutEngine (template-based positioning)
//!        │
//!        ▼
//! LayoutGraph (positioned nodes/edges)
//!        │
//!        ├──► GraphRenderer (draws to egui::Painter)
//!        │         │
//!        │         ├──► LOD (level of detail)
//!        │         └──► Edges (bezier curves, arrows)
//!        │
//!        └──► InputHandler (mouse/keyboard interaction)
//!                    │
//!                    ▼
//!              Camera2D (pan/zoom transform)
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let mut graph_widget = CbuGraphWidget::new();
//! graph_widget.set_data(cbu_graph_data);
//! graph_widget.ui(ui);
//! ```

pub mod camera;
pub mod colors;
pub mod edges;
pub mod focus_card;
pub mod input;
pub mod layout;
pub mod lod;
pub mod render;
pub mod types;

pub use camera::Camera2D;
pub use input::{InputHandler, InputState};
pub use layout::LayoutEngine;
pub use render::GraphRenderer;
pub use types::*;

use egui::{Color32, Rect, Sense, Vec2};

// =============================================================================
// GRAPH WIDGET
// =============================================================================

/// Main widget for CBU entity graph visualization
pub struct CbuGraphWidget {
    /// Camera for pan/zoom
    camera: Camera2D,
    /// Input state (hover, selection)
    input_state: InputState,
    /// Computed layout graph
    layout_graph: Option<LayoutGraph>,
    /// Renderer
    renderer: GraphRenderer,
    /// Current view mode
    view_mode: ViewMode,
    /// Whether to auto-fit on first render
    needs_initial_fit: bool,
}

impl Default for CbuGraphWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl CbuGraphWidget {
    pub fn new() -> Self {
        Self {
            camera: Camera2D::new(),
            input_state: InputState::new(),
            layout_graph: None,
            renderer: GraphRenderer::new(),
            view_mode: ViewMode::KycUbo,
            needs_initial_fit: true,
        }
    }

    /// Set graph data from server response
    pub fn set_data(&mut self, data: CbuGraphData) {
        // Determine category and create layout engine
        let category = data
            .cbu_category
            .as_ref()
            .map(|s| CbuCategory::from_str(s))
            .unwrap_or_default();

        let engine = LayoutEngine::new(category);
        self.layout_graph = Some(engine.compute_layout(&data));
        self.needs_initial_fit = true;
        self.input_state.clear_focus();
    }

    /// Clear the graph
    pub fn clear(&mut self) {
        self.layout_graph = None;
        self.input_state.clear_focus();
    }

    /// Set view mode (KYC/UBO or Onboarding)
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
        // TODO: Re-layout or filter nodes based on view mode
    }

    /// Get current view mode
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Get focused node ID
    pub fn focused_node(&self) -> Option<&str> {
        self.input_state.focused_node.as_deref()
    }

    /// Focus on a specific node by ID
    pub fn focus_node(&mut self, node_id: &str) {
        self.input_state.set_focus(node_id);

        // Pan camera to focused node
        if let Some(ref graph) = self.layout_graph {
            if let Some(node) = graph.get_node(node_id) {
                self.camera.pan_to(node.position);
            }
        }
    }

    /// Fit camera to show all nodes
    pub fn fit_to_content(&mut self, screen_rect: Rect) {
        if let Some(ref graph) = self.layout_graph {
            self.camera.fit_to_bounds(graph.bounds, screen_rect, 50.0);
        }
    }

    /// Reset camera to default position
    pub fn reset_camera(&mut self) {
        self.camera.reset();
    }

    /// Main UI function
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(ref graph) = self.layout_graph else {
            self.render_empty_state(ui);
            return;
        };

        // Allocate space and get painter
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());

        let screen_rect = response.rect;

        // Initial fit on first render
        if self.needs_initial_fit {
            self.camera.fit_to_bounds(graph.bounds, screen_rect, 50.0);
            self.camera.snap_to_target();
            self.needs_initial_fit = false;
        }

        // Update camera interpolation
        let dt = ui.input(|i| i.stable_dt);
        self.camera.update(dt);

        // Handle input - need to clone graph for borrow checker
        let graph_clone = graph.clone();
        let needs_repaint = InputHandler::handle_input(
            &response,
            &mut self.camera,
            &mut self.input_state,
            &graph_clone,
            screen_rect,
        );

        // Set cursor
        ui.ctx()
            .set_cursor_icon(input::cursor_for_state(&self.input_state));

        // Render graph
        self.renderer.render(
            &painter,
            graph,
            &self.camera,
            screen_rect,
            self.input_state.focused_node.as_deref(),
        );

        // Render UI chrome (stats, controls)
        self.render_chrome(&painter, graph, screen_rect);

        // Render focus card if a node is focused
        let mut should_clear_focus = false;
        let mut navigate_to: Option<String> = None;

        if let Some(ref focus_id) = self.input_state.focused_node {
            if let Some(node) = graph.get_node(focus_id) {
                let card_data = focus_card::build_focus_card_data(node, graph);
                focus_card::render_focus_card(
                    ui.ctx(),
                    &card_data,
                    &mut || should_clear_focus = true,
                    &mut |entity_id| navigate_to = Some(entity_id.to_string()),
                );
            }
        }

        // Handle focus card actions after rendering
        if should_clear_focus {
            self.input_state.clear_focus();
        }
        if let Some(entity_id) = navigate_to {
            self.focus_node(&entity_id);
        }

        // Request repaint if animating or needs update
        if needs_repaint || self.camera_is_animating() {
            ui.ctx().request_repaint();
        }
    }

    /// Check if camera is still animating
    fn camera_is_animating(&self) -> bool {
        let center_diff = (self.camera.center - self.camera.target_center).length();
        let zoom_diff = (self.camera.zoom - self.camera.target_zoom).abs();
        center_diff > 0.1 || zoom_diff > 0.001
    }

    /// Render empty state when no graph is loaded
    fn render_empty_state(&self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.label("Select a CBU to visualize");
        });
    }

    /// Render UI chrome (stats, keyboard hints)
    fn render_chrome(&self, painter: &egui::Painter, graph: &LayoutGraph, screen_rect: Rect) {
        // Stats in top-left
        let stats_text = format!(
            "{} entities | {} edges",
            graph.nodes.len(),
            graph.edges.len()
        );
        painter.text(
            screen_rect.left_top() + Vec2::new(10.0, 20.0),
            egui::Align2::LEFT_TOP,
            stats_text,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(150, 150, 150),
        );

        // Category badge
        let category_text = format!("{:?}", graph.cbu_category);
        painter.text(
            screen_rect.left_top() + Vec2::new(10.0, 38.0),
            egui::Align2::LEFT_TOP,
            category_text,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(120, 120, 120),
        );

        // Zoom level in bottom-left
        let zoom_text = format!("Zoom: {:.0}%", self.camera.zoom * 100.0);
        painter.text(
            screen_rect.left_bottom() + Vec2::new(10.0, -30.0),
            egui::Align2::LEFT_BOTTOM,
            zoom_text,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(120, 120, 120),
        );

        // Keyboard hints in bottom-left
        let hints = "Drag: Pan | Scroll: Zoom | Click: Focus | Esc: Clear | R: Fit";
        painter.text(
            screen_rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            hints,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(100, 100, 100),
        );

        // Focus indicator in top-right
        if let Some(ref focus_id) = self.input_state.focused_node {
            if let Some(node) = graph.get_node(focus_id) {
                let focus_text = format!("Focus: {}", node.label);
                painter.text(
                    screen_rect.right_top() + Vec2::new(-10.0, 20.0),
                    egui::Align2::RIGHT_TOP,
                    focus_text,
                    egui::FontId::proportional(12.0),
                    Color32::from_rgb(96, 165, 250),
                );
            }
        }
    }
}

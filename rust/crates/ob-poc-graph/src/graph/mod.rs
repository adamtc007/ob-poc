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

pub mod animation;
pub mod astronomy;
pub mod camera;
pub mod colors;
pub mod edges;
pub mod focus_card;
pub mod force_sim;
pub mod galaxy;
pub mod input;
pub mod layout;
pub mod lod;
pub mod ontology;
pub mod render;
pub mod trading_matrix;
pub mod types;

pub use animation::{SpringConfig, SpringF32, SpringVec2};
pub use astronomy::{AstronomyView, NavigationEntry, TransitionAction, ViewTransition};
pub use camera::Camera2D;
pub use force_sim::{ClusterNode, ForceConfig, ForceSimulation};
#[allow(deprecated)]
pub use galaxy::{ClusterData, ClusterType, GalaxyAction, GalaxyView, RiskSummary};

// Re-export NavigationAction from shared types for galaxy navigation
pub use input::{InputHandler, InputState};
pub use layout::LayoutEngine;
pub use ob_poc_types::galaxy::NavigationAction;
pub use ontology::{
    entity_matches_type, get_entities_for_type, render_type_browser, EntityTypeOntology,
    TaxonomyState, TypeBrowserAction, TypeNode,
};
pub use render::GraphRenderer;
pub use trading_matrix::{
    get_node_type_color, get_node_type_icon, render_node_detail_panel,
    render_trading_matrix_browser, StatusColor, TradingMatrix, TradingMatrixAction,
    TradingMatrixMetadata, TradingMatrixNode, TradingMatrixNodeId, TradingMatrixNodeIdExt,
    TradingMatrixNodeType, TradingMatrixResponse, TradingMatrixState,
};
pub use types::*;

// Re-export PanDirection from ob-poc-types for Esper-style navigation
pub use ob_poc_types::PanDirection;

/// View mode for the graph visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// KYC/UBO view - shows core entities, KYC status, and ownership chains
    #[default]
    KycUbo,
    /// Service Delivery view - shows products, services, resources
    ServiceDelivery,
    /// Products only view - simplified product overview
    ProductsOnly,
    /// Trading view - CBU as container with trading entities (Asset Owner, IM, ManCo, etc.)
    Trading,
}

impl ViewMode {
    /// Get the API string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ViewMode::KycUbo => "KYC_UBO",
            ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
            ViewMode::ProductsOnly => "PRODUCTS_ONLY",
            ViewMode::Trading => "TRADING",
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ViewMode::KycUbo => "KYC / UBO",
            ViewMode::ServiceDelivery => "Services",
            ViewMode::ProductsOnly => "Products",
            ViewMode::Trading => "Trading",
        }
    }

    /// Get all available view modes (in display order)
    pub fn all() -> &'static [ViewMode] {
        &[
            ViewMode::KycUbo,
            ViewMode::Trading,
            ViewMode::ProductsOnly,
            ViewMode::ServiceDelivery,
        ]
    }

    /// Parse from API string representation
    pub fn from_str(s: &str) -> Option<ViewMode> {
        match s.to_uppercase().as_str() {
            "KYC_UBO" | "KYCUBO" | "KYC" | "UBO" => Some(ViewMode::KycUbo),
            "SERVICE_DELIVERY" | "SERVICEDELIVERY" | "SERVICES" => Some(ViewMode::ServiceDelivery),
            "PRODUCTS_ONLY" | "PRODUCTSONLY" | "PRODUCTS" => Some(ViewMode::ProductsOnly),
            "TRADING" | "CUSTODY" => Some(ViewMode::Trading),
            _ => None,
        }
    }
}

use egui::{Color32, Rect, Sense, Vec2};

// =============================================================================
// GRAPH ACTIONS (returned from widget, handled by caller)
// =============================================================================

/// Actions returned from the graph widget for the caller to handle
#[derive(Debug, Clone, Default)]
pub struct GraphWidgetAction {
    /// Container node was double-clicked (open browse panel)
    pub open_container: Option<ContainerInfo>,
    /// Entity was selected (update detail panel)
    pub select_entity: Option<String>,
}

/// Information about a container node for the browse panel
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Container node ID (e.g., share class ID)
    pub container_id: String,
    /// Container type (e.g., "share_class", "service_instance")
    pub container_type: String,
    /// Display label
    pub label: String,
    /// Parent key for scoped queries
    pub parent_key: Option<String>,
    /// EntityGateway nickname for searching children
    pub browse_nickname: Option<String>,
}

// =============================================================================
// GRAPH WIDGET
// =============================================================================

/// Main widget for CBU entity graph visualization
pub struct CbuGraphWidget {
    /// Camera for pan/zoom
    camera: Camera2D,
    /// Input state (hover, selection)
    input_state: InputState,
    /// Raw graph data from server (100% CBU structure)
    raw_data: Option<CbuGraphData>,
    /// Computed layout graph (filtered subset based on view mode)
    layout_graph: Option<LayoutGraph>,
    /// Renderer
    renderer: GraphRenderer,
    /// Current view mode
    view_mode: ViewMode,
    /// Whether to auto-fit on first render
    needs_initial_fit: bool,
    /// Type filter - if set, only show nodes matching this type (and their connected edges)
    type_filter: Option<String>,
    /// Highlighted type - nodes of this type are highlighted but others still visible
    highlighted_type: Option<String>,
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
            raw_data: None,
            layout_graph: None,
            renderer: GraphRenderer::new(),
            view_mode: ViewMode::KycUbo,
            needs_initial_fit: true,
            type_filter: None,
            highlighted_type: None,
        }
    }

    /// Set graph data from server response (stores full data, filters for current view)
    pub fn set_data(&mut self, data: CbuGraphData) {
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::console::log_1(
                &format!(
                    "CbuGraphWidget.set_data: {} nodes, {} edges, current view_mode={:?}",
                    data.nodes.len(),
                    data.edges.len(),
                    self.view_mode
                )
                .into(),
            );
            // Log first few nodes for debugging
            for (i, node) in data.nodes.iter().take(5).enumerate() {
                web_sys::console::log_1(
                    &format!(
                        "  node[{}]: id={}, type={}, layer={}, x={:?}, y={:?}",
                        i, node.id, node.node_type, node.layer, node.x, node.y
                    )
                    .into(),
                );
            }
        }

        self.raw_data = Some(data);
        self.recompute_layout();

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "after recompute_layout: layout_graph has {} nodes",
                self.layout_graph
                    .as_ref()
                    .map(|g| g.nodes.len())
                    .unwrap_or(0)
            )
            .into(),
        );

        self.needs_initial_fit = true;
        self.input_state.clear_focus();
    }

    /// Clear the graph
    pub fn clear(&mut self) {
        self.raw_data = None;
        self.layout_graph = None;
        self.input_state.clear_focus();
    }

    /// Set view mode and re-filter/re-layout
    /// Apply saved layout overrides (positions/sizes)
    /// Check if graph is loaded
    pub fn has_graph(&self) -> bool {
        self.layout_graph.is_some()
    }

    /// Peek at pending layout override without consuming it (for debounce check)
    pub fn peek_pending_layout_override(&self) -> bool {
        self.input_state.layout_dirty
    }

    pub fn apply_layout_override(&mut self, overrides: LayoutOverride) -> usize {
        if let Some(ref mut graph) = self.layout_graph {
            graph.apply_overrides(&overrides)
        } else {
            0
        }
    }

    /// Consume and return pending layout override changes for persistence
    pub fn take_pending_layout_override(&mut self) -> Option<LayoutOverride> {
        if !self.input_state.layout_dirty {
            return None;
        }
        let graph = self.layout_graph.as_ref()?;
        let mut positions = Vec::new();
        let mut sizes = Vec::new();
        for node in graph.nodes.values() {
            if node.offset.length_sq() > 0.01 {
                positions.push(NodeOffset {
                    node_id: node.id.clone(),
                    dx: node.offset.x,
                    dy: node.offset.y,
                });
            }
            if let Some(sz) = node.size_override {
                sizes.push(NodeSizeOverride {
                    node_id: node.id.clone(),
                    w: sz.x,
                    h: sz.y,
                });
            }
        }
        self.input_state.layout_dirty = false;
        if positions.is_empty() && sizes.is_empty() {
            return None;
        }
        Some(LayoutOverride { positions, sizes })
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if self.view_mode != mode {
            self.view_mode = mode;
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(&format!("View mode changed to: {:?}", mode).into());
            // Note: View mode change triggers a server refetch via the UI layer.
            // The client just stores the mode for reference; actual filtering is done server-side.
            self.needs_initial_fit = true;
        }
    }

    /// Compute layout from raw data (no filtering - server provides filtered data)
    fn recompute_layout(&mut self) {
        let Some(ref raw) = self.raw_data else {
            self.layout_graph = None;
            return;
        };

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "Recompute layout: {} nodes (server-filtered for view={:?})",
                raw.nodes.len(),
                self.view_mode
            )
            .into(),
        );

        // Server already filtered by view_mode - client just computes layout
        let category = raw
            .cbu_category
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();

        let engine = LayoutEngine::with_view_mode(category, self.view_mode);
        self.layout_graph = Some(engine.compute_layout(raw));
    }

    /// Get current view mode
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Set type filter - only nodes matching this type (and connected edges) are fully visible
    /// Other nodes are rendered with reduced opacity
    pub fn set_type_filter(&mut self, type_code: Option<String>) {
        self.type_filter = type_code;
    }

    /// Get current type filter
    pub fn type_filter(&self) -> Option<&str> {
        self.type_filter.as_deref()
    }

    /// Set highlighted type - nodes of this type are highlighted but others still visible
    pub fn set_highlighted_type(&mut self, type_code: Option<String>) {
        self.highlighted_type = type_code;
    }

    /// Get current highlighted type
    pub fn highlighted_type(&self) -> Option<&str> {
        self.highlighted_type.as_deref()
    }

    /// Clear all type-based filtering and highlighting
    pub fn clear_type_filter(&mut self) {
        self.type_filter = None;
        self.highlighted_type = None;
    }

    /// Get reference to layout graph (for ontology population)
    pub fn get_layout_graph(&self) -> Option<&LayoutGraph> {
        self.layout_graph.as_ref()
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

    /// Focus on a specific entity by ID (alias for focus_node for WASM bridge)
    pub fn focus_entity(&mut self, entity_id: &str) {
        self.focus_node(entity_id);
    }

    /// Set loading state (shows spinner, clears graph)
    pub fn set_loading(&mut self, loading: bool) {
        if loading {
            self.raw_data = None;
            self.layout_graph = None;
        }
        // Note: actual loading indicator would be rendered in ui()
    }

    /// Check if selected entity changed since last call (for JS bridge notification)
    /// Returns the newly selected entity ID if changed
    pub fn selected_entity_changed(&mut self) -> Option<String> {
        // For now, just return the current focus - the JS bridge will track changes
        self.input_state.focused_node.clone()
    }

    /// Take pending container open action (consumes it)
    /// Returns ContainerInfo if a container node was double-clicked
    pub fn take_container_action(&mut self) -> Option<ContainerInfo> {
        self.input_state.take_pending_container_open()
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

    // =========================================================================
    // ESPER-STYLE NAVIGATION COMMANDS (Blade Runner voice control)
    // =========================================================================

    /// Zoom in by factor ("enhance", "zoom in", "closer")
    /// factor: 1.2 = 20% zoom in, 2.0 = 2x zoom
    pub fn zoom_in(&mut self, factor: Option<f32>) {
        let factor = factor.unwrap_or(1.3);
        let current = self.camera.zoom();
        self.camera.zoom_to(current * factor);
    }

    /// Zoom out by factor ("zoom out", "pull back", "wider")
    pub fn zoom_out(&mut self, factor: Option<f32>) {
        let factor = factor.unwrap_or(1.3);
        let current = self.camera.zoom();
        self.camera.zoom_to(current / factor);
    }

    /// Zoom to fit all content ("zoom fit", "show all", "full view")
    pub fn zoom_fit(&mut self) {
        // Note: needs screen_rect, so we set a flag and do it in ui()
        self.needs_initial_fit = true;
    }

    /// Zoom to specific level ("zoom to 200%", "set zoom 1.5")
    pub fn zoom_to_level(&mut self, level: f32) {
        self.camera.zoom_to(level);
    }

    /// Pan in direction ("track left", "pan right", "move up")
    /// amount: pixels to pan (default 100)
    pub fn pan_direction(&mut self, direction: PanDirection, amount: Option<f32>) {
        let amount = amount.unwrap_or(100.0);
        let delta = match direction {
            PanDirection::Left => egui::Vec2::new(-amount, 0.0),
            PanDirection::Right => egui::Vec2::new(amount, 0.0),
            PanDirection::Up => egui::Vec2::new(0.0, -amount),
            PanDirection::Down => egui::Vec2::new(0.0, amount),
        };
        self.camera.pan(delta);
    }

    /// Center on graph content ("center", "home")
    pub fn center_view(&mut self) {
        if let Some(ref graph) = self.layout_graph {
            self.camera.fly_to(graph.bounds.center());
        }
    }

    /// Stop all animation ("stop", "hold", "freeze")
    pub fn stop_animation(&mut self) {
        self.camera.snap_to_target();
    }

    /// Check if camera is animating
    pub fn is_animating(&self) -> bool {
        self.camera.is_animating()
    }

    // =========================================================================
    // VOICE COMMAND HELPERS (for unified command dispatcher)
    // =========================================================================

    /// Get currently selected/focused entity ID (for voice command context)
    pub fn selected_entity_id(&self) -> Option<String> {
        self.input_state.focused_node.clone()
    }

    /// Focus on entity by ID and pan camera to it
    pub fn focus_on_entity(&mut self, entity_id: &str) {
        self.focus_entity(entity_id);
    }

    /// Select next entity in graph (cycles through nodes)
    pub fn select_next_entity(&mut self) {
        let next_id = {
            let Some(ref graph) = self.layout_graph else {
                return;
            };
            let node_ids: Vec<&String> = graph.nodes.keys().collect();
            if node_ids.is_empty() {
                return;
            }

            let current_idx = self
                .input_state
                .focused_node
                .as_ref()
                .and_then(|id| node_ids.iter().position(|n| *n == id));

            let next_idx = match current_idx {
                Some(idx) => (idx + 1) % node_ids.len(),
                None => 0,
            };

            node_ids.get(next_idx).map(|s| (*s).clone())
        };

        if let Some(id) = next_id {
            self.focus_node(&id);
        }
    }

    /// Select previous entity in graph (cycles through nodes)
    pub fn select_previous_entity(&mut self) {
        let prev_id = {
            let Some(ref graph) = self.layout_graph else {
                return;
            };
            let node_ids: Vec<&String> = graph.nodes.keys().collect();
            if node_ids.is_empty() {
                return;
            }

            let current_idx = self
                .input_state
                .focused_node
                .as_ref()
                .and_then(|id| node_ids.iter().position(|n| *n == id));

            let prev_idx = match current_idx {
                Some(idx) if idx > 0 => idx - 1,
                Some(_) => node_ids.len() - 1,
                None => 0,
            };

            node_ids.get(prev_idx).map(|s| (*s).clone())
        };

        if let Some(id) = prev_id {
            self.focus_node(&id);
        }
    }

    /// Clear current selection/focus
    pub fn clear_selection(&mut self) {
        self.input_state.clear_focus();
    }

    /// Go back in navigation history (placeholder - could track history)
    pub fn go_back(&mut self) {
        // For now, just center the view
        self.center_view();
    }

    /// Zoom to fit all content (triggers fit on next frame)
    pub fn zoom_to_fit(&mut self) {
        self.zoom_fit();
    }

    /// Set zoom level directly ("zoom to 150%")
    pub fn set_zoom(&mut self, level: f32) {
        self.zoom_to_level(level);
    }

    /// Pan by pixel amount ("pan left 50", "move right")
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.camera.pan(egui::Vec2::new(dx, dy));
    }

    /// Reset layout to default positions
    pub fn reset_layout(&mut self) {
        // Trigger re-layout by clearing and re-setting data
        self.needs_initial_fit = true;
        self.center_view();
    }

    /// Highlight entities of a specific type
    pub fn highlight_type(&mut self, _type_code: &str) {
        // TODO: Implement type highlighting
        // This would set a filter or highlight state on nodes matching the type
    }

    /// Clear any type highlighting
    pub fn clear_highlight(&mut self) {
        // TODO: Clear type highlighting
    }

    /// Main UI function
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(graph) = self.layout_graph.as_mut() else {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(&"ui(): No layout_graph, showing empty state".into());
            self.render_empty_state(ui);
            return;
        };

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "ui(): Rendering graph with {} nodes, bounds={:?}",
                graph.nodes.len(),
                graph.bounds
            )
            .into(),
        );

        // Allocate space and get painter
        let available = ui.available_size();
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&format!("graph ui(): available_size={:?}", available).into());

        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());

        let screen_rect = response.rect;

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "graph ui(): screen_rect={:?}, hovered={}",
                screen_rect,
                response.hovered()
            )
            .into(),
        );

        // Initial fit on first render
        if self.needs_initial_fit {
            self.camera.fit_to_bounds(graph.bounds, screen_rect, 50.0);
            self.camera.snap_to_target();
            self.needs_initial_fit = false;
        }

        // Update camera interpolation
        let dt = ui.input(|i| i.stable_dt);
        self.camera.update(dt);

        // Handle input
        let needs_repaint = InputHandler::handle_input(
            &response,
            &mut self.camera,
            &mut self.input_state,
            graph,
            screen_rect,
        );

        // Set cursor
        ui.ctx()
            .set_cursor_icon(input::cursor_for_state(&self.input_state));

        // Render graph with type filter support
        self.renderer.render(
            &painter,
            graph,
            &self.camera,
            screen_rect,
            self.input_state.focused_node.as_deref(),
            self.type_filter.as_deref(),
            self.highlighted_type.as_deref(),
        );

        // Render UI chrome (stats, controls)
        // Drop mutable borrow, reborrow immutably for rendering
        let graph = self.layout_graph.as_ref().unwrap();
        self.render_chrome(&painter, graph, screen_rect);

        // Render focus card if a node is focused
        // Rule 2: Handle returned action AFTER rendering, not via callbacks
        if let Some(ref focus_id) = self.input_state.focused_node {
            if let Some(node) = graph.get_node(focus_id) {
                let card_data = focus_card::build_focus_card_data(node, graph);
                let action = focus_card::render_focus_card(ui.ctx(), &card_data);

                // Handle action AFTER rendering completes (Rule 2)
                match action {
                    focus_card::FocusCardAction::Close => {
                        self.input_state.clear_focus();
                    }
                    focus_card::FocusCardAction::Navigate(entity_id) => {
                        self.focus_node(&entity_id);
                    }
                    focus_card::FocusCardAction::None => {}
                }
            }
        }

        // Request repaint if animating or needs update
        if needs_repaint || self.camera_is_animating() {
            ui.ctx().request_repaint();
        }
    }

    /// Check if camera is still animating
    fn camera_is_animating(&self) -> bool {
        self.camera.is_animating()
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
        let zoom_text = format!("Zoom: {:.0}%", self.camera.zoom() * 100.0);
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

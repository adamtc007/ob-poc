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

pub mod aggregation;
pub mod animation;
pub mod astronomy;
pub mod board_control;
pub mod camera;
pub mod cluster;
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
pub mod sdf;
pub mod service_taxonomy;
pub mod spatial;
pub mod trading_matrix;
pub mod types;
pub mod viewport;
pub mod viewport_fit;

pub use animation::{EsperTransition, EsperTransitionState, SpringConfig, SpringF32, SpringVec2};
pub use astronomy::{AstronomyView, NavigationEntry, TransitionAction, ViewTransition};
pub use board_control::{
    compute_ownership_tree_layout, render_board_control_hud, render_evidence_panel,
    render_psc_badges, render_source_indicator, BoardControlAction, BoardControlState,
    ControlSphereData, ControlSphereEdge, ControlSphereNode, EvidenceItem,
};
pub use camera::Camera2D;
pub use force_sim::{ClusterNode, ForceConfig, ForceSimulation};
#[allow(deprecated)]
pub use galaxy::{ClusterData, ClusterType, GalaxyAction, GalaxyView, RiskSummary};
pub use lod::{DetailLevel, LodConfig};

// Re-export NavigationAction from shared types for galaxy navigation
pub use aggregation::{
    aggregate_to_galaxies, aggregate_to_regions, create_aggregated_graph, GalaxyNode, RegionNode,
};
pub use input::{EnhanceAction, InputHandler, InputState};
pub use layout::LayoutEngine;
pub use ob_poc_types::galaxy::NavigationAction;
pub use ontology::{
    entity_matches_type, get_entities_for_type, render_type_browser, EntityTypeOntology,
    TaxonomyState, TypeBrowserAction, TypeNode,
};
pub use render::GraphRenderer;
pub use service_taxonomy::{
    render_service_detail_panel, render_service_taxonomy, IntentData, ProductData, ResourceData,
    ServiceData, ServiceStatus, ServiceTaxonomy, ServiceTaxonomyAction, ServiceTaxonomyNode,
    ServiceTaxonomyNodeId, ServiceTaxonomyNodeType, ServiceTaxonomyState, ServiceTaxonomyStats,
};
pub use trading_matrix::{
    get_node_type_color, get_node_type_icon, render_node_detail_panel,
    render_trading_matrix_browser, StatusColor, TradingMatrix, TradingMatrixAction,
    TradingMatrixMetadata, TradingMatrixNode, TradingMatrixNodeId, TradingMatrixNodeIdExt,
    TradingMatrixNodeType, TradingMatrixResponse, TradingMatrixState,
};
pub use types::*;
pub use viewport::{
    render_focus_ring, render_node_with_confidence, render_viewport_hud, EsperRenderState, GapType,
    IlluminateAspect, RedFlagCategory, ViewportAction, ViewportRenderState,
};
pub use viewport_fit::{ViewLevel, ViewportFit, MAX_VISIBLE_NODES};

// SDF primitives and operations for hit testing and confidence visualization
pub use sdf::{cluster_blob, hit_test_circle, hit_test_rect, ConfidenceHalo, HitResult};

// R-tree spatial index for O(log n) hit testing
pub use spatial::{SpatialIndex, SpatialNode};

// Re-export PanDirection from ob-poc-types for Esper-style navigation
pub use ob_poc_types::PanDirection;

// Import ViewportState for apply_viewport_state method
use ob_poc_types::viewport::{CbuViewType, EnhanceArg, ViewportFocusState, ViewportState};

/// View mode for the graph visualization - CBU Trading view only
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewMode;

impl ViewMode {
    /// Get the API string representation
    pub fn as_str(&self) -> &'static str {
        "TRADING"
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        "CBU"
    }

    /// Get all available view modes (just one now)
    pub fn all() -> &'static [ViewMode] {
        &[ViewMode]
    }

    /// Parse from API string representation - always returns ViewMode (CBU/Trading)
    pub fn parse(_s: &str) -> Option<ViewMode> {
        Some(ViewMode)
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
    /// Navigate back from current view (BoardControl → CBU, Matrix → CBU)
    pub navigate_back: Option<NavigateBackAction>,
    /// Navigate into an external taxonomy (InstrumentMatrix, Products, etc.)
    pub navigate_taxonomy: Option<TaxonomyNavigationAction>,
}

/// Taxonomy navigation action - triggered by clicking on external taxonomy nodes
#[derive(Debug, Clone)]
pub enum TaxonomyNavigationAction {
    /// Navigate to Trading Matrix view (Instrument Matrix / Markets)
    TradingMatrix,
    /// Navigate to Service Taxonomy view (Products / Services)
    ServiceTaxonomy,
}

/// Navigation back action with context
#[derive(Debug, Clone)]
pub struct NavigateBackAction {
    /// CBU to return to
    pub target_cbu_id: uuid::Uuid,
    /// CBU name for display
    pub target_cbu_name: Option<String>,
    /// Source view we're navigating from
    pub from_view: ViewMode,
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
    /// Auto-fit state (tracks optimal zoom, auto/manual mode)
    viewport_fit: ViewportFit,
    /// Last known viewport size (for resize detection)
    last_viewport_size: Option<egui::Vec2>,
    /// Type filter - if set, only show nodes matching this type (and their connected edges)
    type_filter: Option<String>,
    /// Highlighted type - nodes of this type are highlighted but others still visible
    highlighted_type: Option<String>,
    /// Focused matrix node path - when set, highlights trading-related graph nodes
    /// The path is a list of segments like ["Equities", "Listed", "NYSE"]
    matrix_focus_path: Option<Vec<String>>,
    /// Viewport render state for HUD animations
    viewport_render_state: ViewportRenderState,
    /// Current viewport state from DSL (for HUD rendering)
    viewport_state: Option<ViewportState>,
    /// Esper-style visual mode toggles (xray, peel, shadow, etc.)
    esper_render_state: EsperRenderState,
    /// Esper-style discrete stepped transition for enhance level changes
    esper_transition: Option<EsperTransition>,
    /// Current enhance level (0-4)
    current_enhance_level: u8,
    /// Evidence panel expanded state (for BoardControl view)
    evidence_panel_expanded: bool,
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
            view_mode: ViewMode,
            needs_initial_fit: true,
            viewport_fit: ViewportFit::new(),
            last_viewport_size: None,
            type_filter: None,
            highlighted_type: None,
            matrix_focus_path: None,
            viewport_render_state: ViewportRenderState::new(),
            viewport_state: None,
            esper_render_state: EsperRenderState::new(),
            esper_transition: None,
            current_enhance_level: 0,
            evidence_panel_expanded: false,
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
    /// Preserves user-dragged positions (offsets) from previous layout
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

        // Preserve user-dragged offsets from existing layout
        let saved_offsets: std::collections::HashMap<String, egui::Vec2> = self
            .layout_graph
            .as_ref()
            .map(|g| {
                g.nodes
                    .iter()
                    .filter(|(_, node)| node.offset != egui::Vec2::ZERO)
                    .map(|(id, node)| (id.clone(), node.offset))
                    .collect()
            })
            .unwrap_or_default();

        // Server already filtered by view_mode - client just computes layout
        let category = raw
            .cbu_category
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();

        let engine = LayoutEngine::with_view_mode(category, self.view_mode);
        let mut new_layout = engine.compute_layout(raw);

        // Restore user-dragged offsets to matching nodes
        for (id, offset) in saved_offsets {
            if let Some(node) = new_layout.nodes.get_mut(&id) {
                node.offset = offset;
                // Update position to include the offset
                node.position = node.base_position + offset;
            }
        }

        self.layout_graph = Some(new_layout);
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

    /// Set matrix focus path - highlights trading entities when set
    pub fn set_matrix_focus_path(&mut self, path: Option<Vec<String>>) {
        self.matrix_focus_path = path;
    }

    /// Get current matrix focus path
    pub fn matrix_focus_path(&self) -> Option<&[String]> {
        self.matrix_focus_path.as_deref()
    }

    /// Clear matrix focus highlighting
    pub fn clear_matrix_focus(&mut self) {
        self.matrix_focus_path = None;
    }

    /// Toggle evidence panel visibility (BoardControl mode)
    pub fn toggle_evidence_panel(&mut self) {
        self.evidence_panel_expanded = !self.evidence_panel_expanded;
    }

    /// Check if evidence panel is expanded
    pub fn is_evidence_panel_expanded(&self) -> bool {
        self.evidence_panel_expanded
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

    /// Take pending taxonomy navigation action (consumes it)
    /// Returns TaxonomyNavigationAction if a taxonomy node was double-clicked
    pub fn take_taxonomy_navigation_action(&mut self) -> Option<TaxonomyNavigationAction> {
        self.input_state.take_pending_taxonomy_navigation()
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

    /// Reset view to auto-fit mode (re-enables auto-fit after manual zoom)
    pub fn reset_to_auto_fit(&mut self) {
        self.viewport_fit.enable_auto();
        self.needs_initial_fit = true;
    }

    /// Check if auto-fit is currently enabled
    pub fn is_auto_fit_enabled(&self) -> bool {
        self.viewport_fit.auto_enabled
    }

    /// Get current view level (Galaxy/Region/Cluster/Solar)
    pub fn current_view_level(&self) -> ViewLevel {
        self.viewport_fit.current_view_level()
    }

    /// Check if aggregation is needed at current view level
    pub fn needs_aggregation(&self) -> bool {
        self.viewport_fit.needs_aggregation()
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

    /// Navigate back from special views (BoardControl, InstrumentMatrix) to CBU view
    /// Returns NavigateBackAction if we're in a special view with a source CBU
    pub fn navigate_back_from_view(&mut self) -> Option<NavigateBackAction> {
        // Check if we're in a special view mode with a source CBU
        if let Some(ref state) = self.viewport_state {
            match &state.focus.state {
                ViewportFocusState::BoardControl { source_cbu, .. } => {
                    let action = NavigateBackAction {
                        target_cbu_id: source_cbu.0,
                        target_cbu_name: None, // Will be resolved by caller
                        from_view: ViewMode,
                    };
                    // Reset to KycUbo view mode
                    self.view_mode = ViewMode;
                    return Some(action);
                }
                ViewportFocusState::InstrumentMatrix { cbu, .. }
                | ViewportFocusState::InstrumentType { cbu, .. }
                | ViewportFocusState::ConfigNode { cbu, .. } => {
                    let action = NavigateBackAction {
                        target_cbu_id: cbu.0,
                        target_cbu_name: None,
                        from_view: ViewMode,
                    };
                    // Reset to KycUbo view mode and clear matrix focus
                    self.view_mode = ViewMode;
                    self.matrix_focus_path = None;
                    return Some(action);
                }
                _ => {}
            }
        }
        None
    }

    /// Check if we're in a special view that can navigate back
    pub fn can_navigate_back(&self) -> bool {
        if let Some(ref state) = self.viewport_state {
            matches!(
                &state.focus.state,
                ViewportFocusState::BoardControl { .. }
                    | ViewportFocusState::InstrumentMatrix { .. }
                    | ViewportFocusState::InstrumentType { .. }
                    | ViewportFocusState::ConfigNode { .. }
            )
        } else {
            false
        }
    }

    /// Get the current view mode
    pub fn current_view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Set focus to an instrument matrix node
    /// Used when user clicks on a trading matrix node
    pub fn set_instrument_matrix_focus(&mut self, cbu_id: uuid::Uuid, node_path: Vec<String>) {
        use ob_poc_types::viewport::{
            CbuRef, FocusManager, InstrumentMatrixRef, ViewportFocusState, ViewportState,
        };

        let cbu_ref = CbuRef::new(cbu_id);
        // Use cbu_id as matrix ref since they're 1:1
        let matrix_ref = InstrumentMatrixRef(cbu_id);

        let focus_state = if node_path.is_empty() {
            // Root matrix focus
            ViewportFocusState::InstrumentMatrix {
                cbu: cbu_ref,
                matrix: matrix_ref,
                matrix_enhance: 0,
                container_enhance: 1,
            }
        } else {
            // Specific node in matrix - still use InstrumentMatrix for now
            // Could be enhanced to use InstrumentType if we parse the path
            ViewportFocusState::InstrumentMatrix {
                cbu: cbu_ref,
                matrix: matrix_ref,
                matrix_enhance: 1, // Show more detail when specific node selected
                container_enhance: 1,
            }
        };

        // Create or update viewport state
        let state = ViewportState {
            focus: FocusManager {
                state: focus_state,
                focus_stack: Vec::new(),
                focus_mode: Default::default(),
                view_memory: Default::default(),
            },
            view_type: ob_poc_types::viewport::CbuViewType::Instruments,
            camera: self
                .viewport_state
                .as_ref()
                .map(|s| s.camera.clone())
                .unwrap_or_default(),
            confidence_threshold: 0.0,
            filters: Default::default(),
        };

        self.viewport_state = Some(state);
        self.view_mode = ViewMode;

        // Store the matrix focus path for graph highlighting
        self.matrix_focus_path = if node_path.is_empty() {
            None
        } else {
            Some(node_path.clone())
        };

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "set_instrument_matrix_focus: cbu={}, path={:?}",
                cbu_id, node_path
            )
            .into(),
        );
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

    // =========================================================================
    // ESPER RENDER STATE ACCESSORS (for NavigationVerb handlers)
    // =========================================================================

    /// Get mutable reference to Esper render state for toggle operations
    pub fn esper_render_state_mut(&mut self) -> &mut EsperRenderState {
        &mut self.esper_render_state
    }

    /// Get reference to Esper render state for query operations
    pub fn esper_render_state(&self) -> &EsperRenderState {
        &self.esper_render_state
    }

    /// Check if any Esper visual mode is active
    pub fn has_active_esper_mode(&self) -> bool {
        self.esper_render_state.any_mode_active()
    }

    /// Reset all Esper visual modes to default
    pub fn reset_esper_modes(&mut self) {
        self.esper_render_state.reset();
    }

    // =========================================================================
    // ESPER-STYLE ENHANCE LEVEL TRANSITIONS
    // =========================================================================

    /// Set enhance level with Esper-style discrete stepped transition.
    /// Each step holds for 100ms with a 1.03x scale pulse "click" effect.
    pub fn set_enhance_level(&mut self, target_level: u8) {
        let target = target_level.min(4); // Clamp to 0-4
        if target != self.current_enhance_level {
            self.esper_transition = Some(EsperTransition::new(self.current_enhance_level, target));
        }
    }

    /// Increment enhance level by 1 (with Esper transition)
    pub fn increment_enhance_level(&mut self) {
        let new_level = (self.current_enhance_level + 1).min(4);
        self.set_enhance_level(new_level);
    }

    /// Decrement enhance level by 1 (with Esper transition)
    pub fn decrement_enhance_level(&mut self) {
        let new_level = self.current_enhance_level.saturating_sub(1);
        self.set_enhance_level(new_level);
    }

    /// Get current enhance level (accounts for mid-transition state)
    pub fn enhance_level(&self) -> u8 {
        self.esper_transition
            .as_ref()
            .map(|t| t.current_level())
            .unwrap_or(self.current_enhance_level)
    }

    /// Get enhance scale factor for "click" pulse effect during transitions
    pub fn enhance_scale(&self) -> f32 {
        self.esper_transition
            .as_ref()
            .map(|t| t.scale())
            .unwrap_or(1.0)
    }

    /// Check if an enhance transition is currently in progress
    pub fn is_enhance_transitioning(&self) -> bool {
        self.esper_transition
            .as_ref()
            .map(|t| !t.is_complete())
            .unwrap_or(false)
    }

    /// Tick the Esper transition (call each frame with delta time).
    /// Returns true if still animating (needs repaint).
    fn tick_esper_transition(&mut self, dt: f32) -> bool {
        if let Some(ref mut transition) = self.esper_transition {
            transition.update(dt);
            if transition.is_complete() {
                // Transition complete - commit final level and clear
                self.current_enhance_level = transition.current_level();
                self.esper_transition = None;
                false
            } else {
                true // Still animating
            }
        } else {
            false
        }
    }

    // =========================================================================
    // LAYOUT ORIENTATION & SEARCH (AgentCommand handlers)
    // =========================================================================

    /// Toggle between VERTICAL and HORIZONTAL layout orientation.
    /// This affects how the force simulation arranges nodes.
    pub fn toggle_orientation(&mut self) {
        // For now, trigger a re-layout with the camera reset
        // Future: Add orientation state to LayoutEngine and ForceConfig
        self.needs_initial_fit = true;
        self.recompute_layout();
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"toggle_orientation: layout recomputed".into());
    }

    /// Search for entities matching a query string and highlight/focus them.
    /// If a single match is found, focus on it. If multiple, highlight all.
    pub fn search_entities(&mut self, query: &str) {
        let Some(ref graph) = self.layout_graph else {
            return;
        };

        let query_lower = query.to_lowercase();
        let mut matches: Vec<String> = Vec::new();

        // Search through nodes by label (fuzzy match on lowercase)
        for (id, node) in &graph.nodes {
            if node.label.to_lowercase().contains(&query_lower) {
                matches.push(id.clone());
            }
        }

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "search_entities: query='{}' found {} matches",
                query,
                matches.len()
            )
            .into(),
        );

        match matches.len() {
            0 => {
                // No matches - clear any existing filter
                self.clear_type_filter();
            }
            1 => {
                // Single match - focus on it
                let id = matches.into_iter().next().unwrap();
                self.focus_node(&id);
            }
            _ => {
                // Multiple matches - highlight the first one and set type filter
                // Future: Could implement multi-node highlighting
                if let Some(first_id) = matches.first() {
                    self.focus_node(first_id);
                }
            }
        }
    }

    /// Main UI function
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Update time-based animations BEFORE borrowing graph
        let dt = ui.input(|i| i.stable_dt);
        self.camera.update(dt);
        let esper_animating = self.tick_esper_transition(dt);

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

        // Auto-fit: initial fit, resize detection, content change
        let current_size = screen_rect.size();
        let size_changed = self.last_viewport_size.is_none_or(|prev| {
            (prev.x - current_size.x).abs() > 10.0 || (prev.y - current_size.y).abs() > 10.0
        });

        if self.needs_initial_fit {
            // First render - compute and apply auto-fit
            self.viewport_fit.on_content_change(graph, current_size);
            self.camera.fit_to_bounds(graph.bounds, screen_rect, 50.0);
            self.camera.snap_to_target();
            self.needs_initial_fit = false;
            self.last_viewport_size = Some(current_size);
        } else if size_changed && self.viewport_fit.auto_enabled {
            // Viewport resized and auto-fit is on - recalculate
            self.viewport_fit.on_resize(current_size);
            self.camera
                .fly_to_bounds(self.viewport_fit.content_bounds, screen_rect, 50.0);
            self.last_viewport_size = Some(current_size);
        }

        // Handle input - track zoom before to detect user zoom
        let zoom_before = self.camera.target_zoom();
        let needs_repaint = InputHandler::handle_input(
            &response,
            &mut self.camera,
            &mut self.input_state,
            graph,
            screen_rect,
        );

        // Disable auto-fit if user manually zoomed or panned
        if needs_repaint && (self.camera.target_zoom() - zoom_before).abs() > 0.001 {
            self.viewport_fit.disable_auto();
        }

        // Update view level based on current zoom (for future aggregation)
        let _view_level_changed = self.viewport_fit.update_view_level(self.camera.zoom());

        // Capture pending enhance action (will handle after graph borrow released)
        let pending_enhance = self.input_state.take_pending_enhance_action();

        // Set cursor
        ui.ctx()
            .set_cursor_icon(input::cursor_for_state(&self.input_state));

        // Render graph with type filter support and Esper visual modes
        self.renderer.render(
            &painter,
            graph,
            &self.camera,
            screen_rect,
            &render::RenderOptions {
                focused_node: self.input_state.focused_node.as_deref(),
                type_filter: self.type_filter.as_deref(),
                highlighted_type: self.highlighted_type.as_deref(),
                esper_state: Some(&self.esper_render_state),
                matrix_focus_path: self.matrix_focus_path.as_deref(),
            },
        );

        // Render viewport HUD overlay if viewport_state is available
        // This shows breadcrumbs, enhance level indicator, confidence legend, view type selector
        // Capture action to handle after graph borrows are released
        let pending_hud_action = if let Some(ref viewport_state) = self.viewport_state {
            render_viewport_hud(
                ui,
                viewport_state,
                &mut self.viewport_render_state,
                screen_rect,
            )
        } else {
            ViewportAction::None
        };

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

        // Render evidence panel when in BoardControl mode
        if self.view_mode == ViewMode {
            // Sample evidence items for demo - in production these would come from server
            let evidence = vec![
                board_control::EvidenceItem {
                    source: "GLEIF".to_string(),
                    document_type: "LEI Registration".to_string(),
                    date: Some("2024-01-15".to_string()),
                    description: "Legal Entity Identifier registration record".to_string(),
                    url: Some("https://gleif.org".to_string()),
                },
                board_control::EvidenceItem {
                    source: "UK_PSC".to_string(),
                    document_type: "PSC Statement".to_string(),
                    date: Some("2023-11-22".to_string()),
                    description: "Persons with Significant Control register".to_string(),
                    url: None,
                },
            ];
            board_control::render_evidence_panel(
                &painter,
                screen_rect,
                &evidence,
                self.evidence_panel_expanded,
            );
        }

        // Handle pending enhance action (after graph borrows released)
        if let Some(action) = pending_enhance {
            match action {
                input::EnhanceAction::Increment => self.increment_enhance_level(),
                input::EnhanceAction::Decrement => self.decrement_enhance_level(),
            }
        }

        // Handle pending viewport HUD action (after graph borrows released)
        match pending_hud_action {
            ViewportAction::Enhance { arg } => match arg {
                EnhanceArg::Increment => self.increment_enhance_level(),
                EnhanceArg::Decrement => self.decrement_enhance_level(),
                EnhanceArg::Level(level) => self.set_enhance_level(level),
                EnhanceArg::Max => self.set_enhance_level(4),
                EnhanceArg::Reset => self.set_enhance_level(0),
            },
            ViewportAction::Ascend | ViewportAction::AscendToRoot => {
                // Clear focus when ascending in hierarchy
                self.input_state.clear_focus();
            }
            ViewportAction::FocusEntity { entity_id } => {
                self.focus_node(&entity_id.to_string());
            }
            ViewportAction::ToggleEntityTypeFilter { entity_type } => {
                // Convert ConcreteEntityType to the string format used by type_filter
                let type_str = match entity_type {
                    ob_poc_types::viewport::ConcreteEntityType::Company => "LIMITED_COMPANY",
                    ob_poc_types::viewport::ConcreteEntityType::Partnership => "PARTNERSHIP",
                    ob_poc_types::viewport::ConcreteEntityType::Trust => "TRUST",
                    ob_poc_types::viewport::ConcreteEntityType::Person => "PROPER_PERSON",
                };
                // Toggle the entity type filter
                if self.type_filter.as_deref() == Some(type_str) {
                    self.set_type_filter(None);
                } else {
                    self.set_type_filter(Some(type_str.to_string()));
                }
            }
            ViewportAction::ClearFilters => {
                self.set_type_filter(None);
            }
            ViewportAction::SetConfidenceThreshold { threshold } => {
                self.viewport_render_state
                    .set_confidence_threshold(threshold);
            }
            // Actions that require server round-trip or don't apply to local widget state
            ViewportAction::None
            | ViewportAction::FocusCbu { .. }
            | ViewportAction::FocusMatrix
            | ViewportAction::FocusInstrumentType { .. }
            | ViewportAction::ChangeViewType { .. }
            | ViewportAction::SetSearchText { .. }
            | ViewportAction::NavigateToBreadcrumb { .. } => {
                // These actions need to be propagated to the app layer
                // via GraphWidgetResponse or similar mechanism
            }
        }

        // Request repaint if animating or needs update
        if needs_repaint || self.camera_is_animating() || esper_animating {
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
        let hints = "Drag: Pan | Scroll: Zoom | Click: Focus | E/[]: Enhance | Esc: Clear | R: Fit";
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

        // Enhance level indicator in bottom-right (Esper-style with scale pulse)
        self.render_enhance_indicator(painter, screen_rect);
    }

    /// Render the Esper-style enhance level indicator with discrete step visualization
    fn render_enhance_indicator(&self, painter: &egui::Painter, screen_rect: Rect) {
        let level = self.enhance_level();
        let scale = self.enhance_scale();
        let is_transitioning = self.is_enhance_transitioning();

        // Position in bottom-right
        let base_pos = screen_rect.right_bottom() + Vec2::new(-80.0, -40.0);

        // Apply scale pulse effect during transitions
        let font_size = 14.0 * scale;
        let indicator_color = if is_transitioning {
            // Gold color during transition "click"
            Color32::from_rgb(255, 193, 7)
        } else {
            // Muted color when stable
            Color32::from_rgb(150, 150, 150)
        };

        // Level text
        let level_text = format!("Enhance: {}", level);
        painter.text(
            base_pos,
            egui::Align2::RIGHT_BOTTOM,
            level_text,
            egui::FontId::proportional(font_size),
            indicator_color,
        );

        // Visual pips showing discrete steps (0-4)
        let pip_y = base_pos.y - 20.0;
        let pip_spacing = 12.0 * scale;
        let pip_radius = 4.0 * scale;

        for i in 0..=4u8 {
            let pip_x = base_pos.x - 70.0 + (i as f32 * pip_spacing);
            let pip_pos = egui::Pos2::new(pip_x, pip_y);

            let pip_color = if i <= level {
                if is_transitioning && i == level {
                    // Current step during transition - bright gold
                    Color32::from_rgb(255, 215, 0)
                } else {
                    // Filled pip - cyan
                    Color32::from_rgb(96, 165, 250)
                }
            } else {
                // Empty pip - dark
                Color32::from_rgb(60, 60, 60)
            };

            painter.circle_filled(pip_pos, pip_radius, pip_color);
        }

        // Hint text
        let hint = "+/- keys";
        painter.text(
            base_pos + Vec2::new(0.0, 15.0),
            egui::Align2::RIGHT_BOTTOM,
            hint,
            egui::FontId::proportional(9.0),
            Color32::from_rgb(80, 80, 80),
        );
    }

    // =========================================================================
    // VIEWPORT STATE APPLICATION (from DSL viewport.* verbs via session)
    // =========================================================================

    /// Apply a ViewportState from the session to this graph widget.
    ///
    /// This maps the DSL-driven viewport state to the widget's internal state:
    /// - FocusManager.state → focus_node / clear_selection
    /// - CbuViewType → view_mode mapping
    /// - CameraState → camera position and zoom
    /// - ViewportFilters → type_filter
    /// - confidence_threshold → (stored for HUD rendering)
    ///
    /// Called when UI fetches session context and finds viewport_state.
    pub fn apply_viewport_state(&mut self, state: &ViewportState) {
        // 1. Apply focus state
        match &state.focus.state {
            ViewportFocusState::None => {
                self.input_state.clear_focus();
            }
            ViewportFocusState::CbuContainer { enhance_level, .. } => {
                // CBU container focus - set enhance level, no specific node focus
                self.set_enhance_level(*enhance_level);
            }
            ViewportFocusState::CbuEntity {
                entity,
                entity_enhance,
                container_enhance,
                ..
            } => {
                // Focus on specific entity
                self.focus_node(&entity.id.to_string());
                // Use the higher of entity or container enhance
                self.set_enhance_level((*entity_enhance).max(*container_enhance));
            }
            ViewportFocusState::CbuProductService {
                target,
                target_enhance,
                ..
            } => {
                // Focus on product/service - extract ID from target
                let target_id = match target {
                    ob_poc_types::viewport::ProductServiceRef::Product { id } => id.to_string(),
                    ob_poc_types::viewport::ProductServiceRef::Service { id } => id.to_string(),
                    ob_poc_types::viewport::ProductServiceRef::ServiceResource { id } => {
                        id.to_string()
                    }
                };
                self.focus_node(&target_id);
                self.set_enhance_level(*target_enhance);
            }
            ViewportFocusState::InstrumentMatrix { matrix_enhance, .. } => {
                // Instrument matrix view - set enhance, no node focus
                self.set_enhance_level(*matrix_enhance);
            }
            ViewportFocusState::InstrumentType { type_enhance, .. } => {
                self.set_enhance_level(*type_enhance);
            }
            ViewportFocusState::ConfigNode { node_enhance, .. } => {
                self.set_enhance_level(*node_enhance);
            }
            ViewportFocusState::BoardControl { enhance_level, .. } => {
                // Board control view - switch view mode and set enhance level
                self.view_mode = ViewMode;
                self.set_enhance_level(*enhance_level);
            }
        }

        // 2. Apply view type → view mode mapping
        let view_mode = match state.view_type {
            CbuViewType::Structure | CbuViewType::Ownership => ViewMode,
            CbuViewType::Accounts => ViewMode,
            CbuViewType::Compliance => ViewMode,
            CbuViewType::Geographic => ViewMode,
            CbuViewType::Temporal => ViewMode,
            CbuViewType::Instruments => ViewMode,
        };
        if self.view_mode != view_mode {
            self.view_mode = view_mode;
            // Note: view mode change may need server refetch - handled by caller
        }

        // 3. Apply camera state
        // Only apply if significantly different to avoid jitter
        let camera_pos = egui::Pos2::new(state.camera.x, state.camera.y);
        let current_pos = self.camera.center();
        let pos_diff = (camera_pos - current_pos).length();
        let zoom_diff = (state.camera.zoom - self.camera.zoom()).abs();

        if pos_diff > 1.0 || zoom_diff > 0.01 {
            self.camera.fly_to(camera_pos);
            self.camera.zoom_to(state.camera.zoom);
        }

        // 4. Apply filters
        // Entity type filter - take first if multiple
        if let Some(ref entity_types) = state.filters.entity_types {
            if let Some(first_type) = entity_types.first() {
                let type_str = match first_type {
                    ob_poc_types::viewport::ConcreteEntityType::Company => "LIMITED_COMPANY",
                    ob_poc_types::viewport::ConcreteEntityType::Partnership => "PARTNERSHIP",
                    ob_poc_types::viewport::ConcreteEntityType::Trust => "TRUST",
                    ob_poc_types::viewport::ConcreteEntityType::Person => "PROPER_PERSON",
                };
                self.set_type_filter(Some(type_str.to_string()));
            }
        } else {
            self.set_type_filter(None);
        }

        // 5. Store confidence threshold in viewport render state for HUD
        self.viewport_render_state
            .set_confidence_threshold(state.confidence_threshold);

        // 6. Store the full viewport state for HUD rendering
        self.viewport_state = Some(state.clone());

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "apply_viewport_state: focus={:?}, view_type={:?}, zoom={:.2}",
                state.focus.state, state.view_type, state.camera.zoom
            )
            .into(),
        );
    }

    /// Get reference to the viewport render state (for HUD rendering)
    pub fn viewport_render_state(&self) -> &ViewportRenderState {
        &self.viewport_render_state
    }
}

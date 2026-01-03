//! State Management for ob-poc-ui
//!
//! This module follows the MANDATORY patterns from CLAUDE.md:
//!
//! 1. SERVER DATA: Fetched via API, NEVER modified locally
//! 2. UI-ONLY STATE: TextBuffers (drafts), view_mode, selected_entity
//! 3. ASYNC COORDINATION: Arc<Mutex<AsyncState>> for spawn_local results
//!
//! ANTI-PATTERNS (NEVER DO):
//! - Local Vec<Message> that mirrors server
//! - is_dirty flags for sync logic
//! - Callbacks for widget events (use return values)
//! - Caching entities locally

use crate::panels::ContainerBrowseState;
use crate::tokens::TokenRegistry;
use ob_poc_graph::{
    CbuGraphData, CbuGraphWidget, EntityTypeOntology, TaxonomyState, TradingMatrix,
    TradingMatrixNode, TradingMatrixState, ViewMode,
};
use ob_poc_types::{
    CbuSummary, ExecuteResponse, ResolutionSearchResponse, ResolutionSessionResponse,
    SessionContext, SessionStateResponse, ValidateDslResponse,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// =============================================================================
// LOCAL TYPES (not in ob-poc-types because they're UI-specific)
// =============================================================================

/// Chat message for display
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Message role (user or agent)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Agent,
}

// =============================================================================
// SERVER DATA (fetched via API, NEVER modified locally)
// =============================================================================

/// Main application state
///
/// The state is split into three categories:
/// 1. Server data - fetched from API, treated as read-only
/// 2. UI-only state - ephemeral, not persisted
/// 3. Async coordination - for spawn_local results
pub struct AppState {
    // =========================================================================
    // SERVER DATA (fetched via API, NEVER modified locally)
    // =========================================================================
    /// Current session (includes message_count, active_cbu, bindings)
    /// Set via refetch_session(), never modified directly
    pub session: Option<SessionStateResponse>,

    /// Session ID (persisted to localStorage)
    pub session_id: Option<Uuid>,

    /// Graph data for current CBU
    /// Set via fetch_graph() in update() loop, never modified directly
    pub graph_data: Option<CbuGraphData>,

    /// Last validation errors (empty = valid)
    /// These are plain strings from the validation API
    pub validation_result: Option<ValidateDslResponse>,

    /// Last execution result
    pub execution: Option<ExecuteResponse>,

    /// Resolution session (entity resolution workflow)
    /// Set via start_resolution(), never modified directly
    pub resolution: Option<ResolutionSessionResponse>,

    /// Chat messages (accumulated from ChatResponse)
    pub messages: Vec<ChatMessage>,

    /// Available CBUs for selector dropdown
    pub cbu_list: Vec<CbuSummary>,

    /// Session context (CBU info, linked entities, symbols)
    /// Set via fetch_session_context(), never modified directly
    pub session_context: Option<SessionContext>,

    /// Trading matrix data (hierarchical custody configuration)
    /// Set via fetch_trading_matrix(), never modified directly
    pub trading_matrix: Option<TradingMatrix>,

    // =========================================================================
    // UI-ONLY STATE (ephemeral, not persisted)
    // =========================================================================
    /// Text being edited (drafts before submission)
    /// This is the ONLY local mutable state for user input
    pub buffers: TextBuffers,

    /// Current view mode for graph
    pub view_mode: ViewMode,

    /// Panel visibility and layout
    pub panels: PanelState,

    /// Selected entity in graph (for detail panel)
    pub selected_entity_id: Option<String>,

    /// Resolution panel UI state
    pub resolution_ui: ResolutionPanelUi,

    /// CBU search modal UI state
    pub cbu_search_ui: CbuSearchUi,

    /// Container browse panel state (slide-in panel for browsing container contents)
    pub container_browse: ContainerBrowseState,

    /// Token registry for visual configuration (loaded from YAML)
    pub token_registry: TokenRegistry,

    /// Graph widget (owns camera, input state - rendering only)
    pub graph_widget: CbuGraphWidget,

    /// Entity type ontology (hierarchical type classification)
    pub entity_ontology: EntityTypeOntology,

    /// Taxonomy browser state (expand/collapse, selection)
    pub taxonomy_state: TaxonomyState,

    /// Taxonomy navigation breadcrumbs (from server TaxonomyStack)
    /// Each entry is (level_label, type_code) for display
    pub taxonomy_breadcrumbs: Vec<(String, String)>,

    /// Active type filter (None = show all, Some = filter to type)
    pub type_filter: Option<String>,

    /// Trading matrix browser state (expand/collapse, selection)
    pub trading_matrix_state: TradingMatrixState,

    /// Currently selected trading matrix node (for detail panel)
    pub selected_matrix_node: Option<TradingMatrixNode>,

    // =========================================================================
    // ASYNC COORDINATION
    // =========================================================================
    /// Shared state for async operations (written by spawn_local, read by update)
    pub async_state: Arc<Mutex<AsyncState>>,

    /// egui context for triggering repaints from async callbacks
    pub ctx: Option<egui::Context>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            // Server data - starts empty, fetched on init
            session: None,
            session_id: None,
            graph_data: None,
            validation_result: None,
            execution: None,
            resolution: None,
            messages: Vec::new(),
            cbu_list: Vec::new(),
            session_context: None,
            trading_matrix: None,

            // UI-only state
            buffers: TextBuffers::default(),
            view_mode: ViewMode::KycUbo,
            panels: PanelState::default(),
            selected_entity_id: None,
            resolution_ui: ResolutionPanelUi::default(),
            cbu_search_ui: CbuSearchUi::default(),
            container_browse: ContainerBrowseState::default(),
            token_registry: TokenRegistry::load_defaults().unwrap_or_else(|e| {
                web_sys::console::warn_1(
                    &format!("Failed to load token config: {}, using defaults", e).into(),
                );
                TokenRegistry::new()
            }),
            graph_widget: CbuGraphWidget::new(),
            entity_ontology: EntityTypeOntology::new(),
            taxonomy_state: TaxonomyState::new(),
            taxonomy_breadcrumbs: Vec::new(),
            type_filter: None,
            trading_matrix_state: TradingMatrixState::new(),
            selected_matrix_node: None,

            // Async coordination
            async_state: Arc::new(Mutex::new(AsyncState::default())),
            ctx: None,
        }
    }
}

// =============================================================================
// TEXT BUFFERS - The ONLY local mutable state for user input
// =============================================================================

/// Text buffers for user input
///
/// These are the ONLY pieces of state that the UI "owns".
/// Everything else comes from the server.
#[derive(Default, Clone)]
pub struct TextBuffers {
    /// Chat message being composed
    pub chat_input: String,

    /// DSL source being edited
    pub dsl_editor: String,

    /// Entity search query
    pub entity_search: String,

    /// DSL editor dirty flag
    /// Used ONLY for "unsaved changes" warning, NOT for sync logic
    pub dsl_dirty: bool,
}

// =============================================================================
// PANEL STATE - UI layout configuration
// =============================================================================

/// Panel visibility and layout state
#[derive(Clone)]
pub struct PanelState {
    pub show_chat: bool,
    pub show_dsl_editor: bool,
    pub show_results: bool,
    pub show_ast: bool,
    pub show_entity_detail: bool,
    pub show_debug: bool, // F1 toggle for debug window
    pub layout: LayoutMode,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            show_chat: true,
            show_dsl_editor: true,
            show_results: false,
            show_ast: true, // Default to AST view
            show_entity_detail: false,
            show_debug: false,
            layout: LayoutMode::FourPanel,
        }
    }
}

/// Layout mode for panel arrangement
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    #[default]
    FourPanel, // 2x2 grid
    EditorFocus,   // Large DSL editor + small panels
    GraphFocus,    // Large graph + small panels
    GraphFullSize, // Graph only, full window
}

// =============================================================================
// RESOLUTION PANEL UI STATE
// =============================================================================

/// Resolution panel UI-only state (not persisted, not synced)
#[derive(Default, Clone)]
pub struct ResolutionPanelUi {
    /// Currently selected ref_id for resolution
    pub selected_ref_id: Option<String>,
    /// Search query for current ref
    pub search_query: String,
    /// Search results from last search
    pub search_results: Option<ResolutionSearchResponse>,
    /// Expanded discriminator section
    pub show_discriminators: bool,
    /// Discriminator values being edited
    pub discriminator_values: std::collections::HashMap<String, String>,
    /// Show resolution panel (modal/overlay)
    pub show_panel: bool,
}

/// CBU search modal UI state
#[derive(Default, Clone)]
pub struct CbuSearchUi {
    /// Whether the search modal is open
    pub open: bool,
    /// Whether the modal just opened (for auto-focus)
    pub just_opened: bool,
    /// Current search query
    pub query: String,
    /// Search results (from EntityGateway fuzzy search)
    pub results: Option<crate::api::CbuSearchResponse>,
    /// Whether a search is in progress
    pub searching: bool,
}

// =============================================================================
// ASYNC STATE - Coordination for spawn_local operations
// =============================================================================

/// Async operation results
///
/// Written by spawn_local callbacks, read by update() loop.
/// This is the ONLY place where async results are stored.
///
/// Pattern:
/// 1. spawn_local sets loading_* = true
/// 2. spawn_local fetches from server
/// 3. spawn_local sets pending_* = Some(result), loading_* = false
/// 4. spawn_local calls ctx.request_repaint()
/// 5. update() calls process_async_results() which moves pending_* to AppState
#[derive(Default)]
pub struct AsyncState {
    // Pending results from async operations
    pub pending_session: Option<Result<SessionStateResponse, String>>,
    pub pending_session_id: Option<Uuid>,
    pub pending_graph: Option<Result<CbuGraphData, String>>,
    pub pending_validation: Option<Result<ValidateDslResponse, String>>,
    pub pending_execution: Option<Result<ExecuteResponse, String>>,
    pub pending_cbu_list: Option<Result<Vec<CbuSummary>, String>>,
    pub pending_chat: Option<Result<ChatMessage, String>>,
    pub pending_resolution: Option<Result<ResolutionSessionResponse, String>>,
    pub pending_resolution_search: Option<Result<ResolutionSearchResponse, String>>,
    pub pending_cbu_search: Option<Result<crate::api::CbuSearchResponse, String>>,
    pub pending_session_context: Option<Result<SessionContext, String>>,
    pub pending_trading_matrix: Option<Result<TradingMatrix, String>>,

    // Command triggers (from agent commands)
    pub pending_execute: Option<Uuid>, // Session ID to execute

    // Graph filter commands (from agent chat)
    pub pending_filter_by_type: Option<Vec<String>>, // Type codes to filter
    pub pending_highlight_type: Option<String>,      // Type code to highlight
    pub pending_clear_filter: bool,                  // Clear all filters
    pub pending_view_mode: Option<String>,           // View mode to set

    // Esper-style navigation commands (from agent chat)
    pub pending_zoom_in: Option<f32>, // Zoom in with optional factor
    pub pending_zoom_out: Option<f32>, // Zoom out with optional factor
    pub pending_zoom_fit: bool,       // Zoom to fit all content
    pub pending_zoom_to: Option<f32>, // Zoom to specific level
    pub pending_pan: Option<(ob_poc_types::PanDirection, Option<f32>)>, // Pan direction + amount
    pub pending_center: bool,         // Center view
    pub pending_stop: bool,           // Stop all animation
    pub pending_focus_entity: Option<String>, // Focus on entity by ID
    pub pending_reset_layout: bool,   // Reset layout to default

    // Extended Esper 3D/Multi-dimensional commands
    // Scale navigation (astronomical metaphor)
    pub pending_scale_universe: bool,
    pub pending_scale_galaxy: Option<Option<String>>, // segment filter
    pub pending_scale_system: Option<Option<String>>, // cbu_id
    pub pending_scale_planet: Option<Option<String>>, // entity_id
    pub pending_scale_surface: bool,
    pub pending_scale_core: bool,

    // Depth navigation (Z-axis)
    pub pending_drill_through: bool,
    pub pending_surface_return: bool,
    pub pending_xray: bool,
    pub pending_peel: bool,
    pub pending_cross_section: bool,
    pub pending_depth_indicator: bool,

    // Orbital navigation
    pub pending_orbit: Option<Option<String>>, // entity_id
    pub pending_rotate_layer: Option<String>,  // layer name
    pub pending_flip: bool,
    pub pending_tilt: Option<String>, // dimension

    // Temporal navigation
    pub pending_time_rewind: Option<Option<String>>, // target_date
    pub pending_time_play: Option<(Option<String>, Option<String>)>, // from, to
    pub pending_time_freeze: bool,
    pub pending_time_slice: Option<(Option<String>, Option<String>)>, // date1, date2
    pub pending_time_trail: Option<Option<String>>,                   // entity_id

    // Investigation patterns
    pub pending_follow_money: Option<Option<String>>, // from_entity
    pub pending_who_controls: Option<Option<String>>, // entity_id
    pub pending_illuminate: Option<String>,           // aspect
    pub pending_shadow: bool,
    pub pending_red_flag_scan: bool,
    pub pending_black_hole: bool,

    // Context intentions
    pub pending_context: Option<String>, // "review", "investigation", "onboarding", etc.

    // Taxonomy navigation (fractal navigation via TaxonomyStack on server)
    pub pending_taxonomy_zoom_in: Option<String>, // type_code to zoom into
    pub pending_taxonomy_zoom_out: bool,          // zoom out one level
    pub pending_taxonomy_back_to: Option<usize>,  // jump to specific breadcrumb index
    pub pending_taxonomy_breadcrumbs: bool,       // request current breadcrumbs

    // Taxonomy API responses (from server calls)
    pub pending_taxonomy_breadcrumbs_response:
        Option<Result<crate::api::TaxonomyBreadcrumbsResponse, String>>,
    pub pending_taxonomy_zoom_response: Option<Result<crate::api::TaxonomyZoomResponse, String>>,
    pub loading_taxonomy: bool, // loading flag for taxonomy operations

    // State change flags (set by actions, processed centrally in update loop)
    // These are checked ONCE in update() AFTER process_async_results()
    pub needs_graph_refetch: bool, // CBU selected or view mode changed
    pub needs_context_refetch: bool, // CBU selected, fetch session context
    pub needs_trading_matrix_refetch: bool, // Trading view mode selected
    pub pending_cbu_id: Option<Uuid>, // CBU to fetch graph for (set by select_cbu)

    // Execution tracking - prevents repeated refetch
    pub execution_handled: bool,

    // Loading flags (for spinners)
    pub loading_session: bool,
    pub loading_graph: bool,
    pub loading_chat: bool,
    pub executing: bool,
    pub loading_resolution: bool,
    pub searching_resolution: bool,
    pub loading_session_context: bool,
    pub loading_trading_matrix: bool,

    // Chat focus tracking - set when chat completes to refocus input
    pub chat_just_finished: bool,

    // Error display
    pub last_error: Option<String>,
}

// =============================================================================
// STATE PROCESSING - Called at start of each frame
// =============================================================================

impl AppState {
    /// Process pending async results at the start of each frame
    ///
    /// This is the ONLY place where async results flow into AppState.
    /// Pattern: pending_* -> AppState field, then trigger dependent refetches.
    pub fn process_async_results(&mut self) {
        let mut state = match self.async_state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        // Process new session creation
        if let Some(session_id) = state.pending_session_id.take() {
            self.session_id = Some(session_id);
            // Don't refetch here - we'll do it after dropping the lock
        }

        // Process session fetch
        if let Some(result) = state.pending_session.take() {
            state.loading_session = false;
            match result {
                Ok(session) => {
                    // Sync DSL editor from combined_dsl if server has content and we're not dirty
                    if !self.buffers.dsl_dirty {
                        // Try combined_dsl first (from session state), then dsl_source
                        if let Some(ref combined) = session.combined_dsl {
                            if let Some(dsl_str) = combined.as_str() {
                                if !dsl_str.is_empty() {
                                    self.buffers.dsl_editor = dsl_str.to_string();
                                }
                            }
                        } else if let Some(ref dsl) = session.dsl_source {
                            self.buffers.dsl_editor = dsl.clone();
                        }
                    }
                    self.session = Some(session);
                }
                Err(e) => {
                    state.last_error = Some(format!("Session fetch failed: {}", e));
                }
            }
        }

        // Process graph fetch
        if let Some(result) = state.pending_graph.take() {
            state.loading_graph = false;
            match result {
                Ok(data) => {
                    web_sys::console::log_1(
                        &format!(
                            "process_async_results: graph received {} nodes, {} edges",
                            data.nodes.len(),
                            data.edges.len()
                        )
                        .into(),
                    );
                    self.graph_widget.set_data(data.clone());
                    self.graph_data = Some(data);

                    // Populate ontology counts from graph data
                    if let Some(layout_graph) = self.graph_widget.get_layout_graph() {
                        self.entity_ontology.populate_counts(layout_graph);
                        web_sys::console::log_1(
                            &format!(
                                "process_async_results: ontology populated, root count={}",
                                self.entity_ontology.root.total_count
                            )
                            .into(),
                        );
                    }
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("Graph fetch failed: {}", e).into());
                    state.last_error = Some(format!("Graph fetch failed: {}", e));
                }
            }
        }

        // Process validation
        if let Some(result) = state.pending_validation.take() {
            match result {
                Ok(response) => self.validation_result = Some(response),
                Err(e) => state.last_error = Some(e),
            }
        }

        // Process chat response
        if let Some(result) = state.pending_chat.take() {
            state.loading_chat = false;
            state.chat_just_finished = true; // Trigger focus back to input
            match result {
                Ok(msg) => self.messages.push(msg),
                Err(e) => state.last_error = Some(e),
            }
        }

        // Process execution - triggers refetch of dependent data
        if let Some(result) = state.pending_execution.take() {
            state.executing = false;
            match result {
                Ok(execution) => {
                    self.execution = Some(execution);
                    // Note: graph refetch triggered via needs_graph_refetch flag
                    // by the caller after this returns, since they need &mut self
                }
                Err(e) => state.last_error = Some(e),
            }
        }

        // Process CBU list
        if let Some(result) = state.pending_cbu_list.take() {
            match result {
                Ok(list) => self.cbu_list = list,
                Err(e) => state.last_error = Some(e),
            }
        }

        // Process resolution session
        if let Some(result) = state.pending_resolution.take() {
            state.loading_resolution = false;
            match result {
                Ok(resolution) => {
                    // Auto-show panel when resolution starts
                    if !resolution.unresolved.is_empty() {
                        self.resolution_ui.show_panel = true;
                    }
                    self.resolution = Some(resolution);
                }
                Err(e) => state.last_error = Some(format!("Resolution failed: {}", e)),
            }
        }

        // Process resolution search results
        if let Some(result) = state.pending_resolution_search.take() {
            state.searching_resolution = false;
            match result {
                Ok(search_result) => {
                    self.resolution_ui.search_results = Some(search_result);
                }
                Err(e) => state.last_error = Some(format!("Resolution search failed: {}", e)),
            }
        }

        // Process CBU search results
        if let Some(result) = state.pending_cbu_search.take() {
            self.cbu_search_ui.searching = false;
            match result {
                Ok(search_result) => {
                    self.cbu_search_ui.results = Some(search_result);
                }
                Err(e) => state.last_error = Some(format!("CBU search failed: {}", e)),
            }
        }

        // Process session context
        if let Some(result) = state.pending_session_context.take() {
            state.loading_session_context = false;
            match result {
                Ok(context) => {
                    self.session_context = Some(context);
                }
                Err(e) => state.last_error = Some(format!("Session context fetch failed: {}", e)),
            }
        }

        // Process trading matrix
        if let Some(result) = state.pending_trading_matrix.take() {
            state.loading_trading_matrix = false;
            match result {
                Ok(matrix) => {
                    // Expand first level by default for better UX
                    self.trading_matrix_state.expand_first_level(&matrix);
                    self.trading_matrix = Some(matrix);
                }
                Err(e) => state.last_error = Some(format!("Trading matrix fetch failed: {}", e)),
            }
        }

        // Process taxonomy breadcrumbs response
        if let Some(result) = state.pending_taxonomy_breadcrumbs_response.take() {
            state.loading_taxonomy = false;
            match result {
                Ok(response) => {
                    self.taxonomy_breadcrumbs = response
                        .breadcrumbs
                        .into_iter()
                        .map(|b| (b.label, b.type_code))
                        .collect();
                }
                Err(e) => {
                    state.last_error = Some(format!("Taxonomy breadcrumbs fetch failed: {}", e))
                }
            }
        }

        // Process taxonomy zoom response
        if let Some(result) = state.pending_taxonomy_zoom_response.take() {
            state.loading_taxonomy = false;
            match result {
                Ok(response) => {
                    if response.success {
                        self.taxonomy_breadcrumbs = response
                            .breadcrumbs
                            .into_iter()
                            .map(|b| (b.label, b.type_code))
                            .collect();
                    } else if let Some(error) = response.error {
                        state.last_error = Some(format!("Taxonomy navigation failed: {}", error));
                    }
                }
                Err(e) => state.last_error = Some(format!("Taxonomy navigation failed: {}", e)),
            }
        }
    }

    /// Check if any async operation is in progress
    pub fn is_loading(&self) -> bool {
        if let Ok(state) = self.async_state.lock() {
            state.loading_session || state.loading_graph || state.loading_chat || state.executing
        } else {
            false
        }
    }

    /// Check if execution just completed (for triggering refetches)
    pub fn execution_just_completed(&self) -> bool {
        if let Ok(state) = self.async_state.lock() {
            !state.executing && self.execution.is_some()
        } else {
            false
        }
    }

    /// Check if an execute command is pending and return the session ID
    pub fn take_pending_execute(&self) -> Option<Uuid> {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_execute.take();
            if pending.is_some() {
                web_sys::console::log_1(
                    &format!(
                        "take_pending_execute: found pending execute for {:?}",
                        pending
                    )
                    .into(),
                );
            }
            pending
        } else {
            None
        }
    }

    /// Check if graph refetch is needed and return the CBU ID to fetch
    /// Called ONCE per frame in update() - the single central place for graph fetches
    pub fn take_pending_graph_refetch(&self) -> Option<Uuid> {
        let Ok(mut state) = self.async_state.lock() else {
            return None;
        };

        if !state.needs_graph_refetch {
            return None;
        }

        // Clear the flag
        state.needs_graph_refetch = false;

        // Use pending_cbu_id if set (from select_cbu), otherwise use session_id
        if let Some(cbu_id) = state.pending_cbu_id.take() {
            return Some(cbu_id);
        }

        // Fall back to current session_id (for view mode changes, execution complete)
        drop(state); // Release lock before accessing self
        self.session_id
    }

    /// Check if trading matrix refetch is needed and return the CBU ID to fetch
    /// Called ONCE per frame in update() - the single central place for trading matrix fetches
    pub fn take_pending_trading_matrix_refetch(&self) -> Option<Uuid> {
        let Ok(mut state) = self.async_state.lock() else {
            return None;
        };

        if !state.needs_trading_matrix_refetch {
            return None;
        }

        // Clear the flag
        state.needs_trading_matrix_refetch = false;

        // Use pending_cbu_id if set (from select_cbu), otherwise use session_id
        if let Some(cbu_id) = state.pending_cbu_id.clone() {
            return Some(cbu_id);
        }

        // Fall back to current session_id (for view mode changes)
        drop(state); // Release lock before accessing self
        self.session_id
    }

    /// Check if context refetch is needed and return true if so
    /// Called ONCE per frame in update() - the single central place for context fetches
    pub fn take_pending_context_refetch(&self) -> bool {
        let Ok(mut state) = self.async_state.lock() else {
            return false;
        };

        if !state.needs_context_refetch {
            return false;
        }

        // Clear the flag
        state.needs_context_refetch = false;
        true
    }

    /// Check if a filter by type command is pending
    pub fn take_pending_filter_by_type(&self) -> Option<Vec<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_filter_by_type.take()
        } else {
            None
        }
    }

    /// Check if a highlight type command is pending
    pub fn take_pending_highlight_type(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_highlight_type.take()
        } else {
            None
        }
    }

    /// Check if a clear filter command is pending
    pub fn take_pending_clear_filter(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_clear_filter;
            state.pending_clear_filter = false;
            pending
        } else {
            false
        }
    }

    /// Check if a view mode change command is pending
    pub fn take_pending_view_mode(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_view_mode.take()
        } else {
            None
        }
    }

    // =========================================================================
    // ESPER-STYLE NAVIGATION COMMAND HANDLERS
    // =========================================================================

    /// Check if a zoom in command is pending
    pub fn take_pending_zoom_in(&self) -> Option<f32> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_zoom_in.take()
        } else {
            None
        }
    }

    /// Check if a zoom out command is pending
    pub fn take_pending_zoom_out(&self) -> Option<f32> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_zoom_out.take()
        } else {
            None
        }
    }

    /// Check if a zoom fit command is pending
    pub fn take_pending_zoom_fit(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_zoom_fit;
            state.pending_zoom_fit = false;
            pending
        } else {
            false
        }
    }

    /// Check if a zoom to level command is pending
    pub fn take_pending_zoom_to(&self) -> Option<f32> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_zoom_to.take()
        } else {
            None
        }
    }

    /// Check if a pan command is pending
    pub fn take_pending_pan(&self) -> Option<(ob_poc_types::PanDirection, Option<f32>)> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_pan.take()
        } else {
            None
        }
    }

    /// Check if a center command is pending
    pub fn take_pending_center(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_center;
            state.pending_center = false;
            pending
        } else {
            false
        }
    }

    /// Check if a stop command is pending
    pub fn take_pending_stop(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_stop;
            state.pending_stop = false;
            pending
        } else {
            false
        }
    }

    /// Check if a focus entity command is pending
    pub fn take_pending_focus_entity(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_focus_entity.take()
        } else {
            None
        }
    }

    /// Check if a reset layout command is pending
    pub fn take_pending_reset_layout(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_reset_layout;
            state.pending_reset_layout = false;
            pending
        } else {
            false
        }
    }

    // =========================================================================
    // Extended Esper 3D Navigation - Take Methods
    // =========================================================================

    // Scale navigation
    pub fn take_pending_scale_universe(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_scale_universe;
            state.pending_scale_universe = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_scale_galaxy(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_scale_galaxy.take()
        } else {
            None
        }
    }

    pub fn take_pending_scale_system(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_scale_system.take()
        } else {
            None
        }
    }

    pub fn take_pending_scale_planet(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_scale_planet.take()
        } else {
            None
        }
    }

    pub fn take_pending_scale_surface(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_scale_surface;
            state.pending_scale_surface = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_scale_core(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_scale_core;
            state.pending_scale_core = false;
            pending
        } else {
            false
        }
    }

    // Depth navigation
    pub fn take_pending_drill_through(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_drill_through;
            state.pending_drill_through = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_surface_return(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_surface_return;
            state.pending_surface_return = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_xray(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_xray;
            state.pending_xray = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_peel(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_peel;
            state.pending_peel = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_cross_section(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_cross_section;
            state.pending_cross_section = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_depth_indicator(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_depth_indicator;
            state.pending_depth_indicator = false;
            pending
        } else {
            false
        }
    }

    // Orbital navigation
    pub fn take_pending_orbit(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_orbit.take()
        } else {
            None
        }
    }

    pub fn take_pending_rotate_layer(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_rotate_layer.take()
        } else {
            None
        }
    }

    pub fn take_pending_flip(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_flip;
            state.pending_flip = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_tilt(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_tilt.take()
        } else {
            None
        }
    }

    // Temporal navigation
    pub fn take_pending_time_rewind(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_time_rewind.take()
        } else {
            None
        }
    }

    pub fn take_pending_time_play(&self) -> Option<(Option<String>, Option<String>)> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_time_play.take()
        } else {
            None
        }
    }

    pub fn take_pending_time_freeze(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_time_freeze;
            state.pending_time_freeze = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_time_slice(&self) -> Option<(Option<String>, Option<String>)> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_time_slice.take()
        } else {
            None
        }
    }

    pub fn take_pending_time_trail(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_time_trail.take()
        } else {
            None
        }
    }

    // Investigation patterns
    pub fn take_pending_follow_money(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_follow_money.take()
        } else {
            None
        }
    }

    pub fn take_pending_who_controls(&self) -> Option<Option<String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_who_controls.take()
        } else {
            None
        }
    }

    pub fn take_pending_illuminate(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_illuminate.take()
        } else {
            None
        }
    }

    pub fn take_pending_shadow(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_shadow;
            state.pending_shadow = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_red_flag_scan(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_red_flag_scan;
            state.pending_red_flag_scan = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_black_hole(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_black_hole;
            state.pending_black_hole = false;
            pending
        } else {
            false
        }
    }

    // Context intentions
    pub fn take_pending_context(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_context.take()
        } else {
            None
        }
    }

    // =========================================================================
    // Taxonomy Navigation - Take Methods
    // =========================================================================

    /// Check if a taxonomy zoom-in command is pending
    pub fn take_pending_taxonomy_zoom_in(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_taxonomy_zoom_in.take()
        } else {
            None
        }
    }

    /// Check if a taxonomy zoom-out command is pending
    pub fn take_pending_taxonomy_zoom_out(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_taxonomy_zoom_out;
            state.pending_taxonomy_zoom_out = false;
            pending
        } else {
            false
        }
    }

    /// Check if a taxonomy back-to command is pending
    pub fn take_pending_taxonomy_back_to(&self) -> Option<usize> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_taxonomy_back_to.take()
        } else {
            None
        }
    }

    /// Check if a taxonomy breadcrumbs request is pending
    pub fn take_pending_taxonomy_breadcrumbs(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_taxonomy_breadcrumbs;
            state.pending_taxonomy_breadcrumbs = false;
            pending
        } else {
            false
        }
    }

    /// Add a user message to the chat history
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Check if execution just completed and needs handling (only returns true once per execution)
    pub fn should_handle_execution_complete(&mut self) -> bool {
        if self.execution.is_none() {
            return false;
        }

        let should_handle = {
            let Ok(mut state) = self.async_state.lock() else {
                return false;
            };
            // Only handle if: not currently executing, no pending result, and not yet handled
            if !state.executing && state.pending_execution.is_none() && !state.execution_handled {
                state.execution_handled = true;
                true
            } else {
                false
            }
        };

        should_handle
    }

    /// Get DSL source from session or editor buffer
    pub fn get_dsl_source(&self) -> Option<String> {
        // First try the editor buffer if it has content
        if !self.buffers.dsl_editor.trim().is_empty() {
            return Some(self.buffers.dsl_editor.clone());
        }

        // Then try session's dsl_source
        if let Some(ref session) = self.session {
            if let Some(ref dsl) = session.dsl_source {
                if !dsl.trim().is_empty() {
                    return Some(dsl.clone());
                }
            }
            // Try combined_dsl
            if let Some(ref combined) = session.combined_dsl {
                if let Some(s) = combined.as_str() {
                    if !s.trim().is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
            // Try assembled_dsl.combined
            if let Some(ref assembled) = session.assembled_dsl {
                if let Some(obj) = assembled.as_object() {
                    if let Some(combined) = obj.get("combined").and_then(|v| v.as_str()) {
                        if !combined.trim().is_empty() {
                            return Some(combined.to_string());
                        }
                    }
                }
            }
        }

        None
    }

    /// Fetch session context for the current session
    /// Called when a CBU is selected or context may have changed
    pub fn fetch_session_context(&mut self, session_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_session_context = true;
        }

        spawn_local(async move {
            let result = api::get_session_context(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_session_context = Some(result);
                state.loading_session_context = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Fetch trading matrix for a CBU
    /// Called when a CBU is selected and Trading view mode is active
    pub fn fetch_trading_matrix(&mut self, cbu_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_trading_matrix = true;
        }

        spawn_local(async move {
            let result = api::get_trading_matrix(cbu_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_trading_matrix = Some(result);
                state.loading_trading_matrix = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Execute DSL with explicit content (for agent Execute command)
    pub fn execute_dsl_with_content(&mut self, session_id: Uuid, dsl: String) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        web_sys::console::log_1(
            &format!(
                "execute_dsl_with_content: session={} dsl_len={}",
                session_id,
                dsl.len()
            )
            .into(),
        );

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.executing = true;
            state.execution_handled = false;
        }

        spawn_local(async move {
            let result = api::execute_dsl(session_id, &dsl).await;
            if let Err(ref e) = result {
                web_sys::console::error_1(
                    &format!("execute_dsl_with_content: error: {}", e).into(),
                );
            }
            if let Ok(mut state) = async_state.lock() {
                state.pending_execution = Some(result);
                state.executing = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Parse view mode from string (used by agent commands)
pub fn parse_view_mode(s: &str) -> Option<ViewMode> {
    ViewMode::from_str(s)
}

// =============================================================================
// TAXONOMY NAVIGATION METHODS
// =============================================================================

impl AppState {
    /// Fetch current taxonomy breadcrumbs from server
    pub fn fetch_taxonomy_breadcrumbs(&mut self, session_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_taxonomy = true;
        }

        spawn_local(async move {
            let result = api::get_taxonomy_breadcrumbs(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_taxonomy_breadcrumbs_response = Some(result);
                state.loading_taxonomy = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Zoom into a type (push onto taxonomy stack on server)
    pub fn taxonomy_zoom_in(&mut self, session_id: Uuid, type_code: String) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_taxonomy = true;
        }

        spawn_local(async move {
            let result = api::taxonomy_zoom_in(session_id, &type_code).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_taxonomy_zoom_response = Some(result);
                state.loading_taxonomy = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Zoom out one level (pop from taxonomy stack on server)
    pub fn taxonomy_zoom_out(&mut self, session_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_taxonomy = true;
        }

        spawn_local(async move {
            let result = api::taxonomy_zoom_out(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_taxonomy_zoom_response = Some(result);
                state.loading_taxonomy = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Jump to a specific breadcrumb level (back-to on server)
    pub fn taxonomy_back_to(&mut self, session_id: Uuid, level_index: usize) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_taxonomy = true;
        }

        spawn_local(async move {
            let result = api::taxonomy_back_to(session_id, level_index).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_taxonomy_zoom_response = Some(result);
                state.loading_taxonomy = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }
}

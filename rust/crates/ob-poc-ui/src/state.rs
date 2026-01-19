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

use crate::api::ScopeGraphData;
use crate::panels::ContainerBrowseState;
use crate::tokens::TokenRegistry;
use ob_poc_graph::{
    CbuGraphData, CbuGraphWidget, EntityTypeOntology, GalaxyView, ServiceTaxonomy,
    ServiceTaxonomyState, TaxonomyState, TradingMatrix, TradingMatrixNode, TradingMatrixState,
    ViewMode,
};
use ob_poc_types::investor_register::{
    BreakdownDimension, InvestorFilters, InvestorListResponse, InvestorRegisterView,
};
use ob_poc_types::{
    galaxy::{NavigationScope, UniverseGraph, ViewLevel},
    resolution::{DiscriminatorField, ResolutionModeHint, SearchKeyField, UnresolvedRefResponse},
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

/// Source of a navigation command (for audit trail)
#[derive(Clone, Debug, PartialEq)]
pub enum NavigationSource {
    /// Voice command with transcript and confidence
    Voice { transcript: String, confidence: f32 },
    /// Keyboard shortcut
    Keyboard,
    /// Mouse/touch gesture
    Gesture,
    /// UI button/widget click
    Widget,
    /// Programmatic (e.g., from agent)
    Programmatic,
}

/// Entry in the navigation log (for audit/replay)
#[derive(Clone, Debug)]
pub struct NavigationLogEntry {
    /// DSL representation of the command
    pub dsl: String,
    /// When the command was executed
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Source of the command
    pub source: NavigationSource,
    /// CBU context (if any)
    pub cbu_id: Option<Uuid>,
}

/// Current session scope (from watch response)
#[derive(Clone, Debug)]
pub struct CurrentScope {
    /// Scope type: "galaxy", "book", "cbu", "jurisdiction", "neighborhood", "empty"
    pub scope_type: String,
    /// Scope path for display (e.g., "LU > Apex Capital")
    pub scope_path: String,
    /// Whether the scope data is fully loaded
    pub is_loaded: bool,
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

    /// Service taxonomy data (Product → Service → Resource hierarchy)
    /// Set via fetch_service_taxonomy(), never modified directly
    pub service_taxonomy: Option<ServiceTaxonomy>,

    /// Universe graph data (galaxy navigation - clusters of CBUs)
    /// Set via fetch_universe(), never modified directly
    pub universe_graph: Option<UniverseGraph>,

    /// Investor register data (control holders + aggregate)
    /// Set via fetch_investor_register(), never modified directly
    pub investor_register: Option<InvestorRegisterView>,

    /// Investor list for drill-down (paginated)
    /// Set via fetch_investor_list(), never modified directly
    pub investor_list: Option<InvestorListResponse>,

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

    /// Window stack for modal layer management
    pub window_stack: WindowStack,

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

    /// Service taxonomy browser state (expand/collapse, selection, filters)
    pub service_taxonomy_state: ServiceTaxonomyState,

    /// Galaxy view widget (universe navigation - force-directed cluster layout)
    /// Owns camera, force simulation - call tick() BEFORE render() per egui rules
    pub galaxy_view: GalaxyView,

    /// Current navigation scope (Universe, Cluster, Cbu, Entity, etc.)
    pub navigation_scope: NavigationScope,

    /// Current view level (astronomical metaphor)
    pub view_level: ViewLevel,

    /// Navigation breadcrumb stack for drill-up
    pub navigation_stack: Vec<NavigationScope>,

    /// Last known session version (for detecting external changes from MCP/REPL)
    /// When server version differs from this, we refetch the full session
    pub last_known_version: Option<String>,

    /// Timestamp of last version check (to throttle polling)
    pub last_version_check: Option<f64>,

    /// Navigation command log (DSL audit trail for voice/keyboard navigation)
    /// Capped at 1000 entries to prevent unbounded growth
    pub navigation_log: Vec<NavigationLogEntry>,

    /// Current session scope (from watch response)
    /// E.g., "galaxy", "book", "cbu", "jurisdiction", "neighborhood"
    pub current_scope: Option<CurrentScope>,

    /// Investor register UI state (aggregate breakdown view, drill-down)
    pub investor_register_ui: InvestorRegisterUi,

    /// Pending navigation verb from typed command (needs App context to execute)
    pub pending_navigation_verb: Option<crate::command::NavigationVerb>,

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
            service_taxonomy: None,
            universe_graph: None,
            investor_register: None,
            investor_list: None,

            // UI-only state
            buffers: TextBuffers::default(),
            view_mode: ViewMode,
            panels: PanelState::default(),
            selected_entity_id: None,
            resolution_ui: ResolutionPanelUi::default(),
            window_stack: WindowStack::default(),
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
            service_taxonomy_state: ServiceTaxonomyState::new(),
            galaxy_view: GalaxyView::new(),
            navigation_scope: NavigationScope::default(),
            view_level: ViewLevel::default(),
            navigation_stack: Vec::new(),
            last_known_version: None,
            last_version_check: None,
            navigation_log: Vec::new(),
            current_scope: None,
            investor_register_ui: InvestorRegisterUi::default(),
            pending_navigation_verb: None,

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

    /// Last agent-generated DSL (for correction detection)
    /// Set when agent generates DSL, compared on execute to detect user edits
    pub last_agent_dsl: Option<String>,

    /// Last user message that triggered DSL generation (for correction context)
    pub last_agent_message: Option<String>,
}

// =============================================================================
// PANEL STATE - UI layout configuration
// =============================================================================

/// Which browser is shown in the left panel
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BrowserTab {
    /// Entity type taxonomy browser
    #[default]
    Taxonomy,
    /// Trading matrix configuration browser
    TradingMatrix,
    /// Service resources taxonomy browser
    ServiceResources,
}

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
    /// Which browser tab is active in the left panel
    pub browser_tab: BrowserTab,
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
            layout: LayoutMode::Simplified,
            browser_tab: BrowserTab::default(),
        }
    }
}

/// Layout mode for panel arrangement
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    #[default]
    Simplified, // New default: 90% viewport + 10% bottom (Chat left, Session right)
    FourPanel,     // 2x2 grid (legacy)
    EditorFocus,   // Large DSL editor + small panels
    GraphFocus,    // Large graph + small panels
    GraphFullSize, // Graph only, full window
}

// =============================================================================
// WINDOW STACK (Modal Layer Management)
// =============================================================================

/// Window stack for managing modal overlays
///
/// Follows the layer architecture from strategy-patterns.md:
/// - Layer 0: Base (main panels)
/// - Layer 1: Slide-in panels (entity detail, container browse)
/// - Layer 2: Modals (resolution, confirmation dialogs)
/// - Layer 3: Toasts/notifications
#[derive(Default, Clone)]
pub struct WindowStack {
    /// Stack of active windows (LIFO - top is rendered last/on top)
    pub windows: Vec<WindowEntry>,
}

/// A window entry in the stack
#[derive(Clone, Debug)]
pub struct WindowEntry {
    /// Unique ID for this window instance
    pub id: String,
    /// Type of window
    pub window_type: WindowType,
    /// Layer level (higher = on top)
    pub layer: u8,
    /// Whether this window blocks interaction with lower layers
    pub modal: bool,
    /// Associated data (e.g., subsession ID for resolution)
    pub data: Option<WindowData>,
}

/// Types of windows that can be in the stack
#[derive(Clone, Debug, PartialEq)]
pub enum WindowType {
    /// Entity resolution modal
    Resolution,
    /// Confirmation dialog
    Confirmation,
    /// Entity detail slide-in
    EntityDetail,
    /// Container browse slide-in
    ContainerBrowse,
    /// Help overlay
    Help,
    /// CBU search modal
    CbuSearch,
    /// Error/warning toast
    Toast,
}

/// Data associated with a window
#[derive(Clone, Debug)]
pub enum WindowData {
    /// Resolution window data
    Resolution {
        /// Parent session ID
        parent_session_id: String,
        /// Sub-session ID
        subsession_id: String,
        /// Current ref index
        current_ref_index: usize,
        /// Total refs to resolve
        total_refs: usize,
    },
    /// Disambiguation from agent chat - simpler than full resolution
    Disambiguation {
        /// The disambiguation request from agent
        request: ob_poc_types::DisambiguationRequest,
        /// Current item index being resolved
        current_item_index: usize,
        /// Search results for current item
        search_results: Option<Vec<ob_poc_types::EntityMatch>>,
    },
    /// Confirmation dialog data
    Confirmation {
        title: String,
        message: String,
        confirm_label: String,
        cancel_label: String,
    },
    /// Toast notification data
    Toast {
        message: String,
        severity: ToastSeverity,
        auto_dismiss_ms: Option<u64>,
    },
}

/// Toast severity levels
#[derive(Clone, Debug, PartialEq)]
pub enum ToastSeverity {
    Info,
    Success,
    Warning,
    Error,
}

impl WindowStack {
    /// Push a new window onto the stack
    pub fn push(&mut self, entry: WindowEntry) {
        self.windows.push(entry);
    }

    /// Pop the top window from the stack
    pub fn pop(&mut self) -> Option<WindowEntry> {
        self.windows.pop()
    }

    /// Remove a window by ID
    pub fn remove(&mut self, id: &str) {
        self.windows.retain(|w| w.id != id);
    }

    /// Check if a window type is active
    pub fn has(&self, window_type: &WindowType) -> bool {
        self.windows.iter().any(|w| &w.window_type == window_type)
    }

    /// Get the topmost window
    pub fn top(&self) -> Option<&WindowEntry> {
        self.windows.last()
    }

    /// Check if any modal is blocking
    pub fn is_blocked(&self) -> bool {
        self.windows.iter().any(|w| w.modal)
    }

    /// Open resolution window
    pub fn open_resolution(
        &mut self,
        parent_session_id: String,
        subsession_id: String,
        total_refs: usize,
    ) {
        let id = format!("resolution-{}", subsession_id);
        self.push(WindowEntry {
            id,
            window_type: WindowType::Resolution,
            layer: 2,
            modal: true,
            data: Some(WindowData::Resolution {
                parent_session_id,
                subsession_id,
                current_ref_index: 0,
                total_refs,
            }),
        });
    }

    /// Close resolution window
    pub fn close_resolution(&mut self) {
        self.windows
            .retain(|w| w.window_type != WindowType::Resolution);
    }

    /// Show a toast notification
    pub fn toast(&mut self, message: String, severity: ToastSeverity) {
        let id = format!("toast-{}", chrono::Utc::now().timestamp_millis());
        self.push(WindowEntry {
            id,
            window_type: WindowType::Toast,
            layer: 3,
            modal: false,
            data: Some(WindowData::Toast {
                message,
                severity,
                auto_dismiss_ms: Some(3000),
            }),
        });
    }

    /// Find a window by type (immutable)
    pub fn find_by_type(&self, window_type: WindowType) -> Option<&WindowEntry> {
        self.windows.iter().find(|w| w.window_type == window_type)
    }

    /// Find a window by type (mutable)
    pub fn find_by_type_mut(&mut self, window_type: WindowType) -> Option<&mut WindowEntry> {
        self.windows
            .iter_mut()
            .find(|w| w.window_type == window_type)
    }

    /// Close all windows of a given type
    pub fn close_by_type(&mut self, window_type: WindowType) {
        self.windows.retain(|w| w.window_type != window_type);
    }

    /// Check if any modal is active
    pub fn has_modal(&self) -> bool {
        self.windows.iter().any(|w| w.modal)
    }
}

// =============================================================================
// RESOLUTION PANEL UI STATE
// =============================================================================

/// Resolution panel UI-only state (not persisted, not synced)
#[derive(Default, Clone)]
pub struct ResolutionPanelUi {
    /// Currently selected ref_id for resolution
    pub selected_ref_id: Option<String>,
    /// Search query for current ref (legacy - kept for backward compat)
    pub search_query: String,
    /// Chat buffer for sub-session conversation
    pub chat_buffer: String,
    /// Search results from last search
    pub search_results: Option<ResolutionSearchResponse>,
    /// Expanded discriminator section
    pub show_discriminators: bool,
    /// Discriminator values being edited
    pub discriminator_values: std::collections::HashMap<String, String>,
    /// Show resolution panel (modal/overlay)
    pub show_panel: bool,
    /// Sub-session messages (role, content)
    pub messages: Vec<(String, String)>,
    /// Current ref name being resolved
    pub current_ref_name: Option<String>,
    /// DSL context around the ref
    pub dsl_context: Option<String>,
    /// Voice input active (listening)
    pub voice_active: bool,
    /// Last voice transcript received
    pub last_voice_transcript: Option<String>,

    // === Entity-specific config (from UnresolvedRefResponse) ===
    /// Current entity type being resolved (e.g., "cbu", "person", "jurisdiction")
    pub current_entity_type: Option<String>,
    /// Search key fields for this entity type (from entity_index.yaml)
    pub search_keys: Vec<SearchKeyField>,
    /// Multi-key search values (e.g., {"name": "Allianz", "jurisdiction": "LU"})
    pub search_key_values: std::collections::HashMap<String, String>,
    /// Discriminator fields for scoring refinement
    pub discriminator_fields: Vec<DiscriminatorField>,
    /// Resolution mode hint (SearchModal vs Autocomplete)
    pub resolution_mode: ResolutionModeHint,
    /// Current unresolved ref being worked on
    pub current_ref: Option<UnresolvedRefResponse>,
    /// Debounce: pending search trigger time (for 300ms delay)
    pub pending_search_trigger: Option<f64>,
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
// INVESTOR REGISTER UI STATE
// =============================================================================

/// Investor register panel UI-only state
#[derive(Clone)]
pub struct InvestorRegisterUi {
    /// Whether the investor panel is shown
    pub show_panel: bool,

    /// Whether the aggregate node is expanded (showing breakdown)
    pub aggregate_expanded: bool,

    /// Current breakdown dimension being viewed
    pub breakdown_dimension: BreakdownDimension,

    /// Whether drill-down list is shown
    pub show_drill_down: bool,

    /// Current drill-down page (1-indexed)
    pub drill_down_page: i32,

    /// Drill-down filter: investor type
    pub filter_investor_type: Option<String>,

    /// Drill-down filter: KYC status
    pub filter_kyc_status: Option<String>,

    /// Drill-down filter: jurisdiction
    pub filter_jurisdiction: Option<String>,

    /// Drill-down search query
    pub search_query: String,

    /// Sort field for drill-down list
    pub sort_by: String,

    /// Sort direction (true = ascending)
    pub sort_ascending: bool,

    /// Selected investor entity ID (for detail view)
    pub selected_investor_id: Option<String>,
}

impl Default for InvestorRegisterUi {
    fn default() -> Self {
        Self {
            show_panel: false,
            aggregate_expanded: false,
            breakdown_dimension: BreakdownDimension::InvestorType,
            show_drill_down: false,
            drill_down_page: 1,
            filter_investor_type: None,
            filter_kyc_status: None,
            filter_jurisdiction: None,
            search_query: String::new(),
            sort_by: "name".to_string(),
            sort_ascending: true,
            selected_investor_id: None,
        }
    }
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
    /// Pending version check result (just the version string, not full session)
    pub pending_version_check: Option<Result<String, String>>,
    /// Pending watch result (from long-polling /api/session/:id/watch)
    pub pending_watch: Option<Result<crate::api::WatchSessionResponse, String>>,
    pub pending_graph: Option<Result<CbuGraphData, String>>,
    pub pending_scope_graph: Option<Result<ScopeGraphData, String>>,
    pub pending_validation: Option<Result<ValidateDslResponse, String>>,
    pub pending_execution: Option<Result<ExecuteResponse, String>>,
    pub pending_cbu_list: Option<Result<Vec<CbuSummary>, String>>,
    pub pending_chat: Option<Result<ChatMessage, String>>,
    /// DSL source to put back in chat input (for editing after error)
    pub pending_chat_input: Option<String>,
    pub pending_resolution: Option<Result<ResolutionSessionResponse, String>>,
    pub pending_resolution_search: Option<Result<ResolutionSearchResponse, String>>,
    pub pending_cbu_search: Option<Result<crate::api::CbuSearchResponse, String>>,
    pub pending_session_context: Option<Result<SessionContext, String>>,
    pub pending_trading_matrix: Option<Result<TradingMatrix, String>>,
    pub pending_service_taxonomy: Option<Result<ServiceTaxonomy, String>>,
    pub pending_investor_register: Option<Result<InvestorRegisterView, String>>,
    pub pending_investor_list: Option<Result<InvestorListResponse, String>>,

    // Disambiguation (from agent chat when entity is ambiguous)
    /// Disambiguation request from chat response
    pub pending_disambiguation: Option<ob_poc_types::DisambiguationRequest>,
    /// Search results for disambiguation
    pub pending_disambiguation_results: Option<Result<Vec<ob_poc_types::EntityMatch>, String>>,
    /// Loading flag for disambiguation search
    pub loading_disambiguation: bool,
    /// Flag to trigger session refetch after entity bind
    pub pending_session_refetch: bool,
    /// Pending CBU lookup result (from name search) - tuple of (uuid, display_name)
    pub pending_cbu_lookup: Option<Result<(Uuid, String), String>>,
    /// Loading flag for CBU lookup
    pub loading_cbu_lookup: bool,

    // Unresolved refs (direct from ChatResponse - new simplified flow)
    /// Unresolved refs from chat response - opens resolution modal directly
    pub pending_unresolved_refs: Option<Vec<UnresolvedRefResponse>>,
    /// Current ref index from chat response
    pub pending_current_ref_index: Option<usize>,

    // Command triggers (from agent commands)
    pub pending_execute: Option<Uuid>, // Session ID to execute
    pub pending_search_cbu_query: Option<String>, // Open CBU search with query pre-filled
    pub needs_cbu_search_trigger: Option<String>, // Trigger CBU search with query (set in process_async_results)

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

    // Hierarchy navigation (expand/collapse nodes)
    pub pending_expand_node: Option<String>, // Node key to expand
    pub pending_collapse_node: Option<String>, // Node key to collapse

    // Export and layout commands
    pub pending_export: Option<String>, // Export format: "png", "svg", "pdf"
    pub pending_toggle_orientation: bool, // Toggle VERTICAL/HORIZONTAL layout
    pub pending_search: Option<String>, // Search query for graph
    pub pending_show_help: bool,        // Show help overlay

    // Resolution sub-session commands (from agent chat)
    pub pending_start_resolution: Option<(String, usize)>, // (subsession_id, total_refs)
    pub pending_resolution_select: Option<usize>,          // Selection index
    pub pending_resolution_skip: bool,                     // Skip current ref
    pub pending_resolution_complete: Option<bool>,         // Complete with apply flag
    pub pending_resolution_cancel: bool,                   // Cancel resolution
    pub loading_resolution_search: bool,                   // Loading flag for multi-key search

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
    pub pending_taxonomy_reset: bool,             // reset to root level
    pub pending_taxonomy_filter: Option<String>,  // filter expression
    pub pending_taxonomy_clear_filter: bool,      // clear taxonomy filter

    // Taxonomy API responses (from server calls)
    pub pending_taxonomy_breadcrumbs_response:
        Option<Result<crate::api::TaxonomyBreadcrumbsResponse, String>>,
    pub pending_taxonomy_zoom_response: Option<Result<crate::api::TaxonomyZoomResponse, String>>,
    pub loading_taxonomy: bool, // loading flag for taxonomy operations

    // Galaxy navigation (universe/cluster drill-down)
    pub pending_universe_graph: Option<Result<UniverseGraph, String>>, // From GET /api/universe
    pub pending_drill_cluster: Option<String>,                         // Cluster ID to drill into
    pub pending_drill_cbu: Option<String>, // CBU ID to drill into (from galaxy)
    pub pending_drill_up: bool,            // Go up one level in navigation stack
    pub pending_go_to_universe: bool,      // Jump to universe view
    pub needs_universe_refetch: bool,      // Flag to trigger universe fetch
    pub loading_universe: bool,            // Loading flag for universe fetch

    // State change flags (set by actions, processed centrally in update loop)
    // These are checked ONCE in update() AFTER process_async_results()
    pub needs_graph_refetch: bool, // CBU selected or view mode changed
    pub needs_scope_graph_refetch: bool, // Execution complete, fetch multi-CBU scope graph
    pub needs_context_refetch: bool, // CBU selected, fetch session context
    pub needs_trading_matrix_refetch: bool, // Trading view mode selected
    pub needs_session_refetch: bool, // Version changed, need full session refetch
    // NOTE: needs_resolution_check removed - now using direct ChatResponse.unresolved_refs flow
    // See ai-thoughts/036-session-rip-and-replace.md
    pub needs_investor_register_refetch: bool, // Investor register view selected
    pub pending_cbu_id: Option<Uuid>,          // CBU to fetch graph for (set by select_cbu)
    pub pending_issuer_id: Option<Uuid>,       // Issuer to fetch investor register for

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
    pub loading_service_taxonomy: bool,
    pub loading_investor_register: bool,
    pub loading_investor_list: bool,
    pub checking_version: bool, // Version poll in progress
    pub watching_session: bool, // Long-poll watch in progress

    // Chat focus tracking - set when chat completes to refocus input
    pub chat_just_finished: bool,

    // Initial focus - set to true on first frame to focus chat input
    pub needs_initial_focus: bool,

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
                    // Sync DSL editor from session.combined_dsl if server has content and we're not dirty
                    if !self.buffers.dsl_dirty {
                        if let Some(ref dsl) = session.combined_dsl {
                            self.buffers.dsl_editor = dsl.clone();
                            // Track agent-generated DSL for correction detection
                            self.buffers.last_agent_dsl = Some(dsl.clone());
                        }
                    }

                    // NOTE: Legacy resolution check removed - unresolved refs now come directly
                    // in ChatResponse.unresolved_refs and are handled via pending_unresolved_refs

                    // Track session version for external change detection (MCP/REPL)
                    self.last_known_version = session.version.clone();
                    self.session = Some(session);
                }
                Err(e) => {
                    state.last_error = Some(format!("Session fetch failed: {}", e));
                }
            }
        }

        // Process version check - if version changed, trigger full session refetch
        if let Some(result) = state.pending_version_check.take() {
            state.checking_version = false;
            if let Ok(server_version) = result {
                if let Some(ref known_version) = self.last_known_version {
                    if &server_version != known_version {
                        // Version changed externally (MCP/REPL modified session)
                        // Trigger full session refetch
                        web_sys::console::log_1(
                            &format!(
                                "Session version changed: {} -> {}, triggering refetch",
                                known_version, server_version
                            )
                            .into(),
                        );
                        state.needs_session_refetch = true;
                        state.needs_graph_refetch = true;
                    }
                }
            }
        }

        // Process watch result - reactive session updates via long-polling
        if let Some(result) = state.pending_watch.take() {
            state.watching_session = false;
            match result {
                Ok(watch_response) => {
                    // Check if session actually changed (version comparison)
                    let version_str = watch_response.version.to_string();
                    let changed = self
                        .last_known_version
                        .as_ref()
                        .map(|v| v != &version_str)
                        .unwrap_or(true);

                    if changed {
                        web_sys::console::log_1(
                            &format!(
                                "Session watch: version changed to {}, scope={}, scope_type={:?}, triggering refetch",
                                watch_response.version, watch_response.scope_path, watch_response.scope_type
                            )
                            .into(),
                        );
                        // Update last known version
                        self.last_known_version = Some(version_str);

                        // Trigger refetches based on what changed
                        state.needs_session_refetch = true;

                        // If active_cbu changed, refetch graph
                        if watch_response.active_cbu_id.is_some() {
                            state.needs_graph_refetch = true;
                            state.pending_cbu_id = watch_response.active_cbu_id;
                        }

                        // If scope type changed (session.set-* verb ran), trigger graph refetch
                        // The new scope will be loaded from the session context
                        if watch_response.scope_type.is_some() {
                            web_sys::console::log_1(
                                &format!(
                                    "Session watch: scope changed to {:?}, triggering viewport rebuild",
                                    watch_response.scope_type
                                )
                                .into(),
                            );
                            state.needs_graph_refetch = true;
                        }
                    }

                    // Always update current scope from watch response
                    if let Some(ref scope_type) = watch_response.scope_type {
                        self.current_scope = Some(CurrentScope {
                            scope_type: scope_type.clone(),
                            scope_path: watch_response.scope_path.clone(),
                            is_loaded: watch_response.scope_loaded,
                        });
                    }
                }
                Err(e) => {
                    // Don't show error for timeout - that's expected
                    if !e.contains("timeout") && !e.contains("Timeout") {
                        web_sys::console::warn_1(&format!("Session watch failed: {}", e).into());
                    }
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

        // Process scope graph fetch (multi-CBU session graph)
        if let Some(result) = state.pending_scope_graph.take() {
            match result {
                Ok(data) => {
                    web_sys::console::log_1(
                        &format!(
                            "process_async_results: scope graph received for {} CBUs, {} affected entities",
                            data.cbu_count,
                            data.affected_entity_ids.len()
                        )
                        .into(),
                    );
                    // If we got a graph, use it like a regular graph
                    if let Some(graph_data) = data.graph {
                        self.graph_widget.set_data(graph_data.clone());
                        self.graph_data = Some(graph_data);

                        // Populate ontology counts from graph data
                        if let Some(layout_graph) = self.graph_widget.get_layout_graph() {
                            self.entity_ontology.populate_counts(layout_graph);
                        }
                    }
                    // TODO: Could highlight affected_entity_ids in the viewport
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("Scope graph fetch failed: {}", e).into());
                    state.last_error = Some(format!("Scope graph fetch failed: {}", e));
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

        // If DSL source should be put back in chat input (for editing after error)
        if let Some(dsl_source) = state.pending_chat_input.take() {
            self.buffers.chat_input = dsl_source;
        }

        // Process pending disambiguation request - opens modal
        if let Some(disambig) = state.pending_disambiguation.take() {
            web_sys::console::log_1(
                &format!(
                    "process_async_results: opening disambiguation modal for {} items",
                    disambig.items.len()
                )
                .into(),
            );

            // Create window entry for disambiguation modal
            let window = WindowEntry {
                id: format!("disambig-{}", disambig.request_id),
                window_type: WindowType::Resolution,
                layer: 2, // Modal layer
                modal: true,
                data: Some(WindowData::Disambiguation {
                    request: disambig.clone(),
                    current_item_index: 0,
                    search_results: None,
                }),
            };

            self.window_stack.push(window);

            // Initialize search buffer with first item's search text
            if let Some(ob_poc_types::DisambiguationItem::EntityMatch {
                ref search_text, ..
            }) = disambig.items.first()
            {
                self.resolution_ui.search_query = search_text.clone();
            }
        }

        // Process disambiguation search results
        if let Some(result) = state.pending_disambiguation_results.take() {
            state.loading_disambiguation = false;
            match result {
                Ok(matches) => {
                    // Update the disambiguation window with search results
                    if let Some(window) = self.window_stack.find_by_type_mut(WindowType::Resolution)
                    {
                        if let Some(WindowData::Disambiguation {
                            ref mut search_results,
                            ..
                        }) = window.data
                        {
                            *search_results = Some(matches);
                        }
                    }
                }
                Err(e) => state.last_error = Some(format!("Disambiguation search failed: {}", e)),
            }
        }

        // Process unresolved refs from ChatResponse - DIRECT FLOW (2 hops, not 5+)
        // This is the new simplified resolution flow per ai-thoughts/036
        if let Some(refs) = state.pending_unresolved_refs.take() {
            let current_index = state.pending_current_ref_index.take().unwrap_or(0);

            web_sys::console::log_1(
                &format!(
                    "process_async_results: opening resolution modal for {} refs (index {})",
                    refs.len(),
                    current_index
                )
                .into(),
            );

            // Set up resolution UI state
            self.resolution_ui.show_panel = true;

            // Set current ref from the list
            if let Some(current_ref) = refs.get(current_index) {
                self.resolution_ui.current_ref = Some(current_ref.clone());
                self.resolution_ui.current_entity_type = Some(current_ref.entity_type.clone());
                self.resolution_ui.search_keys = current_ref.search_keys.clone();
                self.resolution_ui.discriminator_fields = current_ref.discriminator_fields.clone();
                self.resolution_ui.resolution_mode = current_ref.resolution_mode.clone();

                // Initialize search with the search value
                self.resolution_ui.search_query = current_ref.search_value.clone();

                // Clear previous search results
                self.resolution_ui.search_results = None;
            }
        }

        // Process CBU lookup result (from disambiguation name search)
        if let Some(result) = state.pending_cbu_lookup.take() {
            state.loading_cbu_lookup = false;
            match result {
                Ok((uuid, display_name)) => {
                    // Need to drop the lock before calling select_cbu since it acquires the lock
                    drop(state);
                    self.select_cbu(uuid, &display_name);
                    return; // Early return since we dropped the lock
                }
                Err(e) => {
                    state.last_error = Some(e);
                }
            }
        }

        // Process session refetch flag (after entity bind)
        if state.pending_session_refetch {
            state.pending_session_refetch = false;
            state.needs_session_refetch = true;
            // Also trigger graph, matrix, and context refetch since CBU likely changed
            state.needs_graph_refetch = true;
            state.needs_trading_matrix_refetch = true;
            state.needs_context_refetch = true;
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

        // Process pending CBU search popup request (from SearchCbu agent command)
        // This handles typos in "show cbu <name>" - opens search popup so user can correct spelling
        if let Some(query) = state.pending_search_cbu_query.take() {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(
                &format!("Opening CBU search popup with query: {}", query).into(),
            );
            self.cbu_search_ui.results = None;
            self.cbu_search_ui.query = query.clone();
            self.cbu_search_ui.open = true;
            self.cbu_search_ui.just_opened = true;
            // Set flag to trigger search in update loop (where search_cbus is accessible)
            state.needs_cbu_search_trigger = Some(query);
        }

        // Process session context
        if let Some(result) = state.pending_session_context.take() {
            state.loading_session_context = false;
            match result {
                Ok(context) => {
                    // Apply viewport_state to graph widget if present
                    // This syncs DSL-driven viewport state (from viewport.* verbs) to the UI
                    if let Some(ref viewport_state) = context.viewport_state {
                        self.graph_widget.apply_viewport_state(viewport_state);
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::log_1(
                            &format!(
                                "Applied viewport_state from session: focus={:?}",
                                viewport_state.focus.state
                            )
                            .into(),
                        );
                    }
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

        // Process service taxonomy
        if let Some(result) = state.pending_service_taxonomy.take() {
            state.loading_service_taxonomy = false;
            match result {
                Ok(taxonomy) => {
                    // Expand first level by default for better UX
                    self.service_taxonomy_state
                        .expand_to_depth(&taxonomy.root, 1);
                    self.service_taxonomy = Some(taxonomy);
                }
                Err(e) => state.last_error = Some(format!("Service taxonomy fetch failed: {}", e)),
            }
        }

        // Process investor register
        if let Some(result) = state.pending_investor_register.take() {
            state.loading_investor_register = false;
            match result {
                Ok(register) => {
                    // Auto-show panel when investor register data arrives
                    if !register.control_holders.is_empty() || register.aggregate.is_some() {
                        self.investor_register_ui.show_panel = true;
                    }
                    self.investor_register = Some(register);
                }
                Err(e) => state.last_error = Some(format!("Investor register fetch failed: {}", e)),
            }
        }

        // Process investor list (drill-down)
        if let Some(result) = state.pending_investor_list.take() {
            state.loading_investor_list = false;
            match result {
                Ok(list) => {
                    self.investor_list = Some(list);
                }
                Err(e) => state.last_error = Some(format!("Investor list fetch failed: {}", e)),
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

    /// Check if CBU search trigger is needed and return the query
    /// This is used when agent SearchCbu command opens the popup
    pub fn take_pending_cbu_search_trigger(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.needs_cbu_search_trigger.take()
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

        // Use pending_cbu_id if set (from select_cbu), otherwise get active CBU from session
        if let Some(cbu_id) = state.pending_cbu_id.take() {
            return Some(cbu_id);
        }

        // Fall back to active CBU from session (for view mode changes)
        // NOTE: We must NOT use session_id here - that's the SESSION UUID, not the CBU ID
        drop(state); // Release lock before accessing self
        self.session
            .as_ref()
            .and_then(|s| s.active_cbu_id())
            .and_then(|id| Uuid::parse_str(&id).ok())
    }

    /// Check if scope graph refetch is needed (multi-CBU session graph after execution)
    /// Called ONCE per frame in update() - returns true if refetch needed
    pub fn take_pending_scope_graph_refetch(&self) -> bool {
        let Ok(mut state) = self.async_state.lock() else {
            return false;
        };

        if !state.needs_scope_graph_refetch {
            return false;
        }

        // Clear the flag
        state.needs_scope_graph_refetch = false;
        true
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

        // Use pending_cbu_id if set (from select_cbu), otherwise get active CBU from session
        if let Some(cbu_id) = state.pending_cbu_id {
            return Some(cbu_id);
        }

        // Fall back to active CBU from session (for view mode changes)
        // NOTE: We must NOT use session_id here - that's the SESSION UUID, not the CBU ID
        drop(state); // Release lock before accessing self
        self.session
            .as_ref()
            .and_then(|s| s.active_cbu_id())
            .and_then(|id| Uuid::parse_str(&id).ok())
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

    /// Check if session refetch is needed (version change detected from MCP/REPL)
    /// Called ONCE per frame in update() - the single central place for session fetches
    pub fn take_pending_session_refetch(&self) -> bool {
        let Ok(mut state) = self.async_state.lock() else {
            return false;
        };

        if !state.needs_session_refetch {
            return false;
        }

        // Clear the flag
        state.needs_session_refetch = false;
        true
    }

    /// Check if investor register refetch is needed and return the issuer ID to fetch
    /// Called ONCE per frame in update() - the single central place for investor register fetches
    pub fn take_pending_investor_register_refetch(&self) -> Option<Uuid> {
        let Ok(mut state) = self.async_state.lock() else {
            return None;
        };

        if !state.needs_investor_register_refetch {
            return None;
        }

        // Clear the flag
        state.needs_investor_register_refetch = false;

        // Use pending_issuer_id if set
        if let Some(issuer_id) = state.pending_issuer_id.take() {
            return Some(issuer_id);
        }

        // No issuer ID available
        None
    }

    // NOTE: take_pending_resolution_check removed - now using direct ChatResponse.unresolved_refs flow
    // See ai-thoughts/036-session-rip-and-replace.md

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
    // Hierarchy Navigation - Take Methods
    // =========================================================================

    /// Check if an expand node command is pending
    pub fn take_pending_expand_node(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_expand_node.take()
        } else {
            None
        }
    }

    /// Check if a collapse node command is pending
    pub fn take_pending_collapse_node(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_collapse_node.take()
        } else {
            None
        }
    }

    // =========================================================================
    // Export and Layout - Take Methods
    // =========================================================================

    /// Check if an export command is pending
    pub fn take_pending_export(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_export.take()
        } else {
            None
        }
    }

    /// Check if a toggle orientation command is pending
    pub fn take_pending_toggle_orientation(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_toggle_orientation;
            state.pending_toggle_orientation = false;
            pending
        } else {
            false
        }
    }

    /// Check if a search command is pending
    pub fn take_pending_search(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_search.take()
        } else {
            None
        }
    }

    /// Check if a show help command is pending
    pub fn take_pending_show_help(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_show_help;
            state.pending_show_help = false;
            pending
        } else {
            false
        }
    }

    // =========================================================================
    // Resolution Sub-Session Commands - Take Methods
    // =========================================================================

    /// Check if a start resolution command is pending
    pub fn take_pending_start_resolution(&self) -> Option<(String, usize)> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_start_resolution.take()
        } else {
            None
        }
    }

    /// Check if a resolution select command is pending
    pub fn take_pending_resolution_select(&self) -> Option<usize> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_resolution_select.take()
        } else {
            None
        }
    }

    /// Check if a resolution skip command is pending
    pub fn take_pending_resolution_skip(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_resolution_skip;
            state.pending_resolution_skip = false;
            pending
        } else {
            false
        }
    }

    /// Check if a resolution complete command is pending
    pub fn take_pending_resolution_complete(&self) -> Option<bool> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_resolution_complete.take()
        } else {
            None
        }
    }

    /// Check if a resolution cancel command is pending
    pub fn take_pending_resolution_cancel(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_resolution_cancel;
            state.pending_resolution_cancel = false;
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

    /// Check if a taxonomy reset is pending
    pub fn take_pending_taxonomy_reset(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_taxonomy_reset;
            state.pending_taxonomy_reset = false;
            pending
        } else {
            false
        }
    }

    /// Take pending taxonomy filter (if any)
    pub fn take_pending_taxonomy_filter(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_taxonomy_filter.take()
        } else {
            None
        }
    }

    /// Check if a taxonomy clear filter is pending
    pub fn take_pending_taxonomy_clear_filter(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_taxonomy_clear_filter;
            state.pending_taxonomy_clear_filter = false;
            pending
        } else {
            false
        }
    }

    // === Galaxy Navigation take_pending_* methods ===

    /// Take pending universe graph result (from GET /api/universe)
    pub fn take_pending_universe_graph(&self) -> Option<Result<UniverseGraph, String>> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_universe_graph.take()
        } else {
            None
        }
    }

    /// Take pending drill into cluster action
    pub fn take_pending_drill_cluster(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_drill_cluster.take()
        } else {
            None
        }
    }

    /// Take pending drill into CBU action (from galaxy view)
    pub fn take_pending_drill_cbu(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_drill_cbu.take()
        } else {
            None
        }
    }

    /// Check if drill up is pending
    pub fn take_pending_drill_up(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_drill_up;
            state.pending_drill_up = false;
            pending
        } else {
            false
        }
    }

    /// Check if go to universe is pending
    pub fn take_pending_go_to_universe(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_go_to_universe;
            state.pending_go_to_universe = false;
            pending
        } else {
            false
        }
    }

    /// Check if universe refetch is needed
    pub fn take_pending_universe_refetch(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.needs_universe_refetch;
            state.needs_universe_refetch = false;
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
    ///
    /// Priority: editor buffer (user editing) > session.combined_dsl (server state)
    pub fn get_dsl_source(&self) -> Option<String> {
        // First try the editor buffer if it has content
        if !self.buffers.dsl_editor.trim().is_empty() {
            return Some(self.buffers.dsl_editor.clone());
        }

        // Then try session's combined_dsl
        if let Some(ref session) = self.session {
            if let Some(ref dsl) = session.combined_dsl {
                if !dsl.is_empty() {
                    return Some(dsl.clone());
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

    /// Fetch service taxonomy for a CBU (Product → Service → Resource hierarchy)
    /// Called when Services tab is selected in the browser panel
    pub fn fetch_service_taxonomy(&mut self, cbu_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_service_taxonomy = true;
        }

        spawn_local(async move {
            let result = api::get_service_taxonomy(cbu_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_service_taxonomy = Some(result);
                state.loading_service_taxonomy = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Fetch investor register view for an issuer
    /// Shows control holders (>5%) as individual nodes, aggregates others
    pub fn fetch_investor_register(&mut self, issuer_id: String, share_class: Option<String>) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_investor_register = true;
        }

        spawn_local(async move {
            let result = api::get_investor_register(&issuer_id, share_class.as_deref()).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_investor_register = Some(result);
                state.loading_investor_register = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Fetch paginated investor list for drill-down
    pub fn fetch_investor_list(&mut self, issuer_id: String, page: i32, page_size: i32) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        // Build filters from UI state
        let filters = InvestorFilters {
            investor_type: self.investor_register_ui.filter_investor_type.clone(),
            kyc_status: self.investor_register_ui.filter_kyc_status.clone(),
            jurisdiction: self.investor_register_ui.filter_jurisdiction.clone(),
            search: if self.investor_register_ui.search_query.is_empty() {
                None
            } else {
                Some(self.investor_register_ui.search_query.clone())
            },
            min_units: None,
        };

        {
            let mut state = async_state.lock().unwrap();
            state.loading_investor_list = true;
        }

        spawn_local(async move {
            let result = api::get_investor_list(&issuer_id, page, page_size, &filters).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_investor_list = Some(result);
                state.loading_investor_list = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Check session version for external changes (MCP/REPL)
    /// Called periodically to detect if session was modified externally
    pub fn check_session_version(&mut self, session_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        // Don't start another check if one is in progress
        {
            let state = async_state.lock().unwrap();
            if state.checking_version {
                return;
            }
        }

        {
            let mut state = async_state.lock().unwrap();
            state.checking_version = true;
        }

        spawn_local(async move {
            let result = api::get_session_version(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_version_check = Some(result);
                state.checking_version = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Start long-poll watch for session changes
    /// Uses /api/session/:id/watch which blocks until changes occur or timeout
    /// This replaces periodic polling with reactive updates
    pub fn start_session_watch(&mut self, session_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        // Don't start another watch if one is in progress
        {
            let state = async_state.lock().unwrap();
            if state.watching_session {
                return;
            }
        }

        {
            let mut state = async_state.lock().unwrap();
            state.watching_session = true;
        }

        spawn_local(async move {
            // Long-poll with 30 second timeout
            let result = api::watch_session(session_id, Some(30000)).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_watch = Some(result);
                state.watching_session = false;
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
    ViewMode::parse(s)
}

// =============================================================================
// NAVIGATION LOG METHODS
// =============================================================================

/// Maximum number of navigation log entries to keep
const MAX_NAVIGATION_LOG_ENTRIES: usize = 1000;

impl AppState {
    /// Log a navigation command to the audit trail
    ///
    /// The DSL string is generated from the NavigationVerb using to_dsl_string().
    /// This enables replay and audit of voice/keyboard navigation commands.
    pub fn log_navigation(&mut self, dsl: String, source: NavigationSource) {
        let cbu_id = self
            .session
            .as_ref()
            .and_then(|s| s.active_cbu_id())
            .and_then(|id| uuid::Uuid::parse_str(&id).ok());

        let entry = NavigationLogEntry {
            dsl,
            timestamp: chrono::Utc::now(),
            source,
            cbu_id,
        };

        self.navigation_log.push(entry);

        // Cap the log to prevent unbounded growth
        if self.navigation_log.len() > MAX_NAVIGATION_LOG_ENTRIES {
            // Remove oldest entries
            let excess = self.navigation_log.len() - MAX_NAVIGATION_LOG_ENTRIES;
            self.navigation_log.drain(0..excess);
        }
    }

    /// Get recent navigation log entries (most recent first)
    pub fn recent_navigation_log(&self, limit: usize) -> Vec<&NavigationLogEntry> {
        self.navigation_log.iter().rev().take(limit).collect()
    }

    /// Clear the navigation log
    pub fn clear_navigation_log(&mut self) {
        self.navigation_log.clear();
    }
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

    /// Reset taxonomy to root level (clear the taxonomy stack on server)
    pub fn taxonomy_reset(&mut self, session_id: Uuid) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_taxonomy = true;
        }

        spawn_local(async move {
            let result = api::taxonomy_reset(session_id).await;
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

// =============================================================================
// GALAXY NAVIGATION METHODS
// =============================================================================

impl AppState {
    /// Fetch universe graph (all clusters) from server
    pub fn fetch_universe_graph(&mut self) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_universe = true;
        }

        spawn_local(async move {
            let result = api::get_universe_graph().await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_universe_graph = Some(result);
                state.loading_universe = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Request drill into a specific cluster
    pub fn request_drill_cluster(&mut self, cluster_id: String) {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_drill_cluster = Some(cluster_id);
        }
    }

    /// Request drill into a specific CBU (from galaxy view)
    pub fn request_drill_cbu(&mut self, cbu_id: String) {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_drill_cbu = Some(cbu_id);
        }
    }

    /// Request drill up one level in navigation stack
    pub fn request_drill_up(&mut self) {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_drill_up = true;
        }
    }

    /// Request jump to universe view
    pub fn request_go_to_universe(&mut self) {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_go_to_universe = true;
        }
    }

    /// Fetch CBUs for a commercial client (book/galaxy view)
    /// Maps to `view.book :client <name>` DSL verb
    pub fn fetch_client_book(&mut self, client_name: &str) {
        use crate::api;
        use wasm_bindgen_futures::spawn_local;

        let async_state = std::sync::Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();
        let client_name = client_name.to_string();

        {
            let mut state = async_state.lock().unwrap();
            state.loading_universe = true; // Reuse universe loading state
        }

        spawn_local(async move {
            // Fetch universe filtered by client
            let result = api::get_universe_graph_by_client(&client_name).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_universe_graph = Some(result);
                state.loading_universe = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Check if universe is currently loading
    pub fn is_loading_universe(&self) -> bool {
        if let Ok(state) = self.async_state.lock() {
            state.loading_universe
        } else {
            false
        }
    }
}

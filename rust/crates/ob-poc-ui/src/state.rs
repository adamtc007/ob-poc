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
    CbuGraphData, CbuGraphWidget, ClusterCbuData, ClusterView, EntityTypeOntology, GalaxyView,
    ManCoData, ServiceTaxonomy, ServiceTaxonomyState, TaxonomyState, TradingMatrix,
    TradingMatrixNode, TradingMatrixState, ViewMode,
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

    /// Verb disambiguation UI state (for ambiguous verb matches)
    pub verb_disambiguation_ui: VerbDisambiguationState,

    /// Macro expansion wizard UI state (for partial macro invocations)
    pub macro_expansion_ui: MacroExpansionState,

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

    /// Cluster view widget (ManCo center + CBU orbital rings)
    /// Used when session loads a "book" (e.g., "show Allianz Lux book")
    pub cluster_view: ClusterView,

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
            verb_disambiguation_ui: VerbDisambiguationState::default(),
            macro_expansion_ui: MacroExpansionState::default(),
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
            cluster_view: ClusterView::new(),
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
    /// DSL hash for resolution commit verification (Issue K)
    /// Stored from ChatResponse, must be passed to select_resolution
    pub dsl_hash: Option<String>,
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
// VERB DISAMBIGUATION UI STATE
// =============================================================================

/// Verb disambiguation UI state
///
/// When the agent returns `verb_disambiguation` in a ChatResponse,
/// the user must select from multiple verb candidates before proceeding.
/// This is earlier in the pipeline than entity disambiguation.
#[derive(Default, Clone)]
pub struct VerbDisambiguationState {
    /// Whether verb disambiguation is currently active
    pub active: bool,
    /// The disambiguation request from the server
    pub request: Option<ob_poc_types::VerbDisambiguationRequest>,
    /// Original user input that triggered disambiguation
    pub original_input: String,
    /// When disambiguation was shown (for timeout handling)
    pub shown_at: Option<f64>,
    /// Loading flag while selecting/abandoning
    pub loading: bool,
}

impl VerbDisambiguationState {
    /// Check if disambiguation has timed out (30 seconds)
    pub fn is_timed_out(&self, current_time: f64) -> bool {
        const TIMEOUT_SECS: f64 = 30.0;
        if let Some(shown_at) = self.shown_at {
            (current_time - shown_at) > TIMEOUT_SECS
        } else {
            false
        }
    }

    /// Clear the disambiguation state
    pub fn clear(&mut self) {
        self.active = false;
        self.request = None;
        self.original_input.clear();
        self.shown_at = None;
        self.loading = false;
    }

    /// Set disambiguation from server response
    pub fn set_from_response(
        &mut self,
        request: ob_poc_types::VerbDisambiguationRequest,
        original_input: String,
        current_time: f64,
    ) {
        self.active = true;
        self.request = Some(request);
        self.original_input = original_input;
        self.shown_at = Some(current_time);
        self.loading = false;
    }
}

// =============================================================================
// MACRO EXPANSION UI STATE
// =============================================================================

/// Information about a missing macro argument
#[derive(Clone, Debug)]
pub struct MissingArgInfo {
    /// Argument name
    pub name: String,
    /// Display label from macro schema
    pub ui_label: String,
    /// Argument type (str, enum, party_ref, etc.)
    pub arg_type: String,
    /// Whether this argument is required
    pub required: bool,
    /// Description/help text
    pub description: Option<String>,
    /// Valid values for enum types
    pub valid_values: Vec<MacroEnumOption>,
    /// Picker type for ref args (party_picker, structure_picker, etc.)
    pub picker: Option<String>,
    /// Default value if any
    pub default_value: Option<String>,
}

/// Enum option for macro arguments
#[derive(Clone, Debug)]
pub struct MacroEnumOption {
    /// Key shown in UI
    pub key: String,
    /// Human-readable label
    pub label: String,
    /// Internal token (mapped during expansion)
    pub internal: String,
}

/// Macro expansion wizard UI state
///
/// When a macro has missing required arguments, the wizard guides the user
/// through providing values step-by-step. This is similar to entity disambiguation
/// but for macro arguments.
#[derive(Default, Clone)]
pub struct MacroExpansionState {
    /// Whether the macro wizard is currently active
    pub active: bool,

    /// Fully qualified macro name (e.g., "struct.lux.ucits.sicav")
    pub macro_fqn: Option<String>,

    /// Macro display label (e.g., "Luxembourg UCITS SICAV")
    pub macro_label: Option<String>,

    /// Macro description
    pub macro_description: Option<String>,

    /// Arguments already provided by user
    pub provided_args: std::collections::HashMap<String, String>,

    /// Missing arguments that need to be filled
    pub missing_args: Vec<MissingArgInfo>,

    /// Current step index (which missing arg we're on)
    pub current_step: usize,

    /// Current input value being edited
    pub current_input: String,

    /// Search results for party_ref/structure_ref pickers
    pub picker_results: Option<Vec<ob_poc_types::EntityMatch>>,

    /// Loading flag while fetching picker results
    pub loading: bool,

    /// Error message if any
    pub error_message: Option<String>,

    /// When wizard was shown (for optional timeout)
    pub shown_at: Option<f64>,

    /// Whether to use placeholder for optional args
    pub use_placeholder: bool,
}

impl MacroExpansionState {
    /// Clear the wizard state
    pub fn clear(&mut self) {
        self.active = false;
        self.macro_fqn = None;
        self.macro_label = None;
        self.macro_description = None;
        self.provided_args.clear();
        self.missing_args.clear();
        self.current_step = 0;
        self.current_input.clear();
        self.picker_results = None;
        self.loading = false;
        self.error_message = None;
        self.shown_at = None;
        self.use_placeholder = false;
    }

    /// Initialize wizard for a macro with missing args
    pub fn start(
        &mut self,
        macro_fqn: String,
        macro_label: String,
        macro_description: String,
        provided_args: std::collections::HashMap<String, String>,
        missing_args: Vec<MissingArgInfo>,
        current_time: f64,
    ) {
        self.active = true;
        self.macro_fqn = Some(macro_fqn);
        self.macro_label = Some(macro_label);
        self.macro_description = Some(macro_description);
        self.provided_args = provided_args;
        self.missing_args = missing_args;
        self.current_step = 0;
        self.current_input.clear();
        self.picker_results = None;
        self.loading = false;
        self.error_message = None;
        self.shown_at = Some(current_time);
        self.use_placeholder = false;
    }

    /// Get current argument being filled
    pub fn current_arg(&self) -> Option<&MissingArgInfo> {
        self.missing_args.get(self.current_step)
    }

    /// Check if we're on the last step
    pub fn is_last_step(&self) -> bool {
        self.current_step + 1 >= self.missing_args.len()
    }

    /// Get total step count
    pub fn total_steps(&self) -> usize {
        self.missing_args.len()
    }

    /// Move to next step, returning true if wizard is complete
    pub fn next_step(&mut self) -> bool {
        if self.current_step + 1 >= self.missing_args.len() {
            true // Wizard complete
        } else {
            self.current_step += 1;
            self.current_input.clear();
            self.picker_results = None;
            self.error_message = None;
            false
        }
    }

    /// Move to previous step
    pub fn prev_step(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
            // Restore previous input if we saved it
            if let Some(arg) = self.current_arg() {
                if let Some(value) = self.provided_args.get(&arg.name) {
                    self.current_input = value.clone();
                } else {
                    self.current_input.clear();
                }
            }
            self.picker_results = None;
            self.error_message = None;
        }
    }

    /// Save current input as provided arg
    pub fn save_current_input(&mut self) {
        if let Some(arg) = self.current_arg() {
            let name = arg.name.clone();
            let value = self.current_input.clone();
            if !value.is_empty() {
                self.provided_args.insert(name, value);
            }
        }
    }

    /// Get all provided args as DSL keyword args
    pub fn to_dsl_args(&self) -> String {
        self.provided_args
            .iter()
            .map(|(k, v)| format!(":{} {}", k.replace('_', "-"), v))
            .collect::<Vec<_>>()
            .join(" ")
    }
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
// PENDING RESULTS - Extracted from AsyncState for lock-free processing
// =============================================================================

/// Pending results extracted from AsyncState in one atomic operation.
///
/// This struct enables the "extract all, drop lock, then process" pattern
/// which avoids borrow checker conflicts when processing needs `&mut self`.
///
/// Pattern:
/// 1. Lock async_state
/// 2. Call async_state.extract_pending() to get all pending results at once
/// 3. Drop the lock (extract_pending returns owned data)
/// 4. Process each field with full `&mut self` access
#[derive(Default)]
pub struct PendingResults {
    // === Results that need &mut self processing ===
    /// CBU lookup result - needs to call select_cbu(&mut self)
    pub cbu_lookup: Option<Result<(Uuid, String), String>>,

    // === Results that update AppState fields directly ===
    pub session: Option<Result<SessionStateResponse, String>>,
    pub session_id: Option<Uuid>,
    pub version_check: Option<Result<String, String>>,
    pub watch: Option<Result<crate::api::WatchSessionResponse, String>>,
    pub graph: Option<Result<CbuGraphData, String>>,
    pub scope_graph: Option<Result<ScopeGraphData, String>>,
    pub validation: Option<Result<ValidateDslResponse, String>>,
    pub execution: Option<Result<ExecuteResponse, String>>,
    pub cbu_list: Option<Result<Vec<CbuSummary>, String>>,
    pub chat: Option<Result<ChatMessage, String>>,
    pub chat_input: Option<String>,
    pub resolution: Option<Result<ResolutionSessionResponse, String>>,
    pub resolution_search: Option<Result<ResolutionSearchResponse, String>>,
    pub cbu_search: Option<Result<crate::api::CbuSearchResponse, String>>,
    pub session_context: Option<Result<SessionContext, String>>,
    pub trading_matrix: Option<Result<TradingMatrix, String>>,
    pub service_taxonomy: Option<Result<ServiceTaxonomy, String>>,
    pub investor_register: Option<Result<InvestorRegisterView, String>>,
    pub investor_list: Option<Result<InvestorListResponse, String>>,

    // Entity disambiguation
    pub disambiguation: Option<ob_poc_types::DisambiguationRequest>,
    pub disambiguation_results: Option<Result<Vec<ob_poc_types::EntityMatch>, String>>,

    // Verb disambiguation (earlier in pipeline than entity disambiguation)
    pub verb_disambiguation: Option<ob_poc_types::VerbDisambiguationRequest>,

    // Unresolved refs (direct from ChatResponse)
    pub unresolved_refs: Option<Vec<UnresolvedRefResponse>>,
    pub current_ref_index: Option<usize>,
    /// DSL hash for resolution commit verification (Issue K)
    pub dsl_hash: Option<String>,

    // Command triggers
    pub search_cbu_query: Option<String>,

    // Taxonomy responses
    pub taxonomy_breadcrumbs_response:
        Option<Result<crate::api::TaxonomyBreadcrumbsResponse, String>>,
    pub taxonomy_zoom_response: Option<Result<crate::api::TaxonomyZoomResponse, String>>,

    // Galaxy/Universe
    pub universe_graph: Option<Result<UniverseGraph, String>>,

    // === Flags that were set ===
    pub session_refetch_requested: bool,
    pub chat_just_finished: bool,
}

/// Flags that were captured from AsyncState for processing
/// These are "needs_*" flags that trigger dependent operations
#[derive(Default)]
pub struct NeedsFlags {
    pub session_refetch: bool,
    pub graph_refetch: bool,
    pub context_refetch: bool,
    pub trading_matrix_refetch: bool,
    pub scope_graph_refetch: bool,
    pub cbu_search_trigger: Option<String>,
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
/// 5. update() calls process_async_results() which extracts PendingResults and processes
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

    // Entity disambiguation (from agent chat when entity is ambiguous)
    /// Disambiguation request from chat response
    pub pending_disambiguation: Option<ob_poc_types::DisambiguationRequest>,
    /// Search results for disambiguation
    pub pending_disambiguation_results: Option<Result<Vec<ob_poc_types::EntityMatch>, String>>,
    /// Loading flag for disambiguation search
    pub loading_disambiguation: bool,

    // Verb disambiguation (from agent chat when verb is ambiguous)
    /// Verb disambiguation request from chat response
    pub pending_verb_disambiguation: Option<ob_poc_types::VerbDisambiguationRequest>,
    /// Result from verb selection API call
    pub pending_verb_selection_result: Option<Result<ob_poc_types::ChatResponse, String>>,
    /// Loading flag for verb selection
    pub loading_verb_selection: bool,

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
    /// DSL hash for resolution commit verification (Issue K)
    pub pending_dsl_hash: Option<String>,

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

    // Ring navigation (cluster view - CBUs orbiting ManCo)
    pub pending_ring_out: bool,
    pub pending_ring_in: bool,
    pub pending_clockwise: Option<u32>,        // steps
    pub pending_counterclockwise: Option<u32>, // steps
    pub pending_snap_to: Option<String>,       // target CBU name

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

impl AsyncState {
    /// Extract all pending results in one atomic operation.
    ///
    /// This allows the caller to drop the lock before processing,
    /// enabling full `&mut self` access during processing.
    pub fn extract_pending(&mut self) -> PendingResults {
        PendingResults {
            // Results that need &mut self processing
            cbu_lookup: self.pending_cbu_lookup.take(),

            // Results that update AppState fields
            session: self.pending_session.take(),
            session_id: self.pending_session_id.take(),
            version_check: self.pending_version_check.take(),
            watch: self.pending_watch.take(),
            graph: self.pending_graph.take(),
            scope_graph: self.pending_scope_graph.take(),
            validation: self.pending_validation.take(),
            execution: self.pending_execution.take(),
            cbu_list: self.pending_cbu_list.take(),
            chat: self.pending_chat.take(),
            chat_input: self.pending_chat_input.take(),
            resolution: self.pending_resolution.take(),
            resolution_search: self.pending_resolution_search.take(),
            cbu_search: self.pending_cbu_search.take(),
            session_context: self.pending_session_context.take(),
            trading_matrix: self.pending_trading_matrix.take(),
            service_taxonomy: self.pending_service_taxonomy.take(),
            investor_register: self.pending_investor_register.take(),
            investor_list: self.pending_investor_list.take(),

            // Entity disambiguation
            disambiguation: self.pending_disambiguation.take(),
            disambiguation_results: self.pending_disambiguation_results.take(),

            // Verb disambiguation
            verb_disambiguation: self.pending_verb_disambiguation.take(),

            // Unresolved refs
            unresolved_refs: self.pending_unresolved_refs.take(),
            current_ref_index: self.pending_current_ref_index.take(),
            dsl_hash: self.pending_dsl_hash.take(),

            // Command triggers
            search_cbu_query: self.pending_search_cbu_query.take(),

            // Taxonomy
            taxonomy_breadcrumbs_response: self.pending_taxonomy_breadcrumbs_response.take(),
            taxonomy_zoom_response: self.pending_taxonomy_zoom_response.take(),

            // Galaxy/Universe
            universe_graph: self.pending_universe_graph.take(),

            // Flags - take and reset
            session_refetch_requested: std::mem::take(&mut self.pending_session_refetch),
            chat_just_finished: std::mem::take(&mut self.chat_just_finished),
        }
    }

    /// Update loading flags after extraction (separate from extract to keep atomicity)
    pub fn clear_loading_flags_for_extracted(&mut self) {
        // These correspond to the pending results we extracted
        self.loading_session = false;
        self.loading_disambiguation = false;
        self.loading_cbu_lookup = false;
        self.loading_taxonomy = false;
        self.loading_universe = false;
    }
}

// =============================================================================
// STATE PROCESSING - Called at start of each frame
// =============================================================================

impl AppState {
    /// Process pending async results at the start of each frame
    ///
    /// This is the ONLY place where async results flow into AppState.
    /// Pattern: extract all pending → drop lock → process with full &mut self.
    pub fn process_async_results(&mut self) {
        // Extract all pending results while holding the lock
        let (pending, mut needs_flags) = {
            let mut state = match self.async_state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };

            let pending = state.extract_pending();

            // Capture flags that need to be processed
            let needs = NeedsFlags {
                session_refetch: state.needs_session_refetch,
                graph_refetch: state.needs_graph_refetch,
                context_refetch: state.needs_context_refetch,
                trading_matrix_refetch: state.needs_trading_matrix_refetch,
                scope_graph_refetch: state.needs_scope_graph_refetch,
                cbu_search_trigger: state.needs_cbu_search_trigger.take(),
            };

            (pending, needs)
        };
        // Lock is now dropped - we have full &mut self access

        // Make pending mutable so we can take values
        let mut pending = pending;

        // Process results that need &mut self methods first (take ownership before passing pending)
        if let Some(result) = pending.cbu_lookup.take() {
            match result {
                Ok((uuid, display_name)) => {
                    self.select_cbu(uuid, &display_name);
                }
                Err(e) => {
                    if let Ok(mut state) = self.async_state.lock() {
                        state.last_error = Some(e);
                    }
                }
            }
        }

        // Now process remaining results (same as before, but lock-free)
        self.process_pending_results(pending, &mut needs_flags);
    }

    /// Process the extracted pending results (called after lock is dropped)
    fn process_pending_results(&mut self, pending: PendingResults, _needs: &mut NeedsFlags) {
        // Helper to set error
        let set_error = |async_state: &Arc<Mutex<AsyncState>>, msg: String| {
            if let Ok(mut state) = async_state.lock() {
                state.last_error = Some(msg);
            }
        };

        let async_state = &self.async_state;

        // Process new session creation
        if let Some(session_id) = pending.session_id {
            self.session_id = Some(session_id);
        }

        // Process session fetch
        if let Some(result) = pending.session {
            if let Ok(mut state) = async_state.lock() {
                state.loading_session = false;
            }
            match result {
                Ok(session) => {
                    // Sync DSL editor from session.combined_dsl if server has content and we're not dirty
                    if !self.buffers.dsl_dirty {
                        if let Some(ref dsl) = session.combined_dsl {
                            self.buffers.dsl_editor = dsl.clone();
                            self.buffers.last_agent_dsl = Some(dsl.clone());
                        }
                    }
                    self.last_known_version = session.version.clone();
                    self.session = Some(session);
                }
                Err(e) => set_error(async_state, format!("Session fetch failed: {}", e)),
            }
        }

        // Process version check
        if let Some(result) = pending.version_check {
            if let Ok(mut state) = async_state.lock() {
                state.checking_version = false;
            }
            if let Ok(server_version) = result {
                if let Some(ref known_version) = self.last_known_version {
                    if &server_version != known_version {
                        web_sys::console::log_1(
                            &format!(
                                "Session version changed: {} -> {}, triggering refetch",
                                known_version, server_version
                            )
                            .into(),
                        );
                        if let Ok(mut state) = async_state.lock() {
                            state.needs_session_refetch = true;
                            state.needs_graph_refetch = true;
                        }
                    }
                }
            }
        }

        // Process watch result
        if let Some(result) = pending.watch {
            if let Ok(mut state) = async_state.lock() {
                state.watching_session = false;
            }
            match result {
                Ok(watch_response) => {
                    let version_str = watch_response.version.to_string();
                    let changed = self
                        .last_known_version
                        .as_ref()
                        .map(|v| v != &version_str)
                        .unwrap_or(true);

                    if changed {
                        web_sys::console::log_1(
                            &format!(
                                "Session watch: version changed to {}, triggering refetch",
                                watch_response.version
                            )
                            .into(),
                        );
                        self.last_known_version = Some(version_str);
                        if let Ok(mut state) = async_state.lock() {
                            state.needs_session_refetch = true;
                            if watch_response.active_cbu_id.is_some() {
                                state.needs_graph_refetch = true;
                                state.pending_cbu_id = watch_response.active_cbu_id;
                            }
                            // Differentiate scope types: single-CBU triggers graph refetch,
                            // multi-CBU scopes (book, jurisdiction, neighborhood) trigger scope_graph refetch
                            if let Some(ref scope_type) = watch_response.scope_type {
                                match scope_type.as_str() {
                                    "cbu" => {
                                        // Single CBU - use regular graph fetch
                                        state.needs_graph_refetch = true;
                                    }
                                    "book" | "jurisdiction" | "neighborhood" | "custom" => {
                                        // Multi-CBU scopes - use scope graph fetch
                                        state.needs_scope_graph_refetch = true;
                                    }
                                    "empty" => {
                                        // No scope set, nothing to fetch
                                    }
                                    _ => {
                                        // Unknown scope type, default to scope graph
                                        state.needs_scope_graph_refetch = true;
                                    }
                                }
                            }
                        }
                    }

                    if let Some(ref scope_type) = watch_response.scope_type {
                        self.current_scope = Some(CurrentScope {
                            scope_type: scope_type.clone(),
                            scope_path: watch_response.scope_path.clone(),
                            is_loaded: watch_response.scope_loaded,
                        });
                    }
                }
                Err(e) => {
                    if !e.contains("timeout") && !e.contains("Timeout") {
                        web_sys::console::warn_1(&format!("Session watch failed: {}", e).into());
                    }
                }
            }
        }

        // Process graph fetch
        if let Some(result) = pending.graph {
            if let Ok(mut state) = async_state.lock() {
                state.loading_graph = false;
            }
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

                    if let Some(layout_graph) = self.graph_widget.get_layout_graph() {
                        self.entity_ontology.populate_counts(layout_graph);
                    }
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("Graph fetch failed: {}", e).into());
                    set_error(async_state, format!("Graph fetch failed: {}", e));
                }
            }
        }

        // Process scope graph fetch (multi-CBU session graph)
        if let Some(result) = pending.scope_graph {
            match result {
                Ok(data) => {
                    web_sys::console::log_1(
                        &format!(
                            "process_async_results: scope graph received for {} CBUs",
                            data.cbu_count
                        )
                        .into(),
                    );

                    // Handle view level based on CBU count
                    if data.cbu_count > 1 {
                        // Multi-CBU: show cluster view
                        use ob_poc_types::galaxy::RiskRating;
                        let manco = ManCoData {
                            entity_id: Uuid::nil(),
                            name: "Governance Controller".to_string(),
                            short_name: "GC".to_string(),
                            jurisdiction: None,
                        };
                        // Build a lookup map from cbu_list for names and jurisdiction
                        let cbu_lookup: std::collections::HashMap<Uuid, &ob_poc_types::CbuSummary> =
                            self.cbu_list
                                .iter()
                                .filter_map(|cbu| {
                                    Uuid::parse_str(&cbu.cbu_id).ok().map(|id| (id, cbu))
                                })
                                .collect();

                        let cbus: Vec<ClusterCbuData> = data
                            .cbu_ids
                            .iter()
                            .enumerate()
                            .map(|(i, cbu_id)| {
                                // Look up CBU info from cbu_list
                                let (name, short_name, jurisdiction) =
                                    if let Some(cbu_info) = cbu_lookup.get(cbu_id) {
                                        let name = cbu_info.name.clone();
                                        // Generate short name: first 3 chars uppercase or abbreviation
                                        let short = if name.len() <= 4 {
                                            name.to_uppercase()
                                        } else {
                                            // Take first letter of each word, up to 3
                                            name.split_whitespace()
                                                .filter_map(|w| w.chars().next())
                                                .take(3)
                                                .collect::<String>()
                                                .to_uppercase()
                                        };
                                        let short_name = if short.is_empty() {
                                            format!("C{}", i + 1)
                                        } else {
                                            short
                                        };
                                        (name, short_name, cbu_info.jurisdiction.clone())
                                    } else {
                                        // Fallback if not found in cbu_list
                                        (format!("CBU {}", i + 1), format!("C{}", i + 1), None)
                                    };
                                ClusterCbuData {
                                    cbu_id: *cbu_id,
                                    name,
                                    short_name,
                                    jurisdiction,
                                    risk_rating: RiskRating::Medium,
                                    entity_count: 0,
                                }
                            })
                            .collect();
                        self.cluster_view.load_data(manco, cbus);
                        self.view_level = ob_poc_types::galaxy::ViewLevel::Cluster;
                    } else if data.cbu_count == 1 {
                        // Single CBU: show system view (CBU graph)
                        // Graph data is already in data.graph - just set the view level
                        if let Some(cbu_id) = data.cbu_ids.first() {
                            web_sys::console::log_1(
                                &format!(
                                    "process_async_results: single CBU {}, setting System view",
                                    cbu_id
                                )
                                .into(),
                            );
                            // Queue CBU selection for the update loop to process
                            if let Ok(mut async_state) = self.async_state.lock() {
                                async_state.pending_cbu_id = Some(*cbu_id);
                                async_state.needs_graph_refetch = false; // Graph already in response
                            }
                            self.view_level = ob_poc_types::galaxy::ViewLevel::System;
                        }
                    }

                    if let Some(graph_data) = data.graph {
                        self.graph_widget.set_data(graph_data.clone());
                        self.graph_data = Some(graph_data);

                        if let Some(layout_graph) = self.graph_widget.get_layout_graph() {
                            self.entity_ontology.populate_counts(layout_graph);
                        }
                    }
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("Scope graph fetch failed: {}", e).into());
                    set_error(async_state, format!("Scope graph fetch failed: {}", e));
                }
            }
        }

        // Process universe graph fetch (galaxy view data)
        if let Some(result) = pending.universe_graph {
            match result {
                Ok(universe) => {
                    web_sys::console::log_1(
                        &format!(
                            "process_async_results: universe graph received with {} clusters",
                            universe.clusters.len()
                        )
                        .into(),
                    );
                    // Update galaxy view with new data
                    self.galaxy_view.set_universe_data(&universe);
                    self.universe_graph = Some(universe);
                }
                Err(e) => {
                    web_sys::console::error_1(
                        &format!("process_async_results: universe fetch failed: {}", e).into(),
                    );
                    set_error(async_state, format!("Universe fetch failed: {}", e));
                }
            }
        }

        // Process validation
        if let Some(result) = pending.validation {
            match result {
                Ok(response) => self.validation_result = Some(response),
                Err(e) => set_error(async_state, e),
            }
        }

        // Process chat response
        if let Some(result) = pending.chat {
            if let Ok(mut state) = async_state.lock() {
                state.loading_chat = false;
                state.chat_just_finished = true;
            }
            match result {
                Ok(msg) => self.messages.push(msg),
                Err(e) => set_error(async_state, e),
            }
        }

        // Chat input restoration
        if let Some(dsl_source) = pending.chat_input {
            self.buffers.chat_input = dsl_source;
        }

        // Process verb disambiguation request (earlier in pipeline than entity disambiguation)
        if let Some(verb_disambig) = pending.verb_disambiguation {
            web_sys::console::log_1(
                &format!(
                    "process_async_results: verb disambiguation with {} options",
                    verb_disambig.options.len()
                )
                .into(),
            );

            // Get current time for timeout tracking
            let current_time = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now() / 1000.0) // Convert to seconds
                .unwrap_or(0.0);

            self.verb_disambiguation_ui.set_from_response(
                verb_disambig.clone(),
                verb_disambig.original_input.clone(),
                current_time,
            );
        }

        // Process entity disambiguation request
        if let Some(disambig) = pending.disambiguation {
            web_sys::console::log_1(
                &format!(
                    "process_async_results: opening entity disambiguation modal for {} items",
                    disambig.items.len()
                )
                .into(),
            );

            let window = WindowEntry {
                id: format!("disambig-{}", disambig.request_id),
                window_type: WindowType::Resolution,
                layer: 2,
                modal: true,
                data: Some(WindowData::Disambiguation {
                    request: disambig.clone(),
                    current_item_index: 0,
                    search_results: None,
                }),
            };
            self.window_stack.push(window);

            if let Some(ob_poc_types::DisambiguationItem::EntityMatch {
                ref search_text, ..
            }) = disambig.items.first()
            {
                self.resolution_ui.search_query = search_text.clone();
            }
        }

        // Process disambiguation search results
        if let Some(result) = pending.disambiguation_results {
            if let Ok(mut state) = async_state.lock() {
                state.loading_disambiguation = false;
            }
            match result {
                Ok(matches) => {
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
                Err(e) => set_error(async_state, format!("Disambiguation search failed: {}", e)),
            }
        }

        // Process unresolved refs from ChatResponse
        if let Some(refs) = pending.unresolved_refs {
            let current_index = pending.current_ref_index.unwrap_or(0);

            web_sys::console::log_1(
                &format!(
                    "process_async_results: opening resolution modal for {} refs, dsl_hash={:?}",
                    refs.len(),
                    pending.dsl_hash
                )
                .into(),
            );

            self.resolution_ui.show_panel = true;
            // Store dsl_hash for resolution commit verification (Issue K)
            self.resolution_ui.dsl_hash = pending.dsl_hash.clone();

            if let Some(current_ref) = refs.get(current_index) {
                self.resolution_ui.current_ref = Some(current_ref.clone());
                self.resolution_ui.current_entity_type = Some(current_ref.entity_type.clone());
                self.resolution_ui.search_keys = current_ref.search_keys.clone();
                self.resolution_ui.discriminator_fields = current_ref.discriminator_fields.clone();
                self.resolution_ui.resolution_mode = current_ref.resolution_mode.clone();
                self.resolution_ui.search_query = current_ref.search_value.clone();
                self.resolution_ui.search_results = None;
            }
        }

        // Process session refetch flag
        if pending.session_refetch_requested {
            if let Ok(mut state) = async_state.lock() {
                state.needs_session_refetch = true;
                state.needs_graph_refetch = true;
                state.needs_trading_matrix_refetch = true;
                state.needs_context_refetch = true;
            }
        }

        // Process execution
        if let Some(result) = pending.execution {
            if let Ok(mut state) = async_state.lock() {
                state.executing = false;
            }
            match result {
                Ok(execution) => self.execution = Some(execution),
                Err(e) => set_error(async_state, e),
            }
        }

        // Process CBU list
        if let Some(result) = pending.cbu_list {
            match result {
                Ok(list) => self.cbu_list = list,
                Err(e) => set_error(async_state, e),
            }
        }

        // Process resolution session
        if let Some(result) = pending.resolution {
            if let Ok(mut state) = async_state.lock() {
                state.loading_resolution = false;
            }
            match result {
                Ok(resolution) => {
                    if !resolution.unresolved.is_empty() {
                        self.resolution_ui.show_panel = true;
                    }
                    self.resolution = Some(resolution);
                }
                Err(e) => set_error(async_state, format!("Resolution failed: {}", e)),
            }
        }

        // Process resolution search results
        if let Some(result) = pending.resolution_search {
            if let Ok(mut state) = async_state.lock() {
                state.searching_resolution = false;
            }
            match result {
                Ok(search_result) => {
                    self.resolution_ui.search_results = Some(search_result);
                }
                Err(e) => set_error(async_state, format!("Resolution search failed: {}", e)),
            }
        }

        // Process CBU search results
        if let Some(result) = pending.cbu_search {
            self.cbu_search_ui.searching = false;
            match result {
                Ok(search_result) => {
                    self.cbu_search_ui.results = Some(search_result);
                }
                Err(e) => set_error(async_state, format!("CBU search failed: {}", e)),
            }
        }

        // Process pending CBU search popup request
        if let Some(query) = pending.search_cbu_query {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(
                &format!("Opening CBU search popup with query: {}", query).into(),
            );
            self.cbu_search_ui.results = None;
            self.cbu_search_ui.query = query.clone();
            self.cbu_search_ui.open = true;
            self.cbu_search_ui.just_opened = true;
            if let Ok(mut state) = async_state.lock() {
                state.needs_cbu_search_trigger = Some(query);
            }
        }

        // Process session context
        if let Some(result) = pending.session_context {
            if let Ok(mut state) = async_state.lock() {
                state.loading_session_context = false;
            }
            match result {
                Ok(context) => {
                    if let Some(ref viewport_state) = context.viewport_state {
                        self.graph_widget.apply_viewport_state(viewport_state);
                    }
                    self.session_context = Some(context);
                }
                Err(e) => set_error(async_state, format!("Session context fetch failed: {}", e)),
            }
        }

        // Process trading matrix
        if let Some(result) = pending.trading_matrix {
            if let Ok(mut state) = async_state.lock() {
                state.loading_trading_matrix = false;
            }
            match result {
                Ok(matrix) => {
                    self.trading_matrix_state.expand_first_level(&matrix);
                    self.trading_matrix = Some(matrix);
                }
                Err(e) => set_error(async_state, format!("Trading matrix fetch failed: {}", e)),
            }
        }

        // Process service taxonomy
        if let Some(result) = pending.service_taxonomy {
            if let Ok(mut state) = async_state.lock() {
                state.loading_service_taxonomy = false;
            }
            match result {
                Ok(taxonomy) => {
                    self.service_taxonomy_state
                        .expand_to_depth(&taxonomy.root, 1);
                    self.service_taxonomy = Some(taxonomy);
                }
                Err(e) => set_error(async_state, format!("Service taxonomy fetch failed: {}", e)),
            }
        }

        // Process investor register
        if let Some(result) = pending.investor_register {
            if let Ok(mut state) = async_state.lock() {
                state.loading_investor_register = false;
            }
            match result {
                Ok(register) => {
                    if !register.control_holders.is_empty() || register.aggregate.is_some() {
                        self.investor_register_ui.show_panel = true;
                    }
                    self.investor_register = Some(register);
                }
                Err(e) => set_error(
                    async_state,
                    format!("Investor register fetch failed: {}", e),
                ),
            }
        }

        // Process investor list
        if let Some(result) = pending.investor_list {
            if let Ok(mut state) = async_state.lock() {
                state.loading_investor_list = false;
            }
            match result {
                Ok(list) => self.investor_list = Some(list),
                Err(e) => set_error(async_state, format!("Investor list fetch failed: {}", e)),
            }
        }

        // Process taxonomy breadcrumbs response
        if let Some(result) = pending.taxonomy_breadcrumbs_response {
            if let Ok(mut state) = async_state.lock() {
                state.loading_taxonomy = false;
            }
            match result {
                Ok(response) => {
                    self.taxonomy_breadcrumbs = response
                        .breadcrumbs
                        .into_iter()
                        .map(|b| (b.label, b.type_code))
                        .collect();
                }
                Err(e) => set_error(
                    async_state,
                    format!("Taxonomy breadcrumbs fetch failed: {}", e),
                ),
            }
        }

        // Process taxonomy zoom response
        if let Some(result) = pending.taxonomy_zoom_response {
            if let Ok(mut state) = async_state.lock() {
                state.loading_taxonomy = false;
            }
            match result {
                Ok(response) => {
                    if response.success {
                        self.taxonomy_breadcrumbs = response
                            .breadcrumbs
                            .into_iter()
                            .map(|b| (b.label, b.type_code))
                            .collect();
                    } else if let Some(error) = response.error {
                        set_error(
                            async_state,
                            format!("Taxonomy navigation failed: {}", error),
                        );
                    }
                }
                Err(e) => set_error(async_state, format!("Taxonomy navigation failed: {}", e)),
            }
        }

        // Set chat_just_finished flag
        if pending.chat_just_finished {
            if let Ok(mut state) = async_state.lock() {
                state.chat_just_finished = true;
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

    // Ring navigation (cluster view)
    pub fn take_pending_ring_out(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_ring_out;
            state.pending_ring_out = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_ring_in(&self) -> bool {
        if let Ok(mut state) = self.async_state.lock() {
            let pending = state.pending_ring_in;
            state.pending_ring_in = false;
            pending
        } else {
            false
        }
    }

    pub fn take_pending_clockwise(&self) -> Option<u32> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_clockwise.take()
        } else {
            None
        }
    }

    pub fn take_pending_counterclockwise(&self) -> Option<u32> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_counterclockwise.take()
        } else {
            None
        }
    }

    pub fn take_pending_snap_to(&self) -> Option<String> {
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_snap_to.take()
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

    // NOTE: take_pending_universe_graph() removed - universe_graph is now processed
    // in process_async_results() to avoid race condition with extract_pending().

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

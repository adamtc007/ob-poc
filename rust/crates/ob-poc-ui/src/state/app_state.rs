//! Application State
//!
//! Central state management following egui architecture patterns.
//! See EGUI_ARCHITECTURE_PATTERN.MD for design rationale.
//!
//! ## Structure
//!
//! - **DomainState**: Business data (CBUs, sessions, DSL, chat messages)
//! - **UiState**: Interaction state (modals, view settings, forms)
//! - **AppEvent**: User intent (what the user did)
//! - **AppCommand**: Domain operations (IO, background work)
//! - **TaskStatus**: Background task lifecycle tracking

use super::{
    CbuSummary, ChatMessage, EntityMatch, Orientation, PendingState, SessionContext,
    SimpleAstStatement,
};
use crate::graph::{CbuGraphData, LayoutOverride, ViewMode};
use uuid::Uuid;

// =============================================================================
// TOP-LEVEL STATE SPLIT: Domain vs UI
// =============================================================================

/// Domain state - business data and DSL results.
///
/// This holds persistent business state that the UI displays.
/// Changes to domain state typically come from API responses or user actions.
#[derive(Default)]
pub struct DomainState {
    // -------------------------------------------------------------------------
    // Session & CBU Context
    // -------------------------------------------------------------------------
    /// Current session context (session ID, selected CBU, bindings)
    pub session: SessionContext,

    /// Currently selected CBU ID
    pub selected_cbu: Option<Uuid>,

    /// List of all available CBUs
    pub cbu_list: Vec<CbuSummary>,

    // -------------------------------------------------------------------------
    // Chat & DSL State
    // -------------------------------------------------------------------------
    /// Chat message history
    pub messages: Vec<ChatMessage>,

    /// Current DSL source text
    pub dsl_source: String,

    /// Parsed AST statements for display
    pub ast_statements: Vec<SimpleAstStatement>,

    /// Pending confirmation state (DSL awaiting execution, etc.)
    pub pending: PendingState,

    // -------------------------------------------------------------------------
    // Background Tasks
    // -------------------------------------------------------------------------
    /// Background task status tracking
    pub tasks: BackgroundTasks,

    // -------------------------------------------------------------------------
    // Pending Commands
    // -------------------------------------------------------------------------
    /// Commands to execute (populated by handle_event, consumed by execute_commands)
    pub pending_commands: Vec<AppCommand>,
}

/// UI state - interaction and visual state.
///
/// This holds state about how we view and interact with domain data.
#[derive(Default)]
pub struct UiState {
    // -------------------------------------------------------------------------
    // View Settings
    // -------------------------------------------------------------------------
    /// Current view mode for graph visualization
    pub view_mode: ViewMode,

    /// Current layout orientation
    pub orientation: Orientation,

    // -------------------------------------------------------------------------
    // Modals & Overlays
    // -------------------------------------------------------------------------
    /// Currently active modal (if any)
    pub modal: Modal,

    // -------------------------------------------------------------------------
    // Global UI State
    // -------------------------------------------------------------------------
    /// Global loading indicator
    pub loading: bool,

    /// Global error message
    pub error: Option<String>,

    // -------------------------------------------------------------------------
    // Graph View State
    // -------------------------------------------------------------------------
    /// Deferred layout to apply after graph loads
    pub deferred_layout: Option<LayoutOverride>,

    /// Timestamp when layout became dirty (for debounced saves)
    pub layout_dirty_since: Option<f64>,
}

// =============================================================================
// MODAL STATE - Single enum for all overlays
// =============================================================================

/// Modal state - represents all possible overlay states.
///
/// Using an enum prevents impossible states like having multiple
/// modals open simultaneously.
#[derive(Debug, Clone, Default)]
pub enum Modal {
    /// No modal is open
    #[default]
    None,

    /// Entity finder modal for resolving EntityRefs
    EntityFinder {
        entity_type: String,
        search_text: String,
        statement_idx: usize,
        arg_key: String,
    },

    /// CBU picker modal for selecting a CBU
    CbuPicker,

    /// Error dialog
    Error(String),

    /// Confirmation dialog
    Confirm {
        title: String,
        message: String,
        on_confirm: ConfirmAction,
    },
}

/// Actions that can be confirmed via the confirmation modal
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    ExecuteDsl,
    // Add more confirmable actions as needed
}

// =============================================================================
// BACKGROUND TASKS - TaskStatus pattern
// =============================================================================

/// Status of a background task.
///
/// Represents the lifecycle of async operations explicitly in state.
#[derive(Debug, Clone, Default)]
pub enum TaskStatus<T, E> {
    /// No task running
    #[default]
    Idle,
    /// Task is in progress
    InProgress,
    /// Task completed with result
    Finished(Result<T, E>),
}

impl<T, E> TaskStatus<T, E> {
    pub fn is_idle(&self) -> bool {
        matches!(self, TaskStatus::Idle)
    }

    pub fn is_in_progress(&self) -> bool {
        matches!(self, TaskStatus::InProgress)
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, TaskStatus::Finished(_))
    }

    /// Take the result if finished, leaving Idle
    pub fn take_result(&mut self) -> Option<Result<T, E>> {
        match std::mem::take(self) {
            TaskStatus::Finished(result) => Some(result),
            other => {
                *self = other;
                None
            }
        }
    }
}

/// Background task tracking.
///
/// Each async operation has its status tracked here.
/// UI can inspect these to show loading states, results, or errors.
#[derive(Default)]
pub struct BackgroundTasks {
    /// CBU list loading status
    pub cbu_list: TaskStatus<Vec<CbuSummary>, String>,

    /// Graph loading status
    pub graph: TaskStatus<CbuGraphData, String>,

    /// Layout loading status
    pub layout: TaskStatus<LayoutOverride, String>,

    /// Session creation status
    pub session: TaskStatus<Uuid, String>,

    /// Chat response status
    pub chat: TaskStatus<super::types::ChatResponse, String>,

    /// Entity search status
    pub entity_search: TaskStatus<Vec<EntityMatch>, String>,

    /// DSL execution status
    pub dsl_execute: TaskStatus<(), String>,

    /// Layout save status (fire-and-forget, but track for debugging)
    pub layout_save: TaskStatus<(), String>,
}

impl BackgroundTasks {
    /// Check if any task is in progress
    pub fn any_in_progress(&self) -> bool {
        self.cbu_list.is_in_progress()
            || self.graph.is_in_progress()
            || self.layout.is_in_progress()
            || self.session.is_in_progress()
            || self.chat.is_in_progress()
            || self.entity_search.is_in_progress()
            || self.dsl_execute.is_in_progress()
    }

    /// Check if graph or layout is loading (need both to render)
    pub fn loading_graph_view(&self) -> bool {
        self.graph.is_in_progress() || self.layout.is_in_progress()
    }
}

// =============================================================================
// APP EVENT - User Intent
// =============================================================================

/// Application events representing user intent.
///
/// UI code emits events rather than mutating state directly.
/// Events are processed by `handle_event` which may emit `AppCommand`s.
#[derive(Debug, Clone)]
pub enum AppEvent {
    // -------------------------------------------------------------------------
    // CBU Events
    // -------------------------------------------------------------------------
    /// Select a CBU by ID
    SelectCbu(Uuid),

    /// Refresh the CBU list from server
    RefreshCbuList,

    /// Open the CBU picker modal
    OpenCbuPicker,

    // -------------------------------------------------------------------------
    // View Events
    // -------------------------------------------------------------------------
    /// Change view mode
    SetViewMode(ViewMode),

    /// Change layout orientation
    SetOrientation(Orientation),

    // -------------------------------------------------------------------------
    // Session Events
    // -------------------------------------------------------------------------
    /// Create a new session
    CreateSession,

    // -------------------------------------------------------------------------
    // Chat Events
    // -------------------------------------------------------------------------
    /// Send a chat message
    SendMessage(String),

    /// Execute pending DSL
    ExecutePendingDsl,

    /// Cancel pending DSL
    CancelPendingDsl,

    // -------------------------------------------------------------------------
    // Entity Resolution Events
    // -------------------------------------------------------------------------
    /// Open entity finder for an unresolved ref
    OpenEntityFinder {
        entity_type: String,
        search_text: String,
        statement_idx: usize,
        arg_key: String,
    },

    /// Search for entities (from within entity finder)
    SearchEntities { entity_type: String, query: String },

    /// Entity was selected in finder
    EntitySelected {
        statement_idx: usize,
        arg_key: String,
        entity: EntityMatch,
    },

    // -------------------------------------------------------------------------
    // Modal Events
    // -------------------------------------------------------------------------
    /// Close any open modal
    CloseModal,

    /// Show error modal
    ShowError(String),

    /// Confirm action was accepted
    ConfirmAccepted(ConfirmAction),

    // -------------------------------------------------------------------------
    // Task Result Events (from background task completion)
    // -------------------------------------------------------------------------
    /// CBU list loaded
    CbuListLoaded(Result<Vec<CbuSummary>, String>),

    /// Session created
    SessionCreated(Result<Uuid, String>),

    /// Chat response received
    ChatResponseReceived(Result<super::types::ChatResponse, String>),

    /// Entity search completed
    EntitySearchCompleted(Result<Vec<EntityMatch>, String>),

    /// Graph loaded
    GraphLoaded(Result<CbuGraphData, String>),

    /// Layout loaded
    LayoutLoaded(Result<LayoutOverride, String>),

    // -------------------------------------------------------------------------
    // Misc Events
    // -------------------------------------------------------------------------
    /// Copy DSL to clipboard
    CopyDsl,

    /// Clear error message
    ClearError,

    /// Add a system message to chat
    SystemMessage {
        text: String,
        level: super::SystemLevel,
    },
}

// =============================================================================
// APP COMMAND - Domain Operations / IO
// =============================================================================

/// Application commands for domain operations and IO.
///
/// Commands are emitted by `handle_event` and executed by `execute_commands`.
/// This separates "what to do" from "how to do it" (the IO layer).
#[derive(Debug, Clone)]
pub enum AppCommand {
    // -------------------------------------------------------------------------
    // API Calls
    // -------------------------------------------------------------------------
    /// Load CBU list from server
    LoadCbuList,

    /// Load CBU graph view
    LoadCbuGraph {
        cbu_id: Uuid,
        view_mode: ViewMode,
        orientation: Orientation,
    },

    /// Load layout overrides for CBU
    LoadLayout { cbu_id: Uuid, view_mode: ViewMode },

    /// Save layout overrides
    SaveLayout {
        cbu_id: Uuid,
        view_mode: ViewMode,
        overrides: LayoutOverride,
    },

    /// Create a new session
    CreateSession,

    /// Send chat message
    SendChatMessage { session_id: Uuid, message: String },

    /// Search for entities
    SearchEntities { entity_type: String, query: String },

    /// Execute DSL
    ExecuteDsl { session_id: Uuid, dsl: String },

    // -------------------------------------------------------------------------
    // Clipboard
    // -------------------------------------------------------------------------
    /// Copy text to clipboard
    CopyToClipboard(String),
}

// =============================================================================
// HELPER IMPLS
// =============================================================================

impl DomainState {
    /// Create a new DomainState with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently selected CBU summary, if any
    pub fn selected_cbu_summary(&self) -> Option<&CbuSummary> {
        self.selected_cbu
            .and_then(|id| self.cbu_list.iter().find(|c| c.cbu_id == id))
    }

    /// Check if we have an active session
    pub fn has_session(&self) -> bool {
        self.session.session_id.is_some()
    }
}

impl UiState {
    /// Create a new UiState with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any modal is currently open
    pub fn has_modal_open(&self) -> bool {
        !matches!(self.modal, Modal::None)
    }

    /// Check if entity finder modal is open
    pub fn is_entity_finder_open(&self) -> bool {
        matches!(self.modal, Modal::EntityFinder { .. })
    }

    /// Check if CBU picker modal is open
    pub fn is_cbu_picker_open(&self) -> bool {
        matches!(self.modal, Modal::CbuPicker)
    }
}

impl Modal {
    /// Check if this is the None variant
    pub fn is_none(&self) -> bool {
        matches!(self, Modal::None)
    }

    /// Get entity finder context if open
    pub fn entity_finder_context(&self) -> Option<(&str, &str, usize, &str)> {
        match self {
            Modal::EntityFinder {
                entity_type,
                search_text,
                statement_idx,
                arg_key,
            } => Some((entity_type, search_text, *statement_idx, arg_key)),
            _ => None,
        }
    }
}

// =============================================================================
// LEGACY COMPAT - AppState wrapper for gradual migration
// =============================================================================

/// Combined application state (wraps Domain + UI for convenience).
///
/// This provides a unified view for code that hasn't been fully migrated
/// to the split DomainState/UiState pattern.
pub struct AppState {
    pub domain: DomainState,
    pub ui: UiState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            domain: DomainState::new(),
            ui: UiState::new(),
        }
    }

    // Convenience accessors for gradual migration

    pub fn has_modal_open(&self) -> bool {
        self.ui.has_modal_open()
    }

    pub fn has_pending_async(&self) -> bool {
        self.domain.tasks.any_in_progress()
    }

    pub fn selected_cbu_summary(&self) -> Option<&CbuSummary> {
        self.domain.selected_cbu_summary()
    }
}

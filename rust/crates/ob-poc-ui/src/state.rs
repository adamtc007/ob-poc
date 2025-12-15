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

use ob_poc_graph::{CbuGraphData, CbuGraphWidget, ViewMode};
use ob_poc_types::{CbuSummary, ExecuteResponse, SessionStateResponse, ValidateDslResponse};
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
    /// Set via refetch_graph(), never modified directly
    pub graph_data: Option<CbuGraphData>,

    /// Last validation errors (empty = valid)
    /// These are plain strings from the validation API
    pub validation_result: Option<ValidateDslResponse>,

    /// Last execution result
    pub execution: Option<ExecuteResponse>,

    /// Chat messages (accumulated from ChatResponse)
    pub messages: Vec<ChatMessage>,

    /// Available CBUs for selector dropdown
    pub cbu_list: Vec<CbuSummary>,

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

    /// Graph widget (owns camera, input state - rendering only)
    pub graph_widget: CbuGraphWidget,

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
            messages: Vec::new(),
            cbu_list: Vec::new(),

            // UI-only state
            buffers: TextBuffers::default(),
            view_mode: ViewMode::KycUbo,
            panels: PanelState::default(),
            selected_entity_id: None,
            graph_widget: CbuGraphWidget::new(),

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
    EditorFocus, // Large DSL editor + small panels
    GraphFocus,  // Large graph + small panels
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

    // Command triggers (from agent commands)
    pub pending_execute: Option<Uuid>, // Session ID to execute
    pub pending_undo: Option<Uuid>,    // Session ID to undo
    pub pending_clear: Option<Uuid>,   // Session ID to clear
    pub pending_delete: Option<(Uuid, u32)>, // Session ID + index to delete

    // Execution tracking - prevents repeated refetch
    pub execution_handled: bool,

    // Loading flags (for spinners)
    pub loading_session: bool,
    pub loading_graph: bool,
    pub loading_chat: bool,
    pub executing: bool,

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
                    self.graph_widget.set_data(data.clone());
                    self.graph_data = Some(data);
                }
                Err(e) => {
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
                    // Note: refetch_graph() and refetch_session() should be called
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

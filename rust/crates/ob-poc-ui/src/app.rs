//! OB-POC Application - 4-Panel Layout
//!
//! Layout:
//! ┌─────────────────┬─────────────────┐
//! │  Graph (TL)     │  DSL Source (TR)│
//! │                 │                 │
//! ├─────────────────┼─────────────────┤
//! │  Chat (BL)      │  AST (BR)       │
//! │                 │                 │
//! └─────────────────┴─────────────────┘
//!
//! ## Architecture
//!
//! This module follows the egui architecture pattern documented in
//! `EGUI_ARCHITECTURE_PATTERN.MD`:
//!
//! - **DomainState**: Business data (CBUs, sessions, DSL, chat)
//! - **UiState**: Interaction state (modals, view settings)
//! - **AppEvent**: User intent (what the user did)
//! - **AppCommand**: Domain operations (IO, background work)
//! - **TaskStatus**: Background task lifecycle
//!
//! Key patterns:
//! - UI emits AppEvents, handle_event mutates state and emits AppCommands
//! - Only execute_commands performs IO
//! - Panels return actions, not mutate state directly

use crate::api::ApiClient;
use crate::graph::{CbuGraphWidget, LayoutOverride, ViewMode};
use crate::modals::{
    CbuPickerModal, CbuPickerResult, EntityFinderModal, EntityFinderResult, ResolveContext,
};
use crate::panels::{AstPanel, ChatPanel, ChatPanelAction, DslPanel, UnresolvedRefClick};
use crate::state::{
    AppCommand, AppEvent, AppState, CbuContext, CbuSummary, ChatMessage, ConfirmAction,
    EntityMatch, MessageStatus, Modal, SimpleAstStatement, SystemLevel, TaskStatus,
};
use eframe::egui;
use egui::{Color32, RichText};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Debounce delay in seconds before saving layout
const LAYOUT_SAVE_DEBOUNCE_SECS: f64 = 1.0;

// =============================================================================
// ASYNC RESULT CHANNELS
// =============================================================================

/// Channels for receiving async results.
///
/// Each task type has its own channel for receiving results from spawned tasks.
#[derive(Default)]
struct AsyncChannels {
    cbu_list: Option<Arc<Mutex<Option<Result<Vec<CbuSummary>, String>>>>>,
    graph: Option<Arc<Mutex<Option<Result<crate::graph::CbuGraphData, String>>>>>,
    layout: Option<Arc<Mutex<Option<Result<LayoutOverride, String>>>>>,
    session: Option<Arc<Mutex<Option<Result<crate::state::types::SessionResponse, String>>>>>,
    chat: Option<Arc<Mutex<Option<Result<crate::state::types::ChatResponse, String>>>>>,
    entity_search: Option<Arc<Mutex<Option<Result<Vec<EntityMatch>, String>>>>>,
}

// =============================================================================
// MAIN APPLICATION
// =============================================================================

/// Main application struct.
///
/// Contains domain state, UI state, and infrastructure (API, widgets, runtime).
pub struct ObPocApp {
    // -------------------------------------------------------------------------
    // State (Domain + UI)
    // -------------------------------------------------------------------------
    /// Combined application state
    state: AppState,

    // -------------------------------------------------------------------------
    // Infrastructure (not state)
    // -------------------------------------------------------------------------
    /// API client for server communication
    api: ApiClient,

    /// Graph widget (stateful for pan/zoom/selection)
    graph_widget: CbuGraphWidget,

    /// Chat panel widget (contains input buffer)
    chat_panel: ChatPanel,

    /// DSL panel widget (stateless)
    dsl_panel: DslPanel,

    /// AST panel widget (stateless)
    ast_panel: AstPanel,

    /// Entity finder modal widget
    entity_finder: EntityFinderModal,

    /// CBU picker modal widget
    cbu_picker: CbuPickerModal,

    /// Async result channels
    channels: AsyncChannels,

    // -------------------------------------------------------------------------
    // Runtime (native only)
    // -------------------------------------------------------------------------
    #[cfg(not(target_arch = "wasm32"))]
    runtime: Arc<tokio::runtime::Runtime>,
}

impl ObPocApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let base_url = {
            web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .unwrap_or_else(|| "http://localhost:3000".to_string())
        };

        #[cfg(not(target_arch = "wasm32"))]
        let base_url = "http://localhost:3000".to_string();

        #[cfg(not(target_arch = "wasm32"))]
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime"),
        );

        let mut app = Self {
            state: AppState::new(),
            api: ApiClient::new(&base_url),
            graph_widget: CbuGraphWidget::new(),
            chat_panel: ChatPanel::new(),
            dsl_panel: DslPanel::new(),
            ast_panel: AstPanel::new(),
            entity_finder: EntityFinderModal::new(),
            cbu_picker: CbuPickerModal::new(),
            channels: AsyncChannels::default(),
            #[cfg(not(target_arch = "wasm32"))]
            runtime,
        };

        // Initial command: load CBU list
        app.state
            .domain
            .pending_commands
            .push(AppCommand::LoadCbuList);
        app
    }

    // =========================================================================
    // EVENT HANDLING - Mutates State, Emits Commands
    // =========================================================================

    /// Handle an application event.
    ///
    /// This mutates DomainState and UiState, and may emit AppCommands.
    /// Does NOT perform IO directly.
    fn handle_event(&mut self, event: AppEvent) {
        let domain = &mut self.state.domain;
        let ui = &mut self.state.ui;

        match event {
            // -----------------------------------------------------------------
            // CBU Events
            // -----------------------------------------------------------------
            AppEvent::SelectCbu(cbu_id) => {
                domain.selected_cbu = Some(cbu_id);
                // Update session context
                if let Some(cbu) = domain.cbu_list.iter().find(|c| c.cbu_id == cbu_id) {
                    domain.session.set_cbu(CbuContext {
                        id: cbu_id,
                        name: cbu.name.clone(),
                        jurisdiction: cbu.jurisdiction.clone(),
                        client_type: cbu.client_type.clone(),
                    });
                }
                // Emit commands to load graph and layout
                domain.pending_commands.push(AppCommand::LoadCbuGraph {
                    cbu_id,
                    view_mode: ui.view_mode,
                    orientation: ui.orientation,
                });
                domain.pending_commands.push(AppCommand::LoadLayout {
                    cbu_id,
                    view_mode: ui.view_mode,
                });
                ui.modal = Modal::None;
            }

            AppEvent::RefreshCbuList => {
                domain.pending_commands.push(AppCommand::LoadCbuList);
            }

            AppEvent::OpenCbuPicker => {
                ui.modal = Modal::CbuPicker;
                self.cbu_picker.open(domain.cbu_list.clone());
            }

            // -----------------------------------------------------------------
            // View Events
            // -----------------------------------------------------------------
            AppEvent::SetViewMode(mode) => {
                ui.view_mode = mode;
                self.graph_widget.set_view_mode(mode);
                if let Some(cbu_id) = domain.selected_cbu {
                    domain.pending_commands.push(AppCommand::LoadCbuGraph {
                        cbu_id,
                        view_mode: mode,
                        orientation: ui.orientation,
                    });
                    domain.pending_commands.push(AppCommand::LoadLayout {
                        cbu_id,
                        view_mode: mode,
                    });
                }
            }

            AppEvent::SetOrientation(orient) => {
                ui.orientation = orient;
                if let Some(cbu_id) = domain.selected_cbu {
                    domain.pending_commands.push(AppCommand::LoadCbuGraph {
                        cbu_id,
                        view_mode: ui.view_mode,
                        orientation: orient,
                    });
                }
            }

            // -----------------------------------------------------------------
            // Session Events
            // -----------------------------------------------------------------
            AppEvent::CreateSession => {
                domain.pending_commands.push(AppCommand::CreateSession);
            }

            // -----------------------------------------------------------------
            // Chat Events
            // -----------------------------------------------------------------
            AppEvent::SendMessage(message) => {
                // Add user message to chat
                domain.messages.push(ChatMessage::User {
                    text: message.clone(),
                });

                // Check for session
                if let Some(session_id) = domain.session.session_id {
                    domain.pending_commands.push(AppCommand::SendChatMessage {
                        session_id,
                        message,
                    });
                } else {
                    domain.messages.push(ChatMessage::System {
                        text: "No active session. Creating one...".to_string(),
                        level: SystemLevel::Info,
                    });
                    domain.pending_commands.push(AppCommand::CreateSession);
                }
            }

            AppEvent::ExecutePendingDsl => {
                // TODO: Execute pending DSL
                domain.messages.push(ChatMessage::System {
                    text: "Execute not yet implemented".to_string(),
                    level: SystemLevel::Info,
                });
                domain.pending.clear();
            }

            AppEvent::CancelPendingDsl => {
                domain.pending.clear();
                domain.dsl_source.clear();
                domain.ast_statements.clear();
            }

            // -----------------------------------------------------------------
            // Entity Resolution Events
            // -----------------------------------------------------------------
            AppEvent::OpenEntityFinder {
                entity_type,
                search_text,
                statement_idx,
                arg_key,
            } => {
                ui.modal = Modal::EntityFinder {
                    entity_type: entity_type.clone(),
                    search_text: search_text.clone(),
                    statement_idx,
                    arg_key: arg_key.clone(),
                };
                let context = ResolveContext {
                    statement_idx,
                    arg_key,
                    original_text: search_text.clone(),
                };
                self.entity_finder.open(&entity_type, &search_text, context);
                domain.pending_commands.push(AppCommand::SearchEntities {
                    entity_type,
                    query: search_text,
                });
            }

            AppEvent::SearchEntities { entity_type, query } => {
                domain
                    .pending_commands
                    .push(AppCommand::SearchEntities { entity_type, query });
            }

            AppEvent::EntitySelected {
                statement_idx,
                arg_key,
                entity,
            } => {
                domain.messages.push(ChatMessage::System {
                    text: format!("Resolved to '{}' ({})", entity.name, entity.value),
                    level: SystemLevel::Info,
                });

                // Update local AST state
                if let Some(SimpleAstStatement::VerbCall { args, .. }) =
                    domain.ast_statements.get_mut(statement_idx)
                {
                    for arg in args.iter_mut() {
                        if arg.key == arg_key {
                            if let crate::state::AstNode::EntityRef {
                                ref mut resolved_key,
                                ..
                            } = arg.value
                            {
                                *resolved_key = Some(entity.value.clone());
                            }
                        }
                    }
                }

                ui.modal = Modal::None;
            }

            // -----------------------------------------------------------------
            // Modal Events
            // -----------------------------------------------------------------
            AppEvent::CloseModal => {
                ui.modal = Modal::None;
            }

            AppEvent::ShowError(msg) => {
                ui.modal = Modal::Error(msg);
            }

            AppEvent::ConfirmAccepted(action) => {
                ui.modal = Modal::None;
                match action {
                    ConfirmAction::ExecuteDsl => {
                        // Handle DSL execution confirmation
                    }
                }
            }

            // -----------------------------------------------------------------
            // Task Result Events
            // -----------------------------------------------------------------
            AppEvent::CbuListLoaded(result) => {
                domain.tasks.cbu_list = TaskStatus::Finished(result.clone());
                match result {
                    Ok(cbus) => domain.cbu_list = cbus,
                    Err(e) => ui.error = Some(format!("Failed to load CBUs: {}", e)),
                }
                ui.loading = false;
            }

            AppEvent::SessionCreated(result) => {
                domain.tasks.session = TaskStatus::Finished(result.clone());
                match result {
                    Ok(session_id) => {
                        domain.session.session_id = Some(session_id);
                        domain.messages.push(ChatMessage::System {
                            text: "Session created. You can now chat.".to_string(),
                            level: SystemLevel::Info,
                        });
                    }
                    Err(e) => {
                        domain.messages.push(ChatMessage::System {
                            text: format!("Failed to create session: {}", e),
                            level: SystemLevel::Error,
                        });
                    }
                }
            }

            AppEvent::ChatResponseReceived(result) => {
                domain.tasks.chat = TaskStatus::Finished(result.clone());
                match result {
                    Ok(response) => {
                        // Update DSL source
                        if let Some(dsl) = &response.dsl_source {
                            domain.dsl_source = dsl.clone();
                            domain.pending.pending_dsl = Some(dsl.clone());
                        }

                        // Update AST
                        if let Some(ast) = &response.ast {
                            domain.ast_statements =
                                ast.iter().map(SimpleAstStatement::from_api).collect();
                        }

                        // Add assistant message
                        let status = if response.can_execute {
                            MessageStatus::PendingConfirmation
                        } else if !response.validation_errors.is_empty() {
                            MessageStatus::Error
                        } else {
                            MessageStatus::Valid
                        };

                        domain.messages.push(ChatMessage::Assistant {
                            text: response
                                .message
                                .unwrap_or_else(|| "DSL generated.".to_string()),
                            dsl: response.dsl_source.clone(),
                            status,
                        });

                        // Show validation errors
                        for err in &response.validation_errors {
                            domain.messages.push(ChatMessage::System {
                                text: err.clone(),
                                level: SystemLevel::Warning,
                            });
                        }
                    }
                    Err(e) => {
                        domain.messages.push(ChatMessage::System {
                            text: format!("Chat error: {}", e),
                            level: SystemLevel::Error,
                        });
                    }
                }
            }

            AppEvent::EntitySearchCompleted(result) => {
                domain.tasks.entity_search = TaskStatus::Finished(result.clone());
                match result {
                    Ok(matches) => {
                        self.entity_finder.set_results(matches);
                    }
                    Err(e) => {
                        self.entity_finder.set_loading(false);
                        domain.messages.push(ChatMessage::System {
                            text: format!("Entity search failed: {}", e),
                            level: SystemLevel::Error,
                        });
                    }
                }
            }

            AppEvent::GraphLoaded(result) => {
                domain.tasks.graph = TaskStatus::Finished(result.clone());
                match result {
                    Ok(graph_data) => {
                        self.graph_widget.set_data(graph_data);
                        // Apply deferred layout if available
                        if let Some(layout) = ui.deferred_layout.take() {
                            self.graph_widget.apply_layout_override(layout);
                        }
                    }
                    Err(e) => {
                        ui.error = Some(format!("Failed to load graph: {}", e));
                    }
                }
                ui.loading = false;
            }

            AppEvent::LayoutLoaded(result) => {
                domain.tasks.layout = TaskStatus::Finished(result.clone());
                if let Ok(layout) = result {
                    ui.deferred_layout = Some(layout);
                }
            }

            // -----------------------------------------------------------------
            // Misc Events
            // -----------------------------------------------------------------
            AppEvent::CopyDsl => {
                domain
                    .pending_commands
                    .push(AppCommand::CopyToClipboard(domain.dsl_source.clone()));
                domain.messages.push(ChatMessage::System {
                    text: "DSL copied to clipboard".to_string(),
                    level: SystemLevel::Info,
                });
            }

            AppEvent::ClearError => {
                ui.error = None;
            }

            AppEvent::SystemMessage { text, level } => {
                domain.messages.push(ChatMessage::System { text, level });
            }
        }
    }

    // =========================================================================
    // COMMAND EXECUTION - Performs IO
    // =========================================================================

    /// Execute pending commands.
    ///
    /// This is the ONLY place where IO operations are initiated.
    fn execute_commands(&mut self) {
        let commands: Vec<AppCommand> = self.state.domain.pending_commands.drain(..).collect();

        for cmd in commands {
            match cmd {
                AppCommand::LoadCbuList => {
                    self.state.domain.tasks.cbu_list = TaskStatus::InProgress;
                    self.state.ui.loading = true;
                    self.spawn_load_cbu_list();
                }

                AppCommand::LoadCbuGraph {
                    cbu_id,
                    view_mode,
                    orientation,
                } => {
                    self.state.domain.tasks.graph = TaskStatus::InProgress;
                    self.state.ui.loading = true;
                    self.state.ui.deferred_layout = None;
                    self.spawn_load_graph(cbu_id, view_mode, orientation);
                }

                AppCommand::LoadLayout { cbu_id, view_mode } => {
                    self.state.domain.tasks.layout = TaskStatus::InProgress;
                    self.spawn_load_layout(cbu_id, view_mode);
                }

                AppCommand::SaveLayout {
                    cbu_id,
                    view_mode,
                    overrides,
                } => {
                    self.state.domain.tasks.layout_save = TaskStatus::InProgress;
                    self.spawn_save_layout(cbu_id, view_mode, overrides);
                }

                AppCommand::CreateSession => {
                    self.state.domain.tasks.session = TaskStatus::InProgress;
                    self.spawn_create_session();
                }

                AppCommand::SendChatMessage {
                    session_id,
                    message,
                } => {
                    self.state.domain.tasks.chat = TaskStatus::InProgress;
                    self.spawn_send_chat(session_id, message);
                }

                AppCommand::SearchEntities { entity_type, query } => {
                    self.state.domain.tasks.entity_search = TaskStatus::InProgress;
                    self.spawn_search_entities(entity_type, query);
                }

                AppCommand::ExecuteDsl { session_id, dsl } => {
                    self.state.domain.tasks.dsl_execute = TaskStatus::InProgress;
                    // TODO: Implement DSL execution
                    let _ = (session_id, dsl);
                }

                AppCommand::CopyToClipboard(text) => {
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(window) = web_sys::window() {
                            if let Some(navigator) = window.navigator().clipboard() {
                                wasm_bindgen_futures::spawn_local(async move {
                                    let _ = wasm_bindgen_futures::JsFuture::from(
                                        navigator.write_text(&text),
                                    )
                                    .await;
                                });
                            }
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        // Native clipboard handling would go here
                        let _ = text;
                    }
                }
            }
        }
    }

    // =========================================================================
    // SPAWN ASYNC TASKS
    // =========================================================================

    fn spawn_load_cbu_list(&mut self) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<Vec<CbuSummary>, String> = api.get("/api/cbu").await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<Vec<CbuSummary>, String> = api.get("/api/cbu").await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.channels.cbu_list = Some(result);
    }

    fn spawn_load_graph(
        &mut self,
        cbu_id: Uuid,
        view_mode: ViewMode,
        orientation: crate::state::Orientation,
    ) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();
        let path = format!(
            "/api/cbu/{}/graph?view_mode={}&orientation={}",
            cbu_id,
            view_mode.as_str(),
            orientation.as_str()
        );

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<crate::graph::CbuGraphData, String> = api.get(&path).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<crate::graph::CbuGraphData, String> = api.get(&path).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.channels.graph = Some(result);
    }

    fn spawn_load_layout(&mut self, cbu_id: Uuid, view_mode: ViewMode) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();
        let path = format!(
            "/api/cbu/{}/layout?view_mode={}",
            cbu_id,
            view_mode.as_str()
        );

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<LayoutOverride, String> = api.get(&path).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<LayoutOverride, String> = api.get(&path).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.channels.layout = Some(result);
    }

    fn spawn_save_layout(&mut self, cbu_id: Uuid, view_mode: ViewMode, overrides: LayoutOverride) {
        let api = self.api.clone();
        let path = format!(
            "/api/cbu/{}/layout?view_mode={}",
            cbu_id,
            view_mode.as_str()
        );

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = api.post::<LayoutOverride, _>(&path, &overrides).await;
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let _ = api.post::<LayoutOverride, _>(&path, &overrides).await;
            });
        }
    }

    fn spawn_create_session(&mut self) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<crate::state::types::SessionResponse, String> =
                    api.post("/api/session", &()).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<crate::state::types::SessionResponse, String> =
                    api.post("/api/session", &()).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.channels.session = Some(result);
    }

    fn spawn_send_chat(&mut self, session_id: Uuid, message: String) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();
        let path = format!("/api/session/{}/chat", session_id);

        #[derive(serde::Serialize)]
        struct ChatRequest {
            message: String,
        }

        let request = ChatRequest { message };

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<crate::state::types::ChatResponse, String> =
                    api.post(&path, &request).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<crate::state::types::ChatResponse, String> =
                    api.post(&path, &request).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.channels.chat = Some(result);
    }

    fn spawn_search_entities(&mut self, entity_type: String, query: String) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();
        let path = format!(
            "/api/entity/search?type={}&q={}",
            urlencoding::encode(&entity_type),
            urlencoding::encode(&query)
        );

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<Vec<EntityMatch>, String> = api.get(&path).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<Vec<EntityMatch>, String> = api.get(&path).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.channels.entity_search = Some(result);
    }

    // =========================================================================
    // POLL ASYNC RESULTS
    // =========================================================================

    /// Poll async channels and dispatch result events.
    fn poll_async_results(&mut self) {
        let mut events: Vec<AppEvent> = Vec::new();

        // Poll CBU list
        if let Some(result) = self
            .channels
            .cbu_list
            .as_ref()
            .and_then(|c| c.try_lock().ok())
            .and_then(|mut g| g.take())
        {
            events.push(AppEvent::CbuListLoaded(result));
            self.channels.cbu_list = None;
        }

        // Poll session
        if let Some(result) = self
            .channels
            .session
            .as_ref()
            .and_then(|c| c.try_lock().ok())
            .and_then(|mut g| g.take())
        {
            events.push(AppEvent::SessionCreated(result.map(|r| r.session_id)));
            self.channels.session = None;
        }

        // Poll chat
        if let Some(result) = self
            .channels
            .chat
            .as_ref()
            .and_then(|c| c.try_lock().ok())
            .and_then(|mut g| g.take())
        {
            events.push(AppEvent::ChatResponseReceived(result));
            self.channels.chat = None;
        }

        // Poll entity search
        if let Some(result) = self
            .channels
            .entity_search
            .as_ref()
            .and_then(|c| c.try_lock().ok())
            .and_then(|mut g| g.take())
        {
            events.push(AppEvent::EntitySearchCompleted(result));
            self.channels.entity_search = None;
        }

        // Poll graph and layout (wait for both if both pending)
        let graph_ready = self
            .channels
            .graph
            .as_ref()
            .and_then(|c| c.try_lock().ok())
            .map(|g| g.is_some())
            .unwrap_or(false);
        let layout_ready = self
            .channels
            .layout
            .as_ref()
            .and_then(|c| c.try_lock().ok())
            .map(|g| g.is_some())
            .unwrap_or(false);
        let has_layout_pending = self.channels.layout.is_some();

        if graph_ready && (!has_layout_pending || layout_ready) {
            // Take layout first
            if let Some(result) = self
                .channels
                .layout
                .as_ref()
                .and_then(|c| c.try_lock().ok())
                .and_then(|mut g| g.take())
            {
                events.push(AppEvent::LayoutLoaded(result));
                self.channels.layout = None;
            }

            // Take graph
            if let Some(result) = self
                .channels
                .graph
                .as_ref()
                .and_then(|c| c.try_lock().ok())
                .and_then(|mut g| g.take())
            {
                events.push(AppEvent::GraphLoaded(result));
                self.channels.graph = None;
            }
        }

        // Dispatch events
        for event in events {
            self.handle_event(event);
        }
    }

    // =========================================================================
    // LAYOUT SAVE DEBOUNCE
    // =========================================================================

    fn check_layout_save(&mut self, now: f64) {
        if self.graph_widget.peek_pending_layout_override() {
            if self.state.ui.layout_dirty_since.is_none() {
                self.state.ui.layout_dirty_since = Some(now);
            }

            if let Some(dirty_since) = self.state.ui.layout_dirty_since {
                if now - dirty_since >= LAYOUT_SAVE_DEBOUNCE_SECS {
                    if let Some(overrides) = self.graph_widget.take_pending_layout_override() {
                        if let Some(cbu_id) = self.state.domain.selected_cbu {
                            self.state
                                .domain
                                .pending_commands
                                .push(AppCommand::SaveLayout {
                                    cbu_id,
                                    view_mode: self.state.ui.view_mode,
                                    overrides,
                                });
                        }
                    }
                    self.state.ui.layout_dirty_since = None;
                }
            }
        } else {
            self.state.ui.layout_dirty_since = None;
        }
    }

    // =========================================================================
    // UI HELPERS: Convert panel/modal results to events
    // =========================================================================

    fn chat_action_to_event(action: ChatPanelAction) -> Option<AppEvent> {
        match action {
            ChatPanelAction::None => None,
            ChatPanelAction::SendMessage(msg) => Some(AppEvent::SendMessage(msg)),
            ChatPanelAction::Execute => Some(AppEvent::ExecutePendingDsl),
            ChatPanelAction::Cancel => Some(AppEvent::CancelPendingDsl),
        }
    }

    fn entity_finder_result_to_event(result: EntityFinderResult) -> Option<AppEvent> {
        match result {
            EntityFinderResult::None => None,
            EntityFinderResult::Search { entity_type, query } => {
                Some(AppEvent::SearchEntities { entity_type, query })
            }
            EntityFinderResult::Selected { context, entity } => Some(AppEvent::EntitySelected {
                statement_idx: context.statement_idx,
                arg_key: context.arg_key,
                entity,
            }),
            EntityFinderResult::Closed => Some(AppEvent::CloseModal),
        }
    }

    fn cbu_picker_result_to_event(result: CbuPickerResult) -> Option<AppEvent> {
        match result {
            CbuPickerResult::None => None,
            CbuPickerResult::Selected(cbu) => Some(AppEvent::SelectCbu(cbu.cbu_id)),
            CbuPickerResult::Closed => Some(AppEvent::CloseModal),
        }
    }

    fn ast_click_to_event(click: UnresolvedRefClick) -> AppEvent {
        AppEvent::OpenEntityFinder {
            entity_type: click.entity_type,
            search_text: click.search_text,
            statement_idx: click.statement_idx,
            arg_key: click.arg_key,
        }
    }
}

// =============================================================================
// EFRAME APP IMPLEMENTATION
// =============================================================================

impl eframe::App for ObPocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Execute pending commands (IO)
        self.execute_commands();

        // 2. Poll async results
        self.poll_async_results();

        let now = ctx.input(|i| i.time);

        // 3. Check layout save debounce
        self.check_layout_save(now);

        // Request repaint if work pending
        if self.state.ui.loading
            || self.state.domain.tasks.any_in_progress()
            || self.state.ui.has_modal_open()
            || self.entity_finder.is_open()
            || self.cbu_picker.is_open()
            || self.state.ui.layout_dirty_since.is_some()
        {
            ctx.request_repaint();
        }

        // Collect events from this frame
        let mut events: Vec<AppEvent> = Vec::new();

        // Global keyboard shortcuts (when modals are not open)
        if !self.state.ui.has_modal_open() {
            let ctrl_or_cmd = ctx.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);

            // Ctrl/Cmd+K: Open CBU Picker
            if ctrl_or_cmd && ctx.input(|i| i.key_pressed(egui::Key::K)) {
                events.push(AppEvent::OpenCbuPicker);
            }

            // Ctrl/Cmd+Enter: Execute pending DSL
            if ctrl_or_cmd
                && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                && self.state.domain.pending.pending_dsl.is_some()
            {
                events.push(AppEvent::ExecutePendingDsl);
            }
        }

        // =====================================================================
        // MODALS
        // =====================================================================

        // Entity Finder Modal
        if self.state.ui.is_entity_finder_open() || self.entity_finder.is_open() {
            let result = self.entity_finder.ui(ctx);
            if let Some(event) = Self::entity_finder_result_to_event(result) {
                events.push(event);
            }
        }

        // CBU Picker Modal
        if self.state.ui.is_cbu_picker_open() || self.cbu_picker.is_open() {
            let result = self.cbu_picker.ui(ctx);
            if let Some(event) = Self::cbu_picker_result_to_event(result) {
                events.push(event);
            }
        }

        // Error Modal
        if let Modal::Error(ref msg) = self.state.ui.modal {
            let msg = msg.clone();
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.colored_label(Color32::RED, &msg);
                    if ui.button("OK").clicked() {
                        events.push(AppEvent::CloseModal);
                    }
                });
        }

        // =====================================================================
        // TOP PANEL - Header
        // =====================================================================
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // CBU selector
                ui.label("CBU:");
                let selected_name = self
                    .state
                    .domain
                    .selected_cbu_summary()
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "Select CBU...".to_string());

                egui::ComboBox::from_id_source("cbu_selector")
                    .selected_text(selected_name)
                    .show_ui(ui, |ui| {
                        for cbu in &self.state.domain.cbu_list {
                            let label = format!(
                                "{} ({})",
                                cbu.name,
                                cbu.jurisdiction.as_deref().unwrap_or("N/A")
                            );
                            if ui
                                .selectable_label(
                                    self.state.domain.selected_cbu == Some(cbu.cbu_id),
                                    label,
                                )
                                .clicked()
                            {
                                events.push(AppEvent::SelectCbu(cbu.cbu_id));
                            }
                        }
                    });

                if ui.button("🔍").on_hover_text("Search CBUs").clicked() {
                    events.push(AppEvent::OpenCbuPicker);
                }

                if ui.button("↻").on_hover_text("Refresh CBU list").clicked() {
                    events.push(AppEvent::RefreshCbuList);
                }

                ui.separator();

                // View mode selector
                ui.label("View:");
                for mode in ViewMode::all() {
                    if ui
                        .selectable_label(self.state.ui.view_mode == *mode, mode.display_name())
                        .clicked()
                    {
                        events.push(AppEvent::SetViewMode(*mode));
                    }
                }

                ui.separator();

                // Orientation selector
                ui.label("Layout:");
                for orient in crate::state::Orientation::all() {
                    if ui
                        .selectable_label(self.state.ui.orientation == *orient, orient.display_name())
                        .clicked()
                    {
                        events.push(AppEvent::SetOrientation(*orient));
                    }
                }

                ui.separator();

                // Session controls
                if ui.button("+ New Session").clicked() {
                    events.push(AppEvent::CreateSession);
                }

                if self.state.domain.has_session() {
                    ui.label(
                        egui::RichText::new("● Session Active")
                            .color(egui::Color32::from_rgb(74, 222, 128))
                            .size(11.0),
                    );
                }

                // Loading indicator
                if self.state.ui.loading {
                    ui.spinner();
                }

                // Keyboard shortcuts help
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("⌘K Search CBU")
                            .size(10.0)
                            .color(Color32::DARK_GRAY),
                    )
                    .on_hover_text("Keyboard shortcuts:\n⌘K / Ctrl+K: Search CBUs\n⌘↵ / Ctrl+Enter: Execute DSL\nEsc: Close modals");
                });
            });
        });

        // =====================================================================
        // CENTRAL PANEL - 2x2 Grid Layout
        // =====================================================================
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref err) = self.state.ui.error {
                ui.colored_label(egui::Color32::RED, err);
                ui.separator();
            }

            let available = ui.available_size();
            let half_width = available.x / 2.0 - 4.0;
            let half_height = available.y / 2.0 - 4.0;

            egui::Grid::new("main_grid")
                .num_columns(2)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    // Top-left: Graph
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(25, 25, 25))
                        .rounding(4.0)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(half_width, half_height));
                            ui.set_max_size(egui::vec2(half_width, half_height));

                            if self.state.domain.tasks.loading_graph_view() {
                                ui.centered_and_justified(|ui| ui.spinner());
                            } else {
                                self.graph_widget.ui(ui);
                            }
                        });

                    // Top-right: DSL Source
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(25, 25, 25))
                        .rounding(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(half_width, half_height));
                            ui.set_max_size(egui::vec2(half_width, half_height));

                            if self.dsl_panel.ui(ui, &self.state.domain.dsl_source) {
                                events.push(AppEvent::CopyDsl);
                            }
                        });

                    ui.end_row();

                    // Bottom-left: Chat
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(25, 25, 25))
                        .rounding(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(half_width, half_height));
                            ui.set_max_size(egui::vec2(half_width, half_height));

                            let action = self.chat_panel.ui(
                                ui,
                                &self.state.domain.messages,
                                self.state.domain.pending.pending_dsl.is_some(),
                                self.state.domain.pending.has_pending(),
                            );
                            if let Some(event) = Self::chat_action_to_event(action) {
                                events.push(event);
                            }
                        });

                    // Bottom-right: AST
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(25, 25, 25))
                        .rounding(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(half_width, half_height));
                            ui.set_max_size(egui::vec2(half_width, half_height));

                            if let Some(click) =
                                self.ast_panel.ui(ui, &self.state.domain.ast_statements)
                            {
                                events.push(Self::ast_click_to_event(click));
                            }
                        });
                });
        });

        // =====================================================================
        // DISPATCH EVENTS
        // =====================================================================
        for event in events {
            self.handle_event(event);
        }
    }
}

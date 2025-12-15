//! Main Application
//!
//! This module implements the eframe::App trait and coordinates:
//! 1. Processing async results at the start of each frame
//! 2. Rendering panels based on layout mode
//! 3. Handling widget responses (no callbacks, return values only)

use crate::api;
use crate::panels::{
    ast_panel, chat_panel, dsl_editor_panel, entity_detail_panel, results_panel, toolbar,
    ToolbarAction,
};
use crate::state::{AppState, AsyncState, LayoutMode, PanelState, TextBuffers};
use ob_poc_graph::{CbuGraphWidget, ViewMode};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

/// Main application struct
pub struct App {
    pub state: AppState,
}

impl App {
    /// Create new application instance
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Setup dark theme
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut state = AppState {
            session: None,
            session_id: None,
            graph_data: None,
            validation_result: None,
            execution: None,
            messages: Vec::new(),
            cbu_list: Vec::new(),
            buffers: TextBuffers::default(),
            view_mode: ViewMode::KycUbo,
            panels: PanelState::default(),
            selected_entity_id: None,
            graph_widget: CbuGraphWidget::new(),
            async_state: Arc::new(Mutex::new(AsyncState::default())),
            ctx: Some(cc.egui_ctx.clone()),
        };

        // Try to restore session from localStorage
        state.restore_session();

        // Fetch initial CBU list
        state.fetch_cbu_list();

        Self { state }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // =================================================================
        // STEP 1: Process any pending async results
        // =================================================================
        self.state.process_async_results();

        // After execution completes, refetch dependent data
        // (This must be done here, not in process_async_results, because
        // we need to call methods that take &mut self)
        // We use execution_handled flag to ensure we only refetch once per execution
        if self.state.should_handle_execution_complete() {
            // Trigger refetches - server is source of truth
            self.state.refetch_session();
            self.state.refetch_graph();
        }

        // Check for pending execute command from agent
        if let Some(session_id) = self.state.take_pending_execute() {
            web_sys::console::log_1(&format!("update: executing session {}", session_id).into());
            // Get DSL from UI state (more reliable than server state after restarts)
            let dsl = self.state.get_dsl_source();
            if let Some(dsl) = dsl {
                if !dsl.trim().is_empty() {
                    self.state.execute_dsl_with_content(session_id, dsl);
                } else {
                    web_sys::console::warn_1(&"execute: DSL is empty".into());
                }
            } else {
                // Fallback to server state
                self.state.execute_session(session_id);
            }
        }

        // =================================================================
        // DEBUG: F1 toggles debug window (dev builds only)
        // =================================================================
        #[cfg(debug_assertions)]
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.state.panels.show_debug = !self.state.panels.show_debug;
        }

        #[cfg(debug_assertions)]
        if self.state.panels.show_debug {
            egui::Window::new("Debug (F1 to close)")
                .default_width(350.0)
                .show(ctx, |ui| {
                    // Session info
                    ui.heading("Session");
                    if let Some(id) = self.state.session_id {
                        ui.label(format!("ID: {}", id));
                    } else {
                        ui.label("No session");
                    }

                    ui.separator();

                    // Async state
                    ui.heading("Async State");
                    if let Ok(state) = self.state.async_state.lock() {
                        ui.label(format!("loading_session: {}", state.loading_session));
                        ui.label(format!("loading_graph: {}", state.loading_graph));
                        ui.label(format!("loading_chat: {}", state.loading_chat));
                        ui.label(format!("executing: {}", state.executing));
                        if let Some(ref err) = state.last_error {
                            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        }
                    }

                    ui.separator();

                    // Session state from server
                    ui.heading("Server Session");
                    if let Some(ref session) = self.state.session {
                        ui.label(format!("state: {:?}", session.state));
                        ui.label(format!("can_execute: {}", session.can_execute));
                        ui.label(format!(
                            "assembled_dsl: {:?}",
                            session.assembled_dsl.as_ref().map(|v| v
                                .to_string()
                                .chars()
                                .take(50)
                                .collect::<String>())
                        ));
                        ui.label(format!(
                            "combined_dsl len: {}",
                            session
                                .combined_dsl
                                .as_ref()
                                .map(|v| v.to_string().len())
                                .unwrap_or(0)
                        ));
                    } else {
                        ui.label("No session data");
                    }

                    ui.separator();

                    // Buffers
                    ui.heading("Buffers");
                    ui.label(format!(
                        "dsl_editor len: {}",
                        self.state.buffers.dsl_editor.len()
                    ));
                    ui.label(format!("dsl_dirty: {}", self.state.buffers.dsl_dirty));
                    ui.label(format!(
                        "chat_input len: {}",
                        self.state.buffers.chat_input.len()
                    ));

                    ui.separator();

                    // Messages
                    ui.heading("Messages");
                    ui.label(format!("count: {}", self.state.messages.len()));

                    ui.separator();

                    // egui inspection
                    ui.collapsing("egui Inspection", |ui| {
                        ctx.inspection_ui(ui);
                    });
                });
        }

        // =================================================================
        // STEP 2: Handle widget responses (return values, not callbacks)
        // =================================================================
        if let Some(entity_id) = self.state.graph_widget.selected_entity_changed() {
            self.state.selected_entity_id = Some(entity_id);
        }

        // =================================================================
        // STEP 3: Render UI and handle actions
        // =================================================================

        // Top toolbar - returns actions
        let toolbar_action = egui::TopBottomPanel::top("toolbar")
            .show(ctx, |ui| toolbar(ui, &mut self.state))
            .inner;

        // Handle toolbar actions
        self.handle_toolbar_action(toolbar_action);

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| match self.state.panels.layout {
            LayoutMode::FourPanel => self.render_four_panel(ui),
            LayoutMode::EditorFocus => self.render_editor_focus(ui),
            LayoutMode::GraphFocus => self.render_graph_focus(ui),
        });

        // =================================================================
        // STEP 4: Request repaint if async operations in progress
        // =================================================================
        if self.state.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl App {
    /// Handle toolbar actions
    fn handle_toolbar_action(&mut self, action: ToolbarAction) {
        if let Some((cbu_id, name)) = action.select_cbu {
            self.state.select_cbu(cbu_id, &name);
        }

        if let Some(mode) = action.change_view_mode {
            self.state.set_view_mode(mode);
        }

        if action.dismiss_error {
            if let Ok(mut async_state) = self.state.async_state.lock() {
                async_state.last_error = None;
            }
        }
    }

    /// Render layout:
    /// - Top 50%: Graph (full width)
    /// - Bottom left 25%: Chat
    /// - Bottom right top: DSL Editor
    /// - Bottom right bottom: Results/AST
    fn render_four_panel(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let top_height = available.y * 0.5;
        let bottom_height = available.y * 0.5 - 4.0;

        // Top: Graph (full width, 50% height)
        ui.allocate_ui(egui::vec2(available.x, top_height), |ui| {
            egui::Frame::default()
                .inner_margin(0.0)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .show(ui, |ui| {
                    self.state.graph_widget.ui(ui);
                });
        });

        ui.separator();

        // Bottom row
        ui.horizontal(|ui| {
            ui.set_height(bottom_height);

            // Chat panel (left, 50% width)
            ui.vertical(|ui| {
                ui.set_width(available.x * 0.5 - 4.0);
                egui::Frame::default()
                    .inner_margin(8.0)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
                        chat_panel(ui, &mut self.state);
                    });
            });

            ui.separator();

            // Right side (50% width) - DSL on top, Results on bottom
            ui.vertical(|ui| {
                ui.set_width(available.x * 0.5 - 4.0);
                let panel_height = (bottom_height - 4.0) / 2.0;

                // DSL Editor (top half of right side)
                ui.allocate_ui(egui::vec2(ui.available_width(), panel_height), |ui| {
                    egui::Frame::default()
                        .inner_margin(8.0)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .show(ui, |ui| {
                            dsl_editor_panel(ui, &mut self.state);
                        });
                });

                ui.separator();

                // Results/AST/Entity Detail (bottom half of right side)
                ui.allocate_ui(egui::vec2(ui.available_width(), panel_height), |ui| {
                    // Tab bar
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.state.panels.show_ast, "AST")
                            .clicked()
                        {
                            self.state.panels.show_ast = true;
                            self.state.panels.show_results = false;
                            self.state.panels.show_entity_detail = false;
                        }
                        if ui
                            .selectable_label(self.state.panels.show_results, "Results")
                            .clicked()
                        {
                            self.state.panels.show_ast = false;
                            self.state.panels.show_results = true;
                            self.state.panels.show_entity_detail = false;
                        }
                        if ui
                            .selectable_label(self.state.panels.show_entity_detail, "Entity")
                            .clicked()
                        {
                            self.state.panels.show_ast = false;
                            self.state.panels.show_results = false;
                            self.state.panels.show_entity_detail = true;
                        }
                    });

                    ui.separator();

                    egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                        if self.state.panels.show_ast {
                            ast_panel(ui, &mut self.state);
                        } else if self.state.panels.show_results {
                            results_panel(ui, &mut self.state);
                        } else {
                            entity_detail_panel(ui, &mut self.state);
                        }
                    });
                });
            });
        });
    }

    /// Render editor-focused layout (large editor, small side panels)
    fn render_editor_focus(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Editor (70% width)
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() * 0.7);
                dsl_editor_panel(ui, &mut self.state);
            });

            ui.separator();

            // Stacked panels on right
            ui.vertical(|ui| {
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    ui.set_height(ui.available_height() / 2.0);
                    chat_panel(ui, &mut self.state);
                });
                ui.separator();
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    results_panel(ui, &mut self.state);
                });
            });
        });
    }

    /// Render graph-focused layout (large graph, small side panels)
    fn render_graph_focus(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Graph (70% width)
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() * 0.7);
                self.state.graph_widget.ui(ui);
            });

            ui.separator();

            // Stacked panels on right
            ui.vertical(|ui| {
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    ui.set_height(ui.available_height() / 2.0);
                    entity_detail_panel(ui, &mut self.state);
                });
                ui.separator();
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    dsl_editor_panel(ui, &mut self.state);
                });
            });
        });
    }
}

// =============================================================================
// AppState Methods - Server Communication
// =============================================================================

impl AppState {
    /// Restore session from localStorage, or create a new one
    pub fn restore_session(&mut self) {
        if let Some(session_id_str) = api::get_local_storage("session_id") {
            if let Ok(session_id) = Uuid::parse_str(&session_id_str) {
                self.session_id = Some(session_id);
                self.refetch_session();
                return;
            }
        }
        // No existing session - create a new one
        self.create_session();
    }

    /// Create a new session
    pub fn create_session(&mut self) {
        web_sys::console::log_1(&"create_session: starting...".into());
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            web_sys::console::log_1(&"create_session: calling API...".into());
            match api::create_session().await {
                Ok(response) => {
                    web_sys::console::log_1(
                        &format!("create_session: got session_id={}", response.session_id).into(),
                    );
                    // Store session_id in localStorage
                    if let Ok(session_id) = Uuid::parse_str(&response.session_id) {
                        let _ = api::set_local_storage("session_id", &response.session_id);
                        if let Ok(mut state) = async_state.lock() {
                            state.pending_session_id = Some(session_id);
                        }
                    }
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("create_session FAILED: {}", e).into());
                    if let Ok(mut state) = async_state.lock() {
                        state.last_error = Some(format!("Failed to create session: {}", e));
                    }
                }
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Fetch CBU list from server
    pub fn fetch_cbu_list(&mut self) {
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::list_cbus().await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_cbu_list = Some(result);
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Refetch session from server
    pub fn refetch_session(&mut self) {
        let Some(session_id) = self.session_id else {
            return;
        };

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_session = true;
        }

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::get_session(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_session = Some(result);
                state.loading_session = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Refetch graph for current CBU
    pub fn refetch_graph(&mut self) {
        let cbu_id = self
            .session
            .as_ref()
            .and_then(|s| s.active_cbu.as_ref())
            .and_then(|c| Uuid::parse_str(&c.id).ok());

        let Some(cbu_id) = cbu_id else {
            return;
        };

        self.fetch_graph(cbu_id);
    }

    /// Fetch graph for specific CBU
    pub fn fetch_graph(&mut self, cbu_id: Uuid) {
        tracing::info!(
            "fetch_graph called: cbu_id={}, view_mode={:?}",
            cbu_id,
            self.view_mode
        );

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_graph = true;
        }

        let view_mode = self.view_mode;
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            tracing::info!("fetch_graph: calling API...");
            let result = api::get_cbu_graph(cbu_id, view_mode).await;
            tracing::info!("fetch_graph: API returned, success={}", result.is_ok());
            if let Err(ref e) = result {
                tracing::error!("fetch_graph error: {}", e);
            }
            if let Ok(mut state) = async_state.lock() {
                state.pending_graph = Some(result);
                state.loading_graph = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Select a CBU (bind to session, then refetch)
    pub fn select_cbu(&mut self, cbu_id: Uuid, display_name: &str) {
        tracing::info!(
            "select_cbu called: cbu_id={}, old_session_id={:?}",
            cbu_id,
            self.session_id
        );

        // Use CBU ID as session ID (session key = entity key)
        self.session_id = Some(cbu_id);
        let _ = api::set_local_storage("session_id", &cbu_id.to_string());

        // Always fetch graph
        self.fetch_graph(cbu_id);

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();
        let display_name = display_name.to_string();

        spawn_local(async move {
            // First: get/create session (session ID = CBU ID)
            let _ = api::get_session(cbu_id).await;

            // Then: bind CBU to session
            let _result = api::bind_entity(cbu_id, cbu_id, "cbu", &display_name).await;

            // Fetch session state again to get updated context
            let session_result = api::get_session(cbu_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_session = Some(session_result);
            }

            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Set view mode and refetch graph
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
        self.graph_widget.set_view_mode(mode);
        self.refetch_graph();
    }

    /// Send chat message
    pub fn send_chat_message(&mut self) {
        let message = std::mem::take(&mut self.buffers.chat_input);
        if message.trim().is_empty() {
            web_sys::console::warn_1(&"send_chat_message: empty message".into());
            return;
        }

        web_sys::console::log_1(&format!("send_chat_message: {}", message).into());

        // Add user message to local history immediately for responsiveness
        self.add_user_message(message.clone());

        let Some(session_id) = self.session_id else {
            web_sys::console::error_1(&"send_chat_message: NO SESSION ID!".into());
            return;
        };

        web_sys::console::log_1(&format!("send_chat_message: session_id={}", session_id).into());

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_chat = true;
        }

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            web_sys::console::log_1(&"send_chat: calling API...".into());
            let result = api::send_chat(session_id, &message).await;
            web_sys::console::log_1(
                &format!("send_chat: API returned, success={}", result.is_ok()).into(),
            );

            if let Ok(mut state) = async_state.lock() {
                state.loading_chat = false;

                match result {
                    Ok(chat_response) => {
                        web_sys::console::log_1(
                            &format!("send_chat: agent message: {}", chat_response.message).into(),
                        );
                        // Add the agent's response to pending_chat
                        state.pending_chat = Some(Ok(crate::state::ChatMessage {
                            role: crate::state::MessageRole::Agent,
                            content: chat_response.message.clone(),
                            timestamp: chrono::Utc::now(),
                        }));

                        // Handle agent commands
                        if let Some(ref commands) = chat_response.commands {
                            web_sys::console::log_1(
                                &format!(
                                    "send_chat: got {} commands: {:?}",
                                    commands.len(),
                                    commands
                                )
                                .into(),
                            );
                            for cmd in commands {
                                match cmd {
                                    // REPL commands
                                    ob_poc_types::AgentCommand::Execute => {
                                        web_sys::console::log_1(
                                            &format!("Command: Execute session_id={}", session_id)
                                                .into(),
                                        );
                                        state.pending_execute = Some(session_id);
                                    }
                                    ob_poc_types::AgentCommand::Undo => {
                                        web_sys::console::log_1(
                                            &format!("Command: Undo session_id={}", session_id)
                                                .into(),
                                        );
                                        state.pending_undo = Some(session_id);
                                    }
                                    ob_poc_types::AgentCommand::Clear => {
                                        web_sys::console::log_1(
                                            &format!("Command: Clear session_id={}", session_id)
                                                .into(),
                                        );
                                        state.pending_clear = Some(session_id);
                                    }
                                    ob_poc_types::AgentCommand::Delete { index } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: Delete index={} session_id={}",
                                                index, session_id
                                            )
                                            .into(),
                                        );
                                        state.pending_delete = Some((session_id, *index));
                                    }
                                    ob_poc_types::AgentCommand::DeleteLast => {
                                        // DeleteLast uses index u32::MAX as sentinel
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: DeleteLast session_id={}",
                                                session_id
                                            )
                                            .into(),
                                        );
                                        state.pending_delete = Some((session_id, u32::MAX));
                                    }
                                    // Navigation commands
                                    ob_poc_types::AgentCommand::ShowCbu { cbu_id } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ShowCbu cbu_id={}", cbu_id).into(),
                                        );
                                        // TODO: Navigate to CBU
                                    }
                                    ob_poc_types::AgentCommand::HighlightEntity { entity_id } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: HighlightEntity entity_id={}",
                                                entity_id
                                            )
                                            .into(),
                                        );
                                        // TODO: Highlight entity in graph
                                    }
                                    ob_poc_types::AgentCommand::NavigateDsl { line } => {
                                        web_sys::console::log_1(
                                            &format!("Command: NavigateDsl line={}", line).into(),
                                        );
                                        // TODO: Scroll DSL editor to line
                                    }
                                    ob_poc_types::AgentCommand::FocusAst { node_id } => {
                                        web_sys::console::log_1(
                                            &format!("Command: FocusAst node_id={}", node_id)
                                                .into(),
                                        );
                                        // TODO: Focus AST node
                                    }
                                }
                            }
                        } else {
                            web_sys::console::log_1(&"send_chat: no commands in response".into());
                        }
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("send_chat error: {}", e).into());
                        state.last_error = Some(e);
                    }
                }
            }

            // After chat, refetch session to get DSL, AST, bindings
            web_sys::console::log_1(&"send_chat: refetching session state...".into());
            let session_result = api::get_session(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_session = Some(session_result);
            }

            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Validate DSL
    pub fn validate_dsl(&mut self) {
        let dsl = self.buffers.dsl_editor.clone();
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::validate_dsl(&dsl).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_validation = Some(result);
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Execute DSL from editor (requires active session)
    pub fn execute_dsl(&mut self) {
        let Some(session_id) = self.session_id else {
            web_sys::console::warn_1(&"execute_dsl: no session_id".into());
            return;
        };
        let dsl = self.buffers.dsl_editor.clone();
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.executing = true;
            state.execution_handled = false; // Reset so we refetch when complete
        }

        spawn_local(async move {
            let result = api::execute_dsl(session_id, &dsl).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_execution = Some(result);
                state.executing = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Execute session's accumulated DSL (triggered by agent Execute command)
    pub fn execute_session(&mut self, session_id: Uuid) {
        web_sys::console::log_1(&format!("execute_session: starting for {}", session_id).into());
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        {
            let mut state = async_state.lock().unwrap();
            state.executing = true;
            state.execution_handled = false; // Reset so we refetch when complete
        }

        spawn_local(async move {
            web_sys::console::log_1(&"execute_session: calling API...".into());
            let result = api::execute_session(session_id).await;
            web_sys::console::log_1(
                &format!("execute_session: API returned {:?}", result.is_ok()).into(),
            );
            if let Err(ref e) = result {
                web_sys::console::error_1(&format!("execute_session: error: {}", e).into());
            }
            if let Ok(mut state) = async_state.lock() {
                state.pending_execution = Some(result);
                state.executing = false;
                web_sys::console::log_1(&"execute_session: stored result, executing=false".into());
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }
}

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

use crate::api::ApiClient;
use crate::graph::{CbuGraphWidget, LayoutOverride, ViewMode};
use crate::modals::{
    CbuPickerModal, CbuPickerResult, EntityFinderModal, EntityFinderResult, ResolveContext,
};
use crate::panels::{AstPanel, ChatPanel, ChatPanelAction, DslPanel, UnresolvedRefClick};
use crate::state::{
    CbuContext, CbuSummary, ChatMessage, EntityMatch, MessageStatus, Orientation, PendingState,
    SessionContext, SessionResponse, SimpleAstStatement, SystemLevel,
};
use eframe::egui;
use egui::{Color32, RichText};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Debounce delay in seconds before saving layout
const LAYOUT_SAVE_DEBOUNCE_SECS: f64 = 1.0;

/// Main application state
pub struct ObPocApp {
    // API client
    api: ApiClient,

    // Graph widget (top-left panel)
    graph_widget: CbuGraphWidget,

    // Panel widgets
    chat_panel: ChatPanel,
    dsl_panel: DslPanel,
    ast_panel: AstPanel,

    // Modal dialogs
    entity_finder: EntityFinderModal,
    cbu_picker: CbuPickerModal,

    // Session state
    session: SessionContext,
    pending: PendingState,

    // Chat state
    messages: Vec<ChatMessage>,
    dsl_source: String,
    ast_statements: Vec<SimpleAstStatement>,

    // CBU selection
    selected_cbu: Option<Uuid>,
    cbu_list: Vec<CbuSummary>,

    // View mode and orientation
    view_mode: ViewMode,
    orientation: Orientation,

    // Loading/error state
    loading: bool,
    error: Option<String>,

    // Async result holders
    pending_cbu_list: Option<Arc<Mutex<Option<Result<Vec<CbuSummary>, String>>>>>,
    pending_graph: Option<Arc<Mutex<Option<Result<crate::graph::CbuGraphData, String>>>>>,
    pending_layout: Option<Arc<Mutex<Option<Result<LayoutOverride, String>>>>>,
    pending_session: Option<Arc<Mutex<Option<Result<SessionResponse, String>>>>>,
    pending_chat: Option<Arc<Mutex<Option<Result<crate::state::ChatResponse, String>>>>>,
    pending_entity_search: Option<Arc<Mutex<Option<Result<Vec<EntityMatch>, String>>>>>,

    // Deferred layout override to apply after graph loads
    deferred_layout: Option<LayoutOverride>,

    // Debounce state for layout saves
    layout_dirty_since: Option<f64>,

    // Tokio runtime for native builds
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
            api: ApiClient::new(&base_url),
            graph_widget: CbuGraphWidget::new(),
            chat_panel: ChatPanel::new(),
            dsl_panel: DslPanel::new(),
            ast_panel: AstPanel::new(),
            entity_finder: EntityFinderModal::new(),
            cbu_picker: CbuPickerModal::new(),
            session: SessionContext::new(),
            pending: PendingState::new(),
            messages: Vec::new(),
            dsl_source: String::new(),
            ast_statements: Vec::new(),
            selected_cbu: None,
            cbu_list: Vec::new(),
            view_mode: ViewMode::KycUbo,
            orientation: Orientation::Vertical,
            loading: false,
            error: None,
            pending_cbu_list: None,
            pending_graph: None,
            pending_layout: None,
            pending_session: None,
            pending_chat: None,
            pending_entity_search: None,
            deferred_layout: None,
            layout_dirty_since: None,
            #[cfg(not(target_arch = "wasm32"))]
            runtime,
        };

        app.load_cbu_list();
        app
    }

    // =========================================================================
    // API CALLS
    // =========================================================================

    fn load_cbu_list(&mut self) {
        self.loading = true;
        self.error = None;

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

        self.pending_cbu_list = Some(result);
    }

    fn create_session(&mut self) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<SessionResponse, String> = api.post("/api/session", &()).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<SessionResponse, String> = api.post("/api/session", &()).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.pending_session = Some(result);
    }

    fn send_chat_message(&mut self, message: String) {
        // Add user message to chat
        self.messages.push(ChatMessage::User {
            text: message.clone(),
        });

        // Ensure we have a session
        let session_id = match self.session.session_id {
            Some(id) => id,
            None => {
                self.messages.push(ChatMessage::System {
                    text: "No active session. Creating one...".to_string(),
                    level: SystemLevel::Info,
                });
                self.create_session();
                return;
            }
        };

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
                let res: Result<crate::state::ChatResponse, String> =
                    api.post(&path, &request).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.runtime.spawn(async move {
                let res: Result<crate::state::ChatResponse, String> =
                    api.post(&path, &request).await;
                *result_clone.lock().unwrap() = Some(res);
            });
        }

        self.pending_chat = Some(result);
    }

    fn search_entities(&mut self, entity_type: &str, query: &str) {
        let api = self.api.clone();
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();
        let path = format!(
            "/api/entity/search?type={}&q={}",
            urlencoding::encode(entity_type),
            urlencoding::encode(query)
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

        self.pending_entity_search = Some(result);
    }

    fn load_cbu_view(&mut self, cbu_id: Uuid) {
        self.loading = true;
        self.error = None;
        self.deferred_layout = None;

        let api = self.api.clone();
        self.graph_widget.set_view_mode(self.view_mode);

        // Load graph
        let graph_result = Arc::new(Mutex::new(None));
        let graph_result_clone = graph_result.clone();
        let graph_path = format!(
            "/api/cbu/{}/graph?view_mode={}&orientation={}",
            cbu_id,
            self.view_mode.as_str(),
            self.orientation.as_str()
        );

        // Load layout
        let layout_result = Arc::new(Mutex::new(None));
        let layout_result_clone = layout_result.clone();
        let layout_path = format!(
            "/api/cbu/{}/layout?view_mode={}",
            cbu_id,
            self.view_mode.as_str()
        );

        #[cfg(target_arch = "wasm32")]
        {
            let api_graph = api.clone();
            let api_layout = api;

            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<crate::graph::CbuGraphData, String> =
                    api_graph.get(&graph_path).await;
                *graph_result_clone.lock().unwrap() = Some(res);
            });

            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<LayoutOverride, String> = api_layout.get(&layout_path).await;
                *layout_result_clone.lock().unwrap() = Some(res);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let api_graph = api.clone();
            let api_layout = api;

            self.runtime.spawn(async move {
                let res: Result<crate::graph::CbuGraphData, String> =
                    api_graph.get(&graph_path).await;
                *graph_result_clone.lock().unwrap() = Some(res);
            });

            self.runtime.spawn(async move {
                let res: Result<LayoutOverride, String> = api_layout.get(&layout_path).await;
                *layout_result_clone.lock().unwrap() = Some(res);
            });
        }

        self.pending_graph = Some(graph_result);
        self.pending_layout = Some(layout_result);
    }

    // =========================================================================
    // ASYNC RESULT HANDLING
    // =========================================================================

    fn check_pending_requests(&mut self) {
        // Check CBU list
        let cbu_result = self
            .pending_cbu_list
            .as_ref()
            .and_then(|p| p.try_lock().ok())
            .and_then(|mut g| g.take());
        if let Some(result) = cbu_result {
            match result {
                Ok(cbus) => self.cbu_list = cbus,
                Err(e) => self.error = Some(format!("Failed to load CBUs: {}", e)),
            }
            self.loading = false;
            self.pending_cbu_list = None;
        }

        // Check session creation
        let session_result = self
            .pending_session
            .as_ref()
            .and_then(|p| p.try_lock().ok())
            .and_then(|mut g| g.take());
        if let Some(result) = session_result {
            match result {
                Ok(session) => {
                    self.session.session_id = Some(session.session_id);
                    self.messages.push(ChatMessage::System {
                        text: "Session created. You can now chat.".to_string(),
                        level: SystemLevel::Info,
                    });
                }
                Err(e) => {
                    self.messages.push(ChatMessage::System {
                        text: format!("Failed to create session: {}", e),
                        level: SystemLevel::Error,
                    });
                }
            }
            self.pending_session = None;
        }

        // Check chat response
        let chat_result = self
            .pending_chat
            .as_ref()
            .and_then(|p| p.try_lock().ok())
            .and_then(|mut g| g.take());
        if let Some(result) = chat_result {
            match result {
                Ok(response) => {
                    // Update DSL source
                    if let Some(dsl) = &response.dsl_source {
                        self.dsl_source = dsl.clone();
                        self.pending.pending_dsl = Some(dsl.clone());
                    }

                    // Update AST
                    if let Some(ast) = &response.ast {
                        self.ast_statements =
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

                    self.messages.push(ChatMessage::Assistant {
                        text: response
                            .message
                            .unwrap_or_else(|| "DSL generated.".to_string()),
                        dsl: response.dsl_source.clone(),
                        status,
                    });

                    // Show validation errors
                    for err in &response.validation_errors {
                        self.messages.push(ChatMessage::System {
                            text: err.clone(),
                            level: SystemLevel::Warning,
                        });
                    }
                }
                Err(e) => {
                    self.messages.push(ChatMessage::System {
                        text: format!("Chat error: {}", e),
                        level: SystemLevel::Error,
                    });
                }
            }
            self.pending_chat = None;
        }

        // Check entity search results
        let entity_search_result = self
            .pending_entity_search
            .as_ref()
            .and_then(|p| p.try_lock().ok())
            .and_then(|mut g| g.take());
        if let Some(result) = entity_search_result {
            match result {
                Ok(matches) => {
                    self.entity_finder.set_results(matches);
                }
                Err(e) => {
                    self.entity_finder.set_loading(false);
                    self.messages.push(ChatMessage::System {
                        text: format!("Entity search failed: {}", e),
                        level: SystemLevel::Error,
                    });
                }
            }
            self.pending_entity_search = None;
        }

        // Check graph and layout (wait for both)
        let has_pending_graph = self.pending_graph.is_some();
        let has_pending_layout = self.pending_layout.is_some();

        let graph_ready = self
            .pending_graph
            .as_ref()
            .and_then(|p| p.try_lock().ok())
            .map(|g| g.is_some())
            .unwrap_or(false);
        let layout_ready = self
            .pending_layout
            .as_ref()
            .and_then(|p| p.try_lock().ok())
            .map(|g| g.is_some())
            .unwrap_or(false);

        if has_pending_graph && graph_ready && (!has_pending_layout || layout_ready) {
            // Take layout first
            let layout_result = self
                .pending_layout
                .as_ref()
                .and_then(|p| p.try_lock().ok())
                .and_then(|mut g| g.take());
            if let Some(result) = layout_result {
                if let Ok(overrides) = result {
                    self.deferred_layout = Some(overrides);
                }
                self.pending_layout = None;
            }

            // Take graph
            let graph_result = self
                .pending_graph
                .as_ref()
                .and_then(|p| p.try_lock().ok())
                .and_then(|mut g| g.take());
            if let Some(result) = graph_result {
                match result {
                    Ok(graph_data) => {
                        self.graph_widget.set_data(graph_data);
                        if let Some(layout) = self.deferred_layout.take() {
                            self.graph_widget.apply_layout_override(layout);
                        }
                    }
                    Err(e) => self.error = Some(format!("Failed to load graph: {}", e)),
                }
                self.loading = false;
                self.pending_graph = None;
            }
        }
    }

    fn save_layout_debounced(&mut self, now: f64) {
        if self.graph_widget.peek_pending_layout_override() {
            if self.layout_dirty_since.is_none() {
                self.layout_dirty_since = Some(now);
            }

            if let Some(dirty_since) = self.layout_dirty_since {
                if now - dirty_since >= LAYOUT_SAVE_DEBOUNCE_SECS {
                    if let Some(overrides) = self.graph_widget.take_pending_layout_override() {
                        if let Some(cbu_id) = self.selected_cbu {
                            let api = self.api.clone();
                            let path = format!(
                                "/api/cbu/{}/layout?view_mode={}",
                                cbu_id,
                                self.view_mode.as_str()
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
                    }
                    self.layout_dirty_since = None;
                }
            }
        } else {
            self.layout_dirty_since = None;
        }
    }

    // =========================================================================
    // EVENT HANDLERS
    // =========================================================================

    fn handle_chat_action(&mut self, action: ChatPanelAction) {
        match action {
            ChatPanelAction::None => {}
            ChatPanelAction::SendMessage(msg) => {
                self.send_chat_message(msg);
            }
            ChatPanelAction::Execute => {
                // TODO: Execute pending DSL
                self.messages.push(ChatMessage::System {
                    text: "Execute not yet implemented".to_string(),
                    level: SystemLevel::Info,
                });
                self.pending.clear();
            }
            ChatPanelAction::Cancel => {
                self.pending.clear();
                self.dsl_source.clear();
                self.ast_statements.clear();
            }
        }
    }

    fn handle_ast_click(&mut self, click: UnresolvedRefClick) {
        // Open Entity Finder modal
        let context = ResolveContext {
            statement_idx: click.statement_idx,
            arg_key: click.arg_key,
            original_text: click.search_text.clone(),
        };
        self.entity_finder
            .open(&click.entity_type, &click.search_text, context);

        // Trigger initial search
        self.search_entities(&click.entity_type, &click.search_text);
    }

    fn handle_entity_finder_result(&mut self, result: EntityFinderResult) {
        match result {
            EntityFinderResult::None => {}
            EntityFinderResult::Search { entity_type, query } => {
                self.search_entities(&entity_type, &query);
            }
            EntityFinderResult::Selected { context, entity } => {
                self.messages.push(ChatMessage::System {
                    text: format!(
                        "Resolved '{}' to '{}' ({})",
                        context.original_text, entity.name, entity.value
                    ),
                    level: SystemLevel::Info,
                });

                // TODO: Call /api/dsl/resolve-ref to update the AST
                // For now, just update the local AST state
                if let Some(SimpleAstStatement::VerbCall { args, .. }) =
                    self.ast_statements.get_mut(context.statement_idx)
                {
                    for arg in args.iter_mut() {
                        if arg.key == context.arg_key {
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
            }
            EntityFinderResult::Closed => {
                // Modal closed, nothing to do
            }
        }
    }

    fn handle_cbu_picker_result(&mut self, result: CbuPickerResult) {
        match result {
            CbuPickerResult::None => {}
            CbuPickerResult::Selected(cbu) => {
                self.selected_cbu = Some(cbu.cbu_id);
                // Update session context
                self.session.set_cbu(CbuContext {
                    id: cbu.cbu_id,
                    name: cbu.name.clone(),
                    jurisdiction: cbu.jurisdiction.clone(),
                    client_type: cbu.client_type.clone(),
                });
                // Load the CBU view
                self.load_cbu_view(cbu.cbu_id);
            }
            CbuPickerResult::Closed => {
                // Modal closed, nothing to do
            }
        }
    }

    fn handle_dsl_copy(&mut self) {
        // Copy DSL to clipboard
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Some(navigator) = window.navigator().clipboard() {
                    let dsl = self.dsl_source.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let _ =
                            wasm_bindgen_futures::JsFuture::from(navigator.write_text(&dsl)).await;
                    });
                }
            }
        }

        self.messages.push(ChatMessage::System {
            text: "DSL copied to clipboard".to_string(),
            level: SystemLevel::Info,
        });
    }
}

impl eframe::App for ObPocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_pending_requests();

        let now = ctx.input(|i| i.time);

        // Request repaint if async operations pending
        if self.loading
            || self.pending_graph.is_some()
            || self.pending_layout.is_some()
            || self.pending_session.is_some()
            || self.pending_chat.is_some()
            || self.pending_entity_search.is_some()
            || self.entity_finder.is_open()
            || self.cbu_picker.is_open()
        {
            ctx.request_repaint();
        }

        // Global keyboard shortcuts (when modals are not open)
        if !self.entity_finder.is_open() && !self.cbu_picker.is_open() {
            // Ctrl/Cmd+K: Open CBU Picker
            let ctrl_or_cmd = ctx.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
            if ctrl_or_cmd && ctx.input(|i| i.key_pressed(egui::Key::K)) {
                self.cbu_picker.open(self.cbu_list.clone());
            }

            // Ctrl/Cmd+Enter: Execute pending DSL
            if ctrl_or_cmd
                && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                && self.pending.pending_dsl.is_some()
            {
                self.handle_chat_action(ChatPanelAction::Execute);
            }
        }

        // Render modals (before main UI so they appear on top)
        let entity_finder_result = self.entity_finder.ui(ctx);
        self.handle_entity_finder_result(entity_finder_result);

        let cbu_picker_result = self.cbu_picker.ui(ctx);
        self.handle_cbu_picker_result(cbu_picker_result);

        // Track UI events
        let mut clicked_cbu_id: Option<Uuid> = None;
        let mut refresh_clicked = false;
        let mut view_changed = false;
        let mut new_view_mode = self.view_mode;
        let mut orientation_changed = false;
        let mut new_orientation = self.orientation;
        let mut new_session_clicked = false;
        let mut open_cbu_picker = false;

        // =====================================================================
        // TOP PANEL - Header with CBU picker, view mode, controls
        // =====================================================================
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // CBU selector
                ui.label("CBU:");
                let selected_name = self
                    .selected_cbu
                    .and_then(|id| self.cbu_list.iter().find(|c| c.cbu_id == id))
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "Select CBU...".to_string());

                egui::ComboBox::from_id_source("cbu_selector")
                    .selected_text(selected_name)
                    .show_ui(ui, |ui| {
                        for cbu in &self.cbu_list {
                            let label = format!(
                                "{} ({})",
                                cbu.name,
                                cbu.jurisdiction.as_deref().unwrap_or("N/A")
                            );
                            if ui
                                .selectable_label(self.selected_cbu == Some(cbu.cbu_id), label)
                                .clicked()
                            {
                                clicked_cbu_id = Some(cbu.cbu_id);
                            }
                        }
                    });

                if ui.button("🔍").on_hover_text("Search CBUs").clicked() {
                    open_cbu_picker = true;
                }

                if ui.button("↻").on_hover_text("Refresh CBU list").clicked() {
                    refresh_clicked = true;
                }

                ui.separator();

                // View mode selector
                ui.label("View:");
                for mode in ViewMode::all() {
                    if ui
                        .selectable_label(self.view_mode == *mode, mode.display_name())
                        .clicked()
                    {
                        new_view_mode = *mode;
                        view_changed = true;
                    }
                }

                ui.separator();

                // Orientation selector
                ui.label("Layout:");
                for orient in Orientation::all() {
                    if ui
                        .selectable_label(self.orientation == *orient, orient.display_name())
                        .clicked()
                    {
                        new_orientation = *orient;
                        orientation_changed = true;
                    }
                }

                ui.separator();

                // Session controls
                if ui.button("+ New Session").clicked() {
                    new_session_clicked = true;
                }

                if self.session.has_session() {
                    ui.label(
                        egui::RichText::new("● Session Active")
                            .color(egui::Color32::from_rgb(74, 222, 128))
                            .size(11.0),
                    );
                }

                // Loading indicator
                if self.loading {
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

        // Handle header events
        if let Some(cbu_id) = clicked_cbu_id {
            self.selected_cbu = Some(cbu_id);
            // Update session context
            if let Some(cbu) = self.cbu_list.iter().find(|c| c.cbu_id == cbu_id) {
                self.session.set_cbu(CbuContext {
                    id: cbu_id,
                    name: cbu.name.clone(),
                    jurisdiction: cbu.jurisdiction.clone(),
                    client_type: cbu.client_type.clone(),
                });
            }
            self.load_cbu_view(cbu_id);
        }
        if refresh_clicked {
            self.load_cbu_list();
        }
        if view_changed {
            self.view_mode = new_view_mode;
            self.graph_widget.set_view_mode(new_view_mode);
            if let Some(cbu_id) = self.selected_cbu {
                self.load_cbu_view(cbu_id);
            }
        }
        if orientation_changed {
            self.orientation = new_orientation;
            if let Some(cbu_id) = self.selected_cbu {
                self.load_cbu_view(cbu_id);
            }
        }
        if new_session_clicked {
            self.create_session();
        }
        if open_cbu_picker {
            self.cbu_picker.open(self.cbu_list.clone());
        }

        // Save layout with debounce
        self.save_layout_debounced(now);
        if self.layout_dirty_since.is_some() {
            ctx.request_repaint();
        }

        // =====================================================================
        // CENTRAL PANEL - 2x2 Grid Layout
        // =====================================================================
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref err) = self.error {
                ui.colored_label(egui::Color32::RED, err);
                ui.separator();
            }

            let available = ui.available_size();
            let half_width = available.x / 2.0 - 4.0;
            let half_height = available.y / 2.0 - 4.0;

            // Use a grid layout for 2x2 panels
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

                            if self.loading && self.pending_graph.is_some() {
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

                            if self.dsl_panel.ui(ui, &self.dsl_source) {
                                self.handle_dsl_copy();
                            }
                        });

                    ui.end_row();

                    // Bottom-left: Chat
                    let mut chat_action = ChatPanelAction::None;
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(25, 25, 25))
                        .rounding(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(half_width, half_height));
                            ui.set_max_size(egui::vec2(half_width, half_height));

                            chat_action = self.chat_panel.ui(
                                ui,
                                &self.messages,
                                self.pending.pending_dsl.is_some(),
                                self.pending.has_pending(),
                            );
                        });
                    self.handle_chat_action(chat_action);

                    // Bottom-right: AST
                    let mut ast_click: Option<UnresolvedRefClick> = None;
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(25, 25, 25))
                        .rounding(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(half_width, half_height));
                            ui.set_max_size(egui::vec2(half_width, half_height));

                            ast_click = self.ast_panel.ui(ui, &self.ast_statements);
                        });
                    if let Some(click) = ast_click {
                        self.handle_ast_click(click);
                    }
                });
        });
    }
}

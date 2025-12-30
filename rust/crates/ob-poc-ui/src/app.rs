//! Main Application
//!
//! This module implements the eframe::App trait and coordinates:
//! 1. Processing async results at the start of each frame
//! 2. Rendering panels based on layout mode
//! 3. Handling widget responses (no callbacks, return values only)

use crate::api;
use crate::panels::{
    ast_panel, cbu_search_modal, chat_panel, container_browse_panel, context_panel,
    dsl_editor_panel, entity_detail_panel, repl_panel, results_panel, taxonomy_panel, toolbar,
    CbuSearchAction, CbuSearchData, ContainerBrowseAction, ContainerBrowseData, ContextPanelAction,
    DslEditorAction, TaxonomyPanelAction, ToolbarAction, ToolbarData,
};
use crate::state::{AppState, AsyncState, CbuSearchUi, LayoutMode, PanelState, TextBuffers};
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
            resolution: None,
            messages: Vec::new(),
            cbu_list: Vec::new(),
            session_context: None,
            buffers: TextBuffers::default(),
            view_mode: ViewMode::KycUbo,
            panels: PanelState::default(),
            selected_entity_id: None,
            resolution_ui: crate::state::ResolutionPanelUi::default(),
            cbu_search_ui: CbuSearchUi::default(),
            container_browse: crate::panels::ContainerBrowseState::default(),
            token_registry: crate::tokens::TokenRegistry::load_defaults().unwrap_or_else(|e| {
                web_sys::console::warn_1(&format!("Failed to load token config: {}", e).into());
                crate::tokens::TokenRegistry::new()
            }),
            graph_widget: CbuGraphWidget::new(),
            async_state: Arc::new(Mutex::new(AsyncState::default())),
            ctx: Some(cc.egui_ctx.clone()),
            entity_ontology: ob_poc_graph::EntityTypeOntology::new(),
            taxonomy_state: ob_poc_graph::TaxonomyState::new(),
            type_filter: None,
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

        // =================================================================
        // STEP 2: Handle state change flags (SINGLE CENTRAL PLACE)
        // All graph/session refetches happen here, after ALL state changes
        // =================================================================

        // After execution completes, refetch session
        if self.state.should_handle_execution_complete() {
            self.state.refetch_session();
            // Also trigger graph refetch
            if let Ok(mut async_state) = self.state.async_state.lock() {
                async_state.needs_graph_refetch = true;
            }
        }

        // Central graph refetch - triggered by: select_cbu, set_view_mode, execution complete
        if let Some(cbu_id) = self.state.take_pending_graph_refetch() {
            web_sys::console::log_1(
                &format!("update: central graph fetch for cbu_id={}", cbu_id).into(),
            );
            self.state.fetch_graph(cbu_id);
        }

        // Central context refetch - triggered by: select_cbu
        if self.state.take_pending_context_refetch() {
            if let Some(session_id) = self.state.session_id {
                web_sys::console::log_1(
                    &format!(
                        "update: central context fetch for session_id={}",
                        session_id
                    )
                    .into(),
                );
                self.state.fetch_session_context(session_id);
            }
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
        // Process graph filter commands from agent chat
        // =================================================================

        // Handle clear filter command
        if self.state.take_pending_clear_filter() {
            web_sys::console::log_1(&"update: clearing graph filter".into());
            self.state.type_filter = None;
            self.state.taxonomy_state.select(None);
            self.state.graph_widget.clear_type_filter();
        }

        // Handle view mode change command
        if let Some(view_mode_str) = self.state.take_pending_view_mode() {
            web_sys::console::log_1(
                &format!("update: setting view mode to {}", view_mode_str).into(),
            );
            if let Some(mode) = crate::state::parse_view_mode(&view_mode_str) {
                self.state.set_view_mode(mode);
            } else {
                web_sys::console::warn_1(
                    &format!("update: unknown view mode '{}'", view_mode_str).into(),
                );
            }
        }

        // Handle filter by type command
        if let Some(type_codes) = self.state.take_pending_filter_by_type() {
            web_sys::console::log_1(
                &format!("update: filtering graph to types {:?}", type_codes).into(),
            );
            if let Some(first_code) = type_codes.first() {
                // Apply the filter to graph (uses first type code for now)
                self.state.type_filter = Some(first_code.clone());
                self.state.taxonomy_state.select(Some(first_code));
                self.state
                    .graph_widget
                    .set_type_filter(Some(first_code.clone()));
                // Also highlight for visual emphasis
                self.state
                    .graph_widget
                    .set_highlighted_type(Some(first_code.clone()));
            }
        }

        // Handle highlight type command (no filter, just visual emphasis)
        if let Some(type_code) = self.state.take_pending_highlight_type() {
            web_sys::console::log_1(&format!("update: highlighting type {}", type_code).into());
            self.state.taxonomy_state.select(Some(&type_code));
            self.state
                .graph_widget
                .set_highlighted_type(Some(type_code));
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

        // Handle container double-click -> open browse panel
        if let Some(container_info) = self.state.graph_widget.take_container_action() {
            web_sys::console::log_1(
                &format!(
                    "Container double-clicked: {} ({})",
                    container_info.label, container_info.container_type
                )
                .into(),
            );
            self.state.container_browse.open_container(
                container_info.container_id,
                container_info.container_type,
                container_info.label,
                container_info.parent_key,
                container_info.browse_nickname,
            );
        }

        // =================================================================
        // STEP 3: Extract data for rendering (Rule 3: short lock, then render)
        // =================================================================

        // Extract toolbar data (Rule 3: short lock, extract, release, then render)
        let toolbar_data = {
            let last_error = self
                .state
                .async_state
                .lock()
                .ok()
                .and_then(|s| s.last_error.clone());

            ToolbarData {
                current_cbu_name: self
                    .state
                    .session
                    .as_ref()
                    .and_then(|s| s.active_cbu.as_ref())
                    .map(|c| c.name.clone()),
                view_mode: self.state.view_mode,
                layout: self.state.panels.layout,
                last_error,
                is_loading: self.state.is_loading(),
            }
        };

        // Extract CBU search modal data
        let cbu_search_data = CbuSearchData {
            open: self.state.cbu_search_ui.open,
            just_opened: self.state.cbu_search_ui.just_opened,
            results: self
                .state
                .cbu_search_ui
                .results
                .as_ref()
                .map(|r| r.matches.as_slice()),
            searching: self.state.cbu_search_ui.searching,
            truncated: self
                .state
                .cbu_search_ui
                .results
                .as_ref()
                .map(|r| r.truncated)
                .unwrap_or(false),
        };

        // =================================================================
        // STEP 4: Render UI and collect actions
        // =================================================================

        // Top toolbar - returns actions
        let toolbar_action = egui::TopBottomPanel::top("toolbar")
            .show(ctx, |ui| toolbar(ui, &toolbar_data))
            .inner;

        // CBU search modal - returns actions and focus_consumed flag
        let (cbu_search_action, focus_consumed) =
            cbu_search_modal(ctx, &mut self.state.cbu_search_ui.query, &cbu_search_data);

        // Clear just_opened flag after focus is consumed
        if focus_consumed {
            self.state.cbu_search_ui.just_opened = false;
        }

        // =================================================================
        // STEP 5: Handle actions AFTER rendering (Rule 2: actions return values)
        // =================================================================
        self.handle_toolbar_action(toolbar_action);
        self.handle_cbu_search_action(cbu_search_action);

        // Container browse panel (side panel, rendered before central)
        // Extract owned copies to avoid borrow conflicts
        let container_browse_action = if self.state.container_browse.open {
            let cb = &self.state.container_browse;
            let container_id = cb.container_id.clone();
            let container_type = cb.container_type.clone();
            let container_label = cb.container_label.clone();
            let browse_nickname = cb.browse_nickname.clone();
            let parent_key = cb.parent_key.clone();
            let active_filters = cb.active_filters.clone();
            let offset = cb.offset;
            let limit = cb.limit;
            let selected_idx = cb.selected_idx;

            let container_browse_data = ContainerBrowseData {
                open: true,
                container_id: container_id.as_deref(),
                container_type: container_type.as_deref(),
                container_label: container_label.as_deref(),
                browse_nickname: browse_nickname.as_deref(),
                parent_key: parent_key.as_deref(),
                active_filters: &active_filters,
                available_facets: &[], // TODO: populate from server
                offset,
                limit,
                total_count: 0, // TODO: populate from server
                items: &[],     // TODO: populate from server
                loading: false, // TODO: track loading state
                error: None,
                selected_idx,
            };

            container_browse_panel(
                ctx,
                &mut self.state.container_browse,
                &container_browse_data,
            )
        } else {
            None
        };
        self.handle_container_browse_action(container_browse_action);

        // Context panel (left side) - shows session context and semantic stages
        let context_action = egui::SidePanel::left("context_panel")
            .default_width(220.0)
            .min_width(180.0)
            .max_width(350.0)
            .resizable(true)
            .show(ctx, |ui| context_panel(ui, &self.state))
            .inner;

        // Handle context panel actions
        self.handle_context_panel_action(context_action);

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| match self.state.panels.layout {
            LayoutMode::FourPanel => self.render_four_panel(ui),
            LayoutMode::EditorFocus => self.render_editor_focus(ui),
            LayoutMode::GraphFocus => self.render_graph_focus(ui),
            LayoutMode::GraphFullSize => self.render_graph_full_size(ui),
        });

        // =================================================================
        // STEP 6: Request repaint if async operations in progress
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

        if action.open_cbu_search {
            self.state.cbu_search_ui.open = true;
            self.state.cbu_search_ui.just_opened = true;
            self.state.cbu_search_ui.query.clear();
            self.state.cbu_search_ui.results = None;
        }

        if let Some(mode) = action.change_view_mode {
            self.state.set_view_mode(mode);
        }

        if let Some(layout) = action.change_layout {
            self.state.panels.layout = layout;
        }

        if action.dismiss_error {
            if let Ok(mut async_state) = self.state.async_state.lock() {
                async_state.last_error = None;
            }
        }
    }

    /// Handle CBU search modal actions
    fn handle_cbu_search_action(&mut self, action: Option<CbuSearchAction>) {
        let Some(action) = action else { return };

        match action {
            CbuSearchAction::Search { query } => {
                self.state.search_cbus(&query);
            }
            CbuSearchAction::Select { id, name } => {
                // Close modal
                self.state.cbu_search_ui.open = false;
                self.state.cbu_search_ui.results = None;

                // Select the CBU
                if let Ok(uuid) = Uuid::parse_str(&id) {
                    self.state.select_cbu(uuid, &name);
                }
            }
            CbuSearchAction::Close => {
                self.state.cbu_search_ui.open = false;
                self.state.cbu_search_ui.results = None;
            }
        }
    }

    /// Handle DSL editor actions
    fn handle_dsl_editor_action(&mut self, action: DslEditorAction) {
        match action {
            DslEditorAction::None => {}
            DslEditorAction::Clear => {
                self.state.clear_dsl();
            }
            DslEditorAction::Validate => {
                self.state.validate_dsl();
            }
            DslEditorAction::Execute => {
                self.state.execute_dsl();
            }
        }
    }

    /// Handle container browse panel actions
    fn handle_container_browse_action(&mut self, action: Option<ContainerBrowseAction>) {
        let Some(action) = action else { return };

        match action {
            ContainerBrowseAction::Close => {
                self.state.container_browse.close();
            }
            ContainerBrowseAction::Search {
                query: _,
                filters: _,
                offset: _,
                limit: _,
            } => {
                // TODO: Trigger search via EntityGateway
                web_sys::console::log_1(
                    &"ContainerBrowse: Search action (not yet implemented)".into(),
                );
            }
            ContainerBrowseAction::PageChange { offset } => {
                self.state.container_browse.offset = offset;
                // TODO: Trigger refetch
            }
            ContainerBrowseAction::SelectItem { id } => {
                web_sys::console::log_1(&format!("ContainerBrowse: Selected item {}", id).into());
                // TODO: Highlight in graph or show detail
            }
            ContainerBrowseAction::OpenItem { id } => {
                web_sys::console::log_1(&format!("ContainerBrowse: Open item {}", id).into());
                // TODO: Navigate to entity detail
            }
            ContainerBrowseAction::FilterChange { field, value } => {
                self.state.container_browse.set_filter(field, value);
                // TODO: Trigger refetch
            }
        }
    }

    /// Handle context panel actions (stage focus, scope switching)
    fn handle_context_panel_action(&mut self, action: Option<ContextPanelAction>) {
        let Some(action) = action else { return };

        match action {
            ContextPanelAction::FocusStage { stage_code } => {
                web_sys::console::log_1(
                    &format!("ContextPanel: Focus stage {}", stage_code).into(),
                );
                self.state.set_stage_focus(Some(stage_code));
            }
            ContextPanelAction::ClearStageFocus => {
                web_sys::console::log_1(&"ContextPanel: Clear stage focus".into());
                self.state.set_stage_focus(None);
            }
            ContextPanelAction::SwitchScope { cbu_id, cbu_name } => {
                web_sys::console::log_1(
                    &format!("ContextPanel: Switch to CBU {} ({})", cbu_name, cbu_id).into(),
                );
                // Parse UUID and switch CBU context
                if let Ok(uuid) = Uuid::parse_str(&cbu_id) {
                    self.state.select_cbu(uuid, &cbu_name);
                }
            }
            ContextPanelAction::SelectContext {
                context_type,
                context_id,
                display_label,
            } => {
                web_sys::console::log_1(
                    &format!(
                        "ContextPanel: Select {} '{}' ({})",
                        context_type, display_label, context_id
                    )
                    .into(),
                );
                // 1. Set as selected entity in UI (for entity detail panel)
                self.state.selected_entity_id = Some(context_id.clone());

                // 2. Bind to session so agent knows about the selected context
                if let Ok(uuid) = Uuid::parse_str(&context_id) {
                    self.state
                        .bind_context_entity(uuid, &context_type, &display_label);
                }

                // 3. Highlight in graph if entity is visible
                self.state.graph_widget.focus_entity(&context_id);
            }
        }
    }

    /// Handle taxonomy browser actions (type selection, filtering)
    fn handle_taxonomy_action(&mut self, action: TaxonomyPanelAction) {
        match action {
            TaxonomyPanelAction::None => {}
            TaxonomyPanelAction::ToggleExpand { type_code } => {
                self.state.taxonomy_state.toggle(&type_code);
            }
            TaxonomyPanelAction::SelectType { type_code } => {
                web_sys::console::log_1(&format!("Taxonomy: Select type {}", type_code).into());
                self.state.taxonomy_state.select(Some(&type_code));
                // Highlight matching entities in graph (single-click = highlight, not filter)
                self.state
                    .graph_widget
                    .set_highlighted_type(Some(type_code));
            }
            TaxonomyPanelAction::ClearSelection => {
                self.state.taxonomy_state.select(None);
                self.state.type_filter = None;
                // Clear both highlight and filter on graph
                self.state.graph_widget.clear_type_filter();
            }
            TaxonomyPanelAction::FilterToType { type_code } => {
                web_sys::console::log_1(&format!("Taxonomy: Filter to type {}", type_code).into());
                self.state.type_filter = Some(type_code.clone());
                self.state.taxonomy_state.select(Some(&type_code));
                // Apply hard filter to graph (double-click = filter, dims non-matching)
                self.state
                    .graph_widget
                    .set_type_filter(Some(type_code.clone()));
                // Also set as highlighted for visual emphasis
                self.state
                    .graph_widget
                    .set_highlighted_type(Some(type_code));
            }
            TaxonomyPanelAction::ExpandAll => {
                self.state
                    .taxonomy_state
                    .expand_to_depth(&self.state.entity_ontology, 10);
            }
            TaxonomyPanelAction::CollapseAll => {
                self.state.taxonomy_state.collapse_all();
            }
        }
    }

    /// Render layout:
    /// - Top 50%: Graph (full width)
    /// - Bottom left 60%: Unified REPL (chat + resolution + DSL)
    /// - Bottom right 40%: Results/AST/Entity tabs
    fn render_four_panel(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let top_height = available.y * 0.5;
        let bottom_height = available.y * 0.5 - 4.0;

        // Top: Graph (full width, 50% height) with taxonomy browser overlay
        let taxonomy_action = ui
            .allocate_ui(egui::vec2(available.x, top_height), |ui| {
                egui::Frame::default()
                    .inner_margin(0.0)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
                        // Horizontal split: taxonomy browser (left) + graph (right)
                        ui.horizontal(|ui| {
                            // Taxonomy browser (collapsible, 180px when open)
                            let taxonomy_width = 180.0;
                            let action = ui
                                .allocate_ui(
                                    egui::vec2(taxonomy_width, ui.available_height()),
                                    |ui| taxonomy_panel(ui, &self.state, ui.available_height()),
                                )
                                .inner;

                            // Graph takes remaining space
                            ui.vertical(|ui| {
                                self.state.graph_widget.ui(ui);
                            });

                            action
                        })
                        .inner
                    })
                    .inner
            })
            .inner;

        // Handle taxonomy actions AFTER rendering (Rule 2)
        self.handle_taxonomy_action(taxonomy_action);

        ui.separator();

        // Bottom row
        ui.horizontal(|ui| {
            ui.set_height(bottom_height);

            // Unified REPL panel (left, 60% width) - chat + resolution + DSL
            ui.vertical(|ui| {
                ui.set_width(available.x * 0.6 - 4.0);
                egui::Frame::default()
                    .inner_margin(8.0)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
                        repl_panel(ui, &mut self.state);
                    });
            });

            ui.separator();

            // Right side (40% width) - Results/AST/Entity tabs
            ui.vertical(|ui| {
                ui.set_width(available.x * 0.4 - 4.0);

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

                egui::Frame::default()
                    .inner_margin(8.0)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
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
    }

    /// Render editor-focused layout (large editor, small side panels)
    fn render_editor_focus(&mut self, ui: &mut egui::Ui) {
        let mut dsl_action = DslEditorAction::None;

        ui.horizontal(|ui| {
            // Editor (70% width)
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() * 0.7);
                dsl_action = dsl_editor_panel(ui, &mut self.state);
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

        // Handle action after render
        self.handle_dsl_editor_action(dsl_action);
    }

    /// Render graph-focused layout (large graph, small side panels)
    fn render_graph_focus(&mut self, ui: &mut egui::Ui) {
        let mut dsl_action = DslEditorAction::None;

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
                    dsl_action = dsl_editor_panel(ui, &mut self.state);
                });
            });
        });

        // Handle action after render
        self.handle_dsl_editor_action(dsl_action);
    }

    /// Render graph full size (graph only, full window)
    fn render_graph_full_size(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default()
            .inner_margin(0.0)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .show(ui, |ui| {
                self.state.graph_widget.ui(ui);
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

    /// Fetch graph for specific CBU (called from central update() loop only)
    pub fn fetch_graph(&mut self, cbu_id: Uuid) {
        web_sys::console::log_1(
            &format!(
                "fetch_graph: cbu_id={}, view_mode={:?}",
                cbu_id, self.view_mode
            )
            .into(),
        );

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_graph = true;
        }

        let view_mode = self.view_mode;
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            web_sys::console::log_1(
                &format!("fetch_graph: calling API for {:?}...", view_mode).into(),
            );
            let result = api::get_cbu_graph(cbu_id, view_mode).await;
            web_sys::console::log_1(
                &format!("fetch_graph: API returned, success={}", result.is_ok()).into(),
            );
            if let Err(ref e) = result {
                web_sys::console::error_1(&format!("fetch_graph error: {}", e).into());
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

    /// Select a CBU - sets state, graph refetch happens centrally in update()
    pub fn select_cbu(&mut self, cbu_id: Uuid, display_name: &str) {
        tracing::info!(
            "select_cbu called: cbu_id={}, old_session_id={:?}",
            cbu_id,
            self.session_id
        );

        // Use CBU ID as session ID (session key = entity key)
        self.session_id = Some(cbu_id);
        let _ = api::set_local_storage("session_id", &cbu_id.to_string());

        // Set flags - actual fetch happens in update() after all state changes
        if let Ok(mut state) = self.async_state.lock() {
            state.pending_cbu_id = Some(cbu_id);
            state.needs_graph_refetch = true;
            state.needs_context_refetch = true;
        }

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

    /// Set view mode - graph refetch happens centrally in update()
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        web_sys::console::log_1(
            &format!(
                "set_view_mode: changing from {:?} to {:?}",
                self.view_mode, mode
            )
            .into(),
        );
        self.view_mode = mode;
        self.graph_widget.set_view_mode(mode);
        // Set flag - actual fetch happens in update() after all state changes
        if let Ok(mut state) = self.async_state.lock() {
            state.needs_graph_refetch = true;
        }
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
                                    // TODO: Implement these commands
                                    ob_poc_types::AgentCommand::Undo
                                    | ob_poc_types::AgentCommand::Clear
                                    | ob_poc_types::AgentCommand::Delete { .. }
                                    | ob_poc_types::AgentCommand::DeleteLast => {
                                        web_sys::console::warn_1(
                                            &format!("Command not implemented: {:?}", cmd).into(),
                                        );
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
                                    // Graph filtering commands - store in AsyncState for processing in update loop
                                    ob_poc_types::AgentCommand::FilterByType { type_codes } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: FilterByType type_codes={:?}",
                                                type_codes
                                            )
                                            .into(),
                                        );
                                        state.pending_filter_by_type = Some(type_codes.clone());
                                    }
                                    ob_poc_types::AgentCommand::HighlightType { type_code } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: HighlightType type_code={}",
                                                type_code
                                            )
                                            .into(),
                                        );
                                        state.pending_highlight_type = Some(type_code.clone());
                                    }
                                    ob_poc_types::AgentCommand::ClearFilter => {
                                        web_sys::console::log_1(&"Command: ClearFilter".into());
                                        state.pending_clear_filter = true;
                                    }
                                    ob_poc_types::AgentCommand::SetViewMode { view_mode } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: SetViewMode view_mode={}",
                                                view_mode
                                            )
                                            .into(),
                                        );
                                        state.pending_view_mode = Some(view_mode.clone());
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

    /// Clear DSL editor
    pub fn clear_dsl(&mut self) {
        self.buffers.dsl_editor.clear();
        self.buffers.dsl_dirty = false;
        self.validation_result = None;
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

    // =========================================================================
    // Resolution Methods
    // =========================================================================

    /// Start entity resolution for current session's DSL
    pub fn start_resolution(&mut self) {
        let Some(session_id) = self.session_id else {
            web_sys::console::warn_1(&"start_resolution: no session_id".into());
            return;
        };

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_resolution = true;
        }

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::start_resolution(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_resolution = Some(result);
                state.loading_resolution = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Search for entity matches for a specific ref
    pub fn search_resolution(&mut self, ref_id: &str) {
        let Some(session_id) = self.session_id else {
            return;
        };

        let query = self.resolution_ui.search_query.clone();
        let discriminators = self.resolution_ui.discriminator_values.clone();
        let ref_id = ref_id.to_string();

        {
            let mut state = self.async_state.lock().unwrap();
            state.searching_resolution = true;
        }

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::search_resolution(session_id, &ref_id, &query, discriminators).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_resolution_search = Some(result);
                state.searching_resolution = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Select an entity for a ref
    pub fn select_resolution(&mut self, ref_id: &str, resolved_key: &str) {
        let Some(session_id) = self.session_id else {
            return;
        };

        let ref_id = ref_id.to_string();
        let resolved_key = resolved_key.to_string();

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_resolution = true;
        }

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::select_resolution(session_id, &ref_id, &resolved_key).await;
            if let Ok(mut state) = async_state.lock() {
                match result {
                    Ok(response) => {
                        state.pending_resolution = Some(Ok(response.session));
                    }
                    Err(e) => {
                        state.pending_resolution = Some(Err(e));
                    }
                }
                state.loading_resolution = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });

        // Clear selection UI state
        self.resolution_ui.selected_ref_id = None;
        self.resolution_ui.search_results = None;
    }

    /// Confirm all high-confidence resolutions
    pub fn confirm_all_resolutions(&mut self) {
        let Some(session_id) = self.session_id else {
            return;
        };

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_resolution = true;
        }

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::confirm_all_resolutions(session_id, Some(0.8)).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_resolution = Some(result);
                state.loading_resolution = false;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Commit resolutions to AST
    pub fn commit_resolution(&mut self) {
        let Some(session_id) = self.session_id else {
            return;
        };

        {
            let mut state = self.async_state.lock().unwrap();
            state.loading_resolution = true;
        }

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::commit_resolution(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.loading_resolution = false;
                match result {
                    Ok(commit_response) => {
                        if commit_response.success {
                            // Clear resolution state on successful commit
                            // The session will be refetched to get updated DSL
                        } else {
                            state.last_error = Some(commit_response.message);
                        }
                    }
                    Err(e) => {
                        state.last_error = Some(format!("Commit failed: {}", e));
                    }
                }
            }
            // Refetch session to get updated DSL
            let session_result = api::get_session(session_id).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_session = Some(session_result);
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });

        // Clear resolution UI state
        self.resolution = None;
        self.resolution_ui = crate::state::ResolutionPanelUi::default();
    }

    /// Cancel resolution session
    pub fn cancel_resolution(&mut self) {
        let Some(session_id) = self.session_id else {
            return;
        };

        let ctx = self.ctx.clone();

        spawn_local(async move {
            let _ = api::cancel_resolution(session_id).await;
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });

        // Clear resolution state immediately
        self.resolution = None;
        self.resolution_ui = crate::state::ResolutionPanelUi::default();
    }

    // =========================================================================
    // CBU Search Methods
    // =========================================================================

    /// Search CBUs using EntityGateway fuzzy search
    pub fn search_cbus(&mut self, query: &str) {
        if query.len() < 2 {
            return;
        }

        self.cbu_search_ui.searching = true;

        let query = query.to_string();
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::search_cbus(&query, 15).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_cbu_search = Some(result);
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    // =========================================================================
    // Stage Focus Methods
    // =========================================================================

    /// Set or clear stage focus - calls POST /api/session/:id/focus
    pub fn set_stage_focus(&mut self, stage_code: Option<String>) {
        let Some(session_id) = self.session_id else {
            web_sys::console::warn_1(&"set_stage_focus: no session_id".into());
            return;
        };

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();

        spawn_local(async move {
            let result = api::set_stage_focus(session_id, stage_code.as_deref()).await;
            match result {
                Ok(response) => {
                    web_sys::console::log_1(
                        &format!(
                            "set_stage_focus: success, stage={:?}, verbs={}",
                            response.stage_code,
                            response.relevant_verbs.len()
                        )
                        .into(),
                    );
                    // Refetch session to get updated context with stage_focus
                    let session_result = api::get_session(session_id).await;
                    if let Ok(mut state) = async_state.lock() {
                        state.pending_session = Some(session_result);
                    }
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("set_stage_focus error: {}", e).into());
                    if let Ok(mut state) = async_state.lock() {
                        state.last_error = Some(format!("Failed to set stage focus: {}", e));
                    }
                }
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Bind a context entity (KYC case, product, ISDA, etc.) to the session
    /// This makes the agent aware of the user's current focus
    pub fn bind_context_entity(
        &mut self,
        entity_id: Uuid,
        context_type: &str,
        display_label: &str,
    ) {
        let Some(session_id) = self.session_id else {
            web_sys::console::warn_1(&"bind_context_entity: no session_id".into());
            return;
        };

        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();
        let context_type = context_type.to_string();
        let display_label = display_label.to_string();

        spawn_local(async move {
            let result =
                api::bind_entity(session_id, entity_id, &context_type, &display_label).await;
            match result {
                Ok(_response) => {
                    web_sys::console::log_1(
                        &format!(
                            "bind_context_entity: bound {} '{}' to session",
                            context_type, display_label
                        )
                        .into(),
                    );
                    // Refetch session to get updated bindings
                    let session_result = api::get_session(session_id).await;
                    if let Ok(mut state) = async_state.lock() {
                        state.pending_session = Some(session_result);
                    }
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("bind_context_entity error: {}", e).into());
                    if let Ok(mut state) = async_state.lock() {
                        state.last_error = Some(format!("Failed to bind {}: {}", context_type, e));
                    }
                }
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }
}

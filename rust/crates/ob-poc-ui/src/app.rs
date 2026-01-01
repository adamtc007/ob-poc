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
    trading_matrix_panel, CbuSearchAction, CbuSearchData, ContainerBrowseAction,
    ContainerBrowseData, ContextPanelAction, DslEditorAction, TaxonomyPanelAction, ToolbarAction,
    ToolbarData, TradingMatrixPanelAction,
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
            trading_matrix: None,
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
            trading_matrix_state: ob_poc_graph::TradingMatrixState::new(),
            selected_matrix_node: None,
        };

        // Try to restore session from localStorage
        state.restore_session();

        // Fetch initial CBU list
        state.fetch_cbu_list();

        // Install voice command listener (WASM only)
        #[cfg(target_arch = "wasm32")]
        {
            if let Err(e) = crate::voice_bridge::install_voice_listener() {
                web_sys::console::warn_1(
                    &format!("Failed to install voice listener: {:?}", e).into(),
                );
            }
        }

        Self { state }
    }

    // =========================================================================
    // Voice Command Processing (WASM only)
    // =========================================================================

    /// Process pending voice commands from the JavaScript voice bridge.
    /// Commands flow through the unified dispatcher and then either:
    /// - NavigationVerb → executed locally
    /// - AgentPrompt → sent via AgentPromptConduit
    #[cfg(target_arch = "wasm32")]
    fn process_voice_commands(&mut self) {
        use crate::command::{
            dispatch_command, CommandResult, CommandSource, InvestigationContext,
        };
        use crate::voice_bridge::take_pending_voice_commands;

        // Take all pending voice commands from the queue
        let commands = take_pending_voice_commands();
        if commands.is_empty() {
            return;
        }

        // Build investigation context from current app state
        let context = InvestigationContext {
            focused_entity_id: self.state.graph_widget.selected_entity_id(),
            current_cbu_id: self
                .state
                .session
                .as_ref()
                .and_then(|s| s.active_cbu.as_ref().map(|cbu| cbu.id.clone())),
            current_view_mode: self.state.graph_widget.view_mode(),
            current_zoom: 1.0,
            selected_entities: self
                .state
                .graph_widget
                .selected_entity_id()
                .map(|id| vec![id])
                .unwrap_or_default(),
        };

        for cmd in commands {
            let source = CommandSource::Voice {
                transcript: cmd.transcript.clone(),
                confidence: cmd.confidence,
                provider: crate::command::VoiceProvider::from_str(&cmd.provider),
            };

            let result = dispatch_command(source, &context);

            web_sys::console::log_1(
                &format!(
                    "Voice command: '{}' -> {:?} (confidence: {:.2})",
                    cmd.transcript, result, cmd.confidence
                )
                .into(),
            );

            // Route based on command result
            match result {
                CommandResult::Navigation(verb) => self.execute_navigation_verb(verb),
                CommandResult::Agent(prompt) => self.send_to_agent(prompt),
                CommandResult::None => {}
            }
        }
    }

    /// Execute a navigation verb from any command source (voice, chat, egui).
    /// These are LOCAL UI commands - no server round-trip needed.
    #[cfg(target_arch = "wasm32")]
    fn execute_navigation_verb(&mut self, verb: crate::command::NavigationVerb) {
        use crate::command::NavigationVerb;
        use ob_poc_graph::ViewMode;
        use ob_poc_types::PanDirection;

        match verb {
            NavigationVerb::None => {}

            // Zoom commands
            NavigationVerb::ZoomIn { factor } => {
                self.state.graph_widget.zoom_in(factor);
            }
            NavigationVerb::ZoomOut { factor } => {
                self.state.graph_widget.zoom_out(factor);
            }
            NavigationVerb::ZoomFit => {
                self.state.graph_widget.zoom_to_fit();
            }
            NavigationVerb::ZoomTo { level } => {
                self.state.graph_widget.set_zoom(level);
            }

            // Pan commands
            NavigationVerb::Pan { direction, amount } => {
                let amt = amount.unwrap_or(100.0);
                match direction {
                    PanDirection::Left => self.state.graph_widget.pan(-amt, 0.0),
                    PanDirection::Right => self.state.graph_widget.pan(amt, 0.0),
                    PanDirection::Up => self.state.graph_widget.pan(0.0, -amt),
                    PanDirection::Down => self.state.graph_widget.pan(0.0, amt),
                }
            }
            NavigationVerb::Center => {
                self.state.graph_widget.center_view();
            }
            NavigationVerb::Stop => {
                self.state.graph_widget.stop_animation();
            }
            NavigationVerb::ResetLayout => {
                self.state.graph_widget.reset_layout();
            }

            // Focus/Selection
            NavigationVerb::FocusEntity { entity_id } => {
                self.state.graph_widget.focus_on_entity(&entity_id);
            }

            // View Mode
            NavigationVerb::SetViewMode { mode } => {
                self.state.graph_widget.set_view_mode(mode);
                self.trigger_graph_refresh();
            }

            // Filtering
            NavigationVerb::FilterByType { type_codes } => {
                self.state.type_filter = Some(type_codes);
            }
            NavigationVerb::HighlightType { type_code } => {
                self.state.graph_widget.highlight_type(&type_code);
            }
            NavigationVerb::ClearFilter => {
                self.state.type_filter = None;
                self.state.graph_widget.clear_highlight();
            }

            // Scale navigation (astronomical metaphor)
            NavigationVerb::ScaleUniverse => {
                self.state.graph_widget.zoom_to_fit();
            }
            NavigationVerb::ScaleGalaxy { segment: _ } => {
                // Zoom to segment level
                self.state.graph_widget.set_zoom(0.3);
            }
            NavigationVerb::ScaleSystem { cbu_id } => {
                if let Some(id) = cbu_id {
                    self.state.select_cbu(Uuid::parse_str(&id).ok());
                }
            }
            NavigationVerb::ScalePlanet { entity_id } => {
                if let Some(id) = entity_id {
                    self.state.graph_widget.focus_on_entity(&id);
                }
            }
            NavigationVerb::ScaleSurface => {
                self.state.graph_widget.set_zoom(1.5);
            }
            NavigationVerb::ScaleCore => {
                self.state.graph_widget.set_zoom(3.0);
            }

            // Depth navigation
            NavigationVerb::DrillThrough => {
                self.state.graph_widget.zoom_in(Some(1.5));
            }
            NavigationVerb::SurfaceReturn => {
                self.state.graph_widget.zoom_to_fit();
            }
            NavigationVerb::Xray => {
                // Toggle transparency mode
                web_sys::console::log_1(&"X-ray mode toggled".into());
            }
            NavigationVerb::Peel => {
                // Peel layer
                web_sys::console::log_1(&"Layer peeled".into());
            }
            NavigationVerb::CrossSection => {
                // Show cross section
                web_sys::console::log_1(&"Cross section view".into());
            }
            NavigationVerb::DepthIndicator => {
                // Show depth indicator
                web_sys::console::log_1(&"Depth indicator shown".into());
            }

            // Orbital navigation
            NavigationVerb::Orbit { entity_id } => {
                if let Some(id) = entity_id {
                    self.state.graph_widget.focus_on_entity(&id);
                }
            }
            NavigationVerb::RotateLayer { layer: _ } => {
                web_sys::console::log_1(&"Rotating layer".into());
            }
            NavigationVerb::Flip => {
                web_sys::console::log_1(&"View flipped".into());
            }
            NavigationVerb::Tilt { dimension: _ } => {
                web_sys::console::log_1(&"View tilted".into());
            }

            // Temporal navigation
            NavigationVerb::TimeRewind { target_date: _ } => {
                web_sys::console::log_1(&"Time rewind".into());
            }
            NavigationVerb::TimePlay { from: _, to: _ } => {
                web_sys::console::log_1(&"Timeline playing".into());
            }
            NavigationVerb::TimeFreeze => {
                web_sys::console::log_1(&"Time frozen".into());
            }
            NavigationVerb::TimeSlice { date1: _, date2: _ } => {
                web_sys::console::log_1(&"Time slice comparison".into());
            }
            NavigationVerb::TimeTrail { entity_id: _ } => {
                web_sys::console::log_1(&"Showing time trail".into());
            }

            // Investigation patterns (these are navigation, not agent)
            NavigationVerb::FollowMoney { from_entity } => {
                if let Some(id) = from_entity {
                    self.state.graph_widget.focus_on_entity(&id);
                }
                // TODO: Highlight money flow edges
            }
            NavigationVerb::WhoControls { entity_id } => {
                if let Some(id) = entity_id {
                    self.state.graph_widget.focus_on_entity(&id);
                }
                // TODO: Trace control chain visually
            }
            NavigationVerb::Illuminate { aspect: _ } => {
                // Highlight specific aspect
            }
            NavigationVerb::Shadow => {
                // Dim background entities
            }
            NavigationVerb::RedFlagScan => {
                // Highlight red flag entities
            }
            NavigationVerb::BlackHole => {
                // Highlight entities with missing data
            }

            // Context
            NavigationVerb::SetContext { context: _ } => {
                // Switch investigation context
            }
        }
    }

    /// Trigger a graph data refresh from the server
    #[cfg(target_arch = "wasm32")]
    fn trigger_graph_refresh(&mut self) {
        if let Ok(mut state) = self.state.async_state.lock() {
            state.needs_graph_refetch = true;
        }
    }
}

// =============================================================================
// AgentPromptConduit Implementation
// =============================================================================

#[cfg(target_arch = "wasm32")]
impl crate::command::AgentPromptConduit for App {
    /// Send an agent prompt for processing.
    /// This is the SINGLE entry point for ALL agent-bound commands,
    /// regardless of source (voice, chat, egui button).
    fn send_to_agent(&mut self, prompt: crate::command::AgentPrompt) {
        use crate::command::AgentPrompt;

        // Log the prompt with source attribution
        let source_desc = match prompt.source() {
            crate::command::PromptSource::Voice { confidence, .. } => {
                format!("voice (confidence: {:.0}%)", confidence * 100.0)
            }
            crate::command::PromptSource::ChatInput => "chat".to_string(),
            crate::command::PromptSource::EguiWidget { widget_id } => {
                format!("widget:{}", widget_id)
            }
            crate::command::PromptSource::System => "system".to_string(),
        };

        web_sys::console::log_1(
            &format!(
                "[AgentConduit] Prompt from {}: {}",
                source_desc,
                prompt.to_chat_message()
            )
            .into(),
        );

        // Check if we're ready to send (not already processing)
        if !self.is_ready() {
            web_sys::console::warn_1(&"[AgentConduit] Request in flight, queueing...".into());
            // TODO: Queue the prompt for later
            return;
        }

        // Convert to chat message and send
        let message = prompt.to_chat_message();

        // Get session ID
        let Some(session_id) = self.state.session_id else {
            web_sys::console::warn_1(&"[AgentConduit] No session, cannot send".into());
            return;
        };

        // Use the existing chat infrastructure
        self.state.buffers.chat_input = message;
        self.state.send_chat(session_id);
    }

    /// Check if the conduit is ready to accept new prompts.
    fn is_ready(&self) -> bool {
        if let Ok(state) = self.state.async_state.lock() {
            !state.loading_chat
        } else {
            false
        }
    }

    /// Get the last error from the conduit, if any.
    fn last_error(&self) -> Option<&str> {
        // TODO: Track last error in async state
        None
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // =================================================================
        // STEP 1: Process any pending async results
        // =================================================================
        self.state.process_async_results();

        // =================================================================
        // STEP 1.5: Process voice commands (WASM only)
        // Voice commands flow through unified dispatcher
        // =================================================================
        #[cfg(target_arch = "wasm32")]
        {
            self.process_voice_commands();
        }

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

        // Central trading matrix refetch - triggered by: Trading view mode selected
        if let Some(cbu_id) = self.state.take_pending_trading_matrix_refetch() {
            web_sys::console::log_1(
                &format!("update: central trading matrix fetch for cbu_id={}", cbu_id).into(),
            );
            self.state.fetch_trading_matrix(cbu_id);
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
        // Process Esper-style navigation commands from agent chat
        // =================================================================

        // Handle zoom in command
        if let Some(factor) = self.state.take_pending_zoom_in() {
            web_sys::console::log_1(&format!("update: zoom in factor={}", factor).into());
            self.state.graph_widget.zoom_in(Some(factor));
        }

        // Handle zoom out command
        if let Some(factor) = self.state.take_pending_zoom_out() {
            web_sys::console::log_1(&format!("update: zoom out factor={}", factor).into());
            self.state.graph_widget.zoom_out(Some(factor));
        }

        // Handle zoom fit command
        if self.state.take_pending_zoom_fit() {
            web_sys::console::log_1(&"update: zoom fit".into());
            self.state.graph_widget.zoom_fit();
        }

        // Handle zoom to level command
        if let Some(level) = self.state.take_pending_zoom_to() {
            web_sys::console::log_1(&format!("update: zoom to level={}", level).into());
            self.state.graph_widget.zoom_to_level(level);
        }

        // Handle pan command
        if let Some((direction, amount)) = self.state.take_pending_pan() {
            web_sys::console::log_1(
                &format!("update: pan direction={:?} amount={:?}", direction, amount).into(),
            );
            self.state.graph_widget.pan_direction(direction, amount);
        }

        // Handle center command
        if self.state.take_pending_center() {
            web_sys::console::log_1(&"update: center view".into());
            self.state.graph_widget.center_view();
        }

        // Handle stop command
        if self.state.take_pending_stop() {
            web_sys::console::log_1(&"update: stop animation".into());
            self.state.graph_widget.stop_animation();
        }

        // Handle focus entity command
        if let Some(entity_id) = self.state.take_pending_focus_entity() {
            web_sys::console::log_1(&format!("update: focus entity {}", entity_id).into());
            self.state.graph_widget.focus_entity(&entity_id);
            self.state.selected_entity_id = Some(entity_id);
        }

        // Handle reset layout command
        if self.state.take_pending_reset_layout() {
            web_sys::console::log_1(&"update: reset layout".into());
            self.state.graph_widget.reset_camera();
        }

        // =================================================================
        // Process Extended Esper 3D/Multi-dimensional Navigation Commands
        // =================================================================

        // Scale Navigation (astronomical metaphor)
        if self.state.take_pending_scale_universe() {
            web_sys::console::log_1(&"update: scale universe (full book view)".into());
            // TODO: Implement universe/full book view - zoom out to show all CBUs
            self.state.graph_widget.zoom_fit();
        }

        if let Some(segment) = self.state.take_pending_scale_galaxy() {
            web_sys::console::log_1(&format!("update: scale galaxy segment={:?}", segment).into());
            // TODO: Implement galaxy/segment view
        }

        if let Some(cbu_id) = self.state.take_pending_scale_system() {
            web_sys::console::log_1(&format!("update: scale system cbu_id={:?}", cbu_id).into());
            // TODO: Focus on specific CBU system
            if let Some(id) = cbu_id {
                self.state.graph_widget.focus_entity(&id);
            }
        }

        if let Some(entity_id) = self.state.take_pending_scale_planet() {
            web_sys::console::log_1(
                &format!("update: scale planet entity_id={:?}", entity_id).into(),
            );
            // Focus on specific entity
            if let Some(id) = entity_id {
                self.state.graph_widget.focus_entity(&id);
                self.state.selected_entity_id = Some(id);
            }
        }

        if self.state.take_pending_scale_surface() {
            web_sys::console::log_1(&"update: scale surface (attribute level)".into());
            // TODO: Zoom in to show entity attributes
            self.state.graph_widget.zoom_in(Some(2.0));
        }

        if self.state.take_pending_scale_core() {
            web_sys::console::log_1(&"update: scale core (raw data level)".into());
            // TODO: Show raw data/JSON view
            self.state.graph_widget.zoom_in(Some(3.0));
        }

        // Depth Navigation (Z-axis through entity structures)
        if self.state.take_pending_drill_through() {
            web_sys::console::log_1(&"update: drill through".into());
            // TODO: Drill through to subsidiary/ownership structure
        }

        if self.state.take_pending_surface_return() {
            web_sys::console::log_1(&"update: surface return".into());
            // TODO: Return to top-level view
            self.state.graph_widget.zoom_fit();
        }

        if self.state.take_pending_xray() {
            web_sys::console::log_1(&"update: x-ray mode".into());
            // TODO: Toggle x-ray/transparency mode to see through layers
        }

        if self.state.take_pending_peel() {
            web_sys::console::log_1(&"update: peel layer".into());
            // TODO: Remove outermost layer to reveal next
        }

        if self.state.take_pending_cross_section() {
            web_sys::console::log_1(&"update: cross section".into());
            // TODO: Show cross-section view of entity structure
        }

        if self.state.take_pending_depth_indicator() {
            web_sys::console::log_1(&"update: depth indicator".into());
            // TODO: Toggle depth indicator overlay
        }

        // Orbital Navigation
        if let Some(entity_id) = self.state.take_pending_orbit() {
            web_sys::console::log_1(&format!("update: orbit entity_id={:?}", entity_id).into());
            // TODO: Start orbiting around entity
            if let Some(id) = entity_id {
                self.state.graph_widget.focus_entity(&id);
            }
        }

        if let Some(layer) = self.state.take_pending_rotate_layer() {
            web_sys::console::log_1(&format!("update: rotate layer={}", layer).into());
            // TODO: Rotate specific layer (ownership, services, etc.)
        }

        if self.state.take_pending_flip() {
            web_sys::console::log_1(&"update: flip view".into());
            // TODO: Flip perspective (top-down vs bottom-up)
        }

        if let Some(dimension) = self.state.take_pending_tilt() {
            web_sys::console::log_1(&format!("update: tilt dimension={}", dimension).into());
            // TODO: Tilt to reveal specific dimension
        }

        // Temporal Navigation
        if let Some(target_date) = self.state.take_pending_time_rewind() {
            web_sys::console::log_1(&format!("update: time rewind to={:?}", target_date).into());
            // TODO: Rewind to historical snapshot
        }

        if let Some((from_date, to_date)) = self.state.take_pending_time_play() {
            web_sys::console::log_1(
                &format!("update: time play from={:?} to={:?}", from_date, to_date).into(),
            );
            // TODO: Animate changes over time period
        }

        if self.state.take_pending_time_freeze() {
            web_sys::console::log_1(&"update: time freeze".into());
            // TODO: Pause temporal animation
        }

        if let Some((date1, date2)) = self.state.take_pending_time_slice() {
            web_sys::console::log_1(
                &format!("update: time slice dates={:?},{:?}", date1, date2).into(),
            );
            // TODO: Show diff between two points in time
        }

        if let Some(entity_id) = self.state.take_pending_time_trail() {
            web_sys::console::log_1(
                &format!("update: time trail entity_id={:?}", entity_id).into(),
            );
            // TODO: Show entity's history trail/timeline
        }

        // Investigation Patterns
        if let Some(from_entity) = self.state.take_pending_follow_money() {
            web_sys::console::log_1(
                &format!("update: follow the money from={:?}", from_entity).into(),
            );
            // TODO: Trace financial flows from entity
        }

        if let Some(entity_id) = self.state.take_pending_who_controls() {
            web_sys::console::log_1(
                &format!("update: who controls entity_id={:?}", entity_id).into(),
            );
            // TODO: Highlight control relationships leading to entity
        }

        if let Some(aspect) = self.state.take_pending_illuminate() {
            web_sys::console::log_1(&format!("update: illuminate aspect={}", aspect).into());
            // TODO: Highlight specific aspect (risks, documents, screenings, etc.)
        }

        if self.state.take_pending_shadow() {
            web_sys::console::log_1(&"update: shadow mode".into());
            // TODO: Dim everything except high-risk/flagged items
        }

        if self.state.take_pending_red_flag_scan() {
            web_sys::console::log_1(&"update: red flag scan".into());
            // TODO: Highlight all red flags and risk indicators
        }

        if self.state.take_pending_black_hole() {
            web_sys::console::log_1(&"update: black hole (find gaps)".into());
            // TODO: Highlight missing data, incomplete chains
        }

        // Context Intentions
        if let Some(context) = self.state.take_pending_context() {
            web_sys::console::log_1(&format!("update: set context={}", context).into());
            // TODO: Adjust UI mode based on context
            // - review: read-only, focus on summary
            // - investigation: forensic tools, full detail
            // - onboarding: workflow-driven, progress focus
            // - monitoring: alerts, changes, flags
            // - remediation: issues, resolutions, actions
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

    /// Handle trading matrix browser actions
    fn handle_trading_matrix_action(&mut self, action: TradingMatrixPanelAction) {
        match action {
            TradingMatrixPanelAction::None => {}
            TradingMatrixPanelAction::ToggleExpand { node_key } => {
                // Parse the node key back to a TradingMatrixNodeId
                let path: Vec<String> = if node_key.is_empty() {
                    Vec::new()
                } else {
                    node_key.split('/').map(|s| s.to_string()).collect()
                };
                let node_id = ob_poc_graph::TradingMatrixNodeId { path };
                self.state.trading_matrix_state.toggle(&node_id);
            }
            TradingMatrixPanelAction::SelectNode { node_key } => {
                web_sys::console::log_1(&format!("TradingMatrix: Select node {}", node_key).into());
                let path: Vec<String> = if node_key.is_empty() {
                    Vec::new()
                } else {
                    node_key.split('/').map(|s| s.to_string()).collect()
                };
                let node_id = ob_poc_graph::TradingMatrixNodeId { path };
                self.state.trading_matrix_state.select(Some(&node_id));
            }
            TradingMatrixPanelAction::ClearSelection => {
                self.state.trading_matrix_state.select(None);
            }
            TradingMatrixPanelAction::NavigateToEntity { entity_id } => {
                web_sys::console::log_1(
                    &format!("TradingMatrix: Navigate to entity {}", entity_id).into(),
                );
                self.state.selected_entity_id = Some(entity_id.clone());
                self.state.graph_widget.focus_entity(&entity_id);
            }
            TradingMatrixPanelAction::OpenSsiDetail { ssi_id } => {
                web_sys::console::log_1(
                    &format!("TradingMatrix: Open SSI detail {}", ssi_id).into(),
                );
                // TODO: Navigate to SSI detail view
            }
            TradingMatrixPanelAction::OpenIsdaDetail { isda_id } => {
                web_sys::console::log_1(
                    &format!("TradingMatrix: Open ISDA detail {}", isda_id).into(),
                );
                // TODO: Navigate to ISDA detail view
            }
            TradingMatrixPanelAction::ExpandAll => {
                if let Some(ref matrix) = self.state.trading_matrix {
                    // Expand all nodes by traversing the tree
                    fn expand_all_nodes(
                        state: &mut ob_poc_graph::TradingMatrixState,
                        node: &ob_poc_graph::TradingMatrixNode,
                    ) {
                        state.expand(&node.id);
                        for child in &node.children {
                            expand_all_nodes(state, child);
                        }
                    }
                    expand_all_nodes(&mut self.state.trading_matrix_state, &matrix.root);
                }
            }
            TradingMatrixPanelAction::CollapseAll => {
                self.state.trading_matrix_state.collapse_all();
            }
            TradingMatrixPanelAction::LoadChildren { node_key } => {
                web_sys::console::log_1(
                    &format!("TradingMatrix: Load children for {}", node_key).into(),
                );
                // TODO: Lazy loading of children
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

        // Top: Graph (full width, 50% height) with taxonomy/trading matrix browser overlay
        // Use Trading Matrix browser when in Trading view mode, Taxonomy browser otherwise
        let is_trading_mode = matches!(self.state.view_mode, ViewMode::Trading);

        let (taxonomy_action, trading_matrix_action) = ui
            .allocate_ui(egui::vec2(available.x, top_height), |ui| {
                egui::Frame::default()
                    .inner_margin(0.0)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
                        // Horizontal split: browser (left) + graph (right)
                        ui.horizontal(|ui| {
                            let browser_width = if is_trading_mode { 240.0 } else { 180.0 };

                            let (tax_action, matrix_action) = ui
                                .allocate_ui(
                                    egui::vec2(browser_width, ui.available_height()),
                                    |ui| {
                                        if is_trading_mode {
                                            // Trading Matrix browser for Trading view
                                            let action = trading_matrix_panel(
                                                ui,
                                                &self.state,
                                                ui.available_height(),
                                            );
                                            (TaxonomyPanelAction::None, action)
                                        } else {
                                            // Taxonomy browser for other views
                                            let action = taxonomy_panel(
                                                ui,
                                                &self.state,
                                                ui.available_height(),
                                            );
                                            (action, TradingMatrixPanelAction::None)
                                        }
                                    },
                                )
                                .inner;

                            // Graph takes remaining space
                            ui.vertical(|ui| {
                                self.state.graph_widget.ui(ui);
                            });

                            (tax_action, matrix_action)
                        })
                        .inner
                    })
                    .inner
            })
            .inner;

        // Handle actions AFTER rendering (Rule 2)
        self.handle_taxonomy_action(taxonomy_action);
        self.handle_trading_matrix_action(trading_matrix_action);

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
            // Also trigger trading matrix refetch when switching to Trading mode
            if matches!(mode, ViewMode::Trading) {
                state.needs_trading_matrix_refetch = true;
            }
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
                                    // Esper-style navigation commands
                                    ob_poc_types::AgentCommand::ZoomIn { factor } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ZoomIn factor={:?}", factor).into(),
                                        );
                                        state.pending_zoom_in = Some(factor.unwrap_or(1.3));
                                    }
                                    ob_poc_types::AgentCommand::ZoomOut { factor } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ZoomOut factor={:?}", factor).into(),
                                        );
                                        state.pending_zoom_out = Some(factor.unwrap_or(1.3));
                                    }
                                    ob_poc_types::AgentCommand::ZoomFit => {
                                        web_sys::console::log_1(&"Command: ZoomFit".into());
                                        state.pending_zoom_fit = true;
                                    }
                                    ob_poc_types::AgentCommand::ZoomTo { level } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ZoomTo level={}", level).into(),
                                        );
                                        state.pending_zoom_to = Some(*level);
                                    }
                                    ob_poc_types::AgentCommand::Pan { direction, amount } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: Pan direction={:?} amount={:?}",
                                                direction, amount
                                            )
                                            .into(),
                                        );
                                        state.pending_pan = Some((direction.clone(), *amount));
                                    }
                                    ob_poc_types::AgentCommand::Center => {
                                        web_sys::console::log_1(&"Command: Center".into());
                                        state.pending_center = true;
                                    }
                                    ob_poc_types::AgentCommand::Stop => {
                                        web_sys::console::log_1(&"Command: Stop".into());
                                        state.pending_stop = true;
                                    }
                                    ob_poc_types::AgentCommand::FocusEntity { entity_id } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: FocusEntity entity_id={}",
                                                entity_id
                                            )
                                            .into(),
                                        );
                                        state.pending_focus_entity = Some(entity_id.clone());
                                    }
                                    ob_poc_types::AgentCommand::ResetLayout => {
                                        web_sys::console::log_1(&"Command: ResetLayout".into());
                                        state.pending_reset_layout = true;
                                    }

                                    // =========================================================
                                    // Extended Esper 3D/Multi-dimensional Navigation Commands
                                    // =========================================================

                                    // Scale Navigation (astronomical metaphor)
                                    ob_poc_types::AgentCommand::ScaleUniverse => {
                                        web_sys::console::log_1(&"Command: ScaleUniverse".into());
                                        state.pending_scale_universe = true;
                                    }
                                    ob_poc_types::AgentCommand::ScaleGalaxy { segment } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ScaleGalaxy segment={:?}", segment)
                                                .into(),
                                        );
                                        state.pending_scale_galaxy = Some(segment.clone());
                                    }
                                    ob_poc_types::AgentCommand::ScaleSystem { cbu_id } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ScaleSystem cbu_id={:?}", cbu_id)
                                                .into(),
                                        );
                                        state.pending_scale_system = Some(cbu_id.clone());
                                    }
                                    ob_poc_types::AgentCommand::ScalePlanet { entity_id } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: ScalePlanet entity_id={:?}",
                                                entity_id
                                            )
                                            .into(),
                                        );
                                        state.pending_scale_planet = Some(entity_id.clone());
                                    }
                                    ob_poc_types::AgentCommand::ScaleSurface => {
                                        web_sys::console::log_1(&"Command: ScaleSurface".into());
                                        state.pending_scale_surface = true;
                                    }
                                    ob_poc_types::AgentCommand::ScaleCore => {
                                        web_sys::console::log_1(&"Command: ScaleCore".into());
                                        state.pending_scale_core = true;
                                    }

                                    // Depth Navigation (Z-axis through entity structures)
                                    ob_poc_types::AgentCommand::DrillThrough => {
                                        web_sys::console::log_1(&"Command: DrillThrough".into());
                                        state.pending_drill_through = true;
                                    }
                                    ob_poc_types::AgentCommand::SurfaceReturn => {
                                        web_sys::console::log_1(&"Command: SurfaceReturn".into());
                                        state.pending_surface_return = true;
                                    }
                                    ob_poc_types::AgentCommand::XRay => {
                                        web_sys::console::log_1(&"Command: XRay".into());
                                        state.pending_xray = true;
                                    }
                                    ob_poc_types::AgentCommand::Peel => {
                                        web_sys::console::log_1(&"Command: Peel".into());
                                        state.pending_peel = true;
                                    }
                                    ob_poc_types::AgentCommand::CrossSection => {
                                        web_sys::console::log_1(&"Command: CrossSection".into());
                                        state.pending_cross_section = true;
                                    }
                                    ob_poc_types::AgentCommand::DepthIndicator => {
                                        web_sys::console::log_1(&"Command: DepthIndicator".into());
                                        state.pending_depth_indicator = true;
                                    }

                                    // Orbital Navigation
                                    ob_poc_types::AgentCommand::Orbit { entity_id } => {
                                        web_sys::console::log_1(
                                            &format!("Command: Orbit entity_id={:?}", entity_id)
                                                .into(),
                                        );
                                        state.pending_orbit = Some(entity_id.clone());
                                    }
                                    ob_poc_types::AgentCommand::RotateLayer { layer } => {
                                        web_sys::console::log_1(
                                            &format!("Command: RotateLayer layer={}", layer).into(),
                                        );
                                        state.pending_rotate_layer = Some(layer.clone());
                                    }
                                    ob_poc_types::AgentCommand::Flip => {
                                        web_sys::console::log_1(&"Command: Flip".into());
                                        state.pending_flip = true;
                                    }
                                    ob_poc_types::AgentCommand::Tilt { dimension } => {
                                        web_sys::console::log_1(
                                            &format!("Command: Tilt dimension={}", dimension)
                                                .into(),
                                        );
                                        state.pending_tilt = Some(dimension.clone());
                                    }

                                    // Temporal Navigation
                                    ob_poc_types::AgentCommand::TimeRewind { target_date } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: TimeRewind target_date={:?}",
                                                target_date
                                            )
                                            .into(),
                                        );
                                        state.pending_time_rewind = Some(target_date.clone());
                                    }
                                    ob_poc_types::AgentCommand::TimePlay { from_date, to_date } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: TimePlay from={:?} to={:?}",
                                                from_date, to_date
                                            )
                                            .into(),
                                        );
                                        state.pending_time_play =
                                            Some((from_date.clone(), to_date.clone()));
                                    }
                                    ob_poc_types::AgentCommand::TimeFreeze => {
                                        web_sys::console::log_1(&"Command: TimeFreeze".into());
                                        state.pending_time_freeze = true;
                                    }
                                    ob_poc_types::AgentCommand::TimeSlice { date1, date2 } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: TimeSlice date1={:?} date2={:?}",
                                                date1, date2
                                            )
                                            .into(),
                                        );
                                        state.pending_time_slice =
                                            Some((date1.clone(), date2.clone()));
                                    }
                                    ob_poc_types::AgentCommand::TimeTrail { entity_id } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: TimeTrail entity_id={:?}",
                                                entity_id
                                            )
                                            .into(),
                                        );
                                        state.pending_time_trail = Some(entity_id.clone());
                                    }

                                    // Investigation Patterns
                                    ob_poc_types::AgentCommand::FollowTheMoney { from_entity } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: FollowTheMoney from={:?}",
                                                from_entity
                                            )
                                            .into(),
                                        );
                                        state.pending_follow_money = Some(from_entity.clone());
                                    }
                                    ob_poc_types::AgentCommand::WhoControls { entity_id } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: WhoControls entity_id={:?}",
                                                entity_id
                                            )
                                            .into(),
                                        );
                                        state.pending_who_controls = Some(entity_id.clone());
                                    }
                                    ob_poc_types::AgentCommand::Illuminate { aspect } => {
                                        web_sys::console::log_1(
                                            &format!("Command: Illuminate aspect={}", aspect)
                                                .into(),
                                        );
                                        state.pending_illuminate = Some(aspect.clone());
                                    }
                                    ob_poc_types::AgentCommand::Shadow => {
                                        web_sys::console::log_1(&"Command: Shadow".into());
                                        state.pending_shadow = true;
                                    }
                                    ob_poc_types::AgentCommand::RedFlagScan => {
                                        web_sys::console::log_1(&"Command: RedFlagScan".into());
                                        state.pending_red_flag_scan = true;
                                    }
                                    ob_poc_types::AgentCommand::BlackHole => {
                                        web_sys::console::log_1(&"Command: BlackHole".into());
                                        state.pending_black_hole = true;
                                    }

                                    // Context Intentions
                                    ob_poc_types::AgentCommand::ContextReview => {
                                        web_sys::console::log_1(&"Command: ContextReview".into());
                                        state.pending_context = Some("review".to_string());
                                    }
                                    ob_poc_types::AgentCommand::ContextInvestigation => {
                                        web_sys::console::log_1(
                                            &"Command: ContextInvestigation".into(),
                                        );
                                        state.pending_context = Some("investigation".to_string());
                                    }
                                    ob_poc_types::AgentCommand::ContextOnboarding => {
                                        web_sys::console::log_1(
                                            &"Command: ContextOnboarding".into(),
                                        );
                                        state.pending_context = Some("onboarding".to_string());
                                    }
                                    ob_poc_types::AgentCommand::ContextMonitoring => {
                                        web_sys::console::log_1(
                                            &"Command: ContextMonitoring".into(),
                                        );
                                        state.pending_context = Some("monitoring".to_string());
                                    }
                                    ob_poc_types::AgentCommand::ContextRemediation => {
                                        web_sys::console::log_1(
                                            &"Command: ContextRemediation".into(),
                                        );
                                        state.pending_context = Some("remediation".to_string());
                                    }

                                    // Unhandled commands - log but don't fail
                                    _ => {
                                        web_sys::console::warn_1(
                                            &format!("Command not yet implemented: {:?}", cmd)
                                                .into(),
                                        );
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

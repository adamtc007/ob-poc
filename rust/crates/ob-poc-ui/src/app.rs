//! Main Application
//!
//! This module implements the eframe::App trait and coordinates:
//! 1. Processing async results at the start of each frame
//! 2. Rendering panels based on layout mode
//! 3. Handling widget responses (no callbacks, return values only)

use crate::api;
use crate::panels::{
    ast_panel, cbu_search_modal, chat_panel, container_browse_panel, context_panel,
    dsl_editor_panel, entity_detail_panel, repl_panel, results_panel, session_panel,
    taxonomy_panel, toolbar, trading_matrix_panel, CbuSearchAction, CbuSearchData,
    ContainerBrowseAction, ContainerBrowseData, ContextPanelAction, DslEditorAction,
    TaxonomyPanelAction, ToolbarAction, ToolbarData, TradingMatrixPanelAction,
};
use crate::state::{AppState, AsyncState, CbuSearchUi, LayoutMode, PanelState, TextBuffers};
use ob_poc_graph::{CbuGraphWidget, TradingMatrixNodeIdExt, ViewMode};
use ob_poc_types::galaxy::{NavigationAction, ViewLevel};
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
            taxonomy_breadcrumbs: Vec::new(),
            type_filter: None,
            trading_matrix_state: ob_poc_graph::TradingMatrixState::new(),
            selected_matrix_node: None,
            // Galaxy navigation state
            galaxy_view: ob_poc_graph::GalaxyView::new(),
            navigation_scope: ob_poc_types::galaxy::NavigationScope::default(),
            view_level: ob_poc_types::galaxy::ViewLevel::default(),
            navigation_stack: Vec::new(),
            universe_graph: None,
            last_known_version: None,
            last_version_check: None,
            navigation_log: Vec::new(),
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
            dispatch_command, AgentPromptConduit, CommandResult, CommandSource,
            InvestigationContext,
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

            let result = dispatch_command(source.clone(), &context);

            web_sys::console::log_1(
                &format!(
                    "Voice command: '{}' -> {:?} (confidence: {:.2})",
                    cmd.transcript, result, cmd.confidence
                )
                .into(),
            );

            // Route based on command result
            match result {
                CommandResult::Navigation(verb) => self.execute_navigation_verb(verb, Some(source)),
                CommandResult::Agent(prompt) => self.send_to_agent(prompt),
                CommandResult::None => {}
            }
        }
    }

    /// Execute a navigation verb from any command source (voice, chat, egui).
    /// These are LOCAL UI commands - no server round-trip needed.
    ///
    /// If `source` is provided, the command is logged to the navigation audit trail.
    #[cfg(target_arch = "wasm32")]
    fn execute_navigation_verb(
        &mut self,
        verb: crate::command::NavigationVerb,
        source: Option<crate::command::CommandSource>,
    ) {
        use crate::command::NavigationVerb;
        use crate::state::NavigationSource;
        use ob_poc_types::PanDirection;

        // Log to navigation audit trail (skip None commands)
        if !matches!(verb, NavigationVerb::None) {
            let dsl = verb.to_dsl_string();
            let nav_source = match source {
                Some(crate::command::CommandSource::Voice {
                    transcript,
                    confidence,
                    ..
                }) => NavigationSource::Voice {
                    transcript,
                    confidence,
                },
                Some(crate::command::CommandSource::Chat { .. }) => NavigationSource::Widget,
                Some(crate::command::CommandSource::Egui { .. }) => NavigationSource::Widget,
                None => NavigationSource::Programmatic,
            };
            self.state.log_navigation(dsl, nav_source);
        }

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
                // Join multiple type codes into comma-separated string
                self.state.type_filter = Some(type_codes.join(","));
            }
            NavigationVerb::HighlightType { type_code } => {
                self.state.graph_widget.highlight_type(&type_code);
            }
            NavigationVerb::ClearFilter => {
                self.state.type_filter = None;
                self.state.graph_widget.clear_highlight();
            }

            // Scale navigation (astronomical metaphor) - wired to GalaxyView
            NavigationVerb::ScaleUniverse => {
                // Return to universe view in galaxy navigation
                self.state.galaxy_view.return_to_universe();
                self.state.navigation_scope = ob_poc_types::galaxy::NavigationScope::Universe;
                self.state.view_level = ob_poc_types::galaxy::ViewLevel::Universe;
                self.state.fetch_universe_graph();
            }
            NavigationVerb::ScaleBook { client_name } => {
                // View all CBUs for a commercial client (the "galaxy" / book view)
                // Maps to `view.book :client <id>` DSL verb
                // Client name will be resolved to entity_id via EntityGateway
                self.state.navigation_scope = ob_poc_types::galaxy::NavigationScope::Book {
                    apex_entity_id: client_name.clone(), // Will be resolved
                    apex_name: client_name.clone(),
                };
                self.state.view_level = ob_poc_types::galaxy::ViewLevel::Cluster;
                // Fetch the book/client's CBUs via API
                self.state.fetch_client_book(&client_name);
            }
            NavigationVerb::ScaleGalaxy { segment } => {
                // Drill into a cluster/segment in galaxy view
                if let Some(cluster_id) = segment {
                    self.state.galaxy_view.drill_into_cluster(&cluster_id);
                    self.state.navigation_scope = ob_poc_types::galaxy::NavigationScope::Cluster {
                        cluster_id: cluster_id.clone(),
                        cluster_type: ob_poc_types::galaxy::ClusterType::default(),
                    };
                    self.state.view_level = ob_poc_types::galaxy::ViewLevel::Cluster;
                }
            }
            NavigationVerb::ScaleSystem { cbu_id } => {
                // Select a specific CBU (solar system level)
                if let Some(ref id) = cbu_id {
                    if let Ok(uuid) = Uuid::parse_str(id) {
                        self.state.select_cbu(uuid, id);
                        self.state.view_level = ob_poc_types::galaxy::ViewLevel::System;
                    }
                }
            }
            NavigationVerb::ScalePlanet { entity_id } => {
                // Focus on a specific entity (planet level)
                if let Some(ref id) = entity_id {
                    // Use galaxy focus stack for inline expansion
                    self.state.galaxy_view.push_focus(
                        id.clone(),
                        "entity".to_string(),
                        id.clone(), // Use ID as label for now
                    );
                    self.state.view_level = ob_poc_types::galaxy::ViewLevel::Planet;
                }
            }
            NavigationVerb::ScaleSurface => {
                // Surface level - deep focus with high zoom
                self.state.graph_widget.set_zoom(1.5);
                self.state.view_level = ob_poc_types::galaxy::ViewLevel::Surface;
            }
            NavigationVerb::ScaleCore => {
                // Core level - deepest zoom into entity details
                self.state.graph_widget.set_zoom(3.0);
                self.state.view_level = ob_poc_types::galaxy::ViewLevel::Core;
            }

            // Depth navigation - uses galaxy focus stack
            NavigationVerb::DrillThrough => {
                // Drill deeper into current focus
                if let Some(focus) = self.state.galaxy_view.current_focus() {
                    // Trigger expansion of current focus
                    let node_id = focus.node_id.clone();
                    self.state.galaxy_view.expand_node(
                        node_id,
                        ob_poc_types::galaxy::ExpansionType::Children,
                        vec![], // Children will be fetched
                    );
                } else {
                    self.state.graph_widget.zoom_in(Some(1.5));
                }
            }
            NavigationVerb::SurfaceReturn => {
                // Pop focus stack or return to universe
                if self.state.galaxy_view.pop_focus().is_none() {
                    self.state.galaxy_view.return_to_universe();
                    self.state.navigation_scope = ob_poc_types::galaxy::NavigationScope::Universe;
                    self.state.view_level = ob_poc_types::galaxy::ViewLevel::Universe;
                }
            }
            NavigationVerb::Xray => {
                // Toggle X-ray transparency mode (local, no server)
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_xray();
                let enabled = self.state.graph_widget.esper_render_state().xray_enabled;
                web_sys::console::log_1(&format!("X-ray mode: {}", enabled).into());
            }
            NavigationVerb::Peel => {
                // Peel layer - incrementally hide outer layers (local, no server)
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_peel();
                let depth = self.state.graph_widget.esper_render_state().peel_depth;
                let enabled = self.state.graph_widget.esper_render_state().peel_enabled;
                web_sys::console::log_1(
                    &format!("Peel: enabled={}, depth={}", enabled, depth).into(),
                );
            }
            NavigationVerb::CrossSection => {
                // Toggle cross section view (local, no server)
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_cross_section();
                let enabled = self
                    .state
                    .graph_widget
                    .esper_render_state()
                    .cross_section_enabled;
                web_sys::console::log_1(&format!("Cross section: {}", enabled).into());
            }
            NavigationVerb::DepthIndicator => {
                // Toggle depth indicator (local, no server)
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_depth_indicator();
                let enabled = self
                    .state
                    .graph_widget
                    .esper_render_state()
                    .depth_indicator_enabled;
                web_sys::console::log_1(&format!("Depth indicator: {}", enabled).into());
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

            // Investigation patterns - use galaxy focus stack for deep navigation
            NavigationVerb::FollowRabbit { from_entity } => {
                // "Follow the rabbit" - trace ownership chain to terminus
                if let Some(ref id) = from_entity {
                    // Push focus onto entity
                    self.state.galaxy_view.push_focus(
                        id.clone(),
                        "ownership_chain".to_string(),
                        format!("Following ownership from {}", id),
                    );
                    // Request ownership chain expansion
                    self.state.galaxy_view.expand_node(
                        id.clone(),
                        ob_poc_types::galaxy::ExpansionType::Ownership,
                        vec![],
                    );
                    self.state.view_level = ob_poc_types::galaxy::ViewLevel::Planet;
                }
            }
            NavigationVerb::DiveInto { entity_id } => {
                // Dive into entity - push focus and expand children
                if let Some(ref id) = entity_id {
                    self.state.galaxy_view.push_focus(
                        id.clone(),
                        "entity".to_string(),
                        format!("Diving into {}", id),
                    );
                    self.state.galaxy_view.expand_node(
                        id.clone(),
                        ob_poc_types::galaxy::ExpansionType::Children,
                        vec![],
                    );
                    self.state.view_level = ob_poc_types::galaxy::ViewLevel::Surface;
                }
            }
            NavigationVerb::WhoControls { entity_id } => {
                // "Who controls this?" - trace control chain
                if let Some(ref id) = entity_id {
                    self.state.galaxy_view.push_focus(
                        id.clone(),
                        "control_chain".to_string(),
                        format!("Control chain for {}", id),
                    );
                    // Request control relationships
                    self.state.galaxy_view.expand_node(
                        id.clone(),
                        ob_poc_types::galaxy::ExpansionType::Control,
                        vec![],
                    );
                }
            }
            NavigationVerb::Illuminate { aspect } => {
                // Illuminate specific aspect (local, no server)
                // Map aspect string to IlluminateAspect enum
                let illuminate_aspect = match aspect.to_lowercase().as_str() {
                    "ownership" => ob_poc_graph::IlluminateAspect::Ownership,
                    "control" => ob_poc_graph::IlluminateAspect::Control,
                    "risk" => ob_poc_graph::IlluminateAspect::Risk,
                    "documents" | "docs" => ob_poc_graph::IlluminateAspect::Documents,
                    "kyc" | "kyc_status" => ob_poc_graph::IlluminateAspect::KycStatus,
                    _ => ob_poc_graph::IlluminateAspect::Custom,
                };
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_illuminate(illuminate_aspect);
                let enabled = self
                    .state
                    .graph_widget
                    .esper_render_state()
                    .illuminate_enabled;
                web_sys::console::log_1(&format!("Illuminate {}: {}", aspect, enabled).into());
            }
            NavigationVerb::Shadow => {
                // Toggle shadow mode - dim non-focused entities (local, no server)
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_shadow();
                let enabled = self.state.graph_widget.esper_render_state().shadow_enabled;
                web_sys::console::log_1(&format!("Shadow mode: {}", enabled).into());
            }
            NavigationVerb::RedFlagScan => {
                // Toggle red flag scan - highlight entities with anomalies (local, no server)
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_red_flag_scan(Some(ob_poc_graph::RedFlagCategory::All));
                let enabled = self
                    .state
                    .graph_widget
                    .esper_render_state()
                    .red_flag_scan_enabled;
                web_sys::console::log_1(&format!("Red flag scan: {}", enabled).into());
                // Also set galaxy agent mode for compatibility
                if enabled && self.state.galaxy_view.has_anomalies() {
                    let count = self.state.galaxy_view.total_anomaly_count();
                    web_sys::console::log_1(&format!("Found {} anomalies", count).into());
                    self.state
                        .galaxy_view
                        .set_agent_mode(ob_poc_graph::AgentMode::Scanning);
                }
            }
            NavigationVerb::BlackHole => {
                // Toggle black hole mode - highlight entities with missing data (local, no server)
                self.state
                    .graph_widget
                    .esper_render_state_mut()
                    .toggle_black_hole(Some(ob_poc_graph::GapType::All));
                let enabled = self
                    .state
                    .graph_widget
                    .esper_render_state()
                    .black_hole_enabled;
                web_sys::console::log_1(&format!("Black hole scan: {}", enabled).into());
                if enabled {
                    self.state
                        .galaxy_view
                        .set_agent_mode(ob_poc_graph::AgentMode::Scanning);
                }
            }

            // Context
            NavigationVerb::SetContext { context } => {
                // Switch investigation context
                web_sys::console::log_1(&format!("Switching context to: {}", context).into());
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

        // Ensure we have a session
        let Some(_session_id) = self.state.session_id else {
            web_sys::console::warn_1(&"[AgentConduit] No session, cannot send".into());
            return;
        };

        // Use the existing chat infrastructure
        self.state.buffers.chat_input = message;
        self.state.send_chat_message();
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

        // Central session refetch - triggered by: version change detected (MCP/REPL)
        if self.state.take_pending_session_refetch() {
            if let Some(session_id) = self.state.session_id {
                web_sys::console::log_1(
                    &format!(
                        "update: session refetch triggered by version change for session_id={}",
                        session_id
                    )
                    .into(),
                );
                self.state.refetch_session();
            }
        }

        // Central resolution check - triggered when session has DSL with potential unresolved refs
        // This enables automatic entity lookup popup for disambiguation
        if self.state.take_pending_resolution_check() {
            web_sys::console::log_1(&"update: triggering resolution check for DSL".into());
            self.state.start_resolution();
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
        // Process Hierarchy Navigation Commands
        // =================================================================

        // Handle expand node command - expands a node in the graph or taxonomy
        if let Some(node_key) = self.state.take_pending_expand_node() {
            web_sys::console::log_1(&format!("update: expand node {}", node_key).into());
            // Expand in taxonomy state (type browser)
            self.state.taxonomy_state.expand(&node_key);
            // Also expand in trading matrix (parse node key as TradingMatrixNodeId)
            if let Ok(node_id) =
                ob_poc_graph::graph::trading_matrix::TradingMatrixNodeId::parse(&node_key)
            {
                self.state.trading_matrix_state.expand(&node_id);
            }
        }

        // Handle collapse node command - collapses a node in the graph or taxonomy
        if let Some(node_key) = self.state.take_pending_collapse_node() {
            web_sys::console::log_1(&format!("update: collapse node {}", node_key).into());
            // Collapse in taxonomy state (type browser)
            self.state.taxonomy_state.collapse(&node_key);
            // Also collapse in trading matrix
            if let Ok(node_id) =
                ob_poc_graph::graph::trading_matrix::TradingMatrixNodeId::parse(&node_key)
            {
                self.state.trading_matrix_state.collapse(&node_id);
            }
        }

        // =================================================================
        // Process Export and Layout Commands
        // =================================================================

        // Handle export command
        if let Some(format) = self.state.take_pending_export() {
            web_sys::console::log_1(&format!("update: export format={}", format).into());
            // TODO: Implement export to PNG/SVG/PDF via canvas screenshot or SVG generation
            // For now, log that it's not yet implemented
            web_sys::console::warn_1(
                &format!(
                    "Export to {} not yet implemented - requires canvas API",
                    format
                )
                .into(),
            );
        }

        // Handle toggle orientation command - flip between VERTICAL and HORIZONTAL layout
        if self.state.take_pending_toggle_orientation() {
            web_sys::console::log_1(&"update: toggle orientation".into());
            // Toggle the graph layout orientation
            self.state.graph_widget.toggle_orientation();
        }

        // Handle search command - search for entities in the graph
        if let Some(query) = self.state.take_pending_search() {
            web_sys::console::log_1(&format!("update: search query={}", query).into());
            // Use the graph widget's search functionality or highlight matching entities
            self.state.graph_widget.search_entities(&query);
        }

        // Handle show help command - display help overlay
        if self.state.take_pending_show_help() {
            web_sys::console::log_1(&"update: show help".into());
            // TODO: Implement help overlay panel
            // For now, just log that help was requested
            web_sys::console::log_1(
                &"Help: Drag=Pan, Scroll=Zoom, Click=Focus, Esc=Clear, R=Fit, Tab=Next".into(),
            );
        }

        // =================================================================
        // Process Extended Esper 3D/Multi-dimensional Navigation Commands
        // =================================================================

        // Scale Navigation (astronomical metaphor) - execute via DSL session
        if self.state.take_pending_scale_universe() {
            web_sys::console::log_1(&"update: scale universe (full book view)".into());
            // Execute view.universe via session to show all CBUs
            if let Some(session_id) = self.state.session_id {
                self.state
                    .execute_dsl_with_content(session_id, "(view.universe)".to_string());
            } else {
                web_sys::console::warn_1(
                    &"scale universe: no session, falling back to zoom_fit".into(),
                );
                self.state.graph_widget.zoom_fit();
            }
        }

        if let Some(segment) = self.state.take_pending_scale_galaxy() {
            web_sys::console::log_1(&format!("update: scale galaxy segment={:?}", segment).into());
            // Execute view.book via session to show CBUs for a client/segment
            if let Some(session_id) = self.state.session_id {
                let dsl = if let Some(client_name) = segment {
                    // view.book :client "ClientName" - show CBUs for this commercial client
                    format!("(view.book :client \"{}\")", client_name)
                } else {
                    // No segment specified - show full universe
                    "(view.universe)".to_string()
                };
                self.state.execute_dsl_with_content(session_id, dsl);
            } else {
                web_sys::console::warn_1(&"scale galaxy: no session, cannot execute DSL".into());
            }
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
        // Process Taxonomy Navigation Commands (fractal drill-down via server)
        // =================================================================

        // Handle taxonomy zoom-in command
        if let Some(type_code) = self.state.take_pending_taxonomy_zoom_in() {
            web_sys::console::log_1(&format!("update: taxonomy zoom in to {}", type_code).into());
            if let Some(session_id) = self.state.session_id {
                self.state.taxonomy_zoom_in(session_id, type_code);
            }
        }

        // Handle taxonomy zoom-out command
        if self.state.take_pending_taxonomy_zoom_out() {
            web_sys::console::log_1(&"update: taxonomy zoom out".into());
            if let Some(session_id) = self.state.session_id {
                self.state.taxonomy_zoom_out(session_id);
            }
        }

        // Handle taxonomy back-to command
        if let Some(level_index) = self.state.take_pending_taxonomy_back_to() {
            web_sys::console::log_1(
                &format!("update: taxonomy back to level {}", level_index).into(),
            );
            if let Some(session_id) = self.state.session_id {
                self.state.taxonomy_back_to(session_id, level_index);
            }
        }

        // Handle taxonomy breadcrumbs refresh request
        if self.state.take_pending_taxonomy_breadcrumbs() {
            web_sys::console::log_1(&"update: taxonomy fetch breadcrumbs".into());
            if let Some(session_id) = self.state.session_id {
                self.state.fetch_taxonomy_breadcrumbs(session_id);
            }
        }

        // Handle taxonomy reset command
        if self.state.take_pending_taxonomy_reset() {
            web_sys::console::log_1(&"update: taxonomy reset".into());
            if let Some(session_id) = self.state.session_id {
                self.state.taxonomy_reset(session_id);
            }
        }

        // Handle taxonomy filter command
        if let Some(filter) = self.state.take_pending_taxonomy_filter() {
            web_sys::console::log_1(&format!("update: taxonomy filter: {}", filter).into());
            // TODO: Implement taxonomy filtering via API
            web_sys::console::warn_1(&"Taxonomy filtering not yet implemented".into());
        }

        // Handle taxonomy clear filter command
        if self.state.take_pending_taxonomy_clear_filter() {
            web_sys::console::log_1(&"update: taxonomy clear filter".into());
            // TODO: Implement taxonomy filter clearing via API
            web_sys::console::warn_1(&"Taxonomy clear filter not yet implemented".into());
        }

        // =================================================================
        // Process Galaxy Navigation Commands (universe/cluster drill-down)
        // =================================================================

        // Handle universe graph result (from GET /api/universe)
        if let Some(result) = self.state.take_pending_universe_graph() {
            match result {
                Ok(universe) => {
                    web_sys::console::log_1(
                        &format!(
                            "update: received universe graph with {} clusters",
                            universe.clusters.len()
                        )
                        .into(),
                    );
                    // Update galaxy view with new data
                    self.state.galaxy_view.set_universe_data(&universe);
                    self.state.universe_graph = Some(universe);
                }
                Err(e) => {
                    web_sys::console::error_1(
                        &format!("update: failed to fetch universe: {}", e).into(),
                    );
                }
            }
        }

        // Handle universe refetch flag
        if self.state.take_pending_universe_refetch() {
            web_sys::console::log_1(&"update: fetching universe graph".into());
            self.state.fetch_universe_graph();
        }

        // Handle drill into cluster command
        if let Some(cluster_id) = self.state.take_pending_drill_cluster() {
            web_sys::console::log_1(
                &format!("update: drilling into cluster {}", cluster_id).into(),
            );
            // Push current scope to navigation stack
            self.state
                .navigation_stack
                .push(self.state.navigation_scope.clone());
            // Update navigation scope to cluster
            self.state.navigation_scope = ob_poc_types::galaxy::NavigationScope::Cluster {
                cluster_id: cluster_id.clone(),
                cluster_type: ob_poc_types::galaxy::ClusterType::default(),
            };
            self.state.view_level = ob_poc_types::galaxy::ViewLevel::Cluster;
            // Trigger cluster fetch via galaxy view
            self.state.galaxy_view.drill_into_cluster(&cluster_id);
        }

        // Handle drill into CBU command (from galaxy view)
        if let Some(cbu_id) = self.state.take_pending_drill_cbu() {
            web_sys::console::log_1(
                &format!("update: drilling into CBU {} from galaxy", cbu_id).into(),
            );
            // Push current scope to navigation stack
            self.state
                .navigation_stack
                .push(self.state.navigation_scope.clone());
            // Update navigation scope to CBU (System level in astronomical metaphor)
            self.state.navigation_scope = ob_poc_types::galaxy::NavigationScope::Cbu {
                cbu_id: cbu_id.clone(),
                cbu_name: String::new(), // Will be populated when CBU data is fetched
            };
            self.state.view_level = ob_poc_types::galaxy::ViewLevel::System;
            // Switch to CBU graph view by selecting the CBU
            if let Ok(uuid) = uuid::Uuid::parse_str(&cbu_id) {
                self.state.select_cbu(uuid, ""); // Name populated on fetch
            }
        }

        // Handle drill up command
        if self.state.take_pending_drill_up() {
            web_sys::console::log_1(&"update: drilling up one level".into());
            // Pop from navigation stack
            if let Some(prev_scope) = self.state.navigation_stack.pop() {
                self.state.navigation_scope = prev_scope.clone();
                // Update view level based on scope (using astronomical metaphor)
                self.state.view_level = match &prev_scope {
                    ob_poc_types::galaxy::NavigationScope::Universe => {
                        ob_poc_types::galaxy::ViewLevel::Universe
                    }
                    ob_poc_types::galaxy::NavigationScope::Book { .. } => {
                        ob_poc_types::galaxy::ViewLevel::Cluster
                    }
                    ob_poc_types::galaxy::NavigationScope::Cluster { .. } => {
                        ob_poc_types::galaxy::ViewLevel::Cluster
                    }
                    ob_poc_types::galaxy::NavigationScope::Cbu { .. } => {
                        ob_poc_types::galaxy::ViewLevel::System
                    }
                    ob_poc_types::galaxy::NavigationScope::Entity { .. } => {
                        ob_poc_types::galaxy::ViewLevel::Planet
                    }
                    ob_poc_types::galaxy::NavigationScope::Deep { .. } => {
                        ob_poc_types::galaxy::ViewLevel::Core
                    }
                };
                // If back to universe, trigger galaxy view refresh
                if matches!(prev_scope, ob_poc_types::galaxy::NavigationScope::Universe) {
                    self.state.galaxy_view.return_to_universe();
                }
            }
        }

        // Handle go to universe command
        if self.state.take_pending_go_to_universe() {
            web_sys::console::log_1(&"update: jumping to universe view".into());
            // Clear navigation stack
            self.state.navigation_stack.clear();
            // Reset to universe scope
            self.state.navigation_scope = ob_poc_types::galaxy::NavigationScope::Universe;
            self.state.view_level = ob_poc_types::galaxy::ViewLevel::Universe;
            // Reset galaxy view
            self.state.galaxy_view.return_to_universe();
            // Fetch fresh universe data
            self.state.fetch_universe_graph();
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
                        ui.label(format!(
                            "can_execute: {}",
                            session.dsl.as_ref().map(|d| d.can_execute).unwrap_or(false)
                        ));
                        ui.label(format!(
                            "dsl source: {:?}",
                            session
                                .dsl
                                .as_ref()
                                .and_then(|d| d.source.as_ref())
                                .map(|s| { s.chars().take(50).collect::<String>() })
                        ));
                        ui.label(format!(
                            "dsl source len: {}",
                            session
                                .dsl
                                .as_ref()
                                .and_then(|d| d.source.as_ref())
                                .map(|s| s.len())
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
                view_level: self.state.view_level,
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

        // For Simplified layout: Chat on left side, skip context panel
        // For other layouts: Context panel on left side
        if self.state.panels.layout == LayoutMode::Simplified {
            // Chat panel (left side) for simplified layout
            egui::SidePanel::left("chat_side_panel")
                .default_width(280.0)
                .min_width(200.0)
                .max_width(400.0)
                .resizable(true)
                .show(ctx, |ui| {
                    chat_panel(ui, &mut self.state);
                });

            // Session panel (bottom) for simplified layout
            egui::TopBottomPanel::bottom("session_bottom_panel")
                .default_height(120.0)
                .min_height(80.0)
                .max_height(300.0)
                .resizable(true)
                .show(ctx, |ui| {
                    session_panel(ui, &mut self.state);
                });

            // Main content area - just the graph viewport
            egui::CentralPanel::default().show(ctx, |ui| {
                if self.state.view_level == ViewLevel::Universe {
                    self.render_galaxy_view(ui);
                } else {
                    self.state.graph_widget.ui(ui);
                }
            });
        } else {
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
                LayoutMode::Simplified => self.render_simplified(ui), // Won't reach here
                LayoutMode::FourPanel => self.render_four_panel(ui),
                LayoutMode::EditorFocus => self.render_editor_focus(ui),
                LayoutMode::GraphFocus => self.render_graph_focus(ui),
                LayoutMode::GraphFullSize => self.render_graph_full_size(ui),
            });
        }

        // =================================================================
        // STEP 6: Session watch via long-polling (detect MCP/REPL changes)
        // =================================================================
        // Start long-poll watch if we have a session and aren't already watching
        if let Some(session_id) = self.state.session_id {
            // Check if we have a known version (initial session load complete)
            // and are not currently watching
            let should_start_watch = {
                let async_state = self.state.async_state.lock().unwrap();
                self.state.last_known_version.is_some() && !async_state.watching_session
            };

            if should_start_watch {
                self.state.start_session_watch(session_id);
            }
        }

        // =================================================================
        // STEP 7: Request repaint if async operations in progress
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

        if let Some(level) = action.change_view_level {
            self.state.set_view_level(level);
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
            TaxonomyPanelAction::ZoomIn { type_code } => {
                web_sys::console::log_1(&format!("Taxonomy: Zoom into type {}", type_code).into());
                // Set pending zoom-in command (will be processed by server)
                if let Ok(mut async_state) = self.state.async_state.lock() {
                    async_state.pending_taxonomy_zoom_in = Some(type_code);
                }
            }
            TaxonomyPanelAction::ZoomOut => {
                web_sys::console::log_1(&"Taxonomy: Zoom out".into());
                // Set pending zoom-out command
                if let Ok(mut async_state) = self.state.async_state.lock() {
                    async_state.pending_taxonomy_zoom_out = true;
                }
            }
            TaxonomyPanelAction::BackTo { level_index } => {
                web_sys::console::log_1(&format!("Taxonomy: Back to level {}", level_index).into());
                // Set pending back-to command
                if let Ok(mut async_state) = self.state.async_state.lock() {
                    async_state.pending_taxonomy_back_to = Some(level_index);
                }
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
                let node_id = ob_poc_graph::TradingMatrixNodeId::new(path);
                self.state.trading_matrix_state.toggle(&node_id);
            }
            TradingMatrixPanelAction::SelectNode { node_key } => {
                web_sys::console::log_1(&format!("TradingMatrix: Select node {}", node_key).into());
                let path: Vec<String> = if node_key.is_empty() {
                    Vec::new()
                } else {
                    node_key.split('/').map(|s| s.to_string()).collect()
                };
                let node_id = ob_poc_graph::TradingMatrixNodeId::new(path);
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
                    // Expand all children (category nodes and their descendants)
                    for child in matrix.children() {
                        expand_all_nodes(&mut self.state.trading_matrix_state, child);
                    }
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

    /// Render simplified layout:
    /// - Top 90%: Single viewport (graph widget)
    /// - Bottom 10%: Split - Chat (left 50%) + Session/REPL (right 50%)
    fn render_simplified(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let top_height = available.y * 0.9;
        let bottom_height = available.y * 0.1 - 4.0;

        // Top: Graph viewport (90% height, full width)
        ui.allocate_ui(egui::vec2(available.x, top_height), |ui| {
            egui::Frame::default()
                .inner_margin(0.0)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .show(ui, |ui| {
                    // Render galaxy view at Universe level, otherwise render CBU graph
                    if self.state.view_level == ViewLevel::Universe {
                        self.render_galaxy_view(ui);
                    } else {
                        self.state.graph_widget.ui(ui);
                    }
                });
        });

        ui.separator();

        // Bottom row: Chat (left) + Session/REPL (right)
        ui.horizontal(|ui| {
            ui.set_height(bottom_height);

            // Chat panel (left, 50% width)
            ui.vertical(|ui| {
                ui.set_width(available.x * 0.5 - 4.0);
                egui::Frame::default()
                    .inner_margin(4.0)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
                        chat_panel(ui, &mut self.state);
                    });
            });

            ui.separator();

            // Session/REPL panel (right, 50% width) - human-readable session state
            ui.vertical(|ui| {
                ui.set_width(available.x * 0.5 - 4.0);
                egui::Frame::default()
                    .inner_margin(4.0)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
                        session_panel(ui, &mut self.state);
                    });
            });
        });
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
                                // Render galaxy view at Universe level, otherwise render CBU graph
                                if self.state.view_level == ViewLevel::Universe {
                                    self.render_galaxy_view(ui);
                                } else {
                                    self.state.graph_widget.ui(ui);
                                }
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
                // Render galaxy view at Universe level, otherwise render CBU graph
                if self.state.view_level == ViewLevel::Universe {
                    self.render_galaxy_view(ui);
                } else {
                    self.state.graph_widget.ui(ui);
                }
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
                // Render galaxy view at Universe level, otherwise render CBU graph
                if self.state.view_level == ViewLevel::Universe {
                    self.render_galaxy_view(ui);
                } else {
                    self.state.graph_widget.ui(ui);
                }
            });
    }

    /// Render the galaxy view widget
    fn render_galaxy_view(&mut self, ui: &mut egui::Ui) {
        // Tick animations before rendering (egui-rules: tick BEFORE ui)
        let dt = ui.input(|i| i.stable_dt);
        let zoom = 1.0; // TODO: Get from camera if needed
        let _threshold_crossed = self.state.galaxy_view.tick(dt, zoom);

        // Load mock data if not already loaded
        if !self.state.galaxy_view.has_data() {
            self.state.galaxy_view.load_mock_data();
        }

        // Allocate space and get response
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let screen_rect = response.rect;

        // Create a simple camera for rendering
        use ob_poc_graph::graph::Camera2D;
        let camera = Camera2D::new();

        // Render the galaxy
        self.state
            .galaxy_view
            .render(&painter, &camera, screen_rect);

        // Handle input and get any navigation action
        if let Some(action) =
            self.state
                .galaxy_view
                .handle_input_v2(&response, &camera, screen_rect)
        {
            web_sys::console::log_1(&format!("Galaxy navigation action: {:?}", action).into());
            self.handle_galaxy_action(action);
        }
    }

    /// Handle navigation actions from the galaxy view by sending DSL commands to the agent.
    /// All navigation is agent-directed - UI clicks translate to DSL view verbs.
    fn handle_galaxy_action(&mut self, action: NavigationAction) {
        let dsl_command = match action {
            NavigationAction::DrillIntoCluster { cluster_id } => {
                web_sys::console::log_1(
                    &format!("Galaxy: drilling into cluster: {}", cluster_id).into(),
                );
                // Cluster IDs are formatted as "type:value" (e.g., "client:Allianz", "jurisdiction:LU")
                // Parse and generate appropriate DSL command
                self.cluster_id_to_dsl_command(&cluster_id)
            }
            NavigationAction::DrillIntoCbu { cbu_id } => {
                web_sys::console::log_1(&format!("Galaxy: drilling into CBU: {}", cbu_id).into());
                // CBU drill-down uses view.cbu verb
                Some(format!("(view.cbu :cbu-id \"{}\")", cbu_id))
            }
            NavigationAction::DrillUp => {
                web_sys::console::log_1(&"Galaxy: drilling up".into());
                Some("(view.zoom-out)".to_string())
            }
            NavigationAction::GoToUniverse => {
                web_sys::console::log_1(&"Galaxy: going to universe".into());
                Some("(view.universe)".to_string())
            }
            NavigationAction::GoToBreadcrumb { index } => {
                web_sys::console::log_1(&format!("Galaxy: going to breadcrumb {}", index).into());
                Some(format!("(view.back-to :depth {})", index))
            }
            NavigationAction::Select { node_id, .. } => {
                web_sys::console::log_1(&format!("Galaxy: selected node: {}", node_id).into());
                // Selection doesn't need a DSL command, handled locally
                None
            }
            NavigationAction::DrillIntoEntity { entity_id } => {
                web_sys::console::log_1(
                    &format!("Galaxy: drilling into entity: {}", entity_id).into(),
                );
                // Entity focus - could use view.refine or similar
                Some(format!("(view.select :ids [\"{}\"])", entity_id))
            }
            NavigationAction::SetClusterType { cluster_type } => {
                web_sys::console::log_1(
                    &format!("Galaxy: setting cluster type to {:?}", cluster_type).into(),
                );
                // Re-render universe with new clustering
                Some(format!("(view.universe :cluster-by {:?})", cluster_type))
            }
            NavigationAction::FetchData { scope } => {
                web_sys::console::log_1(&format!("Galaxy: fetch data for {:?}", scope).into());
                // Data fetch is handled by the graph widget, but could trigger refresh
                None
            }
            NavigationAction::Prefetch { scope_id } => {
                web_sys::console::log_1(&format!("Galaxy: prefetch for {}", scope_id).into());
                // Prefetch is a hint, no DSL needed
                None
            }
            // Camera/hover actions are handled locally by the graph widget, no DSL needed
            NavigationAction::Hover { .. }
            | NavigationAction::ClearHover
            | NavigationAction::Deselect
            | NavigationAction::FlyTo { .. }
            | NavigationAction::ZoomTo { .. }
            | NavigationAction::ZoomIn { .. }
            | NavigationAction::ZoomOut { .. }
            | NavigationAction::ZoomFit
            | NavigationAction::Pan { .. }
            | NavigationAction::Center => None,
        };

        // Execute DSL command via session if we have one
        if let Some(cmd) = dsl_command {
            web_sys::console::log_1(&format!("Galaxy: executing DSL: {}", cmd).into());

            // Get session ID - navigation requires an active session
            let Some(session_id) = self.state.session_id else {
                web_sys::console::warn_1(&"Galaxy: no session, cannot execute DSL".into());
                return;
            };

            // Execute DSL directly via session (not through LLM chat)
            if cmd.starts_with('(') {
                // DSL command - execute directly via session
                self.state.execute_dsl_with_content(session_id, cmd);
            } else {
                // Natural language fallback - send via chat infrastructure
                // (All cluster_id_to_dsl_command outputs should be DSL, but this is a safety net)
                web_sys::console::log_1(
                    &format!("Galaxy: sending natural language to chat: {}", cmd).into(),
                );
                self.state.buffers.chat_input = cmd;
                self.state.send_chat_message();
            }
        }
    }

    /// Convert a cluster ID to the appropriate DSL view command.
    /// Cluster IDs are formatted as "type:value" (e.g., "client:Allianz", "jurisdiction:LU")
    fn cluster_id_to_dsl_command(&self, cluster_id: &str) -> Option<String> {
        // Parse cluster ID - expected format: "type:value"
        let parts: Vec<&str> = cluster_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            web_sys::console::warn_1(
                &format!("Galaxy: unexpected cluster ID format: {}", cluster_id).into(),
            );
            // Fallback: filter by name
            return Some(format!(
                "(view.universe :filter {{ :name \"{}\" }})",
                cluster_id
            ));
        }

        let cluster_type = parts[0];
        let cluster_value = parts[1];

        match cluster_type {
            "client" => {
                // Client cluster -> view.book with client name filter
                // Need to look up the client entity ID - for now use universe with filter
                Some(format!(
                    "(view.universe :filter {{ :client \"{}\" }})",
                    cluster_value
                ))
            }
            "jurisdiction" => {
                // Jurisdiction cluster -> view.universe :jurisdiction ["CODE"]
                Some(format!(
                    "(view.universe :jurisdiction [\"{}\"])",
                    cluster_value
                ))
            }
            "fund-type" | "fund_type" => {
                // Fund type cluster -> view.universe :fund-type ["TYPE"]
                Some(format!(
                    "(view.universe :fund-type [\"{}\"])",
                    cluster_value
                ))
            }
            "status" => {
                // Status cluster -> view.universe :status ["STATUS"]
                Some(format!("(view.universe :status [\"{}\"])", cluster_value))
            }
            "risk" | "risk_rating" => {
                // Risk rating cluster
                Some(format!(
                    "(view.universe :risk-rating [\"{}\"])",
                    cluster_value
                ))
            }
            "product" => {
                // Product cluster
                Some(format!("(view.universe :product [\"{}\"])", cluster_value))
            }
            _ => {
                web_sys::console::warn_1(
                    &format!("Galaxy: unknown cluster type: {}", cluster_type).into(),
                );
                // Fallback: try as generic filter
                Some(format!(
                    "(view.universe :filter {{ :{} \"{}\" }})",
                    cluster_type, cluster_value
                ))
            }
        }
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

        // Sync view mode to server session (fire-and-forget)
        if let Some(session_id) = self.session_id {
            let view_mode_str = match mode {
                ViewMode::KycUbo => "KYC_UBO",
                ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
                ViewMode::ProductsOnly => "PRODUCTS_ONLY",
                ViewMode::Trading => "TRADING",
            };
            let view_level_str = format!("{:?}", self.view_level);
            spawn_local(async move {
                if let Err(e) =
                    api::set_view_mode(session_id, view_mode_str, Some(&view_level_str)).await
                {
                    web_sys::console::warn_1(
                        &format!("Failed to sync view mode to server: {}", e).into(),
                    );
                }
            });
        }
    }

    /// Set view level (Universe/Cluster/System/etc) - controls navigation scope
    pub fn set_view_level(&mut self, level: ViewLevel) {
        web_sys::console::log_1(
            &format!(
                "set_view_level: changing from {:?} to {:?}",
                self.view_level, level
            )
            .into(),
        );
        self.view_level = level;

        // When navigating to Universe level, trigger universe fetch
        // When navigating to lower levels, graph refetch handles it
        if let Ok(mut state) = self.async_state.lock() {
            match level {
                ViewLevel::Universe => {
                    state.needs_universe_refetch = true;
                }
                ViewLevel::Cluster => {
                    // TODO: fetch cluster detail when we have cluster_id context
                    state.needs_universe_refetch = true;
                }
                _ => {
                    // System/Planet/Surface/Core levels use CBU graph
                    state.needs_graph_refetch = true;
                }
            }
        }

        // Sync view level to server session (fire-and-forget)
        if let Some(session_id) = self.session_id {
            let view_mode_str = match self.view_mode {
                ViewMode::KycUbo => "KYC_UBO",
                ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
                ViewMode::ProductsOnly => "PRODUCTS_ONLY",
                ViewMode::Trading => "TRADING",
            };
            let view_level_str = format!("{:?}", level);
            spawn_local(async move {
                if let Err(e) =
                    api::set_view_mode(session_id, view_mode_str, Some(&view_level_str)).await
                {
                    web_sys::console::warn_1(
                        &format!("Failed to sync view level to server: {}", e).into(),
                    );
                }
            });
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
                                    // REPL undo/clear commands - require backend session API support
                                    ob_poc_types::AgentCommand::Undo => {
                                        web_sys::console::warn_1(
                                            &"Command: Undo - requires session undo API".into(),
                                        );
                                    }
                                    ob_poc_types::AgentCommand::Clear => {
                                        web_sys::console::warn_1(
                                            &"Command: Clear - requires session clear API".into(),
                                        );
                                    }
                                    ob_poc_types::AgentCommand::Delete { index } => {
                                        web_sys::console::warn_1(
                                            &format!(
                                                "Command: Delete statement {} - requires session delete API",
                                                index
                                            )
                                            .into(),
                                        );
                                    }
                                    ob_poc_types::AgentCommand::DeleteLast => {
                                        web_sys::console::warn_1(
                                            &"Command: DeleteLast - requires session delete API"
                                                .into(),
                                        );
                                    }
                                    // Navigation commands
                                    ob_poc_types::AgentCommand::ShowCbu { cbu_id } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ShowCbu cbu_id={}", cbu_id).into(),
                                        );
                                        // Navigate to CBU using ScaleSystem mechanism
                                        state.pending_scale_system = Some(Some(cbu_id.clone()));
                                    }
                                    ob_poc_types::AgentCommand::HighlightEntity { entity_id } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: HighlightEntity entity_id={}",
                                                entity_id
                                            )
                                            .into(),
                                        );
                                        // Focus entity using FocusEntity mechanism
                                        state.pending_focus_entity = Some(entity_id.clone());
                                    }
                                    ob_poc_types::AgentCommand::NavigateDsl { line } => {
                                        // No DSL editor in simplified UI - log and ignore
                                        web_sys::console::warn_1(
                                            &format!(
                                                "Command: NavigateDsl line={} - not applicable in simplified UI",
                                                line
                                            )
                                            .into(),
                                        );
                                    }
                                    ob_poc_types::AgentCommand::FocusAst { node_id } => {
                                        // No AST view in simplified UI - log and ignore
                                        web_sys::console::warn_1(
                                            &format!(
                                                "Command: FocusAst node_id={} - not applicable in simplified UI",
                                                node_id
                                            )
                                            .into(),
                                        );
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
                                        state.pending_pan = Some((*direction, *amount));
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

                                    // Taxonomy Navigation Commands
                                    ob_poc_types::AgentCommand::TaxonomyShow => {
                                        web_sys::console::log_1(&"Command: TaxonomyShow".into());
                                        // Request taxonomy breadcrumbs refresh to show current position
                                        state.pending_taxonomy_breadcrumbs = true;
                                    }
                                    ob_poc_types::AgentCommand::TaxonomyDrillIn { node_label } => {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Command: TaxonomyDrillIn node_label={}",
                                                node_label
                                            )
                                            .into(),
                                        );
                                        state.pending_taxonomy_zoom_in = Some(node_label.clone());
                                    }
                                    ob_poc_types::AgentCommand::TaxonomyZoomOut => {
                                        web_sys::console::log_1(&"Command: TaxonomyZoomOut".into());
                                        state.pending_taxonomy_zoom_out = true;
                                    }
                                    ob_poc_types::AgentCommand::TaxonomyReset => {
                                        web_sys::console::log_1(&"Command: TaxonomyReset".into());
                                        state.pending_taxonomy_reset = true;
                                    }
                                    ob_poc_types::AgentCommand::TaxonomyFilter { filter } => {
                                        web_sys::console::log_1(
                                            &format!("Command: TaxonomyFilter filter={}", filter)
                                                .into(),
                                        );
                                        state.pending_taxonomy_filter = Some(filter.clone());
                                    }
                                    ob_poc_types::AgentCommand::TaxonomyClearFilter => {
                                        web_sys::console::log_1(
                                            &"Command: TaxonomyClearFilter".into(),
                                        );
                                        state.pending_taxonomy_clear_filter = true;
                                    }

                                    // Hierarchy navigation commands
                                    ob_poc_types::AgentCommand::ExpandNode { node_key } => {
                                        web_sys::console::log_1(
                                            &format!("Command: ExpandNode {}", node_key).into(),
                                        );
                                        state.pending_expand_node = Some(node_key.clone());
                                    }
                                    ob_poc_types::AgentCommand::CollapseNode { node_key } => {
                                        web_sys::console::log_1(
                                            &format!("Command: CollapseNode {}", node_key).into(),
                                        );
                                        state.pending_collapse_node = Some(node_key.clone());
                                    }

                                    // Export and layout commands
                                    ob_poc_types::AgentCommand::Export { format, .. } => {
                                        web_sys::console::log_1(
                                            &format!("Command: Export {:?}", format).into(),
                                        );
                                        state.pending_export = format.clone();
                                    }
                                    ob_poc_types::AgentCommand::ToggleOrientation => {
                                        web_sys::console::log_1(
                                            &"Command: ToggleOrientation".into(),
                                        );
                                        state.pending_toggle_orientation = true;
                                    }
                                    ob_poc_types::AgentCommand::Search { query } => {
                                        web_sys::console::log_1(
                                            &format!("Command: Search {}", query).into(),
                                        );
                                        state.pending_search = Some(query.clone());
                                    }
                                    ob_poc_types::AgentCommand::ShowHelp { .. } => {
                                        web_sys::console::log_1(&"Command: ShowHelp".into());
                                        state.pending_show_help = true;
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

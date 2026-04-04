//! ObservatoryApp — eframe::App implementation.
//!
//! The main update + ui loop. Follows egui rules:
//! 1. Process async results FIRST
//! 2. Tick animations (mutations in update, not ui)
//! 3. Render shell panels → returns Option<Action>
//! 4. Render central canvas → returns Option<Action>
//! 5. Collect and process all actions

use eframe::egui;

use crate::actions::{ObservatoryAction, Tab};
use crate::fetch;
use crate::state::ObservatoryState;

/// The Observatory egui application.
pub struct ObservatoryApp {
    pub state: ObservatoryState,
}

impl ObservatoryApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Extract session ID from browser URL: /observatory/:sessionId
        let session_id = extract_session_id().unwrap_or_default();
        let base_url = extract_base_url();

        log::info!("Observatory starting for session: {session_id}");

        let mut state = ObservatoryState {
            session_id: session_id.clone(),
            base_url,
            ..Default::default()
        };

        // Trigger initial data fetches
        let ctx = cc.egui_ctx.clone();
        fetch::fetch_orientation(&mut state, ctx.clone());
        fetch::fetch_show_packet(&mut state, ctx.clone());
        fetch::fetch_graph_scene(&mut state, ctx);

        Self { state }
    }
}

impl eframe::App for ObservatoryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut actions: Vec<ObservatoryAction> = Vec::new();

        // ── 1. Process async results ──
        fetch::process_results(&mut self.state);

        // ── 2. Tick animations (mutations HERE, not in ui) ──
        let dt = ctx.input(|i| i.predicted_dt);
        self.tick_camera(dt);

        // Request repaint if animation is active
        if self.state.camera.is_animating() {
            ctx.request_repaint();
        }

        // ── 3. Render shell: top bar ──
        egui::TopBottomPanel::top("location_header").show(ctx, |ui| {
            if let Some(action) = crate::shell::location_header::ui(ui, &self.state) {
                actions.push(action);
            }
        });

        // ── 4. Render shell: breadcrumbs ──
        egui::TopBottomPanel::top("breadcrumbs").show(ctx, |ui| {
            if let Some(action) = crate::shell::breadcrumbs::ui(ui, &self.state) {
                actions.push(action);
            }
        });

        // ── 5. Render shell: tab bar ──
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            if let Some(action) = crate::shell::tab_bar::ui(ui, &self.state) {
                actions.push(action);
            }
        });

        // ── 5. Render based on active tab ──
        match self.state.active_tab {
            Tab::Observe => {
                // Side panel: viewports
                egui::SidePanel::right("viewports")
                    .default_width(300.0)
                    .show(ctx, |ui| {
                        if let Some(action) =
                            crate::panels::viewport_dispatcher::ui(ui, &self.state)
                        {
                            actions.push(action);
                        }
                    });

                // Central panel: constellation canvas (painter-driven)
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        if let Some(action) = crate::canvas::render(ui, &self.state) {
                            actions.push(action);
                        }
                    });
            }
            Tab::MissionControl => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(action) =
                        crate::panels::mission_control::ui(ui, &self.state)
                    {
                        actions.push(action);
                    }
                });
            }
        }

        // ── 6. Process all collected actions ──
        for action in actions {
            self.process_action(action, ctx);
        }
    }
}

impl ObservatoryApp {
    /// Advance camera spring interpolation toward target.
    fn tick_camera(&mut self, dt: f32) {
        let cam = &mut self.state.camera;
        let lerp_speed = 8.0 * dt;

        cam.pan_x += (cam.target_pan_x - cam.pan_x) * lerp_speed;
        cam.pan_y += (cam.target_pan_y - cam.pan_y) * lerp_speed;
        cam.zoom += (cam.target_zoom - cam.zoom) * lerp_speed;
    }

    /// Process a single action. Semantic actions trigger server fetch.
    fn process_action(&mut self, action: ObservatoryAction, ctx: &egui::Context) {
        match action {
            // ── Semantic (server round-trip) ──
            ObservatoryAction::Drill { node_id, .. } => {
                log::info!("Drill into: {node_id}");
                fetch::fetch_orientation(&mut self.state, ctx.clone());
                fetch::fetch_show_packet(&mut self.state, ctx.clone());
                fetch::fetch_graph_scene(&mut self.state, ctx.clone());
            }
            ObservatoryAction::SemanticZoomOut => {
                log::info!("Semantic zoom out");
                fetch::fetch_orientation(&mut self.state, ctx.clone());
                fetch::fetch_show_packet(&mut self.state, ctx.clone());
                fetch::fetch_graph_scene(&mut self.state, ctx.clone());
            }
            ObservatoryAction::RefreshData => {
                fetch::fetch_orientation(&mut self.state, ctx.clone());
                fetch::fetch_show_packet(&mut self.state, ctx.clone());
                fetch::fetch_graph_scene(&mut self.state, ctx.clone());
                fetch::fetch_health(&mut self.state, ctx.clone());
            }
            ObservatoryAction::NavigateHistory { .. } => {
                // Phase 7: history replay
            }
            ObservatoryAction::InvokeVerb { verb_fqn } => {
                log::info!("Invoke verb: {verb_fqn}");
                // TODO: POST verb invocation
            }
            ObservatoryAction::SetLens { .. } => {
                // TODO: POST lens change
            }

            // ── Observation frame (local only) ──
            ObservatoryAction::VisualZoom { delta } => {
                let factor = (delta * 0.002).exp();
                self.state.camera.target_zoom =
                    (self.state.camera.target_zoom * factor).clamp(0.05, 10.0);
                ctx.request_repaint();
            }
            ObservatoryAction::Pan { dx, dy } => {
                let z = self.state.camera.zoom;
                self.state.camera.target_pan_x -= dx / z;
                self.state.camera.target_pan_y -= dy / z;
                ctx.request_repaint();
            }
            ObservatoryAction::SelectNode { node_id } => {
                self.state.interaction.selected_node = Some(node_id);
            }
            ObservatoryAction::DeselectNode => {
                self.state.interaction.selected_node = None;
            }
            ObservatoryAction::AnchorNode { node_id } => {
                self.state.camera.anchor_node_id = Some(node_id);
            }
            ObservatoryAction::ClearAnchor => {
                self.state.camera.anchor_node_id = None;
            }
            ObservatoryAction::ResetView => {
                self.state.camera = Default::default();
                ctx.request_repaint();
            }

            // ── UI mode ──
            ObservatoryAction::SwitchTab { tab } => {
                self.state.active_tab = tab;
                if matches!(tab, Tab::MissionControl) {
                    fetch::fetch_health(&mut self.state, ctx.clone());
                }
            }
        }
    }
}

/// Extract session ID from browser URL path: /observatory/:sessionId
fn extract_session_id() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window()?;
        let pathname = window.location().pathname().ok()?;
        // Expect: /observatory/<uuid>
        let parts: Vec<&str> = pathname.trim_matches('/').split('/').collect();
        if parts.len() >= 2 && parts[0] == "observatory" {
            return Some(parts[1].to_string());
        }
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native dev mode: use env var or default
        std::env::var("OBSERVATORY_SESSION_ID").ok()
    }
}

/// Extract base URL for API calls.
fn extract_base_url() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(origin) = window.location().origin() {
                return origin;
            }
        }
        String::new()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("OBSERVATORY_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string())
    }
}

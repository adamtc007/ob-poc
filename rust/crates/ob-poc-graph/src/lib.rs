//! OB-POC Graph Visualization
//!
//! WASM/egui widget for CBU graph visualization.
//! This crate contains ONLY the graph - all text panels are in HTML/TypeScript.

mod api;
mod bridge;
pub mod graph;

use eframe::egui;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub use graph::{CbuGraphData, CbuGraphWidget, GraphEdgeData, GraphNodeData};

/// Shared state for async graph loading
#[derive(Default)]
struct AsyncGraphState {
    /// Pending graph data from async fetch
    pending_data: Option<CbuGraphData>,
    /// Error message if fetch failed
    error: Option<String>,
    /// Whether a fetch is in progress
    loading: bool,
}

/// Graph-only application for WASM embedding
pub struct GraphApp {
    /// The graph widget
    graph_widget: CbuGraphWidget,
    /// Currently loaded CBU
    current_cbu: Option<Uuid>,
    /// JS bridge for HTML panel communication
    js_bridge: bridge::JsBridge,
    /// View mode
    view_mode: graph::ViewMode,
    /// Shared async state
    async_state: Arc<Mutex<AsyncGraphState>>,
    /// egui context for requesting repaints from async code
    #[cfg(target_arch = "wasm32")]
    ctx: Option<egui::Context>,
}

impl GraphApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Setup custom fonts if needed
        let ctx = &cc.egui_ctx;
        ctx.set_visuals(egui::Visuals::dark());

        Self {
            graph_widget: CbuGraphWidget::new(),
            current_cbu: None,
            js_bridge: bridge::JsBridge::new(),
            view_mode: graph::ViewMode::KycUbo,
            async_state: Arc::new(Mutex::new(AsyncGraphState::default())),
            #[cfg(target_arch = "wasm32")]
            ctx: Some(ctx.clone()),
        }
    }

    /// Load a CBU graph (async fetch in WASM)
    pub fn load_cbu(&mut self, cbu_id: Uuid) {
        self.current_cbu = Some(cbu_id);
        self.graph_widget.set_loading(true);

        #[cfg(target_arch = "wasm32")]
        {
            // Set loading state
            if let Ok(mut state) = self.async_state.lock() {
                state.loading = true;
                state.pending_data = None;
                state.error = None;
            }

            // Clone what we need for the async block
            let async_state = Arc::clone(&self.async_state);
            let view_mode = self.view_mode;
            let ctx = self.ctx.clone();

            // Spawn async fetch
            wasm_bindgen_futures::spawn_local(async move {
                web_sys::console::log_1(
                    &format!(
                        "Fetching graph for CBU: {} view_mode: {:?}",
                        cbu_id, view_mode
                    )
                    .into(),
                );

                let api = api::ApiClient::new("");
                let result = api.get_cbu_graph(cbu_id, view_mode).await;

                web_sys::console::log_1(
                    &format!(
                        "API result: {:?}",
                        result
                            .as_ref()
                            .map(|r| r.nodes.len())
                            .map_err(|e| e.clone())
                    )
                    .into(),
                );

                if let Ok(mut state) = async_state.lock() {
                    state.loading = false;
                    match result {
                        Ok(response) => {
                            // Use From impl to convert CbuGraphResponse to CbuGraphData
                            let node_count = response.nodes.len();
                            let edge_count = response.edges.len();
                            let graph_data: CbuGraphData = response.into();

                            web_sys::console::log_1(
                                &format!(
                                    "Graph data converted: {} nodes, {} edges, first node x={:?}",
                                    graph_data.nodes.len(),
                                    graph_data.edges.len(),
                                    graph_data.nodes.first().map(|n| n.x)
                                )
                                .into(),
                            );

                            state.pending_data = Some(graph_data);
                            web_sys::console::log_1(
                                &format!(
                                    "Graph data loaded: {} nodes, {} edges",
                                    node_count, edge_count
                                )
                                .into(),
                            );
                        }
                        Err(e) => {
                            state.error = Some(e.clone());
                            web_sys::console::error_1(
                                &format!("Failed to load graph: {}", e).into(),
                            );
                        }
                    }
                } else {
                    web_sys::console::error_1(&"Failed to acquire async_state lock".into());
                }

                // Request repaint
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            tracing::debug!("load_cbu called in non-WASM environment: {}", cbu_id);
        }
    }
}

impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for pending graph data from async fetch
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(mut state) = self.async_state.lock() {
                if let Some(data) = state.pending_data.take() {
                    self.graph_widget.set_data(data);
                }
            }
        }

        // Check for focus requests from JS
        if let Some(entity_id) = self.js_bridge.poll_focus_request() {
            self.graph_widget.focus_entity(&entity_id);
        }

        // Check for CBU load requests from JS
        if let Some(cbu_id) = self.js_bridge.poll_cbu_request() {
            if let Ok(uuid) = Uuid::parse_str(&cbu_id) {
                self.load_cbu(uuid);
            }
        }

        // Check for view mode changes from JS
        if let Some(mode) = self.js_bridge.poll_view_mode_request() {
            self.view_mode = mode;
            // Reload graph with new view mode
            if let Some(cbu_id) = self.current_cbu {
                self.load_cbu(cbu_id);
            }
        }

        // Render graph - full canvas, no panels
        egui::CentralPanel::default().show(ctx, |ui| {
            self.graph_widget.ui(ui);
        });

        // Notify JS of selection changes
        if let Some(selected) = self.graph_widget.selected_entity_changed() {
            self.js_bridge.emit_entity_selected(&selected);
        }
    }
}

// WASM entry point
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    tracing::info!("OB-POC Graph WASM starting");

    Ok(())
}

/// Start the graph app on a canvas element
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start_graph(canvas_id: &str) -> Result<(), JsValue> {
    use web_sys::HtmlCanvasElement;

    let web_options = eframe::WebOptions::default();

    // Get the canvas element by ID
    let document = web_sys::window()
        .ok_or_else(|| JsValue::from_str("No window"))?
        .document()
        .ok_or_else(|| JsValue::from_str("No document"))?;

    let canvas: HtmlCanvasElement = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("Canvas '{}' not found", canvas_id)))?
        .dyn_into()
        .map_err(|_| JsValue::from_str("Element is not a canvas"))?;

    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(GraphApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });

    Ok(())
}

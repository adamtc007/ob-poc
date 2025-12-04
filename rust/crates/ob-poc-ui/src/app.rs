//! CBU Visualization Application

use crate::api::ApiClient;
use crate::graph::{CbuGraphWidget, LayoutOverride, ViewMode};
use eframe::egui;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

/// Debounce delay in seconds before saving layout
const LAYOUT_SAVE_DEBOUNCE_SECS: f64 = 1.0;

/// Main application
pub struct ObPocApp {
    api: ApiClient,
    graph_widget: CbuGraphWidget,
    selected_cbu: Option<uuid::Uuid>,
    cbu_list: Vec<CbuSummary>,
    loading: bool,
    error: Option<String>,
    view_mode: ViewMode,

    // Async result holders
    pending_cbu_list: Option<Arc<Mutex<Option<Result<Vec<CbuSummary>, String>>>>>,
    pending_graph: Option<Arc<Mutex<Option<Result<crate::graph::CbuGraphData, String>>>>>,
    pending_layout: Option<Arc<Mutex<Option<Result<LayoutOverride, String>>>>>,

    // Deferred layout override to apply after graph loads
    deferred_layout: Option<LayoutOverride>,

    // Debounce state for layout saves
    layout_dirty_since: Option<f64>,

    #[cfg(not(target_arch = "wasm32"))]
    runtime: Arc<tokio::runtime::Runtime>,
}

#[derive(Clone, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: uuid::Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
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
            selected_cbu: None,
            cbu_list: Vec::new(),
            loading: false,
            error: None,
            view_mode: ViewMode::KycUbo,
            pending_cbu_list: None,
            pending_graph: None,
            pending_layout: None,
            deferred_layout: None,
            layout_dirty_since: None,
            #[cfg(not(target_arch = "wasm32"))]
            runtime,
        };

        app.load_cbu_list();
        app
    }

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

    fn load_cbu_view(&mut self, cbu_id: uuid::Uuid) {
        self.loading = true;
        self.error = None;
        self.deferred_layout = None;

        let api = self.api.clone();
        self.graph_widget.set_view_mode(self.view_mode);

        // Load graph
        let graph_result = Arc::new(Mutex::new(None));
        let graph_result_clone = graph_result.clone();
        let graph_path = format!("/api/cbu/{}/graph", cbu_id);

        // Load layout
        let layout_result = Arc::new(Mutex::new(None));
        let layout_result_clone = layout_result.clone();
        let view_str = match self.view_mode {
            ViewMode::KycUbo => "KYC_UBO",
            ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
        };
        let layout_path = format!("/api/cbu/{}/layout?view_mode={}", cbu_id, view_str);

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

        // Check if both graph and layout are pending - wait for BOTH before processing
        let has_pending_graph = self.pending_graph.is_some();
        let has_pending_layout = self.pending_layout.is_some();

        // Check if results are ready (non-destructively)
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

        // Only process when BOTH are ready (or layout is not pending)
        if has_pending_graph && graph_ready && (!has_pending_layout || layout_ready) {
            // First, take the layout if available
            let layout_result = self
                .pending_layout
                .as_ref()
                .and_then(|p| p.try_lock().ok())
                .and_then(|mut g| g.take());
            if let Some(result) = layout_result {
                match result {
                    Ok(overrides) => {
                        self.deferred_layout = Some(overrides);
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to load layout: {}", e));
                    }
                }
                self.pending_layout = None;
            }

            // Now take and process the graph
            let graph_result = self
                .pending_graph
                .as_ref()
                .and_then(|p| p.try_lock().ok())
                .and_then(|mut g| g.take());
            if let Some(result) = graph_result {
                match result {
                    Ok(graph_data) => {
                        self.graph_widget.set_data(graph_data);

                        // Apply deferred layout NOW
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

        // Handle case where layout arrives after graph (fallback)
        if self.graph_widget.has_graph() && self.pending_graph.is_none() {
            let layout_result = self
                .pending_layout
                .as_ref()
                .and_then(|p| p.try_lock().ok())
                .and_then(|mut g| g.take());
            if let Some(result) = layout_result {
                if let Ok(layout) = result {
                    self.graph_widget.apply_layout_override(layout);
                }
                self.pending_layout = None;
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
                            let view_str = match self.view_mode {
                                ViewMode::KycUbo => "KYC_UBO",
                                ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
                            };
                            let path = format!("/api/cbu/{}/layout?view_mode={}", cbu_id, view_str);

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
}

impl eframe::App for ObPocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_pending_requests();

        let now = ctx.input(|i| i.time);

        if self.loading || self.pending_graph.is_some() || self.pending_layout.is_some() {
            ctx.request_repaint();
        }

        let mut clicked_cbu_id: Option<uuid::Uuid> = None;
        let mut refresh_clicked = false;
        let mut view_changed = false;
        let mut new_view_mode = self.view_mode;

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
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

                if ui.button("Refresh").clicked() {
                    refresh_clicked = true;
                }

                ui.separator();

                ui.label("View:");
                if ui
                    .selectable_label(self.view_mode == ViewMode::KycUbo, "KYC / UBO")
                    .clicked()
                {
                    new_view_mode = ViewMode::KycUbo;
                    view_changed = true;
                }
                if ui
                    .selectable_label(
                        self.view_mode == ViewMode::ServiceDelivery,
                        "Service Delivery",
                    )
                    .clicked()
                {
                    new_view_mode = ViewMode::ServiceDelivery;
                    view_changed = true;
                }

                if self.loading {
                    ui.spinner();
                }
            });
        });

        if let Some(cbu_id) = clicked_cbu_id {
            self.selected_cbu = Some(cbu_id);
            self.load_cbu_view(cbu_id);
        }
        if refresh_clicked {
            self.load_cbu_list();
        }
        if view_changed {
            self.view_mode = new_view_mode;
            self.graph_widget.set_view_mode(new_view_mode);
        }

        self.save_layout_debounced(now);

        if self.layout_dirty_since.is_some() {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            } else if let Some(ref err) = self.error {
                ui.colored_label(egui::Color32::RED, err);
            } else {
                self.graph_widget.ui(ui);
            }
        });
    }
}

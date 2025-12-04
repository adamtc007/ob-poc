//! CBU Visualization Application

use crate::api::ApiClient;
use crate::graph::{CbuGraphWidget, ViewMode};
use eframe::egui;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

/// Main application
pub struct ObPocApp {
    api: ApiClient,
    graph_widget: CbuGraphWidget,
    selected_cbu: Option<uuid::Uuid>,
    cbu_list: Vec<CbuSummary>,
    loading: bool,
    error: Option<String>,
    view_mode: ViewMode,

    // Async result holders (work for both native and WASM)
    pending_cbu_list: Option<Arc<Mutex<Option<Result<Vec<CbuSummary>, String>>>>>,
    pending_graph: Option<Arc<Mutex<Option<Result<crate::graph::CbuGraphData, String>>>>>,

    // Tokio runtime for native async
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

        let api = self.api.clone();

        // Load FULL graph data - widget filters by view mode
        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();
        let path = format!("/api/cbu/{}/graph", cbu_id);
        self.graph_widget.set_view_mode(self.view_mode);

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

        self.pending_graph = Some(result);
    }

    fn check_pending_requests(&mut self) {
        // Check CBU list - extract result first, then clear pending
        let cbu_result = self
            .pending_cbu_list
            .as_ref()
            .and_then(|pending| pending.try_lock().ok().and_then(|mut guard| guard.take()));
        if let Some(result) = cbu_result {
            match result {
                Ok(cbus) => self.cbu_list = cbus,
                Err(e) => self.error = Some(format!("Failed to load CBUs: {}", e)),
            }
            self.loading = false;
            self.pending_cbu_list = None;
        }

        // Check graph - extract result first, then clear pending
        let graph_result = self
            .pending_graph
            .as_ref()
            .and_then(|pending| pending.try_lock().ok().and_then(|mut guard| guard.take()));
        if let Some(result) = graph_result {
            match result {
                Ok(graph_data) => self.graph_widget.set_data(graph_data),
                Err(e) => self.error = Some(format!("Failed to load graph: {}", e)),
            }
            self.loading = false;
            self.pending_graph = None;
        }
    }
}

impl eframe::App for ObPocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_pending_requests();

        if self.loading {
            ctx.request_repaint();
        }

        let mut clicked_cbu_id: Option<uuid::Uuid> = None;
        let mut refresh_clicked = false;
        let mut view_changed = false;
        let mut new_view_mode = self.view_mode;

        // Top panel - CBU selector and view toggle
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
            // Graph widget filters cached data by view mode - no reload needed
            self.graph_widget.set_view_mode(new_view_mode);
        }
        // Central panel - Graph view (full width)
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

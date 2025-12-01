//! Main application state and UI

use crate::agent_panel::AgentPanel;
use crate::api::ApiClient;
use crate::graph_view::{CbuGraph, GraphView};
use eframe::egui;
use serde::Deserialize;

/// Main application
pub struct ObPocApp {
    api: ApiClient,
    graph_view: GraphView,
    agent_panel: AgentPanel,
    selected_cbu: Option<uuid::Uuid>,
    cbu_list: Vec<CbuSummary>,
    loading: bool,
    error: Option<String>,

    // Async state
    #[cfg(target_arch = "wasm32")]
    pending_cbu_list:
        Option<std::sync::Arc<std::sync::Mutex<Option<Result<Vec<CbuSummary>, String>>>>>,
    #[cfg(target_arch = "wasm32")]
    pending_graph: Option<std::sync::Arc<std::sync::Mutex<Option<Result<CbuGraph, String>>>>>,
}

#[derive(Clone, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: uuid::Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
}

impl ObPocApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Determine base URL based on environment
        #[cfg(target_arch = "wasm32")]
        let base_url = {
            // In WASM, use the current origin
            web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .unwrap_or_else(|| "http://localhost:3000".to_string())
        };

        #[cfg(not(target_arch = "wasm32"))]
        let base_url = "http://localhost:3000".to_string();

        let mut app = Self {
            api: ApiClient::new(&base_url),
            graph_view: GraphView::new(),
            agent_panel: AgentPanel::new(),
            selected_cbu: None,
            cbu_list: Vec::new(),
            loading: false,
            error: None,
            #[cfg(target_arch = "wasm32")]
            pending_cbu_list: None,
            #[cfg(target_arch = "wasm32")]
            pending_graph: None,
        };

        // Load initial CBU list
        app.load_cbu_list();

        app
    }

    fn load_cbu_list(&mut self) {
        self.loading = true;
        self.error = None;

        #[cfg(target_arch = "wasm32")]
        {
            let api = self.api.clone();
            let result = std::sync::Arc::new(std::sync::Mutex::new(None));
            let result_clone = result.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let res: Result<Vec<CbuSummary>, String> = api.get("/api/cbu").await;
                *result_clone.lock().unwrap() = Some(res);
            });

            self.pending_cbu_list = Some(result);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // For native, we'd need to handle async differently
            // For now, just mark loading complete
            self.loading = false;
        }
    }

    fn load_cbu_graph(&mut self, cbu_id: uuid::Uuid) {
        self.loading = true;
        self.error = None;

        #[cfg(target_arch = "wasm32")]
        {
            let api = self.api.clone();
            let result = std::sync::Arc::new(std::sync::Mutex::new(None));
            let result_clone = result.clone();

            let custody = self.graph_view.show_custody;
            let kyc = self.graph_view.show_kyc;
            let ubo = self.graph_view.show_ubo;
            let services = self.graph_view.show_services;

            wasm_bindgen_futures::spawn_local(async move {
                let path = format!(
                    "/api/cbu/{}/graph?custody={}&kyc={}&ubo={}&services={}",
                    cbu_id, custody, kyc, ubo, services
                );
                let res: Result<CbuGraph, String> = api.get(&path).await;
                *result_clone.lock().unwrap() = Some(res);
            });

            self.pending_graph = Some(result);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.loading = false;
        }
    }

    fn check_pending_requests(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            // Check pending CBU list
            let cbu_result = self
                .pending_cbu_list
                .as_ref()
                .and_then(|pending| pending.try_lock().ok().and_then(|mut guard| guard.take()));
            if let Some(result) = cbu_result {
                match result {
                    Ok(cbus) => {
                        self.cbu_list = cbus;
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to load CBUs: {}", e));
                    }
                }
                self.loading = false;
                self.pending_cbu_list = None;
            }

            // Check pending graph
            let graph_result = self
                .pending_graph
                .as_ref()
                .and_then(|pending| pending.try_lock().ok().and_then(|mut guard| guard.take()));
            if let Some(result) = graph_result {
                match result {
                    Ok(graph) => {
                        self.graph_view.set_graph(graph);
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to load graph: {}", e));
                    }
                }
                self.loading = false;
                self.pending_graph = None;
            }
        }
    }
}

impl eframe::App for ObPocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for completed async requests
        self.check_pending_requests();

        // Request repaint while loading
        if self.loading {
            ctx.request_repaint();
        }

        // Track which CBU was clicked (to handle after borrow ends)
        let mut clicked_cbu_id: Option<uuid::Uuid> = None;
        let mut refresh_clicked = false;
        let mut layers_changed = false;

        // Top panel - CBU selector
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

                // Layer toggles
                ui.label("Layers:");
                let custody_changed = ui
                    .checkbox(&mut self.graph_view.show_custody, "Custody")
                    .changed();
                let kyc_changed = ui.checkbox(&mut self.graph_view.show_kyc, "KYC").changed();
                let ubo_changed = ui.checkbox(&mut self.graph_view.show_ubo, "UBO").changed();
                let services_changed = ui
                    .checkbox(&mut self.graph_view.show_services, "Services")
                    .changed();

                layers_changed = custody_changed || kyc_changed || ubo_changed || services_changed;

                if self.loading {
                    ui.spinner();
                }
            });
        });

        // Handle deferred actions after UI borrows are released
        if let Some(cbu_id) = clicked_cbu_id {
            self.selected_cbu = Some(cbu_id);
            self.load_cbu_graph(cbu_id);
        }
        if refresh_clicked {
            self.load_cbu_list();
        }
        if layers_changed && self.selected_cbu.is_some() {
            self.load_cbu_graph(self.selected_cbu.unwrap());
        }

        // Left panel - Agent prompt
        egui::SidePanel::left("agent_panel")
            .default_width(350.0)
            .show(ctx, |ui| {
                self.agent_panel.ui(ui);
            });

        // Central panel - Graph view
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            } else if let Some(ref err) = self.error {
                ui.colored_label(egui::Color32::RED, err);
            } else {
                self.graph_view.ui(ui);
            }
        });
    }
}

//! Main DSL Visualizer Application
//!
//! This module implements the core egui application for visualizing DSL instances
//! and their corresponding AST structures. It provides a two-panel interface:
//! - Left panel: DSL Browser for listing and filtering DSL instances
//! - Right panel: AST Viewer for interactive tree visualization
//!
//! The application connects to the backend via gRPC to fetch DSL data and
//! parse AST structures in real-time.

use super::{
    ast_viewer::{ASTViewerPanel, LayoutMode},
    constants::*,
    dsl_browser::DSLBrowserPanel,
    grpc_client::DSLServiceClient,
    models::*,
    VisualizerConfig, VisualizerError, VisualizerResult,
};

use eframe::egui;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Main application state for the DSL Visualizer
pub struct DSLVisualizerApp {
    /// Configuration settings
    config: VisualizerConfig,

    /// Current view mode
    current_view: AppView,

    /// DSL browser panel
    dsl_browser: DSLBrowserPanel,

    /// AST viewer panel
    ast_viewer: ASTViewerPanel,

    /// gRPC client for backend communication
    grpc_client: Option<DSLServiceClient>,

    /// Loading states
    is_loading: bool,
    loading_message: String,

    /// Error handling
    error_message: Option<String>,

    /// Auto-refresh timer
    last_refresh: Instant,

    /// Performance metrics
    frame_count: usize,
    last_fps_update: Instant,
    current_fps: f32,

    /// UI state
    show_debug_panel: bool,
    show_settings: bool,
    dark_mode: bool,
}

/// Different views available in the application
#[derive(Debug, Clone, PartialEq)]
pub enum AppView {
    /// Main view showing DSL browser and AST viewer
    Main,
    /// Settings/configuration view
    Settings,
    /// About/help view
    About,
}

impl DSLVisualizerApp {
    /// Create a new DSL visualizer application
    pub fn new(config: VisualizerConfig) -> Self {
        info!("Initializing DSL Visualizer App");

        let dark_mode = config.dark_mode;

        Self {
            config: config.clone(),
            current_view: AppView::Main,
            dsl_browser: DSLBrowserPanel::new(),
            ast_viewer: ASTViewerPanel::new(),
            grpc_client: None,
            is_loading: false,
            loading_message: String::new(),
            error_message: None,
            last_refresh: Instant::now(),
            frame_count: 0,
            last_fps_update: Instant::now(),
            current_fps: 0.0,
            show_debug_panel: config.debug_mode,
            show_settings: false,
            dark_mode,
        }
    }

    /// Initialize the gRPC client connection
    pub fn initialize_grpc_client(&mut self) {
        info!("Connecting to gRPC server at {}", self.config.grpc_endpoint);

        match DSLServiceClient::new(&self.config.grpc_endpoint) {
            Ok(client) => {
                self.grpc_client = Some(client);
                info!("Successfully connected to gRPC server");
            }
            Err(e) => {
                error!("Failed to connect to gRPC server: {}", e);
                self.error_message = Some(format!("gRPC Connection Error: {}", e));
            }
        }
    }

    /// Refresh DSL data from the backend
    pub async fn refresh_dsl_data(&mut self) {
        if self.grpc_client.is_none() {
            self.initialize_grpc_client();
            return;
        }

        if let Some(ref mut client) = self.grpc_client {
            self.is_loading = true;
            self.loading_message = "Fetching DSL instances...".to_string();

            match client.list_dsl_instances(self.config.max_dsl_entries).await {
                Ok(instances) => {
                    info!("Fetched {} DSL instances", instances.len());
                    self.dsl_browser.update_instances(instances);
                    self.error_message = None;
                }
                Err(e) => {
                    error!("Failed to fetch DSL instances: {}", e);
                    self.error_message = Some(format!("Failed to fetch data: {}", e));
                }
            }

            self.is_loading = false;
            self.loading_message.clear();
        }
    }

    /// Handle DSL selection from the browser panel
    pub async fn on_dsl_selected(&mut self, dsl_entry: &DSLEntry) {
        debug!("DSL selected: {}", dsl_entry.id);

        if let Some(ref mut client) = self.grpc_client {
            self.is_loading = true;
            self.loading_message = "Parsing AST...".to_string();

            // Fetch the full DSL content
            match client.get_dsl_content(&dsl_entry.id).await {
                Ok(content) => {
                    // Parse the AST
                    match client.parse_dsl_to_ast(&content).await {
                        Ok(ast) => {
                            info!("Successfully parsed AST for DSL {}", dsl_entry.id);
                            self.ast_viewer.update_ast(ast);
                            self.error_message = None;
                        }
                        Err(e) => {
                            error!("Failed to parse AST: {}", e);
                            self.error_message = Some(format!("AST Parsing Error: {}", e));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch DSL content: {}", e);
                    self.error_message = Some(format!("Failed to fetch DSL content: {}", e));
                }
            }

            self.is_loading = false;
            self.loading_message.clear();
        }
    }

    /// Update FPS counter
    fn update_fps(&mut self) {
        self.frame_count += 1;
        let now = Instant::now();

        if now.duration_since(self.last_fps_update) >= Duration::from_secs(1) {
            self.current_fps = self.frame_count as f32;
            self.frame_count = 0;
            self.last_fps_update = now;
        }
    }

    /// Check if auto-refresh should trigger
    fn should_auto_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= Duration::from_secs(self.config.auto_refresh_interval)
    }

    /// Render the main menu bar
    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Refresh").clicked() {
                    // Trigger refresh
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Settings").clicked() {
                    self.show_settings = true;
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });

            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.dark_mode, "Dark Mode");
                ui.checkbox(&mut self.show_debug_panel, "Debug Panel");

                ui.separator();

                ui.menu_button("Layout", |ui| {
                    if ui.button("Tree View").clicked() {
                        self.ast_viewer.set_layout_mode(LayoutMode::Tree);
                        ui.close_menu();
                    }
                    if ui.button("Graph View").clicked() {
                        self.ast_viewer.set_layout_mode(LayoutMode::Graph);
                        ui.close_menu();
                    }
                    if ui.button("Compact View").clicked() {
                        self.ast_viewer.set_layout_mode(LayoutMode::Compact);
                        ui.close_menu();
                    }
                });
            });

            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    self.current_view = AppView::About;
                    ui.close_menu();
                }
            });

            // Status indicators on the right side
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Connection status
                let connection_color = if self.grpc_client.is_some() {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                };

                ui.colored_label(
                    connection_color,
                    if self.grpc_client.is_some() {
                        "● Connected"
                    } else {
                        "● Disconnected"
                    },
                );

                ui.separator();

                // FPS counter
                ui.label(format!("FPS: {:.0}", self.current_fps));
            });
        });
    }

    /// Render the main application content
    fn render_main_view(&mut self, ctx: &egui::Context) {
        // Auto-refresh logic
        if self.should_auto_refresh() && !self.is_loading {
            // Note: In a real async environment, this would be handled differently
            // For now, we'll just update the timestamp
            self.last_refresh = Instant::now();
        }

        // Main horizontal split
        egui::SidePanel::left("dsl_browser")
            .resizable(true)
            .default_width(ctx.available_rect().width() * DSL_BROWSER_WIDTH_RATIO)
            .min_width(MIN_PANEL_WIDTH)
            .show(ctx, |ui| {
                ui.heading("DSL Browser");
                ui.separator();

                // Pass callback for when DSL is selected
                self.dsl_browser.render(ui, |selected_dsl| {
                    // Store the selection for async processing
                    // In a real implementation, this would trigger an async operation
                    debug!("DSL selected for AST parsing: {}", selected_dsl.id);
                });
            });

        // Main content area for AST viewer
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("AST Visualizer");
            ui.separator();

            if self.is_loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                    ui.label(&self.loading_message);
                });
            } else {
                self.ast_viewer.render(ui);
            }
        });

        // Error handling
        let mut close_error = false;
        let mut retry_error = false;

        if let Some(ref error) = self.error_message {
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.colored_label(egui::Color32::RED, error);

                    ui.horizontal(|ui| {
                        if ui.button("Retry").clicked() {
                            retry_error = true;
                        }

                        if ui.button("Close").clicked() {
                            close_error = true;
                        }
                    });
                });
        }

        if close_error || retry_error {
            self.error_message = None;
            // Retry logic would go here if retry_error is true
        }

        // Debug panel
        if self.show_debug_panel {
            egui::Window::new("Debug Info")
                .default_size([300.0, 200.0])
                .show(ctx, |ui| {
                    ui.label(format!("gRPC Endpoint: {}", self.config.grpc_endpoint));
                    ui.label(format!("Connected: {}", self.grpc_client.is_some()));
                    ui.label(format!("Loading: {}", self.is_loading));
                    ui.label(format!("FPS: {:.1}", self.current_fps));
                    ui.label(format!("Frame Count: {}", self.frame_count));

                    ui.separator();

                    ui.label("DSL Browser State:");
                    ui.label(format!(
                        "  Instances: {}",
                        self.dsl_browser.get_instance_count()
                    ));
                    ui.label(format!(
                        "  Selected: {:?}",
                        self.dsl_browser.get_selected_index()
                    ));

                    ui.separator();

                    ui.label("AST Viewer State:");
                    ui.label(format!("  Has AST: {}", self.ast_viewer.has_ast()));
                    ui.label(format!("  Layout: {:?}", self.ast_viewer.get_layout_mode()));
                });
        }
    }

    /// Render the settings view
    fn render_settings_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Settings");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("gRPC Endpoint:");
                ui.text_edit_singleline(&mut self.config.grpc_endpoint);
            });

            ui.horizontal(|ui| {
                ui.label("Auto-refresh interval (seconds):");
                ui.add(egui::Slider::new(
                    &mut self.config.auto_refresh_interval,
                    10..=300,
                ));
            });

            ui.horizontal(|ui| {
                ui.label("Max DSL entries:");
                ui.add(egui::Slider::new(
                    &mut self.config.max_dsl_entries,
                    10..=1000,
                ));
            });

            ui.checkbox(&mut self.config.dark_mode, "Dark mode");
            ui.checkbox(&mut self.config.debug_mode, "Debug mode");

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    self.dark_mode = self.config.dark_mode;
                    self.show_debug_panel = self.config.debug_mode;
                    self.current_view = AppView::Main;
                }

                if ui.button("Cancel").clicked() {
                    self.current_view = AppView::Main;
                }
            });
        });
    }

    /// Render the about view
    fn render_about_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("About DSL/AST Visualizer");
            ui.separator();

            ui.label("DSL/AST Visualizer v1.0.0");
            ui.label("Part of the OB-POC (Onboarding Proof of Concept) project");
            ui.add_space(10.0);

            ui.label("This application provides interactive visualization of:");
            ui.label("• DSL (Domain Specific Language) instances");
            ui.label("• AST (Abstract Syntax Tree) structures");
            ui.label("• Multi-domain workflow definitions");
            ui.add_space(10.0);

            ui.label("Features:");
            ui.label("• Real-time DSL browsing and filtering");
            ui.label("• Interactive AST tree/graph visualization");
            ui.label("• gRPC integration with backend services");
            ui.label("• Multiple layout modes and customization");
            ui.add_space(10.0);

            ui.label("Built with Rust and egui");
            ui.add_space(20.0);

            if ui.button("Back to Main").clicked() {
                self.current_view = AppView::Main;
            }
        });
    }
}

impl eframe::App for DSLVisualizerApp {
    /// Called each frame to render the UI
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update FPS counter
        self.update_fps();

        // Apply theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        // Render menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu_bar(ui);
        });

        // Render main content based on current view
        match self.current_view {
            AppView::Main => self.render_main_view(ctx),
            AppView::Settings => self.render_settings_view(ctx),
            AppView::About => self.render_about_view(ctx),
        }

        // Request repaint for animations and real-time updates
        ctx.request_repaint_after(Duration::from_millis(DEFAULT_REFRESH_RATE_MS));
    }

    /// Called when the application is about to exit
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        info!("DSL Visualizer application shutting down");
    }

    /// Save application state
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dark_mode", &self.dark_mode);
        eframe::set_value(storage, "show_debug_panel", &self.show_debug_panel);
        eframe::set_value(storage, "grpc_endpoint", &self.config.grpc_endpoint);
    }
}

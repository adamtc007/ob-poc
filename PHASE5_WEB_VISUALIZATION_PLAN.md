# Phase 5: Web-Based AST Visualization (egui/wasm) - Implementation Plan

## Overview

Phase 5 focuses on creating a web-based interface for AST visualization using egui and WebAssembly (wasm). This will provide an interactive, browser-based visualization tool that leverages the domain-specific visualization capabilities developed in Phase 4.

**Timeline**: 2-3 weeks  
**Priority**: High - Critical for user-facing AST visualization capabilities  
**Dependencies**: Phase 4 (Domain-Specific Visualization Features)

## Architecture Overview

### Technology Stack
- **Frontend Framework**: egui (immediate mode GUI)
- **Web Target**: WebAssembly (wasm32-unknown-unknown)
- **HTTP Server**: warp or axum for API endpoints
- **Serialization**: serde_json for data exchange
- **Bundling**: trunk for wasm build and serve

### High-Level Architecture

```
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│   Browser (WASM)    │◄──►│   Web API Server    │◄──►│  AST Visualization  │
│                     │    │                     │    │     Engine          │
│  ┌─────────────────┐│    │ ┌─────────────────┐ │    │ ┌─────────────────┐ │
│  │  egui Web App   ││    │ │ REST Endpoints  │ │    │ │ DslManagerV2    │ │
│  │                 ││    │ │                 │ │    │ │                 │ │
│  │ • AST Renderer  ││    │ │ • /domains      │ │    │ │ • AST Storage   │ │
│  │ • Domain Panel  ││    │ │ • /visualize    │ │    │ │ • Visualization │ │
│  │ • Controls UI   ││    │ │ • /versions     │ │    │ │ • Domain Rules  │ │
│  └─────────────────┘│    │ └─────────────────┘ │    │ └─────────────────┘ │
└─────────────────────┘    └─────────────────────┘    └─────────────────────┘
```

## Implementation Plan

### 5.1 Project Structure Setup (3 days)

#### 5.1.1 Create Web Frontend Crate
**Path**: `ob-poc/web-frontend/`

```rust
// Cargo.toml
[package]
name = "ob-poc-web"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
egui = "0.24"
eframe = { version = "0.24", features = ["default", "wgpu"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = "0.3"
js-sys = "0.3"
reqwest = { version = "0.11", features = ["json"] }
petgraph = "0.6"
egui_extras = { version = "0.24", features = ["svg", "image"] }

[dependencies.web-sys]
version = "0.3"
features = [
  "console",
  "Window",
  "Document",
  "HtmlCanvasElement",
  "WebGlRenderingContext",
  "CanvasRenderingContext2d",
]
```

#### 5.1.2 Setup Trunk Configuration
**Path**: `ob-poc/web-frontend/Trunk.toml`

```toml
[build]
target = "index.html"
dist = "dist"

[watch]
watch = ["src", "../rust/src"]
ignore = ["dist"]

[serve]
address = "127.0.0.1"
port = 8080
open = true

[[hooks]]
stage = "pre_build"
command = "cargo"
command_args = ["build", "--package", "ob-poc", "--release"]
```

#### 5.1.3 HTML Template
**Path**: `ob-poc/web-frontend/index.html`

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>AST Visualization - OB PoC</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        html, body {
            margin: 0;
            padding: 0;
            height: 100%;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #1e1e1e;
            color: #ffffff;
        }
        
        #loading {
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            font-size: 1.2em;
        }
        
        canvas {
            display: block;
            width: 100%;
            height: 100%;
        }
    </style>
</head>
<body>
    <div id="loading">Loading AST Visualization...</div>
    <canvas id="canvas"></canvas>
    
    <script type="module">
        import init from './ob-poc-web.js';
        init().then(() => {
            document.getElementById('loading').style.display = 'none';
        });
    </script>
</body>
</html>
```

### 5.2 Core Web Application Structure (4 days)

#### 5.2.1 Main Application Entry Point
**Path**: `ob-poc/web-frontend/src/lib.rs`

```rust
use eframe::egui;
use wasm_bindgen::prelude::*;

mod app;
mod api_client;
mod ast_renderer;
mod components;
mod types;

use app::AstVisualizationApp;

#[wasm_bindgen]
pub fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "canvas",
            web_options,
            Box::new(|cc| Box::new(AstVisualizationApp::new(cc))),
        )
        .await
        .expect("Failed to start eframe web app");
    });
}
```

#### 5.2.2 Main Application Structure
**Path**: `ob-poc/web-frontend/src/app.rs`

```rust
use eframe::egui;
use std::collections::HashMap;
use crate::{
    api_client::ApiClient,
    ast_renderer::AstRenderer,
    components::{DomainPanel, VisualizationControls, StatusBar},
    types::{Domain, DslVersion, VisualizationData, AppState},
};

pub struct AstVisualizationApp {
    state: AppState,
    api_client: ApiClient,
    ast_renderer: AstRenderer,
    
    // UI Components
    domain_panel: DomainPanel,
    visualization_controls: VisualizationControls,
    status_bar: StatusBar,
    
    // Data
    domains: Vec<Domain>,
    selected_domain: Option<String>,
    selected_version: Option<String>,
    visualization_data: Option<VisualizationData>,
    
    // UI State
    loading: bool,
    error_message: Option<String>,
    show_domain_panel: bool,
    show_controls: bool,
}

impl AstVisualizationApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure egui style for dark theme
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        
        let mut app = Self {
            state: AppState::Loading,
            api_client: ApiClient::new("http://localhost:8081/api".to_string()),
            ast_renderer: AstRenderer::new(),
            domain_panel: DomainPanel::new(),
            visualization_controls: VisualizationControls::new(),
            status_bar: StatusBar::new(),
            domains: Vec::new(),
            selected_domain: None,
            selected_version: None,
            visualization_data: None,
            loading: false,
            error_message: None,
            show_domain_panel: true,
            show_controls: true,
        };
        
        // Start loading domains
        app.load_domains();
        app
    }
    
    fn load_domains(&mut self) {
        let api_client = self.api_client.clone();
        let ctx = egui::Context::default(); // Get from app context
        
        wasm_bindgen_futures::spawn_local(async move {
            match api_client.get_domains().await {
                Ok(domains) => {
                    // Update app state with domains
                    // This requires sharing state between async context and app
                    // Implementation depends on chosen state management pattern
                }
                Err(e) => {
                    // Handle error
                }
            }
            ctx.request_repaint();
        });
    }
    
    fn load_visualization(&mut self, domain: &str, version: &str) {
        self.loading = true;
        self.error_message = None;
        
        let api_client = self.api_client.clone();
        let domain = domain.to_string();
        let version = version.to_string();
        
        wasm_bindgen_futures::spawn_local(async move {
            match api_client.get_visualization(&domain, &version).await {
                Ok(viz_data) => {
                    // Update visualization data
                }
                Err(e) => {
                    // Handle error
                }
            }
        });
    }
}

impl eframe::App for AstVisualizationApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Export SVG").clicked() {
                        // Export functionality
                    }
                    if ui.button("Export PNG").clicked() {
                        // Export functionality
                    }
                });
                
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_domain_panel, "Domain Panel");
                    ui.checkbox(&mut self.show_controls, "Controls");
                });
                
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        // Show about dialog
                    }
                });
            });
        });
        
        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            self.status_bar.show(ui, &self.state, self.loading);
        });
        
        // Left sidebar for domain selection
        if self.show_domain_panel {
            egui::SidePanel::left("domain_panel")
                .resizable(true)
                .min_width(250.0)
                .show(ctx, |ui| {
                    self.domain_panel.show(
                        ui,
                        &self.domains,
                        &mut self.selected_domain,
                        &mut self.selected_version,
                        |domain, version| self.load_visualization(domain, version)
                    );
                });
        }
        
        // Right sidebar for visualization controls
        if self.show_controls {
            egui::SidePanel::right("controls_panel")
                .resizable(true)
                .min_width(200.0)
                .show(ctx, |ui| {
                    self.visualization_controls.show(
                        ui,
                        self.visualization_data.as_ref(),
                        &mut |changes| {
                            // Apply visualization changes
                            if let Some(ref mut viz_data) = self.visualization_data {
                                // Update visualization based on control changes
                            }
                        }
                    );
                });
        }
        
        // Central panel for AST visualization
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                    ui.label("Loading visualization...");
                });
            } else if let Some(error) = &self.error_message {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                });
            } else if let Some(ref viz_data) = self.visualization_data {
                // Render AST visualization
                self.ast_renderer.render(ui, viz_data);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a domain and version to visualize");
                });
            }
        });
    }
}
```

### 5.3 AST Renderer Implementation (5 days)

#### 5.3.1 Core AST Renderer
**Path**: `ob-poc/web-frontend/src/ast_renderer.rs`

```rust
use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use std::collections::HashMap;
use crate::types::{VisualizationData, VisualNode, VisualEdge, NodeStyle, EdgeStyle};

pub struct AstRenderer {
    zoom: f32,
    pan_offset: Vec2,
    node_positions: HashMap<String, Pos2>,
    selected_node: Option<String>,
    hovered_node: Option<String>,
    drag_start: Option<Pos2>,
}

impl AstRenderer {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            node_positions: HashMap::new(),
            selected_node: None,
            hovered_node: None,
            drag_start: None,
        }
    }
    
    pub fn render(&mut self, ui: &mut egui::Ui, viz_data: &VisualizationData) {
        let available_rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available_rect, egui::Sense::click_and_drag());
        
        // Handle input events
        self.handle_input(&response, ui);
        
        // Set up coordinate transformation
        let transform = self.get_transform(available_rect);
        
        // Render background
        ui.painter().rect_filled(
            available_rect,
            0.0,
            Color32::from_rgb(30, 30, 30)
        );
        
        // Render edges first (so they appear behind nodes)
        for edge in &viz_data.edges {
            self.render_edge(ui, edge, &transform, viz_data);
        }
        
        // Render nodes
        for node in &viz_data.nodes {
            self.render_node(ui, node, &transform, viz_data);
        }
        
        // Render domain-specific highlights
        self.render_domain_highlights(ui, viz_data, &transform);
        
        // Render selection indicators
        self.render_selection_indicators(ui, &transform);
    }
    
    fn handle_input(&mut self, response: &egui::Response, ui: &mut egui::Ui) {
        // Handle zoom
        if let Some(hover_pos) = response.hover_pos() {
            let scroll_delta = ui.input(|i| i.scroll_delta);
            if scroll_delta.y != 0.0 {
                let zoom_delta = 1.0 + scroll_delta.y * 0.001;
                self.zoom = (self.zoom * zoom_delta).clamp(0.1, 5.0);
            }
        }
        
        // Handle pan
        if response.dragged() {
            self.pan_offset += response.drag_delta();
        }
        
        // Handle node selection
        if response.clicked() {
            if let Some(hover_pos) = response.interact_pointer_pos() {
                self.selected_node = self.get_node_at_position(hover_pos);
            }
        }
    }
    
    fn get_transform(&self, rect: Rect) -> Transform {
        Transform {
            offset: rect.center().to_vec2() + self.pan_offset,
            scale: self.zoom,
        }
    }
    
    fn render_node(&mut self, ui: &mut egui::Ui, node: &VisualNode, transform: &Transform, viz_data: &VisualizationData) {
        let world_pos = self.node_positions.get(&node.id)
            .copied()
            .unwrap_or_else(|| self.calculate_node_position(&node.id, viz_data));
        
        let screen_pos = transform.world_to_screen(world_pos);
        let node_size = Vec2::splat(60.0 * transform.scale);
        let node_rect = Rect::from_center_size(screen_pos, node_size);
        
        // Get node style (domain-specific or default)
        let style = self.get_node_style(node, viz_data);
        
        // Render node background
        ui.painter().rect_filled(
            node_rect,
            5.0,
            style.background_color
        );
        
        // Render node border
        ui.painter().rect_stroke(
            node_rect,
            5.0,
            Stroke::new(style.border_width, style.border_color)
        );
        
        // Render node label
        let text_color = if self.selected_node.as_ref() == Some(&node.id) {
            Color32::WHITE
        } else {
            style.text_color
        };
        
        ui.painter().text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            &node.label,
            egui::FontId::proportional(12.0 * transform.scale),
            text_color
        );
        
        // Render domain-specific annotations
        if let Some(annotations) = &node.domain_annotations {
            self.render_node_annotations(ui, screen_pos, annotations, transform);
        }
    }
    
    fn render_edge(&self, ui: &mut egui::Ui, edge: &VisualEdge, transform: &Transform, viz_data: &VisualizationData) {
        let from_pos = self.node_positions.get(&edge.from)
            .and_then(|pos| Some(transform.world_to_screen(*pos)));
        let to_pos = self.node_positions.get(&edge.to)
            .and_then(|pos| Some(transform.world_to_screen(*pos)));
        
        if let (Some(from), Some(to)) = (from_pos, to_pos) {
            let style = self.get_edge_style(edge, viz_data);
            
            // Render edge line
            ui.painter().line_segment(
                [from, to],
                Stroke::new(style.width * transform.scale, style.color)
            );
            
            // Render arrow head
            if style.show_arrow {
                self.render_arrow_head(ui, from, to, &style, transform.scale);
            }
            
            // Render edge label
            if let Some(label) = &edge.label {
                let mid_point = (from + to.to_vec2()) * 0.5;
                ui.painter().text(
                    mid_point.to_pos2(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0 * transform.scale),
                    style.color
                );
            }
        }
    }
    
    fn render_domain_highlights(&self, ui: &mut egui::Ui, viz_data: &VisualizationData, transform: &Transform) {
        // Render critical paths with special highlighting
        for highlight in &viz_data.domain_highlights {
            match highlight.highlight_type.as_str() {
                "critical_path" => self.render_critical_path_highlight(ui, highlight, transform),
                "ubo_chain" => self.render_ubo_chain_highlight(ui, highlight, transform),
                "compliance_checkpoint" => self.render_compliance_highlight(ui, highlight, transform),
                _ => {}
            }
        }
    }
    
    fn get_node_style(&self, node: &VisualNode, viz_data: &VisualizationData) -> NodeRenderStyle {
        // Apply domain-specific styling
        if let Some(domain_style) = &node.domain_style {
            NodeRenderStyle {
                background_color: Color32::from_hex(&domain_style.color).unwrap_or(Color32::GRAY),
                border_color: Color32::from_hex(&domain_style.border_color).unwrap_or(Color32::WHITE),
                border_width: domain_style.border_width,
                text_color: Color32::from_hex(&domain_style.font_color).unwrap_or(Color32::WHITE),
            }
        } else {
            NodeRenderStyle::default()
        }
    }
    
    fn calculate_node_position(&self, node_id: &str, viz_data: &VisualizationData) -> Pos2 {
        // Implement force-directed layout algorithm
        // or use predefined layout from server
        // For now, simple grid layout
        let index = viz_data.nodes.iter().position(|n| n.id == node_id).unwrap_or(0);
        let cols = (viz_data.nodes.len() as f32).sqrt().ceil() as usize;
        let row = index / cols;
        let col = index % cols;
        
        Pos2::new(
            col as f32 * 100.0,
            row as f32 * 80.0
        )
    }
}

struct Transform {
    offset: Vec2,
    scale: f32,
}

impl Transform {
    fn world_to_screen(&self, world_pos: Pos2) -> Pos2 {
        (world_pos.to_vec2() * self.scale + self.offset).to_pos2()
    }
}

#[derive(Debug, Clone)]
struct NodeRenderStyle {
    background_color: Color32,
    border_color: Color32,
    border_width: f32,
    text_color: Color32,
}

impl Default for NodeRenderStyle {
    fn default() -> Self {
        Self {
            background_color: Color32::from_rgb(70, 70, 70),
            border_color: Color32::from_rgb(150, 150, 150),
            border_width: 2.0,
            text_color: Color32::WHITE,
        }
    }
}
```

### 5.4 API Client Implementation (2 days)

#### 5.4.1 HTTP API Client
**Path**: `ob-poc/web-frontend/src/api_client.rs`

```rust
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::types::{Domain, DslVersion, VisualizationData, VisualizationOptions};

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
    
    pub async fn get_domains(&self) -> Result<Vec<Domain>, ApiError> {
        let url = format!("{}/domains", self.base_url);
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if response.status().is_success() {
            let domains: Vec<Domain> = response.json().await?;
            Ok(domains)
        } else {
            Err(ApiError::HttpError(response.status().as_u16()))
        }
    }
    
    pub async fn get_domain_versions(&self, domain_name: &str) -> Result<Vec<DslVersion>, ApiError> {
        let url = format!("{}/domains/{}/versions", self.base_url, domain_name);
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if response.status().is_success() {
            let versions: Vec<DslVersion> = response.json().await?;
            Ok(versions)
        } else {
            Err(ApiError::HttpError(response.status().as_u16()))
        }
    }
    
    pub async fn get_visualization(&self, domain: &str, version: &str) -> Result<VisualizationData, ApiError> {
        let url = format!("{}/visualize/{}/{}", self.base_url, domain, version);
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if response.status().is_success() {
            let viz_data: VisualizationData = response.json().await?;
            Ok(viz_data)
        } else {
            Err(ApiError::HttpError(response.status().as_u16()))
        }
    }
    
    pub async fn get_domain_enhanced_visualization(
        &self,
        domain: &str,
        version: &str,
        options: Option<VisualizationOptions>
    ) -> Result<VisualizationData, ApiError> {
        let url = format!("{}/visualize/{}/{}/enhanced", self.base_url, domain, version);
        let mut request = self.client.get(&url);
        
        if let Some(opts) = options {
            request = request.json(&opts);
        }
        
        let response = request.send().await?;
        
        if response.status().is_success() {
            let viz_data: VisualizationData = response.json().await?;
            Ok(viz_data)
        } else {
            Err(ApiError::HttpError(response.status().as_u16()))
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    NetworkError(reqwest::Error),
    HttpError(u16),
    ParseError(String),
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        ApiError::NetworkError(error)
    }
}
```

### 5.5 Web API Server (3 days)

#### 5.5.1 API Server Implementation
**Path**: `ob-poc/web-api/src/main.rs`

```rust
use warp::Filter;
use serde_json;
use std::sync::Arc;
use ob_poc::{
    dsl_manager::DslManager,
    database::DslDomainRepository,
};

mod handlers;
mod types;

use handlers::{
    get_domains,
    get_domain_versions,
    get_visualization,
    get_enhanced_visualization,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Setup database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());
    
    let pool = sqlx::PgPool::connect(&database_url).await?;
    let repository = DslDomainRepository::new(pool);
    let manager = Arc::new(DslManagerV2::new(repository));

    // CORS configuration
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "DELETE", "PUT"]);

    // API routes
    let api_routes = warp::path("api")
        .and(
            // GET /api/domains
            warp::path("domains")
                .and(warp::path::end())
                .and(warp::get())
                .and(with_manager(manager.clone()))
                .and_then(get_domains)
                
            .or(
                // GET /api/domains/{domain}/versions
                warp::path("domains")
                    .and(warp::path::param::<String>())
                    .and(warp::path("versions"))
                    .and(warp::path::end())
                    .and(warp::get())
                    .and(with_manager(manager.clone()))
                    .and_then(get_domain_versions)
            )
            
            .or(
                // GET /api/visualize/{domain}/{version}
                warp::path("visualize")
                    .and(warp::path::param::<String>())
                    .and(warp::path::param::<String>())
                    .and(warp::path::end())
                    .and(warp::get())
                    .and(with_manager(manager.clone()))
                    .and_then(get_visualization)
            )
            
            .or(
                // GET /api/visualize/{domain}/{version}/enhanced
                warp::path("visualize")
                    .and(warp::path::param::<String>())
                    .and(warp::path::param::<String>())
                    .and(warp::path("enhanced"))
                    .and(warp::path::end())
                    .and(warp::get())
                    .and(warp::query::query::<std::collections::HashMap<String, String>>())
                    .and(with_manager(manager.clone()))
                    .and_then(get_enhanced_visualization)
            )
        );

    // Static file serving for development
    let static_files = warp::fs::dir("../web-frontend/dist");

    let routes = api_routes
        .or(static_files)
        .with(cors)
        .recover(handle_rejection);

    println!("🚀 Web API server starting on http://localhost:8081");
    warp::serve(routes)
        .run(([127, 0, 0, 1], 8081))
        .await;

    Ok(())
}

fn with_manager(
    manager: Arc<DslManagerV2>,
) -> impl Filter<Extract = (Arc<DslManagerV2>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || manager.clone())
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = warp::http::StatusCode::NOT_FOUND;
        message = "Not Found";
    } else if let Some(_) = err.find::<warp::filters::body::BodyDeserializeError>() {
        code = warp::http::StatusCode::BAD_REQUEST;
        message = "Invalid Body";
    } else {
        code = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal Server Error";
    }

    let json = warp::reply::json(&serde_json::json!({
        "error": message,
        "code": code.as_u16(),
    }));

    Ok(warp::reply::with_status(json, code))
}
```

### 5.6 Integration and Testing (3 days)

#### 5.6.1 Development Workflow
1. **Backend Development**: `cargo run --bin web-api

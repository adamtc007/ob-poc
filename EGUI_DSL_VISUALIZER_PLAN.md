# egui DSL/AST Visualizer Implementation Plan

## 🎯 Overview

Create an interactive egui-based desktop application to visualize DSL code and corresponding AST structures with filtering, gRPC integration, and clean UI navigation.

## 🏗️ Architecture Stack

```
┌─────────────────────────────────────────────────────────────┐
│                    egui Desktop App                         │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │   DSL Browser   │  │  AST Visualizer │  │ Filter Panel │ │
│  │     Panel       │  │     Panel       │  │              │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                   gRPC Client Layer                        │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │   DSL Service   │  │ Parser Service  │  │ Query Service│ │
│  │    Client       │  │     Client      │  │    Client    │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                    Rust Backend Services                   │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │   gRPC Server   │  │  DSL Repository │  │  AST Engine  │ │
│  │   (Tonic)       │  │   (Database)    │  │   (Parser)   │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 📁 Project Structure

```
ob-poc/
├── rust/
│   ├── src/
│   │   ├── bin/
│   │   │   └── egui_visualizer.rs          # Main egui desktop app
│   │   ├── grpc_server.rs                  # gRPC server implementation
│   │   ├── visualizer/                     # Visualizer components
│   │   │   ├── mod.rs
│   │   │   ├── app.rs                      # Main egui application
│   │   │   ├── dsl_browser.rs             # DSL list/filter panel
│   │   │   ├── ast_viewer.rs              # AST tree visualization
│   │   │   ├── grpc_client.rs             # gRPC client wrapper
│   │   │   └── models.rs                  # UI data models
│   │   └── proto/                         # Generated gRPC code
│   ├── proto/                             # Protocol definitions
│   │   ├── dsl_service.proto
│   │   ├── parser_service.proto
│   │   └── visualizer_service.proto
│   └── Cargo.toml                         # Updated dependencies
```

## 🔧 Dependencies & Features

### Cargo.toml Updates
```toml
[dependencies]
# Existing dependencies...

# egui for desktop GUI
egui = "0.28"
eframe = { version = "0.28", default-features = false, features = [
    "default_fonts",
    "glow",
    "persistence",
] }

# Tree visualization
egui_extras = { version = "0.28", features = ["table", "datepicker"] }

# Icons and styling
egui_phosphor = "0.4"

# Async runtime integration with egui
tokio = { version = "1", features = ["full"] }
futures = "0.3"

[features]
default = ["visualizer"]
visualizer = ["dep:egui", "dep:eframe", "dep:egui_extras"]
```

## 🎨 UI Component Design

### 1. Main Application Window

```rust
pub struct DSLVisualizerApp {
    // State management
    current_view: AppView,
    selected_dsl: Option<DslEntry>,
    
    // UI Panels
    dsl_browser: DslBrowserPanel,
    ast_viewer: AstViewerPanel,
    filter_panel: FilterPanel,
    
    // gRPC client
    grpc_client: Arc<VisualizerGrpcClient>,
    
    // Async state
    loading: bool,
    error_message: Option<String>,
}

enum AppView {
    DslBrowser,
    AstViewer { dsl: DslEntry, ast: AstData },
}
```

### 2. DSL Browser Panel

```rust
pub struct DslBrowserPanel {
    search_filter: String,
    domain_filter: Option<String>,
    date_filter: Option<DateRange>,
    dsl_entries: Vec<DslEntry>,
    selected_index: Option<usize>,
}

pub struct DslEntry {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub created_at: DateTime<Utc>,
    pub version: u32,
    pub description: String,
    pub content_preview: String,
}
```

### 3. AST Viewer Panel

```rust
pub struct AstViewerPanel {
    ast_tree: Option<AstNode>,
    selected_node: Option<NodeId>,
    show_details: bool,
    layout_mode: LayoutMode,
}

pub struct AstNode {
    pub id: NodeId,
    pub node_type: String,
    pub label: String,
    pub properties: HashMap<String, Value>,
    pub children: Vec<AstNode>,
    pub position: Option<(f32, f32)>,
}

enum LayoutMode {
    Tree,       // Traditional tree layout
    Graph,      // Force-directed graph
    Compact,    // Compact indented list
}
```

## 🌐 gRPC Service Definitions

### dsl_service.proto
```protobuf
syntax = "proto3";
package ob_poc.visualizer;

service DSLService {
  rpc ListDSLs(ListDSLsRequest) returns (ListDSLsResponse);
  rpc GetDSL(GetDSLRequest) returns (GetDSLResponse);
  rpc ParseDSL(ParseDSLRequest) returns (ParseDSLResponse);
}

message ListDSLsRequest {
  string search_filter = 1;
  string domain_filter = 2;
  int32 limit = 3;
  int32 offset = 4;
}

message ListDSLsResponse {
  repeated DSLEntry entries = 1;
  int32 total_count = 2;
}

message DSLEntry {
  string id = 1;
  string name = 2;
  string domain = 3;
  string created_at = 4;
  uint32 version = 5;
  string description = 6;
  string content_preview = 7;
}

message GetDSLRequest {
  string id = 1;
}

message GetDSLResponse {
  string id = 1;
  string content = 2;
  ASTData ast = 3;
}

message ParseDSLRequest {
  string content = 1;
}

message ParseDSLResponse {
  ASTData ast = 1;
  repeated ParseError errors = 2;
}

message ASTData {
  ASTNode root = 1;
  map<string, string> metadata = 2;
}

message ASTNode {
  string id = 1;
  string node_type = 2;
  string label = 3;
  map<string, string> properties = 4;
  repeated ASTNode children = 5;
}

message ParseError {
  string message = 1;
  int32 line = 2;
  int32 column = 3;
}
```

## 🎯 Implementation Phases

### Phase 1: Core Infrastructure (Week 1)
- [ ] Set up egui desktop application skeleton
- [ ] Implement basic gRPC client/server communication
- [ ] Create DSL list view with mock data
- [ ] Basic window layout and navigation

### Phase 2: DSL Browser (Week 2)
- [ ] Implement DSL filtering and search
- [ ] Connect to real database via gRPC
- [ ] Add DSL entry selection and preview
- [ ] Implement pagination for large DSL lists

### Phase 3: AST Visualization (Week 3)
- [ ] Create tree-style AST viewer
- [ ] Add node selection and property display
- [ ] Implement expandable/collapsible nodes
- [ ] Add syntax highlighting for DSL content

### Phase 4: Advanced Features (Week 4)
- [ ] Graph layout for complex ASTs
- [ ] Export functionality (PNG, SVG, JSON)
- [ ] Real-time DSL parsing and validation
- [ ] Error highlighting and debugging

## 🔍 Key UI Features

### DSL Browser Panel
```rust
impl DslBrowserPanel {
    fn render(&mut self, ui: &mut Ui, grpc_client: &Arc<VisualizerGrpcClient>) -> Option<DslEntry> {
        ui.heading("DSL Library");
        
        // Search bar
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search_filter);
            if ui.button("🔍").clicked() {
                self.refresh_list(grpc_client);
            }
        });
        
        // Filters
        ui.horizontal(|ui| {
            ui.label("Domain:");
            ComboBox::from_label("")
                .selected_text(self.domain_filter.as_ref().unwrap_or(&"All".to_string()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.domain_filter, None, "All");
                    ui.selectable_value(&mut self.domain_filter, Some("KYC".to_string()), "KYC");
                    ui.selectable_value(&mut self.domain_filter, Some("UBO".to_string()), "UBO");
                    ui.selectable_value(&mut self.domain_filter, Some("Onboarding".to_string()), "Onboarding");
                });
        });
        
        // DSL list
        ScrollArea::vertical().show(ui, |ui| {
            for (i, entry) in self.dsl_entries.iter().enumerate() {
                let selected = self.selected_index == Some(i);
                
                ui.group(|ui| {
                    if ui.selectable_label(selected, &entry.name).clicked() {
                        self.selected_index = Some(i);
                        return Some(entry.clone());
                    }
                    
                    ui.small(format!("Domain: {} | Version: {} | {}", 
                        entry.domain, entry.version, entry.created_at.format("%Y-%m-%d")));
                    
                    ui.small(&entry.description);
                    
                    // Preview
                    ui.collapsing("Preview", |ui| {
                        ui.code(&entry.content_preview);
                    });
                });
            }
            None
        }).inner
    }
}
```

### AST Viewer Panel
```rust
impl AstViewerPanel {
    fn render(&mut self, ui: &mut Ui, dsl_entry: &DslEntry, ast: &AstData) {
        ui.heading(format!("AST: {}", dsl_entry.name));
        
        // Toolbar
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.layout_mode, LayoutMode::Tree, "🌳 Tree");
            ui.selectable_value(&mut self.layout_mode, LayoutMode::Graph, "🕸 Graph");
            ui.selectable_value(&mut self.layout_mode, LayoutMode::Compact, "📋 Compact");
            
            ui.separator();
            
            if ui.button("💾 Export PNG").clicked() {
                // Export functionality
            }
            
            if ui.button("❌ Close").clicked() {
                return Some(AppView::DslBrowser);
            }
        });
        
        // Split panel: AST tree + details
        ui.horizontal(|ui| {
            // AST Tree (left side)
            ui.vertical(|ui| {
                ui.label("AST Structure:");
                ScrollArea::both().show(ui, |ui| {
                    if let Some(ref root) = ast.root {
                        self.render_ast_node(ui, root, 0);
                    }
                });
            });
            
            // Node details (right side)
            ui.separator();
            ui.vertical(|ui| {
                ui.label("Node Details:");
                if let Some(selected) = &self.selected_node {
                    self.render_node_details(ui, selected, ast);
                } else {
                    ui.label("Select a node to view details");
                }
            });
        });
        
        None
    }
    
    fn render_ast_node(&mut self, ui: &mut Ui, node: &AstNode, indent: usize) {
        let indent_str = "  ".repeat(indent);
        let selected = self.selected_node.as_ref() == Some(&node.id);
        
        ui.horizontal(|ui| {
            ui.add_space(indent as f32 * 20.0);
            
            let label = format!("{}{} ({})", indent_str, node.label, node.node_type);
            if ui.selectable_label(selected, label).clicked() {
                self.selected_node = Some(node.id.clone());
            }
        });
        
        // Render children
        for child in &node.children {
            self.render_ast_node(ui, child, indent + 1);
        }
    }
}
```

## 🚀 gRPC Server Implementation

### Server Setup
```rust
// src/grpc_server.rs
use tonic::{transport::Server, Request, Response, Status};

pub mod dsl_service {
    tonic::include_proto!("ob_poc.visualizer");
}

use dsl_service::{
    dsl_service_server::{DslService, DslServiceServer},
    ListDslsRequest, ListDslsResponse, GetDslRequest, GetDslResponse
};

#[derive(Default)]
pub struct DSLServiceImpl {
    // Database connection, etc.
}

#[tonic::async_trait]
impl DslService for DSLServiceImpl {
    async fn list_dsls(&self, request: Request<ListDslsRequest>) -> Result<Response<ListDslsResponse>, Status> {
        let req = request.into_inner();
        
        // Query database for DSL entries
        let entries = self.query_dsl_entries(&req).await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        
        let response = ListDslsResponse {
            entries,
            total_count: entries.len() as i32,
        };
        
        Ok(Response::new(response))
    }
    
    async fn get_dsl(&self, request: Request<GetDslRequest>) -> Result<Response<GetDslResponse>, Status> {
        let req = request.into_inner();
        
        // Get DSL content and parse AST
        let dsl_content = self.get_dsl_content(&req.id).await
            .map_err(|e| Status::not_found(format!("DSL not found: {}", e)))?;
            
        let ast = self.parse_dsl_to_ast(&dsl_content).await
            .map_err(|e| Status::internal(format!("Parse error: {}", e)))?;
        
        let response = GetDslResponse {
            id: req.id,
            content: dsl_content,
            ast: Some(ast),
        };
        
        Ok(Response::new(response))
    }
}

pub async fn start_grpc_server() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let dsl_service = DSLServiceImpl::default();

    Server::builder()
        .add_service(DslServiceServer::new(dsl_service))
        .serve(addr)
        .await?;

    Ok(())
}
```

## 🎨 Main Application Entry Point

```rust
// src/bin/egui_visualizer.rs
use eframe::{egui, NativeOptions};
use ob_poc::visualizer::DSLVisualizerApp;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("DSL/AST Visualizer"),
        ..Default::default()
    };

    eframe::run_native(
        "DSL Visualizer",
        options,
        Box::new(|cc| {
            // Setup app
            let app = DSLVisualizerApp::new(cc);
            Ok(Box::new(app))
        }),
    )
}
```

## 🧪 Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dsl_entry_filtering() {
        let mut browser = DslBrowserPanel::new();
        browser.dsl_entries = vec![
            DslEntry { domain: "KYC".to_string(), ..Default::default() },
            DslEntry { domain: "UBO".to_string(), ..Default::default() },
        ];
        
        browser.domain_filter = Some("KYC".to_string());
        let filtered = browser.get_filtered_entries();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].domain, "KYC");
    }
    
    #[tokio::test]
    async fn test_grpc_dsl_retrieval() {
        let client = create_test_grpc_client().await;
        let response = client.get_dsl("test-dsl-123").await.unwrap();
        assert!(!response.content.is_empty());
        assert!(response.ast.is_some());
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_end_to_end_visualization() {
    // Start gRPC server
    let server_handle = tokio::spawn(start_grpc_server());
    
    // Create client and test full flow
    let client = create_grpc_client().await;
    
    // Test DSL list retrieval
    let list_response = client.list_dsls(ListDslsRequest::default()).await.unwrap();
    assert!(!list_response.entries.is_empty());
    
    // Test DSL content and AST retrieval
    let dsl_id = &list_response.entries[0].id;
    let get_response = client.get_dsl(dsl_id).await.unwrap();
    
    assert!(!get_response.content.is_empty());
    assert!(get_response.ast.is_some());
    
    // Cleanup
    server_handle.abort();
}
```

## 📋 Development Checklist

### Setup Phase
- [ ] Add egui dependencies to Cargo.toml
- [ ] Create protobuf service definitions
- [ ] Set up basic egui application structure
- [ ] Implement gRPC server skeleton

### Core Features
- [ ] DSL list retrieval via gRPC
- [ ] Search and filtering functionality
- [ ] AST tree visualization
- [ ] Node selection and details display
- [ ] Navigation between DSL browser and AST viewer

### Polish & Testing
- [ ] Error handling and user feedback
- [ ] Loading states and progress indicators
- [ ] Comprehensive test suite
- [ ] Documentation and usage examples
- [ ] Performance optimization

## 🔄 Future Enhancements

### Advanced Visualization
- [ ] Interactive graph layout with drag-and-drop
- [ ] Minimap for large ASTs
- [ ] Zoom and pan controls
- [ ] Multiple layout algorithms (force-directed, hierarchical, circular)

### Analysis Features
- [ ] AST diff comparison between versions
- [ ] Syntax error highlighting
- [ ] DSL validation and linting
- [ ] Performance metrics and timing

### Export & Integration
- [ ] Multiple export formats (PNG, SVG, PDF, JSON)
- [ ] Integration with external tools
- [ ] Plugin system for custom node renderers
- [ ] Theme customization

This comprehensive plan provides a complete roadmap for implementing the egui DSL/AST visualizer with clean architecture, proper separation of concerns, and a great user experience.
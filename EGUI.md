# EGUI.md - Full egui/WASM/Rust Stack Refactoring Brief

## ⛔ QUICK REFERENCE: Read EGUI-RULES.md First

**Before implementing ANY egui panels, read `/EGUI-RULES.md` (246 lines).**

It contains the 5 non-negotiable rules that MUST be followed. This document (EGUI.md) is the comprehensive brief. EGUI-RULES.md is the short, sharp checklist.

---

## Executive Summary

Refactor ob-poc from hybrid architecture (Axum + TypeScript/HTML + egui/WASM graph) to **full egui/WASM/Rust** UI. The server remains the single source of truth. The UI becomes a pure view/render layer with zero business logic.

**Current State:**
```
Axum Server → HTML/TypeScript Panels + egui/WASM Graph
             ↓
      3 language boundaries, serialization bugs, sync complexity
```

**Target State:**
```
Axum Server → egui/WASM (entire UI)
             ↓
      1 language, server-first state, immediate mode rendering
```

---

## Prerequisites (MUST complete before Phase 1)

### Type Consolidation

All API request/response types MUST live in `ob-poc-types` before starting egui work. The WASM UI needs these types, and inline definitions in route handlers are not accessible.

**Currently scattered inline types to move:**

| File | Types to Move |
|------|---------------|
| `dsl_viewer_routes.rs` | `DslListResponse`, `DslShowResponse`, `DslHistoryResponse`, `DslInstanceSummary`, `DslVersionSummary`, `ErrorResponse` |
| `entity_routes.rs` | `EntitySearchResponse` |
| `attribute_routes.rs` | `UploadDocumentRequest`, `UploadDocumentResponse`, `ValidateDslRequest`, `ValidateDslResponse`, `ValidateValueRequest`, `ValidateValueResponse`, `AttributeListResponse`, `AttributeValue` |
| `graph_routes.rs` | `LayoutSaveRequest`, `NodeOffset`, `NodeSizeOverride` |
| `agent_service.rs` | `AgentChatRequest`, `AgentChatResponse` (if different from session chat types) |

**Rules for moving types:**

1. **All types get TS export:**
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, TS)]
   #[ts(export)]
   pub struct MyType { ... }
   ```

2. **UUIDs as String** for TypeScript compatibility:
   ```rust
   pub cbu_id: String,  // NOT Uuid
   ```

3. **Tagged enums** for TypeScript discrimination:
   ```rust
   #[serde(tag = "type", rename_all = "snake_case")]
   ```

4. **Run binding export** after changes:
   ```bash
   cargo test --package ob-poc-types export_bindings
   ```

5. **Update route handlers** to import from `ob_poc_types` instead of local definitions

**Verification:**
```bash
# Should find NO inline struct definitions in route files
grep -r "struct.*Request\|struct.*Response" rust/src/api/*.rs
# All should be in ob-poc-types
```

### Why This Matters

```
BEFORE (broken):
  route_handler.rs → struct MyResponse { ... }  # Not visible to WASM
  
AFTER (works):
  ob-poc-types/src/lib.rs → pub struct MyResponse { ... }
  route_handler.rs → use ob_poc_types::MyResponse;
  ob-poc-ui (WASM) → use ob_poc_types::MyResponse;  ✓
```

---

## Core Principles

### 1. Server is the ONLY Source of Truth

```rust
// CORRECT: State comes from server
pub struct AppState {
    session: Option<SessionStateResponse>,  // GET /api/session/:id
    graph: Option<CbuGraphData>,            // GET /api/cbu/:id/graph
    validation: Option<ValidationResponse>, // POST /api/agent/validate
}

// WRONG: Local state that can drift
pub struct AppState {
    local_messages: Vec<Message>,           // NO - server owns messages
    is_dirty: bool,                         // NO - refetch instead
    cached_entities: HashMap<Uuid, Entity>, // NO - server owns entities
}
```

### 2. Action → Server → Refetch → Render

Every user action that changes data follows this pattern:

```rust
// User clicks "Execute DSL"
fn handle_execute(&mut self, ctx: &egui::Context) {
    let dsl = self.buffers.dsl_editor.clone();
    let async_state = Arc::clone(&self.async_state);
    let ctx = ctx.clone();
    
    spawn_local(async move {
        // 1. POST to server
        let result = api::execute_dsl(&dsl).await;
        
        // 2. Store result for next frame
        if let Ok(mut state) = async_state.lock() {
            state.pending_execution = Some(result);
        }
        
        // 3. Trigger repaint (which will refetch dependent data)
        ctx.request_repaint();
    });
}

// In update(), after execution completes:
fn process_pending(&mut self) {
    if self.async_state.lock().unwrap().pending_execution.take().is_some() {
        // Refetch everything that might have changed
        self.refetch_session();
        self.refetch_graph();
    }
}
```

### 3. Text Buffers are the ONLY Local State

The UI owns draft text being edited. Everything else comes from server.

```rust
pub struct TextBuffers {
    pub chat_input: String,      // Draft message being typed
    pub dsl_editor: String,      // DSL being edited (synced on blur/save)
    pub search_query: String,    // Entity search input
}
```

### 4. No "Dirty" Flags - Just Refetch

Instead of tracking what changed locally:

```rust
// WRONG: Complex sync logic
if self.is_dirty && self.last_sync > 5.0 {
    self.sync_to_server();
}

// CORRECT: Refetch after actions
fn after_any_mutation(&mut self) {
    self.refetch_session();  // Server tells us current state
}
```

---

## Architecture

### Crate Structure

```
rust/crates/
├── ob-poc-ui/                    # NEW: Full egui application
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                # WASM entry point
│   │   ├── app.rs                # Main App struct, update loop
│   │   ├── state.rs              # AppState, AsyncState
│   │   ├── api.rs                # HTTP client (reuse from ob-poc-graph)
│   │   ├── panels/
│   │   │   ├── mod.rs
│   │   │   ├── chat.rs           # Chat panel
│   │   │   ├── dsl_editor.rs     # DSL editor with syntax highlighting
│   │   │   ├── entity_detail.rs  # Entity details panel
│   │   │   ├── results.rs        # Execution results panel
│   │   │   └── toolbar.rs        # Top toolbar (CBU selector, view mode)
│   │   ├── widgets/
│   │   │   ├── mod.rs
│   │   │   ├── entity_search.rs  # Entity autocomplete combo
│   │   │   ├── status_badge.rs   # KYC status, risk rating badges
│   │   │   ├── code_editor.rs    # Syntax-highlighted text editor
│   │   │   └── message.rs        # Chat message renderer
│   │   └── theme.rs              # Colors, fonts, spacing
│   └── pkg/                      # WASM output
│
├── ob-poc-graph/                 # KEEP: Graph widget (embed in ob-poc-ui)
│   └── ...                       # Already well-structured
│
├── ob-poc-types/                 # KEEP: Shared API types
│   └── ...                       # Already has all needed types
│
└── ob-poc-web/                   # DEPRECATE: Remove after migration
    └── ...                       # HTML/TypeScript panels
```

### Dependencies (ob-poc-ui/Cargo.toml)

```toml
[package]
name = "ob-poc-ui"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# egui ecosystem
eframe = { version = "0.29", default-features = false, features = ["wgpu", "persistence"] }
egui = "0.29"
egui_extras = { version = "0.29", features = ["syntect"] }  # Syntax highlighting

# Async
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "console", "Window", "Document", "HtmlCanvasElement",
    "Request", "RequestInit", "RequestMode", "Response", "Headers",
    "EventSource", "MessageEvent"  # For SSE
]}

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Internal
ob-poc-types = { path = "../ob-poc-types" }
ob-poc-graph = { path = "../ob-poc-graph" }

# Utils
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
tracing-wasm = "0.2"
console_error_panic_hook = "0.1"

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.2", features = ["js"] }
```

---

## State Management

### AppState Structure

```rust
// src/state.rs

use ob_poc_types::*;
use ob_poc_graph::{CbuGraphData, CbuGraphWidget, ViewMode};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Main application state
pub struct AppState {
    // =========================================================================
    // SERVER DATA (fetched via API, never modified locally)
    // =========================================================================
    
    /// Current session (includes messages, active_cbu, bindings)
    pub session: Option<SessionStateResponse>,
    
    /// Session ID (persisted to localStorage)
    pub session_id: Option<Uuid>,
    
    /// Graph data for current CBU
    pub graph_data: Option<CbuGraphData>,
    
    /// Last validation result
    pub validation: Option<ValidationResponse>,
    
    /// Last execution result
    pub execution: Option<ExecuteResponse>,
    
    /// Available CBUs for selector
    pub cbu_list: Vec<CbuSummary>,
    
    // =========================================================================
    // UI-ONLY STATE (ephemeral, not persisted)
    // =========================================================================
    
    /// Text being edited (drafts before submission)
    pub buffers: TextBuffers,
    
    /// Current view mode
    pub view_mode: ViewMode,
    
    /// Panel visibility
    pub panels: PanelState,
    
    /// Selected entity in graph (for detail panel)
    pub selected_entity_id: Option<String>,
    
    /// Graph widget (owns camera, input state)
    pub graph_widget: CbuGraphWidget,
    
    // =========================================================================
    // ASYNC COORDINATION
    // =========================================================================
    
    /// Shared state for async operations
    pub async_state: Arc<Mutex<AsyncState>>,
    
    /// egui context for triggering repaints from async
    pub ctx: Option<egui::Context>,
}

/// Text buffers for user input (the ONLY local mutable state)
#[derive(Default)]
pub struct TextBuffers {
    /// Chat message being composed
    pub chat_input: String,
    
    /// DSL source being edited
    pub dsl_editor: String,
    
    /// Entity search query
    pub entity_search: String,
    
    /// DSL editor dirty flag (for "unsaved changes" warning only)
    pub dsl_dirty: bool,
}

/// Panel visibility state
#[derive(Default)]
pub struct PanelState {
    pub show_chat: bool,
    pub show_dsl_editor: bool,
    pub show_results: bool,
    pub show_entity_detail: bool,
    pub layout: LayoutMode,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    #[default]
    FourPanel,      // 2x2 grid
    EditorFocus,    // Large DSL editor + small panels
    GraphFocus,     // Large graph + small panels
}

/// Async operation results (written by spawn_local, read by update loop)
#[derive(Default)]
pub struct AsyncState {
    // Pending results from async operations
    pub pending_session: Option<Result<SessionStateResponse, String>>,
    pub pending_graph: Option<Result<CbuGraphData, String>>,
    pub pending_validation: Option<Result<ValidationResponse, String>>,
    pub pending_execution: Option<Result<ExecuteResponse, String>>,
    pub pending_chat_event: Option<ChatStreamEvent>,
    pub pending_cbu_list: Option<Result<Vec<CbuSummary>, String>>,
    
    // Loading flags
    pub loading_session: bool,
    pub loading_graph: bool,
    pub loading_chat: bool,
    pub executing: bool,
    
    // Error state
    pub last_error: Option<String>,
}

/// Summary for CBU selector dropdown
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
}
```

### Processing Async Results

```rust
// src/app.rs

impl App {
    /// Called at start of each frame to process async results
    fn process_async_results(&mut self) {
        let mut state = self.async_state.lock().unwrap();
        
        // Process session fetch
        if let Some(result) = state.pending_session.take() {
            state.loading_session = false;
            match result {
                Ok(session) => {
                    // Sync DSL editor if server has different content
                    if let Some(ref pending_dsl) = session.pending_dsl {
                        if self.buffers.dsl_editor.is_empty() || !self.buffers.dsl_dirty {
                            self.buffers.dsl_editor = pending_dsl.clone();
                            self.buffers.dsl_dirty = false;
                        }
                    }
                    self.session = Some(session);
                }
                Err(e) => {
                    state.last_error = Some(format!("Session fetch failed: {}", e));
                }
            }
        }
        
        // Process graph fetch
        if let Some(result) = state.pending_graph.take() {
            state.loading_graph = false;
            match result {
                Ok(data) => {
                    self.graph_widget.set_data(data.clone());
                    self.graph_data = Some(data);
                }
                Err(e) => {
                    state.last_error = Some(format!("Graph fetch failed: {}", e));
                }
            }
        }
        
        // Process chat SSE events
        if let Some(event) = state.pending_chat_event.take() {
            self.handle_chat_event(event);
        }
        
        // Process validation
        if let Some(result) = state.pending_validation.take() {
            match result {
                Ok(validation) => self.validation = Some(validation),
                Err(e) => state.last_error = Some(e),
            }
        }
        
        // Process execution
        if let Some(result) = state.pending_execution.take() {
            state.executing = false;
            match result {
                Ok(execution) => {
                    self.execution = Some(execution);
                    // Refetch graph - execution may have changed entities
                    self.refetch_graph();
                    // Refetch session - bindings may have changed
                    self.refetch_session();
                }
                Err(e) => state.last_error = Some(e),
            }
        }
        
        // Process CBU list
        if let Some(result) = state.pending_cbu_list.take() {
            match result {
                Ok(list) => self.cbu_list = list,
                Err(e) => state.last_error = Some(e),
            }
        }
    }
}
```

---

## Panel Implementations

### Chat Panel

```rust
// src/panels/chat.rs

use egui::{ScrollArea, TextEdit, Ui, Vec2};
use crate::state::AppState;
use crate::widgets::message::render_message;

pub fn chat_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        // Header
        ui.horizontal(|ui| {
            ui.heading("Agent Chat");
            if state.async_state.lock().unwrap().loading_chat {
                ui.spinner();
            }
        });
        
        ui.separator();
        
        // Messages area (scrollable)
        let available_height = ui.available_height() - 60.0; // Reserve space for input
        ScrollArea::vertical()
            .max_height(available_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if let Some(ref session) = state.session {
                    for msg in &session.messages {
                        render_message(ui, msg);
                        ui.add_space(8.0);
                    }
                }
            });
        
        ui.separator();
        
        // Input area
        ui.horizontal(|ui| {
            let response = TextEdit::multiline(&mut state.buffers.chat_input)
                .desired_width(ui.available_width() - 80.0)
                .desired_rows(2)
                .hint_text("Ask the agent to generate DSL...")
                .show(ui);
            
            // Send on Ctrl+Enter
            let send_shortcut = ui.input(|i| {
                i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl
            });
            
            let can_send = !state.buffers.chat_input.trim().is_empty() 
                && !state.async_state.lock().unwrap().loading_chat;
            
            if (ui.add_enabled(can_send, egui::Button::new("Send")).clicked() || send_shortcut) 
                && can_send 
            {
                state.send_chat_message();
            }
        });
    });
}

impl AppState {
    pub fn send_chat_message(&mut self) {
        let message = std::mem::take(&mut self.buffers.chat_input);
        if message.trim().is_empty() {
            return;
        }
        
        let Some(session_id) = self.session_id else { return };
        
        // Set loading state
        {
            let mut async_state = self.async_state.lock().unwrap();
            async_state.loading_chat = true;
        }
        
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();
        
        // Start SSE connection for streaming response
        spawn_local(async move {
            let url = format!("/api/session/{}/chat/stream", session_id);
            
            match api::post_sse(&url, &ChatRequest { message }).await {
                Ok(event_source) => {
                    // Process SSE events
                    // Each event updates async_state.pending_chat_event
                    // and calls ctx.request_repaint()
                    api::process_chat_sse(event_source, async_state, ctx).await;
                }
                Err(e) => {
                    if let Ok(mut state) = async_state.lock() {
                        state.loading_chat = false;
                        state.last_error = Some(e);
                    }
                }
            }
        });
    }
}
```

### DSL Editor Panel

```rust
// src/panels/dsl_editor.rs

use egui::{ScrollArea, TextEdit, Ui, Color32, RichText};
use egui_extras::syntax_highlighting::{self, CodeTheme};
use crate::state::AppState;

pub fn dsl_editor_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        // Header with actions
        ui.horizontal(|ui| {
            ui.heading("DSL Editor");
            
            if state.buffers.dsl_dirty {
                ui.label(RichText::new("●").color(Color32::YELLOW).size(12.0));
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Execute button
                let executing = state.async_state.lock().unwrap().executing;
                if ui.add_enabled(!executing, egui::Button::new("▶ Execute")).clicked() {
                    state.execute_dsl();
                }
                
                // Validate button
                if ui.button("✓ Validate").clicked() {
                    state.validate_dsl();
                }
                
                // Clear button
                if ui.button("Clear").clicked() {
                    state.buffers.dsl_editor.clear();
                    state.buffers.dsl_dirty = true;
                }
            });
        });
        
        ui.separator();
        
        // Validation errors (if any)
        if let Some(ref validation) = state.validation {
            if !validation.errors.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("⚠").color(Color32::RED));
                    for error in &validation.errors {
                        ui.label(RichText::new(&error.message).color(Color32::RED).small());
                    }
                });
                ui.separator();
            }
        }
        
        // Editor area with syntax highlighting
        let theme = CodeTheme::from_memory(ui.ctx());
        ScrollArea::vertical().show(ui, |ui| {
            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                let mut layout_job = syntax_highlighting::highlight(
                    ui.ctx(),
                    &theme,
                    string,
                    "clojure", // S-expression syntax
                );
                layout_job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(layout_job))
            };
            
            let response = TextEdit::multiline(&mut state.buffers.dsl_editor)
                .font(egui::TextStyle::Monospace)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(20)
                .layouter(&mut layouter)
                .show(ui);
            
            if response.response.changed() {
                state.buffers.dsl_dirty = true;
            }
        });
        
        // Status bar
        ui.separator();
        ui.horizontal(|ui| {
            let line_count = state.buffers.dsl_editor.lines().count();
            ui.label(format!("{} lines", line_count));
            
            if let Some(ref session) = state.session {
                if let Some(ref cbu) = session.active_cbu {
                    ui.separator();
                    ui.label(format!("Context: {}", cbu.name));
                }
            }
        });
    });
}

impl AppState {
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
    
    pub fn execute_dsl(&mut self) {
        let dsl = self.buffers.dsl_editor.clone();
        let session_id = self.session_id;
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();
        
        {
            let mut state = async_state.lock().unwrap();
            state.executing = true;
        }
        
        spawn_local(async move {
            let result = api::execute_dsl(session_id, &dsl).await;
            if let Ok(mut state) = async_state.lock() {
                state.pending_execution = Some(result);
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }
}
```

### Entity Detail Panel

```rust
// src/panels/entity_detail.rs

use egui::{ScrollArea, Ui, RichText, Color32};
use crate::state::AppState;
use crate::widgets::status_badge::{kyc_status_badge, risk_rating_badge};

pub fn entity_detail_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.heading("Entity Details");
        ui.separator();
        
        let Some(ref entity_id) = state.selected_entity_id else {
            ui.centered_and_justified(|ui| {
                ui.label("Select an entity in the graph");
            });
            return;
        };
        
        // Find entity in graph data
        let Some(ref graph) = state.graph_data else {
            ui.label("No graph loaded");
            return;
        };
        
        let Some(node) = graph.nodes.iter().find(|n| &n.id == entity_id) else {
            ui.label(format!("Entity {} not found", entity_id));
            return;
        };
        
        ScrollArea::vertical().show(ui, |ui| {
            // Entity header
            ui.horizontal(|ui| {
                ui.heading(&node.label);
                if let Some(ref entity_type) = node.entity_type {
                    ui.label(RichText::new(entity_type).italics().color(Color32::GRAY));
                }
            });
            
            ui.add_space(8.0);
            
            // Status badges
            ui.horizontal(|ui| {
                if let Some(ref status) = node.kyc_status {
                    kyc_status_badge(ui, status);
                }
                if let Some(ref risk) = node.risk_rating {
                    risk_rating_badge(ui, risk);
                }
            });
            
            ui.add_space(12.0);
            ui.separator();
            
            // Roles
            if !node.roles.is_empty() {
                ui.label(RichText::new("Roles").strong());
                for role in &node.roles {
                    ui.horizontal(|ui| {
                        ui.label("•");
                        ui.label(role);
                    });
                }
                ui.add_space(8.0);
            }
            
            // Jurisdiction
            if let Some(ref jurisdiction) = node.jurisdiction {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Jurisdiction:").strong());
                    ui.label(jurisdiction);
                });
            }
            
            // Ownership percentage
            if let Some(pct) = node.ownership_pct {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Ownership:").strong());
                    ui.label(format!("{:.1}%", pct));
                });
            }
            
            ui.add_space(12.0);
            ui.separator();
            
            // Actions
            ui.horizontal(|ui| {
                if ui.button("View KYC Case").clicked() {
                    // TODO: Navigate to KYC case
                }
                if ui.button("Edit Entity").clicked() {
                    // TODO: Open edit dialog
                }
            });
        });
    });
}
```

### Results Panel

```rust
// src/panels/results.rs

use egui::{ScrollArea, Ui, RichText, Color32};
use crate::state::AppState;

pub fn results_panel(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.heading("Execution Results");
        ui.separator();
        
        let Some(ref execution) = state.execution else {
            ui.centered_and_justified(|ui| {
                ui.label("Execute DSL to see results");
            });
            return;
        };
        
        // Summary
        ui.horizontal(|ui| {
            if execution.success {
                ui.label(RichText::new("✓ Success").color(Color32::GREEN));
            } else {
                ui.label(RichText::new("✗ Failed").color(Color32::RED));
            }
            ui.separator();
            ui.label(format!("{} steps", execution.steps.len()));
        });
        
        ui.add_space(8.0);
        
        // Step results
        ScrollArea::vertical().show(ui, |ui| {
            for (i, step) in execution.steps.iter().enumerate() {
                ui.horizontal(|ui| {
                    let icon = if step.success { "✓" } else { "✗" };
                    let color = if step.success { Color32::GREEN } else { Color32::RED };
                    ui.label(RichText::new(icon).color(color));
                    ui.label(format!("{}.", i + 1));
                    ui.label(&step.verb);
                });
                
                if let Some(ref binding) = step.binding {
                    ui.indent(format!("step_{}_binding", i), |ui| {
                        ui.label(RichText::new(format!("@{}", binding)).monospace().small());
                    });
                }
                
                if let Some(ref error) = step.error {
                    ui.indent(format!("step_{}_error", i), |ui| {
                        ui.label(RichText::new(error).color(Color32::RED).small());
                    });
                }
                
                ui.add_space(4.0);
            }
        });
        
        // Bindings summary
        if !execution.bindings.is_empty() {
            ui.separator();
            ui.label(RichText::new("Bindings").strong());
            for (name, value) in &execution.bindings {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("@{}", name)).monospace());
                    ui.label("→");
                    ui.label(RichText::new(value).small());
                });
            }
        }
    });
}
```

### Toolbar Panel

```rust
// src/panels/toolbar.rs

use egui::{Ui, ComboBox};
use ob_poc_graph::ViewMode;
use crate::state::AppState;

pub fn toolbar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        // CBU Selector
        ui.label("CBU:");
        let current_name = state.session
            .as_ref()
            .and_then(|s| s.active_cbu.as_ref())
            .map(|c| c.name.as_str())
            .unwrap_or("Select...");
        
        ComboBox::from_id_salt("cbu_selector")
            .selected_text(current_name)
            .show_ui(ui, |ui| {
                for cbu in &state.cbu_list {
                    if ui.selectable_label(
                        state.session.as_ref()
                            .and_then(|s| s.active_cbu.as_ref())
                            .map(|c| c.id == cbu.cbu_id.to_string())
                            .unwrap_or(false),
                        &cbu.name
                    ).clicked() {
                        state.select_cbu(cbu.cbu_id);
                    }
                }
            });
        
        ui.separator();
        
        // View Mode Selector
        ui.label("View:");
        ComboBox::from_id_salt("view_mode")
            .selected_text(state.view_mode.display_name())
            .show_ui(ui, |ui| {
                for mode in ViewMode::all() {
                    if ui.selectable_label(
                        state.view_mode == *mode,
                        mode.display_name()
                    ).clicked() {
                        state.set_view_mode(*mode);
                    }
                }
            });
        
        ui.separator();
        
        // Layout selector
        ui.label("Layout:");
        if ui.selectable_label(state.panels.layout == LayoutMode::FourPanel, "4-Panel").clicked() {
            state.panels.layout = LayoutMode::FourPanel;
        }
        if ui.selectable_label(state.panels.layout == LayoutMode::EditorFocus, "Editor").clicked() {
            state.panels.layout = LayoutMode::EditorFocus;
        }
        if ui.selectable_label(state.panels.layout == LayoutMode::GraphFocus, "Graph").clicked() {
            state.panels.layout = LayoutMode::GraphFocus;
        }
        
        // Spacer
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Error indicator
            if let Some(ref error) = state.async_state.lock().unwrap().last_error {
                if ui.button(RichText::new("⚠").color(Color32::RED)).clicked() {
                    // Show error dialog
                }
                ui.label(RichText::new(error).color(Color32::RED).small());
            }
            
            // Loading indicator
            let async_state = state.async_state.lock().unwrap();
            if async_state.loading_session || async_state.loading_graph || async_state.loading_chat {
                ui.spinner();
            }
        });
    });
}

impl AppState {
    pub fn select_cbu(&mut self, cbu_id: Uuid) {
        let Some(session_id) = self.session_id else { return };
        
        let async_state = Arc::clone(&self.async_state);
        let ctx = self.ctx.clone();
        
        spawn_local(async move {
            // Bind CBU to session
            let result = api::bind_entity(session_id, cbu_id, "cbu").await;
            
            if result.is_ok() {
                // Refetch session to get updated context
                let session_result = api::get_session(session_id).await;
                if let Ok(mut state) = async_state.lock() {
                    state.pending_session = Some(session_result);
                }
            }
            
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        
        // Also fetch graph for new CBU
        self.fetch_graph(cbu_id);
    }
    
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
        self.graph_widget.set_view_mode(mode);
        
        // Refetch graph with new view mode
        if let Some(cbu_id) = self.session
            .as_ref()
            .and_then(|s| s.active_cbu.as_ref())
            .and_then(|c| Uuid::parse_str(&c.id).ok())
        {
            self.fetch_graph(cbu_id);
        }
    }
}
```

---

## Main App Structure

```rust
// src/app.rs

use eframe::egui;
use crate::state::{AppState, AsyncState, PanelState, TextBuffers, LayoutMode};
use crate::panels::{chat_panel, dsl_editor_panel, entity_detail_panel, results_panel, toolbar};
use ob_poc_graph::CbuGraphWidget;
use std::sync::{Arc, Mutex};

pub struct App {
    state: AppState,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Setup visuals
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        
        let mut state = AppState {
            session: None,
            session_id: None,
            graph_data: None,
            validation: None,
            execution: None,
            cbu_list: Vec::new(),
            buffers: TextBuffers::default(),
            view_mode: ob_poc_graph::ViewMode::KycUbo,
            panels: PanelState {
                show_chat: true,
                show_dsl_editor: true,
                show_results: true,
                show_entity_detail: true,
                layout: LayoutMode::FourPanel,
            },
            selected_entity_id: None,
            graph_widget: CbuGraphWidget::new(),
            async_state: Arc::new(Mutex::new(AsyncState::default())),
            ctx: Some(cc.egui_ctx.clone()),
        };
        
        // Try to restore session from localStorage
        state.restore_session();
        
        // Fetch initial data
        state.fetch_cbu_list();
        
        Self { state }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending async results
        self.state.process_async_results();
        
        // Check for entity selection changes from graph widget
        if let Some(entity_id) = self.state.graph_widget.selected_entity_changed() {
            self.state.selected_entity_id = Some(entity_id);
        }
        
        // Top toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            toolbar(ui, &mut self.state);
        });
        
        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.panels.layout {
                LayoutMode::FourPanel => self.render_four_panel(ui),
                LayoutMode::EditorFocus => self.render_editor_focus(ui),
                LayoutMode::GraphFocus => self.render_graph_focus(ui),
            }
        });
        
        // Request repaint if async operations are in progress
        let needs_repaint = {
            let async_state = self.state.async_state.lock().unwrap();
            async_state.loading_session 
                || async_state.loading_graph 
                || async_state.loading_chat
                || async_state.executing
        };
        
        if needs_repaint {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl App {
    fn render_four_panel(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let half_width = available.x / 2.0;
        let half_height = available.y / 2.0;
        
        // Use egui's layout system for 2x2 grid
        ui.horizontal(|ui| {
            // Left column
            ui.vertical(|ui| {
                ui.set_max_width(half_width);
                ui.set_max_height(half_height);
                
                egui::Frame::default()
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        chat_panel(ui, &mut self.state);
                    });
            });
            
            // Right column  
            ui.vertical(|ui| {
                ui.set_max_width(half_width);
                ui.set_max_height(half_height);
                
                egui::Frame::default()
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        dsl_editor_panel(ui, &mut self.state);
                    });
            });
        });
        
        ui.horizontal(|ui| {
            // Graph (bottom left)
            ui.vertical(|ui| {
                ui.set_max_width(half_width);
                
                egui::Frame::default()
                    .inner_margin(0.0)
                    .show(ui, |ui| {
                        self.state.graph_widget.ui(ui);
                    });
            });
            
            // Results/Entity detail (bottom right)
            ui.vertical(|ui| {
                ui.set_max_width(half_width);
                
                // Tab bar for results vs entity detail
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.state.panels.show_results, "Results").clicked() {
                        self.state.panels.show_results = true;
                        self.state.panels.show_entity_detail = false;
                    }
                    if ui.selectable_label(self.state.panels.show_entity_detail, "Entity").clicked() {
                        self.state.panels.show_results = false;
                        self.state.panels.show_entity_detail = true;
                    }
                });
                
                ui.separator();
                
                egui::Frame::default()
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        if self.state.panels.show_results {
                            results_panel(ui, &mut self.state);
                        } else {
                            entity_detail_panel(ui, &mut self.state);
                        }
                    });
            });
        });
    }
    
    fn render_editor_focus(&mut self, ui: &mut egui::Ui) {
        // Large editor on left, small panels stacked on right
        ui.horizontal(|ui| {
            // Editor (70% width)
            ui.vertical(|ui| {
                ui.set_max_width(ui.available_width() * 0.7);
                dsl_editor_panel(ui, &mut self.state);
            });
            
            // Stacked panels on right
            ui.vertical(|ui| {
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    chat_panel(ui, &mut self.state);
                });
                ui.separator();
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    results_panel(ui, &mut self.state);
                });
            });
        });
    }
    
    fn render_graph_focus(&mut self, ui: &mut egui::Ui) {
        // Large graph on left, small panels stacked on right
        ui.horizontal(|ui| {
            // Graph (70% width)
            ui.vertical(|ui| {
                ui.set_max_width(ui.available_width() * 0.7);
                self.state.graph_widget.ui(ui);
            });
            
            // Stacked panels on right
            ui.vertical(|ui| {
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    entity_detail_panel(ui, &mut self.state);
                });
                ui.separator();
                egui::Frame::default().inner_margin(8.0).show(ui, |ui| {
                    dsl_editor_panel(ui, &mut self.state);
                });
            });
        });
    }
}
```

---

## API Client

```rust
// src/api.rs

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response, Headers};
use serde::{de::DeserializeOwned, Serialize};
use ob_poc_types::*;

const BASE_URL: &str = "";  // Same origin

pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{}{}", BASE_URL, path);
    
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::SameOrigin);
    
    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request creation failed: {:?}", e))?;
    
    request.headers()
        .set("Accept", "application/json")
        .map_err(|e| format!("Header set failed: {:?}", e))?;
    
    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;
    
    let resp: Response = resp_value.dyn_into()
        .map_err(|_| "Response cast failed")?;
    
    if !resp.ok() {
        return Err(format!("HTTP {}: {}", resp.status(), resp.status_text()));
    }
    
    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON parse failed: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON await failed: {:?}", e))?;
    
    serde_wasm_bindgen::from_value(json)
        .map_err(|e| format!("Deserialize failed: {:?}", e))
}

pub async fn post<T: DeserializeOwned, B: Serialize>(path: &str, body: &B) -> Result<T, String> {
    let url = format!("{}{}", BASE_URL, path);
    
    let body_str = serde_json::to_string(body)
        .map_err(|e| format!("Serialize failed: {:?}", e))?;
    
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::SameOrigin);
    opts.set_body(&JsValue::from_str(&body_str));
    
    let headers = Headers::new().map_err(|e| format!("Headers creation failed: {:?}", e))?;
    headers.set("Content-Type", "application/json").ok();
    headers.set("Accept", "application/json").ok();
    opts.set_headers(&headers);
    
    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request creation failed: {:?}", e))?;
    
    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;
    
    let resp: Response = resp_value.dyn_into()
        .map_err(|_| "Response cast failed")?;
    
    if !resp.ok() {
        return Err(format!("HTTP {}: {}", resp.status(), resp.status_text()));
    }
    
    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON parse failed: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON await failed: {:?}", e))?;
    
    serde_wasm_bindgen::from_value(json)
        .map_err(|e| format!("Deserialize failed: {:?}", e))
}

// Convenience functions
pub async fn get_session(session_id: Uuid) -> Result<SessionStateResponse, String> {
    get(&format!("/api/session/{}", session_id)).await
}

pub async fn create_session() -> Result<CreateSessionResponse, String> {
    post("/api/session", &CreateSessionRequest {}).await
}

pub async fn bind_entity(session_id: Uuid, entity_id: Uuid, entity_type: &str) -> Result<BindEntityResponse, String> {
    post(
        &format!("/api/session/{}/bind", session_id),
        &BindEntityRequest {
            entity_id: entity_id.to_string(),
            entity_type: entity_type.to_string(),
        }
    ).await
}

pub async fn validate_dsl(dsl: &str) -> Result<ValidationResponse, String> {
    post("/api/agent/validate", &ValidateRequest { dsl: dsl.to_string() }).await
}

pub async fn execute_dsl(session_id: Option<Uuid>, dsl: &str) -> Result<ExecuteResponse, String> {
    let path = match session_id {
        Some(id) => format!("/api/session/{}/execute", id),
        None => "/api/agent/execute".to_string(),
    };
    post(&path, &ExecuteRequest { dsl: dsl.to_string() }).await
}

pub async fn get_cbu_graph(cbu_id: Uuid, view_mode: ViewMode) -> Result<CbuGraphResponse, String> {
    get(&format!("/api/cbu/{}/graph?view_mode={}", cbu_id, view_mode.as_str())).await
}

pub async fn list_cbus() -> Result<Vec<CbuSummary>, String> {
    get("/api/cbu/list").await
}
```

---

## WASM Entry Point

```rust
// src/lib.rs

mod api;
mod app;
mod panels;
mod state;
mod theme;
mod widgets;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    
    tracing::info!("OB-POC UI starting");
    Ok(())
}

#[wasm_bindgen]
pub fn start_app(canvas_id: &str) -> Result<(), JsValue> {
    let web_options = eframe::WebOptions::default();
    
    let document = web_sys::window()
        .ok_or_else(|| JsValue::from_str("No window"))?
        .document()
        .ok_or_else(|| JsValue::from_str("No document"))?;
    
    let canvas: web_sys::HtmlCanvasElement = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("Canvas '{}' not found", canvas_id)))?
        .dyn_into()
        .map_err(|_| JsValue::from_str("Element is not a canvas"))?;
    
    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });
    
    Ok(())
}
```

---

## Server-Side Changes

### Minimal HTML Shell

Replace the complex HTML/TypeScript with a simple shell:

```html
<!-- rust/crates/ob-poc-web/static/index.html -->
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OB-POC</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        html, body { width: 100%; height: 100%; overflow: hidden; }
        canvas { width: 100%; height: 100%; display: block; }
    </style>
</head>
<body>
    <canvas id="app"></canvas>
    <script type="module">
        import init, { start_app } from '/pkg/ob_poc_ui.js';
        
        async function run() {
            await init();
            start_app('app');
        }
        
        run();
    </script>
</body>
</html>
```

### Server Route Changes

```rust
// rust/crates/ob-poc-web/src/routes.rs

// Remove all HTML template routes
// Keep only:
// - Static file serving (for WASM bundle)
// - API routes (unchanged)

pub fn routes() -> Router {
    Router::new()
        // Static files (WASM bundle, index.html)
        .nest_service("/pkg", ServeDir::new("pkg"))
        .route("/", get(|| async { 
            axum::response::Html(include_str!("../static/index.html"))
        }))
        // API routes (keep all existing)
        .nest("/api", api_routes())
}
```

---

## Migration Steps

### Phase 1: Setup (Day 1)

1. Create `rust/crates/ob-poc-ui/` crate structure
2. Copy API client from `ob-poc-graph/src/api.rs`
3. Setup basic `App` struct with state
4. Verify WASM builds and loads

### Phase 2: Core Panels (Days 2-3)

1. Implement toolbar with CBU selector
2. Implement chat panel (display only, then input)
3. Implement DSL editor (basic, then syntax highlighting)
4. Implement results panel

### Phase 3: Integration (Days 4-5)

1. Wire up async state management
2. Implement SSE for chat streaming
3. Connect graph widget (embed existing `ob-poc-graph`)
4. Implement entity detail panel

### Phase 4: Polish (Day 6)

1. Layout modes (4-panel, editor focus, graph focus)
2. Keyboard shortcuts
3. Session persistence (localStorage)
4. Error handling UI

### Phase 5: Cleanup (Day 7)

1. Remove `ob-poc-web` TypeScript/HTML
2. Update documentation
3. Update deployment scripts

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_transitions() {
        let mut state = AppState::default();
        
        // Verify initial state
        assert!(state.session.is_none());
        assert!(state.buffers.dsl_editor.is_empty());
        
        // Simulate session load
        state.session = Some(SessionStateResponse {
            session_id: Uuid::new_v4().to_string(),
            messages: vec![],
            active_cbu: None,
            bindings: Default::default(),
            pending_dsl: Some("(cbu.ensure :name \"Test\")".to_string()),
        });
        
        // Verify DSL sync
        state.process_async_results();
        // DSL should be synced from server when not dirty
    }
}
```

### Integration Tests

Use `wasm-pack test` for browser-based integration tests:

```rust
#[wasm_bindgen_test]
async fn test_api_client() {
    // Mock server or use test server
    let result = api::get::<Vec<CbuSummary>>("/api/cbu/list").await;
    assert!(result.is_ok());
}
```

---

## Key Files to Create

```
rust/crates/ob-poc-ui/
├── Cargo.toml
├── src/
│   ├── lib.rs              # WASM entry, start_app()
│   ├── app.rs              # Main App, update loop, layout rendering
│   ├── state.rs            # AppState, AsyncState, TextBuffers
│   ├── api.rs              # HTTP client (get, post, SSE)
│   ├── theme.rs            # Colors, fonts, spacing constants
│   ├── panels/
│   │   ├── mod.rs          # pub use all panels
│   │   ├── toolbar.rs      # Top toolbar
│   │   ├── chat.rs         # Chat panel
│   │   ├── dsl_editor.rs   # DSL editor
│   │   ├── results.rs      # Execution results
│   │   └── entity_detail.rs # Entity details
│   └── widgets/
│       ├── mod.rs          # pub use all widgets
│       ├── message.rs      # Chat message renderer
│       ├── status_badge.rs # KYC/risk badges
│       ├── entity_search.rs # Entity autocomplete
│       └── code_editor.rs  # Syntax highlighted editor
```

---

## Success Criteria

1. **Single language**: All UI code is Rust
2. **Single build**: `cargo build -p ob-poc-ui` produces working WASM
3. **Zero TS**: No TypeScript files in the repository
4. **Server-first**: All state fetched from server, no local business logic
5. **Parity**: All existing functionality preserved
6. **Performance**: 60fps for graph navigation, <100ms for panel interactions

---

## Reference: Existing Patterns to Preserve

### From ob-poc-graph (KEEP)

- `AsyncGraphState` pattern for async coordination
- `CbuGraphWidget` immediate mode rendering
- Camera interpolation for smooth pan/zoom
- Focus card rendering
- Input handling (drag, click, keyboard)

### From ob-poc-types (KEEP)

- All API request/response types
- Session state types
- Graph data types

### From ob-poc-web (MIGRATE, THEN DELETE)

- API routes (keep server-side)
- Session management logic (keep server-side)
- HTML templates (delete)
- TypeScript panels (delete)
- CSS styles (delete - use egui theming)

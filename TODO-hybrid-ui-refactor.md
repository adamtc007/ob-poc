# Hybrid UI Architecture Refactor

## Overview

Refactor ob-poc-ui from monolithic egui/WASM to hybrid architecture:
- **Keep in egui/WASM**: CBU graph visualization (canvas-based, interactive)
- **Migrate to HTML/TypeScript**: Chat, DSL, AST panels (text-based, debuggable)

### Why This Architecture
1. Text rendering belongs in HTML - 30 years of browser optimization
2. Debugging HTML/TS is browser DevTools; debugging WASM is pain
3. Chat/DSL/AST are 80% of iteration time - now fully debuggable
4. Graph visualization is where egui earns its complexity

### Target Layout
```
┌─────────────────────────────────────────────────────────────┐
│  index.html (served by Axum)                                │
│  ┌─────────────────────────┐  ┌──────────────────────────┐  │
│  │ #cbu-panel              │  │ #chat-panel              │  │
│  │ <canvas>                │  │ HTML/TypeScript          │  │
│  │ egui/WASM               │  │ SSE streaming            │  │
│  └─────────────────────────┘  └──────────────────────────┘  │
│  ┌─────────────────────────┐  ┌──────────────────────────┐  │
│  │ #dsl-panel              │  │ #ast-panel               │  │
│  │ HTML/TypeScript         │  │ HTML/TypeScript          │  │
│  │ Syntax highlighting     │  │ Tree view                │  │
│  └─────────────────────────┘  └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Axum Web Server Foundation

### 1.1 Create web server crate
```
rust/crates/ob-poc-web/
├── Cargo.toml
├── src/
│   ├── main.rs          # Server entry point
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── api.rs       # REST endpoints
│   │   ├── chat.rs      # SSE streaming for agent chat
│   │   └── static.rs    # Serve static files
│   └── state.rs         # Shared app state
```

### 1.2 Cargo.toml dependencies
```toml
[dependencies]
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### 1.3 Basic Axum routes
- `GET /` → serve index.html
- `GET /static/*` → serve static files (JS, CSS, WASM)
- `GET /api/cbu/:id` → CBU data for graph
- `GET /api/cbu/:id/dsl` → DSL source for panel
- `GET /api/cbu/:id/ast` → AST structure
- `GET /api/chat/stream` → SSE endpoint for agent responses
- `POST /api/chat` → Submit chat message
- `POST /api/dsl/execute` → Execute DSL

### 1.4 CORS configuration for dev
```rust
use tower_http::cors::CorsLayer;
// Dev: permissive, Prod: restrict to origin
```

---

## Phase 2: Static HTML Shell

### 2.1 Create web assets directory
```
rust/crates/ob-poc-web/
├── static/
│   ├── index.html       # Main shell page
│   ├── styles/
│   │   └── main.css     # Panel layout, chat styling
│   ├── js/
│   │   └── app.ts       # TypeScript source
│   └── wasm/            # egui WASM output copied here
```

### 2.2 index.html structure
```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>OB-POC</title>
    <link rel="stylesheet" href="/static/styles/main.css">
</head>
<body>
    <div id="app-container">
        <!-- Top row -->
        <div id="cbu-panel" class="panel">
            <canvas id="egui-canvas"></canvas>
        </div>
        <div id="chat-panel" class="panel">
            <div id="chat-messages"></div>
            <div id="chat-input-area">
                <textarea id="chat-input" placeholder="Ask the agent..."></textarea>
                <button id="chat-send">Send</button>
            </div>
        </div>
        
        <!-- Bottom row -->
        <div id="dsl-panel" class="panel">
            <pre><code id="dsl-source" class="language-rust"></code></pre>
        </div>
        <div id="ast-panel" class="panel">
            <div id="ast-tree"></div>
        </div>
    </div>
    
    <!-- WASM for graph only -->
    <script type="module">
        import init, { start_graph } from '/static/wasm/ob_poc_graph.js';
        await init();
        start_graph(document.getElementById('egui-canvas'));
    </script>
    
    <!-- HTML panels logic -->
    <script type="module" src="/static/js/app.js"></script>
</body>
</html>
```

### 2.3 CSS grid layout
```css
#app-container {
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
    height: 100vh;
    gap: 4px;
}

.panel {
    overflow: auto;
    border: 1px solid #333;
    background: #1e1e1e;
}

#cbu-panel { grid-area: 1 / 1; }
#chat-panel { grid-area: 1 / 2; }
#dsl-panel { grid-area: 2 / 1; }
#ast-panel { grid-area: 2 / 2; }
```

---

## Phase 3: TypeScript Panel Implementation

### 3.1 Setup TypeScript build
```
rust/crates/ob-poc-web/
├── package.json
├── tsconfig.json
├── static/
│   └── ts/
│       ├── app.ts           # Entry point
│       ├── chat.ts          # Chat panel logic
│       ├── dsl.ts           # DSL panel logic
│       ├── ast.ts           # AST panel logic
│       ├── bridge.ts        # WASM ↔ HTML communication
│       └── types.ts         # Shared types
```

### 3.2 package.json (minimal)
```json
{
  "name": "ob-poc-web",
  "scripts": {
    "build": "esbuild static/ts/app.ts --bundle --outfile=static/js/app.js --format=esm",
    "watch": "esbuild static/ts/app.ts --bundle --outfile=static/js/app.js --format=esm --watch"
  },
  "devDependencies": {
    "esbuild": "^0.20",
    "typescript": "^5"
  }
}
```

### 3.3 Chat panel (SSE streaming)
```typescript
// chat.ts
export class ChatPanel {
    private messages: HTMLElement;
    private input: HTMLTextAreaElement;
    private currentStream?: EventSource;
    
    async sendMessage(text: string) {
        this.appendMessage('user', text);
        
        const response = await fetch('/api/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message: text, cbu_id: this.currentCbuId })
        });
        
        const { stream_id } = await response.json();
        this.streamResponse(stream_id);
    }
    
    private streamResponse(streamId: string) {
        const msgEl = this.appendMessage('assistant', '');
        const events = new EventSource(`/api/chat/stream?id=${streamId}`);
        
        events.onmessage = (e) => {
            const data = JSON.parse(e.data);
            if (data.type === 'chunk') {
                msgEl.textContent += data.content;
            } else if (data.type === 'done') {
                events.close();
            }
        };
    }
}
```

### 3.4 DSL panel (syntax highlighting)
```typescript
// dsl.ts
import Prism from 'prismjs'; // Or highlight.js

export class DslPanel {
    private codeEl: HTMLElement;
    
    setSource(dsl: string) {
        this.codeEl.textContent = dsl;
        Prism.highlightElement(this.codeEl);
    }
    
    highlightLine(line: number) {
        // Add highlight class to specific line
    }
}
```

### 3.5 AST panel (tree view)
```typescript
// ast.ts
export class AstPanel {
    private treeEl: HTMLElement;
    
    setAst(ast: AstNode) {
        this.treeEl.innerHTML = this.renderNode(ast);
    }
    
    private renderNode(node: AstNode, depth = 0): string {
        const children = node.children?.map(c => this.renderNode(c, depth + 1)).join('') ?? '';
        return `
            <div class="ast-node" style="margin-left: ${depth * 16}px" data-id="${node.id}">
                <span class="ast-type">${node.type}</span>
                <span class="ast-name">${node.name ?? ''}</span>
                ${children}
            </div>
        `;
    }
    
    highlightNode(id: string) {
        this.treeEl.querySelectorAll('.ast-node').forEach(n => n.classList.remove('highlight'));
        this.treeEl.querySelector(`[data-id="${id}"]`)?.classList.add('highlight');
    }
}
```

### 3.6 WASM ↔ HTML bridge
```typescript
// bridge.ts
export class WasmBridge {
    private canvas: HTMLCanvasElement;
    
    constructor(canvas: HTMLCanvasElement) {
        this.canvas = canvas;
        
        // Listen for events from WASM
        window.addEventListener('egui-entity-selected', (e: CustomEvent) => {
            this.onEntitySelected(e.detail.id);
        });
    }
    
    // Called when HTML panels select an entity
    focusEntity(id: string) {
        this.canvas.dispatchEvent(new CustomEvent('focus-entity', { detail: { id } }));
    }
    
    // Called when WASM selects an entity
    onEntitySelected: (id: string) => void = () => {};
}
```

---

## Phase 4: Refactor egui Crate (Graph Only)

### 4.1 Rename and restructure
```
rust/crates/ob-poc-graph/     # Renamed from ob-poc-ui
├── Cargo.toml
├── src/
│   ├── lib.rs               # WASM entry point
│   ├── app.rs               # Graph-only app
│   ├── bridge.rs            # JS interop (CustomEvents)
│   └── graph/               # Keep entire graph/ directory
│       ├── camera.rs
│       ├── colors.rs
│       ├── edges.rs
│       ├── focus_card.rs
│       ├── input.rs
│       ├── layout.rs
│       ├── lod.rs
│       ├── mod.rs
│       ├── render.rs
│       └── types.rs
```

### 4.2 Remove from egui crate
- `panels/chat_panel.rs` → DELETE (moving to TypeScript)
- `panels/dsl_panel.rs` → DELETE (moving to TypeScript)
- `panels/ast_panel.rs` → DELETE (moving to TypeScript)
- `agent_panel.rs` → DELETE
- `panels/mod.rs` → DELETE (no panels remain)
- Remove panel-related state from `state/` module

### 4.3 Simplify app.rs
```rust
// New app.rs - graph only
pub struct GraphApp {
    graph_widget: CbuGraphWidget,
    api: ApiClient,
    current_cbu: Option<Uuid>,
    js_bridge: JsBridge,
}

impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for focus requests from JS
        if let Some(entity_id) = self.js_bridge.poll_focus_request() {
            self.graph_widget.focus_entity(entity_id);
        }
        
        // Render graph only - full canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            self.graph_widget.show(ui);
        });
        
        // Notify JS of selection changes
        if let Some(selected) = self.graph_widget.selected_entity_changed() {
            self.js_bridge.emit_entity_selected(selected);
        }
    }
}
```

### 4.4 JS bridge (WASM side)
```rust
// bridge.rs
use wasm_bindgen::prelude::*;
use web_sys::{CustomEvent, CustomEventInit, Window};

pub struct JsBridge {
    window: Window,
    pending_focus: Option<String>,
}

impl JsBridge {
    pub fn new() -> Self {
        let window = web_sys::window().unwrap();
        let bridge = Self { window, pending_focus: None };
        bridge.setup_listener();
        bridge
    }
    
    fn setup_listener(&self) {
        // Listen for focus-entity events from JS
        let closure = Closure::wrap(Box::new(|e: CustomEvent| {
            // Store in static or channel for polling
        }) as Box<dyn FnMut(_)>);
        
        self.window
            .add_event_listener_with_callback("focus-entity", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
    
    pub fn emit_entity_selected(&self, id: &str) {
        let mut init = CustomEventInit::new();
        init.detail(&JsValue::from_str(id));
        
        let event = CustomEvent::new_with_event_init_dict("egui-entity-selected", &init).unwrap();
        self.window.dispatch_event(&event).unwrap();
    }
    
    pub fn poll_focus_request(&mut self) -> Option<String> {
        self.pending_focus.take()
    }
}
```

### 4.5 WASM build output
Configure wasm-pack to output to `ob-poc-web/static/wasm/`:
```toml
# Cargo.toml
[package.metadata.wasm-pack.profile.release]
wasm-opt = ["-O3"]
```

Build script:
```bash
#!/bin/bash
cd rust/crates/ob-poc-graph
wasm-pack build --target web --out-dir ../ob-poc-web/static/wasm
```

---

## Phase 5: API Endpoints

### 5.1 Chat SSE endpoint
```rust
// routes/chat.rs
use axum::{
    response::sse::{Event, Sse},
    extract::{Query, State},
};
use tokio_stream::StreamExt;

pub async fn chat_stream(
    Query(params): Query<ChatStreamParams>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = state.agent
        .stream_response(params.id)
        .map(|chunk| {
            Ok(Event::default()
                .event("message")
                .data(serde_json::to_string(&chunk).unwrap()))
        });
    
    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::default())
}
```

### 5.2 DSL endpoints
```rust
// routes/api.rs
pub async fn get_dsl(
    Path(cbu_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Json<DslResponse> {
    let dsl = state.dsl_service.get_source(cbu_id).await;
    Json(DslResponse { source: dsl })
}

pub async fn execute_dsl(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    let result = state.dsl_service.execute(&req.source).await;
    Json(result)
}
```

### 5.3 AST endpoint
```rust
pub async fn get_ast(
    Path(cbu_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Json<AstResponse> {
    let ast = state.dsl_service.parse_to_ast(cbu_id).await;
    Json(AstResponse { root: ast })
}
```

---

## Phase 6: Integration & Testing

### 6.1 Development workflow
```bash
# Terminal 1: Rust server (auto-reload with cargo-watch)
cd rust/crates/ob-poc-web
cargo watch -x run

# Terminal 2: TypeScript (watch mode)
npm run watch

# Terminal 3: WASM rebuild (when graph changes)
cd rust/crates/ob-poc-graph
wasm-pack build --target web --out-dir ../ob-poc-web/static/wasm
```

### 6.2 Test cross-panel communication
1. Click entity in graph (WASM) → should highlight in DSL + AST panels (HTML)
2. Click AST node (HTML) → should focus in graph (WASM)
3. Ask agent about entity (HTML) → should highlight relevant code

### 6.3 Debug checklist
- [ ] Chat: Open DevTools Network tab, see SSE frames streaming
- [ ] DSL: Inspect element, verify syntax highlighting classes
- [ ] AST: Console.log node clicks, verify event propagation
- [ ] Graph: `tracing-wasm` output in console for WASM debugging

---

## Migration Checklist

### Delete from ob-poc-ui
- [ ] `src/panels/chat_panel.rs`
- [ ] `src/panels/dsl_panel.rs`
- [ ] `src/panels/ast_panel.rs`
- [ ] `src/panels/mod.rs`
- [ ] `src/agent_panel.rs`
- [ ] Chat-related state structs
- [ ] DSL panel state structs
- [ ] AST panel state structs

### Keep in ob-poc-graph (renamed)
- [ ] `src/graph/*` (entire directory)
- [ ] `src/api.rs` (for fetching graph data)
- [ ] Graph-related state only

### New in ob-poc-web
- [ ] Axum server with routes
- [ ] index.html shell
- [ ] TypeScript panel implementations
- [ ] CSS layout
- [ ] WASM ↔ HTML bridge

### Verify functionality preserved
- [ ] CBU graph renders with full interactivity
- [ ] Agent chat works with streaming responses
- [ ] DSL source displays with syntax highlighting
- [ ] AST tree renders and is navigable
- [ ] Cross-panel selection sync works

---

## Notes for Claude Code

1. **Incremental approach**: Complete each phase before moving to next
2. **Test at boundaries**: After each phase, verify the new code works standalone
3. **Preserve graph code**: The `graph/` directory is battle-tested - don't refactor it
4. **Minimal TypeScript**: Keep TS simple - no frameworks, just DOM manipulation
5. **Use existing API patterns**: The current `api.rs` patterns should inform the Axum endpoints

## File References

Current egui crate location: `rust/crates/ob-poc-ui/`
Current panels to migrate:
- `rust/crates/ob-poc-ui/src/panels/chat_panel.rs`
- `rust/crates/ob-poc-ui/src/panels/dsl_panel.rs`
- `rust/crates/ob-poc-ui/src/panels/ast_panel.rs`

Graph code to preserve:
- `rust/crates/ob-poc-ui/src/graph/` (entire directory)

# Task: Agent Session API for DSL Generation

## Implementation Status: COMPLETE

**Implemented on:** 2025-11-26  
**Build Status:** Compiles successfully with 2 warnings (unused import, unused function)

### Files Created:
- `rust/src/api/session.rs` (217 lines) - Session state management
- `rust/static/index.html` (459 lines) - Web UI

### Files Modified:
- `rust/src/api/agent_routes.rs` (789 lines) - Added session endpoints
- `rust/src/api/mod.rs` - Added module exports
- `rust/Cargo.toml` - Added `fs` feature to tower-http
- `rust/src/bin/agentic_server.rs` - Added static file serving

### API Endpoints Implemented:
- `POST /api/session` - Create new session
- `GET /api/session/:id` - Get session state  
- `POST /api/session/:id/chat` - Chat and generate DSL
- `POST /api/session/:id/execute` - Execute accumulated DSL
- `POST /api/session/:id/clear` - Clear accumulated DSL
- `DELETE /api/session/:id` - Delete session

### Build Warnings (non-blocking):
```
warning: unused import: `db_loading::*`
   --> src/forth_engine/schema/cache.rs:504:9

warning: function `extract_domain_from_verb` is never used
   --> src/api/agent_routes.rs:732:4
```

---

## Objective

Create a stateful agent session API that connects a web UI to the Rust DSL infrastructure for intelligent onboarding request generation.

## Current State

### What EXISTS and WORKS:
- `rust/src/dsl_source/agentic/llm_generator.rs` - LLM-powered DSL generation with RAG
- `rust/src/dsl_source/agentic/rag_context.rs` - Vocabulary/examples/attributes context
- `rust/src/forth_engine/runtime.rs` - DSL execution runtime
- `rust/src/forth_engine/parser_nom.rs` - S-expression parser
- `rust/src/forth_engine/vocab_registry.rs` - 53 verbs across 8 domains
- `rust/src/api/agent_routes.rs` - Basic generate/validate endpoints
- `rust/src/bin/agentic_server.rs` - Axum server binary

### What DOES NOT EXIST:
- Stateful agent sessions (conversation memory)
- WebSocket/SSE streaming for real-time responses
- Session-scoped DSL accumulation (build up DSL across turns)
- Web UI connected to the real backend

### What is OLD/DISCONNECTED (ignore):
- `web-interface/` - Old mock implementations, not connected to DSL engine

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    WEB UI (New)                              │
│  ┌──────────────┐  ┌─────────────┐  ┌───────────────────┐   │
│  │ Chat Input   │  │ DSL Preview │  │ Execute Button    │   │
│  └──────────────┘  └─────────────┘  └───────────────────┘   │
└─────────────────────────────┬───────────────────────────────┘
                              │ HTTP + SSE
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 AGENT SESSION API (New)                      │
│  POST /api/session          → Create new session             │
│  POST /api/session/{id}/chat → Send message, get DSL        │
│  GET  /api/session/{id}     → Get session state              │
│  POST /api/session/{id}/execute → Execute accumulated DSL   │
│  DELETE /api/session/{id}   → End session                    │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│                 EXISTING INFRASTRUCTURE                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ LlmDslGen   │  │ RagContext  │  │ Runtime + Parser    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Add Session State Management

Create `rust/src/api/session.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub messages: Vec<ChatMessage>,
    pub accumulated_dsl: Vec<String>,  // DSL statements built up over conversation
    pub context: SessionContext,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub dsl: Option<String>,  // Generated DSL if any
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Agent,
    System,
}

#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    pub cbu_id: Option<Uuid>,
    pub entity_ids: Vec<Uuid>,
    pub domain_hint: Option<String>,
}

pub type SessionStore = Arc<RwLock<HashMap<Uuid, AgentSession>>>;

pub fn create_session_store() -> SessionStore {
    Arc::new(RwLock::new(HashMap::new()))
}
```

### Step 2: Add Session Endpoints

Update `rust/src/api/agent_routes.rs` to add:

```rust
// New request/response types
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub domain_hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub session_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub auto_validate: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub message: String,
    pub dsl: Option<String>,
    pub validation: Option<ValidationResult>,
    pub accumulated_dsl: Vec<String>,
    pub can_execute: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionStateResponse {
    pub session_id: Uuid,
    pub message_count: usize,
    pub accumulated_dsl: Vec<String>,
    pub combined_dsl: String,  // All DSL joined
    pub context: SessionContextResponse,
}

#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    pub success: bool,
    pub results: Vec<ExecutionResult>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionResult {
    pub dsl_statement: String,
    pub success: bool,
    pub message: String,
    pub entity_id: Option<Uuid>,
}

// New endpoints
async fn create_session(...) -> Result<Json<CreateSessionResponse>, StatusCode>
async fn chat_in_session(...) -> Result<Json<ChatResponse>, StatusCode>
async fn get_session_state(...) -> Result<Json<SessionStateResponse>, StatusCode>
async fn execute_session_dsl(...) -> Result<Json<ExecuteResponse>, StatusCode>
async fn delete_session(...) -> Result<StatusCode, StatusCode>
```

### Step 3: Update Router

Add new routes to `create_agent_router`:

```rust
pub fn create_agent_router(pool: PgPool) -> Router {
    let state = AgentState::new(pool);
    let sessions = create_session_store();
    
    Router::new()
        // Existing
        .route("/api/agent/generate", post(generate_dsl))
        .route("/api/agent/validate", post(validate_dsl))
        .route("/api/agent/domains", get(list_domains))
        .route("/api/agent/vocabulary", get(get_vocabulary))
        .route("/api/agent/health", get(agent_health))
        // New session endpoints
        .route("/api/session", post(create_session))
        .route("/api/session/:id", get(get_session_state))
        .route("/api/session/:id", delete(delete_session))
        .route("/api/session/:id/chat", post(chat_in_session))
        .route("/api/session/:id/execute", post(execute_session_dsl))
        .with_state((state, sessions))
}
```

### Step 4: Implement Chat Logic

The `chat_in_session` handler should:

1. Get session from store
2. Add user message to history
3. Build context from session (previous DSL, entities created, etc.)
4. Call `LlmDslGenerator::generate()` with context
5. Validate generated DSL using `NomDslParser` and `Runtime`
6. If valid, add to `accumulated_dsl`
7. Add agent response to history
8. Return response with DSL and validation

```rust
async fn chat_in_session(
    State((agent_state, sessions)): State<(AgentState, SessionStore)>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    // Get or create generator
    let generator = LlmDslGenerator::from_env_with_runtime(
        agent_state.rag_provider.clone(),
        agent_state.runtime.clone(),
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Get session
    let mut sessions_write = sessions.write().await;
    let session = sessions_write.get_mut(&session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    // Add user message
    session.messages.push(ChatMessage {
        role: MessageRole::User,
        content: req.message.clone(),
        dsl: None,
        timestamp: Utc::now(),
    });
    
    // Generate DSL
    let domain = session.context.domain_hint.as_deref();
    let result = generator.generate(&req.message, "CREATE", domain).await;
    
    match result {
        Ok(generated) => {
            // Validate
            let validation = validate_dsl_internal(&generated.dsl_text, &agent_state.runtime);
            
            // If valid, accumulate
            if validation.valid {
                session.accumulated_dsl.push(generated.dsl_text.clone());
            }
            
            // Add agent message
            session.messages.push(ChatMessage {
                role: MessageRole::Agent,
                content: generated.reasoning,
                dsl: Some(generated.dsl_text.clone()),
                timestamp: Utc::now(),
            });
            
            Ok(Json(ChatResponse {
                message: generated.reasoning,
                dsl: Some(generated.dsl_text),
                validation: Some(validation.clone()),
                accumulated_dsl: session.accumulated_dsl.clone(),
                can_execute: validation.valid && !session.accumulated_dsl.is_empty(),
            }))
        }
        Err(e) => {
            session.messages.push(ChatMessage {
                role: MessageRole::Agent,
                content: format!("Failed to generate DSL: {}", e),
                dsl: None,
                timestamp: Utc::now(),
            });
            
            Ok(Json(ChatResponse {
                message: format!("Generation failed: {}", e),
                dsl: None,
                validation: None,
                accumulated_dsl: session.accumulated_dsl.clone(),
                can_execute: false,
            }))
        }
    }
}
```

### Step 5: Simple Web UI

Create `rust/static/index.html` - a single-file HTML/JS UI:

```html
<!DOCTYPE html>
<html>
<head>
    <title>OB-POC Agent</title>
    <style>
        /* Minimal CSS */
        body { font-family: system-ui; max-width: 1200px; margin: 0 auto; padding: 20px; }
        .container { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; }
        .chat { border: 1px solid #ccc; padding: 10px; height: 400px; overflow-y: auto; }
        .dsl-preview { background: #1e1e1e; color: #d4d4d4; padding: 10px; height: 400px; overflow-y: auto; font-family: monospace; }
        .input-area { display: flex; gap: 10px; margin-top: 10px; }
        .input-area input { flex: 1; padding: 10px; }
        .input-area button { padding: 10px 20px; }
        .message { margin: 10px 0; padding: 10px; border-radius: 8px; }
        .user { background: #e3f2fd; }
        .agent { background: #f5f5f5; }
        .dsl-block { background: #2d2d2d; padding: 5px; margin: 5px 0; }
        .valid { border-left: 3px solid #4caf50; }
        .invalid { border-left: 3px solid #f44336; }
        .execute-btn { background: #4caf50; color: white; border: none; cursor: pointer; }
        .execute-btn:disabled { background: #ccc; }
    </style>
</head>
<body>
    <h1>🤖 OB-POC Agent Session</h1>
    <div id="session-info">No session</div>
    <button onclick="createSession()">New Session</button>
    
    <div class="container">
        <div>
            <h3>Chat</h3>
            <div class="chat" id="chat"></div>
            <div class="input-area">
                <input type="text" id="message" placeholder="Describe what you want to create..." onkeypress="if(event.key==='Enter')sendMessage()">
                <button onclick="sendMessage()">Send</button>
            </div>
        </div>
        <div>
            <h3>Accumulated DSL</h3>
            <div class="dsl-preview" id="dsl-preview"></div>
            <button class="execute-btn" id="execute-btn" disabled onclick="executeDsl()">Execute All DSL</button>
        </div>
    </div>

    <script>
        let sessionId = null;
        const API = 'http://localhost:3000';
        
        async function createSession() {
            const res = await fetch(`${API}/api/session`, {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({})
            });
            const data = await res.json();
            sessionId = data.session_id;
            document.getElementById('session-info').textContent = `Session: ${sessionId}`;
            document.getElementById('chat').innerHTML = '';
            document.getElementById('dsl-preview').innerHTML = '';
        }
        
        async function sendMessage() {
            if (!sessionId) { alert('Create a session first'); return; }
            const input = document.getElementById('message');
            const message = input.value.trim();
            if (!message) return;
            
            // Show user message
            addMessage('user', message);
            input.value = '';
            
            // Send to API
            const res = await fetch(`${API}/api/session/${sessionId}/chat`, {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ message, auto_validate: true })
            });
            const data = await res.json();
            
            // Show agent response
            let content = data.message;
            if (data.dsl) {
                const valid = data.validation?.valid ? 'valid' : 'invalid';
                content += `<div class="dsl-block ${valid}"><pre>${escapeHtml(data.dsl)}</pre></div>`;
            }
            addMessage('agent', content);
            
            // Update DSL preview
            updateDslPreview(data.accumulated_dsl);
            
            // Update execute button
            document.getElementById('execute-btn').disabled = !data.can_execute;
        }
        
        async function executeDsl() {
            if (!sessionId) return;
            const res = await fetch(`${API}/api/session/${sessionId}/execute`, {
                method: 'POST',
                headers: {'Content-Type': 'application/json'}
            });
            const data = await res.json();
            
            if (data.success) {
                addMessage('agent', `✅ Executed ${data.results.length} DSL statements successfully`);
            } else {
                addMessage('agent', `❌ Execution failed: ${data.errors.join(', ')}`);
            }
        }
        
        function addMessage(role, content) {
            const chat = document.getElementById('chat');
            const div = document.createElement('div');
            div.className = `message ${role}`;
            div.innerHTML = content;
            chat.appendChild(div);
            chat.scrollTop = chat.scrollHeight;
        }
        
        function updateDslPreview(dslList) {
            const preview = document.getElementById('dsl-preview');
            preview.innerHTML = dslList.map(d => `<div class="dsl-block">${escapeHtml(d)}</div>`).join('');
        }
        
        function escapeHtml(text) {
            return text.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
        }
    </script>
</body>
</html>
```

### Step 6: Serve Static Files

Update `agentic_server.rs` to serve the UI:

```rust
use tower_http::services::ServeDir;

// In main():
let app = create_agent_router(pool.clone())
    .merge(create_attribute_router(pool))
    .nest_service("/", ServeDir::new("static"))  // Serve static/index.html
    .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
    .layer(TraceLayer::new_for_http());
```

## Files to Create/Modify

### CREATE:
- `rust/src/api/session.rs` - Session state types and store
- `rust/static/index.html` - Simple web UI

### MODIFY:
- `rust/src/api/mod.rs` - Add `pub mod session;`
- `rust/src/api/agent_routes.rs` - Add session endpoints
- `rust/src/bin/agentic_server.rs` - Serve static files
- `rust/Cargo.toml` - Add `tower-http = { version = "0.5", features = ["cors", "trace", "fs"] }`

## Testing

1. Build and run:
```bash
cd rust
DATABASE_URL=postgresql://adamtc007@localhost:5432/ob-poc \
ANTHROPIC_API_KEY=your-key \
cargo run --bin agentic_server --features server
```

2. Open http://localhost:3000 in browser

3. Click "New Session"

4. Try: "Create a CBU for a hedge fund managing high net worth client assets"

5. Verify DSL appears in preview, validation passes

6. Try: "Add a proper person as the fund manager with role DIRECTOR"

7. Click "Execute All DSL" when ready

## Success Criteria

- [x] Session created with UUID
- [x] Chat messages send and receive
- [x] DSL generated from natural language
- [x] DSL validated against parser + runtime vocabulary
- [x] Valid DSL accumulates in session
- [x] DSL preview updates in real-time
- [x] Execute button enabled when valid DSL exists
- [x] Execute calls actual DSL runtime (validates and reports results)

## Notes

- Keep it simple - single HTML file, no build tools
- Session store is in-memory (resets on server restart) - fine for now
- Focus on the agent→DSL flow, not persistence
- LLM calls require ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable

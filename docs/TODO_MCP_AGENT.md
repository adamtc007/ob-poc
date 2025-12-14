# TODO: MCP Agent Integration Layer

**Purpose**: Thin MCP wrapper over existing DSL infrastructure  
**Effort**: ~4-6 hours  
**Dependencies**: All existing - ReplSession, planning_facade, EntityGateway, execute_with_dag

---

## Architecture

```
Agent (Claude) → MCP Tools → Existing Infrastructure
                    │
                    ├── dsl_validate    → analyse_and_plan()
                    ├── dsl_execute     → execute_with_dag()
                    ├── entity_search   → GatewayRefResolver::search_fuzzy()
                    └── session_context → ReplSession
```

**No new business logic.** MCP tools are format adapters only.

---

## Files to Create

```
rust/src/mcp/
├── mod.rs              # Module exports
├── server.rs           # MCP JSON-RPC server
├── types.rs            # Agent-friendly response types
└── tools/
    ├── mod.rs          # Tool registration
    ├── validate.rs     # dsl_validate tool
    ├── execute.rs      # dsl_execute tool
    ├── search.rs       # entity_search tool
    └── session.rs      # session_context tool
```

---

## Step 1: Create `rust/src/mcp/mod.rs`

```rust
//! MCP (Model Context Protocol) server for agent integration
//! 
//! Thin wrappers over existing DSL infrastructure:
//! - ReplSession for conversation state
//! - planning_facade for validation
//! - execute_with_dag for execution
//! - EntityGateway for search

pub mod server;
pub mod tools;
pub mod types;

pub use server::McpServer;
pub use types::*;
```

---

## Step 2: Create `rust/src/mcp/types.rs`

Agent-friendly response types. Transform internal types to JSON-serializable structs.

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// dsl_validate types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ValidationOutput {
    pub valid: bool,
    pub diagnostics: Vec<AgentDiagnostic>,
    pub plan_summary: Option<String>,
    pub suggested_fixes: Vec<SuggestedFix>,
}

#[derive(Debug, Serialize)]
pub struct AgentDiagnostic {
    pub severity: String,           // "error", "warning", "hint"
    pub message: String,
    pub location: Option<Location>,
    pub code: String,               // E0001, W0002, etc.
    pub resolution_options: Vec<ResolutionOption>,
}

#[derive(Debug, Serialize)]
pub struct Location {
    pub line: u32,
    pub column: u32,
    pub length: u32,
}

#[derive(Debug, Serialize)]
pub struct ResolutionOption {
    pub description: String,
    pub action: String,             // "replace", "insert_before", "delete"
    pub replacement: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SuggestedFix {
    pub description: String,
    pub dsl: String,                // The DSL to insert/replace
    pub insert_at: Option<u32>,     // Line number
}

// ============================================================================
// dsl_execute types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ExecutionOutput {
    pub success: bool,
    pub results: Vec<StepResultSummary>,
    pub bindings: HashMap<String, String>,  // name → uuid
    pub error: Option<String>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct StepResultSummary {
    pub verb: String,
    pub action: String,             // "created", "updated", "linked", "deleted"
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub entity_display: String,
    pub binding: Option<String>,
}

// ============================================================================
// entity_search types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub matches: Vec<EntityMatch>,
    pub exact_match: Option<EntityMatch>,
    pub ambiguous: bool,
    pub disambiguation_prompt: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EntityMatch {
    pub id: String,
    pub display: String,
    pub entity_type: String,
    pub score: f32,
    pub context: HashMap<String, String>,  // Additional info for disambiguation
}

// ============================================================================
// session_context types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SessionAction {
    Create,
    Get { session_id: String },
    Update { session_id: String, bindings: HashMap<String, String> },
    Undo { session_id: String },
    Clear { session_id: String },
}

#[derive(Debug, Serialize)]
pub struct SessionState {
    pub session_id: String,
    pub bindings: HashMap<String, BindingInfo>,
    pub history_count: usize,
    pub can_undo: bool,
}

#[derive(Debug, Serialize)]
pub struct BindingInfo {
    pub name: String,
    pub uuid: String,
    pub entity_type: String,
}
```

---

## Step 3: Create `rust/src/mcp/tools/mod.rs`

```rust
pub mod validate;
pub mod execute;
pub mod search;
pub mod session;

pub use validate::dsl_validate;
pub use execute::dsl_execute;
pub use search::entity_search;
pub use session::session_context;
```

---

## Step 4: Create `rust/src/mcp/tools/validate.rs`

Wraps `analyse_and_plan()` from planning_facade.

```rust
use crate::dsl_v2::planning_facade::{analyse_and_plan, PlanningInput, ImplicitCreateMode};
use crate::dsl_v2::validation::{Diagnostic, DiagnosticCode, Severity};
use crate::dsl_v2::dag::describe_plan;
use crate::mcp::types::*;
use crate::mcp::tools::session::get_session;

pub async fn dsl_validate(
    source: &str,
    session_id: Option<&str>,
) -> Result<ValidationOutput, String> {
    // Get binding context from session if provided
    let binding_context = session_id
        .and_then(|id| get_session(id))
        .map(|s| s.binding_context());
    
    // Use existing planning facade
    let output = analyse_and_plan(PlanningInput {
        source,
        executed_bindings: binding_context.as_ref(),
        strict_semantics: false,
        implicit_create_mode: ImplicitCreateMode::Enabled,
    }).await;
    
    // Transform to agent-friendly format
    let has_errors = output.diagnostics.iter().any(|d| d.severity == Severity::Error);
    
    Ok(ValidationOutput {
        valid: !has_errors,
        diagnostics: output.diagnostics.into_iter().map(to_agent_diagnostic).collect(),
        plan_summary: output.plan.as_ref().map(|p| describe_plan(p)),
        suggested_fixes: output.synthetic_steps.into_iter().map(to_suggested_fix).collect(),
    })
}

fn to_agent_diagnostic(d: Diagnostic) -> AgentDiagnostic {
    AgentDiagnostic {
        severity: match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Hint => "hint",
            Severity::Info => "info",
        }.to_string(),
        message: d.message,
        location: d.span.map(|s| Location {
            line: s.line,
            column: s.column,
            length: s.length,
        }),
        code: format!("{:?}", d.code),
        resolution_options: build_resolution_options(&d),
    }
}

fn build_resolution_options(d: &Diagnostic) -> Vec<ResolutionOption> {
    let mut options = Vec::new();
    
    // Convert suggestions to resolution options
    for suggestion in &d.suggestions {
        options.push(ResolutionOption {
            description: suggestion.clone(),
            action: "replace".to_string(),
            replacement: Some(suggestion.clone()),
        });
    }
    
    // Add specific options based on error code
    match d.code {
        DiagnosticCode::UndefinedSymbol => {
            options.push(ResolutionOption {
                description: "Create entity before use".to_string(),
                action: "insert_before".to_string(),
                replacement: None,  // Agent should use suggested_fixes
            });
        }
        DiagnosticCode::InvalidValue => {
            options.push(ResolutionOption {
                description: "Search for similar entities".to_string(),
                action: "search".to_string(),
                replacement: None,
            });
        }
        _ => {}
    }
    
    options
}

fn to_suggested_fix(step: crate::dsl_v2::synthetic_steps::SyntheticStep) -> SuggestedFix {
    SuggestedFix {
        description: format!("Create {} '{}'", step.entity_type, step.binding),
        dsl: step.suggested_dsl,
        insert_at: Some(step.insert_before_line),
    }
}
```

---

## Step 5: Create `rust/src/mcp/tools/execute.rs`

Wraps `execute_with_dag()` from executor.

```rust
use crate::dsl_v2::executor::DslExecutor;
use crate::dsl_v2::validation::ValidationContext;
use crate::dsl_v2::execution_result::StepResult;
use crate::mcp::types::*;
use crate::mcp::tools::session::{get_session, get_session_mut};
use std::collections::HashMap;

pub async fn dsl_execute(
    executor: &DslExecutor,
    source: &str,
    session_id: Option<&str>,
    dry_run: bool,
) -> Result<ExecutionOutput, String> {
    // Build context from session
    let known_symbols = session_id
        .and_then(|id| get_session(id))
        .map(|s| s.all_bindings())
        .unwrap_or_default();
    
    let context = ValidationContext {
        known_symbols,
        ..Default::default()
    };
    
    // Use existing DAG executor
    let result = executor
        .execute_with_dag(source, &context, dry_run)
        .await
        .map_err(|e| format!("{:?}", e))?;
    
    // Update session with new bindings
    if !dry_run {
        if let Some(session) = session_id.and_then(|id| get_session_mut(id)) {
            for (name, uuid) in &result.bindings_created {
                session.record_binding(name.clone(), *uuid);
            }
        }
    }
    
    // Transform to agent-friendly format
    Ok(ExecutionOutput {
        success: result.errors.is_empty(),
        results: result.step_results.iter().map(|(_, r)| to_step_summary(r)).collect(),
        bindings: result.bindings_created.iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect(),
        error: result.errors.first().map(|(_, e)| e.clone()),
        summary: build_summary(&result),
    })
}

fn to_step_summary(result: &StepResult) -> StepResultSummary {
    match result {
        StepResult::Created { pk, entity_type } => StepResultSummary {
            verb: format!("{}.create", entity_type),
            action: "created".to_string(),
            entity_type: entity_type.clone(),
            entity_id: Some(pk.to_string()),
            entity_display: format!("{} ({})", entity_type, pk),
            binding: None,
        },
        StepResult::Updated { pk, entity_type } => StepResultSummary {
            verb: format!("{}.update", entity_type),
            action: "updated".to_string(),
            entity_type: entity_type.clone(),
            entity_id: Some(pk.to_string()),
            entity_display: format!("{} ({})", entity_type, pk),
            binding: None,
        },
        StepResult::NoOp => StepResultSummary {
            verb: "no-op".to_string(),
            action: "skipped".to_string(),
            entity_type: "".to_string(),
            entity_id: None,
            entity_display: "No changes".to_string(),
            binding: None,
        },
        // ... handle other variants
        _ => StepResultSummary {
            verb: "unknown".to_string(),
            action: "unknown".to_string(),
            entity_type: "".to_string(),
            entity_id: None,
            entity_display: format!("{:?}", result),
            binding: None,
        },
    }
}

fn build_summary(result: &crate::dsl_v2::execution_result::ExecutionResults) -> String {
    let created = result.step_results.iter()
        .filter(|(_, r)| matches!(r, StepResult::Created { .. }))
        .count();
    let updated = result.step_results.iter()
        .filter(|(_, r)| matches!(r, StepResult::Updated { .. }))
        .count();
    
    if result.errors.is_empty() {
        format!("Success: {} created, {} updated", created, updated)
    } else {
        format!("Failed: {}", result.errors[0].1)
    }
}
```

---

## Step 6: Create `rust/src/mcp/tools/search.rs`

Wraps `GatewayRefResolver::search_fuzzy()`.

```rust
use crate::dsl_v2::gateway_resolver::GatewayRefResolver;
use crate::dsl_v2::validation::RefType;
use crate::mcp::types::*;
use std::collections::HashMap;

pub async fn entity_search(
    gateway: &GatewayRefResolver,
    query: &str,
    entity_type: Option<&str>,
    limit: Option<u32>,
) -> Result<SearchOutput, String> {
    let ref_type = entity_type
        .map(str_to_ref_type)
        .unwrap_or(RefType::Entity);
    
    let limit = limit.unwrap_or(10) as usize;
    
    // Use existing fuzzy search
    let matches = gateway
        .search_fuzzy(ref_type, query, limit)
        .await
        .map_err(|e| e.to_string())?;
    
    let entity_matches: Vec<EntityMatch> = matches
        .iter()
        .map(|m| EntityMatch {
            id: m.token.clone(),
            display: m.display.clone(),
            entity_type: format!("{:?}", ref_type),
            score: m.score,
            context: HashMap::new(),  // Could enrich with additional lookups
        })
        .collect();
    
    // Determine if ambiguous (multiple high-scoring matches)
    let ambiguous = entity_matches.len() > 1 
        && entity_matches.get(0).map(|m| m.score).unwrap_or(0.0) 
         - entity_matches.get(1).map(|m| m.score).unwrap_or(0.0) < 0.1;
    
    let exact_match = entity_matches.iter()
        .find(|m| m.score > 0.95)
        .cloned();
    
    let disambiguation_prompt = if ambiguous {
        Some(build_disambiguation_prompt(&entity_matches))
    } else {
        None
    };
    
    Ok(SearchOutput {
        matches: entity_matches,
        exact_match,
        ambiguous,
        disambiguation_prompt,
    })
}

fn str_to_ref_type(s: &str) -> RefType {
    match s.to_lowercase().as_str() {
        "cbu" => RefType::Cbu,
        "entity" | "person" | "company" => RefType::Entity,
        "document" => RefType::Document,
        "jurisdiction" | "country" => RefType::Jurisdiction,
        "role" => RefType::Role,
        "product" => RefType::Product,
        "service" => RefType::Service,
        _ => RefType::Entity,
    }
}

fn build_disambiguation_prompt(matches: &[EntityMatch]) -> String {
    let options: Vec<String> = matches.iter()
        .take(5)
        .enumerate()
        .map(|(i, m)| format!("{}. {} ({})", i + 1, m.display, m.entity_type))
        .collect();
    
    format!("Multiple matches found. Which did you mean?\n{}", options.join("\n"))
}
```

---

## Step 7: Create `rust/src/mcp/tools/session.rs`

Wraps `ReplSession` with in-memory storage.

```rust
use crate::dsl_v2::repl_session::ReplSession;
use crate::mcp::types::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use uuid::Uuid;

// In-memory session store
static SESSIONS: Lazy<Arc<RwLock<HashMap<String, ReplSession>>>> = 
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub fn get_session(id: &str) -> Option<ReplSession> {
    SESSIONS.read().ok()?.get(id).cloned()
}

pub fn get_session_mut(id: &str) -> Option<impl std::ops::DerefMut<Target = ReplSession> + '_> {
    let sessions = SESSIONS.write().ok()?;
    if sessions.contains_key(id) {
        Some(std::sync::RwLockWriteGuard::map(sessions, |s| s.get_mut(id).unwrap()))
    } else {
        None
    }
}

pub async fn session_context(action: SessionAction) -> Result<SessionState, String> {
    match action {
        SessionAction::Create => {
            let session = ReplSession::new();
            let id = Uuid::new_v4().to_string();
            
            SESSIONS.write()
                .map_err(|e| e.to_string())?
                .insert(id.clone(), session);
            
            Ok(SessionState {
                session_id: id,
                bindings: HashMap::new(),
                history_count: 0,
                can_undo: false,
            })
        }
        
        SessionAction::Get { session_id } => {
            let sessions = SESSIONS.read().map_err(|e| e.to_string())?;
            let session = sessions.get(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            
            Ok(to_session_state(&session_id, session))
        }
        
        SessionAction::Update { session_id, bindings } => {
            let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
            let session = sessions.get_mut(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            
            for (name, uuid_str) in bindings {
                if let Ok(uuid) = Uuid::parse_str(&uuid_str) {
                    session.record_binding(name, uuid);
                }
            }
            
            Ok(to_session_state(&session_id, session))
        }
        
        SessionAction::Undo { session_id } => {
            let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
            let session = sessions.get_mut(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            
            session.undo();
            
            Ok(to_session_state(&session_id, session))
        }
        
        SessionAction::Clear { session_id } => {
            let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
            let session = sessions.get_mut(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            
            session.reset();
            
            Ok(to_session_state(&session_id, session))
        }
    }
}

fn to_session_state(id: &str, session: &ReplSession) -> SessionState {
    SessionState {
        session_id: id.to_string(),
        bindings: session.all_bindings().iter()
            .map(|(name, uuid)| (name.clone(), BindingInfo {
                name: name.clone(),
                uuid: uuid.to_string(),
                entity_type: session.binding_type(name).unwrap_or_default(),
            }))
            .collect(),
        history_count: session.history_len(),
        can_undo: session.can_undo(),
    }
}
```

---

## Step 8: Create `rust/src/mcp/server.rs`

MCP JSON-RPC server. Can start simple with stdio transport.

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::mcp::tools;

#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Option<Value>,
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

pub struct McpServer {
    executor: Arc<DslExecutor>,
    gateway: Arc<GatewayRefResolver>,
}

impl McpServer {
    pub fn new(executor: Arc<DslExecutor>, gateway: Arc<GatewayRefResolver>) -> Self {
        Self { executor, gateway }
    }
    
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        let result = match request.method.as_str() {
            "tools/list" => self.list_tools(),
            "tools/call" => self.call_tool(request.params).await,
            _ => Err(format!("Unknown method: {}", request.method)),
        };
        
        match result {
            Ok(value) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(value),
                error: None,
            },
            Err(e) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(McpError { code: -1, message: e }),
            },
        }
    }
    
    fn list_tools(&self) -> Result<Value, String> {
        Ok(json!({
            "tools": [
                {
                    "name": "dsl_validate",
                    "description": "Validate DSL source code without executing. Returns structured errors and suggested fixes.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "source": { "type": "string", "description": "DSL source code" },
                            "session_id": { "type": "string", "description": "Optional session ID for binding context" }
                        },
                        "required": ["source"]
                    }
                },
                {
                    "name": "dsl_execute",
                    "description": "Execute validated DSL. Use dry_run=true to preview without executing.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "source": { "type": "string", "description": "DSL source code" },
                            "session_id": { "type": "string", "description": "Optional session ID" },
                            "dry_run": { "type": "boolean", "description": "If true, show plan without executing" }
                        },
                        "required": ["source"]
                    }
                },
                {
                    "name": "entity_search",
                    "description": "Search for existing entities by name. Use to resolve references before generating DSL.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query" },
                            "entity_type": { "type": "string", "description": "Filter by type: cbu, entity, document, etc." },
                            "limit": { "type": "integer", "description": "Max results (default 10)" }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "session_context",
                    "description": "Manage conversation session state. Create session at start, tracks bindings across DSL executions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": { 
                                "type": "string", 
                                "enum": ["create", "get", "update", "undo", "clear"],
                                "description": "Session action"
                            },
                            "session_id": { "type": "string", "description": "Session ID (required for get/update/undo/clear)" },
                            "bindings": { "type": "object", "description": "For update: name → uuid mappings" }
                        },
                        "required": ["action"]
                    }
                }
            ]
        }))
    }
    
    async fn call_tool(&self, params: Option<Value>) -> Result<Value, String> {
        let params = params.ok_or("Missing params")?;
        let tool_name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing tool name")?;
        let arguments = params.get("arguments")
            .cloned()
            .unwrap_or(json!({}));
        
        match tool_name {
            "dsl_validate" => {
                let source = arguments.get("source")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing source")?;
                let session_id = arguments.get("session_id")
                    .and_then(|v| v.as_str());
                
                let result = tools::dsl_validate(source, session_id).await?;
                Ok(serde_json::to_value(result).unwrap())
            }
            
            "dsl_execute" => {
                let source = arguments.get("source")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing source")?;
                let session_id = arguments.get("session_id")
                    .and_then(|v| v.as_str());
                let dry_run = arguments.get("dry_run")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                
                let result = tools::dsl_execute(&self.executor, source, session_id, dry_run).await?;
                Ok(serde_json::to_value(result).unwrap())
            }
            
            "entity_search" => {
                let query = arguments.get("query")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing query")?;
                let entity_type = arguments.get("entity_type")
                    .and_then(|v| v.as_str());
                let limit = arguments.get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
                
                let result = tools::entity_search(&self.gateway, query, entity_type, limit).await?;
                Ok(serde_json::to_value(result).unwrap())
            }
            
            "session_context" => {
                let action: tools::session::SessionAction = 
                    serde_json::from_value(arguments).map_err(|e| e.to_string())?;
                
                let result = tools::session_context(action).await?;
                Ok(serde_json::to_value(result).unwrap())
            }
            
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
```

---

## Step 9: Add to `rust/src/lib.rs`

```rust
pub mod mcp;
```

---

## Step 10: Create Binary Entry Point (Optional)

If you want a standalone MCP server binary:

```rust
// rust/src/bin/mcp_server.rs
use std::io::{BufRead, Write};
use ob_poc::mcp::server::McpServer;

#[tokio::main]
async fn main() {
    // Initialize executor, gateway, etc.
    let server = McpServer::new(executor, gateway);
    
    // Simple stdio transport
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    
    for line in stdin.lock().lines() {
        let line = line.expect("Failed to read line");
        if line.is_empty() { continue; }
        
        let request: McpRequest = serde_json::from_str(&line)
            .expect("Invalid JSON-RPC request");
        
        let response = server.handle_request(request).await;
        
        let response_json = serde_json::to_string(&response).unwrap();
        writeln!(stdout, "{}", response_json).unwrap();
        stdout.flush().unwrap();
    }
}
```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_validate_returns_structured_errors() {
        let result = dsl_validate(
            r#"(cbu.create :name "Test" :jurisdiction "INVALID")"#,
            None
        ).await.unwrap();
        
        assert!(!result.valid);
        assert!(!result.diagnostics.is_empty());
        assert!(result.diagnostics[0].resolution_options.len() > 0);
    }
    
    #[tokio::test]
    async fn test_session_persists_bindings() {
        // Create session
        let state = session_context(SessionAction::Create).await.unwrap();
        let id = state.session_id;
        
        // Execute DSL that creates binding
        dsl_execute(&executor, 
            r#"(cbu.create :name "Test" :jurisdiction "US" :as @test)"#,
            Some(&id),
            false
        ).await.unwrap();
        
        // Verify binding persisted
        let state = session_context(SessionAction::Get { session_id: id.clone() }).await.unwrap();
        assert!(state.bindings.contains_key("test"));
    }
    
    #[tokio::test]
    async fn test_search_returns_disambiguation() {
        // Assuming "John Smith" has multiple matches
        let result = entity_search(&gateway, "John Smith", Some("entity"), None).await.unwrap();
        
        if result.matches.len() > 1 {
            assert!(result.ambiguous || result.exact_match.is_some());
        }
    }
}
```

---

## Verification Checklist

- [ ] `dsl_validate` returns structured diagnostics with resolution options
- [ ] `dsl_execute` updates session bindings on success
- [ ] `dsl_execute` with `dry_run=true` returns plan without executing
- [ ] `entity_search` returns disambiguation prompt when ambiguous
- [ ] `session_context` create/get/update/undo/clear all work
- [ ] Session bindings persist across multiple `dsl_execute` calls
- [ ] Undo reverts last execution block

---

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
once_cell = "1.19"
# serde, serde_json already present
```

---

## Notes

- All tools are **thin wrappers** - no new business logic
- Session storage is in-memory (add Redis/DB persistence later if needed)
- MCP server uses stdio transport (simplest) - can add HTTP/WebSocket later
- Error recovery logic lives in the agent prompt, not in Rust code

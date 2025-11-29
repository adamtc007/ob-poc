# DSL MCP Server Implementation

**Goal**: Expose the DSL pipeline as an MCP (Model Context Protocol) server so Claude can directly validate, execute, and query DSL operations without human middleware.

**Prerequisites**:
- CLI working with database execution ✅
- All 34 tests passing ✅

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Claude Agent                              │
│  "Onboard Acme Corp with Jane (60%) and Bob (40%) as UBOs"      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ MCP Protocol (JSON-RPC over stdio)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      MCP Server (Rust)                           │
├─────────────────────────────────────────────────────────────────┤
│  Tools:                                                          │
│  ├── dsl_validate    - Parse + CSG lint                         │
│  ├── dsl_execute     - Full execution to DB                     │
│  ├── dsl_plan        - Show execution plan                      │
│  ├── cbu_get         - Get CBU with all related data            │
│  ├── cbu_list        - List/search CBUs                         │
│  ├── entity_get      - Get entity details                       │
│  ├── verbs_list      - List available DSL verbs                 │
│  └── schema_info     - Get entity types, roles, doc types       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     DSL Pipeline + PostgreSQL                    │
└─────────────────────────────────────────────────────────────────┘
```

---

## MCP Protocol Basics

MCP uses JSON-RPC 2.0 over stdio. The server:
1. Reads JSON-RPC requests from stdin
2. Processes them
3. Writes JSON-RPC responses to stdout
4. Logs to stderr (not stdout!)

### Lifecycle

```
Client                          Server
   │                               │
   │──── initialize ──────────────▶│
   │◀─── capabilities ─────────────│
   │                               │
   │──── tools/list ──────────────▶│
   │◀─── tool definitions ─────────│
   │                               │
   │──── tools/call ──────────────▶│
   │◀─── tool result ──────────────│
```

---

## Tool Definitions

### 1. dsl_validate

```json
{
  "name": "dsl_validate",
  "description": "Validate DSL source code. Parses and runs CSG linting.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "source": { "type": "string", "description": "DSL source code" },
      "client_type": { "type": "string", "enum": ["individual", "corporate", "trust", "fund"] },
      "jurisdiction": { "type": "string", "description": "ISO 2-letter code" }
    },
    "required": ["source"]
  }
}
```

### 2. dsl_execute

```json
{
  "name": "dsl_execute",
  "description": "Execute DSL against the database.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "source": { "type": "string", "description": "DSL source code" },
      "dry_run": { "type": "boolean", "default": false }
    },
    "required": ["source"]
  }
}
```

### 3. dsl_plan

```json
{
  "name": "dsl_plan",
  "description": "Show execution plan without running.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "source": { "type": "string" }
    },
    "required": ["source"]
  }
}
```

### 4. cbu_get

```json
{
  "name": "cbu_get",
  "description": "Get CBU with all related entities, roles, documents, screenings.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "cbu_id": { "type": "string", "description": "UUID" }
    },
    "required": ["cbu_id"]
  }
}
```

### 5. cbu_list

```json
{
  "name": "cbu_list",
  "description": "List CBUs with filtering.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "status": { "type": "string", "enum": ["active", "pending", "closed", "deleted"] },
      "client_type": { "type": "string" },
      "jurisdiction": { "type": "string" },
      "search": { "type": "string" },
      "limit": { "type": "integer", "default": 20 }
    }
  }
}
```

### 6. entity_get

```json
{
  "name": "entity_get",
  "description": "Get entity details with roles, documents, screenings.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_id": { "type": "string" }
    },
    "required": ["entity_id"]
  }
}
```

### 7. verbs_list

```json
{
  "name": "verbs_list",
  "description": "List available DSL verbs.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "domain": { "type": "string", "description": "Filter by domain" }
    }
  }
}
```

### 8. schema_info

```json
{
  "name": "schema_info",
  "description": "Get entity types, roles, document types.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "category": { "type": "string", "enum": ["entity_types", "roles", "document_types", "all"], "default": "all" }
    }
  }
}
```

---

## Implementation Files

### File Structure

```
rust/src/
├── mcp/
│   ├── mod.rs           # Module exports
│   ├── server.rs        # Main server loop
│   ├── protocol.rs      # JSON-RPC types
│   ├── tools.rs         # Tool definitions
│   └── handlers.rs      # Tool implementations
└── bin/
    └── dsl_mcp.rs       # Binary entry point
```

---

## Step 1: Protocol Types

Create `rust/src/mcp/protocol.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None, error: Some(JsonRpcError { code, message: message.into() }) }
    }
}

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;

#[derive(Debug, Serialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize)]
pub struct ToolsListResult {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Serialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl ToolCallResult {
    pub fn json(value: &Value) -> Self {
        Self {
            content: vec![ToolContent {
                content_type: "text".into(),
                text: serde_json::to_string_pretty(value).unwrap_or_default(),
            }],
            is_error: None,
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent { content_type: "text".into(), text: msg.into() }],
            is_error: Some(true),
        }
    }
}
```

---

## Step 2: Tool Definitions

Create `rust/src/mcp/tools.rs`:

```rust
use super::protocol::Tool;
use serde_json::json;

pub fn get_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "dsl_validate".into(),
            description: "Validate DSL source code.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "DSL source code"}
                },
                "required": ["source"]
            }),
        },
        Tool {
            name: "dsl_execute".into(),
            description: "Execute DSL against the database.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {"type": "string"},
                    "dry_run": {"type": "boolean", "default": false}
                },
                "required": ["source"]
            }),
        },
        Tool {
            name: "dsl_plan".into(),
            description: "Show execution plan without running.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"source": {"type": "string"}},
                "required": ["source"]
            }),
        },
        Tool {
            name: "cbu_get".into(),
            description: "Get CBU with entities, roles, documents, screenings.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"cbu_id": {"type": "string"}},
                "required": ["cbu_id"]
            }),
        },
        Tool {
            name: "cbu_list".into(),
            description: "List CBUs with filtering.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string"},
                    "client_type": {"type": "string"},
                    "search": {"type": "string"},
                    "limit": {"type": "integer", "default": 20}
                }
            }),
        },
        Tool {
            name: "entity_get".into(),
            description: "Get entity details.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"entity_id": {"type": "string"}},
                "required": ["entity_id"]
            }),
        },
        Tool {
            name: "verbs_list".into(),
            description: "List available DSL verbs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"domain": {"type": "string"}}
            }),
        },
        Tool {
            name: "schema_info".into(),
            description: "Get entity types, roles, document types.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "category": {"type": "string", "enum": ["entity_types", "roles", "document_types", "all"]}
                }
            }),
        },
    ]
}
```

---

## Step 3: Handlers

Create `rust/src/mcp/handlers.rs`:

```rust
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::dsl_v2::{parse_program, compile, DslExecutor, ExecutionContext, verb_registry::registry};
use super::protocol::ToolCallResult;

pub struct ToolHandlers {
    pool: PgPool,
}

impl ToolHandlers {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn handle(&self, name: &str, args: Value) -> ToolCallResult {
        match self.dispatch(name, args).await {
            Ok(v) => ToolCallResult::json(&v),
            Err(e) => ToolCallResult::error(e.to_string()),
        }
    }

    async fn dispatch(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "dsl_validate" => self.dsl_validate(args).await,
            "dsl_execute" => self.dsl_execute(args).await,
            "dsl_plan" => self.dsl_plan(args).await,
            "cbu_get" => self.cbu_get(args).await,
            "cbu_list" => self.cbu_list(args).await,
            "entity_get" => self.entity_get(args).await,
            "verbs_list" => self.verbs_list(args).await,
            "schema_info" => self.schema_info(args).await,
            _ => Err(anyhow!("Unknown tool: {}", name)),
        }
    }

    async fn dsl_validate(&self, args: Value) -> Result<Value> {
        let source = args["source"].as_str().ok_or_else(|| anyhow!("source required"))?;
        
        let ast = match parse_program(source) {
            Ok(ast) => ast,
            Err(e) => return Ok(json!({"valid": false, "errors": [{"message": format!("{:?}", e)}]})),
        };

        match compile(&ast) {
            Ok(plan) => Ok(json!({"valid": true, "step_count": plan.steps.len()})),
            Err(e) => Ok(json!({"valid": false, "errors": [{"message": format!("{:?}", e)}]})),
        }
    }

    async fn dsl_execute(&self, args: Value) -> Result<Value> {
        let source = args["source"].as_str().ok_or_else(|| anyhow!("source required"))?;
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);

        let ast = parse_program(source).map_err(|e| anyhow!("Parse error: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow!("Compile error: {:?}", e))?;

        if dry_run {
            let steps: Vec<_> = plan.steps.iter().enumerate()
                .map(|(i, s)| json!({"index": i, "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb), "binding": s.bind_as}))
                .collect();
            return Ok(json!({"success": true, "dry_run": true, "steps": steps}));
        }

        let executor = DslExecutor::new(self.pool.clone());
        let mut ctx = ExecutionContext::new();

        if let Err(e) = executor.execute(&plan, &mut ctx).await {
            return Ok(json!({"success": false, "error": e.to_string(), "completed": ctx.results().len()}));
        }

        let bindings: serde_json::Map<_, _> = ctx.bindings().iter()
            .map(|(k, v)| (k.clone(), json!(v.to_string()))).collect();

        Ok(json!({"success": true, "steps_executed": ctx.results().len(), "bindings": bindings}))
    }

    async fn dsl_plan(&self, args: Value) -> Result<Value> {
        let source = args["source"].as_str().ok_or_else(|| anyhow!("source required"))?;
        let ast = parse_program(source).map_err(|e| anyhow!("Parse: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow!("Compile: {:?}", e))?;

        let steps: Vec<_> = plan.steps.iter().enumerate()
            .map(|(i, s)| json!({"index": i, "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb), "binding": s.bind_as}))
            .collect();

        Ok(json!({"valid": true, "step_count": plan.steps.len(), "steps": steps}))
    }

    async fn cbu_get(&self, args: Value) -> Result<Value> {
        let cbu_id = Uuid::parse_str(args["cbu_id"].as_str().ok_or_else(|| anyhow!("cbu_id required"))?)?;

        let cbu = sqlx::query!(r#"SELECT cbu_id, name, jurisdiction, client_type, status FROM "ob-poc".cbus WHERE cbu_id = $1"#, cbu_id)
            .fetch_optional(&self.pool).await?.ok_or_else(|| anyhow!("CBU not found"))?;

        let entities = sqlx::query!(r#"SELECT e.entity_id, e.name, et.type_code as entity_type, e.status 
            FROM "ob-poc".entities e JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id 
            WHERE e.cbu_id = $1"#, cbu_id).fetch_all(&self.pool).await?;

        let roles = sqlx::query!(r#"SELECT er.entity_id, er.target_entity_id, r.role_code, er.ownership_percentage 
            FROM "ob-poc".entity_roles er JOIN "ob-poc".roles r ON er.role_id = r.role_id 
            WHERE er.cbu_id = $1"#, cbu_id).fetch_all(&self.pool).await?;

        let documents = sqlx::query!(r#"SELECT d.document_id, d.entity_id, dt.type_code, d.status 
            FROM "ob-poc".documents d JOIN "ob-poc".document_types dt ON d.document_type_id = dt.type_id 
            WHERE d.cbu_id = $1"#, cbu_id).fetch_all(&self.pool).await?;

        let screenings = sqlx::query!(r#"SELECT s.screening_id, s.entity_id, s.screening_type, s.status 
            FROM "ob-poc".screenings s JOIN "ob-poc".entities e ON s.entity_id = e.entity_id 
            WHERE e.cbu_id = $1"#, cbu_id).fetch_all(&self.pool).await?;

        Ok(json!({
            "cbu": {"cbu_id": cbu.cbu_id.to_string(), "name": cbu.name, "client_type": cbu.client_type, "jurisdiction": cbu.jurisdiction, "status": cbu.status},
            "entities": entities.iter().map(|e| json!({"entity_id": e.entity_id.to_string(), "name": e.name, "entity_type": e.entity_type, "status": e.status})).collect::<Vec<_>>(),
            "roles": roles.iter().map(|r| json!({"entity_id": r.entity_id.to_string(), "target_entity_id": r.target_entity_id.to_string(), "role": r.role_code, "ownership_percentage": r.ownership_percentage})).collect::<Vec<_>>(),
            "documents": documents.iter().map(|d| json!({"document_id": d.document_id.to_string(), "entity_id": d.entity_id.to_string(), "document_type": d.type_code, "status": d.status})).collect::<Vec<_>>(),
            "screenings": screenings.iter().map(|s| json!({"screening_id": s.screening_id.to_string(), "entity_id": s.entity_id.to_string(), "screening_type": s.screening_type, "status": s.status})).collect::<Vec<_>>(),
            "summary": {"entities": entities.len(), "documents": documents.len(), "screenings": screenings.len()}
        }))
    }

    async fn cbu_list(&self, args: Value) -> Result<Value> {
        let limit = args["limit"].as_i64().unwrap_or(20) as i32;
        let search = args["search"].as_str();

        let cbus = if let Some(s) = search {
            sqlx::query!(r#"SELECT cbu_id, name, client_type, jurisdiction, status FROM "ob-poc".cbus WHERE name ILIKE $1 LIMIT $2"#, 
                format!("%{}%", s), limit).fetch_all(&self.pool).await?
        } else {
            sqlx::query!(r#"SELECT cbu_id, name, client_type, jurisdiction, status FROM "ob-poc".cbus ORDER BY created_at DESC LIMIT $1"#, 
                limit).fetch_all(&self.pool).await?
        };

        Ok(json!({
            "cbus": cbus.iter().map(|c| json!({"cbu_id": c.cbu_id.to_string(), "name": c.name, "client_type": c.client_type, "jurisdiction": c.jurisdiction, "status": c.status})).collect::<Vec<_>>(),
            "total": cbus.len()
        }))
    }

    async fn entity_get(&self, args: Value) -> Result<Value> {
        let entity_id = Uuid::parse_str(args["entity_id"].as_str().ok_or_else(|| anyhow!("entity_id required"))?)?;

        let entity = sqlx::query!(r#"SELECT e.entity_id, e.cbu_id, e.name, e.status, e.attributes, et.type_code 
            FROM "ob-poc".entities e JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id 
            WHERE e.entity_id = $1"#, entity_id).fetch_optional(&self.pool).await?.ok_or_else(|| anyhow!("Entity not found"))?;

        let roles = sqlx::query!(r#"SELECT r.role_code, er.ownership_percentage, e2.name as target_name 
            FROM "ob-poc".entity_roles er JOIN "ob-poc".roles r ON er.role_id = r.role_id 
            JOIN "ob-poc".entities e2 ON er.target_entity_id = e2.entity_id WHERE er.entity_id = $1"#, entity_id)
            .fetch_all(&self.pool).await?;

        let documents = sqlx::query!(r#"SELECT dt.type_code, d.status FROM "ob-poc".documents d 
            JOIN "ob-poc".document_types dt ON d.document_type_id = dt.type_id WHERE d.entity_id = $1"#, entity_id)
            .fetch_all(&self.pool).await?;

        let screenings = sqlx::query!(r#"SELECT screening_type, status FROM "ob-poc".screenings WHERE entity_id = $1"#, entity_id)
            .fetch_all(&self.pool).await?;

        Ok(json!({
            "entity": {"entity_id": entity.entity_id.to_string(), "cbu_id": entity.cbu_id.to_string(), "name": entity.name, "entity_type": entity.type_code, "status": entity.status, "attributes": entity.attributes},
            "roles": roles.iter().map(|r| json!({"role": r.role_code, "target": r.target_name, "ownership": r.ownership_percentage})).collect::<Vec<_>>(),
            "documents": documents.iter().map(|d| json!({"type": d.type_code, "status": d.status})).collect::<Vec<_>>(),
            "screenings": screenings.iter().map(|s| json!({"type": s.screening_type, "status": s.status})).collect::<Vec<_>>()
        }))
    }

    async fn verbs_list(&self, args: Value) -> Result<Value> {
        let domain_filter = args["domain"].as_str();
        let reg = registry();

        let verbs: Vec<_> = reg.all_verbs()
            .filter(|v| domain_filter.map_or(true, |d| v.domain == d))
            .map(|v| json!({
                "verb": v.full_name(),
                "description": v.description,
                "args": v.args.iter().map(|a| json!({"name": a.name, "type": a.arg_type, "required": a.required})).collect::<Vec<_>>()
            }))
            .collect();

        Ok(json!({"domains": reg.domains(), "verbs": verbs}))
    }

    async fn schema_info(&self, args: Value) -> Result<Value> {
        let category = args["category"].as_str().unwrap_or("all");
        let mut result = json!({});

        if category == "all" || category == "entity_types" {
            let types = sqlx::query!(r#"SELECT type_code, name FROM "ob-poc".entity_types"#).fetch_all(&self.pool).await?;
            result["entity_types"] = json!(types.iter().map(|t| json!({"code": t.type_code, "name": t.name})).collect::<Vec<_>>());
        }
        if category == "all" || category == "roles" {
            let roles = sqlx::query!(r#"SELECT role_code, name FROM "ob-poc".roles"#).fetch_all(&self.pool).await?;
            result["roles"] = json!(roles.iter().map(|r| json!({"code": r.role_code, "name": r.name})).collect::<Vec<_>>());
        }
        if category == "all" || category == "document_types" {
            let docs = sqlx::query!(r#"SELECT type_code, name FROM "ob-poc".document_types"#).fetch_all(&self.pool).await?;
            result["document_types"] = json!(docs.iter().map(|d| json!({"code": d.type_code, "name": d.name})).collect::<Vec<_>>());
        }
        Ok(result)
    }
}
```

---

## Step 4: Server Loop

Create `rust/src/mcp/server.rs`:

```rust
use std::io::{BufRead, Write};
use serde_json::Value;
use sqlx::PgPool;

use super::protocol::*;
use super::tools::get_tools;
use super::handlers::ToolHandlers;

pub struct McpServer {
    handlers: ToolHandlers,
}

impl McpServer {
    pub fn new(pool: PgPool) -> Self {
        Self { handlers: ToolHandlers::new(pool) }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        eprintln!("[dsl_mcp] Server started");

        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() { continue; }

            eprintln!("[dsl_mcp] <- {}", &line[..line.len().min(100)]);

            let response = self.handle(&line).await;
            let out = serde_json::to_string(&response)?;
            
            eprintln!("[dsl_mcp] -> {}", &out[..out.len().min(100)]);
            writeln!(stdout, "{}", out)?;
            stdout.flush()?;
        }
        Ok(())
    }

    async fn handle(&self, msg: &str) -> JsonRpcResponse {
        let req: JsonRpcRequest = match serde_json::from_str(msg) {
            Ok(r) => r,
            Err(e) => return JsonRpcResponse::error(None, PARSE_ERROR, e.to_string()),
        };

        let id = req.id.clone();
        match req.method.as_str() {
            "initialize" => JsonRpcResponse::success(id, serde_json::to_value(InitializeResult {
                protocol_version: "2024-11-05".into(),
                capabilities: ServerCapabilities { tools: ToolsCapability { list_changed: false } },
                server_info: ServerInfo { name: "dsl-mcp".into(), version: env!("CARGO_PKG_VERSION").into() },
            }).unwrap()),

            "notifications/initialized" => JsonRpcResponse::success(id, Value::Null),

            "tools/list" => JsonRpcResponse::success(id, serde_json::to_value(ToolsListResult { tools: get_tools() }).unwrap()),

            "tools/call" => {
                let params: ToolCallParams = match serde_json::from_value(req.params) {
                    Ok(p) => p,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };
                let result = self.handlers.handle(&params.name, params.arguments).await;
                JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
            }

            _ => JsonRpcResponse::error(id, METHOD_NOT_FOUND, format!("Unknown: {}", req.method)),
        }
    }
}
```

---

## Step 5: Module and Binary

Create `rust/src/mcp/mod.rs`:

```rust
pub mod protocol;
pub mod tools;
pub mod handlers;
pub mod server;

pub use server::McpServer;
```

Create `rust/src/bin/dsl_mcp.rs`:

```rust
use anyhow::Result;
use ob_poc::dsl_v2::{create_pool, DbConfig};
use ob_poc::mcp::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    
    eprintln!("[dsl_mcp] Connecting...");
    let pool = create_pool(&DbConfig::from_url(&db_url)).await?;
    eprintln!("[dsl_mcp] Connected");

    McpServer::new(pool).run().await
}
```

Update `rust/src/lib.rs`:

```rust
#[cfg(feature = "mcp")]
pub mod mcp;
```

---

## Step 6: Cargo.toml

```toml
[[bin]]
name = "dsl_mcp"
path = "src/bin/dsl_mcp.rs"
required-features = ["mcp"]

[features]
mcp = ["database"]
```

---

## Testing

```bash
# Build
cargo build --features mcp --bin dsl_mcp

# Run manually
DATABASE_URL=postgresql://localhost/ob-poc ./target/debug/dsl_mcp

# Send test messages (in another terminal, pipe to the process)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' 
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"verbs_list","arguments":{}}}'
```

---

## Claude Desktop Config

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "dsl": {
      "command": "/path/to/ob-poc/rust/target/release/dsl_mcp",
      "env": {
        "DATABASE_URL": "postgresql://localhost/ob-poc"
      }
    }
  }
}
```

---

## Execution Checklist

- [ ] Create `rust/src/mcp/protocol.rs`
- [ ] Create `rust/src/mcp/tools.rs`
- [ ] Create `rust/src/mcp/handlers.rs`
- [ ] Create `rust/src/mcp/server.rs`
- [ ] Create `rust/src/mcp/mod.rs`
- [ ] Create `rust/src/bin/dsl_mcp.rs`
- [ ] Update `rust/src/lib.rs`
- [ ] Update `Cargo.toml`
- [ ] `cargo build --features mcp`
- [ ] Test with manual JSON-RPC
- [ ] Configure Claude Desktop
- [ ] Test end-to-end with Claude

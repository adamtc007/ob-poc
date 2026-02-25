//! MCP Server
//!
//! Main server loop handling JSON-RPC messages over stdio.

use std::io::{BufRead, Write};
use std::sync::Arc;

use serde_json::Value;
use sqlx::PgPool;

use super::handlers::ToolHandlers;
use super::protocol::*;
use super::resources_sem_reg;
use super::tools::get_tools;
use super::tools_sem_reg;
use crate::agent::learning::embedder::SharedEmbedder;
use crate::policy::ActorResolver;

use sem_os_client::SemOsClient;

/// MCP Server
pub struct McpServer {
    handlers: ToolHandlers,
    pool: PgPool,
    sem_os_client: Option<Arc<dyn SemOsClient>>,
}

impl McpServer {
    /// Create a new MCP server with database pool and embedder (REQUIRED)
    ///
    /// There is only ONE path - all MCP tools require the Candle embedder.
    pub fn new(pool: PgPool, embedder: SharedEmbedder) -> Self {
        Self {
            handlers: ToolHandlers::new(pool.clone(), embedder),
            pool,
            sem_os_client: None,
        }
    }

    /// Set the Semantic OS client for sem_reg resource reads and tool dispatch.
    pub fn with_sem_os_client(mut self, client: Arc<dyn SemOsClient>) -> Self {
        self.sem_os_client = Some(client.clone());
        self.handlers = self.handlers.with_sem_os_client(client);
        self
    }

    /// Run the server, reading from stdin and writing to stdout
    pub async fn run(&self) -> anyhow::Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        eprintln!("[dsl_mcp] Server started, waiting for messages...");

        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            // Log truncated message to stderr
            let preview = if line.len() > 100 {
                format!("{}...", &line[..100])
            } else {
                line.clone()
            };
            eprintln!("[dsl_mcp] <- {}", preview);

            let response = self.handle(&line).await;
            let out = serde_json::to_string(&response)?;

            // Log truncated response
            let preview = if out.len() > 100 {
                format!("{}...", &out[..100])
            } else {
                out.clone()
            };
            eprintln!("[dsl_mcp] -> {}", preview);

            writeln!(stdout, "{}", out)?;
            stdout.flush()?;
        }

        eprintln!("[dsl_mcp] Server shutting down");
        Ok(())
    }

    /// Handle a single JSON-RPC message
    async fn handle(&self, msg: &str) -> JsonRpcResponse {
        let req: JsonRpcRequest = match serde_json::from_str(msg) {
            Ok(r) => r,
            Err(e) => return JsonRpcResponse::error(None, PARSE_ERROR, e.to_string()),
        };

        let id = req.id.clone();

        match req.method.as_str() {
            "initialize" => {
                let result = InitializeResult {
                    protocol_version: "2024-11-05".into(),
                    capabilities: ServerCapabilities {
                        tools: ToolsCapability {
                            list_changed: false,
                        },
                        resources: Some(ResourcesCapability {
                            list_changed: false,
                        }),
                    },
                    server_info: ServerInfo {
                        name: "dsl-mcp".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                    },
                };
                match serde_json::to_value(result) {
                    Ok(v) => JsonRpcResponse::success(id, v),
                    Err(e) => JsonRpcResponse::error(
                        id,
                        INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    ),
                }
            }

            "notifications/initialized" => JsonRpcResponse::success(id, Value::Null),

            "resources/list" => {
                let result = ResourcesListResult {
                    resources: resources_sem_reg::static_resources(),
                    resource_templates: resources_sem_reg::resource_templates(),
                };
                match serde_json::to_value(result) {
                    Ok(v) => JsonRpcResponse::success(id, v),
                    Err(e) => JsonRpcResponse::error(
                        id,
                        INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    ),
                }
            }

            "resources/read" => {
                let params: ResourceReadParams = match serde_json::from_value(req.params) {
                    Ok(p) => p,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };

                eprintln!("[dsl_mcp] Reading resource: {}", params.uri);
                let actor = ActorResolver::from_env();
                let result = if let Some(ref client) = self.sem_os_client {
                    resources_sem_reg::read_resource_via_client(
                        &params.uri,
                        client.as_ref(),
                        &self.pool,
                        &actor,
                    )
                    .await
                } else {
                    resources_sem_reg::read_resource(&params.uri, &self.pool, &actor).await
                };
                match serde_json::to_value(result) {
                    Ok(v) => JsonRpcResponse::success(id, v),
                    Err(e) => JsonRpcResponse::error(
                        id,
                        INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    ),
                }
            }

            "tools/list" => {
                // Base tools (non-sem_reg) always come from get_tools() which
                // includes sem_reg tools via the direct path. When a SemOsClient
                // is available, replace the sem_reg portion with client-sourced specs.
                let tools = if let Some(ref client) = self.sem_os_client {
                    let mut base = get_tools();
                    // Remove directly-sourced sem_reg tools
                    base.retain(|t| !t.name.starts_with("sem_reg_"));
                    // Add client-sourced sem_reg tools
                    let client_tools =
                        tools_sem_reg::sem_reg_tools_via_client(client.as_ref()).await;
                    base.extend(client_tools);
                    base
                } else {
                    get_tools()
                };
                let result = ToolsListResult { tools };
                match serde_json::to_value(result) {
                    Ok(v) => JsonRpcResponse::success(id, v),
                    Err(e) => JsonRpcResponse::error(
                        id,
                        INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    ),
                }
            }

            "tools/call" => {
                let params: ToolCallParams = match serde_json::from_value(req.params) {
                    Ok(p) => p,
                    Err(e) => return JsonRpcResponse::error(id, INVALID_PARAMS, e.to_string()),
                };

                eprintln!("[dsl_mcp] Calling tool: {}", params.name);
                let result = self.handlers.handle(&params.name, params.arguments).await;
                match serde_json::to_value(result) {
                    Ok(v) => JsonRpcResponse::success(id, v),
                    Err(e) => JsonRpcResponse::error(
                        id,
                        INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    ),
                }
            }

            _ => JsonRpcResponse::error(
                id,
                METHOD_NOT_FOUND,
                format!("Unknown method: {}", req.method),
            ),
        }
    }
}

//! MCP Server
//!
//! Main server loop handling JSON-RPC messages over stdio.

use std::io::{BufRead, Write};

use serde_json::Value;
use sqlx::PgPool;

use super::handlers::ToolHandlers;
use super::protocol::*;
use super::tools::get_tools;

/// MCP Server
pub struct McpServer {
    handlers: ToolHandlers,
}

impl McpServer {
    /// Create a new MCP server with database pool
    pub fn new(pool: PgPool) -> Self {
        Self {
            handlers: ToolHandlers::new(pool),
        }
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

            "tools/list" => {
                let result = ToolsListResult { tools: get_tools() };
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

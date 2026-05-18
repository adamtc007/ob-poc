//! Server dispatch — translate JSON-RPC requests into tool
//! invocations.
//!
//! The dispatcher is transport-agnostic: it parses a line, decides
//! what to do, and returns serialised responses. Phase 4.2c wires
//! a stdio binary entry that loops over stdin lines and writes the
//! returns to stdout.
//!
//! ## Methods
//!
//! - `initialize` — capability handshake.
//! - `tools/list` — enumerate registered tools.
//! - `tools/invoke` — dispatch by `name`.
//!
//! Mutation is not exposed: any method outside the three above
//! returns `METHOD_NOT_FOUND`. Tools themselves are read-only by
//! construction (the MCP write path doesn't exist in this crate).

use serde_json::{json, Value};

use crate::protocol::{
    JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND,
};
use crate::tools::ToolRegistry;

pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "sem_os_mcp";
pub const SERVER_VERSION: &str = "0.1.0";

/// Server holding the tool registry. Construct once at startup.
pub struct McpServer {
    registry: ToolRegistry,
}

impl McpServer {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Handle a parsed JSON-RPC request and return the response.
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        match request.method.as_str() {
            "initialize" => JsonRpcResponse::success(id, self.initialize_result()),
            "tools/list" => JsonRpcResponse::success(
                id,
                json!({
                    "tools": self.registry.specs(),
                }),
            ),
            "tools/invoke" => self.invoke_tool(id, request.params).await,
            other => {
                JsonRpcResponse::error(id, METHOD_NOT_FOUND, format!("unknown method: {other}"))
            }
        }
    }

    fn initialize_result(&self) -> Value {
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION,
            },
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            }
        })
    }

    async fn invoke_tool(&self, id: Option<Value>, params: Value) -> JsonRpcResponse {
        let Some(name) = params.get("name").and_then(|v| v.as_str()) else {
            return JsonRpcResponse::error(
                id,
                INVALID_PARAMS,
                "tools/invoke requires `name` (string)",
            );
        };
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let Some(tool) = self.registry.get(name) else {
            return JsonRpcResponse::error(
                id,
                METHOD_NOT_FOUND,
                format!("tool '{name}' not registered"),
            );
        };

        match tool.invoke(arguments).await {
            Ok(result) => JsonRpcResponse::success(
                id,
                json!({
                    "tool": name,
                    "result": result,
                }),
            ),
            Err(error) => JsonRpcResponse::error_with_data(
                id,
                INTERNAL_ERROR,
                format!("tool '{name}' failed: {error}"),
                json!({
                    "tool": name,
                    "error_kind": error_kind(&error),
                }),
            ),
        }
    }
}

fn error_kind(error: &crate::tools::ToolError) -> &'static str {
    match error {
        crate::tools::ToolError::InvalidArguments(_) => "invalid_arguments",
        crate::tools::ToolError::Transport(_) => "transport",
        crate::tools::ToolError::Unsupported(_) => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{KnowledgeTool, ToolError, ToolSpec};
    use async_trait::async_trait;
    use std::sync::Arc;

    struct EchoTool;

    #[async_trait]
    impl KnowledgeTool for EchoTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: "echo".to_string(),
                description: "echo".to_string(),
                input_schema: json!({"type": "object"}),
            }
        }
        async fn invoke(&self, arguments: Value) -> Result<Value, ToolError> {
            Ok(json!({"echoed": arguments}))
        }
    }

    fn server_with_echo() -> McpServer {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));
        McpServer::new(registry)
    }

    fn request(id: i64, method: &str, params: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(id)),
            method: method.to_string(),
            params,
        }
    }

    #[tokio::test]
    async fn initialize_returns_capability_handshake() {
        let server = server_with_echo();
        let resp = server
            .handle_request(request(1, "initialize", json!({})))
            .await;
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[tokio::test]
    async fn tools_list_returns_registered_specs() {
        let server = server_with_echo();
        let resp = server
            .handle_request(request(1, "tools/list", json!({})))
            .await;
        let tools = &resp.result.unwrap()["tools"];
        assert_eq!(tools.as_array().unwrap().len(), 1);
        assert_eq!(tools[0]["name"], "echo");
    }

    #[tokio::test]
    async fn tools_invoke_dispatches_to_named_tool() {
        let server = server_with_echo();
        let resp = server
            .handle_request(request(
                1,
                "tools/invoke",
                json!({"name": "echo", "arguments": {"x": 42}}),
            ))
            .await;
        let result = resp.result.unwrap();
        assert_eq!(result["tool"], "echo");
        assert_eq!(result["result"]["echoed"]["x"], 42);
    }

    #[tokio::test]
    async fn tools_invoke_unknown_tool_returns_method_not_found() {
        let server = server_with_echo();
        let resp = server
            .handle_request(request(
                1,
                "tools/invoke",
                json!({"name": "nope", "arguments": {}}),
            ))
            .await;
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let server = server_with_echo();
        let resp = server
            .handle_request(request(1, "obpoc/mutation", json!({})))
            .await;
        let err = resp.error.unwrap();
        assert_eq!(err.code, METHOD_NOT_FOUND);
        assert!(err.message.contains("obpoc/mutation"));
    }

    #[tokio::test]
    async fn invoke_without_name_returns_invalid_params() {
        let server = server_with_echo();
        let resp = server
            .handle_request(request(1, "tools/invoke", json!({"arguments": {}})))
            .await;
        assert_eq!(resp.error.unwrap().code, INVALID_PARAMS);
    }
}

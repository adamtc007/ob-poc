//! MCP Protocol Types
//!
//! JSON-RPC 2.0 types for the Model Context Protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request
#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error
#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub(crate) fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub(crate) fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// Standard JSON-RPC error codes
pub(crate) const PARSE_ERROR: i32 = -32700;
pub(crate) const METHOD_NOT_FOUND: i32 = -32601;
pub(crate) const INVALID_PARAMS: i32 = -32602;
pub(crate) const INTERNAL_ERROR: i32 = -32603;

/// MCP initialize result
#[derive(Debug, Serialize)]
pub(crate) struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

/// Server capabilities
#[derive(Debug, Serialize)]
pub(crate) struct ServerCapabilities {
    pub tools: ToolsCapability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
}

/// Tools capability
#[derive(Debug, Serialize)]
pub(crate) struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

/// Server info
#[derive(Debug, Serialize)]
pub(crate) struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Tool definition
#[derive(Debug, Serialize)]
pub(crate) struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Tools list result
#[derive(Debug, Serialize)]
pub(crate) struct ToolsListResult {
    pub tools: Vec<Tool>,
}

/// Tool call parameters
#[derive(Debug, Deserialize)]
pub(crate) struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// Tool call result
#[derive(Debug, Serialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Tool content block
#[derive(Debug, Serialize)]
pub(crate) struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

// ── MCP Resource Types ─────────────────────────────────────────

/// Resource template definition (advertised via `resources/list`)
#[derive(Debug, Serialize)]
pub(crate) struct ResourceTemplate {
    /// URI template with placeholders, e.g. `sem_reg://attributes/{fqn}`
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Static resource definition (advertised via `resources/list`)
#[derive(Debug, Serialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Result for `resources/list`
#[derive(Debug, Serialize)]
pub(crate) struct ResourcesListResult {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<Resource>,
    #[serde(rename = "resourceTemplates", skip_serializing_if = "Vec::is_empty")]
    pub resource_templates: Vec<ResourceTemplate>,
}

/// Parameters for `resources/read`
#[derive(Debug, Deserialize)]
pub(crate) struct ResourceReadParams {
    pub uri: String,
}

/// A single content block returned by `resources/read`
#[derive(Debug, Serialize)]
pub(crate) struct ResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Result for `resources/read`
#[derive(Debug, Serialize)]
pub(crate) struct ResourceReadResult {
    pub contents: Vec<ResourceContent>,
}

impl ResourceReadResult {
    pub(crate) fn json_content(uri: &str, value: &serde_json::Value) -> Self {
        Self {
            contents: vec![ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".into()),
                text: Some(serde_json::to_string_pretty(value).unwrap_or_default()),
            }],
        }
    }

    pub(crate) fn not_found(uri: &str) -> Self {
        Self {
            contents: vec![ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("text/plain".into()),
                text: Some(format!("Resource not found: {}", uri)),
            }],
        }
    }
}

/// Resources capability
#[derive(Debug, Serialize)]
pub(crate) struct ResourcesCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
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
            content: vec![ToolContent {
                content_type: "text".into(),
                text: msg.into(),
            }],
            is_error: Some(true),
        }
    }
}

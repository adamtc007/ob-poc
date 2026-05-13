//! Knowledge-tool trait + registry.
//!
//! Every SemOS knowledge tool exposed over MCP implements
//! [`KnowledgeTool`]. The [`ToolRegistry`] keys tools by their
//! `name()` and dispatches by name from the server's
//! `tools/invoke` method.
//!
//! ## Method shapes (MCP-compatible)
//!
//! - `tools/list` — returns the spec list (name, description,
//!   input schema).
//! - `tools/invoke` — `{name, arguments}` → tool result.
//! - `initialize` — handshake. Returns the server's protocol
//!   version + capability flags.
//!
//! Phase 4.2a defines the framework only; Phase 4.2b/c register
//! the five knowledge tools.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Stable spec describing one tool. Returned by `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's input.
    pub input_schema: Value,
}

/// Error returned by [`KnowledgeTool::invoke`].
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("transport failure: {0}")]
    Transport(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

/// Async tool the server can dispatch to.
#[async_trait]
pub trait KnowledgeTool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    async fn invoke(&self, arguments: Value) -> Result<Value, ToolError>;
}

/// Name-keyed registry. Tools are registered at startup and
/// dispatched per `tools/invoke` request.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    inner: BTreeMap<String, Arc<dyn KnowledgeTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn KnowledgeTool>) {
        let name = tool.spec().name;
        self.inner.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn KnowledgeTool>> {
        self.inner.get(name)
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.inner.values().map(|t| t.spec()).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct EchoTool;

    #[async_trait]
    impl KnowledgeTool for EchoTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: "echo".to_string(),
                description: "echo back the input".to_string(),
                input_schema: json!({"type": "object"}),
            }
        }

        async fn invoke(&self, arguments: Value) -> Result<Value, ToolError> {
            Ok(json!({"echoed": arguments}))
        }
    }

    #[tokio::test]
    async fn registry_dispatches_to_registered_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));
        assert_eq!(registry.len(), 1);
        let tool = registry.get("echo").unwrap();
        let out = tool.invoke(json!({"x": 1})).await.unwrap();
        assert_eq!(out, json!({"echoed": {"x": 1}}));
    }

    #[tokio::test]
    async fn unknown_tool_returns_none() {
        let registry = ToolRegistry::new();
        assert!(registry.get("nope").is_none());
        assert!(registry.is_empty());
    }
}

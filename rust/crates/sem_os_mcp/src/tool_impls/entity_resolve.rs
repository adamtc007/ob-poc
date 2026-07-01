//! `entity_resolve` tool — Phase 4.2b.
//!
//! Maps a natural-language fragment to candidate substrate
//! entities. Delegates into [`SemOsBridge::entity_resolve`].

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::bridge::SemOsBridge;
use crate::tools::{KnowledgeTool, ToolError, ToolSpec};

/// `entity_resolve` tool — `{entity_kind?, text}` →
/// `{matches: [{id, kind, display_name, confidence}, …]}`.
pub struct EntityResolveTool {
    bridge: Arc<dyn SemOsBridge>,
}

impl EntityResolveTool {
    pub fn new(bridge: Arc<dyn SemOsBridge>) -> Self {
        Self { bridge }
    }
}

#[derive(Debug, Deserialize)]
struct EntityResolveArgs {
    /// Optional entity-kind hint (`cbu`, `entity`, `kyc_case`,
    /// …).
    #[serde(default)]
    entity_kind: Option<String>,
    /// Raw fragment from the utterance / Sage context.
    text: String,
}

#[async_trait]
impl KnowledgeTool for EntityResolveTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "entity_resolve".to_string(),
            description: "Resolve a natural-language fragment to candidate substrate entities. \
                 Read-only."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "entity_kind": {
                        "type": "string",
                        "description":
                            "Optional kind hint (cbu, entity, kyc_case, …). Narrows the \
                             candidate set."
                    },
                    "text": {
                        "type": "string",
                        "description": "Natural-language fragment to resolve."
                    }
                },
                "required": ["text"]
            }),
        }
    }

    async fn invoke(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: EntityResolveArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let matches = self
            .bridge
            .entity_resolve(args.entity_kind.as_deref(), &args.text)
            .await
            .map_err(|error| ToolError::Transport(error.to_string()))?;
        Ok(json!({"matches": matches}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::NullBridge;

    #[tokio::test]
    async fn spec_describes_text_as_required() {
        let tool = EntityResolveTool::new(Arc::new(NullBridge::new()));
        let spec = tool.spec();
        assert_eq!(spec.name, "entity_resolve");
        let required = spec.input_schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "text");
    }

    #[tokio::test]
    async fn stub_bridge_returns_empty_matches() {
        let tool = EntityResolveTool::new(Arc::new(NullBridge::new()));
        let out = tool
            .invoke(json!({"entity_kind": "cbu", "text": "Allianz"}))
            .await
            .unwrap();
        assert_eq!(out["matches"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn missing_text_returns_invalid_arguments() {
        let tool = EntityResolveTool::new(Arc::new(NullBridge::new()));
        let err = tool
            .invoke(json!({"entity_kind": "cbu"}))
            .await
            .expect_err("missing text must reject");
        assert!(matches!(err, ToolError::InvalidArguments(_)));
    }

    #[tokio::test]
    async fn entity_kind_is_optional() {
        let tool = EntityResolveTool::new(Arc::new(NullBridge::new()));
        let out = tool.invoke(json!({"text": "Allianz"})).await.unwrap();
        assert!(out["matches"].is_array());
    }
}

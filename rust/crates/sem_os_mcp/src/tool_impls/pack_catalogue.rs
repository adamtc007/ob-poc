//! `pack_catalogue` tool — Phase 4.2c.
//!
//! Returns the substrate's pack catalogue for a workspace.
//! Delegates into [`SemOsBridge::pack_catalogue`].

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::bridge::SemOsBridge;
use crate::tools::{KnowledgeTool, ToolError, ToolSpec};

pub struct PackCatalogueTool {
    bridge: Arc<dyn SemOsBridge>,
}

impl PackCatalogueTool {
    pub fn new(bridge: Arc<dyn SemOsBridge>) -> Self {
        Self { bridge }
    }
}

#[derive(Debug, Deserialize)]
struct PackCatalogueArgs {
    workspace: String,
}

#[async_trait]
impl KnowledgeTool for PackCatalogueTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "pack_catalogue".to_string(),
            description:
                "Walk the pack catalogue for the named workspace. Returns id / name / version \
                 / workspace per entry. Read-only."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspace": {
                        "type": "string",
                        "description":
                            "Workspace tag (cbu, kyc, onboarding_request, …) matching the \
                             serde rename used elsewhere in the system."
                    }
                },
                "required": ["workspace"]
            }),
        }
    }

    async fn invoke(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: PackCatalogueArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let packs = self
            .bridge
            .pack_catalogue(&args.workspace)
            .await
            .map_err(|error| ToolError::Transport(error.to_string()))?;
        Ok(json!({"packs": packs}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::NullBridge;

    #[tokio::test]
    async fn stub_returns_empty_packs() {
        let tool = PackCatalogueTool::new(Arc::new(NullBridge::new()));
        let out = tool.invoke(json!({"workspace": "cbu"})).await.unwrap();
        assert_eq!(out["packs"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn missing_workspace_returns_invalid_arguments() {
        let tool = PackCatalogueTool::new(Arc::new(NullBridge::new()));
        let err = tool
            .invoke(json!({}))
            .await
            .expect_err("workspace required");
        assert!(matches!(err, ToolError::InvalidArguments(_)));
    }
}

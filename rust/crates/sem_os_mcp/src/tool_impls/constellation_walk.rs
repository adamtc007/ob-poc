//! `constellation_walk` tool — Phase 4.2c.
//!
//! Returns the slot-tree projection for a session-anchored
//! constellation. Delegates into
//! [`SemOsBridge::constellation_walk`].

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::bridge::SemOsBridge;
use crate::tools::{KnowledgeTool, ToolError, ToolSpec};

pub struct ConstellationWalkTool {
    bridge: Arc<dyn SemOsBridge>,
}

impl ConstellationWalkTool {
    pub fn new(bridge: Arc<dyn SemOsBridge>) -> Self {
        Self { bridge }
    }
}

#[derive(Debug, Deserialize)]
struct ConstellationWalkArgs {
    workspace: String,
    constellation_id: String,
}

#[async_trait]
impl KnowledgeTool for ConstellationWalkTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "constellation_walk".to_string(),
            description:
                "Walk the slot tree for a session-anchored constellation. Each slot carries \
                 id / kind / state / children. Consumed by the agent's planning loop for \
                 hydration. Read-only."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspace": {
                        "type": "string",
                        "description":
                            "Workspace tag (cbu, kyc, onboarding_request, …) matching the \
                             serde rename used elsewhere in the system."
                    },
                    "constellation_id": {
                        "type": "string",
                        "description":
                            "Session-facing constellation identifier (e.g. \
                             struct.lux.ucits.sicav)."
                    }
                },
                "required": ["workspace", "constellation_id"]
            }),
        }
    }

    async fn invoke(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: ConstellationWalkArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let slots = self
            .bridge
            .constellation_walk(&args.workspace, &args.constellation_id)
            .await
            .map_err(|error| ToolError::Transport(error.to_string()))?;
        Ok(json!({"slots": slots}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::NullBridge;

    #[tokio::test]
    async fn stub_returns_empty_slots() {
        let tool = ConstellationWalkTool::new(Arc::new(NullBridge::new()));
        let out = tool
            .invoke(json!({"workspace": "cbu", "constellation_id": "struct.lux.ucits.sicav"}))
            .await
            .unwrap();
        assert_eq!(out["slots"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn missing_field_returns_invalid_arguments() {
        let tool = ConstellationWalkTool::new(Arc::new(NullBridge::new()));
        let err = tool
            .invoke(json!({"workspace": "cbu"}))
            .await
            .expect_err("constellation_id required");
        assert!(matches!(err, ToolError::InvalidArguments(_)));
    }
}

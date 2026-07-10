//! `fsm_transitions` tool — Phase 4.2c.
//!
//! Returns the FSM transition options from a given state node for
//! an entity kind. Delegates into
//! [`SemOsBridge::fsm_transitions`].

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::bridge::SemOsBridge;
use crate::tools::{KnowledgeTool, ToolError, ToolSpec};

pub struct FsmTransitionsTool {
    bridge: Arc<dyn SemOsBridge>,
}

impl FsmTransitionsTool {
    pub fn new(bridge: Arc<dyn SemOsBridge>) -> Self {
        Self { bridge }
    }
}

#[derive(Debug, Deserialize)]
struct FsmTransitionsArgs {
    entity_kind: String,
    from_state: String,
}

#[async_trait]
impl KnowledgeTool for FsmTransitionsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "fsm_transitions".to_string(),
            description: "Return FSM transition options from the given state for the named entity \
                 kind. Each option carries `from_state`, `to_state`, and the trigger verb \
                 FQN. Read-only."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "entity_kind": {
                        "type": "string",
                        "description": "Entity kind (cbu, kyc_case, instrument, …)."
                    },
                    "from_state": {
                        "type": "string",
                        "description": "Current lifecycle state node."
                    }
                },
                "required": ["entity_kind", "from_state"]
            }),
        }
    }

    async fn invoke(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: FsmTransitionsArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let transitions = self
            .bridge
            .fsm_transitions(&args.entity_kind, &args.from_state)
            .await
            .map_err(|error| ToolError::Transport(error.to_string()))?;
        Ok(json!({"transitions": transitions}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::NullBridge;

    #[tokio::test]
    async fn stub_returns_empty_transitions() {
        let tool = FsmTransitionsTool::new(Arc::new(NullBridge::new()));
        let out = tool
            .invoke(json!({"entity_kind": "cbu", "from_state": "draft"}))
            .await
            .unwrap();
        assert_eq!(out["transitions"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn missing_field_returns_invalid_arguments() {
        let tool = FsmTransitionsTool::new(Arc::new(NullBridge::new()));
        let err = tool
            .invoke(json!({"entity_kind": "cbu"}))
            .await
            .expect_err("from_state required");
        assert!(matches!(err, ToolError::InvalidArguments(_)));
    }
}

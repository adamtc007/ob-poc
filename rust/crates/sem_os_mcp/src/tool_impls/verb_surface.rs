//! `active_verb_surface_at_state` tool — Phase 4.2b.
//!
//! Returns the substrate's ABAC- and lifecycle-pruned legal-verb
//! surface at a given `(workspace, constellation_id, state_node)`.
//! Substitutes the agent's pack-allowlist approximation with the
//! authoritative surface from
//! [`SemOsBridge::active_verb_surface_at_state`].

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::bridge::SemOsBridge;
use crate::tools::{KnowledgeTool, ToolError, ToolSpec};

/// `active_verb_surface_at_state` tool — `{workspace,
/// constellation_id, state_node}` →
/// `{verbs: [{fqn, description, preconditions_met}, …]}`.
pub struct ActiveVerbSurfaceTool {
    bridge: Arc<dyn SemOsBridge>,
}

impl ActiveVerbSurfaceTool {
    pub fn new(bridge: Arc<dyn SemOsBridge>) -> Self {
        Self { bridge }
    }
}

#[derive(Debug, Deserialize)]
struct ActiveVerbSurfaceArgs {
    workspace: String,
    constellation_id: String,
    state_node: String,
}

#[async_trait]
impl KnowledgeTool for ActiveVerbSurfaceTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "active_verb_surface_at_state".to_string(),
            description:
                "Return the substrate's ABAC- and lifecycle-pruned legal-verb surface at the \
                 named state node. Read-only."
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
                    },
                    "state_node": {
                        "type": "string",
                        "description":
                            "Current lifecycle state of the dominant entity (e.g. draft, \
                             awaiting_docs)."
                    }
                },
                "required": ["workspace", "constellation_id", "state_node"]
            }),
        }
    }

    async fn invoke(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: ActiveVerbSurfaceArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let verbs = self
            .bridge
            .active_verb_surface_at_state(
                &args.workspace,
                &args.constellation_id,
                &args.state_node,
            )
            .await
            .map_err(|error| ToolError::Transport(error.to_string()))?;
        Ok(json!({"verbs": verbs}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::StubBridge;

    #[tokio::test]
    async fn spec_required_fields() {
        let tool = ActiveVerbSurfaceTool::new(Arc::new(StubBridge::new()));
        let spec = tool.spec();
        let required: Vec<&str> = spec.input_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(required, vec!["workspace", "constellation_id", "state_node"]);
    }

    #[tokio::test]
    async fn stub_bridge_returns_empty_verbs() {
        let tool = ActiveVerbSurfaceTool::new(Arc::new(StubBridge::new()));
        let out = tool
            .invoke(json!({
                "workspace": "cbu",
                "constellation_id": "struct.lux.ucits.sicav",
                "state_node": "draft"
            }))
            .await
            .unwrap();
        assert_eq!(out["verbs"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn missing_required_returns_invalid_arguments() {
        let tool = ActiveVerbSurfaceTool::new(Arc::new(StubBridge::new()));
        let err = tool
            .invoke(json!({"workspace": "cbu"}))
            .await
            .expect_err("missing fields must reject");
        assert!(matches!(err, ToolError::InvalidArguments(_)));
    }
}

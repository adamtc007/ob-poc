//! Switch adaptor protocol for the bpmn-lite runtime (§6.6).
//!
//! When the runtime reaches an exclusive, inclusive, or event-based gateway it
//! asks the [`SwitchAdaptor`] which outgoing edge(s) to take. The adaptor is
//! responsible for evaluating conditions and returning the selected targets.
//!
//! [`ScriptedAdaptor`] is the test-friendly implementation that returns
//! pre-programmed replies without touching any external state.

use crate::types::InstanceId;
use std::collections::HashMap;

/// Description of one outgoing sequence-flow edge at a gateway.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EdgeInfo {
    pub target: String,
    pub condition: Option<String>,
    pub is_default: bool,
}

/// A decision request sent to the switch adaptor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwitchRequest {
    pub instance_id: InstanceId,
    pub gateway_name: String,
    pub gateway_kind: String,
    pub context_data: serde_json::Value,
    pub outgoing_edges: Vec<EdgeInfo>,
}

/// The adaptor's reply: the set of edge targets to activate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwitchReply {
    /// Targets to activate. Must be non-empty.
    pub selected_targets: Vec<String>,
}

/// Errors from the switch adaptor.
#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    #[error("no branches selected for gateway {gateway}")]
    NoBranchSelected { gateway: String },
    #[error("adaptor error: {0}")]
    AdaptorError(String),
}

/// Adaptor that the runtime queries when it reaches a gateway.
#[async_trait::async_trait]
pub trait SwitchAdaptor: Send + Sync {
    async fn handle(&self, request: SwitchRequest) -> Result<SwitchReply, SwitchError>;
}

// ---------------------------------------------------------------------------
// ScriptedAdaptor — deterministic test helper
// ---------------------------------------------------------------------------

/// A [`SwitchAdaptor`] that returns pre-programmed replies keyed by gateway name.
///
/// If no reply is registered for a gateway the adaptor takes the default edge,
/// or returns [`SwitchError::NoBranchSelected`] when no default exists.
pub struct ScriptedAdaptor {
    replies: HashMap<String, Vec<String>>,
}

impl ScriptedAdaptor {
    pub fn new() -> Self {
        Self {
            replies: HashMap::new(),
        }
    }

    /// Programme a reply: when the runtime reaches `gateway`, activate `targets`.
    pub fn set_reply(&mut self, gateway: &str, targets: Vec<String>) {
        self.replies.insert(gateway.to_string(), targets);
    }
}

impl Default for ScriptedAdaptor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SwitchAdaptor for ScriptedAdaptor {
    async fn handle(&self, request: SwitchRequest) -> Result<SwitchReply, SwitchError> {
        if let Some(targets) = self.replies.get(&request.gateway_name) {
            return Ok(SwitchReply {
                selected_targets: targets.clone(),
            });
        }
        // Fall back to the default edge.
        let default_target = request
            .outgoing_edges
            .iter()
            .find(|e| e.is_default)
            .map(|e| e.target.clone());
        if let Some(t) = default_target {
            Ok(SwitchReply {
                selected_targets: vec![t],
            })
        } else {
            Err(SwitchError::NoBranchSelected {
                gateway: request.gateway_name,
            })
        }
    }
}

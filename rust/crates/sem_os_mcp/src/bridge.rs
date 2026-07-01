//! SemOS substrate bridge — narrow trait the knowledge tools call
//! through.
//!
//! The full `sem_os_client::SemOsClient` trait is heavyweight (it
//! owns governance, changeset publish, affinity-graph access, …).
//! The MCP knowledge tools need only a narrow read surface, so we
//! define [`SemOsBridge`] here and let the binary integrator
//! adapt either an in-process `SemOsClient` or the spike null
//! object ([`NullBridge`]).
//!
//! Phase 4.2b ships:
//!
//! - [`SemOsBridge`] trait — five read-only methods, one per
//!   knowledge tool.
//! - [`NullBridge`] — returns empty / placeholder responses so
//!   `sem_os_mcp` can run hermetically (no DB, no
//!   `sem_os_client`).
//! - Phase 4.3 will introduce an `InProcessSemOsBridge` that
//!   adapts `sem_os_client::SemOsClient` to this surface (likely
//!   in the `sem_os_mcp` binary integrator or a sibling crate).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Slim candidate entity from `entity_resolve`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub confidence: f32,
}

/// Slim verb FQN + metadata from `active_verb_surface_at_state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveVerb {
    pub fqn: String,
    pub description: String,
    pub preconditions_met: bool,
}

/// Slim pack catalogue entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub workspace: String,
}

/// One FSM transition option at a state node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmTransition {
    pub from_state: String,
    pub to_state: String,
    pub trigger_verb_fqn: String,
}

/// One slot in the constellation walk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationSlot {
    pub slot_id: String,
    pub kind: String,
    pub state: String,
    pub children: Vec<ConstellationSlot>,
}

/// Bridge errors. Narrow on purpose; richer typing surfaces at the
/// tool layer (`ToolError`).
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("unsupported by this bridge implementation: {0}")]
    Unsupported(String),
    #[error("substrate failure: {0}")]
    Substrate(String),
}

/// Read-only substrate surface the MCP knowledge tools call.
#[async_trait]
pub trait SemOsBridge: Send + Sync {
    /// Resolve a natural-language fragment to candidate entities.
    /// `kind` is an optional kind hint (e.g. `"cbu"`, `"entity"`).
    async fn entity_resolve(
        &self,
        kind: Option<&str>,
        text: &str,
    ) -> Result<Vec<EntityMatch>, BridgeError>;

    /// Active verbs at a state node, scoped to `(workspace,
    /// constellation_id, state_node)`. Returned set is the ABAC-
    /// and lifecycle-pruned legal-verb surface.
    async fn active_verb_surface_at_state(
        &self,
        workspace: &str,
        constellation_id: &str,
        state_node: &str,
    ) -> Result<Vec<ActiveVerb>, BridgeError>;

    /// Pack catalogue for a workspace.
    async fn pack_catalogue(&self, workspace: &str) -> Result<Vec<PackEntry>, BridgeError>;

    /// FSM transition options from a state node.
    async fn fsm_transitions(
        &self,
        entity_kind: &str,
        from_state: &str,
    ) -> Result<Vec<FsmTransition>, BridgeError>;

    /// Constellation walk for a `(workspace, constellation_id)`.
    async fn constellation_walk(
        &self,
        workspace: &str,
        constellation_id: &str,
    ) -> Result<Vec<ConstellationSlot>, BridgeError>;

    /// Provider label for diagnostics / audit.
    fn provider_label(&self) -> &str {
        "unknown"
    }
}

/// Hermetic null-object bridge — returns empty responses for every
/// query and records the call at debug level. Used by:
/// - Unit tests in this crate.
/// - The spike `sem_os_mcp` binary when no real `SemOsClient` is
///   wired (e.g. running without `DATABASE_URL`).
#[derive(Debug, Default, Clone)]
pub struct NullBridge {
    label: String,
}

impl NullBridge {
    pub fn new() -> Self {
        Self {
            label: "null".to_string(),
        }
    }

    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

#[async_trait]
impl SemOsBridge for NullBridge {
    async fn entity_resolve(
        &self,
        kind: Option<&str>,
        text: &str,
    ) -> Result<Vec<EntityMatch>, BridgeError> {
        tracing::debug!(
            target: "sem_os_mcp",
            ?kind, text, "null bridge entity_resolve — returning []"
        );
        Ok(Vec::new())
    }

    async fn active_verb_surface_at_state(
        &self,
        workspace: &str,
        constellation_id: &str,
        state_node: &str,
    ) -> Result<Vec<ActiveVerb>, BridgeError> {
        tracing::debug!(
            target: "sem_os_mcp",
            workspace, constellation_id, state_node,
            "null bridge active_verb_surface_at_state — returning []"
        );
        Ok(Vec::new())
    }

    async fn pack_catalogue(&self, workspace: &str) -> Result<Vec<PackEntry>, BridgeError> {
        tracing::debug!(
            target: "sem_os_mcp",
            workspace, "null bridge pack_catalogue — returning []"
        );
        Ok(Vec::new())
    }

    async fn fsm_transitions(
        &self,
        entity_kind: &str,
        from_state: &str,
    ) -> Result<Vec<FsmTransition>, BridgeError> {
        tracing::debug!(
            target: "sem_os_mcp",
            entity_kind, from_state,
            "null bridge fsm_transitions — returning []"
        );
        Ok(Vec::new())
    }

    async fn constellation_walk(
        &self,
        workspace: &str,
        constellation_id: &str,
    ) -> Result<Vec<ConstellationSlot>, BridgeError> {
        tracing::debug!(
            target: "sem_os_mcp",
            workspace, constellation_id,
            "null bridge constellation_walk — returning []"
        );
        Ok(Vec::new())
    }

    fn provider_label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn null_bridge_returns_empty_for_every_method() {
        let bridge = NullBridge::new();
        assert!(bridge
            .entity_resolve(Some("cbu"), "Allianz")
            .await
            .unwrap()
            .is_empty());
        assert!(bridge
            .active_verb_surface_at_state("cbu", "struct.lux.ucits.sicav", "draft")
            .await
            .unwrap()
            .is_empty());
        assert!(bridge.pack_catalogue("cbu").await.unwrap().is_empty());
        assert!(bridge
            .fsm_transitions("cbu", "draft")
            .await
            .unwrap()
            .is_empty());
        assert!(bridge
            .constellation_walk("cbu", "struct.lux.ucits.sicav")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn null_bridge_label() {
        assert_eq!(NullBridge::new().provider_label(), "null");
        assert_eq!(
            NullBridge::with_label("phase-4-spike").provider_label(),
            "phase-4-spike"
        );
    }
}

//! MCP-backed knowledge + hydration clients — Phase 4.3.
//!
//! Replaces the Phase 2.8 `StubKnowledgeClient` and Phase 3.2
//! `StubConstellationHydrator` with implementations that drive the
//! `sem_os_mcp` server over the standard MCP `tools/invoke`
//! protocol.
//!
//! ## In-process today, subprocess later
//!
//! The Phase 4.3 wiring keeps the MCP server in the same address
//! space as the agent — both consume the protocol DTOs from
//! `sem_os_mcp::protocol` and the dispatcher from
//! `sem_os_mcp::server`. Production deployments will swap the
//! in-process server for a subprocess client speaking newline-
//! delimited JSON-RPC over stdio; the trait surface this module
//! exposes (`SemOsKnowledgeClient`, `ConstellationHydrator`) is
//! transport-agnostic so the swap is mechanical.
//!
//! ## Translation
//!
//! - `KnowledgeQuery::ResolveEntity` → `entity_resolve` tool.
//! - `KnowledgeQuery::ActiveVerbsAtState` →
//!   `active_verb_surface_at_state` tool.
//! - `KnowledgeQuery::PackCatalogue` → `pack_catalogue` tool.
//! - Constellation hydration → `constellation_walk` tool.
//!
//! `KnowledgeResponse::Empty` is returned for any tool result that
//! produces zero entries, so consumers always see a structured
//! response.

use std::sync::Arc;

use async_trait::async_trait;
use sem_os_mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use sem_os_mcp::server::McpServer;
use serde_json::{json, Value};

use crate::constellation::{
    ConstellationHydrator, ConstellationSnapshot, EntityStateDTO, HydrationError, HydrationScope,
};
use crate::knowledge::{
    EntityMatch, KnowledgeError, KnowledgeQuery, KnowledgeResponse, PackSummary,
    SemOsKnowledgeClient,
};

/// Wraps an `McpServer` and dispatches knowledge queries through
/// it. Constructed once at startup; cheap to share via `Arc`.
pub struct McpKnowledgeClient {
    server: Arc<McpServer>,
    provider_label: String,
}

impl McpKnowledgeClient {
    pub fn new(server: Arc<McpServer>, provider_label: impl Into<String>) -> Self {
        Self {
            server,
            provider_label: provider_label.into(),
        }
    }

    async fn invoke_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<Value, KnowledgeError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(0)),
            method: "tools/invoke".to_string(),
            params: json!({
                "name": name,
                "arguments": arguments,
            }),
        };
        let response = self.server.handle_request(request).await;
        extract_tool_result(name, response)
    }
}

#[async_trait]
impl SemOsKnowledgeClient for McpKnowledgeClient {
    async fn query(&self, query: KnowledgeQuery) -> Result<KnowledgeResponse, KnowledgeError> {
        match query {
            KnowledgeQuery::ResolveEntity { entity_kind, text } => {
                let args = json!({
                    "entity_kind": entity_kind,
                    "text": text,
                });
                let result = self.invoke_tool("entity_resolve", args).await?;
                let matches = parse_matches(&result)?;
                if matches.is_empty() {
                    Ok(KnowledgeResponse::Empty)
                } else {
                    Ok(KnowledgeResponse::Entities { matches })
                }
            }
            KnowledgeQuery::ActiveVerbsAtState {
                workspace,
                constellation_id,
                state_node,
            } => {
                let args = json!({
                    "workspace": workspace,
                    "constellation_id": constellation_id,
                    "state_node": state_node,
                });
                let result = self
                    .invoke_tool("active_verb_surface_at_state", args)
                    .await?;
                let fqns = parse_verbs(&result)?;
                if fqns.is_empty() {
                    Ok(KnowledgeResponse::Empty)
                } else {
                    Ok(KnowledgeResponse::Verbs { fqns })
                }
            }
            KnowledgeQuery::PackCatalogue { workspace } => {
                let args = json!({"workspace": workspace});
                let result = self.invoke_tool("pack_catalogue", args).await?;
                let entries = parse_packs(&result)?;
                if entries.is_empty() {
                    Ok(KnowledgeResponse::Empty)
                } else {
                    Ok(KnowledgeResponse::Packs { entries })
                }
            }
        }
    }

    fn provider_label(&self) -> &str {
        &self.provider_label
    }
}

/// MCP-backed constellation hydrator. Calls the
/// `constellation_walk` tool and reshapes its slot tree into the
/// agent's `ConstellationSnapshot`.
pub struct McpConstellationHydrator {
    server: Arc<McpServer>,
    provider_label: String,
}

impl McpConstellationHydrator {
    pub fn new(server: Arc<McpServer>, provider_label: impl Into<String>) -> Self {
        Self {
            server,
            provider_label: provider_label.into(),
        }
    }
}

#[async_trait]
impl ConstellationHydrator for McpConstellationHydrator {
    async fn hydrate(
        &self,
        scope: HydrationScope<'_>,
    ) -> Result<ConstellationSnapshot, HydrationError> {
        let args = json!({
            "workspace": scope.workspace,
            "constellation_id": scope.constellation_id.unwrap_or(""),
        });
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(0)),
            method: "tools/invoke".to_string(),
            params: json!({
                "name": "constellation_walk",
                "arguments": args,
            }),
        };
        let response = self.server.handle_request(request).await;
        let result = match response.result {
            Some(value) => value,
            None => {
                let message = response
                    .error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "no result".to_string());
                return Err(HydrationError::Transport(message));
            }
        };
        // `result.result.slots` per server contract.
        let entity_states = parse_slot_tree(&result["result"]["slots"])
            .map_err(HydrationError::Transport)?;
        Ok(ConstellationSnapshot {
            entity_states,
            hydrated_at: chrono::Utc::now(),
        })
    }

    fn provider_label(&self) -> &str {
        &self.provider_label
    }
}

fn extract_tool_result(
    tool: &str,
    response: JsonRpcResponse,
) -> Result<Value, KnowledgeError> {
    if let Some(error) = response.error {
        return Err(KnowledgeError::Transport(format!(
            "{tool} failed: {} (code {})",
            error.message, error.code
        )));
    }
    let value = response
        .result
        .ok_or_else(|| KnowledgeError::Transport(format!("{tool} returned no result")))?;
    Ok(value)
}

fn parse_matches(value: &Value) -> Result<Vec<EntityMatch>, KnowledgeError> {
    let array = value["result"]["matches"]
        .as_array()
        .ok_or_else(|| KnowledgeError::Transport("matches not an array".to_string()))?;
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let parsed: EntityMatch = serde_json::from_value(entry.clone())
            .map_err(|e| KnowledgeError::Transport(format!("entity match decode: {e}")))?;
        out.push(parsed);
    }
    Ok(out)
}

fn parse_verbs(value: &Value) -> Result<Vec<String>, KnowledgeError> {
    let array = value["result"]["verbs"]
        .as_array()
        .ok_or_else(|| KnowledgeError::Transport("verbs not an array".to_string()))?;
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let fqn = entry["fqn"]
            .as_str()
            .ok_or_else(|| KnowledgeError::Transport("verb fqn missing".to_string()))?;
        out.push(fqn.to_string());
    }
    Ok(out)
}

fn parse_packs(value: &Value) -> Result<Vec<PackSummary>, KnowledgeError> {
    let array = value["result"]["packs"]
        .as_array()
        .ok_or_else(|| KnowledgeError::Transport("packs not an array".to_string()))?;
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let parsed: PackSummary = serde_json::from_value(entry.clone())
            .map_err(|e| KnowledgeError::Transport(format!("pack decode: {e}")))?;
        out.push(parsed);
    }
    Ok(out)
}

fn parse_slot_tree(value: &Value) -> Result<Vec<EntityStateDTO>, String> {
    let array = value
        .as_array()
        .ok_or_else(|| "slots not an array".to_string())?;
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let entity_id = entry["slot_id"]
            .as_str()
            .ok_or_else(|| "slot_id missing".to_string())?;
        let entity_kind = entry["kind"]
            .as_str()
            .ok_or_else(|| "kind missing".to_string())?;
        let state = entry["state"]
            .as_str()
            .ok_or_else(|| "state missing".to_string())?;
        out.push(EntityStateDTO {
            entity_id: entity_id.to_string(),
            entity_kind: entity_kind.to_string(),
            state: state.to_string(),
            attributes: Default::default(),
        });
        // Children are not flattened here — Phase 4 widens the
        // DTO if the planning loop needs them.
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_mcp::bridge::StubBridge;
    use sem_os_mcp::tool_impls::build_registry;
    use std::sync::Arc;

    fn build_server() -> Arc<McpServer> {
        let registry = build_registry(Arc::new(StubBridge::with_label("phase-4-test")));
        Arc::new(McpServer::new(registry))
    }

    #[tokio::test]
    async fn stub_bridge_yields_empty_response_for_resolve_entity() {
        let client =
            McpKnowledgeClient::new(build_server(), "phase-4-test");
        let response = client
            .query(KnowledgeQuery::ResolveEntity {
                entity_kind: Some("cbu".to_string()),
                text: "Allianz".to_string(),
            })
            .await
            .unwrap();
        assert!(matches!(response, KnowledgeResponse::Empty));
    }

    #[tokio::test]
    async fn stub_bridge_yields_empty_response_for_active_verbs() {
        let client = McpKnowledgeClient::new(build_server(), "phase-4-test");
        let response = client
            .query(KnowledgeQuery::ActiveVerbsAtState {
                workspace: "cbu".to_string(),
                constellation_id: "struct.lux.ucits.sicav".to_string(),
                state_node: "draft".to_string(),
            })
            .await
            .unwrap();
        assert!(matches!(response, KnowledgeResponse::Empty));
    }

    #[tokio::test]
    async fn stub_bridge_yields_empty_response_for_pack_catalogue() {
        let client = McpKnowledgeClient::new(build_server(), "phase-4-test");
        let response = client
            .query(KnowledgeQuery::PackCatalogue {
                workspace: "cbu".to_string(),
            })
            .await
            .unwrap();
        assert!(matches!(response, KnowledgeResponse::Empty));
    }

    #[tokio::test]
    async fn provider_label_round_trips() {
        let client = McpKnowledgeClient::new(build_server(), "phase-4-spike");
        assert_eq!(client.provider_label(), "phase-4-spike");
    }

    #[tokio::test]
    async fn hydrator_returns_empty_snapshot_against_stub() {
        let hydrator = McpConstellationHydrator::new(build_server(), "phase-4-test");
        let snapshot = hydrator
            .hydrate(HydrationScope {
                workspace: "cbu",
                pack_id: "book-setup",
                constellation_id: Some("struct.lux.ucits.sicav"),
            })
            .await
            .unwrap();
        assert!(snapshot.is_empty());
    }
}

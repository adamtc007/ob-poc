//! SemOS knowledge query surface for the Sage runtime.
//!
//! Phase 2.8 of the Sage ACP capability plan. The Sage planning loop
//! needs SemOS-resident knowledge — entity resolution, the active
//! verb surface at a state node, the macro / pack catalogue, FSM
//! transitions, the constellation walk — to ground its draft against
//! the substrate. This module defines the trait the loop calls
//! through; concrete impls live behind transport-specific clients.
//!
//! ## Design
//!
//! - [`SemOsKnowledgeClient`] — async trait the planning loop calls.
//!   Phase 2 ships a [`StubKnowledgeClient`] that returns empty
//!   results so the spike binary runs end-to-end without any SemOS
//!   substrate dependency. Phase 4 introduces a `sem_os_mcp`-backed
//!   impl that speaks MCP to a dedicated knowledge server.
//! - The shape of the trait is intentionally narrow (one
//!   `query(KnowledgeQuery) -> KnowledgeResponse` method) so the
//!   Phase 4 cutover is mechanical — only the impl changes.
//!
//! ## Charter-clean dep wall
//!
//! No `ob-poc` dep here. The agent crate consumes the trait; the
//! binary integrator wires the impl at startup. For the spike that
//! impl is the in-crate stub; in production it will be the
//! `sem_os_mcp` MCP client. An `ob-poc`-resident bridge impl could
//! also be wired by a different binary if needed during the
//! transition.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::index::SessionIndex;

/// Query shapes the planning loop sends through the knowledge
/// surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "query_kind", rename_all = "snake_case")]
pub enum KnowledgeQuery {
    /// Resolve a natural-language fragment to a candidate entity.
    /// Phase 4 wires `sem_os_mcp::entity_resolve`.
    ResolveEntity {
        /// Entity kind hint (e.g. `cbu`, `entity`, `kyc_case`).
        entity_kind: Option<String>,
        /// Raw text from the utterance.
        text: String,
    },
    /// Ask for the active-verb surface at a given state node. The
    /// spike's pack allowlist is a coarse approximation; this
    /// query returns the SemOS substrate's session-aware surface,
    /// pruned by ABAC + workspace + lifecycle state.
    ActiveVerbsAtState {
        workspace: String,
        constellation_id: String,
        state_node: String,
    },
    /// Walk the macro / pack catalogue for compound intent
    /// matching. Phase 4 wires `sem_os_mcp::pack_catalogue`.
    PackCatalogue { workspace: String },
}

/// Response shapes from the knowledge surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "response_kind", rename_all = "snake_case")]
pub enum KnowledgeResponse {
    /// Resolved entities (zero or more).
    Entities { matches: Vec<EntityMatch> },
    /// Active verb FQNs at the requested state.
    Verbs { fqns: Vec<String> },
    /// Pack catalogue summaries (id + name + version).
    Packs { entries: Vec<PackSummary> },
    /// Nothing was returned (substrate confirms the negative).
    Empty,
}

/// One entity resolution candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub confidence: f32,
}

/// Slim pack catalogue summary returned by the substrate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSummary {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// Async client surface for the SemOS knowledge substrate.
///
/// Two impls planned:
/// - [`StubKnowledgeClient`] — Phase 2 spike. Returns
///   [`KnowledgeResponse::Empty`] for every query. Lets the binary
///   construct a full planning loop without any substrate
///   dependency.
/// - `SemOsMcpClient` (Phase 4) — speaks MCP to `sem_os_mcp` for
///   real knowledge queries.
#[async_trait]
pub trait SemOsKnowledgeClient: Send + Sync {
    async fn query(&self, query: KnowledgeQuery) -> Result<KnowledgeResponse, KnowledgeError>;

    /// Optional human-readable label for diagnostics / audit. The
    /// stub returns `"stub"`; the MCP client returns the transport
    /// URL.
    fn provider_label(&self) -> &str {
        "unknown"
    }
}

/// Errors surfaced by the knowledge client. Stays narrow on purpose
/// — Phase 4 widens this when MCP transport errors land.
#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("knowledge query unsupported: {0}")]
    Unsupported(String),
    #[error("knowledge transport failure: {0}")]
    Transport(String),
}

/// Spike-only stub. Returns `Empty` for every query and records the
/// fact that a query reached it. Useful for confirming the planning
/// loop wires the knowledge surface end-to-end before the MCP client
/// is built.
#[derive(Debug, Default, Clone)]
pub struct StubKnowledgeClient {
    label: String,
}

impl StubKnowledgeClient {
    pub fn new() -> Self {
        Self {
            label: "stub".to_string(),
        }
    }

    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

#[async_trait]
impl SemOsKnowledgeClient for StubKnowledgeClient {
    async fn query(&self, query: KnowledgeQuery) -> Result<KnowledgeResponse, KnowledgeError> {
        tracing::debug!(target: "sage-acp", ?query, "stub knowledge query — returning Empty");
        Ok(KnowledgeResponse::Empty)
    }

    fn provider_label(&self) -> &str {
        &self.label
    }
}

/// Helper for the planning loop: build the standard
/// "verbs-at-current-state" query from a session index. Phase 4.6 —
/// the spike has no real constellation_id (Phase 3.2 hydrator
/// returns `Empty`), so the query carries a synthetic `session_root`
/// state node so the substrate can still answer at workspace + pack
/// granularity. Returns the query unconditionally; the planning loop
/// uses an empty `KnowledgeResponse::Verbs` to mean "substrate has
/// nothing to say, fall back to pack allowlist".
pub fn active_verbs_query_for_index(index: &SessionIndex) -> Option<KnowledgeQuery> {
    Some(KnowledgeQuery::ActiveVerbsAtState {
        workspace: workspace_label(&index.workspace),
        constellation_id: "session_root".to_string(),
        state_node: "session_root".to_string(),
    })
}

fn workspace_label(workspace: &ob_poc_types::session::kinds::WorkspaceKind) -> String {
    use ob_poc_types::session::kinds::WorkspaceKind;
    match workspace {
        WorkspaceKind::Cbu => "cbu".to_string(),
        WorkspaceKind::Kyc => "kyc".to_string(),
        WorkspaceKind::Deal => "deal".to_string(),
        WorkspaceKind::InstrumentMatrix => "instrument-matrix".to_string(),
        WorkspaceKind::BookingPrincipal => "booking-principal".to_string(),
        WorkspaceKind::LifecycleResources => "lifecycle-resources".to_string(),
        WorkspaceKind::ProductMaintenance => "product-maintenance".to_string(),
        WorkspaceKind::SemOsMaintenance => "sem-os-maintenance".to_string(),
        WorkspaceKind::OnBoarding => "onboarding".to_string(),
        WorkspaceKind::Catalogue => "catalogue".to_string(),
        WorkspaceKind::Bpmn => "bpmn".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_client_returns_empty_for_every_query() {
        let client = StubKnowledgeClient::new();
        let response = client
            .query(KnowledgeQuery::ResolveEntity {
                entity_kind: Some("cbu".to_string()),
                text: "Allianz".to_string(),
            })
            .await
            .expect("stub never errors");
        assert!(matches!(response, KnowledgeResponse::Empty));
    }

    #[tokio::test]
    async fn stub_client_label_is_stub() {
        let client = StubKnowledgeClient::new();
        assert_eq!(client.provider_label(), "stub");
    }

    #[tokio::test]
    async fn stub_client_label_override() {
        let client = StubKnowledgeClient::with_label("phase-2-spike");
        assert_eq!(client.provider_label(), "phase-2-spike");
    }
}

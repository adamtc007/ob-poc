//! API request/response types for the Semantic OS service boundary.
//!
//! These are the "proto" types that both `SemOsClient` and `CoreService` use.
//! Today they are plain Rust structs; in Stage 2.x they will be generated
//! from `.proto` files via prost-build for the gRPC boundary.

use serde::{Deserialize, Serialize};

// ── Re-exports from context_resolution ────────────────────────
// These are the canonical request/response types for resolve_context().
// Re-exported here so that `SemOsClient` and `CoreService` both import
// from `sem_os_core::proto::*` without knowing the internal module path.

pub use crate::context_resolution::{
    ContextResolutionRequest as ResolveContextRequest,
    ContextResolutionResponse as ResolveContextResponse,
};

// ── Manifest ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetManifestResponse {
    pub snapshot_set_id: String,
    pub published_at: String,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub snapshot_id: String,
    pub object_type: String,
    pub fqn: String,
    pub content_hash: String,
}

// ── Export ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSnapshotSetResponse {
    pub snapshots: Vec<ExportedSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedSnapshot {
    pub snapshot_id: String,
    pub fqn: String,
    pub object_type: String,
    pub payload: serde_json::Value,
}

// ── Bootstrap ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapSeedBundleResponse {
    pub created: u32,
    pub skipped: u32,
    pub bundle_hash: String,
    /// The snapshot_set_id created for this bootstrap (None if all seeds were skipped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_set_id: Option<String>,
}

// ── Tool Dispatch ─────────────────────────────────────────────

/// Request to invoke a named MCP tool with JSON arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

/// Result of an MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub success: bool,
    pub data: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Description of a single tool parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParamSpec {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
}

/// Specification for a single MCP tool (name, description, parameters).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParamSpec>,
}

/// Response containing the list of available tool specifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolSpecsResponse {
    pub tools: Vec<ToolSpec>,
}

// ── Changeset / Workbench ─────────────────────────────────────

/// Response for listing changesets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChangesetsResponse {
    pub changesets: Vec<crate::types::Changeset>,
}

/// Query parameters for listing changesets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListChangesetsQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// A single diff entry showing what changed vs the current active snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub entry_id: String,
    pub object_fqn: String,
    pub object_type: String,
    pub change_kind: String,
    /// The base snapshot this entry was drafted against (None for new additions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_snapshot_id: Option<String>,
    /// The current active snapshot for this FQN (None if not yet published).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_snapshot_id: Option<String>,
    /// True if base_snapshot_id differs from current_snapshot_id (stale draft).
    pub is_stale: bool,
    /// The draft payload that would be published.
    pub draft_payload: serde_json::Value,
    /// The current active payload for comparison (None for additions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_payload: Option<serde_json::Value>,
}

/// Response for changeset diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesetDiffResponse {
    pub changeset_id: String,
    pub status: String,
    pub entries: Vec<DiffEntry>,
    pub stale_count: u32,
}

/// A downstream dependent affected by a changeset entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactEntry {
    pub source_fqn: String,
    pub source_object_type: String,
    pub dependent_fqn: String,
    pub dependent_object_type: String,
    pub relationship: String,
}

/// Response for changeset impact analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesetImpactResponse {
    pub changeset_id: String,
    pub impacts: Vec<ImpactEntry>,
}

/// Response for gate preview on a changeset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePreviewResponse {
    pub changeset_id: String,
    pub would_block: bool,
    pub error_count: u32,
    pub warning_count: u32,
    pub gate_results: Vec<GatePreviewEntry>,
}

/// A single gate result in the preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePreviewEntry {
    pub entry_id: String,
    pub object_fqn: String,
    pub gate_name: String,
    pub severity: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Response for changeset publish (promotion).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesetPublishResponse {
    pub changeset_id: String,
    pub snapshots_created: u32,
    pub snapshot_set_id: String,
}

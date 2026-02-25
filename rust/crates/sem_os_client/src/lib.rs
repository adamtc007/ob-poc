//! SemOsClient trait — the sole API boundary between ob-poc and Semantic OS.
//! ob-poc depends on this crate, never on sem_os_postgres or sem_os_server.

pub mod http;
pub mod inprocess;

use async_trait::async_trait;
use sem_os_core::{error::SemOsError, principal::Principal, proto::*, seeds::SeedBundle};

pub type Result<T> = std::result::Result<T, SemOsError>;

#[async_trait]
pub trait SemOsClient: Send + Sync {
    async fn resolve_context(
        &self,
        principal: &Principal,
        req: ResolveContextRequest,
    ) -> Result<ResolveContextResponse>;

    async fn get_manifest(&self, snapshot_set_id: &str) -> Result<GetManifestResponse>;

    async fn export_snapshot_set(&self, snapshot_set_id: &str)
        -> Result<ExportSnapshotSetResponse>;

    async fn bootstrap_seed_bundle(
        &self,
        principal: &Principal,
        bundle: SeedBundle,
    ) -> Result<BootstrapSeedBundleResponse>;

    /// Dispatch a named MCP tool with JSON arguments.
    /// Returns the tool result (success + data or error).
    async fn dispatch_tool(
        &self,
        principal: &Principal,
        req: ToolCallRequest,
    ) -> Result<ToolCallResponse>;

    /// List all available tool specifications.
    async fn list_tool_specs(&self) -> Result<ListToolSpecsResponse>;

    // ── Changeset / Workbench ──────────────────────────────────

    /// List changesets with optional filters.
    async fn list_changesets(&self, query: ListChangesetsQuery) -> Result<ListChangesetsResponse>;

    /// Diff a changeset's entries against current active snapshots.
    async fn changeset_diff(&self, changeset_id: &str) -> Result<ChangesetDiffResponse>;

    /// Impact analysis — find downstream dependents for each modified FQN.
    async fn changeset_impact(&self, changeset_id: &str) -> Result<ChangesetImpactResponse>;

    /// Gate preview — run publish gates against draft entries without persisting.
    async fn changeset_gate_preview(&self, changeset_id: &str) -> Result<GatePreviewResponse>;

    /// Publish an approved changeset — promote all entries to active snapshots.
    async fn publish_changeset(
        &self,
        principal: &Principal,
        changeset_id: &str,
    ) -> Result<ChangesetPublishResponse>;

    /// Test-only: synchronously drain and process all pending outbox events.
    /// Only implemented by InProcessClient. HttpClient returns Ok(()) immediately.
    async fn drain_outbox_for_test(&self) -> Result<()>;
}

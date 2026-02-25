//! InProcessClient — calls CoreService directly (no network).
//!
//! This is the primary client for in-process usage where `ob-poc-web`
//! hosts the Semantic OS directly (no standalone server needed).
//! All methods delegate to `CoreService` which holds the port trait references.

use std::sync::Arc;

use async_trait::async_trait;
use sem_os_core::{principal::Principal, proto::*, seeds::SeedBundle, service::CoreService};

use crate::{Result, SemOsClient};

pub struct InProcessClient {
    service: Arc<dyn CoreService>,
    /// Default principal for operations that don't take an explicit principal.
    /// Used by `drain_outbox_for_test` and future convenience methods.
    #[allow(dead_code)]
    principal: Principal,
}

impl InProcessClient {
    /// Create a new InProcessClient wrapping a CoreService.
    ///
    /// The `principal` is used as the default identity for operations
    /// that don't take an explicit principal parameter.
    pub fn new(service: Arc<dyn CoreService>, principal: Principal) -> Self {
        Self { service, principal }
    }
}

#[async_trait]
impl SemOsClient for InProcessClient {
    async fn resolve_context(
        &self,
        principal: &Principal,
        req: ResolveContextRequest,
    ) -> Result<ResolveContextResponse> {
        self.service.resolve_context(principal, req).await
    }

    async fn get_manifest(&self, snapshot_set_id: &str) -> Result<GetManifestResponse> {
        self.service.get_manifest(snapshot_set_id).await
    }

    async fn export_snapshot_set(
        &self,
        snapshot_set_id: &str,
    ) -> Result<ExportSnapshotSetResponse> {
        self.service.export_snapshot_set(snapshot_set_id).await
    }

    async fn bootstrap_seed_bundle(
        &self,
        principal: &Principal,
        bundle: SeedBundle,
    ) -> Result<BootstrapSeedBundleResponse> {
        self.service.bootstrap_seed_bundle(principal, bundle).await
    }

    async fn dispatch_tool(
        &self,
        principal: &Principal,
        req: ToolCallRequest,
    ) -> Result<ToolCallResponse> {
        self.service.dispatch_tool(principal, req).await
    }

    async fn list_tool_specs(&self) -> Result<ListToolSpecsResponse> {
        self.service.list_tool_specs().await
    }

    // ── Changeset / Workbench ──────────────────────────────────

    async fn list_changesets(&self, query: ListChangesetsQuery) -> Result<ListChangesetsResponse> {
        self.service.list_changesets(query).await
    }

    async fn changeset_diff(&self, changeset_id: &str) -> Result<ChangesetDiffResponse> {
        self.service.changeset_diff(changeset_id).await
    }

    async fn changeset_impact(&self, changeset_id: &str) -> Result<ChangesetImpactResponse> {
        self.service.changeset_impact(changeset_id).await
    }

    async fn changeset_gate_preview(&self, changeset_id: &str) -> Result<GatePreviewResponse> {
        self.service.changeset_gate_preview(changeset_id).await
    }

    async fn publish_changeset(
        &self,
        principal: &Principal,
        changeset_id: &str,
    ) -> Result<ChangesetPublishResponse> {
        let snapshots_created = self
            .service
            .promote_changeset(principal, changeset_id)
            .await?;
        Ok(ChangesetPublishResponse {
            changeset_id: changeset_id.to_string(),
            snapshots_created,
            snapshot_set_id: changeset_id.to_string(), // changeset_id == snapshot_set_id by design
        })
    }

    async fn drain_outbox_for_test(&self) -> Result<()> {
        self.service.drain_outbox_for_test().await
    }
}

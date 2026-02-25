//! HttpClient — calls Semantic OS REST server over HTTP with JWT bearer.
//!
//! All methods map to the corresponding server endpoints.
//! Error bodies are deserialized to `SemOsError` based on HTTP status.

use async_trait::async_trait;
use sem_os_core::{error::SemOsError, principal::Principal, proto::*, seeds::SeedBundle};

use crate::{Result, SemOsClient};

pub struct HttpClient {
    base_url: String,
    jwt_token: String,
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new(base_url: impl Into<String>, jwt_token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            jwt_token: jwt_token.into(),
            client: reqwest::Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    async fn handle_error_response(&self, resp: reqwest::Response) -> SemOsError {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();

        // Try to extract the error message from the JSON body
        let msg = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or(body);

        match status {
            404 => SemOsError::NotFound(msg),
            422 => SemOsError::GateFailed(vec![]),
            403 => SemOsError::Unauthorized(msg),
            409 => SemOsError::Conflict(msg),
            400 => SemOsError::InvalidInput(msg),
            503 => SemOsError::MigrationPending(msg),
            _ => SemOsError::Internal(anyhow::anyhow!("HTTP {status}: {msg}")),
        }
    }
}

#[async_trait]
impl SemOsClient for HttpClient {
    async fn resolve_context(
        &self,
        _principal: &Principal,
        req: ResolveContextRequest,
    ) -> Result<ResolveContextResponse> {
        let resp = self
            .client
            .post(self.url("/resolve_context"))
            .bearer_auth(&self.jwt_token)
            .json(&req)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ResolveContextResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn get_manifest(&self, snapshot_set_id: &str) -> Result<GetManifestResponse> {
        let resp = self
            .client
            .get(self.url(&format!("/snapshot_sets/{}/manifest", snapshot_set_id)))
            .bearer_auth(&self.jwt_token)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<GetManifestResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn export_snapshot_set(
        &self,
        snapshot_set_id: &str,
    ) -> Result<ExportSnapshotSetResponse> {
        let resp = self
            .client
            .get(self.url(&format!("/exports/snapshot_set/{}", snapshot_set_id)))
            .bearer_auth(&self.jwt_token)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ExportSnapshotSetResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn bootstrap_seed_bundle(
        &self,
        _principal: &Principal,
        bundle: SeedBundle,
    ) -> Result<BootstrapSeedBundleResponse> {
        let resp = self
            .client
            .post(self.url("/bootstrap/seed_bundle"))
            .bearer_auth(&self.jwt_token)
            .json(&bundle)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<BootstrapSeedBundleResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn dispatch_tool(
        &self,
        _principal: &Principal,
        req: ToolCallRequest,
    ) -> Result<ToolCallResponse> {
        let resp = self
            .client
            .post(self.url("/tools/call"))
            .bearer_auth(&self.jwt_token)
            .json(&req)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ToolCallResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn list_tool_specs(&self) -> Result<ListToolSpecsResponse> {
        let resp = self
            .client
            .get(self.url("/tools/list"))
            .bearer_auth(&self.jwt_token)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ListToolSpecsResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    // ── Changeset / Workbench ──────────────────────────────────

    async fn list_changesets(&self, query: ListChangesetsQuery) -> Result<ListChangesetsResponse> {
        let resp = self
            .client
            .get(self.url("/changesets"))
            .bearer_auth(&self.jwt_token)
            .query(&query)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ListChangesetsResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn changeset_diff(&self, changeset_id: &str) -> Result<ChangesetDiffResponse> {
        let resp = self
            .client
            .get(self.url(&format!("/changesets/{}/diff", changeset_id)))
            .bearer_auth(&self.jwt_token)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ChangesetDiffResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn changeset_impact(&self, changeset_id: &str) -> Result<ChangesetImpactResponse> {
        let resp = self
            .client
            .get(self.url(&format!("/changesets/{}/impact", changeset_id)))
            .bearer_auth(&self.jwt_token)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ChangesetImpactResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn changeset_gate_preview(&self, changeset_id: &str) -> Result<GatePreviewResponse> {
        let resp = self
            .client
            .post(self.url(&format!("/changesets/{}/gate_preview", changeset_id)))
            .bearer_auth(&self.jwt_token)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<GatePreviewResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn publish_changeset(
        &self,
        _principal: &Principal,
        changeset_id: &str,
    ) -> Result<ChangesetPublishResponse> {
        let resp = self
            .client
            .post(self.url(&format!("/changesets/{}/publish", changeset_id)))
            .bearer_auth(&self.jwt_token)
            .send()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        if !resp.status().is_success() {
            return Err(self.handle_error_response(resp).await);
        }

        resp.json::<ChangesetPublishResponse>()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))
    }

    async fn drain_outbox_for_test(&self) -> Result<()> {
        // HttpClient never drains outbox — that's a server-side concern.
        // The outbox dispatcher runs as a background task in the server.
        Ok(())
    }
}

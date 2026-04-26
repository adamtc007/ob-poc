//! Minimal SemOsClient stub for the agentic scenario harness.
//!
//! Background: since 2026-03-17 (commit cc611d69 "Cut over Sem OS discovery
//! bootstrap flow"), `orchestrator::resolve_sem_reg_verbs` returns
//! `SemOsContextEnvelope::unavailable()` when `ctx.sem_os_client` is None,
//! which triggers `PipelineOutcome::NoAllowedVerbs` — bypassing the verb
//! searcher entirely. The harness `stub.rs` had `sem_os_client: None`,
//! so every scenario short-circuited to NoAllowedVerbs instead of letting
//! the minimal verb searcher return its expected `NoMatch` outcome.
//!
//! This module provides `HarnessSemOsClient` — a deterministic stub that
//! returns a single safe-harbour allowed verb per resolve_context call.
//! That suffices to keep `envelope.deny_all = false` so the orchestrator
//! falls through to the verb searcher's `NoMatch` outcome on gibberish
//! input. Other SemOsClient methods return `Err(unsupported)` since
//! harness scenarios don't exercise them.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sem_os_client::SemOsClient;
use sem_os_core::abac::AccessDecision;
use sem_os_core::context_resolution::{ContextResolutionResponse, ResolutionStage, VerbCandidate};
use sem_os_core::error::SemOsError;
use sem_os_core::principal::Principal;
use sem_os_core::proto::{
    BootstrapSeedBundleResponse, ChangesetDiffResponse, ChangesetImpactResponse,
    ChangesetPublishResponse, ExportSnapshotSetResponse, GatePreviewResponse, GetManifestResponse,
    ListChangesetsQuery, ListChangesetsResponse, ListToolSpecsResponse, ResolveContextRequest,
    ResolveContextResponse, ToolCallRequest, ToolCallResponse,
};
use sem_os_core::types::{Changeset, GovernanceTier, TrustClass};
use uuid::Uuid;

/// Minimal SemOsClient stub for harness scenarios.
///
/// Returns a single safe-harbour allowed verb to keep
/// `envelope.deny_all = false`. The minimal verb searcher then fails to
/// match anything and the orchestrator emits `NoMatch` as scenarios expect.
pub struct HarnessSemOsClient;

impl HarnessSemOsClient {
    /// Build the stub as a trait-object Arc ready to assign to
    /// `OrchestratorContext::sem_os_client`.
    pub fn new_arc() -> Arc<dyn SemOsClient> {
        Arc::new(Self)
    }

    fn unsupported() -> SemOsError {
        SemOsError::InvalidInput("unsupported harness SemOS operation".to_string())
    }
}

impl Default for HarnessSemOsClient {
    fn default() -> Self {
        Self
    }
}

fn safe_harbour_response() -> ContextResolutionResponse {
    ContextResolutionResponse {
        as_of_time: Utc::now(),
        resolved_at: Utc::now(),
        applicable_views: vec![],
        candidate_verbs: vec![VerbCandidate {
            verb_snapshot_id: Uuid::nil(),
            verb_id: Uuid::nil(),
            fqn: "harness.no-op".to_string(),
            description: "Harness safe-harbour verb (deterministic stub)".to_string(),
            governance_tier: GovernanceTier::Operational,
            trust_class: TrustClass::Convenience,
            rank_score: 0.0,
            preconditions_met: true,
            access_decision: AccessDecision::Allow,
            usable_for_proof: false,
        }],
        candidate_attributes: vec![],
        required_preconditions: vec![],
        disambiguation_questions: vec![],
        evidence: Default::default(),
        policy_verdicts: vec![],
        security_handling: AccessDecision::Allow,
        governance_signals: vec![],
        entity_kind_pruned_verbs: vec![],
        confidence: 0.0,
        grounded_action_surface: None,
        resolution_stage: ResolutionStage::Grounded,
        discovery_surface: None,
    }
}

#[async_trait]
impl SemOsClient for HarnessSemOsClient {
    async fn resolve_context(
        &self,
        _principal: &Principal,
        _req: ResolveContextRequest,
    ) -> sem_os_client::Result<ResolveContextResponse> {
        Ok(safe_harbour_response())
    }

    async fn get_manifest(
        &self,
        _snapshot_set_id: &str,
    ) -> sem_os_client::Result<GetManifestResponse> {
        Err(Self::unsupported())
    }

    async fn export_snapshot_set(
        &self,
        _snapshot_set_id: &str,
    ) -> sem_os_client::Result<ExportSnapshotSetResponse> {
        Err(Self::unsupported())
    }

    async fn bootstrap_seed_bundle(
        &self,
        _principal: &Principal,
        _bundle: sem_os_core::seeds::SeedBundle,
    ) -> sem_os_client::Result<BootstrapSeedBundleResponse> {
        Err(Self::unsupported())
    }

    async fn dispatch_tool(
        &self,
        _principal: &Principal,
        _req: ToolCallRequest,
    ) -> sem_os_client::Result<ToolCallResponse> {
        Err(Self::unsupported())
    }

    async fn list_tool_specs(&self) -> sem_os_client::Result<ListToolSpecsResponse> {
        Err(Self::unsupported())
    }

    async fn list_changesets(
        &self,
        _query: ListChangesetsQuery,
    ) -> sem_os_client::Result<ListChangesetsResponse> {
        Ok(ListChangesetsResponse {
            changesets: Vec::<Changeset>::new(),
        })
    }

    async fn changeset_diff(
        &self,
        _changeset_id: &str,
    ) -> sem_os_client::Result<ChangesetDiffResponse> {
        Err(Self::unsupported())
    }

    async fn changeset_impact(
        &self,
        _changeset_id: &str,
    ) -> sem_os_client::Result<ChangesetImpactResponse> {
        Err(Self::unsupported())
    }

    async fn changeset_gate_preview(
        &self,
        _changeset_id: &str,
    ) -> sem_os_client::Result<GatePreviewResponse> {
        Err(Self::unsupported())
    }

    async fn publish_changeset(
        &self,
        _principal: &Principal,
        _changeset_id: &str,
    ) -> sem_os_client::Result<ChangesetPublishResponse> {
        Err(Self::unsupported())
    }

    async fn get_affinity_graph(
        &self,
    ) -> sem_os_client::Result<Arc<sem_os_core::affinity::AffinityGraph>> {
        Err(Self::unsupported())
    }

    async fn drain_outbox_for_test(&self) -> sem_os_client::Result<()> {
        Ok(())
    }
}

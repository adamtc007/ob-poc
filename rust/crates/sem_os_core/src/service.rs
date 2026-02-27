//! CoreService — the central domain service for Semantic OS.
//!
//! Refactored from `rust/src/sem_reg/registry.rs`. Takes port traits via `Arc<dyn PortTrait>`
//! so that the same logic works against Postgres (InProcessClient) or test doubles.
//!
//! `InProcessClient` wraps `Arc<dyn CoreService>` and delegates all calls here.
//! `HttpClient` calls `sem_os_server` over HTTP, which itself holds a CoreService.

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    authoring::{
        bundle::BundleContents,
        governance_verbs::GovernanceVerbService,
        ports::{AuthoringStore, ScratchSchemaRunner},
        types::{
            ChangeSetFull, ChangeSetStatus, DiffSummary, DryRunReport, PublishBatch, PublishPlan,
            ValidationReport,
        },
    },
    context_resolution::{ContextResolutionRequest, ContextResolutionResponse},
    error::SemOsError,
    ports::{
        AuditStore, BootstrapAuditStore, ChangesetStore, EvidenceInstanceStore, ObjectStore,
        OutboxStore, ProjectionWriter, SnapshotStore,
    },
    principal::Principal,
    proto::*,
    seeds::SeedBundle,
    types::*,
};

pub type Result<T> = std::result::Result<T, SemOsError>;

// ── CoreService trait ─────────────────────────────────────────

/// The single service interface that `InProcessClient` delegates to.
///
/// All methods take `&Principal` explicitly — no implicit identity, no thread-local context.
/// Implementations are expected to enforce ABAC and publish gates internally.
#[async_trait]
pub trait CoreService: Send + Sync {
    /// Run the 12-step context resolution pipeline.
    async fn resolve_context(
        &self,
        principal: &Principal,
        req: ContextResolutionRequest,
    ) -> Result<ContextResolutionResponse>;

    /// Get the manifest (list of snapshots) for a snapshot set.
    async fn get_manifest(&self, snapshot_set_id: &str) -> Result<GetManifestResponse>;

    /// Export all snapshots in a snapshot set.
    async fn export_snapshot_set(&self, snapshot_set_id: &str)
        -> Result<ExportSnapshotSetResponse>;

    /// Bootstrap seed data (idempotent — skips already-seeded objects).
    async fn bootstrap_seed_bundle(
        &self,
        principal: &Principal,
        bundle: SeedBundle,
    ) -> Result<BootstrapSeedBundleResponse>;

    /// Dispatch a named MCP tool with JSON arguments.
    async fn dispatch_tool(
        &self,
        principal: &Principal,
        req: crate::proto::ToolCallRequest,
    ) -> Result<crate::proto::ToolCallResponse>;

    /// List all available tool specifications.
    async fn list_tool_specs(&self) -> Result<crate::proto::ListToolSpecsResponse>;

    /// Promote an approved changeset — publish all entries as new snapshots.
    /// Returns the number of snapshots created.
    async fn promote_changeset(&self, principal: &Principal, changeset_id: &str) -> Result<u32>;

    /// List changesets with optional filters.
    async fn list_changesets(
        &self,
        query: crate::proto::ListChangesetsQuery,
    ) -> Result<crate::proto::ListChangesetsResponse>;

    /// Diff a changeset's entries against current active snapshots.
    async fn changeset_diff(
        &self,
        changeset_id: &str,
    ) -> Result<crate::proto::ChangesetDiffResponse>;

    /// Impact analysis — find downstream dependents for each modified FQN.
    async fn changeset_impact(
        &self,
        changeset_id: &str,
    ) -> Result<crate::proto::ChangesetImpactResponse>;

    /// Gate preview — run publish gates against draft entries without persisting.
    async fn changeset_gate_preview(
        &self,
        changeset_id: &str,
    ) -> Result<crate::proto::GatePreviewResponse>;

    // ── Authoring pipeline (governance verbs) ──────────────────

    /// Propose a new ChangeSet from a parsed bundle.
    /// Content-addressed idempotent: if an active ChangeSet with the same hash
    /// exists, returns the existing one.
    async fn authoring_propose(
        &self,
        principal: &Principal,
        bundle: &BundleContents,
    ) -> Result<ChangeSetFull>;

    /// Run Stage 1 (pure) validation. Transitions Draft → Validated or Rejected.
    async fn authoring_validate(&self, change_set_id: Uuid) -> Result<ValidationReport>;

    /// Run Stage 2 (DB-backed) dry-run. Transitions Validated → DryRunPassed or DryRunFailed.
    async fn authoring_dry_run(&self, change_set_id: Uuid) -> Result<DryRunReport>;

    /// Generate a publish plan with blast-radius analysis. Read-only.
    async fn authoring_plan_publish(&self, change_set_id: Uuid) -> Result<PublishPlan>;

    /// Publish a ChangeSet. Transitions DryRunPassed → Published.
    async fn authoring_publish(&self, change_set_id: Uuid, publisher: &str)
        -> Result<PublishBatch>;

    /// Publish multiple ChangeSets atomically in topological order.
    async fn authoring_publish_batch(
        &self,
        change_set_ids: &[Uuid],
        publisher: &str,
    ) -> Result<PublishBatch>;

    /// Compute structural diff between two ChangeSets.
    async fn authoring_diff(&self, base_id: Uuid, target_id: Uuid) -> Result<DiffSummary>;

    /// List ChangeSets with optional status filter.
    async fn authoring_list(
        &self,
        status: Option<ChangeSetStatus>,
        limit: i64,
    ) -> Result<Vec<ChangeSetFull>>;

    /// Get a single ChangeSet by ID.
    async fn authoring_get(&self, change_set_id: Uuid) -> Result<ChangeSetFull>;

    // ── Authoring health ──────────────────────────────────────────

    /// Health check: pending ChangeSets grouped by status.
    async fn authoring_health_pending(
        &self,
    ) -> Result<crate::authoring::types::PendingChangeSetsHealth>;

    /// Health check: ChangeSets with stale dry-run evaluations.
    async fn authoring_health_stale_dryruns(
        &self,
    ) -> Result<crate::authoring::types::StaleDryRunsHealth>;

    /// Run the cleanup process to archive old terminal/orphan ChangeSets.
    async fn authoring_run_cleanup(
        &self,
        policy: &crate::authoring::cleanup::CleanupPolicy,
    ) -> Result<crate::authoring::cleanup::CleanupReport>;

    // ── Bootstrap audit (idempotent seed bundle tracking) ──────

    /// Check if a bootstrap audit record exists for the given bundle hash.
    async fn bootstrap_check(
        &self,
        bundle_hash: &str,
    ) -> Result<Option<(String, Option<uuid::Uuid>)>>;

    /// Insert or update a bootstrap audit record to 'in_progress'.
    async fn bootstrap_start(
        &self,
        bundle_hash: &str,
        actor_id: &str,
        bundle_counts: serde_json::Value,
    ) -> Result<()>;

    /// Mark a bootstrap audit record as 'published'.
    async fn bootstrap_mark_published(&self, bundle_hash: &str) -> Result<()>;

    /// Mark a bootstrap audit record as 'failed' with an error message.
    async fn bootstrap_mark_failed(&self, bundle_hash: &str, error: &str) -> Result<()>;

    /// Test-only: synchronously drain and process all pending outbox events.
    async fn drain_outbox_for_test(&self) -> Result<()>;
}

// ── CoreServiceImpl ───────────────────────────────────────────

/// Concrete implementation holding port trait references.
///
/// Constructed at startup in `ob-poc-web/src/main.rs` (in-process mode) or
/// in `sem_os_server/src/main.rs` (standalone server mode).
pub struct CoreServiceImpl {
    pub snapshots: Arc<dyn SnapshotStore>,
    pub objects: Arc<dyn ObjectStore>,
    pub changesets: Arc<dyn ChangesetStore>,
    pub audit: Arc<dyn AuditStore>,
    pub outbox: Arc<dyn OutboxStore>,
    pub evidence: Arc<dyn EvidenceInstanceStore>,
    pub projections: Arc<dyn ProjectionWriter>,
    pub authoring: Option<Arc<dyn AuthoringStore>>,
    pub scratch_runner: Option<Arc<dyn ScratchSchemaRunner>>,
    pub cleanup: Option<Arc<dyn crate::authoring::cleanup::CleanupStore>>,
    pub bootstrap_audit: Option<Arc<dyn BootstrapAuditStore>>,
}

impl CoreServiceImpl {
    pub fn new(
        snapshots: Arc<dyn SnapshotStore>,
        objects: Arc<dyn ObjectStore>,
        changesets: Arc<dyn ChangesetStore>,
        audit: Arc<dyn AuditStore>,
        outbox: Arc<dyn OutboxStore>,
        evidence: Arc<dyn EvidenceInstanceStore>,
        projections: Arc<dyn ProjectionWriter>,
    ) -> Self {
        Self {
            snapshots,
            objects,
            changesets,
            audit,
            outbox,
            evidence,
            projections,
            authoring: None,
            scratch_runner: None,
            cleanup: None,
            bootstrap_audit: None,
        }
    }

    /// Set the authoring store (builder pattern).
    pub fn with_authoring(mut self, authoring: Arc<dyn AuthoringStore>) -> Self {
        self.authoring = Some(authoring);
        self
    }

    /// Set the scratch schema runner (builder pattern).
    pub fn with_scratch_runner(mut self, runner: Arc<dyn ScratchSchemaRunner>) -> Self {
        self.scratch_runner = Some(runner);
        self
    }

    /// Set the cleanup store (builder pattern).
    pub fn with_cleanup(
        mut self,
        cleanup: Arc<dyn crate::authoring::cleanup::CleanupStore>,
    ) -> Self {
        self.cleanup = Some(cleanup);
        self
    }

    /// Set the bootstrap audit store (builder pattern).
    pub fn with_bootstrap_audit(mut self, store: Arc<dyn BootstrapAuditStore>) -> Self {
        self.bootstrap_audit = Some(store);
        self
    }

    /// Build a `GovernanceVerbService` from the authoring + scratch_runner stores.
    /// Returns an error if either is not configured.
    fn governance_verb_service(&self) -> Result<(&dyn AuthoringStore, &dyn ScratchSchemaRunner)> {
        let authoring = self
            .authoring
            .as_deref()
            .ok_or_else(|| SemOsError::MigrationPending("authoring store not configured".into()))?;
        let scratch = self
            .scratch_runner
            .as_deref()
            .ok_or_else(|| SemOsError::MigrationPending("scratch runner not configured".into()))?;
        Ok((authoring, scratch))
    }
}

#[async_trait]
impl CoreService for CoreServiceImpl {
    async fn resolve_context(
        &self,
        _principal: &Principal,
        _req: ContextResolutionRequest,
    ) -> Result<ContextResolutionResponse> {
        // The full 12-step pipeline exists in the monolith
        // (rust/src/sem_reg/context_resolution.rs) and is invoked via the
        // PgPool-based path in agent/orchestrator.rs. Wiring it here requires
        // additional port trait methods (BulkLoadStore) — tracked separately.
        Err(SemOsError::MigrationPending(
            "context resolution pipeline not yet wired to standalone service \
             (requires BulkLoadStore port for Steps 1-2c)"
                .into(),
        ))
    }

    async fn get_manifest(&self, snapshot_set_id: &str) -> Result<GetManifestResponse> {
        let set_id = parse_uuid(snapshot_set_id, "snapshot_set_id")?;
        let manifest = self.snapshots.get_manifest(&SnapshotSetId(set_id)).await?;
        Ok(GetManifestResponse {
            snapshot_set_id: manifest.snapshot_set_id.0.to_string(),
            published_at: manifest.published_at.to_rfc3339(),
            entries: manifest
                .entries
                .into_iter()
                .map(|e| ManifestEntry {
                    snapshot_id: e.snapshot_id.0.to_string(),
                    object_type: e.object_type,
                    fqn: e.fqn.0,
                    content_hash: e.content_hash,
                })
                .collect(),
        })
    }

    async fn export_snapshot_set(
        &self,
        snapshot_set_id: &str,
    ) -> Result<ExportSnapshotSetResponse> {
        let set_id = parse_uuid(snapshot_set_id, "snapshot_set_id")?;
        let exports = self.snapshots.export(&SnapshotSetId(set_id)).await?;
        Ok(ExportSnapshotSetResponse {
            snapshots: exports
                .into_iter()
                .map(|e| ExportedSnapshot {
                    snapshot_id: e.snapshot_id.0.to_string(),
                    fqn: e.fqn.0,
                    object_type: e.object_type,
                    payload: e.payload,
                })
                .collect(),
        })
    }

    async fn bootstrap_seed_bundle(
        &self,
        principal: &Principal,
        bundle: SeedBundle,
    ) -> Result<BootstrapSeedBundleResponse> {
        // v3.3 compliant bootstrap:
        // 1. Collect all missing seeds into a Vec<(SnapshotMeta, Value)>.
        // 2. Publish the entire batch as one snapshot set with one outbox event.
        let mut to_publish: Vec<(SnapshotMeta, serde_json::Value)> = Vec::new();
        let mut skipped = 0u32;

        // Helper: check if FQN already exists, if not queue for batch publish.
        let all_seeds: Vec<(ObjectType, &str, &serde_json::Value)> = bundle
            .verb_contracts
            .iter()
            .map(|s| (ObjectType::VerbContract, s.fqn.as_str(), &s.payload))
            .chain(
                bundle
                    .attributes
                    .iter()
                    .map(|s| (ObjectType::AttributeDef, s.fqn.as_str(), &s.payload)),
            )
            .chain(
                bundle
                    .entity_types
                    .iter()
                    .map(|s| (ObjectType::EntityTypeDef, s.fqn.as_str(), &s.payload)),
            )
            .chain(
                bundle
                    .taxonomies
                    .iter()
                    .map(|s| (ObjectType::TaxonomyDef, s.fqn.as_str(), &s.payload)),
            )
            .chain(
                bundle
                    .policies
                    .iter()
                    .map(|s| (ObjectType::PolicyRule, s.fqn.as_str(), &s.payload)),
            )
            .chain(
                bundle
                    .views
                    .iter()
                    .map(|s| (ObjectType::ViewDef, s.fqn.as_str(), &s.payload)),
            )
            .chain(
                bundle
                    .derivation_specs
                    .iter()
                    .map(|s| (ObjectType::DerivationSpec, s.fqn.as_str(), &s.payload)),
            )
            .collect();

        for (object_type, fqn, payload) in &all_seeds {
            let fqn_obj = Fqn::new(*fqn);
            match self.snapshots.resolve(&fqn_obj, None).await {
                Ok(_) => {
                    skipped += 1;
                }
                Err(SemOsError::NotFound(_)) => {
                    let object_id = crate::ids::object_id_for(*object_type, fqn);
                    let meta =
                        SnapshotMeta::new_operational(*object_type, object_id, &principal.actor_id);
                    to_publish.push((meta, (*payload).clone()));
                }
                Err(e) => return Err(e),
            }
        }

        let created = to_publish.len() as u32;
        let mut snapshot_set_id_str: Option<String> = None;

        if !to_publish.is_empty() {
            // Create a single snapshot set for the entire bootstrap batch.
            let set_id = self
                .snapshots
                .publish(
                    principal,
                    PublishInput {
                        payload: serde_json::json!({
                            "source": "bootstrap_seed_bundle",
                            "bundle_hash": &bundle.bundle_hash,
                        }),
                    },
                )
                .await?;
            let correlation_id = Uuid::new_v4();

            // Atomic batch publish: all snapshots + exactly one outbox event.
            self.snapshots
                .publish_batch_into_set(to_publish, set_id.0, correlation_id)
                .await?;

            snapshot_set_id_str = Some(set_id.0.to_string());
        }

        // Audit the bootstrap
        self.audit
            .append(
                principal,
                AuditEntry {
                    action: "bootstrap_seed_bundle".into(),
                    details: serde_json::json!({
                        "bundle_hash": bundle.bundle_hash,
                        "created": created,
                        "skipped": skipped,
                        "snapshot_set_id": snapshot_set_id_str,
                    }),
                },
            )
            .await?;

        Ok(BootstrapSeedBundleResponse {
            created,
            skipped,
            bundle_hash: bundle.bundle_hash,
            snapshot_set_id: snapshot_set_id_str,
        })
    }

    async fn dispatch_tool(
        &self,
        _principal: &Principal,
        _req: crate::proto::ToolCallRequest,
    ) -> Result<crate::proto::ToolCallResponse> {
        // Tool dispatch requires the ob-poc host's sem_reg::agent::mcp_tools.
        // InProcessClient overrides this; HttpClient calls the server endpoint.
        Err(SemOsError::MigrationPending(
            "tool dispatch requires ob-poc host (InProcessClient overrides this method)".into(),
        ))
    }

    async fn list_tool_specs(&self) -> Result<crate::proto::ListToolSpecsResponse> {
        // Tool specs are provided by ob-poc's all_tool_specs().
        // InProcessClient overrides this; HttpClient calls the server endpoint.
        Err(SemOsError::MigrationPending(
            "tool specs require ob-poc host (InProcessClient overrides this method)".into(),
        ))
    }

    async fn promote_changeset(&self, principal: &Principal, changeset_id: &str) -> Result<u32> {
        let cs_id = parse_uuid(changeset_id, "changeset_id")?;

        // 1. Load changeset and verify status is 'approved'.
        let changeset = self.changesets.get_changeset(cs_id).await?;
        if changeset.status != ChangesetStatus::Approved {
            return Err(SemOsError::Conflict(format!(
                "changeset {} is in '{}' status — only 'approved' changesets can be promoted",
                cs_id, changeset.status
            )));
        }

        // 2. Load all entries.
        let entries = self.changesets.list_entries(cs_id).await?;
        if entries.is_empty() {
            return Err(SemOsError::InvalidInput(format!(
                "changeset {} has no entries to promote",
                cs_id
            )));
        }

        // 2b. Stewardship guardrails: role constraints + proof chain.
        crate::stewardship::validate_role_constraints(principal, &entries)?;
        crate::stewardship::check_proof_chain_compatibility(&entries, self.snapshots.as_ref())
            .await?;

        // 3. Stale detection + predecessor resolution: for each entry with a
        //    base_snapshot_id, verify it still matches the current active snapshot.
        //    Collect resolved predecessors for version derivation in step 5.
        let mut predecessors: std::collections::HashMap<Uuid, SnapshotRow> =
            std::collections::HashMap::new();
        for entry in &entries {
            if let Some(base_id) = entry.base_snapshot_id {
                let fqn = Fqn::new(&entry.object_fqn);
                match self.snapshots.resolve(&fqn, None).await {
                    Ok(current) => {
                        if current.snapshot_id != base_id {
                            return Err(SemOsError::Conflict(format!(
                                "stale draft: FQN {} — base_snapshot_id {} does not match \
                                 current active snapshot {}",
                                entry.object_fqn, base_id, current.snapshot_id
                            )));
                        }
                        predecessors.insert(entry.entry_id, current);
                    }
                    Err(SemOsError::NotFound(_)) => {
                        return Err(SemOsError::Conflict(format!(
                            "stale draft: FQN {} — base_snapshot_id {} no longer has an \
                             active snapshot",
                            entry.object_fqn, base_id
                        )));
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // 4. Create a snapshot set for this promotion.
        let set_id = self
            .snapshots
            .publish(
                principal,
                PublishInput {
                    payload: serde_json::json!({
                        "promotion_source": "changeset",
                        "changeset_id": cs_id,
                    }),
                },
            )
            .await?;
        let correlation_id = Uuid::new_v4();

        // 5. Build batch of snapshots for atomic publish (v3.3: one outbox event).
        let mut items: Vec<(SnapshotMeta, serde_json::Value)> = Vec::with_capacity(entries.len());
        for entry in &entries {
            let object_type = entry.object_type;

            let object_id = crate::ids::object_id_for(object_type, &entry.object_fqn);

            let change_type = match entry.change_kind {
                ChangeKind::Add => ChangeType::Created,
                ChangeKind::Modify => ChangeType::NonBreaking,
                ChangeKind::Remove => ChangeType::Retirement,
            };

            let status = match entry.change_kind {
                ChangeKind::Remove => SnapshotStatus::Retired,
                _ => SnapshotStatus::Active,
            };

            // Derive version from predecessor (resolved in step 3).
            let (version_major, version_minor) =
                if let Some(pred) = predecessors.get(&entry.entry_id) {
                    match entry.change_kind {
                        ChangeKind::Add => (1, 0),
                        ChangeKind::Modify => (pred.version_major, pred.version_minor + 1),
                        ChangeKind::Remove => (pred.version_major, pred.version_minor),
                    }
                } else {
                    (1, 0)
                };

            let meta = SnapshotMeta {
                object_type,
                object_id,
                version_major,
                version_minor,
                status,
                governance_tier: GovernanceTier::Operational,
                trust_class: TrustClass::Convenience,
                security_label: SecurityLabel::default(),
                change_type,
                change_rationale: Some(format!("Promoted from changeset {}", cs_id)),
                created_by: principal.actor_id.clone(),
                approved_by: Some(principal.actor_id.clone()),
                predecessor_id: entry.base_snapshot_id,
            };

            items.push((meta, entry.draft_payload.clone()));
        }

        let created = items.len() as u32;
        self.snapshots
            .publish_batch_into_set(items, set_id.0, correlation_id)
            .await?;

        // 6. Update changeset status to 'published'.
        self.changesets
            .update_status(cs_id, ChangesetStatus::Published)
            .await?;

        // 7. Audit the promotion.
        self.audit
            .append(
                principal,
                AuditEntry {
                    action: "promote_changeset".into(),
                    details: serde_json::json!({
                        "changeset_id": cs_id,
                        "snapshot_set_id": set_id.0,
                        "snapshots_created": created,
                    }),
                },
            )
            .await?;

        Ok(created)
    }

    async fn list_changesets(
        &self,
        query: crate::proto::ListChangesetsQuery,
    ) -> Result<crate::proto::ListChangesetsResponse> {
        let changesets = self
            .changesets
            .list_changesets(
                query.status.as_deref(),
                query.owner.as_deref(),
                query.scope.as_deref(),
            )
            .await?;
        Ok(crate::proto::ListChangesetsResponse { changesets })
    }

    async fn changeset_diff(
        &self,
        changeset_id: &str,
    ) -> Result<crate::proto::ChangesetDiffResponse> {
        let cs_id = parse_uuid(changeset_id, "changeset_id")?;
        let changeset = self.changesets.get_changeset(cs_id).await?;
        let entries = self.changesets.list_entries(cs_id).await?;

        let mut diff_entries = Vec::with_capacity(entries.len());
        let mut stale_count = 0u32;

        for entry in &entries {
            let fqn = Fqn::new(&entry.object_fqn);
            let current = self.snapshots.resolve(&fqn, None).await.ok();

            let current_snapshot_id = current.as_ref().map(|r| r.snapshot_id.to_string());
            let current_payload = current.as_ref().map(|r| r.definition.clone());

            let is_stale = match (entry.base_snapshot_id, current.as_ref()) {
                (Some(base_id), Some(curr)) => base_id != curr.snapshot_id,
                (Some(_), None) => true, // base existed but object was removed
                (None, _) => false,      // new addition — never stale
            };

            if is_stale {
                stale_count += 1;
            }

            diff_entries.push(crate::proto::DiffEntry {
                entry_id: entry.entry_id.to_string(),
                object_fqn: entry.object_fqn.clone(),
                object_type: entry.object_type,
                change_kind: entry.change_kind.to_string(),
                base_snapshot_id: entry.base_snapshot_id.map(|id| id.to_string()),
                current_snapshot_id,
                is_stale,
                draft_payload: entry.draft_payload.clone(),
                current_payload,
            });
        }

        Ok(crate::proto::ChangesetDiffResponse {
            changeset_id: cs_id.to_string(),
            status: changeset.status.to_string(),
            entries: diff_entries,
            stale_count,
        })
    }

    async fn changeset_impact(
        &self,
        changeset_id: &str,
    ) -> Result<crate::proto::ChangesetImpactResponse> {
        let cs_id = parse_uuid(changeset_id, "changeset_id")?;
        let _changeset = self.changesets.get_changeset(cs_id).await?;
        let entries = self.changesets.list_entries(cs_id).await?;

        // Real JSONB dependency traversal: for each modified FQN, search active
        // snapshots whose definition JSON references that FQN.
        let mut impacts = Vec::new();

        for entry in &entries {
            let source_fqn = &entry.object_fqn;
            let source_type = &entry.object_type;

            let dependents = self
                .snapshots
                .find_dependents(source_fqn, 100)
                .await
                .unwrap_or_default();

            for dep in dependents {
                let relationship = match dep.object_type {
                    ObjectType::ViewDef => "surfaces",
                    ObjectType::PolicyRule => "governed_by",
                    ObjectType::VerbContract => "references",
                    ObjectType::EntityTypeDef => "uses_attribute",
                    ObjectType::TaxonomyNode => "member_of",
                    _ => "depends_on",
                };

                impacts.push(crate::proto::ImpactEntry {
                    source_fqn: source_fqn.clone(),
                    source_object_type: *source_type,
                    dependent_fqn: dep.fqn,
                    dependent_object_type: dep.object_type,
                    relationship: relationship.into(),
                });
            }
        }

        Ok(crate::proto::ChangesetImpactResponse {
            changeset_id: cs_id.to_string(),
            impacts,
        })
    }

    async fn changeset_gate_preview(
        &self,
        changeset_id: &str,
    ) -> Result<crate::proto::GatePreviewResponse> {
        let cs_id = parse_uuid(changeset_id, "changeset_id")?;
        let _changeset = self.changesets.get_changeset(cs_id).await?;
        let entries = self.changesets.list_entries(cs_id).await?;

        let mut gate_results = Vec::new();
        let mut total_errors = 0u32;
        let mut total_warnings = 0u32;

        // Stewardship: proof chain compatibility check
        if let Err(e) =
            crate::stewardship::check_proof_chain_compatibility(&entries, self.snapshots.as_ref())
                .await
        {
            gate_results.push(crate::proto::GatePreviewEntry {
                entry_id: String::new(),
                object_fqn: String::new(),
                gate_name: "proof_chain_compatibility".into(),
                severity: "error".into(),
                passed: false,
                reason: Some(e.to_string()),
            });
            total_errors += 1;
        }

        // Stewardship: stale draft detection
        if let Ok(stale) =
            crate::stewardship::detect_stale_drafts(&entries, self.snapshots.as_ref()).await
        {
            for conflict in &stale {
                gate_results.push(crate::proto::GatePreviewEntry {
                    entry_id: conflict.entry_id.to_string(),
                    object_fqn: conflict.object_fqn.clone(),
                    gate_name: "stale_draft_detection".into(),
                    severity: "warning".into(),
                    passed: false,
                    reason: Some(format!(
                        "base_snapshot_id {} does not match current active {}",
                        conflict.base_snapshot_id, conflict.current_snapshot_id,
                    )),
                });
                total_warnings += 1;
            }
        }

        for entry in &entries {
            let object_type = entry.object_type;

            let object_id = crate::ids::object_id_for(object_type, &entry.object_fqn);

            // Build SnapshotMeta from the draft entry
            let meta = SnapshotMeta {
                object_type,
                object_id,
                version_major: 1,
                version_minor: 0,
                status: match entry.change_kind {
                    ChangeKind::Remove => SnapshotStatus::Retired,
                    _ => SnapshotStatus::Active,
                },
                governance_tier: GovernanceTier::Operational,
                trust_class: TrustClass::Convenience,
                security_label: SecurityLabel::default(),
                change_type: match entry.change_kind {
                    ChangeKind::Add => ChangeType::Created,
                    ChangeKind::Modify => ChangeType::NonBreaking,
                    ChangeKind::Remove => ChangeType::Retirement,
                },
                change_rationale: Some(format!("Gate preview for changeset {}", cs_id)),
                created_by: "gate_preview".into(),
                approved_by: Some("gate_preview".into()),
                predecessor_id: entry.base_snapshot_id,
            };

            // Load predecessor if it exists
            let predecessor = if entry.base_snapshot_id.is_some() {
                let fqn = Fqn::new(&entry.object_fqn);
                self.snapshots.resolve(&fqn, None).await.ok()
            } else {
                None
            };

            // Run the simple 4-gate pipeline
            let gate_result = crate::gates::evaluate_publish_gates(&meta, predecessor.as_ref());

            for gr in &gate_result.results {
                if !gr.passed {
                    total_errors += 1;
                }
                gate_results.push(crate::proto::GatePreviewEntry {
                    entry_id: entry.entry_id.to_string(),
                    object_fqn: entry.object_fqn.clone(),
                    gate_name: gr.gate_name.to_string(),
                    severity: if gr.passed {
                        "info".into()
                    } else {
                        "error".into()
                    },
                    passed: gr.passed,
                    reason: gr.reason.clone(),
                });
            }
        }

        Ok(crate::proto::GatePreviewResponse {
            changeset_id: cs_id.to_string(),
            would_block: total_errors > 0,
            error_count: total_errors,
            warning_count: total_warnings,
            gate_results,
        })
    }

    // ── Authoring pipeline implementations ─────────────────────

    async fn authoring_propose(
        &self,
        principal: &Principal,
        bundle: &BundleContents,
    ) -> Result<ChangeSetFull> {
        let (authoring, scratch) = self.governance_verb_service()?;
        let svc = GovernanceVerbService::new(authoring, scratch);
        svc.propose(bundle, principal).await
    }

    async fn authoring_validate(&self, change_set_id: Uuid) -> Result<ValidationReport> {
        let (authoring, scratch) = self.governance_verb_service()?;
        let svc = GovernanceVerbService::new(authoring, scratch);
        svc.validate(change_set_id).await
    }

    async fn authoring_dry_run(&self, change_set_id: Uuid) -> Result<DryRunReport> {
        let (authoring, scratch) = self.governance_verb_service()?;
        let svc = GovernanceVerbService::new(authoring, scratch);
        svc.dry_run(change_set_id).await
    }

    async fn authoring_plan_publish(&self, change_set_id: Uuid) -> Result<PublishPlan> {
        let (authoring, scratch) = self.governance_verb_service()?;
        let svc = GovernanceVerbService::new(authoring, scratch);
        svc.plan_publish(change_set_id).await
    }

    async fn authoring_publish(
        &self,
        change_set_id: Uuid,
        publisher: &str,
    ) -> Result<PublishBatch> {
        let (authoring, scratch) = self.governance_verb_service()?;
        let svc = GovernanceVerbService::new(authoring, scratch);
        svc.publish(change_set_id, publisher).await
    }

    async fn authoring_publish_batch(
        &self,
        change_set_ids: &[Uuid],
        publisher: &str,
    ) -> Result<PublishBatch> {
        let (authoring, scratch) = self.governance_verb_service()?;
        let svc = GovernanceVerbService::new(authoring, scratch);
        svc.publish_batch(change_set_ids, publisher).await
    }

    async fn authoring_diff(&self, base_id: Uuid, target_id: Uuid) -> Result<DiffSummary> {
        let (authoring, scratch) = self.governance_verb_service()?;
        let svc = GovernanceVerbService::new(authoring, scratch);
        svc.diff(base_id, target_id).await
    }

    async fn authoring_list(
        &self,
        status: Option<ChangeSetStatus>,
        limit: i64,
    ) -> Result<Vec<ChangeSetFull>> {
        let authoring = self
            .authoring
            .as_deref()
            .ok_or_else(|| SemOsError::MigrationPending("authoring store not configured".into()))?;
        authoring.list_change_sets(status, limit).await
    }

    async fn authoring_get(&self, change_set_id: Uuid) -> Result<ChangeSetFull> {
        let authoring = self
            .authoring
            .as_deref()
            .ok_or_else(|| SemOsError::MigrationPending("authoring store not configured".into()))?;
        authoring.get_change_set(change_set_id).await
    }

    async fn authoring_health_pending(
        &self,
    ) -> Result<crate::authoring::types::PendingChangeSetsHealth> {
        let authoring = self
            .authoring
            .as_deref()
            .ok_or_else(|| SemOsError::MigrationPending("authoring store not configured".into()))?;
        let status_counts = authoring.count_by_status().await?;
        let mut counts = Vec::new();
        let mut total_pending: i64 = 0;
        for (status, count) in &status_counts {
            counts.push(crate::authoring::types::StatusCount {
                status: status.to_string(),
                count: *count,
            });
            if !status.is_terminal() {
                total_pending += count;
            }
        }
        Ok(crate::authoring::types::PendingChangeSetsHealth {
            counts,
            total_pending,
        })
    }

    async fn authoring_health_stale_dryruns(
        &self,
    ) -> Result<crate::authoring::types::StaleDryRunsHealth> {
        let authoring = self
            .authoring
            .as_deref()
            .ok_or_else(|| SemOsError::MigrationPending("authoring store not configured".into()))?;
        let stale = authoring.find_stale_dry_runs().await?;
        let stale_change_set_ids: Vec<Uuid> = stale.iter().map(|cs| cs.change_set_id).collect();
        Ok(crate::authoring::types::StaleDryRunsHealth {
            stale_count: stale_change_set_ids.len() as i64,
            stale_change_set_ids,
        })
    }

    async fn authoring_run_cleanup(
        &self,
        policy: &crate::authoring::cleanup::CleanupPolicy,
    ) -> Result<crate::authoring::cleanup::CleanupReport> {
        let cleanup = self
            .cleanup
            .as_ref()
            .ok_or_else(|| SemOsError::MigrationPending("cleanup store not configured".into()))?;
        crate::authoring::cleanup::run_cleanup(cleanup.as_ref(), policy)
            .await
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("{e}")))
    }

    // ── Bootstrap audit implementations ────────────────────────

    async fn bootstrap_check(
        &self,
        bundle_hash: &str,
    ) -> Result<Option<(String, Option<uuid::Uuid>)>> {
        let store = self.bootstrap_audit.as_deref().ok_or_else(|| {
            SemOsError::MigrationPending("bootstrap audit store not configured".into())
        })?;
        store.check_bootstrap(bundle_hash).await
    }

    async fn bootstrap_start(
        &self,
        bundle_hash: &str,
        actor_id: &str,
        bundle_counts: serde_json::Value,
    ) -> Result<()> {
        let store = self.bootstrap_audit.as_deref().ok_or_else(|| {
            SemOsError::MigrationPending("bootstrap audit store not configured".into())
        })?;
        store
            .start_bootstrap(bundle_hash, actor_id, bundle_counts)
            .await
    }

    async fn bootstrap_mark_published(&self, bundle_hash: &str) -> Result<()> {
        let store = self.bootstrap_audit.as_deref().ok_or_else(|| {
            SemOsError::MigrationPending("bootstrap audit store not configured".into())
        })?;
        store.mark_published(bundle_hash).await
    }

    async fn bootstrap_mark_failed(&self, bundle_hash: &str, error: &str) -> Result<()> {
        let store = self.bootstrap_audit.as_deref().ok_or_else(|| {
            SemOsError::MigrationPending("bootstrap audit store not configured".into())
        })?;
        store.mark_failed(bundle_hash, error).await
    }

    async fn drain_outbox_for_test(&self) -> Result<()> {
        // Claim and process all pending outbox events.
        // Each event triggers the ProjectionWriter to update the active snapshot set.
        let claimer_id = "drain_outbox_for_test";
        while let Some(event) = self.outbox.claim_next(claimer_id).await? {
            match self.projections.write_active_snapshot_set(&event).await {
                Ok(()) => {
                    self.outbox.mark_processed(&event.event_id).await?;
                }
                Err(e) => {
                    let error_msg = format!("{e}");
                    // In test mode, dead-letter immediately and propagate the error.
                    self.outbox
                        .mark_dead_letter(&event.event_id, &error_msg)
                        .await?;
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────

fn parse_uuid(s: &str, field_name: &str) -> Result<Uuid> {
    Uuid::parse_str(s)
        .map_err(|_| SemOsError::InvalidInput(format!("invalid UUID for {field_name}: {s}")))
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uuid_valid() {
        let id = Uuid::new_v4();
        let result = parse_uuid(&id.to_string(), "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);
    }

    #[test]
    fn test_parse_uuid_invalid() {
        let result = parse_uuid("not-a-uuid", "test");
        assert!(result.is_err());
        match result.unwrap_err() {
            SemOsError::InvalidInput(msg) => {
                assert!(msg.contains("test"));
                assert!(msg.contains("not-a-uuid"));
            }
            other => panic!("Expected InvalidInput, got {other:?}"),
        }
    }
}

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
    context_resolution::{ContextResolutionRequest, ContextResolutionResponse},
    error::SemOsError,
    ports::{
        AuditStore, ChangesetStore, EvidenceInstanceStore, ObjectStore, OutboxStore,
        ProjectionWriter, SnapshotStore,
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
}

impl CoreServiceImpl {
    pub fn new(
        snapshots: Arc<dyn SnapshotStore>,
        objects: Arc<dyn ObjectStore>,
        audit: Arc<dyn AuditStore>,
        outbox: Arc<dyn OutboxStore>,
        evidence: Arc<dyn EvidenceInstanceStore>,
        projections: Arc<dyn ProjectionWriter>,
    ) -> Self {
        // Legacy 6-arg constructor — creates a no-op changeset store.
        // Use `with_changesets()` to add a real changeset store.
        Self {
            snapshots,
            objects,
            changesets: Arc::new(NoopChangesetStore),
            audit,
            outbox,
            evidence,
            projections,
        }
    }

    /// Set the changeset store (builder pattern).
    pub fn with_changesets(mut self, changesets: Arc<dyn ChangesetStore>) -> Self {
        self.changesets = changesets;
        self
    }
}

/// No-op changeset store for backward compatibility with 6-arg constructor.
struct NoopChangesetStore;

#[async_trait]
impl ChangesetStore for NoopChangesetStore {
    async fn create_changeset(&self, _: CreateChangesetInput) -> crate::ports::Result<Changeset> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
    async fn get_changeset(&self, _: Uuid) -> crate::ports::Result<Changeset> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
    async fn list_changesets(
        &self,
        _: Option<&str>,
        _: Option<&str>,
        _: Option<&str>,
    ) -> crate::ports::Result<Vec<Changeset>> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
    async fn update_status(&self, _: Uuid, _: ChangesetStatus) -> crate::ports::Result<()> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
    async fn add_entry(
        &self,
        _: Uuid,
        _: AddChangesetEntryInput,
    ) -> crate::ports::Result<ChangesetEntry> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
    async fn list_entries(&self, _: Uuid) -> crate::ports::Result<Vec<ChangesetEntry>> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
    async fn submit_review(
        &self,
        _: Uuid,
        _: SubmitReviewInput,
    ) -> crate::ports::Result<ChangesetReview> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
    async fn list_reviews(&self, _: Uuid) -> crate::ports::Result<Vec<ChangesetReview>> {
        Err(SemOsError::MigrationPending(
            "changesets not configured".into(),
        ))
    }
}

#[async_trait]
impl CoreService for CoreServiceImpl {
    async fn resolve_context(
        &self,
        _principal: &Principal,
        req: ContextResolutionRequest,
    ) -> Result<ContextResolutionResponse> {
        // The full 12-step pipeline requires DB-side loading (Steps 1, 2, 2b, 2c)
        // followed by pure scoring/filtering (Steps 3-12) from context_resolution.rs.
        //
        // Stage 1.3 wires the ports; the full pipeline orchestration is wired in
        // Stage 1.5 once the adapter provides the DB loading functions.
        // For now, delegate to the snapshot store for the data-loading steps
        // and return a minimal response proving the wiring works.

        let as_of = req.point_in_time.unwrap_or_else(chrono::Utc::now);

        Ok(ContextResolutionResponse {
            as_of_time: as_of,
            resolved_at: chrono::Utc::now(),
            applicable_views: vec![],
            candidate_verbs: vec![],
            candidate_attributes: vec![],
            required_preconditions: vec![],
            disambiguation_questions: vec![],
            evidence: Default::default(),
            policy_verdicts: vec![],
            security_handling: crate::abac::AccessDecision::Allow,
            governance_signals: vec![],
            confidence: 0.0,
        })
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
        req: crate::proto::ToolCallRequest,
    ) -> Result<crate::proto::ToolCallResponse> {
        // CoreServiceImpl does not host tool dispatch logic directly —
        // the InProcessClient delegates to ob-poc's sem_reg::agent::mcp_tools,
        // and the HttpClient calls the server endpoint.
        // This stub exists to satisfy the trait bound.
        Ok(crate::proto::ToolCallResponse {
            success: false,
            data: serde_json::Value::Null,
            error: Some(format!(
                "Tool dispatch not available via CoreService stub: {}",
                req.tool_name
            )),
        })
    }

    async fn list_tool_specs(&self) -> Result<crate::proto::ListToolSpecsResponse> {
        // Stub — InProcessClient overrides with ob-poc's all_tool_specs().
        Ok(crate::proto::ListToolSpecsResponse { tools: vec![] })
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

        // 3. Stale detection: for each entry with a base_snapshot_id,
        //    verify it still matches the current active snapshot for that FQN.
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
                    }
                    Err(SemOsError::NotFound(_)) => {
                        // The base snapshot was active when the entry was created,
                        // but has since been removed/retired — stale.
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
            let object_type = ObjectType::from_str(&entry.object_type).ok_or_else(|| {
                SemOsError::InvalidInput(format!(
                    "unknown object_type '{}' in changeset entry {}",
                    entry.object_type, entry.entry_id
                ))
            })?;

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

            let meta = SnapshotMeta {
                object_type,
                object_id,
                version_major: 1, // TODO: derive from predecessor
                version_minor: 0,
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
                object_type: entry.object_type.clone(),
                change_kind: entry.change_kind.as_str().to_string(),
                base_snapshot_id: entry.base_snapshot_id.map(|id| id.to_string()),
                current_snapshot_id,
                is_stale,
                draft_payload: entry.draft_payload.clone(),
                current_payload,
            });
        }

        Ok(crate::proto::ChangesetDiffResponse {
            changeset_id: cs_id.to_string(),
            status: changeset.status.as_str().to_string(),
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
                let relationship = match dep.object_type.as_str() {
                    "view_def" => "surfaces",
                    "policy_rule" => "governed_by",
                    "verb_contract" => "references",
                    "entity_type_def" => "uses_attribute",
                    "taxonomy_node" => "member_of",
                    _ => "depends_on",
                };

                impacts.push(crate::proto::ImpactEntry {
                    source_fqn: source_fqn.clone(),
                    source_object_type: source_type.clone(),
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
            let object_type = match ObjectType::from_str(&entry.object_type) {
                Some(t) => t,
                None => {
                    gate_results.push(crate::proto::GatePreviewEntry {
                        entry_id: entry.entry_id.to_string(),
                        object_fqn: entry.object_fqn.clone(),
                        gate_name: "object_type_valid".into(),
                        severity: "error".into(),
                        passed: false,
                        reason: Some(format!("Unknown object_type '{}'", entry.object_type)),
                    });
                    total_errors += 1;
                    continue;
                }
            };

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

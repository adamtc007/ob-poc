//! Postgres implementations of all sem_os_core port traits.
//!
//! Each adapter is a newtype wrapping PgPool. All SQL is runtime-checked
//! (sqlx::query, not sqlx::query!) to avoid compile-time DB requirement.

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use sem_os_core::error::SemOsError;
use sem_os_core::ports::{
    AuditStore, BootstrapAuditStore, ChangesetStore, EvidenceInstanceStore, ObjectStore,
    OutboxStore, ProjectionWriter, Result, SnapshotStore,
};
use sem_os_core::principal::Principal;
use sem_os_core::types::*;

use crate::sqlx_types::PgSnapshotRow;

// ── PgSnapshotStore ───────────────────────────────────────────

/// Postgres-backed snapshot store.
/// Migrated from `rust/src/sem_reg/store.rs`.
pub struct PgSnapshotStore {
    pool: PgPool,
}

impl PgSnapshotStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Resolve the currently active snapshot for an object.
    pub async fn resolve_active(
        pool: &PgPool,
        object_type: ObjectType,
        object_id: Uuid,
    ) -> std::result::Result<Option<SnapshotRow>, SemOsError> {
        let row = sqlx::query_as::<_, PgSnapshotRow>(
            r#"
            SELECT snapshot_id, snapshot_set_id,
                   object_type::text, object_id,
                   version_major, version_minor,
                   status::text, governance_tier::text, trust_class::text,
                   security_label, effective_from, effective_until,
                   predecessor_id, change_type::text, change_rationale,
                   created_by, approved_by, definition, created_at
            FROM sem_reg.snapshots
            WHERE object_type = $1::sem_reg.object_type
              AND object_id = $2
              AND status = 'active'
              AND effective_until IS NULL
            ORDER BY effective_from DESC
            LIMIT 1
            "#,
        )
        .bind(object_type.as_ref())
        .bind(object_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow!(e))?;
        row.map(|r| {
            r.try_into()
                .map_err(|e: String| SemOsError::Internal(anyhow!(e)))
        })
        .transpose()
    }

    /// Find an active snapshot by a JSONB definition field.
    pub async fn find_active_by_definition_field(
        pool: &PgPool,
        object_type: ObjectType,
        field_name: &str,
        field_value: &str,
    ) -> std::result::Result<Option<SnapshotRow>, SemOsError> {
        let query = format!(
            r#"
            SELECT snapshot_id, snapshot_set_id,
                   object_type::text, object_id,
                   version_major, version_minor,
                   status::text, governance_tier::text, trust_class::text,
                   security_label, effective_from, effective_until,
                   predecessor_id, change_type::text, change_rationale,
                   created_by, approved_by, definition, created_at
            FROM sem_reg.snapshots
            WHERE object_type = $1::sem_reg.object_type
              AND status = 'active'
              AND effective_until IS NULL
              AND definition->>'{field_name}' = $2
            ORDER BY effective_from DESC
            LIMIT 1
            "#,
        );
        let row = sqlx::query_as::<_, PgSnapshotRow>(&query)
            .bind(object_type.as_ref())
            .bind(field_value)
            .fetch_optional(pool)
            .await
            .map_err(|e| anyhow!(e))?;
        row.map(|r| {
            r.try_into()
                .map_err(|e: String| SemOsError::Internal(anyhow!(e)))
        })
        .transpose()
    }

    /// List active snapshots of a given object type.
    pub async fn list_active(
        pool: &PgPool,
        object_type: ObjectType,
        limit: i64,
        offset: i64,
    ) -> std::result::Result<Vec<SnapshotRow>, SemOsError> {
        let rows = sqlx::query_as::<_, PgSnapshotRow>(
            r#"
            SELECT snapshot_id, snapshot_set_id,
                   object_type::text, object_id,
                   version_major, version_minor,
                   status::text, governance_tier::text, trust_class::text,
                   security_label, effective_from, effective_until,
                   predecessor_id, change_type::text, change_rationale,
                   created_by, approved_by, definition, created_at
            FROM sem_reg.snapshots
            WHERE object_type = $1::sem_reg.object_type
              AND status = 'active'
              AND effective_until IS NULL
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(object_type.as_ref())
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!(e))?;
        rows.into_iter()
            .map(|r| {
                r.try_into()
                    .map_err(|e: String| SemOsError::Internal(anyhow!(e)))
            })
            .collect()
    }

    /// Publish a new snapshot, superseding the predecessor atomically.
    /// Also enqueues an outbox event in the same transaction.
    pub async fn publish_snapshot(
        pool: &PgPool,
        meta: &SnapshotMeta,
        definition: &serde_json::Value,
        snapshot_set_id: Option<Uuid>,
        correlation_id: Uuid,
    ) -> std::result::Result<Uuid, SemOsError> {
        let mut tx = pool.begin().await.map_err(|e| anyhow!(e))?;

        // Supersede predecessor if specified
        if let Some(pred_id) = meta.predecessor_id {
            let result = sqlx::query(
                r#"
                UPDATE sem_reg.snapshots
                SET effective_until = now()
                WHERE snapshot_id = $1
                  AND effective_until IS NULL
                "#,
            )
            .bind(pred_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow!(e))?;

            if result.rows_affected() == 0 {
                return Err(SemOsError::Conflict(format!(
                    "Predecessor snapshot {} not found or already superseded",
                    pred_id
                )));
            }
        }

        // Insert the new snapshot
        let security_label_json =
            serde_json::to_value(&meta.security_label).map_err(|e| anyhow!(e))?;

        let snapshot_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.snapshots (
                snapshot_set_id, object_type, object_id,
                version_major, version_minor, status,
                governance_tier, trust_class, security_label,
                predecessor_id, change_type, change_rationale,
                created_by, approved_by, definition
            ) VALUES (
                $1, $2::sem_reg.object_type, $3,
                $4, $5, $6::sem_reg.snapshot_status,
                $7::sem_reg.governance_tier, $8::sem_reg.trust_class, $9,
                $10, $11::sem_reg.change_type, $12,
                $13, $14, $15
            )
            RETURNING snapshot_id
            "#,
        )
        .bind(snapshot_set_id)
        .bind(meta.object_type.as_ref())
        .bind(meta.object_id)
        .bind(meta.version_major)
        .bind(meta.version_minor)
        .bind(meta.status.as_ref())
        .bind(meta.governance_tier.as_ref())
        .bind(meta.trust_class.as_ref())
        .bind(&security_label_json)
        .bind(meta.predecessor_id)
        .bind(meta.change_type.as_ref())
        .bind(&meta.change_rationale)
        .bind(&meta.created_by)
        .bind(&meta.approved_by)
        .bind(definition)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| anyhow!(e))?;

        // Enqueue outbox event atomically in the same transaction.
        // This is the publish invariant: snapshot + outbox in one tx.
        if let Some(set_id) = snapshot_set_id {
            let payload = serde_json::json!({
                "snapshot_set_id": set_id,
                "snapshot_id": snapshot_id,
                "object_type": meta.object_type.as_ref(),
                "object_id": meta.object_id,
            });
            sqlx::query(
                r#"
                INSERT INTO sem_reg.outbox_events (event_type, snapshot_set_id, correlation_id, payload)
                VALUES ('snapshot_set_published', $1, $2, $3)
                "#,
            )
            .bind(set_id)
            .bind(correlation_id)
            .bind(&payload)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow!(e))?;
        }

        tx.commit().await.map_err(|e| anyhow!(e))?;
        Ok(snapshot_id)
    }

    /// Create a new snapshot set.
    pub async fn create_snapshot_set(
        pool: &PgPool,
        description: Option<&str>,
        created_by: &str,
    ) -> std::result::Result<Uuid, SemOsError> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.snapshot_sets (description, created_by)
            VALUES ($1, $2)
            RETURNING snapshot_set_id
            "#,
        )
        .bind(description)
        .bind(created_by)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(row)
    }

    /// Count active snapshots by object type.
    pub async fn count_active(
        pool: &PgPool,
        object_type: Option<ObjectType>,
    ) -> std::result::Result<Vec<(String, i64)>, SemOsError> {
        let ot: Option<String> = object_type.map(|t| t.as_ref().to_string());
        let rows = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT object_type::text, COUNT(*) as cnt
            FROM sem_reg.snapshots
            WHERE status = 'active'
              AND effective_until IS NULL
              AND ($1::sem_reg.object_type IS NULL OR object_type = $1::sem_reg.object_type)
            GROUP BY object_type
            ORDER BY object_type
            "#,
        )
        .bind(ot)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(rows)
    }
}

#[async_trait]
impl SnapshotStore for PgSnapshotStore {
    async fn resolve(&self, fqn: &Fqn, _as_of: Option<&SnapshotSetId>) -> Result<SnapshotRow> {
        // Try each object type to find one matching this FQN.
        // In practice the caller would know the object type, but the port
        // trait signature uses FQN only for simplicity.
        let all_types = [
            ObjectType::AttributeDef,
            ObjectType::EntityTypeDef,
            ObjectType::RelationshipTypeDef,
            ObjectType::VerbContract,
            ObjectType::TaxonomyDef,
            ObjectType::TaxonomyNode,
            ObjectType::MembershipRule,
            ObjectType::ViewDef,
            ObjectType::PolicyRule,
            ObjectType::EvidenceRequirement,
            ObjectType::DocumentTypeDef,
            ObjectType::ObservationDef,
            ObjectType::DerivationSpec,
        ];

        for ot in all_types {
            if let Some(row) =
                Self::find_active_by_definition_field(&self.pool, ot, "fqn", fqn.as_str()).await?
            {
                return Ok(row);
            }
        }

        Err(SemOsError::NotFound(format!(
            "No active snapshot for fqn={}",
            fqn
        )))
    }

    async fn publish(&self, principal: &Principal, _req: PublishInput) -> Result<SnapshotSetId> {
        let set_id = Self::create_snapshot_set(&self.pool, None, &principal.actor_id).await?;
        // The actual snapshot insertion happens via CoreService which calls
        // publish_snapshot() directly. This method creates the set container.
        Ok(SnapshotSetId(set_id))
    }

    async fn list_as_of(&self, as_of: &SnapshotSetId) -> Result<Vec<SnapshotSummary>> {
        let rows = sqlx::query_as::<_, (Uuid, String, serde_json::Value)>(
            r#"
            SELECT snapshot_id, object_type::text, definition
            FROM sem_reg.snapshots
            WHERE snapshot_set_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(as_of.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        let summaries = rows
            .into_iter()
            .map(|(sid, ot, def)| {
                let fqn = def
                    .get("fqn")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("snapshot {} has no fqn in definition", sid))?;
                let object_type = ot
                    .parse::<ObjectType>()
                    .map_err(|_| anyhow!("unknown object_type '{}' for snapshot {}", ot, sid))?;
                Ok(SnapshotSummary {
                    snapshot_id: SnapshotId(sid),
                    object_type,
                    fqn: Fqn::new(fqn.to_string()),
                    content_hash: String::new(), // computed on demand
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(summaries)
    }

    async fn get_manifest(&self, id: &SnapshotSetId) -> Result<Manifest> {
        let entries = self.list_as_of(id).await?;
        let published_at = sqlx::query_scalar::<_, DateTime<Utc>>(
            r#"SELECT created_at FROM sem_reg.snapshot_sets WHERE snapshot_set_id = $1"#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?
        .unwrap_or_else(Utc::now);

        Ok(Manifest {
            snapshot_set_id: id.clone(),
            published_at,
            entries,
        })
    }

    async fn export(&self, id: &SnapshotSetId) -> Result<Vec<SnapshotExport>> {
        let rows = sqlx::query_as::<_, (Uuid, String, serde_json::Value)>(
            r#"
            SELECT snapshot_id, object_type::text, definition
            FROM sem_reg.snapshots
            WHERE snapshot_set_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        let exports = rows
            .into_iter()
            .map(|(sid, ot, def)| {
                let fqn = def
                    .get("fqn")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("snapshot {} has no fqn in definition", sid))?;
                let object_type = ot
                    .parse::<ObjectType>()
                    .map_err(|_| anyhow!("unknown object_type '{}' for snapshot {}", ot, sid))?;
                Ok(SnapshotExport {
                    snapshot_id: SnapshotId(sid),
                    fqn: Fqn::new(fqn.to_string()),
                    object_type,
                    payload: def,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(exports)
    }

    async fn publish_into_set(
        &self,
        meta: &SnapshotMeta,
        definition: &serde_json::Value,
        snapshot_set_id: Uuid,
        correlation_id: Uuid,
    ) -> Result<Uuid> {
        let snapshot_id = Self::publish_snapshot(
            &self.pool,
            meta,
            definition,
            Some(snapshot_set_id),
            correlation_id,
        )
        .await?;
        Ok(snapshot_id)
    }

    async fn publish_batch_into_set(
        &self,
        items: Vec<(SnapshotMeta, serde_json::Value)>,
        snapshot_set_id: Uuid,
        correlation_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        if items.is_empty() {
            return Ok(vec![]);
        }

        let mut tx = self.pool.begin().await.map_err(|e| anyhow!(e))?;
        let mut snapshot_ids = Vec::with_capacity(items.len());

        for (meta, definition) in &items {
            // Supersede predecessor if specified
            if let Some(pred_id) = meta.predecessor_id {
                let result = sqlx::query(
                    r#"
                    UPDATE sem_reg.snapshots
                    SET effective_until = now()
                    WHERE snapshot_id = $1
                      AND effective_until IS NULL
                    "#,
                )
                .bind(pred_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| anyhow!(e))?;

                if result.rows_affected() == 0 {
                    return Err(SemOsError::Conflict(format!(
                        "Predecessor snapshot {} not found or already superseded",
                        pred_id
                    )));
                }
            }

            // Insert the new snapshot
            let security_label_json =
                serde_json::to_value(&meta.security_label).map_err(|e| anyhow!(e))?;
            let snapshot_id = sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO sem_reg.snapshots (
                    snapshot_set_id, object_type, object_id,
                    version_major, version_minor, status,
                    governance_tier, trust_class, security_label,
                    predecessor_id, change_type, change_rationale,
                    created_by, approved_by, definition
                ) VALUES (
                    $1, $2::sem_reg.object_type, $3,
                    $4, $5, $6::sem_reg.snapshot_status,
                    $7::sem_reg.governance_tier, $8::sem_reg.trust_class, $9,
                    $10, $11::sem_reg.change_type, $12,
                    $13, $14, $15
                )
                RETURNING snapshot_id
                "#,
            )
            .bind(snapshot_set_id)
            .bind(meta.object_type.as_ref())
            .bind(meta.object_id)
            .bind(meta.version_major)
            .bind(meta.version_minor)
            .bind(meta.status.as_ref())
            .bind(meta.governance_tier.as_ref())
            .bind(meta.trust_class.as_ref())
            .bind(&security_label_json)
            .bind(meta.predecessor_id)
            .bind(meta.change_type.as_ref())
            .bind(&meta.change_rationale)
            .bind(&meta.created_by)
            .bind(&meta.approved_by)
            .bind(definition)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| anyhow!(e))?;

            snapshot_ids.push(snapshot_id);
        }

        // v3.3 invariant: exactly ONE outbox event for the entire batch.
        let payload = serde_json::json!({
            "snapshot_set_id": snapshot_set_id,
            "snapshot_ids": snapshot_ids,
            "count": snapshot_ids.len(),
        });
        sqlx::query(
            r#"
            INSERT INTO sem_reg.outbox_events (event_type, snapshot_set_id, correlation_id, payload)
            VALUES ('snapshot_set_published', $1, $2, $3)
            "#,
        )
        .bind(snapshot_set_id)
        .bind(correlation_id)
        .bind(&payload)
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!(e))?;

        tx.commit().await.map_err(|e| anyhow!(e))?;
        Ok(snapshot_ids)
    }

    async fn find_dependents(&self, fqn: &str, limit: i64) -> Result<Vec<DependentSnapshot>> {
        let rows = sqlx::query_as::<_, (Uuid, String, String)>(
            r#"
            SELECT snapshot_id, object_type::text, COALESCE(definition->>'fqn', object_id::text)
            FROM sem_reg.snapshots
            WHERE status = 'active'
              AND effective_until IS NULL
              AND definition::text LIKE '%' || $1 || '%'
              AND COALESCE(definition->>'fqn', '') != $1
            LIMIT $2
            "#,
        )
        .bind(fqn)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        rows.into_iter()
            .map(|(snapshot_id, ot, dep_fqn)| {
                let object_type = ot.parse::<ObjectType>().map_err(|_| {
                    anyhow!("unknown object_type '{}' for snapshot {}", ot, snapshot_id)
                })?;
                Ok(DependentSnapshot {
                    snapshot_id,
                    object_type,
                    fqn: dep_fqn,
                })
            })
            .collect::<Result<Vec<_>>>()
    }
}

// ── PgObjectStore ─────────────────────────────────────────────

pub struct PgObjectStore {
    pool: PgPool,
}

impl PgObjectStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ObjectStore for PgObjectStore {
    async fn load_typed(&self, snapshot_id: &SnapshotId, fqn: &Fqn) -> Result<TypedObject> {
        let row = sqlx::query_as::<_, (String, serde_json::Value)>(
            r#"
            SELECT object_type::text, definition
            FROM sem_reg.snapshots
            WHERE snapshot_id = $1
            "#,
        )
        .bind(snapshot_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?
        .ok_or_else(|| SemOsError::NotFound(format!("Snapshot {} not found", snapshot_id.0)))?;

        let object_type = row.0.parse::<ObjectType>().map_err(|_| {
            anyhow!(
                "unknown object_type '{}' for snapshot {}",
                row.0,
                snapshot_id.0
            )
        })?;
        Ok(TypedObject {
            snapshot_id: snapshot_id.clone(),
            fqn: fqn.clone(),
            object_type,
            definition: row.1,
        })
    }
}

// ── PgChangesetStore ──────────────────────────────────────────

pub struct PgChangesetStore {
    pool: PgPool,
}

impl PgChangesetStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_changeset_row(
        row: (Uuid, String, String, String, DateTime<Utc>, DateTime<Utc>),
    ) -> std::result::Result<Changeset, SemOsError> {
        let status = row
            .1
            .parse::<ChangesetStatus>()
            .map_err(|_| SemOsError::Internal(anyhow!("invalid changeset status: {}", row.1)))?;
        Ok(Changeset {
            changeset_id: row.0,
            status,
            owner_actor_id: row.2,
            scope: row.3,
            created_at: row.4,
            updated_at: row.5,
        })
    }
}

#[async_trait]
impl ChangesetStore for PgChangesetStore {
    async fn create_changeset(&self, input: CreateChangesetInput) -> Result<Changeset> {
        let row =
            sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>, DateTime<Utc>)>(
                r#"
            INSERT INTO sem_reg.changesets (status, owner_actor_id, scope)
            VALUES ('draft', $1, $2)
            RETURNING changeset_id, status, owner_actor_id, scope, created_at, updated_at
            "#,
            )
            .bind(&input.owner_actor_id)
            .bind(&input.scope)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!(e))?;

        Self::parse_changeset_row(row)
    }

    async fn get_changeset(&self, changeset_id: Uuid) -> Result<Changeset> {
        let row =
            sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>, DateTime<Utc>)>(
                r#"
            SELECT changeset_id, status, owner_actor_id, scope, created_at, updated_at
            FROM sem_reg.changesets
            WHERE changeset_id = $1
            "#,
            )
            .bind(changeset_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow!(e))?
            .ok_or_else(|| SemOsError::NotFound(format!("changeset {changeset_id} not found")))?;

        Self::parse_changeset_row(row)
    }

    async fn list_changesets(
        &self,
        status: Option<&str>,
        owner: Option<&str>,
        scope: Option<&str>,
    ) -> Result<Vec<Changeset>> {
        let rows =
            sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>, DateTime<Utc>)>(
                r#"
            SELECT changeset_id, status, owner_actor_id, scope, created_at, updated_at
            FROM sem_reg.changesets
            WHERE ($1::text IS NULL OR status = $1)
              AND ($2::text IS NULL OR owner_actor_id = $2)
              AND ($3::text IS NULL OR scope = $3)
            ORDER BY updated_at DESC
            LIMIT 200
            "#,
            )
            .bind(status)
            .bind(owner)
            .bind(scope)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow!(e))?;

        rows.into_iter().map(Self::parse_changeset_row).collect()
    }

    async fn update_status(&self, changeset_id: Uuid, new_status: ChangesetStatus) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE sem_reg.changesets
            SET status = $2, updated_at = now()
            WHERE changeset_id = $1
            "#,
        )
        .bind(changeset_id)
        .bind(new_status.as_ref())
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        if result.rows_affected() == 0 {
            return Err(SemOsError::NotFound(format!(
                "changeset {changeset_id} not found"
            )));
        }
        Ok(())
    }

    async fn add_entry(
        &self,
        changeset_id: Uuid,
        input: AddChangesetEntryInput,
    ) -> Result<ChangesetEntry> {
        // Verify the changeset exists and is in draft status.
        let cs = self.get_changeset(changeset_id).await?;
        if cs.status != ChangesetStatus::Draft {
            return Err(SemOsError::Conflict(format!(
                "changeset {} is in '{}' status — entries can only be added to 'draft' changesets",
                changeset_id, cs.status
            )));
        }

        let row = sqlx::query_as::<_, (Uuid, Uuid, String, String, String, serde_json::Value, Option<Uuid>, DateTime<Utc>)>(
            r#"
            INSERT INTO sem_reg.changeset_entries
                (changeset_id, object_fqn, object_type, change_kind, draft_payload, base_snapshot_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING entry_id, changeset_id, object_fqn, object_type, change_kind, draft_payload, base_snapshot_id, created_at
            "#,
        )
        .bind(changeset_id)
        .bind(&input.object_fqn)
        .bind(input.object_type.as_ref())
        .bind(input.change_kind.as_ref())
        .bind(&input.draft_payload)
        .bind(input.base_snapshot_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        let change_kind = row
            .4
            .parse::<ChangeKind>()
            .map_err(|_| SemOsError::Internal(anyhow!("invalid change_kind: {}", row.4)))?;

        let object_type = row
            .3
            .parse::<ObjectType>()
            .map_err(|_| SemOsError::Internal(anyhow!("invalid object_type: {}", row.3)))?;

        Ok(ChangesetEntry {
            entry_id: row.0,
            changeset_id: row.1,
            object_fqn: row.2,
            object_type,
            change_kind,
            draft_payload: row.5,
            base_snapshot_id: row.6,
            created_at: row.7,
        })
    }

    async fn list_entries(&self, changeset_id: Uuid) -> Result<Vec<ChangesetEntry>> {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                String,
                String,
                serde_json::Value,
                Option<Uuid>,
                DateTime<Utc>,
            ),
        >(
            r#"
            SELECT entry_id, changeset_id, object_fqn, object_type, change_kind,
                   draft_payload, base_snapshot_id, created_at
            FROM sem_reg.changeset_entries
            WHERE changeset_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(changeset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        rows.into_iter()
            .map(|row| {
                let object_type = row
                    .3
                    .parse::<ObjectType>()
                    .map_err(|_| SemOsError::Internal(anyhow!("invalid object_type: {}", row.3)))?;
                let change_kind = row
                    .4
                    .parse::<ChangeKind>()
                    .map_err(|_| SemOsError::Internal(anyhow!("invalid change_kind: {}", row.4)))?;
                Ok(ChangesetEntry {
                    entry_id: row.0,
                    changeset_id: row.1,
                    object_fqn: row.2,
                    object_type,
                    change_kind,
                    draft_payload: row.5,
                    base_snapshot_id: row.6,
                    created_at: row.7,
                })
            })
            .collect()
    }

    async fn submit_review(
        &self,
        changeset_id: Uuid,
        input: SubmitReviewInput,
    ) -> Result<ChangesetReview> {
        // Verify changeset exists
        let _ = self.get_changeset(changeset_id).await?;

        let row = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, DateTime<Utc>)>(
            r#"
            INSERT INTO sem_reg.changeset_reviews
                (changeset_id, actor_id, verdict, comment)
            VALUES ($1, $2, $3, $4)
            RETURNING review_id, changeset_id, actor_id, verdict, comment, reviewed_at
            "#,
        )
        .bind(changeset_id)
        .bind(&input.actor_id)
        .bind(input.verdict.as_ref())
        .bind(&input.comment)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        let verdict = row
            .3
            .parse::<ReviewVerdict>()
            .map_err(|_| SemOsError::Internal(anyhow!("invalid verdict: {}", row.3)))?;

        Ok(ChangesetReview {
            review_id: row.0,
            changeset_id: row.1,
            actor_id: row.2,
            verdict,
            comment: row.4,
            reviewed_at: row.5,
        })
    }

    async fn list_reviews(&self, changeset_id: Uuid) -> Result<Vec<ChangesetReview>> {
        let rows =
            sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, DateTime<Utc>)>(
                r#"
            SELECT review_id, changeset_id, actor_id, verdict, comment, reviewed_at
            FROM sem_reg.changeset_reviews
            WHERE changeset_id = $1
            ORDER BY reviewed_at
            "#,
            )
            .bind(changeset_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow!(e))?;

        rows.into_iter()
            .map(|row| {
                let verdict = row
                    .3
                    .parse::<ReviewVerdict>()
                    .map_err(|_| SemOsError::Internal(anyhow!("invalid verdict: {}", row.3)))?;
                Ok(ChangesetReview {
                    review_id: row.0,
                    changeset_id: row.1,
                    actor_id: row.2,
                    verdict,
                    comment: row.4,
                    reviewed_at: row.5,
                })
            })
            .collect()
    }
}

// ── PgAuditStore ──────────────────────────────────────────────

pub struct PgAuditStore {
    pool: PgPool,
}

impl PgAuditStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditStore for PgAuditStore {
    async fn append(&self, principal: &Principal, entry: AuditEntry) -> Result<()> {
        // Audit goes to decision_records for now (existing table).
        // A dedicated audit_log table can be added in a later stage.
        sqlx::query(
            r#"
            INSERT INTO sem_reg.decision_records (
                chosen_action, chosen_action_description,
                alternatives_considered, evidence_for, evidence_against,
                negative_evidence, policy_verdicts, snapshot_manifest,
                confidence, escalation_flag, decided_by
            ) VALUES (
                $1, $2,
                '[]'::jsonb, '[]'::jsonb, '[]'::jsonb,
                '[]'::jsonb, '[]'::jsonb, $3,
                1.0, false, $4
            )
            "#,
        )
        .bind(&entry.action)
        .bind(&entry.action)
        .bind(&entry.details)
        .bind(&principal.actor_id)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }
}

// ── PgOutboxStore ─────────────────────────────────────────────

pub struct PgOutboxStore {
    pool: PgPool,
}

impl PgOutboxStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxStore for PgOutboxStore {
    async fn enqueue(&self, event: OutboxEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sem_reg.outbox_events (
                event_id, event_type, snapshot_set_id, correlation_id,
                attempt_count, payload
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(event.event_id.0)
        .bind(&event.event_type)
        .bind(event.snapshot_set_id.0)
        .bind(event.correlation_id)
        .bind(event.attempt_count as i32)
        .bind(&event.payload)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }

    async fn claim_next(&self, claimer_id: &str) -> Result<Option<OutboxEvent>> {
        // Atomic claim using CTE + FOR UPDATE SKIP LOCKED.
        // P1.1: `AND failed_at IS NULL` ensures dead-lettered events are never re-claimed.
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                i64,
                String,
                Uuid,
                Uuid,
                i32,
                serde_json::Value,
                DateTime<Utc>,
            ),
        >(
            r#"
            WITH claimable AS (
                SELECT outbox_seq
                FROM sem_reg.outbox_events
                WHERE processed_at IS NULL
                  AND failed_at IS NULL
                  AND (claimed_at IS NULL OR claim_timeout_at < now())
                ORDER BY outbox_seq
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE sem_reg.outbox_events e
            SET claimed_at = now(),
                claimer_id = $1,
                claim_timeout_at = now() + interval '5 minutes',
                attempt_count = attempt_count + 1
            FROM claimable c
            WHERE e.outbox_seq = c.outbox_seq
            RETURNING e.event_id, e.outbox_seq, e.event_type,
                      e.snapshot_set_id, e.correlation_id,
                      e.attempt_count, e.payload, e.created_at
            "#,
        )
        .bind(claimer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        Ok(row.map(
            |(
                event_id,
                outbox_seq,
                event_type,
                snapshot_set_id,
                correlation_id,
                attempt_count,
                payload,
                created_at,
            )| {
                OutboxEvent {
                    event_id: EventId(event_id),
                    snapshot_set_id: SnapshotSetId(snapshot_set_id),
                    outbox_seq,
                    event_type,
                    attempt_count: attempt_count as u32,
                    correlation_id,
                    payload,
                    created_at,
                }
            },
        ))
    }

    async fn mark_processed(&self, event_id: &EventId) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sem_reg.outbox_events
            SET processed_at = now()
            WHERE event_id = $1
            "#,
        )
        .bind(event_id.0)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }

    async fn record_failure(&self, event_id: &EventId, error: &str) -> Result<()> {
        // Retryable failure: clear the claim so the event can be re-claimed on next poll.
        // `failed_at` stays NULL so `claim_next()` still picks it up.
        sqlx::query(
            r#"
            UPDATE sem_reg.outbox_events
            SET claimed_at = NULL,
                claimer_id = NULL,
                claim_timeout_at = NULL,
                last_error = $2
            WHERE event_id = $1
            "#,
        )
        .bind(event_id.0)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }

    async fn mark_dead_letter(&self, event_id: &EventId, error: &str) -> Result<()> {
        // Permanent dead-letter: sets `failed_at` so `claim_next()` skips it.
        sqlx::query(
            r#"
            UPDATE sem_reg.outbox_events
            SET failed_at = now(),
                last_error = $2
            WHERE event_id = $1
            "#,
        )
        .bind(event_id.0)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }
}

// ── PgEvidenceStore ───────────────────────────────────────────

pub struct PgEvidenceStore {
    pool: PgPool,
}

impl PgEvidenceStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EvidenceInstanceStore for PgEvidenceStore {
    async fn record(&self, principal: &Principal, instance: EvidenceInstance) -> Result<()> {
        // Insert into entity-centric attribute_observations table (migration 091).
        sqlx::query(
            r#"
            INSERT INTO sem_reg.attribute_observations (
                subject_ref, attribute_fqn, confidence, observed_at,
                observer_id, evidence_grade, raw_payload
            ) VALUES (
                $1, $2, $3, COALESCE($4, now()), $5, $6, $7
            )
            "#,
        )
        .bind(instance.subject_ref)
        .bind(&instance.attribute_fqn)
        .bind(instance.confidence)
        .bind(instance.observed_at)
        .bind(&principal.actor_id)
        .bind(&instance.evidence_grade)
        .bind(&instance.raw_payload)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }
}

// ── PgProjectionWriter ────────────────────────────────────────

pub struct PgProjectionWriter {
    pool: PgPool,
}

impl PgProjectionWriter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectionWriter for PgProjectionWriter {
    async fn write_active_snapshot_set(&self, event: &OutboxEvent) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(|e| anyhow!(e))?;

        // Idempotency: check watermark — if already processed, no-op.
        let current_watermark = sqlx::query_scalar::<_, Option<i64>>(
            r#"
            SELECT last_outbox_seq
            FROM sem_reg_pub.projection_watermark
            WHERE projection_name = 'active_snapshot_set'
            "#,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| anyhow!(e))?
        .flatten();

        if let Some(wm) = current_watermark {
            if event.outbox_seq <= wm {
                // Already processed — idempotent no-op.
                return Ok(());
            }
        }

        let set_id = event.snapshot_set_id.0;
        let now = chrono::Utc::now();

        // Load all snapshots for this snapshot_set_id from sem_reg.snapshots.
        let rows = sqlx::query_as::<_, (Uuid, String, serde_json::Value)>(
            r#"
            SELECT snapshot_id, object_type::text, definition
            FROM sem_reg.snapshots
            WHERE snapshot_set_id = $1
              AND status = 'active'
            ORDER BY created_at
            "#,
        )
        .bind(set_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| anyhow!(e))?;

        // Upsert into the appropriate projection table based on object_type.
        for (snapshot_id, object_type, definition) in &rows {
            let fqn = definition
                .get("fqn")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "projection corruption: snapshot {} (type={}) has no fqn in definition",
                        snapshot_id,
                        object_type
                    )
                })?;

            match object_type.as_str() {
                "verb_contract" => {
                    let verb_name = definition
                        .get("verb_name")
                        .or_else(|| definition.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(fqn);
                    sqlx::query(
                        r#"
                        INSERT INTO sem_reg_pub.active_verb_contracts
                            (snapshot_set_id, snapshot_id, fqn, verb_name, payload, published_at)
                        VALUES ($1, $2, $3, $4, $5, $6)
                        ON CONFLICT (snapshot_set_id, fqn)
                        DO UPDATE SET snapshot_id = $2, verb_name = $4,
                                      payload = $5, published_at = $6
                        "#,
                    )
                    .bind(set_id)
                    .bind(snapshot_id)
                    .bind(fqn)
                    .bind(verb_name)
                    .bind(definition)
                    .bind(now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| anyhow!(e))?;
                }
                "entity_type_def" => {
                    sqlx::query(
                        r#"
                        INSERT INTO sem_reg_pub.active_entity_types
                            (snapshot_set_id, snapshot_id, fqn, payload, published_at)
                        VALUES ($1, $2, $3, $4, $5)
                        ON CONFLICT (snapshot_set_id, fqn)
                        DO UPDATE SET snapshot_id = $2, payload = $4, published_at = $5
                        "#,
                    )
                    .bind(set_id)
                    .bind(snapshot_id)
                    .bind(fqn)
                    .bind(definition)
                    .bind(now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| anyhow!(e))?;
                }
                "taxonomy_def" | "taxonomy_node" => {
                    sqlx::query(
                        r#"
                        INSERT INTO sem_reg_pub.active_taxonomies
                            (snapshot_set_id, snapshot_id, fqn, payload, published_at)
                        VALUES ($1, $2, $3, $4, $5)
                        ON CONFLICT (snapshot_set_id, fqn)
                        DO UPDATE SET snapshot_id = $2, payload = $4, published_at = $5
                        "#,
                    )
                    .bind(set_id)
                    .bind(snapshot_id)
                    .bind(fqn)
                    .bind(definition)
                    .bind(now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| anyhow!(e))?;
                }
                _ => {
                    // Other object types are not projected to sem_reg_pub yet.
                    // They still exist in sem_reg.snapshots.
                }
            }
        }

        // Advance watermark
        sqlx::query(
            r#"
            UPDATE sem_reg_pub.projection_watermark
            SET last_outbox_seq = $1,
                updated_at = now()
            WHERE projection_name = 'active_snapshot_set'
            "#,
        )
        .bind(event.outbox_seq)
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow!(e))?;

        tx.commit().await.map_err(|e| anyhow!(e))?;
        Ok(())
    }
}

// ── PgBootstrapAuditStore ────────────────────────────────────

/// Postgres-backed bootstrap audit store.
/// Extracted from `sem_os_server::handlers::bootstrap` so the server
/// no longer needs a raw `PgPool`.
pub struct PgBootstrapAuditStore {
    pool: PgPool,
}

impl PgBootstrapAuditStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BootstrapAuditStore for PgBootstrapAuditStore {
    async fn check_bootstrap(&self, bundle_hash: &str) -> Result<Option<(String, Option<Uuid>)>> {
        let row = sqlx::query_as::<_, (String, Option<Uuid>)>(
            r#"
            SELECT status, snapshot_set_id
            FROM sem_reg.bootstrap_audit
            WHERE bundle_hash = $1
            "#,
        )
        .bind(bundle_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        Ok(row)
    }

    async fn start_bootstrap(
        &self,
        bundle_hash: &str,
        actor_id: &str,
        bundle_counts: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sem_reg.bootstrap_audit (
                bundle_hash, origin_actor_id, bundle_counts, status
            ) VALUES ($1, $2, $3, 'in_progress')
            ON CONFLICT (bundle_hash) DO UPDATE
            SET status = 'in_progress',
                started_at = now(),
                error = NULL
            "#,
        )
        .bind(bundle_hash)
        .bind(actor_id)
        .bind(&bundle_counts)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        Ok(())
    }

    async fn mark_published(&self, bundle_hash: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sem_reg.bootstrap_audit
            SET status = 'published',
                completed_at = now()
            WHERE bundle_hash = $1
            "#,
        )
        .bind(bundle_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        Ok(())
    }

    async fn mark_failed(&self, bundle_hash: &str, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sem_reg.bootstrap_audit
            SET status = 'failed',
                completed_at = now(),
                error = $2
            WHERE bundle_hash = $1
            "#,
        )
        .bind(bundle_hash)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e))?;

        Ok(())
    }
}

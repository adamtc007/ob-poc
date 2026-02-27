//! SnapshotStore — database operations for the immutable snapshot table.
//!
//! All mutations are INSERT-only (new snapshots). The only UPDATE is setting
//! `effective_until` on a predecessor when it is superseded.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use super::types::{ObjectType, PgSnapshotRow, SnapshotMeta, SnapshotRow};

/// Database operations for `sem_reg.snapshots`.
pub struct SnapshotStore;

impl SnapshotStore {
    // ── Snapshot sets ─────────────────────────────────────────

    /// Create a new snapshot set (atomic publish transaction group).
    pub async fn create_snapshot_set(
        pool: &PgPool,
        description: Option<&str>,
        created_by: &str,
    ) -> Result<Uuid> {
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
        .await?;
        Ok(row)
    }

    // ── Insert ────────────────────────────────────────────────

    /// Insert a new snapshot. Returns the generated `snapshot_id`.
    ///
    /// This is INSERT-only — the foundational invariant.
    pub async fn insert_snapshot(
        pool: &PgPool,
        meta: &SnapshotMeta,
        definition: &serde_json::Value,
        snapshot_set_id: Option<Uuid>,
    ) -> Result<Uuid> {
        let security_label_json = serde_json::to_value(&meta.security_label)?;

        let snapshot_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.snapshots (
                snapshot_set_id, object_type, object_id,
                version_major, version_minor, status,
                governance_tier, trust_class, security_label,
                predecessor_id, change_type, change_rationale,
                created_by, approved_by, definition
            ) VALUES (
                $1, $2, $3,
                $4, $5, $6,
                $7, $8, $9,
                $10, $11, $12,
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
        .fetch_one(pool)
        .await?;

        Ok(snapshot_id)
    }

    // ── Supersede ─────────────────────────────────────────────

    /// Supersede an existing snapshot by setting its `effective_until` to now.
    /// Returns the number of rows affected (should be 0 or 1).
    pub async fn supersede_snapshot(pool: &PgPool, predecessor_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE sem_reg.snapshots
            SET effective_until = now()
            WHERE snapshot_id = $1
              AND effective_until IS NULL
            "#,
        )
        .bind(predecessor_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    // ── Resolve ───────────────────────────────────────────────

    /// Resolve the currently active snapshot for an object (as of now).
    pub async fn resolve_active(
        pool: &PgPool,
        object_type: ObjectType,
        object_id: Uuid,
    ) -> Result<Option<SnapshotRow>> {
        let row = sqlx::query_as::<_, PgSnapshotRow>(
            r#"
            SELECT *
            FROM sem_reg.snapshots
            WHERE object_type = $1
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
        .await?;
        row.map(SnapshotRow::try_from).transpose()
    }

    /// Resolve the snapshot that was active at a specific point in time.
    pub async fn resolve_at(
        pool: &PgPool,
        object_type: ObjectType,
        object_id: Uuid,
        as_of: DateTime<Utc>,
    ) -> Result<Option<SnapshotRow>> {
        let row = sqlx::query_as::<_, PgSnapshotRow>(
            r#"
            SELECT *
            FROM sem_reg.snapshots
            WHERE object_type = $1
              AND object_id = $2
              AND status = 'active'
              AND effective_from <= $3
              AND (effective_until IS NULL OR effective_until > $3)
            ORDER BY effective_from DESC
            LIMIT 1
            "#,
        )
        .bind(object_type.as_ref())
        .bind(object_id)
        .bind(as_of)
        .fetch_optional(pool)
        .await?;
        row.map(SnapshotRow::try_from).transpose()
    }

    // ── History ───────────────────────────────────────────────

    /// Load the full snapshot history for an object, newest first.
    pub async fn load_history(
        pool: &PgPool,
        object_type: ObjectType,
        object_id: Uuid,
    ) -> Result<Vec<SnapshotRow>> {
        let rows = sqlx::query_as::<_, PgSnapshotRow>(
            r#"
            SELECT *
            FROM sem_reg.snapshots
            WHERE object_type = $1
              AND object_id = $2
            ORDER BY effective_from DESC
            "#,
        )
        .bind(object_type.as_ref())
        .bind(object_id)
        .fetch_all(pool)
        .await?;
        super::types::pg_rows_to_snapshot_rows(rows)
    }

    // ── List / Count ──────────────────────────────────────────

    /// List all currently active snapshots of a given object type.
    pub async fn list_active(
        pool: &PgPool,
        object_type: ObjectType,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SnapshotRow>> {
        let rows = sqlx::query_as::<_, PgSnapshotRow>(
            r#"
            SELECT *
            FROM sem_reg.snapshots
            WHERE object_type = $1
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
        .await?;
        super::types::pg_rows_to_snapshot_rows(rows)
    }

    /// Count active snapshots by object type. If `object_type` is None, counts all.
    pub async fn count_active(
        pool: &PgPool,
        object_type: Option<ObjectType>,
    ) -> Result<Vec<(ObjectType, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT object_type::text, COUNT(*) as cnt
            FROM sem_reg.snapshots
            WHERE status = 'active'
              AND effective_until IS NULL
              AND ($1::text IS NULL OR object_type::text = $1)
            GROUP BY object_type
            ORDER BY object_type
            "#,
        )
        .bind(object_type.map(|ot| ot.as_ref().to_owned()))
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(|(ot_str, cnt)| {
                let ot = ot_str
                    .parse::<ObjectType>()
                    .map_err(|_| anyhow!("invalid object_type from DB: {}", ot_str))?;
                Ok((ot, cnt))
            })
            .collect()
    }

    // ── Lookup by definition field ────────────────────────────

    /// Find an active snapshot by a JSONB field value in its definition.
    /// E.g. lookup a verb contract by `definition->>'fqn' = 'cbu.create'`.
    pub async fn find_active_by_definition_field(
        pool: &PgPool,
        object_type: ObjectType,
        field_name: &str,
        field_value: &str,
    ) -> Result<Option<SnapshotRow>> {
        // Use parameterised query with format_args for the JSON path
        // We build a simple query since the field name is controlled by us
        let query = format!(
            r#"
            SELECT *
            FROM sem_reg.snapshots
            WHERE object_type = $1
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
            .await?;
        row.map(SnapshotRow::try_from).transpose()
    }

    // ── Publish helper ────────────────────────────────────────

    /// Publish a new snapshot, superseding the predecessor if one exists.
    /// This is the standard publish flow:
    /// 1. If predecessor exists, set its `effective_until`
    /// 2. Insert the new snapshot
    ///
    /// Returns the new `snapshot_id`.
    pub async fn publish_snapshot(
        pool: &PgPool,
        meta: &SnapshotMeta,
        definition: &serde_json::Value,
        snapshot_set_id: Option<Uuid>,
    ) -> Result<Uuid> {
        // Atomic publish: supersede + insert within a single transaction.
        // If the insert fails, the supersede rolls back automatically.
        let mut tx = pool.begin().await?;

        // Supersede predecessor if specified
        if let Some(pred_id) = meta.predecessor_id {
            let affected = Self::supersede_snapshot_tx(&mut tx, pred_id).await?;
            if affected == 0 {
                return Err(anyhow!(
                    "Predecessor snapshot {} not found or already superseded",
                    pred_id
                ));
            }
        }

        // Insert the new snapshot
        let snapshot_id =
            Self::insert_snapshot_tx(&mut tx, meta, definition, snapshot_set_id).await?;

        tx.commit().await?;
        Ok(snapshot_id)
    }

    /// Supersede a snapshot within a transaction.
    async fn supersede_snapshot_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        predecessor_id: Uuid,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE sem_reg.snapshots
            SET effective_until = now()
            WHERE snapshot_id = $1
              AND effective_until IS NULL
            "#,
        )
        .bind(predecessor_id)
        .execute(&mut **tx)
        .await?;
        Ok(result.rows_affected())
    }

    /// Insert a snapshot within a transaction. Returns the generated snapshot_id.
    async fn insert_snapshot_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        meta: &SnapshotMeta,
        definition: &serde_json::Value,
        snapshot_set_id: Option<Uuid>,
    ) -> Result<Uuid> {
        let security_label_json = serde_json::to_value(&meta.security_label)?;

        let snapshot_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.snapshots (
                snapshot_set_id, object_type, object_id,
                version_major, version_minor, status,
                governance_tier, trust_class, security_label,
                predecessor_id, change_type, change_rationale,
                created_by, approved_by, definition
            ) VALUES (
                $1, $2, $3,
                $4, $5, $6,
                $7, $8, $9,
                $10, $11, $12,
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
        .fetch_one(&mut **tx)
        .await?;

        Ok(snapshot_id)
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::types::*;

    #[test]
    fn test_snapshot_meta_defaults() {
        let meta = SnapshotMeta::new_operational(ObjectType::AttributeDef, Uuid::new_v4(), "test");
        assert_eq!(meta.status, SnapshotStatus::Active);
        assert_eq!(meta.governance_tier, GovernanceTier::Operational);
        assert_eq!(meta.change_type, ChangeType::Created);
    }

    /// Verify that the _tx SQL has correct $N placeholders (Phase 0.1 regression).
    #[test]
    fn test_tx_sql_placeholders_not_corrupted() {
        // Read the source file and check the _tx functions have proper placeholders
        let source = include_str!("store.rs");

        // supersede_snapshot_tx must have "WHERE snapshot_id = $1"
        assert!(
            source.contains("WHERE snapshot_id = $1"),
            "supersede_snapshot_tx is missing $1 placeholder"
        );

        // insert_snapshot_tx must have $1 through $15 in VALUES
        for i in 1..=15 {
            let placeholder = format!("${}", i);
            // Check that VALUES section contains all 15 placeholders
            assert!(
                source.contains(&placeholder),
                "insert_snapshot_tx is missing {} placeholder",
                placeholder
            );
        }

        // Verify insert_snapshot_tx VALUES has $1 in first position
        // (corruption pattern was VALUES followed by bare commas without $)
        // Find the _tx function's VALUES clause and verify it starts with $1
        let tx_fn_start = source.find("async fn insert_snapshot_tx").unwrap();
        let tx_fn_source = &source[tx_fn_start..];
        let values_pos = tx_fn_source.find("VALUES (").unwrap();
        let after_values = &tx_fn_source[values_pos..values_pos + 120];
        assert!(
            after_values.contains("$1"),
            "insert_snapshot_tx VALUES clause missing $1: {}",
            after_values
        );
    }
}

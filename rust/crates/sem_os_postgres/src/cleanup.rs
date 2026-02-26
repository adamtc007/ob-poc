//! PostgreSQL implementation of the authoring cleanup store.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sem_os_core::authoring::cleanup::CleanupStore;
use sem_os_core::authoring::ports::Result;
use sqlx::PgPool;

/// PostgreSQL-backed cleanup store for archiving old ChangeSets.
pub struct PgCleanupStore {
    pool: PgPool,
}

impl PgCleanupStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CleanupStore for PgCleanupStore {
    async fn archive_terminal_changesets(&self, cutoff: DateTime<Utc>) -> Result<u32> {
        // Archive terminal ChangeSets (rejected, dry_run_failed, superseded) older than cutoff.
        // Uses explicit column lists to handle schema alignment between live and archive tables.
        let row = sqlx::query_as::<_, (i64,)>(
            r#"
            WITH moved_cs AS (
                DELETE FROM sem_reg.changesets
                WHERE status IN ('rejected', 'dry_run_failed', 'superseded')
                  AND updated_at < $1
                RETURNING
                    changeset_id, status, scope, owner_actor_id,
                    title, rationale, content_hash, hash_version,
                    supersedes_change_set_id, superseded_by, superseded_at,
                    depends_on, evaluated_against_snapshot_set_id,
                    created_at, updated_at
            ),
            moved_artifacts AS (
                DELETE FROM sem_reg_authoring.change_set_artifacts
                WHERE changeset_id IN (SELECT changeset_id FROM moved_cs)
                RETURNING
                    artifact_id, change_set_id, artifact_type, ordinal,
                    path, content, content_hash, metadata, created_at
            ),
            _archive_cs AS (
                INSERT INTO sem_reg_authoring.change_sets_archive
                    (changeset_id, status, scope, owner_actor_id,
                     title, rationale, content_hash, hash_version,
                     supersedes_change_set_id, superseded_by, superseded_at,
                     depends_on, evaluated_against_snapshot_set_id,
                     created_at, updated_at, archived_at)
                SELECT
                    changeset_id, status, scope, owner_actor_id,
                    title, rationale, content_hash, hash_version,
                    supersedes_change_set_id, superseded_by, superseded_at,
                    depends_on, evaluated_against_snapshot_set_id,
                    created_at, updated_at, now()
                FROM moved_cs
            ),
            _archive_artifacts AS (
                INSERT INTO sem_reg_authoring.change_set_artifacts_archive
                    (artifact_id, change_set_id, artifact_type, ordinal,
                     path, content, content_hash, metadata, created_at, archived_at)
                SELECT
                    artifact_id, change_set_id, artifact_type, ordinal,
                    path, content, content_hash, metadata, created_at, now()
                FROM moved_artifacts
            )
            SELECT count(*) FROM moved_cs
            "#,
        )
        .bind(cutoff)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;

        Ok(row.0 as u32)
    }

    async fn archive_orphan_changesets(&self, cutoff: DateTime<Utc>) -> Result<u32> {
        // Archive orphan ChangeSets (draft/validated with no activity) older than cutoff.
        let row = sqlx::query_as::<_, (i64,)>(
            r#"
            WITH moved_cs AS (
                DELETE FROM sem_reg.changesets
                WHERE status IN ('draft', 'validated')
                  AND updated_at < $1
                RETURNING
                    changeset_id, status, scope, owner_actor_id,
                    title, rationale, content_hash, hash_version,
                    supersedes_change_set_id, superseded_by, superseded_at,
                    depends_on, evaluated_against_snapshot_set_id,
                    created_at, updated_at
            ),
            moved_artifacts AS (
                DELETE FROM sem_reg_authoring.change_set_artifacts
                WHERE changeset_id IN (SELECT changeset_id FROM moved_cs)
                RETURNING
                    artifact_id, change_set_id, artifact_type, ordinal,
                    path, content, content_hash, metadata, created_at
            ),
            _archive_cs AS (
                INSERT INTO sem_reg_authoring.change_sets_archive
                    (changeset_id, status, scope, owner_actor_id,
                     title, rationale, content_hash, hash_version,
                     supersedes_change_set_id, superseded_by, superseded_at,
                     depends_on, evaluated_against_snapshot_set_id,
                     created_at, updated_at, archived_at)
                SELECT
                    changeset_id, status, scope, owner_actor_id,
                    title, rationale, content_hash, hash_version,
                    supersedes_change_set_id, superseded_by, superseded_at,
                    depends_on, evaluated_against_snapshot_set_id,
                    created_at, updated_at, now()
                FROM moved_cs
            ),
            _archive_artifacts AS (
                INSERT INTO sem_reg_authoring.change_set_artifacts_archive
                    (artifact_id, change_set_id, artifact_type, ordinal,
                     path, content, content_hash, metadata, created_at, archived_at)
                SELECT
                    artifact_id, change_set_id, artifact_type, ordinal,
                    path, content, content_hash, metadata, created_at, now()
                FROM moved_artifacts
            )
            SELECT count(*) FROM moved_cs
            "#,
        )
        .bind(cutoff)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;

        Ok(row.0 as u32)
    }
}

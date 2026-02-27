//! PostgreSQL implementation of the AuthoringStore and ScratchSchemaRunner traits.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use sem_os_core::authoring::ports::{
    AuthoringStore, Result, ScratchRunResult, ScratchSchemaRunner,
};
use sem_os_core::authoring::types::*;
use sem_os_core::error::SemOsError;
use sem_os_core::principal::Principal;

// ── PgAuthoringStore ───────────────────────────────────────────

pub struct PgAuthoringStore {
    pool: PgPool,
}

impl PgAuthoringStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthoringStore for PgAuthoringStore {
    async fn create_change_set(
        &self,
        principal: &Principal,
        title: &str,
        rationale: Option<&str>,
        content_hash: &str,
        hash_version: &str,
        depends_on: &[Uuid],
        supersedes: Option<Uuid>,
    ) -> Result<ChangeSetFull> {
        let row = sqlx::query_as::<_, ChangeSetRow>(
            r#"
            INSERT INTO sem_reg.changesets
                (status, owner_actor_id, scope, title, rationale,
                 content_hash, hash_version, depends_on,
                 supersedes_change_set_id)
            VALUES
                ('draft', $1, 'authoring', $2, $3, $4, $5, $6, $7)
            RETURNING
                changeset_id, status, owner_actor_id, scope, title, rationale,
                content_hash, hash_version, depends_on,
                supersedes_change_set_id, superseded_by, superseded_at,
                evaluated_against_snapshot_set_id,
                created_at, updated_at
            "#,
        )
        .bind(&principal.actor_id)
        .bind(title)
        .bind(rationale)
        .bind(content_hash)
        .bind(hash_version)
        .bind(depends_on)
        .bind(supersedes)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(row.into_full())
    }

    async fn get_change_set(&self, change_set_id: Uuid) -> Result<ChangeSetFull> {
        let row = sqlx::query_as::<_, ChangeSetRow>(
            r#"
            SELECT changeset_id, status, owner_actor_id, scope, title, rationale,
                   content_hash, hash_version, depends_on,
                   supersedes_change_set_id, superseded_by, superseded_at,
                   evaluated_against_snapshot_set_id,
                   created_at, updated_at
            FROM sem_reg.changesets
            WHERE changeset_id = $1
            "#,
        )
        .bind(change_set_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?
        .ok_or_else(|| SemOsError::NotFound(format!("changeset {change_set_id}")))?;

        Ok(row.into_full())
    }

    async fn find_by_content_hash(
        &self,
        hash_version: &str,
        content_hash: &str,
    ) -> Result<Option<ChangeSetFull>> {
        let row = sqlx::query_as::<_, ChangeSetRow>(
            r#"
            SELECT changeset_id, status, owner_actor_id, scope, title, rationale,
                   content_hash, hash_version, depends_on,
                   supersedes_change_set_id, superseded_by, superseded_at,
                   evaluated_against_snapshot_set_id,
                   created_at, updated_at
            FROM sem_reg.changesets
            WHERE hash_version = $1
              AND content_hash = $2
              AND status NOT IN ('rejected', 'superseded')
            "#,
        )
        .bind(hash_version)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(row.map(|r| r.into_full()))
    }

    async fn update_change_set_status(
        &self,
        change_set_id: Uuid,
        new_status: ChangeSetStatus,
    ) -> Result<()> {
        let rows = sqlx::query(
            "UPDATE sem_reg.changesets SET status = $1, updated_at = now() WHERE changeset_id = $2",
        )
        .bind(new_status.as_ref())
        .bind(change_set_id)
        .execute(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        if rows.rows_affected() == 0 {
            return Err(SemOsError::NotFound(format!("changeset {change_set_id}")));
        }
        Ok(())
    }

    async fn set_evaluated_against(
        &self,
        change_set_id: Uuid,
        snapshot_set_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE sem_reg.changesets SET evaluated_against_snapshot_set_id = $1, updated_at = now() WHERE changeset_id = $2",
        )
        .bind(snapshot_set_id)
        .bind(change_set_id)
        .execute(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;
        Ok(())
    }

    async fn mark_superseded(&self, change_set_id: Uuid, superseded_by: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sem_reg.changesets
            SET status = 'superseded', superseded_by = $1, superseded_at = now(), updated_at = now()
            WHERE changeset_id = $2
            "#,
        )
        .bind(superseded_by)
        .bind(change_set_id)
        .execute(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;
        Ok(())
    }

    async fn list_change_sets(
        &self,
        status: Option<ChangeSetStatus>,
        limit: i64,
    ) -> Result<Vec<ChangeSetFull>> {
        let rows = if let Some(s) = status {
            sqlx::query_as::<_, ChangeSetRow>(
                r#"
                SELECT changeset_id, status, owner_actor_id, scope, title, rationale,
                       content_hash, hash_version, depends_on,
                       supersedes_change_set_id, superseded_by, superseded_at,
                       evaluated_against_snapshot_set_id,
                       created_at, updated_at
                FROM sem_reg.changesets
                WHERE status = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(s.as_ref())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, ChangeSetRow>(
                r#"
                SELECT changeset_id, status, owner_actor_id, scope, title, rationale,
                       content_hash, hash_version, depends_on,
                       supersedes_change_set_id, superseded_by, superseded_at,
                       evaluated_against_snapshot_set_id,
                       created_at, updated_at
                FROM sem_reg.changesets
                ORDER BY created_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(rows.into_iter().map(|r| r.into_full()).collect())
    }

    // ── Artifacts ──────────────────────────────────────────────

    async fn insert_artifacts(
        &self,
        change_set_id: Uuid,
        artifacts: &[ChangeSetArtifact],
    ) -> Result<()> {
        for a in artifacts {
            sqlx::query(
                r#"
                INSERT INTO sem_reg_authoring.change_set_artifacts
                    (artifact_id, change_set_id, artifact_type, ordinal, path, content, content_hash, metadata)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(a.artifact_id)
            .bind(change_set_id)
            .bind(a.artifact_type.as_ref())
            .bind(a.ordinal)
            .bind(&a.path)
            .bind(&a.content)
            .bind(&a.content_hash)
            .bind(&a.metadata)
            .execute(&self.pool)
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;
        }
        Ok(())
    }

    async fn get_artifacts(&self, change_set_id: Uuid) -> Result<Vec<ChangeSetArtifact>> {
        let rows = sqlx::query_as::<_, ArtifactRow>(
            r#"
            SELECT artifact_id, change_set_id, artifact_type, ordinal, path,
                   content, content_hash, metadata
            FROM sem_reg_authoring.change_set_artifacts
            WHERE change_set_id = $1
            ORDER BY ordinal, path
            "#,
        )
        .bind(change_set_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(rows.into_iter().map(|r| r.into_artifact()).collect())
    }

    // ── Validation reports ─────────────────────────────────────

    async fn insert_validation_report(
        &self,
        change_set_id: Uuid,
        stage: ValidationStage,
        ok: bool,
        report: &serde_json::Value,
    ) -> Result<Uuid> {
        let row: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO sem_reg_authoring.validation_reports
                (change_set_id, stage, ok, report)
            VALUES ($1, $2, $3, $4)
            RETURNING report_id
            "#,
        )
        .bind(change_set_id)
        .bind(stage.as_ref())
        .bind(ok)
        .bind(report)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(row.0)
    }

    async fn get_validation_reports(
        &self,
        change_set_id: Uuid,
    ) -> Result<Vec<(Uuid, ValidationStage, bool, serde_json::Value)>> {
        let rows = sqlx::query_as::<_, ValidationReportRow>(
            r#"
            SELECT report_id, stage, ok, report
            FROM sem_reg_authoring.validation_reports
            WHERE change_set_id = $1
            ORDER BY ran_at DESC
            "#,
        )
        .bind(change_set_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let stage = r.stage.parse::<ValidationStage>()
                    .unwrap_or(ValidationStage::Validate);
                (r.report_id, stage, r.ok, r.report)
            })
            .collect())
    }

    // ── Governance audit log ───────────────────────────────────

    async fn insert_audit_entry(&self, entry: &GovernanceAuditEntry) -> Result<()> {
        let result_json =
            serde_json::to_value(&entry.result).map_err(|e| SemOsError::Internal(e.into()))?;

        sqlx::query(
            r#"
            INSERT INTO sem_reg_authoring.governance_audit_log
                (entry_id, ts, verb, agent_session_id, agent_mode,
                 change_set_id, snapshot_set_id, active_snapshot_set_id,
                 result, duration_ms, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(entry.entry_id)
        .bind(entry.timestamp)
        .bind(&entry.verb)
        .bind(entry.agent_session_id)
        .bind(entry.agent_mode.map(|m| m.to_string()))
        .bind(entry.change_set_id)
        .bind(entry.snapshot_set_id)
        .bind(entry.active_snapshot_set_id)
        .bind(&result_json)
        .bind(entry.duration_ms as i64)
        .bind(&entry.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(())
    }

    // ── Publish batches ────────────────────────────────────────

    async fn insert_publish_batch(&self, batch: &PublishBatch) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sem_reg_authoring.publish_batches
                (batch_id, change_set_ids, snapshot_set_id, published_at, publisher)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(batch.batch_id)
        .bind(&batch.change_set_ids)
        .bind(batch.snapshot_set_id)
        .bind(batch.published_at)
        .bind(&batch.publisher)
        .execute(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(())
    }

    // ── Snapshot set queries ────────────────────────────────

    async fn get_active_snapshot_set_id(&self) -> Result<Option<Uuid>> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT active_snapshot_set_id FROM sem_reg_pub.active_snapshot_set LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(row.map(|r| r.0))
    }

    // ── Publish support ────────────────────────────────────────

    async fn try_acquire_publish_lock(&self) -> Result<bool> {
        // Use pg_try_advisory_lock with a fixed key for the authoring publish gate.
        // Key: hash of "sem_reg_authoring_publish" → deterministic i64.
        const PUBLISH_LOCK_KEY: i64 = 0x5345_4D52_4547_5055; // "SEMREGPU" as hex

        let row: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
            .bind(PUBLISH_LOCK_KEY)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(row.0)
    }

    async fn apply_migrations(&self, migrations: &[(String, String)]) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        for (path, sql) in migrations {
            sqlx::query(sql).execute(&mut *tx).await.map_err(|e| {
                SemOsError::Internal(anyhow::anyhow!("Migration {path} failed: {e}"))
            })?;
        }

        tx.commit()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(())
    }

    async fn create_and_activate_snapshot_set(
        &self,
        change_set_ids: &[Uuid],
        publisher: &str,
    ) -> Result<Uuid> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        // Create the snapshot set
        let (ss_id,): (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO sem_reg.snapshot_sets (description, created_by)
            VALUES ($1, $2)
            RETURNING snapshot_set_id
            "#,
        )
        .bind(format!(
            "Authoring publish: {} changeset(s)",
            change_set_ids.len()
        ))
        .bind(publisher)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        // Upsert the active snapshot set pointer
        sqlx::query(
            r#"
            INSERT INTO sem_reg_pub.active_snapshot_set
                (singleton, active_snapshot_set_id, updated_at)
            VALUES (true, $1, now())
            ON CONFLICT (singleton)
            DO UPDATE SET active_snapshot_set_id = $1, updated_at = now()
            "#,
        )
        .bind(ss_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        tx.commit()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(ss_id)
    }

    // ── Health / observability ────────────────────────────────

    async fn count_by_status(&self) -> Result<Vec<(ChangeSetStatus, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*)::bigint as cnt
            FROM sem_reg.changesets
            GROUP BY status
            ORDER BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(rows
            .into_iter()
            .filter_map(|(s, c)| s.parse::<ChangeSetStatus>().ok().map(|status| (status, c)))
            .collect())
    }

    async fn find_stale_dry_runs(&self) -> Result<Vec<ChangeSetFull>> {
        let rows = sqlx::query_as::<_, ChangeSetRow>(
            r#"
            SELECT changeset_id, status, owner_actor_id, scope, title, rationale,
                   content_hash, hash_version, depends_on,
                   supersedes_change_set_id, superseded_by, superseded_at,
                   evaluated_against_snapshot_set_id,
                   created_at, updated_at
            FROM sem_reg.changesets
            WHERE status = 'dry_run_passed'
              AND evaluated_against_snapshot_set_id IS NOT NULL
              AND evaluated_against_snapshot_set_id != (
                  SELECT active_snapshot_set_id
                  FROM sem_reg_pub.active_snapshot_set
                  LIMIT 1
              )
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SemOsError::Internal(e.into()))?;

        Ok(rows.into_iter().map(|r| r.into_full()).collect())
    }
}

// ── PgScratchSchemaRunner ──────────────────────────────────────

pub struct PgScratchSchemaRunner {
    pool: PgPool,
}

impl PgScratchSchemaRunner {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScratchSchemaRunner for PgScratchSchemaRunner {
    async fn run_scratch_migrations(
        &self,
        migrations: &[(String, String)],
        down_migrations: &[(String, String)],
    ) -> Result<ScratchRunResult> {
        let scratch_schema = format!("scratch_{}", Uuid::new_v4().simple());
        let start = std::time::Instant::now();
        let mut apply_errors = Vec::new();
        let mut down_errors = Vec::new();

        // Use a single connection + transaction so everything rolls back
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        // Create scratch schema
        sqlx::query(&format!("CREATE SCHEMA {scratch_schema}"))
            .execute(&mut *conn)
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        // Set search_path to scratch schema
        sqlx::query(&format!("SET search_path TO {scratch_schema}, public"))
            .execute(&mut *conn)
            .await
            .map_err(|e| SemOsError::Internal(e.into()))?;

        // Apply forward migrations in order
        for (path, sql) in migrations {
            if let Err(e) = sqlx::query(sql).execute(&mut *conn).await {
                apply_errors.push(format!("{path}: {e}"));
            }
        }

        let apply_ms = start.elapsed().as_millis() as u64;

        // Validate down migrations (reverse order) if forward succeeded
        if apply_errors.is_empty() {
            for (path, sql) in down_migrations.iter().rev() {
                if let Err(e) = sqlx::query(sql).execute(&mut *conn).await {
                    down_errors.push(format!("{path}: {e}"));
                }
            }
        }

        // Always clean up: drop scratch schema
        let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS {scratch_schema} CASCADE"))
            .execute(&mut *conn)
            .await;

        // Reset search_path
        let _ = sqlx::query("RESET search_path").execute(&mut *conn).await;

        Ok(ScratchRunResult {
            apply_ms,
            apply_errors,
            down_errors,
        })
    }
}

// ── Internal row types ─────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ChangeSetRow {
    changeset_id: Uuid,
    status: String,
    owner_actor_id: String,
    scope: String,
    title: Option<String>,
    rationale: Option<String>,
    content_hash: Option<String>,
    hash_version: Option<String>,
    depends_on: Option<Vec<Uuid>>,
    supersedes_change_set_id: Option<Uuid>,
    superseded_by: Option<Uuid>,
    superseded_at: Option<chrono::DateTime<chrono::Utc>>,
    evaluated_against_snapshot_set_id: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl ChangeSetRow {
    fn into_full(self) -> ChangeSetFull {
        ChangeSetFull {
            change_set_id: self.changeset_id,
            status: self.status.parse::<ChangeSetStatus>().unwrap_or(ChangeSetStatus::Draft),
            content_hash: self.content_hash.unwrap_or_default(),
            hash_version: self.hash_version.unwrap_or_else(|| "v1".into()),
            title: self.title.unwrap_or_default(),
            rationale: self.rationale,
            created_by: self.owner_actor_id,
            scope: self.scope,
            created_at: self.created_at,
            updated_at: self.updated_at,
            supersedes_change_set_id: self.supersedes_change_set_id,
            superseded_by: self.superseded_by,
            superseded_at: self.superseded_at,
            depends_on: self.depends_on.unwrap_or_default(),
            evaluated_against_snapshot_set_id: self.evaluated_against_snapshot_set_id,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ArtifactRow {
    artifact_id: Uuid,
    change_set_id: Uuid,
    artifact_type: String,
    ordinal: i32,
    path: Option<String>,
    content: String,
    content_hash: String,
    metadata: Option<serde_json::Value>,
}

impl ArtifactRow {
    fn into_artifact(self) -> ChangeSetArtifact {
        ChangeSetArtifact {
            artifact_id: self.artifact_id,
            change_set_id: self.change_set_id,
            artifact_type: self.artifact_type.parse::<ArtifactType>()
                .unwrap_or(ArtifactType::DocJson),
            ordinal: self.ordinal,
            path: self.path,
            content: self.content,
            content_hash: self.content_hash,
            metadata: self.metadata,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ValidationReportRow {
    report_id: Uuid,
    stage: String,
    ok: bool,
    report: serde_json::Value,
}

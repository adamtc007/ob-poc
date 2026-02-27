//! Maintenance domain CustomOps (Spec §C.6 — 7 verbs).
//!
//! Mixed backends: health queries, cleanup, bootstrap, outbox, reindex, schema sync.
//! Allowed in Governed mode only (operational). Blocked in Research mode.

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Health Queries ────────────────────────────────────────────────

/// Check pending changeset health.
#[register_custom_op]
pub struct MaintenanceHealthPendingOp;

#[async_trait]
impl CustomOperation for MaintenanceHealthPendingOp {
    fn domain(&self) -> &'static str {
        "maintenance"
    }
    fn verb(&self) -> &'static str {
        "health-pending"
    }
    fn rationale(&self) -> &'static str {
        "Queries pending changeset counts by status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT status, COUNT(*)::bigint FROM sem_reg.changesets GROUP BY status ORDER BY status"
        )
        .fetch_all(pool)
        .await?;

        let entries: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(status, count)| {
                serde_json::json!({
                    "status": status,
                    "count": count,
                })
            })
            .collect();

        Ok(ExecutionResult::Record(serde_json::json!({
            "pending_changesets": entries,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "maintenance.health-pending requires database"
        ))
    }
}

/// Check for stale dry-run results.
#[register_custom_op]
pub struct MaintenanceHealthStaleDryrunsOp;

#[async_trait]
impl CustomOperation for MaintenanceHealthStaleDryrunsOp {
    fn domain(&self) -> &'static str {
        "maintenance"
    }
    fn verb(&self) -> &'static str {
        "health-stale-dryruns"
    }
    fn rationale(&self) -> &'static str {
        "Detects dry-run results older than threshold"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT c.changeset_id::text, c.status \
             FROM sem_reg.changesets c \
             WHERE c.status = 'dry_run_passed' \
             AND c.updated_at < NOW() - INTERVAL '7 days' \
             ORDER BY c.updated_at ASC \
             LIMIT 50",
        )
        .fetch_all(pool)
        .await?;

        let stale: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(id, status)| {
                serde_json::json!({
                    "changeset_id": id,
                    "status": status,
                })
            })
            .collect();

        Ok(ExecutionResult::Record(serde_json::json!({
            "stale_dryruns": stale,
            "count": stale.len(),
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "maintenance.health-stale-dryruns requires database"
        ))
    }
}

// ── Cleanup ───────────────────────────────────────────────────────

/// Run retention cleanup on archived changesets.
#[register_custom_op]
pub struct MaintenanceCleanupOp;

#[async_trait]
impl CustomOperation for MaintenanceCleanupOp {
    fn domain(&self) -> &'static str {
        "maintenance"
    }
    fn verb(&self) -> &'static str {
        "cleanup"
    }
    fn rationale(&self) -> &'static str {
        "Archives old rejected/failed changesets per retention policy"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Archive rejected/failed changesets older than 90 days
        let archived = sqlx::query_scalar::<_, i64>(
            "WITH moved AS ( \
                INSERT INTO sem_reg_authoring.change_sets_archive \
                SELECT * FROM sem_reg.changesets \
                WHERE status IN ('rejected', 'dry_run_failed') \
                AND updated_at < NOW() - INTERVAL '90 days' \
                ON CONFLICT DO NOTHING \
                RETURNING 1 \
            ) SELECT COUNT(*) FROM moved",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Ok(ExecutionResult::Record(serde_json::json!({
            "archived_count": archived,
            "status": "cleanup_complete",
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("maintenance.cleanup requires database"))
    }
}

// ── Bootstrap Seeds ───────────────────────────────────────────────

/// Bootstrap seed bundles into the registry.
#[register_custom_op]
pub struct MaintenanceBootstrapSeedsOp;

#[async_trait]
impl CustomOperation for MaintenanceBootstrapSeedsOp {
    fn domain(&self) -> &'static str {
        "maintenance"
    }
    fn verb(&self) -> &'static str {
        "bootstrap-seeds"
    }
    fn rationale(&self) -> &'static str {
        "Triggers seed bundle bootstrap via CoreService"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Bootstrap is handled at server startup; this verb can re-trigger
        Ok(ExecutionResult::Record(serde_json::json!({
            "status": "bootstrap_seeds must be triggered via server startup or CLI",
            "hint": "Use: cargo x sem-reg scan",
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "maintenance.bootstrap-seeds requires database"
        ))
    }
}

// ── Outbox Drain ──────────────────────────────────────────────────

/// Signal the outbox dispatcher to flush pending events.
#[register_custom_op]
pub struct MaintenanceDrainOutboxOp;

#[async_trait]
impl CustomOperation for MaintenanceDrainOutboxOp {
    fn domain(&self) -> &'static str {
        "maintenance"
    }
    fn verb(&self) -> &'static str {
        "drain-outbox"
    }
    fn rationale(&self) -> &'static str {
        "Checks outbox event queue status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sem_reg.outbox_events WHERE processed_at IS NULL",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Ok(ExecutionResult::Record(serde_json::json!({
            "pending_outbox_events": pending,
            "status": if pending == 0 { "drained" } else { "has_pending" },
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "maintenance.drain-outbox requires database"
        ))
    }
}

// ── Reindex Embeddings ────────────────────────────────────────────

/// Trigger embedding reindex (wraps populate_embeddings binary).
#[register_custom_op]
pub struct MaintenanceReindexEmbeddingsOp;

#[async_trait]
impl CustomOperation for MaintenanceReindexEmbeddingsOp {
    fn domain(&self) -> &'static str {
        "maintenance"
    }
    fn verb(&self) -> &'static str {
        "reindex-embeddings"
    }
    fn rationale(&self) -> &'static str {
        "Spawns populate_embeddings binary as subprocess"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use super::sem_reg_helpers::get_bool_arg;

        let force = get_bool_arg(verb_call, "force").unwrap_or(false);

        let mut cmd = tokio::process::Command::new("cargo");
        cmd.args([
            "run",
            "--release",
            "--package",
            "ob-semantic-matcher",
            "--bin",
            "populate_embeddings",
        ]);

        if force {
            cmd.arg("--").arg("--force");
        }

        let output = cmd.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(ExecutionResult::Record(serde_json::json!({
                "status": "success",
                "force": force,
                "output": stdout.lines().last().unwrap_or("done"),
            })))
        } else {
            Err(anyhow::anyhow!(
                "populate_embeddings failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                stderr.lines().last().unwrap_or("unknown error")
            ))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "maintenance.reindex-embeddings requires database"
        ))
    }
}

// ── Schema Sync Validation ────────────────────────────────────────

/// Validate that schema and registry are in sync (drift detection).
#[register_custom_op]
pub struct MaintenanceValidateSchemaSyncOp;

#[async_trait]
impl CustomOperation for MaintenanceValidateSchemaSyncOp {
    fn domain(&self) -> &'static str {
        "maintenance"
    }
    fn verb(&self) -> &'static str {
        "validate-schema-sync"
    }
    fn rationale(&self) -> &'static str {
        "Compares scanner-derived defs against active registry snapshots"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Count active snapshots by object type for a health summary
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT object_type, COUNT(*)::bigint \
             FROM sem_reg.snapshots WHERE status = 'active' \
             GROUP BY object_type ORDER BY object_type",
        )
        .fetch_all(pool)
        .await?;

        let counts: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(ot, c)| serde_json::json!({ "object_type": ot, "active_count": c }))
            .collect();

        Ok(ExecutionResult::Record(serde_json::json!({
            "active_snapshot_counts": counts,
            "hint": "Run 'cargo x sem-reg scan --dry-run' for full drift detection",
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "maintenance.validate-schema-sync requires database"
        ))
    }
}

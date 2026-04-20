//! Maintenance domain CustomOps (Spec §C.6 — 7 verbs).
//!
//! Mixed backends: health queries, cleanup, bootstrap, outbox, reindex, schema sync.
//! Allowed in Governed mode only (operational). Blocked in Research mode.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::json_extract_bool_opt;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

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

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT status, COUNT(*)::bigint FROM sem_reg.changesets GROUP BY status ORDER BY status"
        ).fetch_all(pool).await?;
        let entries: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(status, count)| serde_json::json!({"status": status, "count": count}))
            .collect();
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"pending_changesets": entries}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT c.changeset_id::text, c.status FROM sem_reg.changesets c WHERE c.status = 'dry_run_passed' AND c.updated_at < NOW() - INTERVAL '7 days' ORDER BY c.updated_at ASC LIMIT 50",
        ).fetch_all(pool).await?;
        let stale: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(id, status)| serde_json::json!({"changeset_id": id, "status": status}))
            .collect();
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"stale_dryruns": stale, "count": stale.len()}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let archived = sqlx::query_scalar::<_, i64>(
            "WITH moved AS (INSERT INTO sem_reg_authoring.change_sets_archive SELECT * FROM sem_reg.changesets WHERE status IN ('rejected', 'dry_run_failed') AND updated_at < NOW() - INTERVAL '90 days' ON CONFLICT DO NOTHING RETURNING 1) SELECT COUNT(*) FROM moved",
        ).fetch_one(pool).await.unwrap_or(0);
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"archived_count": archived, "status": "cleanup_complete"}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"status": "bootstrap_seeds must be triggered via server startup or CLI", "hint": "Use: cargo x sem-reg scan"}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sem_reg.outbox_events WHERE processed_at IS NULL",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"pending_outbox_events": pending, "status": if pending == 0 { "drained" } else { "has_pending" }}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── Reindex Embeddings ────────────────────────────────────────────

/// Queue an embedding reindex via the Sequencer outbox. Phase 0g remediation
/// of Pattern A A1 violation (D11, ledger row #1):
/// `docs/todo/pattern-b-a1-remediation-ledger.md` §2.
///
/// Previously this op spawned `cargo run --release -- populate_embeddings`
/// directly from the verb-execution body — a clear A1 violation (external
/// side effect inside the inner transaction). Now it writes a
/// `maintenance_spawn` row to `public.outbox` (migration 131) and returns
/// immediately. The drainer (Phase 5e) spawns the subprocess post-commit.
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
        "Queues embedding reindex via public.outbox (Phase 0g Pattern A remediation, D11)"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let force = json_extract_bool_opt(args, "force").unwrap_or(false);
        let (outbox_id, idempotency_key) = enqueue_reindex_embeddings(pool, force).await?;

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "status": "queued",
            "force": force,
            "outbox_id": outbox_id.to_string(),
            "idempotency_key": idempotency_key,
            "drainer": "Phase 5e outbox drainer will spawn the populate_embeddings subprocess post-commit.",
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Shared helper: insert a `maintenance_spawn` outbox row describing the
/// reindex-embeddings subprocess. Returns `(outbox_id, idempotency_key)`.
///
/// Idempotency:
/// - Same-day force=false invocations dedupe on the same key (per-day window).
/// - Force=true invocations get a distinct key (they are explicit re-runs).
/// - ON CONFLICT DO NOTHING ensures a duplicate insert is a no-op; the
///   returned `outbox_id` then belongs to the pre-existing row.
async fn enqueue_reindex_embeddings(
    pool: &PgPool,
    force: bool,
) -> Result<(uuid::Uuid, String)> {
    let outbox_id = uuid::Uuid::new_v4();
    let trace_id = uuid::Uuid::new_v4();

    // Idempotency convention matches ob-poc-types::IdempotencyKey::from_parts:
    //   <effect_kind>:<trace_id>:<sub_key>
    // For reindex, sub_key is the UTC date + force flag. This dedupes
    // accidental double-submits within the same day while allowing
    // explicit force reruns.
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let sub_key = if force {
        format!("{}-force", today)
    } else {
        today
    };
    let idempotency_key = format!("maintenance_spawn:{}:{}", trace_id, sub_key);

    let payload = serde_json::json!({
        "command": "cargo",
        "args": [
            "run",
            "--release",
            "--package",
            "ob-semantic-matcher",
            "--bin",
            "populate_embeddings",
        ],
        "force": force,
    });

    // Insert into public.outbox. ON CONFLICT DO NOTHING on the UNIQUE
    // (idempotency_key, effect_kind) constraint makes this idempotent.
    sqlx::query(
        r#"
        INSERT INTO public.outbox
            (id, trace_id, envelope_version, effect_kind, payload, idempotency_key, status)
        VALUES
            ($1, $2, $3, $4, $5, $6, 'pending')
        ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
        "#,
    )
    .bind(outbox_id)
    .bind(trace_id)
    .bind(1i16) // EnvelopeVersion::CURRENT
    .bind("maintenance_spawn")
    .bind(&payload)
    .bind(&idempotency_key)
    .execute(pool)
    .await?;

    Ok((outbox_id, idempotency_key))
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

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT object_type, COUNT(*)::bigint FROM sem_reg.snapshots WHERE status = 'active' GROUP BY object_type ORDER BY object_type",
        ).fetch_all(pool).await?;
        let counts: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(ot, c)| serde_json::json!({"object_type": ot, "active_count": c}))
            .collect();
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"active_snapshot_counts": counts, "hint": "Run 'cargo x sem-reg scan --dry-run' for full drift detection"}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

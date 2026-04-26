//! Maintenance domain verbs (Spec §C.6 — 7 verbs) — SemOS-side
//! YAML-first re-implementation.
//!
//! Mixed backends: health queries, cleanup, bootstrap signal, outbox
//! status, reindex-embeddings (outbox-queued, Pattern A A1 remediation
//! per Phase 0g — `docs/todo/pattern-b-a1-remediation-ledger.md` §2),
//! and schema-sync validation. All writes go through `scope.executor()`
//! so they participate in the Sequencer-owned txn. Allowed in Governed
//! mode only.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::domain_ops::helpers::json_extract_bool_opt;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct HealthPending;

#[async_trait]
impl SemOsVerbOp for HealthPending {
    fn fqn(&self) -> &str {
        "maintenance.health-pending"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT status, COUNT(*)::bigint FROM sem_reg.changesets GROUP BY status ORDER BY status",
        )
        .fetch_all(scope.executor())
        .await?;
        let entries: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(status, count)| serde_json::json!({"status": status, "count": count}))
            .collect();
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"pending_changesets": entries}),
        ))
    }
}

pub struct HealthStaleDryruns;

#[async_trait]
impl SemOsVerbOp for HealthStaleDryruns {
    fn fqn(&self) -> &str {
        "maintenance.health-stale-dryruns"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT c.changeset_id::text, c.status FROM sem_reg.changesets c WHERE c.status = 'dry_run_passed' AND c.updated_at < NOW() - INTERVAL '7 days' ORDER BY c.updated_at ASC LIMIT 50",
        )
        .fetch_all(scope.executor())
        .await?;
        let stale: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(id, status)| serde_json::json!({"changeset_id": id, "status": status}))
            .collect();
        let count = stale.len();
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"stale_dryruns": stale, "count": count}),
        ))
    }
}

pub struct Cleanup;

#[async_trait]
impl SemOsVerbOp for Cleanup {
    fn fqn(&self) -> &str {
        "maintenance.cleanup"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let archived = sqlx::query_scalar::<_, i64>(
            "WITH moved AS (INSERT INTO sem_reg_authoring.change_sets_archive SELECT * FROM sem_reg.changesets WHERE status IN ('rejected', 'dry_run_failed') AND updated_at < NOW() - INTERVAL '90 days' ON CONFLICT DO NOTHING RETURNING 1) SELECT COUNT(*) FROM moved",
        )
        .fetch_one(scope.executor())
        .await
        .unwrap_or(0);
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"archived_count": archived, "status": "cleanup_complete"}),
        ))
    }
}

pub struct BootstrapSeeds;

#[async_trait]
impl SemOsVerbOp for BootstrapSeeds {
    fn fqn(&self) -> &str {
        "maintenance.bootstrap-seeds"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"status": "bootstrap_seeds must be triggered via server startup or CLI", "hint": "Use: cargo x sem-reg scan"}),
        ))
    }
}

pub struct DrainOutbox;

#[async_trait]
impl SemOsVerbOp for DrainOutbox {
    fn fqn(&self) -> &str {
        "maintenance.drain-outbox"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sem_reg.outbox_events WHERE processed_at IS NULL",
        )
        .fetch_one(scope.executor())
        .await
        .unwrap_or(0);
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "pending_outbox_events": pending,
            "status": if pending == 0 { "drained" } else { "has_pending" }
        })))
    }
}

/// `maintenance.reindex-embeddings` — queues a populate_embeddings
/// subprocess spawn via `public.outbox` (Phase 0g Pattern A A1
/// remediation, D11). The Phase 5e drainer picks it up post-commit.
pub struct ReindexEmbeddings;

#[async_trait]
impl SemOsVerbOp for ReindexEmbeddings {
    fn fqn(&self) -> &str {
        "maintenance.reindex-embeddings"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let force = json_extract_bool_opt(args, "force").unwrap_or(false);
        let (outbox_id, idempotency_key) = enqueue_reindex_embeddings(scope, force).await?;

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "status": "queued",
            "force": force,
            "outbox_id": outbox_id.to_string(),
            "idempotency_key": idempotency_key,
            "drainer": "Phase 5e outbox drainer will spawn the populate_embeddings subprocess post-commit.",
        })))
    }
}

async fn enqueue_reindex_embeddings(
    scope: &mut dyn TransactionScope,
    force: bool,
) -> Result<(uuid::Uuid, String)> {
    let outbox_id = uuid::Uuid::new_v4();
    let trace_id = uuid::Uuid::new_v4();

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
    .execute(scope.executor())
    .await?;

    Ok((outbox_id, idempotency_key))
}

pub struct ValidateSchemaSync;

#[async_trait]
impl SemOsVerbOp for ValidateSchemaSync {
    fn fqn(&self) -> &str {
        "maintenance.validate-schema-sync"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT object_type, COUNT(*)::bigint FROM sem_reg.snapshots WHERE status = 'active' GROUP BY object_type ORDER BY object_type",
        )
        .fetch_all(scope.executor())
        .await?;
        let counts: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(ot, c)| serde_json::json!({"object_type": ot, "active_count": c}))
            .collect();
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "active_snapshot_counts": counts,
            "hint": "Run 'cargo x sem-reg scan --dry-run' for full drift detection"
        })))
    }
}

//! bpmn-controller plugin verbs (7 ops) — Pattern B bridges to the
//! `bpmn-controller` crate (`loader.*` + `bpmn-controller.*`).
//!
//! Read/provisioning ops follow the pre_fetch → execute pattern from
//! bpmn_lite_ops:
//! - `pre_fetch` performs the actual work against bpmn-lite's DB (+ K8s for
//!   pool mutations) outside the ob-poc transaction scope.
//! - `execute` reads the pre-fetched result from args and returns the outcome.
//!
//! T0.4 (EOP-PLAN-CONTROLPLANE-001, closes C-037) exception:
//! `bpmn-controller.start-instance` does its write in `execute`, not
//! `pre_fetch` — see the note on `BpmnControllerStartInstance` below.
//!
//! Runtime requirement: `BPMN_LITE_DATABASE_URL` must point to bpmn-lite's
//! Postgres instance (with migrations 001–032 applied). Pool mutating verbs
//! additionally require a reachable Kubernetes API server.

use anyhow::{Context, Result};
use async_trait::async_trait;
use bpmn_controller::{
    deprovision_pool, instance_status, list_pools, list_tenant_instances, pool_status,
    provision_pool, start_instance, K8sClient,
};
use ob_poc_types::{PoolConfig, PoolType};
use sem_os_postgres::ops::SemOsVerbOp;
use std::sync::OnceLock;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

// ── Shared bpmn-lite DB pool ──────────────────────────────────────────────────

/// Return the shared lazy pool for bpmn-lite's Postgres DB.
///
/// The pool is initialised once on first call using `BPMN_LITE_DATABASE_URL`.
/// `connect_lazy` defers the actual TCP handshake to the first query, so
/// startup cost is negligible.
fn bpmn_lite_pool() -> Result<&'static sqlx::PgPool> {
    static POOL: OnceLock<sqlx::PgPool> = OnceLock::new();

    if let Some(p) = POOL.get() {
        return Ok(p);
    }

    let url = std::env::var("BPMN_LITE_DATABASE_URL")
        .context("BPMN_LITE_DATABASE_URL not set; bpmn-controller verbs require this env var")?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect_lazy(&url)
        .context("bpmn-controller: failed to create lazy pool")?;

    // Harmless race: if two threads init simultaneously, one pool is dropped.
    let _ = POOL.set(pool);
    Ok(POOL.get().expect("just set"))
}

// ── Arg extraction helpers ────────────────────────────────────────────────────

fn extract_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .with_context(|| format!("required arg '{}' missing or not a string", key))
}

fn extract_str_opt<'a>(args: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn extract_pool_config(args: &serde_json::Value) -> Result<PoolConfig> {
    let image = extract_str(args, "image")?.to_string();
    let namespace =
        std::env::var("BPMN_LITE_K8S_NAMESPACE").unwrap_or_else(|_| "default".to_string());
    Ok(PoolConfig {
        image,
        replicas: args.get("replicas").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
        min_replicas: args
            .get("min-replicas")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32,
        max_replicas: args
            .get("max-replicas")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as u32,
        cpu_request: extract_str_opt(args, "cpu-request")
            .unwrap_or("250m")
            .to_string(),
        memory_request: extract_str_opt(args, "memory-request")
            .unwrap_or("256Mi")
            .to_string(),
        cpu_limit: extract_str_opt(args, "cpu-limit")
            .unwrap_or("1000m")
            .to_string(),
        memory_limit: extract_str_opt(args, "memory-limit")
            .unwrap_or("512Mi")
            .to_string(),
        namespace,
    })
}

fn extract_pool_type(args: &serde_json::Value) -> PoolType {
    match extract_str_opt(args, "pool-type") {
        Some("dedicated") => PoolType::Dedicated,
        _ => PoolType::Default,
    }
}

fn extract_tenants(args: &serde_json::Value) -> Vec<String> {
    args.get("tenants")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ── loader.provision-pool ─────────────────────────────────────────────────────

pub(super) struct LoaderProvisionPool;

#[async_trait]
impl SemOsVerbOp for LoaderProvisionPool {
    fn fqn(&self) -> &str {
        "loader.provision-pool"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let pg = bpmn_lite_pool()?;
        let k8s = K8sClient::from_infer()
            .await
            .unwrap_or_else(|_| K8sClient::placeholder());

        let pool_id = extract_str(args, "pool-id")?;
        let pool_type = extract_pool_type(args);
        let tenants = extract_tenants(args);
        let config = extract_pool_config(args)?;

        provision_pool(pg, &k8s, pool_id, pool_type, &tenants, config).await?;
        Ok(Some(serde_json::json!({"_ok": true})))
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        Ok(VerbExecutionOutcome::Void)
    }
}

// ── loader.deprovision-pool ───────────────────────────────────────────────────

pub(super) struct LoaderDeprovisionPool;

#[async_trait]
impl SemOsVerbOp for LoaderDeprovisionPool {
    fn fqn(&self) -> &str {
        "loader.deprovision-pool"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let pg = bpmn_lite_pool()?;
        let k8s = K8sClient::from_infer()
            .await
            .unwrap_or_else(|_| K8sClient::placeholder());

        let pool_id = extract_str(args, "pool-id")?;
        deprovision_pool(pg, &k8s, pool_id).await?;
        Ok(Some(serde_json::json!({"_ok": true})))
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        Ok(VerbExecutionOutcome::Void)
    }
}

// ── loader.pool-status ────────────────────────────────────────────────────────

pub(super) struct LoaderPoolStatus;

#[async_trait]
impl SemOsVerbOp for LoaderPoolStatus {
    fn fqn(&self) -> &str {
        "loader.pool-status"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let pg = bpmn_lite_pool()?;
        // Pool status only reads K8s; placeholder gives None for pod counts.
        let k8s = K8sClient::from_infer()
            .await
            .unwrap_or_else(|_| K8sClient::placeholder());

        let pool_id = extract_str(args, "pool-id")?;
        let status = pool_status(pg, &k8s, pool_id).await?;
        Ok(Some(serde_json::json!({
            "_pool_status": serde_json::to_value(status)?
        })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let result = args
            .get("_pool_status")
            .cloned()
            .context("loader.pool-status: pre_fetch result missing")?;
        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ── loader.list-pools ─────────────────────────────────────────────────────────

pub(super) struct LoaderListPools;

#[async_trait]
impl SemOsVerbOp for LoaderListPools {
    fn fqn(&self) -> &str {
        "loader.list-pools"
    }

    async fn pre_fetch(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let pg = bpmn_lite_pool()?;
        let pools = list_pools(pg).await?;
        let rows: Vec<serde_json::Value> = pools
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(Some(serde_json::json!({"_pools": rows})))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rows = args
            .get("_pools")
            .and_then(|v| v.as_array())
            .context("loader.list-pools: pre_fetch result missing")?
            .clone();
        Ok(VerbExecutionOutcome::RecordSet(rows))
    }
}

// ── bpmn-controller.start-instance ───────────────────────────────────────────
//
// T0.4 (EOP-PLAN-CONTROLPLANE-001, closes C-037): the ONE exception to this
// file's module-level "work happens in pre_fetch" pattern. Every other op
// here is a read (or a pool-provisioning op out of T0.4's scope); this one
// is a write with a durable side effect (`process_instances` row in
// bpmn-lite's DB). Per the `SemOsVerbOp::pre_fetch` contract ("do HTTP in
// pre_fetch, DB writes in execute"), the write belongs in `execute` — it
// used to run in `pre_fetch`, which the dispatcher calls BEFORE the
// execute-scope even opens, so a failure anywhere else in the op's
// lifecycle could never see, correlate with, or compensate the already-
// started bpmn-lite instance. `start_instance`'s idempotency-key check
// (`bpmn-controller/src/instance.rs:L100-L190`) still protects retries —
// this relocation doesn't add new safety there, it just puts the write in
// the phase the framework actually intends for writes.
pub(super) struct BpmnControllerStartInstance;

#[async_trait]
impl SemOsVerbOp for BpmnControllerStartInstance {
    fn fqn(&self) -> &str {
        "bpmn-controller.start-instance"
    }

    // No pre_fetch override — see the T0.4 note above.

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pg = bpmn_lite_pool()?;

        let tenant_id = extract_str(args, "tenant-id")?;
        let process_key = extract_str(args, "process-key")?;
        let payload = args
            .get("payload")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let idempotency_key = extract_str_opt(args, "idempotency-key");

        let instance_id =
            start_instance(pg, tenant_id, process_key, payload, idempotency_key).await?;

        Ok(VerbExecutionOutcome::Uuid(instance_id))
    }
}

// ── bpmn-controller.instance-status ──────────────────────────────────────────

pub(super) struct BpmnControllerInstanceStatus;

#[async_trait]
impl SemOsVerbOp for BpmnControllerInstanceStatus {
    fn fqn(&self) -> &str {
        "bpmn-controller.instance-status"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let pg = bpmn_lite_pool()?;
        let id_str = extract_str(args, "instance-id")?;
        let id = Uuid::parse_str(id_str)
            .context("bpmn-controller.instance-status: instance-id is not a valid UUID")?;
        let status = instance_status(pg, id).await?;
        Ok(Some(serde_json::json!({
            "_instance_status": serde_json::to_value(status)?
        })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let result = args
            .get("_instance_status")
            .cloned()
            .context("bpmn-controller.instance-status: pre_fetch result missing")?;
        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ── bpmn-controller.list-instances ───────────────────────────────────────────

pub(super) struct BpmnControllerListInstances;

#[async_trait]
impl SemOsVerbOp for BpmnControllerListInstances {
    fn fqn(&self) -> &str {
        "bpmn-controller.list-instances"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let pg = bpmn_lite_pool()?;
        let tenant_id = extract_str(args, "tenant-id")?;
        let instances = list_tenant_instances(pg, tenant_id).await?;
        let rows: Vec<serde_json::Value> = instances
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(Some(serde_json::json!({"_instances": rows})))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rows = args
            .get("_instances")
            .and_then(|v| v.as_array())
            .context("bpmn-controller.list-instances: pre_fetch result missing")?
            .clone();
        Ok(VerbExecutionOutcome::RecordSet(rows))
    }
}

// ── T0.4 tests (EOP-PLAN-CONTROLPLANE-001, closes C-037) ────────────────────

#[cfg(test)]
mod t0_4_tests {
    use super::*;

    /// `TransactionScope` double: `BpmnControllerStartInstance::execute`
    /// never calls `scope.executor()`/`transaction()`/`pool()` (its write
    /// goes through `bpmn_lite_pool()`, a separate connection pool from the
    /// ob-poc scope) — a panic on any of those methods is itself part of
    /// the assertion that the op doesn't misuse the ob-poc transaction.
    struct PanicScope;

    impl TransactionScope for PanicScope {
        fn scope_id(&self) -> ob_poc_types::TransactionScopeId {
            ob_poc_types::TransactionScopeId::new()
        }
        fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres> {
            panic!("PanicScope: transaction() reached — this op must not touch the ob-poc scope")
        }
        fn pool(&self) -> &sqlx::PgPool {
            panic!("PanicScope: pool() reached — this op must not touch the ob-poc scope")
        }
    }

    async fn seed_tenant(pg: &sqlx::PgPool, tenant_id: &str) {
        sqlx::query("INSERT INTO tenants (tenant_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(tenant_id)
            .execute(pg)
            .await
            .unwrap();
    }

    /// Insert a minimal published workflow_template for test use.
    /// 64-char hex string — fake but structurally valid (decimal digits are
    /// valid hex), matching `bpmn-controller/tests/integration_test.rs`.
    async fn seed_template(pg: &sqlx::PgPool, process_key: &str) -> String {
        let bytecode_hex = format!("{:0>64}", process_key.len());
        sqlx::query(
            "INSERT INTO workflow_templates \
             (template_key, template_version, process_key, bytecode_version, \
              state, dto_snapshot, task_manifest) \
             VALUES ($1, 1, $2, $3, 'published', '{}'::jsonb, '[]'::jsonb) \
             ON CONFLICT DO NOTHING",
        )
        .bind(format!("t0-4-test-{}", process_key))
        .bind(process_key)
        .bind(&bytecode_hex)
        .execute(pg)
        .await
        .unwrap();
        bytecode_hex
    }

    async fn count_instances_for_correlation(pg: &sqlx::PgPool, correlation_id: &str) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM process_instances WHERE correlation_id = $1")
            .bind(correlation_id)
            .fetch_one(pg)
            .await
            .unwrap()
    }

    /// T0.4 exit criterion — "op failure after execute-start leaves no
    /// process_instances row (or a compensated one)":
    ///
    /// 1. `pre_fetch` is now the trait default (no override) — proves the
    ///    write no longer happens before the execute-scope opens: calling
    ///    it creates zero rows.
    /// 2. `execute` performs the write and returns the real instance id.
    /// 3. Retrying `execute` with the SAME idempotency key (simulating a
    ///    caller retry after some unrelated failure downstream of this
    ///    op's own return) is compensated by `start_instance`'s existing
    ///    idempotency check — same id, no second row.
    #[tokio::test]
    #[ignore = "requires BPMN_LITE_DATABASE_URL (bpmn-lite schema)"]
    async fn t0_4_write_moved_to_execute_and_retry_is_compensated() {
        let url = std::env::var("BPMN_LITE_DATABASE_URL")
            .expect("BPMN_LITE_DATABASE_URL must be set");
        let pg = sqlx::PgPool::connect(&url).await.expect("connect");

        let tenant_id = "t0-4-test-tenant";
        let process_key = "t0-4-test-process";
        let idempotency_key = format!("t0-4-idem-{}", Uuid::new_v4());
        seed_tenant(&pg, tenant_id).await;
        seed_template(&pg, process_key).await;

        let op = BpmnControllerStartInstance;
        let args = serde_json::json!({
            "tenant-id": tenant_id,
            "process-key": process_key,
            "payload": {"amount": 1},
            "idempotency-key": idempotency_key,
        });

        // 1. pre_fetch is the trait default — no DB write.
        let mut ctx = VerbExecutionContext::default();
        let pre = op.pre_fetch(&args, &mut ctx, &pg).await.unwrap();
        assert!(
            pre.is_none(),
            "pre_fetch must be a no-op post-T0.4 (write relocated to execute)"
        );
        assert_eq!(
            count_instances_for_correlation(&pg, &idempotency_key).await,
            0,
            "pre_fetch must not have created a process_instances row"
        );

        // 2. execute performs the write.
        let mut scope = PanicScope;
        let outcome1 = op.execute(&args, &mut ctx, &mut scope).await.unwrap();
        let id1 = match outcome1 {
            VerbExecutionOutcome::Uuid(id) => id,
            other => panic!("expected Uuid outcome, got {other:?}"),
        };
        assert_eq!(
            count_instances_for_correlation(&pg, &idempotency_key).await,
            1,
            "execute must have created exactly one process_instances row"
        );

        // 3. Retry with the same idempotency key — compensated, not duplicated.
        let outcome2 = op.execute(&args, &mut ctx, &mut scope).await.unwrap();
        let id2 = match outcome2 {
            VerbExecutionOutcome::Uuid(id) => id,
            other => panic!("expected Uuid outcome, got {other:?}"),
        };
        assert_eq!(id1, id2, "retry with same idempotency key must return the same instance");
        assert_eq!(
            count_instances_for_correlation(&pg, &idempotency_key).await,
            1,
            "retry must not create a second process_instances row (compensated)"
        );
    }
}

//! bpmn-controller plugin verbs (7 ops) — Pattern B bridges to the
//! `bpmn-controller` crate (`loader.*` + `bpmn-controller.*`).
//!
//! All ops follow the pre_fetch → execute pattern from bpmn_lite_ops:
//! - `pre_fetch` performs the actual work against bpmn-lite's DB (+ K8s for
//!   pool mutations) outside the ob-poc transaction scope.
//! - `execute` reads the pre-fetched result from args and returns the outcome.
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

use dsl_runtime::tx::TransactionScope;
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

pub struct LoaderProvisionPool;

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

pub struct LoaderDeprovisionPool;

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

pub struct LoaderPoolStatus;

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

pub struct LoaderListPools;

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
            .map(|p| serde_json::to_value(p))
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

pub struct BpmnControllerStartInstance;

#[async_trait]
impl SemOsVerbOp for BpmnControllerStartInstance {
    fn fqn(&self) -> &str {
        "bpmn-controller.start-instance"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
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

        Ok(Some(serde_json::json!({
            "_instance_id": instance_id.to_string()
        })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let id_str = args
            .get("_instance_id")
            .and_then(|v| v.as_str())
            .context("bpmn-controller.start-instance: pre_fetch result missing")?;
        let id = Uuid::parse_str(id_str).context("bpmn-controller.start-instance: invalid UUID")?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── bpmn-controller.instance-status ──────────────────────────────────────────

pub struct BpmnControllerInstanceStatus;

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

pub struct BpmnControllerListInstances;

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
            .map(|i| serde_json::to_value(i))
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

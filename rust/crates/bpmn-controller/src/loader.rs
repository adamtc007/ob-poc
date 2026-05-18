use anyhow::Result;
use chrono::{DateTime, Utc};
use ob_poc_types::{Pool, PoolConfig, PoolStatus, PoolType};
use sqlx::PgPool;
use tracing::{info, warn};

use crate::deployment;
use crate::error::BpmnControllerError;
use crate::k8s::K8sClient;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pool_type_from_str(s: &str) -> PoolType {
    if s == "dedicated" {
        PoolType::Dedicated
    } else {
        PoolType::Default
    }
}

fn pool_type_to_str(pt: PoolType) -> &'static str {
    match pt {
        PoolType::Default => "default",
        PoolType::Dedicated => "dedicated",
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// List all pools ordered by creation time.
pub async fn list_pools(pg: &PgPool) -> Result<Vec<Pool>> {
    let rows = sqlx::query(
        "SELECT pool_id, pool_type, description, paused, created_at \
         FROM tenant_pools \
         ORDER BY created_at",
    )
    .fetch_all(pg)
    .await?;

    use sqlx::Row;
    let pools = rows
        .iter()
        .map(|r| Pool {
            pool_id: r.get("pool_id"),
            pool_type: pool_type_from_str(r.get::<&str, _>("pool_type")),
            description: r.get("description"),
            paused: r.get("paused"),
            created_at: r.get::<DateTime<Utc>, _>("created_at"),
        })
        .collect();

    Ok(pools)
}

/// Return current status of a pool, including live K8s replica counts.
pub async fn pool_status(pg: &PgPool, k8s: &K8sClient, pool_id: &str) -> Result<PoolStatus> {
    use sqlx::Row;

    let pool_row =
        sqlx::query("SELECT pool_id, pool_type, paused FROM tenant_pools WHERE pool_id = $1")
            .bind(pool_id)
            .fetch_optional(pg)
            .await?;

    let pool_row =
        pool_row.ok_or_else(|| BpmnControllerError::PoolNotFound(pool_id.to_string()))?;

    let tenant_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenants WHERE pool_id = $1")
        .bind(pool_id)
        .fetch_one(pg)
        .await?;

    let queue_depth: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM process_instances pi \
         JOIN tenants t ON pi.tenant_id = t.tenant_id \
         WHERE t.pool_id = $1 \
           AND pi.state = '\"Running\"'::jsonb \
           AND pi.quarantine_state IS NULL",
    )
    .bind(pool_id)
    .fetch_one(pg)
    .await?;

    let (desired_replicas, ready_replicas) =
        match deployment::get_deployment_status(k8s, pool_id).await {
            Ok(Some(s)) => (s.desired, s.ready),
            Ok(None) => (None, None),
            Err(e) => {
                warn!(pool_id, "K8s get_deployment_status failed: {}", e);
                (None, None)
            }
        };

    Ok(PoolStatus {
        pool_id: pool_row.get("pool_id"),
        pool_type: pool_type_from_str(pool_row.get::<&str, _>("pool_type")),
        paused: pool_row.get("paused"),
        tenant_count: tenant_count as usize,
        queue_depth,
        desired_replicas,
        ready_replicas,
    })
}

/// List tenant IDs assigned to a pool, ordered by first_seen_at.
pub async fn list_pool_tenants(pg: &PgPool, pool_id: &str) -> Result<Vec<String>> {
    let rows =
        sqlx::query("SELECT tenant_id FROM tenants WHERE pool_id = $1 ORDER BY first_seen_at")
            .bind(pool_id)
            .fetch_all(pg)
            .await?;

    use sqlx::Row;
    Ok(rows
        .iter()
        .map(|r| r.get::<String, _>("tenant_id"))
        .collect())
}

/// Provision a new pool: validate, write DB rows, create K8s Deployment + HPA.
///
/// **Idempotent.** The DB insert uses `ON CONFLICT DO NOTHING`; tenant
/// reassignment and K8s server-side apply are both no-ops if unchanged. Safe
/// to retry on partial failure at any step.
///
/// **Non-atomic across Postgres + K8s.** If K8s fails after the DB write, the
/// pool row exists but has no Deployment. Retrying is safe: the DB insert is a
/// no-op and the K8s apply creates the missing Deployment.
///
/// # Errors
/// - `TenantNotFound` — a listed tenant does not exist in the `tenants` table.
pub async fn provision_pool(
    pg: &PgPool,
    k8s: &K8sClient,
    pool_id: &str,
    pool_type: PoolType,
    tenants: &[String],
    config: PoolConfig,
) -> Result<()> {
    // 1. Validate all listed tenants exist before touching the DB.
    if !tenants.is_empty() {
        let found: Vec<String> =
            sqlx::query_scalar("SELECT tenant_id FROM tenants WHERE tenant_id = ANY($1)")
                .bind(tenants.to_vec())
                .fetch_all(pg)
                .await?;
        for tenant_id in tenants {
            if !found.contains(tenant_id) {
                return Err(BpmnControllerError::TenantNotFound(tenant_id.clone()).into());
            }
        }
    }

    // 2. DB: insert pool row + assign tenants atomically.
    let type_str = pool_type_to_str(pool_type);
    let mut tx = pg.begin().await?;

    sqlx::query(
        "INSERT INTO tenant_pools (pool_id, pool_type) VALUES ($1, $2) \
         ON CONFLICT (pool_id) DO NOTHING",
    )
    .bind(pool_id)
    .bind(type_str)
    .execute(&mut *tx)
    .await?;

    if !tenants.is_empty() {
        sqlx::query("UPDATE tenants SET pool_id = $1 WHERE tenant_id = ANY($2)")
            .bind(pool_id)
            .bind(tenants.to_vec())
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    // 3. K8s: create Deployment + HPA (idempotent, outside DB transaction).
    deployment::create_deployment(k8s, pool_id, type_str, &config).await?;
    deployment::create_hpa(k8s, pool_id, &config).await?;

    info!(
        pool_id,
        tenant_count = tenants.len(),
        pool_type = type_str,
        "pool provisioned"
    );
    Ok(())
}

/// Deprovision a pool: validate, delete K8s resources, remove DB row.
///
/// **Idempotent.** K8s deletes are 404-tolerant. The DB delete is a no-op if
/// the row was already removed.
///
/// **Non-atomic across Postgres + K8s.** K8s is cleaned up first. If the DB
/// delete fails after K8s cleanup, retrying is safe: K8s returns 404 (no-op)
/// and the DB delete succeeds on the second attempt.
///
/// # Errors
/// - `CannotDeprovisionDefaultPool` — pool_id is `"default"`.
/// - `PoolNotFound` — pool does not exist.
/// - `PoolHasTenants` — tenants are still assigned; caller must reassign first.
pub async fn deprovision_pool(pg: &PgPool, k8s: &K8sClient, pool_id: &str) -> Result<()> {
    if pool_id == "default" {
        return Err(BpmnControllerError::CannotDeprovisionDefaultPool.into());
    }

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_pools WHERE pool_id = $1)")
            .bind(pool_id)
            .fetch_one(pg)
            .await?;

    if !exists {
        return Err(BpmnControllerError::PoolNotFound(pool_id.to_string()).into());
    }

    let tenant_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenants WHERE pool_id = $1")
        .bind(pool_id)
        .fetch_one(pg)
        .await?;

    if tenant_count > 0 {
        return Err(BpmnControllerError::PoolHasTenants(pool_id.to_string()).into());
    }

    // K8s first — if this fails the pool row survives and retry is safe.
    deployment::delete_deployment(k8s, pool_id).await?;
    deployment::delete_hpa(k8s, pool_id).await?;

    sqlx::query("DELETE FROM tenant_pools WHERE pool_id = $1")
        .bind(pool_id)
        .execute(pg)
        .await?;

    info!(pool_id, "pool deprovisioned");
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_type_roundtrip() {
        assert_eq!(pool_type_from_str("default"), PoolType::Default);
        assert_eq!(pool_type_from_str("dedicated"), PoolType::Dedicated);
        assert_eq!(pool_type_from_str("unknown"), PoolType::Default);
        assert_eq!(pool_type_to_str(PoolType::Default), "default");
        assert_eq!(pool_type_to_str(PoolType::Dedicated), "dedicated");
    }
}

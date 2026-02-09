//! PendingDispatchStore — local durable queue for BPMN dispatch requests.
//!
//! When the bpmn-lite gRPC service is temporarily unavailable, the
//! `WorkflowDispatcher` persists dispatch requests here. The
//! `PendingDispatchWorker` background task scans this queue and retries
//! periodically until the service recovers.
//!
//! Idempotency: a UNIQUE index on `(payload_hash WHERE status='pending')`
//! prevents duplicate pending entries for the same canonical payload.

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

use super::types::{PendingDispatch, PendingDispatchStatus};

/// Postgres-backed store for pending BPMN dispatch requests.
pub struct PendingDispatchStore {
    pool: PgPool,
}

impl PendingDispatchStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a pending dispatch. Returns `false` if a pending dispatch
    /// with the same payload_hash already exists (idempotent).
    pub async fn insert(&self, dispatch: &PendingDispatch) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".bpmn_pending_dispatches
                (dispatch_id, payload_hash, verb_fqn, process_key,
                 bytecode_version, domain_payload, dsl_source,
                 entry_id, runbook_id, correlation_id, correlation_key,
                 domain_correlation_key, status, attempts, last_error,
                 created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (payload_hash) WHERE status = 'pending'
            DO NOTHING
            "#,
            dispatch.dispatch_id,
            dispatch.payload_hash,
            dispatch.verb_fqn,
            dispatch.process_key,
            dispatch.bytecode_version,
            dispatch.domain_payload,
            dispatch.dsl_source,
            dispatch.entry_id,
            dispatch.runbook_id,
            dispatch.correlation_id,
            dispatch.correlation_key,
            dispatch.domain_correlation_key.as_deref(),
            dispatch.status.as_str(),
            dispatch.attempts,
            dispatch.last_error.as_deref(),
            dispatch.created_at,
        )
        .execute(&self.pool)
        .await
        .context("Failed to insert bpmn_pending_dispatch")?;

        Ok(result.rows_affected() > 0)
    }

    /// Claim up to `limit` pending dispatches for retry.
    ///
    /// Uses `FOR UPDATE SKIP LOCKED` to allow concurrent workers (future-proof).
    /// Only returns rows where `last_attempted_at` is older than `backoff` or NULL.
    pub async fn claim_pending(
        &self,
        limit: i32,
        backoff: Duration,
    ) -> Result<Vec<PendingDispatch>> {
        let backoff_secs = backoff.as_secs() as i32;

        let rows = sqlx::query!(
            r#"
            SELECT dispatch_id, payload_hash, verb_fqn, process_key,
                   bytecode_version, domain_payload, dsl_source,
                   entry_id, runbook_id, correlation_id, correlation_key,
                   domain_correlation_key, status, attempts, last_error,
                   created_at, last_attempted_at, dispatched_at
            FROM "ob-poc".bpmn_pending_dispatches
            WHERE status = 'pending'
              AND (last_attempted_at IS NULL
                   OR last_attempted_at < now() - make_interval(secs => $1::double precision))
            ORDER BY created_at ASC
            LIMIT $2
            FOR UPDATE SKIP LOCKED
            "#,
            backoff_secs as f64,
            limit as i64,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to claim pending dispatches")?;

        Ok(rows
            .into_iter()
            .map(|r| PendingDispatch {
                dispatch_id: r.dispatch_id,
                payload_hash: r.payload_hash,
                verb_fqn: r.verb_fqn,
                process_key: r.process_key,
                bytecode_version: r.bytecode_version,
                domain_payload: r.domain_payload,
                dsl_source: r.dsl_source,
                entry_id: r.entry_id,
                runbook_id: r.runbook_id,
                correlation_id: r.correlation_id,
                correlation_key: r.correlation_key,
                domain_correlation_key: r.domain_correlation_key,
                status: PendingDispatchStatus::parse(&r.status)
                    .unwrap_or(PendingDispatchStatus::Pending),
                attempts: r.attempts,
                last_error: r.last_error,
                created_at: r.created_at,
                last_attempted_at: r.last_attempted_at,
                dispatched_at: r.dispatched_at,
            })
            .collect())
    }

    /// Mark a dispatch as successfully sent to bpmn-lite.
    pub async fn mark_dispatched(&self, dispatch_id: Uuid) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_pending_dispatches
            SET status = 'dispatched', dispatched_at = now()
            WHERE dispatch_id = $1 AND status = 'pending'
            "#,
            dispatch_id,
        )
        .execute(&self.pool)
        .await
        .context("Failed to mark pending dispatch as dispatched")?;

        Ok(result.rows_affected() > 0)
    }

    /// Record a failed retry attempt. If `attempts >= max_attempts`, sets
    /// status to `failed_permanent`.
    pub async fn record_failure(
        &self,
        dispatch_id: Uuid,
        error: &str,
        max_attempts: i32,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_pending_dispatches
            SET attempts = attempts + 1,
                last_error = $2,
                last_attempted_at = now(),
                status = CASE
                    WHEN attempts + 1 >= $3 THEN 'failed_permanent'
                    ELSE 'pending'
                END
            WHERE dispatch_id = $1 AND status = 'pending'
            "#,
            dispatch_id,
            error,
            max_attempts,
        )
        .execute(&self.pool)
        .await
        .context("Failed to record pending dispatch failure")?;

        Ok(result.rows_affected() > 0)
    }

    /// List all pending dispatches (for monitoring).
    pub async fn list_pending(&self) -> Result<Vec<PendingDispatch>> {
        let rows = sqlx::query!(
            r#"
            SELECT dispatch_id, payload_hash, verb_fqn, process_key,
                   bytecode_version, domain_payload, dsl_source,
                   entry_id, runbook_id, correlation_id, correlation_key,
                   domain_correlation_key, status, attempts, last_error,
                   created_at, last_attempted_at, dispatched_at
            FROM "ob-poc".bpmn_pending_dispatches
            WHERE status = 'pending'
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list pending dispatches")?;

        Ok(rows
            .into_iter()
            .map(|r| PendingDispatch {
                dispatch_id: r.dispatch_id,
                payload_hash: r.payload_hash,
                verb_fqn: r.verb_fqn,
                process_key: r.process_key,
                bytecode_version: r.bytecode_version,
                domain_payload: r.domain_payload,
                dsl_source: r.dsl_source,
                entry_id: r.entry_id,
                runbook_id: r.runbook_id,
                correlation_id: r.correlation_id,
                correlation_key: r.correlation_key,
                domain_correlation_key: r.domain_correlation_key,
                status: PendingDispatchStatus::parse(&r.status)
                    .unwrap_or(PendingDispatchStatus::Pending),
                attempts: r.attempts,
                last_error: r.last_error,
                created_at: r.created_at,
                last_attempted_at: r.last_attempted_at,
                dispatched_at: r.dispatched_at,
            })
            .collect())
    }
}

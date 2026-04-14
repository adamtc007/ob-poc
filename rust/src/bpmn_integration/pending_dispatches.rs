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
        let session_stack = serde_json::to_value(&dispatch.session_stack)
            .context("Failed to serialize pending dispatch session_stack")?;
        let result = sqlx::query(
            r#"
            INSERT INTO "ob-poc".bpmn_pending_dispatches
                (dispatch_id, payload_hash, verb_fqn, process_key,
                 bytecode_version, domain_payload, session_stack, dsl_source,
                 entry_id, runbook_id, correlation_id, correlation_key,
                 domain_correlation_key, status, attempts, last_error,
                 created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            ON CONFLICT (payload_hash) WHERE status = 'pending'
            DO NOTHING
            "#,
        )
        .bind(dispatch.dispatch_id)
        .bind(&dispatch.payload_hash)
        .bind(&dispatch.verb_fqn)
        .bind(&dispatch.process_key)
        .bind(&dispatch.bytecode_version)
        .bind(&dispatch.domain_payload)
        .bind(&session_stack)
        .bind(&dispatch.dsl_source)
        .bind(dispatch.entry_id)
        .bind(dispatch.runbook_id)
        .bind(dispatch.correlation_id)
        .bind(&dispatch.correlation_key)
        .bind(dispatch.domain_correlation_key.as_deref())
        .bind(dispatch.status.as_str())
        .bind(dispatch.attempts)
        .bind(dispatch.last_error.as_deref())
        .bind(dispatch.created_at)
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

        let rows = sqlx::query(
            r#"
            SELECT dispatch_id, payload_hash, verb_fqn, process_key,
                   bytecode_version, domain_payload, session_stack, dsl_source,
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
        )
        .bind(backoff_secs as f64)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .context("Failed to claim pending dispatches")?;

        rows.into_iter()
            .map(|r| {
                use sqlx::Row;
                let status = r.get::<String, _>("status");
                let session_stack =
                    serde_json::from_value(r.get::<serde_json::Value, _>("session_stack"))
                        .context("Failed to deserialize pending dispatch session_stack")?;
                Ok(PendingDispatch {
                    dispatch_id: r.get("dispatch_id"),
                    payload_hash: r.get("payload_hash"),
                    verb_fqn: r.get("verb_fqn"),
                    process_key: r.get("process_key"),
                    bytecode_version: r.get("bytecode_version"),
                    domain_payload: r.get("domain_payload"),
                    session_stack,
                    dsl_source: r.get("dsl_source"),
                    entry_id: r.get("entry_id"),
                    runbook_id: r.get("runbook_id"),
                    correlation_id: r.get("correlation_id"),
                    correlation_key: r.get("correlation_key"),
                    domain_correlation_key: r.get("domain_correlation_key"),
                    status: PendingDispatchStatus::parse(&status)
                        .unwrap_or(PendingDispatchStatus::Pending),
                    attempts: r.get("attempts"),
                    last_error: r.get("last_error"),
                    created_at: r.get("created_at"),
                    last_attempted_at: r.get("last_attempted_at"),
                    dispatched_at: r.get("dispatched_at"),
                })
            })
            .collect::<Result<Vec<_>>>()
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

}

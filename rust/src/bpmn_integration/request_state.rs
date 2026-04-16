//! RequestStateStore — requester-side lifecycle projection for durable BPMN requests.
//!
//! This store keeps a coarse-grained request view for `ob-poc`, separate from
//! BPMN's internal runtime state machine.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::{RequestStateRecord, RequestStatus};

/// Postgres-backed store for coarse request lifecycle state.
#[derive(Clone)]
pub struct RequestStateStore {
    pool: PgPool,
}

impl RequestStateStore {
    /// Create a new request-state store.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let store = RequestStateStore::new(pool.clone());
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert or refresh the initial `requested` state for a durable request.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// store.upsert_requested(&record).await?;
    /// ```
    pub async fn upsert_requested(&self, record: &RequestStateRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".bpmn_request_states
                (request_key, correlation_key, session_id, runbook_id, entry_id,
                 process_key, process_instance_id, status, requested_at,
                 started_at, completed_at, failed_at, killed_at, last_error)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, NULL, NULL, NULL, NULL)
            ON CONFLICT (request_key) DO UPDATE
                SET correlation_key = EXCLUDED.correlation_key,
                    session_id = EXCLUDED.session_id,
                    runbook_id = EXCLUDED.runbook_id,
                    entry_id = EXCLUDED.entry_id,
                    process_key = EXCLUDED.process_key,
                    status = EXCLUDED.status,
                    requested_at = EXCLUDED.requested_at,
                    process_instance_id = EXCLUDED.process_instance_id,
                    last_error = NULL,
                    started_at = NULL,
                    completed_at = NULL,
                    failed_at = NULL,
                    killed_at = NULL
            "#,
        )
        .bind(&record.request_key)
        .bind(&record.correlation_key)
        .bind(record.session_id)
        .bind(record.runbook_id)
        .bind(record.entry_id)
        .bind(&record.process_key)
        .bind(record.process_instance_id)
        .bind(RequestStatus::Requested.as_str())
        .bind(record.requested_at)
        .execute(&self.pool)
        .await
        .context("Failed to upsert bpmn_request_state (requested)")?;
        Ok(())
    }

    /// Mark a request as queued for later BPMN dispatch.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// store.mark_dispatch_pending(request_key, Some("grpc unavailable")).await?;
    /// ```
    pub async fn mark_dispatch_pending(
        &self,
        request_key: &str,
        last_error: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".bpmn_request_states
            SET status = $2,
                last_error = $3
            WHERE request_key = $1
            "#,
        )
        .bind(request_key)
        .bind(RequestStatus::DispatchPending.as_str())
        .bind(last_error)
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_request_state dispatch_pending")?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark a request as actively running in BPMN.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// store.mark_in_progress(request_key, process_instance_id).await?;
    /// ```
    pub async fn mark_in_progress(
        &self,
        request_key: &str,
        process_instance_id: Uuid,
    ) -> Result<bool> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".bpmn_request_states
            SET status = $2,
                process_instance_id = $3,
                started_at = COALESCE(started_at, $4),
                last_error = NULL
            WHERE request_key = $1
            "#,
        )
        .bind(request_key)
        .bind(RequestStatus::InProgress.as_str())
        .bind(process_instance_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_request_state in_progress")?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark a request as successfully returned.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// store.mark_returned(request_key).await?;
    /// ```
    pub async fn mark_returned(&self, request_key: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".bpmn_request_states
            SET status = $2,
                completed_at = now()
            WHERE request_key = $1
            "#,
        )
        .bind(request_key)
        .bind(RequestStatus::Returned.as_str())
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_request_state returned")?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark a request as killed/cancelled.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// store.mark_killed(request_key, Some("cancelled by operator")).await?;
    /// ```
    pub async fn mark_killed(&self, request_key: &str, reason: Option<&str>) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".bpmn_request_states
            SET status = $2,
                killed_at = now(),
                last_error = $3
            WHERE request_key = $1
            "#,
        )
        .bind(request_key)
        .bind(RequestStatus::Killed.as_str())
        .bind(reason)
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_request_state killed")?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark a request as failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// store.mark_failed(request_key, Some("incident")).await?;
    /// ```
    pub async fn mark_failed(&self, request_key: &str, error: Option<&str>) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".bpmn_request_states
            SET status = $2,
                failed_at = now(),
                last_error = $3
            WHERE request_key = $1
            "#,
        )
        .bind(request_key)
        .bind(RequestStatus::Failed.as_str())
        .bind(error)
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_request_state failed")?;
        Ok(result.rows_affected() > 0)
    }

    /// Find a request-state projection by request key.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let row = store.find_by_request_key(request_key).await?;
    /// ```
    pub async fn find_by_request_key(
        &self,
        request_key: &str,
    ) -> Result<Option<RequestStateRecord>> {
        let row = sqlx::query(
            r#"
            SELECT request_key, correlation_key, session_id, runbook_id, entry_id,
                   process_key, process_instance_id, status, requested_at,
                   started_at, completed_at, failed_at, killed_at, last_error
            FROM "ob-poc".bpmn_request_states
            WHERE request_key = $1
            "#,
        )
        .bind(request_key)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query bpmn_request_state by request_key")?;

        row.map(Self::row_to_record).transpose()
    }

    fn row_to_record(row: sqlx::postgres::PgRow) -> Result<RequestStateRecord> {
        use sqlx::Row;

        let status = row.get::<String, _>("status");
        Ok(RequestStateRecord {
            request_key: row.get("request_key"),
            correlation_key: row.get("correlation_key"),
            session_id: row.get("session_id"),
            runbook_id: row.get("runbook_id"),
            entry_id: row.get("entry_id"),
            process_key: row.get("process_key"),
            process_instance_id: row.get("process_instance_id"),
            status: RequestStatus::parse(&status).unwrap_or(RequestStatus::Requested),
            requested_at: row.get("requested_at"),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
            failed_at: row.get("failed_at"),
            killed_at: row.get("killed_at"),
            last_error: row.get("last_error"),
        })
    }
}

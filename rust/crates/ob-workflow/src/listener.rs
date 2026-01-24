//! Task Queue Listener
//!
//! Single consumer that drains the task result queue and advances workflows.
//! Uses FOR UPDATE SKIP LOCKED for safe concurrent processing.
//!
//! NOTE: All queries use runtime-checked sqlx::query() instead of compile-time
//! sqlx::query!() macros because the tables are created by migrations that may
//! not exist at compile time.

use std::sync::Arc;
use std::time::Duration;

use sqlx::{FromRow, PgPool, Row};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::cargo_ref::CargoRef;
use crate::task_queue::{TaskResult, TaskResultRow, TaskStatus};
use crate::WorkflowEngine;

/// Maximum retry attempts before moving to DLQ
const MAX_RETRIES: i32 = 3;

/// Polling interval when queue is empty
const POLL_INTERVAL_MS: u64 = 100;

/// Backoff interval after error
const ERROR_BACKOFF_MS: u64 = 1000;

/// Task queue listener that processes results and advances workflows
pub struct TaskQueueListener {
    pool: PgPool,
    engine: Arc<WorkflowEngine>,
}

/// Pending task row from database (runtime query version)
/// Note: Some fields are unused but we fetch all columns for consistency with the table schema.
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
struct PendingTaskRow {
    task_id: Uuid,
    instance_id: Uuid,
    blocker_type: String,
    blocker_key: Option<String>,
    verb: String,
    args: Option<serde_json::Value>,
    expected_cargo_count: i32,
    received_cargo_count: i32,
    failed_count: Option<i32>,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    last_error: Option<String>,
}

impl TaskQueueListener {
    pub fn new(pool: PgPool, engine: Arc<WorkflowEngine>) -> Self {
        Self { pool, engine }
    }

    /// Start the listener loop (blocks until shutdown signal)
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        info!("Task queue listener started");

        loop {
            // Check for shutdown
            if *shutdown.borrow() {
                info!("Task queue listener shutting down");
                break;
            }

            // Try to pop and process a result
            match self.process_one().await {
                Ok(true) => {
                    // Processed a result, immediately check for more
                    continue;
                }
                Ok(false) => {
                    // Queue empty, wait before polling again
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)) => {}
                        _ = shutdown.changed() => {
                            if *shutdown.borrow() {
                                info!("Task queue listener shutting down");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(?e, "Error processing task result");
                    tokio::time::sleep(Duration::from_millis(ERROR_BACKOFF_MS)).await;
                }
            }
        }
    }

    /// Process one result from the queue
    /// Returns Ok(true) if a result was processed, Ok(false) if queue empty
    async fn process_one(&self) -> Result<bool, ListenerError> {
        // Atomic pop with CTE form (safer than subquery, planner-independent)
        // Uses runtime-checked query because tables may not exist at compile time
        let row = sqlx::query(
            r#"
            WITH next AS (
                SELECT id
                FROM "ob-poc".task_result_queue
                WHERE processed_at IS NULL
                ORDER BY id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            UPDATE "ob-poc".task_result_queue q
            SET processed_at = now()
            FROM next
            WHERE q.id = next.id
            RETURNING
                q.id,
                q.task_id,
                q.status,
                q.cargo_type,
                q.cargo_ref,
                q.error,
                q.payload,
                q.retry_count,
                q.queued_at,
                q.idempotency_key
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(false); // Queue empty
        };

        // Extract fields from row
        let id: i64 = row.get("id");
        let task_id: Uuid = row.get("task_id");
        let status_str: String = row.get("status");
        let cargo_type: Option<String> = row.get("cargo_type");
        let cargo_ref: Option<String> = row.get("cargo_ref");
        let error: Option<String> = row.get("error");
        let payload: Option<serde_json::Value> = row.get("payload");
        let retry_count: i32 = row.get("retry_count");
        let queued_at: chrono::DateTime<chrono::Utc> = row.get("queued_at");
        let idempotency_key: String = row.get("idempotency_key");

        // Convert to TaskResultRow
        let status = TaskStatus::try_from(status_str.clone()).unwrap_or(TaskStatus::Failed);

        let task_result_row = TaskResultRow {
            id,
            task_id,
            status,
            cargo_type,
            cargo_ref,
            error,
            payload,
            queued_at,
            retry_count,
            idempotency_key,
        };

        // Parse the row
        let task_result = TaskResult::from(&task_result_row);

        debug!(
            task_id = %task_result.task_id,
            status = ?task_result.status,
            "Processing task result"
        );

        // Handle the result
        match self.handle_task_result(&task_result).await {
            Ok(_) => {
                // Success - delete from queue (events table has permanent record)
                self.delete_queue_row(task_result_row.id).await;
                Ok(true)
            }
            Err(e) if task_result_row.retry_count < MAX_RETRIES => {
                // Requeue with incremented retry
                warn!(
                    task_id = %task_result.task_id,
                    retry_count = task_result_row.retry_count,
                    error = %e,
                    "Retrying task result"
                );
                self.requeue_with_retry(task_result_row.id, &e.to_string())
                    .await;
                Ok(true)
            }
            Err(e) => {
                // Move to DLQ
                error!(
                    task_id = %task_result.task_id,
                    error = %e,
                    "Moving task result to DLQ after {} retries",
                    task_result_row.retry_count
                );
                self.move_to_dlq(&task_result_row, &e.to_string()).await;
                Ok(true)
            }
        }
    }

    /// Handle a single task result
    async fn handle_task_result(&self, result: &TaskResult) -> Result<(), ListenerError> {
        // 1. Find the pending task (source of truth for verb, args, etc.)
        let pending: PendingTaskRow = sqlx::query_as(
            r#"
            SELECT
                task_id,
                instance_id,
                blocker_type,
                blocker_key,
                verb,
                args,
                expected_cargo_count,
                received_cargo_count,
                failed_count,
                status,
                created_at,
                expires_at,
                completed_at,
                last_error
            FROM "ob-poc".workflow_pending_tasks
            WHERE task_id = $1
            "#,
        )
        .bind(result.task_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ListenerError::UnknownTask(result.task_id))?;

        // 2. Record event in permanent history table (audit trail)
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".workflow_task_events
                (task_id, event_type, result_status, cargo_type, cargo_ref, error,
                 payload, source, idempotency_key, occurred_at)
            VALUES ($1, 'result_received', $2, $3, $4, $5, $6, 'webhook', $7, now())
            "#,
        )
        .bind(result.task_id)
        .bind(result.status.as_str())
        .bind(&result.cargo_type)
        .bind(result.cargo_ref.as_ref().map(|c| c.to_uri()))
        .bind(&result.error)
        .bind(&result.payload)
        .bind(&result.idempotency_key)
        .execute(&self.pool)
        .await?;

        // 3. Link version to task if it's a completed document cargo
        if result.status == TaskStatus::Completed {
            if let Some(CargoRef::Version { id, .. }) = &result.cargo_ref {
                let updated = sqlx::query(
                    r#"
                    UPDATE "ob-poc".document_versions SET task_id = $1
                    WHERE version_id = $2 AND task_id IS NULL
                    "#,
                )
                .bind(result.task_id)
                .bind(id)
                .execute(&self.pool)
                .await?;

                if updated.rows_affected() == 0 {
                    warn!(
                        task_id = %result.task_id,
                        version_id = %id,
                        "Version not found or already linked to another task"
                    );
                }

                // Update requirement status if version is linked to a document with a requirement
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".document_requirements dr
                    SET status = 'received',
                        latest_version_id = $2,
                        updated_at = now()
                    FROM "ob-poc".document_versions dv
                    JOIN "ob-poc".documents d ON d.document_id = dv.document_id
                    WHERE dv.version_id = $2
                      AND d.requirement_id = dr.requirement_id
                      AND dr.status IN ('missing', 'requested', 'rejected')
                    "#,
                )
                .bind(result.task_id)
                .bind(id)
                .execute(&self.pool)
                .await?;
            }
        }

        // 4. Update pending task counters
        // IMPORTANT: Only increment received_cargo_count for Completed + cargo_ref
        let (new_received, new_failed): (i32, i32) = match result.status {
            TaskStatus::Completed if result.cargo_ref.is_some() => (1, 0),
            TaskStatus::Failed | TaskStatus::Expired => (0, 1),
            _ => (0, 0), // Completed without cargo - unusual, don't count
        };

        let updated_pending: PendingTaskRow = sqlx::query_as(
            r#"
            UPDATE "ob-poc".workflow_pending_tasks
            SET received_cargo_count = received_cargo_count + $2,
                failed_count = COALESCE(failed_count, 0) + $3,
                last_error = COALESCE($4, last_error)
            WHERE task_id = $1
            RETURNING
                task_id,
                instance_id,
                blocker_type,
                blocker_key,
                verb,
                args,
                expected_cargo_count,
                received_cargo_count,
                failed_count,
                status,
                created_at,
                expires_at,
                completed_at,
                last_error
            "#,
        )
        .bind(result.task_id)
        .bind(new_received)
        .bind(new_failed)
        .bind(&result.error)
        .fetch_one(&self.pool)
        .await?;

        // 5. Determine new status based on updated counts
        let new_status =
            if updated_pending.received_cargo_count >= updated_pending.expected_cargo_count {
                "completed"
            } else if updated_pending.failed_count.unwrap_or(0) > 0
                && updated_pending.received_cargo_count + updated_pending.failed_count.unwrap_or(0)
                    >= updated_pending.expected_cargo_count
            {
                // All expected results received, but some failed
                "failed"
            } else if updated_pending.received_cargo_count > 0 {
                "partial"
            } else {
                "pending"
            };

        sqlx::query(
            r#"
            UPDATE "ob-poc".workflow_pending_tasks
            SET status = $2,
                completed_at = CASE WHEN $2 IN ('completed', 'failed') THEN now() ELSE NULL END
            WHERE task_id = $1
            "#,
        )
        .bind(result.task_id)
        .bind(new_status)
        .execute(&self.pool)
        .await?;

        // 6. Try to advance workflow if task is complete (success or all-failed)
        if new_status == "completed" || new_status == "failed" {
            // Record terminal event
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".workflow_task_events
                    (task_id, event_type, occurred_at)
                VALUES ($1, $2, now())
                "#,
            )
            .bind(result.task_id)
            .bind(new_status)
            .execute(&self.pool)
            .await?;

            // Try to advance the workflow
            if let Err(e) = self.engine.try_advance(pending.instance_id).await {
                warn!(
                    instance_id = %pending.instance_id,
                    error = %e,
                    "Failed to advance workflow after task completion"
                );
                // Don't fail the listener - task was processed successfully
            }
        }

        Ok(())
    }

    /// Delete processed queue row
    async fn delete_queue_row(&self, queue_id: i64) {
        // Use runtime-checked query (tables may not exist at compile time)
        if let Err(e) = sqlx::query(r#"DELETE FROM "ob-poc".task_result_queue WHERE id = $1"#)
            .bind(queue_id)
            .execute(&self.pool)
            .await
        {
            error!(queue_id, error = %e, "Failed to delete queue row");
        }
    }

    /// Requeue with incremented retry count
    async fn requeue_with_retry(&self, id: i64, err_msg: &str) {
        // Use runtime-checked query (tables may not exist at compile time)
        if let Err(e) = sqlx::query(
            r#"
            UPDATE "ob-poc".task_result_queue
            SET processed_at = NULL,
                retry_count = retry_count + 1,
                last_error = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(err_msg)
        .execute(&self.pool)
        .await
        {
            error!(id, error = %e, "Failed to requeue task result");
        }
    }

    /// Move to dead letter queue
    async fn move_to_dlq(&self, row: &TaskResultRow, reason: &str) {
        // Insert into DLQ (use runtime-checked query)
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO "ob-poc".task_result_dlq
                (original_id, task_id, status, cargo_type, cargo_ref, error,
                 payload, retry_count, queued_at, failure_reason)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(row.id)
        .bind(row.task_id)
        .bind(row.status.as_str())
        .bind(&row.cargo_type)
        .bind(&row.cargo_ref)
        .bind(&row.error)
        .bind(&row.payload)
        .bind(row.retry_count)
        .bind(row.queued_at)
        .bind(reason)
        .execute(&self.pool)
        .await
        {
            error!(id = row.id, error = %e, "Failed to insert into DLQ");
            return;
        }

        // Delete from main queue
        self.delete_queue_row(row.id).await;
    }
}

/// Errors that can occur in the listener
#[derive(Debug, thiserror::Error)]
pub enum ListenerError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Unknown task_id: {0}")]
    UnknownTask(Uuid),

    #[error("Workflow error: {0}")]
    Workflow(#[from] crate::WorkflowError),
}

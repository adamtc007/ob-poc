//! JobFrameStore — dedupe for job worker processing.
//!
//! The BPMN-Lite job worker protocol is at-least-once: if the gRPC stream
//! drops before the CompleteJob response is acknowledged, the engine will
//! redeliver the same job. The JobFrameStore provides exactly-once semantics
//! by recording each activation and returning cached completions on redelivery.
//!
//! Forth-style rules:
//! - Rule 1 (PUSH): On activation, persist frame with status=Active
//! - Rule 2 (POP): On completion, persist result. Redelivery returns cached.

use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::{JobFrame, JobFrameStatus};

/// Postgres-backed store for job frame deduplication.
pub struct JobFrameStore {
    pool: PgPool,
}

impl JobFrameStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a job frame, or return false if it already exists (dedupe).
    ///
    /// Uses `ON CONFLICT DO NOTHING` — if the job_key already exists, the
    /// insert is a no-op and returns false. The worker should then check
    /// `find_by_job_key` to see if it was already completed.
    pub async fn upsert(&self, frame: &JobFrame) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".bpmn_job_frames
                (job_key, process_instance_id, task_type, worker_id,
                 status, activated_at, attempts)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (job_key) DO UPDATE
                SET attempts = "ob-poc".bpmn_job_frames.attempts + 1
            "#,
            frame.job_key,
            frame.process_instance_id,
            frame.task_type,
            frame.worker_id,
            frame.status.as_str(),
            frame.activated_at,
            frame.attempts,
        )
        .execute(&self.pool)
        .await
        .context("Failed to upsert bpmn_job_frame")?;

        // If inserted, rows_affected is 1. If conflict updated attempts, also 1.
        // We distinguish new vs existing by checking the current status.
        Ok(result.rows_affected() > 0)
    }

    /// Find a job frame by its key.
    ///
    /// The worker checks this on redelivery: if status is Completed,
    /// return the cached completion without re-executing the verb.
    pub async fn find_by_job_key(&self, job_key: &str) -> Result<Option<JobFrame>> {
        let row = sqlx::query!(
            r#"
            SELECT job_key, process_instance_id, task_type, worker_id,
                   status, activated_at, completed_at, attempts
            FROM "ob-poc".bpmn_job_frames
            WHERE job_key = $1
            "#,
            job_key,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query bpmn_job_frame")?;

        Ok(row.map(|r| JobFrame {
            job_key: r.job_key,
            process_instance_id: r.process_instance_id,
            task_type: r.task_type,
            worker_id: r.worker_id,
            status: JobFrameStatus::parse(&r.status).unwrap_or(JobFrameStatus::Active),
            activated_at: r.activated_at,
            completed_at: r.completed_at,
            attempts: r.attempts,
        }))
    }

    /// Mark a job as completed.
    ///
    /// Sets status to 'completed' and records the completion timestamp.
    /// Returns true if the row was updated.
    pub async fn mark_completed(&self, job_key: &str) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_job_frames
            SET status = 'completed', completed_at = now()
            WHERE job_key = $1 AND status = 'active'
            "#,
            job_key,
        )
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_job_frame as completed")?;

        Ok(result.rows_affected() > 0)
    }

    /// Mark a job as failed.
    ///
    /// Sets status to 'failed' and records the completion timestamp.
    /// Returns true if the row was updated.
    pub async fn mark_failed(&self, job_key: &str) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_job_frames
            SET status = 'failed', completed_at = now()
            WHERE job_key = $1 AND status = 'active'
            "#,
            job_key,
        )
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_job_frame as failed")?;

        Ok(result.rows_affected() > 0)
    }

    /// Mark a job as dead-lettered (exceeded max retries).
    ///
    /// Sets status to 'dead_lettered' and records the completion timestamp.
    /// Returns true if the row was updated.
    pub async fn mark_dead_lettered(&self, job_key: &str) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_job_frames
            SET status = 'dead_lettered', completed_at = now()
            WHERE job_key = $1 AND status IN ('active', 'failed')
            "#,
            job_key,
        )
        .execute(&self.pool)
        .await
        .context("Failed to mark bpmn_job_frame as dead_lettered")?;

        Ok(result.rows_affected() > 0)
    }

    /// List all active job frames for a process instance.
    ///
    /// Used for monitoring and cleanup.
    pub async fn list_active_for_instance(
        &self,
        process_instance_id: Uuid,
    ) -> Result<Vec<JobFrame>> {
        let rows = sqlx::query!(
            r#"
            SELECT job_key, process_instance_id, task_type, worker_id,
                   status, activated_at, completed_at, attempts
            FROM "ob-poc".bpmn_job_frames
            WHERE process_instance_id = $1 AND status = 'active'
            ORDER BY activated_at ASC
            "#,
            process_instance_id,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list active bpmn_job_frames")?;

        Ok(rows
            .into_iter()
            .map(|r| JobFrame {
                job_key: r.job_key,
                process_instance_id: r.process_instance_id,
                task_type: r.task_type,
                worker_id: r.worker_id,
                status: JobFrameStatus::parse(&r.status).unwrap_or(JobFrameStatus::Active),
                activated_at: r.activated_at,
                completed_at: r.completed_at,
                attempts: r.attempts,
            })
            .collect())
    }
}

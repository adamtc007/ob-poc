//! CorrelationStore — links BPMN process instances to ob-poc sessions.
//!
//! Each orchestrated workflow dispatch creates one correlation record that
//! maps the BPMN-Lite process instance back to the originating REPL session,
//! runbook, and entry. This enables bidirectional lookup:
//!
//! - Forward: session entry → process instance (for cancellation, inspection)
//! - Reverse: process instance → session entry (for event bridge signaling)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::{CorrelationRecord, CorrelationStatus};

/// Postgres-backed store for BPMN correlation records.
pub struct CorrelationStore {
    pool: PgPool,
}

impl CorrelationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new correlation record.
    pub async fn insert(&self, record: &CorrelationRecord) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".bpmn_correlations
                (correlation_id, process_instance_id, session_id, runbook_id,
                 entry_id, process_key, domain_payload_hash, status, created_at,
                 domain_correlation_key)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            record.correlation_id,
            record.process_instance_id,
            record.session_id,
            record.runbook_id,
            record.entry_id,
            record.process_key,
            record.domain_payload_hash,
            record.status.as_str(),
            record.created_at,
            record.domain_correlation_key.as_deref(),
        )
        .execute(&self.pool)
        .await
        .context("Failed to insert bpmn_correlation")?;
        Ok(())
    }

    /// Find a correlation by its BPMN process instance ID.
    ///
    /// This is the primary lookup path for the EventBridge: a BPMN lifecycle
    /// event arrives with a process_instance_id, and we need to find which
    /// ob-poc session/entry it belongs to.
    pub async fn find_by_process_instance(
        &self,
        process_instance_id: Uuid,
    ) -> Result<Option<CorrelationRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT correlation_id, process_instance_id, session_id, runbook_id,
                   entry_id, process_key, domain_payload_hash, status,
                   created_at, completed_at, domain_correlation_key
            FROM "ob-poc".bpmn_correlations
            WHERE process_instance_id = $1
            "#,
            process_instance_id,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query bpmn_correlation by process_instance_id")?;

        Ok(row.map(|r| CorrelationRecord {
            correlation_id: r.correlation_id,
            process_instance_id: r.process_instance_id,
            session_id: r.session_id,
            runbook_id: r.runbook_id,
            entry_id: r.entry_id,
            process_key: r.process_key,
            domain_payload_hash: r.domain_payload_hash,
            status: CorrelationStatus::parse(&r.status).unwrap_or(CorrelationStatus::Active),
            created_at: r.created_at,
            completed_at: r.completed_at,
            domain_correlation_key: r.domain_correlation_key,
        }))
    }

    /// Find a correlation by session and entry IDs.
    ///
    /// Used by the REPL to look up the process instance for a parked entry
    /// (e.g., for cancellation or inspection).
    pub async fn find_by_session_entry(
        &self,
        session_id: Uuid,
        entry_id: Uuid,
    ) -> Result<Option<CorrelationRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT correlation_id, process_instance_id, session_id, runbook_id,
                   entry_id, process_key, domain_payload_hash, status,
                   created_at, completed_at, domain_correlation_key
            FROM "ob-poc".bpmn_correlations
            WHERE session_id = $1 AND entry_id = $2
            "#,
            session_id,
            entry_id,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query bpmn_correlation by session_entry")?;

        Ok(row.map(|r| CorrelationRecord {
            correlation_id: r.correlation_id,
            process_instance_id: r.process_instance_id,
            session_id: r.session_id,
            runbook_id: r.runbook_id,
            entry_id: r.entry_id,
            process_key: r.process_key,
            domain_payload_hash: r.domain_payload_hash,
            status: CorrelationStatus::parse(&r.status).unwrap_or(CorrelationStatus::Active),
            created_at: r.created_at,
            completed_at: r.completed_at,
            domain_correlation_key: r.domain_correlation_key,
        }))
    }

    /// Update the status of a correlation record.
    ///
    /// Sets `completed_at` to now when transitioning to a terminal state
    /// (completed, failed, cancelled).
    pub async fn update_status(
        &self,
        correlation_id: Uuid,
        status: CorrelationStatus,
    ) -> Result<bool> {
        let completed_at: Option<DateTime<Utc>> = match status {
            CorrelationStatus::Active => None,
            _ => Some(Utc::now()),
        };

        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_correlations
            SET status = $2, completed_at = $3
            WHERE correlation_id = $1
            "#,
            correlation_id,
            status.as_str(),
            completed_at,
        )
        .execute(&self.pool)
        .await
        .context("Failed to update bpmn_correlation status")?;

        Ok(result.rows_affected() > 0)
    }

    /// List all active correlations (for monitoring and reconnection on startup).
    pub async fn list_active(&self) -> Result<Vec<CorrelationRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT correlation_id, process_instance_id, session_id, runbook_id,
                   entry_id, process_key, domain_payload_hash, status,
                   created_at, completed_at, domain_correlation_key
            FROM "ob-poc".bpmn_correlations
            WHERE status = 'active'
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list active bpmn_correlations")?;

        Ok(rows
            .into_iter()
            .map(|r| CorrelationRecord {
                correlation_id: r.correlation_id,
                process_instance_id: r.process_instance_id,
                session_id: r.session_id,
                runbook_id: r.runbook_id,
                entry_id: r.entry_id,
                process_key: r.process_key,
                domain_payload_hash: r.domain_payload_hash,
                status: CorrelationStatus::parse(&r.status).unwrap_or(CorrelationStatus::Active),
                created_at: r.created_at,
                completed_at: r.completed_at,
                domain_correlation_key: r.domain_correlation_key,
            })
            .collect())
    }

    /// Update the process_instance_id on a correlation record.
    ///
    /// Used by the `PendingDispatchWorker` after a queued dispatch succeeds:
    /// the correlation was initially created with a placeholder
    /// (dispatch_id), and the worker patches it with the real
    /// process_instance_id returned by bpmn-lite's StartProcess.
    pub async fn update_process_instance_id(
        &self,
        correlation_id: Uuid,
        process_instance_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_correlations
            SET process_instance_id = $2
            WHERE correlation_id = $1
            "#,
            correlation_id,
            process_instance_id,
        )
        .execute(&self.pool)
        .await
        .context("Failed to update process_instance_id on bpmn_correlation")?;

        Ok(result.rows_affected() > 0)
    }

    /// Find an active correlation by process_key and domain correlation key.
    ///
    /// Used by lifecycle signal verbs (request.remind, request.cancel, etc.)
    /// to discover if a domain entity (e.g., a KYC case) has an active BPMN
    /// process instance. If found, signals can be routed through the BPMN
    /// engine alongside legacy DB updates.
    pub async fn find_active_by_domain_key(
        &self,
        process_key: &str,
        domain_correlation_key: &str,
    ) -> Result<Option<CorrelationRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT correlation_id, process_instance_id, session_id, runbook_id,
                   entry_id, process_key, domain_payload_hash, status,
                   created_at, completed_at, domain_correlation_key
            FROM "ob-poc".bpmn_correlations
            WHERE process_key = $1
              AND domain_correlation_key = $2
              AND status = 'active'
            LIMIT 1
            "#,
            process_key,
            domain_correlation_key,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query bpmn_correlation by domain_key")?;

        Ok(row.map(|r| CorrelationRecord {
            correlation_id: r.correlation_id,
            process_instance_id: r.process_instance_id,
            session_id: r.session_id,
            runbook_id: r.runbook_id,
            entry_id: r.entry_id,
            process_key: r.process_key,
            domain_payload_hash: r.domain_payload_hash,
            status: CorrelationStatus::parse(&r.status).unwrap_or(CorrelationStatus::Active),
            created_at: r.created_at,
            completed_at: r.completed_at,
            domain_correlation_key: r.domain_correlation_key,
        }))
    }
}

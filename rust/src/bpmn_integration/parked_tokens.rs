//! ParkedTokenStore — ob-poc entries waiting for BPMN signals.
//!
//! When the EventBridge receives a wait event from BPMN-Lite (WaitMsg,
//! WaitTimer, UserTask), it creates a parked token. When the corresponding
//! signal arrives (message received, timer fired, human decision), the
//! token is resolved and the REPL entry is resumed.
//!
//! The correlation_key format is "{runbook_id}:{entry_id}" for O(1) lookup
//! from the REPL's `invocation_index`.

use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::{ParkedToken, ParkedTokenStatus};

/// Postgres-backed store for parked tokens.
pub struct ParkedTokenStore {
    pool: PgPool,
}

impl ParkedTokenStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new parked token.
    pub async fn insert(&self, token: &ParkedToken) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".bpmn_parked_tokens
                (token_id, correlation_key, session_id, entry_id,
                 process_instance_id, expected_signal, status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            token.token_id,
            token.correlation_key,
            token.session_id,
            token.entry_id,
            token.process_instance_id,
            token.expected_signal,
            token.status.as_str(),
            token.created_at,
        )
        .execute(&self.pool)
        .await
        .context("Failed to insert bpmn_parked_token")?;
        Ok(())
    }

    /// Find a parked token by its correlation key.
    ///
    /// Returns only waiting tokens — resolved/cancelled tokens are ignored.
    pub async fn find_by_correlation_key(
        &self,
        correlation_key: &str,
    ) -> Result<Option<ParkedToken>> {
        let row = sqlx::query!(
            r#"
            SELECT token_id, correlation_key, session_id, entry_id,
                   process_instance_id, expected_signal, status,
                   created_at, resolved_at, result_payload
            FROM "ob-poc".bpmn_parked_tokens
            WHERE correlation_key = $1 AND status = 'waiting'
            "#,
            correlation_key,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query bpmn_parked_token by correlation_key")?;

        Ok(row.map(|r| ParkedToken {
            token_id: r.token_id,
            correlation_key: r.correlation_key,
            session_id: r.session_id,
            entry_id: r.entry_id,
            process_instance_id: r.process_instance_id,
            expected_signal: r.expected_signal,
            status: ParkedTokenStatus::parse(&r.status).unwrap_or(ParkedTokenStatus::Waiting),
            created_at: r.created_at,
            resolved_at: r.resolved_at,
            result_payload: r.result_payload,
        }))
    }

    /// Find all parked tokens for a process instance.
    ///
    /// Used by the EventBridge to resolve all tokens when a process completes.
    pub async fn find_by_process_instance(
        &self,
        process_instance_id: Uuid,
    ) -> Result<Vec<ParkedToken>> {
        let rows = sqlx::query!(
            r#"
            SELECT token_id, correlation_key, session_id, entry_id,
                   process_instance_id, expected_signal, status,
                   created_at, resolved_at, result_payload
            FROM "ob-poc".bpmn_parked_tokens
            WHERE process_instance_id = $1
            ORDER BY created_at ASC
            "#,
            process_instance_id,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to query bpmn_parked_tokens by process_instance_id")?;

        Ok(rows
            .into_iter()
            .map(|r| ParkedToken {
                token_id: r.token_id,
                correlation_key: r.correlation_key,
                session_id: r.session_id,
                entry_id: r.entry_id,
                process_instance_id: r.process_instance_id,
                expected_signal: r.expected_signal,
                status: ParkedTokenStatus::parse(&r.status).unwrap_or(ParkedTokenStatus::Waiting),
                created_at: r.created_at,
                resolved_at: r.resolved_at,
                result_payload: r.result_payload,
            })
            .collect())
    }

    /// Resolve a parked token by its correlation key.
    ///
    /// Sets status to 'resolved', records the resolution timestamp,
    /// and optionally stores the result payload from the signal.
    /// Returns true if a token was resolved.
    pub async fn resolve(
        &self,
        correlation_key: &str,
        result_payload: Option<&serde_json::Value>,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_parked_tokens
            SET status = 'resolved',
                resolved_at = now(),
                result_payload = $2
            WHERE correlation_key = $1 AND status = 'waiting'
            "#,
            correlation_key,
            result_payload,
        )
        .execute(&self.pool)
        .await
        .context("Failed to resolve bpmn_parked_token")?;

        Ok(result.rows_affected() > 0)
    }

    /// Resolve all waiting tokens for a process instance.
    ///
    /// Used when a process completes or is cancelled — all outstanding
    /// wait states are resolved in bulk.
    /// Returns the number of tokens resolved.
    pub async fn resolve_all_for_instance(&self, process_instance_id: Uuid) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".bpmn_parked_tokens
            SET status = 'resolved', resolved_at = now()
            WHERE process_instance_id = $1 AND status = 'waiting'
            "#,
            process_instance_id,
        )
        .execute(&self.pool)
        .await
        .context("Failed to resolve all bpmn_parked_tokens for instance")?;

        Ok(result.rows_affected())
    }

    /// List all waiting tokens (for monitoring and startup reconnection).
    pub async fn list_waiting(&self) -> Result<Vec<ParkedToken>> {
        let rows = sqlx::query!(
            r#"
            SELECT token_id, correlation_key, session_id, entry_id,
                   process_instance_id, expected_signal, status,
                   created_at, resolved_at, result_payload
            FROM "ob-poc".bpmn_parked_tokens
            WHERE status = 'waiting'
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list waiting bpmn_parked_tokens")?;

        Ok(rows
            .into_iter()
            .map(|r| ParkedToken {
                token_id: r.token_id,
                correlation_key: r.correlation_key,
                session_id: r.session_id,
                entry_id: r.entry_id,
                process_instance_id: r.process_instance_id,
                expected_signal: r.expected_signal,
                status: ParkedTokenStatus::parse(&r.status).unwrap_or(ParkedTokenStatus::Waiting),
                created_at: r.created_at,
                resolved_at: r.resolved_at,
                result_payload: r.result_payload,
            })
            .collect())
    }

    /// List waiting tokens for a specific session.
    ///
    /// Used by the REPL to show parked entries to the user.
    pub async fn list_waiting_for_session(&self, session_id: Uuid) -> Result<Vec<ParkedToken>> {
        let rows = sqlx::query!(
            r#"
            SELECT token_id, correlation_key, session_id, entry_id,
                   process_instance_id, expected_signal, status,
                   created_at, resolved_at, result_payload
            FROM "ob-poc".bpmn_parked_tokens
            WHERE session_id = $1 AND status = 'waiting'
            ORDER BY created_at ASC
            "#,
            session_id,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list waiting bpmn_parked_tokens for session")?;

        Ok(rows
            .into_iter()
            .map(|r| ParkedToken {
                token_id: r.token_id,
                correlation_key: r.correlation_key,
                session_id: r.session_id,
                entry_id: r.entry_id,
                process_instance_id: r.process_instance_id,
                expected_signal: r.expected_signal,
                status: ParkedTokenStatus::parse(&r.status).unwrap_or(ParkedTokenStatus::Waiting),
                created_at: r.created_at,
                resolved_at: r.resolved_at,
                result_payload: r.result_payload,
            })
            .collect())
    }
}

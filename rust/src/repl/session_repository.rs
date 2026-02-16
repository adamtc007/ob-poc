//! Session Repository V2 — Database persistence for REPL v2 sessions.
//!
//! Sessions are persisted on parking/resumption events (checkpoint-based).
//! Normal in-memory mutations (adding entries, editing) are NOT persisted —
//! only critical state changes (park, resume, complete) trigger writes.

use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

use super::runbook::InvocationRecord;
use super::session_v2::ReplSessionV2;

/// Repository for v2 REPL session persistence.
pub struct SessionRepositoryV2 {
    pool: PgPool,
}

impl SessionRepositoryV2 {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Save or update a session with optimistic concurrency.
    ///
    /// `version` is the last known version — if a concurrent writer has
    /// incremented it, this call will fail (returns 0 rows affected).
    /// Returns the new version on success.
    #[allow(deprecated)] // Reads deprecated fields for DB persistence (migration compat)
    pub async fn save_session(&self, session: &ReplSessionV2, version: i64) -> Result<i64> {
        let state =
            serde_json::to_value(&session.state).context("Failed to serialize session state")?;
        let client_context = session
            .client_context
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .context("Failed to serialize client context")?;
        let journey_context = session
            .journey_context
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .context("Failed to serialize journey context")?;
        let runbook =
            serde_json::to_value(&session.runbook).context("Failed to serialize runbook")?;
        let messages =
            serde_json::to_value(&session.messages).context("Failed to serialize messages")?;

        let new_version = version + 1;

        let rows = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".repl_sessions_v2
                (session_id, state, client_context, journey_context, runbook, messages,
                 created_at, last_active_at, version)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (session_id) DO UPDATE
                SET state = $2,
                    client_context = $3,
                    journey_context = $4,
                    runbook = $5,
                    messages = $6,
                    last_active_at = $8,
                    version = $9
                WHERE "ob-poc".repl_sessions_v2.version = $10
            "#,
            session.id,
            state,
            client_context,
            journey_context,
            runbook,
            messages,
            session.created_at,
            session.last_active_at,
            new_version,
            version,
        )
        .execute(&self.pool)
        .await
        .context("Failed to save session")?
        .rows_affected();

        if rows == 0 {
            anyhow::bail!(
                "Optimistic concurrency conflict: session {} version {} stale",
                session.id,
                version
            );
        }

        Ok(new_version)
    }

    /// Load a session by ID. Returns (session, version) or None.
    #[allow(deprecated)] // Constructs ReplSessionV2 with deprecated fields from DB
    pub async fn load_session(&self, session_id: Uuid) -> Result<Option<(ReplSessionV2, i64)>> {
        let row = sqlx::query!(
            r#"
            SELECT
                session_id,
                state,
                client_context,
                journey_context,
                runbook,
                messages,
                created_at,
                last_active_at,
                version
            FROM "ob-poc".repl_sessions_v2
            WHERE session_id = $1
            "#,
            session_id,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load session")?;

        match row {
            Some(r) => {
                let state = serde_json::from_value(r.state)
                    .context("Failed to deserialize session state")?;
                let client_context = r
                    .client_context
                    .map(serde_json::from_value)
                    .transpose()
                    .context("Failed to deserialize client context")?;
                let journey_context = r
                    .journey_context
                    .map(serde_json::from_value)
                    .transpose()
                    .context("Failed to deserialize journey context")?;
                let runbook =
                    serde_json::from_value(r.runbook).context("Failed to deserialize runbook")?;
                let messages =
                    serde_json::from_value(r.messages).context("Failed to deserialize messages")?;

                let mut session = ReplSessionV2 {
                    id: r.session_id,
                    state,
                    client_context,
                    journey_context,
                    staged_pack: None,
                    staged_pack_hash: None,
                    runbook,
                    messages,
                    pending_arg_audit: None,
                    pending_slot_provenance: None,
                    last_proposal_set: None,
                    decision_log: super::decision_log::SessionDecisionLog::new(r.session_id),
                    created_at: r.created_at,
                    last_active_at: r.last_active_at,
                    // Transient field — the authoritative counter is on `runbook.next_version_counter`
                    // which is persisted in the runbook JSONB. We sync the legacy field below.
                    next_runbook_version: 0, // set below from persisted counter
                };

                // Rebuild transient indexes after deserialization.
                session.runbook.rebuild_invocation_index();

                // Sync the transient legacy field from the persisted counter.
                // If loading an old session that pre-dates this field, serde(default)
                // gives 0 and we fall back to entries.len() as a safe floor.
                if session.runbook.next_version_counter == 0 && !session.runbook.entries.is_empty()
                {
                    session.runbook.next_version_counter = session.runbook.entries.len() as u64;
                }
                session.next_runbook_version = session.runbook.next_version_counter;

                Ok(Some((session, r.version)))
            }
            None => Ok(None),
        }
    }

    /// Delete a session.
    pub async fn delete_session(&self, session_id: Uuid) -> Result<bool> {
        let rows = sqlx::query!(
            r#"DELETE FROM "ob-poc".repl_sessions_v2 WHERE session_id = $1"#,
            session_id,
        )
        .execute(&self.pool)
        .await
        .context("Failed to delete session")?
        .rows_affected();

        Ok(rows > 0)
    }

    /// Save an invocation record (when an entry parks).
    pub async fn save_invocation(&self, record: &InvocationRecord) -> Result<()> {
        let gate_type_str = match record.gate_type {
            super::runbook::GateType::DurableTask => "durable_task",
            super::runbook::GateType::HumanApproval => "human_approval",
        };

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".repl_invocation_records
                (invocation_id, session_id, entry_id, runbook_id, correlation_key,
                 gate_type, task_id, status, parked_at, timeout_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', $8, $9)
            ON CONFLICT (invocation_id) DO NOTHING
            "#,
            record.invocation_id,
            record.session_id,
            record.entry_id,
            record.runbook_id,
            record.correlation_key,
            gate_type_str,
            record.task_id,
            record.parked_at,
            record.timeout_at,
        )
        .execute(&self.pool)
        .await
        .context("Failed to save invocation record")?;

        Ok(())
    }

    /// Find session by correlation key (for signal routing).
    /// Returns (session_id, entry_id, runbook_id) if found with active status.
    pub async fn find_session_by_correlation_key(
        &self,
        key: &str,
    ) -> Result<Option<(Uuid, Uuid, Uuid)>> {
        let row = sqlx::query!(
            r#"
            SELECT session_id, entry_id, runbook_id
            FROM "ob-poc".repl_invocation_records
            WHERE correlation_key = $1 AND status = 'active'
            "#,
            key,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to find session by correlation key")?;

        Ok(row.map(|r| (r.session_id, r.entry_id, r.runbook_id)))
    }

    /// Mark an invocation as completed.
    pub async fn complete_invocation(&self, correlation_key: &str) -> Result<bool> {
        let rows = sqlx::query!(
            r#"
            UPDATE "ob-poc".repl_invocation_records
            SET status = 'completed', resumed_at = now()
            WHERE correlation_key = $1 AND status = 'active'
            "#,
            correlation_key,
        )
        .execute(&self.pool)
        .await
        .context("Failed to complete invocation")?
        .rows_affected();

        Ok(rows > 0)
    }

    /// List session IDs that have active parked invocations.
    pub async fn list_parked_sessions(&self) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT session_id as "session_id!"
            FROM "ob-poc".repl_invocation_records
            WHERE status = 'active'
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list parked sessions")?;

        Ok(rows)
    }
}

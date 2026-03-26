//! Session Repository V2 — Database persistence for REPL v2 sessions.
//!
//! Sessions are persisted on parking/resumption events (checkpoint-based).
//! Normal in-memory mutations (adding entries, editing) are NOT persisted —
//! only critical state changes (park, resume, complete) trigger writes.

use anyhow::{Context, Result};
use sqlx::PgPool;
use sqlx::Row;
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
        let extended_state = serde_json::json!({
            "active_workspace": session.active_workspace,
            "workspace_stack": session.workspace_stack,
            "pending_verb": session.pending_verb,
            "conversation_mode": session.conversation_mode,
            "agent_mode": session.agent_mode,
            "trace_sequence": session.trace_sequence,
            "snapshot_policy": session.snapshot_policy,
            "runbook_plan": session.runbook_plan,
            "runbook_plan_cursor": session.runbook_plan_cursor,
            "execution_log": session.execution_log,
        });

        let new_version = version + 1;

        let rows = sqlx::query(
            r#"
            INSERT INTO "ob-poc".repl_sessions_v2
                (session_id, state, client_context, journey_context, runbook, messages,
                 extended_state, created_at, last_active_at, version)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (session_id) DO UPDATE
                SET state = $2,
                    client_context = $3,
                    journey_context = $4,
                    runbook = $5,
                    messages = $6,
                    extended_state = $7,
                    last_active_at = $9,
                    version = $10
                WHERE "ob-poc".repl_sessions_v2.version = $11
            "#,
        )
        .bind(session.id)
        .bind(state)
        .bind(client_context)
        .bind(journey_context)
        .bind(runbook)
        .bind(messages)
        .bind(extended_state)
        .bind(session.created_at)
        .bind(session.last_active_at)
        .bind(new_version)
        .bind(version)
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

        #[cfg(feature = "database")]
        crate::repl::trace_repository::SessionTraceRepository::append_batch(
            &self.pool,
            &session.trace,
        )
        .await
        .context("Failed to persist session trace")?;

        if let Some(plan) = &session.runbook_plan {
            let status = runbook_plan_status_name(&plan.status);
            let steps = serde_json::to_value(&plan.steps)
                .context("Failed to serialize runbook plan steps")?;
            let bindings = serde_json::to_value(&plan.bindings)
                .context("Failed to serialize runbook plan bindings")?;
            let approval = plan
                .approval
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .context("Failed to serialize runbook plan approval")?;

            sqlx::query(
                r#"
                INSERT INTO "ob-poc".runbook_plans
                    (plan_id, session_id, status, steps, bindings, approval, compiled_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (plan_id) DO UPDATE
                    SET status = EXCLUDED.status,
                        steps = EXCLUDED.steps,
                        bindings = EXCLUDED.bindings,
                        approval = EXCLUDED.approval
                "#,
            )
            .bind(plan.id.0.clone())
            .bind(session.id)
            .bind(status)
            .bind(steps)
            .bind(bindings)
            .bind(approval)
            .bind(plan.compiled_at)
            .execute(&self.pool)
            .await
            .context("Failed to persist runbook plan")?;
        }

        Ok(new_version)
    }

    /// Load a session by ID. Returns (session, version) or None.
    #[allow(deprecated)] // Constructs ReplSessionV2 with deprecated fields from DB
    pub async fn load_session(&self, session_id: Uuid) -> Result<Option<(ReplSessionV2, i64)>> {
        let row = sqlx::query(
            r#"
            SELECT
                session_id,
                state,
                client_context,
                journey_context,
                runbook,
                messages,
                extended_state,
                created_at,
                last_active_at,
                version
            FROM "ob-poc".repl_sessions_v2
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load session")?;

        match row {
            Some(r) => {
                let state = serde_json::from_value(r.try_get("state")?)
                    .context("Failed to deserialize session state")?;
                let client_context = r
                    .try_get::<Option<serde_json::Value>, _>("client_context")?
                    .map(serde_json::from_value)
                    .transpose()
                    .context("Failed to deserialize client context")?;
                let journey_context = r
                    .try_get::<Option<serde_json::Value>, _>("journey_context")?
                    .map(serde_json::from_value)
                    .transpose()
                    .context("Failed to deserialize journey context")?;
                let runbook = serde_json::from_value(r.try_get("runbook")?)
                    .context("Failed to deserialize runbook")?;
                let messages = serde_json::from_value(r.try_get("messages")?)
                    .context("Failed to deserialize messages")?;
                let extended_state = r
                    .try_get::<Option<serde_json::Value>, _>("extended_state")?
                    .unwrap_or_else(|| serde_json::json!({}));

                let mut session = ReplSessionV2 {
                    id: r.try_get("session_id")?,
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
                    decision_log: super::decision_log::SessionDecisionLog::new(
                        r.try_get("session_id")?,
                    ),
                    last_trace_id: None,
                    pending_trace_id: None,
                    pending_sem_os_envelope: None,
                    pending_lookup_result: None,
                    pending_execution_rechecks: Vec::new(),
                    active_workspace: serde_json::from_value(
                        extended_state
                            .get("active_workspace")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    )
                    .context("Failed to deserialize active_workspace")?,
                    workspace_stack: serde_json::from_value(
                        extended_state
                            .get("workspace_stack")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!([])),
                    )
                    .context("Failed to deserialize workspace_stack")?,
                    pending_verb: serde_json::from_value(
                        extended_state
                            .get("pending_verb")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    )
                    .context("Failed to deserialize pending_verb")?,
                    conversation_mode: serde_json::from_value(
                        extended_state
                            .get("conversation_mode")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!("inspect")),
                    )
                    .context("Failed to deserialize conversation_mode")?,
                    agent_mode: serde_json::from_value(
                        extended_state
                            .get("agent_mode")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!("sage")),
                    )
                    .context("Failed to deserialize agent_mode")?,
                    trace: Vec::new(),
                    trace_sequence: serde_json::from_value(
                        extended_state
                            .get("trace_sequence")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!(0)),
                    )
                    .context("Failed to deserialize trace_sequence")?,
                    snapshot_policy: serde_json::from_value(
                        extended_state
                            .get("snapshot_policy")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!("never")),
                    )
                    .context("Failed to deserialize snapshot_policy")?,
                    runbook_plan: serde_json::from_value(
                        extended_state
                            .get("runbook_plan")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    )
                    .context("Failed to deserialize runbook_plan")?,
                    runbook_plan_cursor: serde_json::from_value(
                        extended_state
                            .get("runbook_plan_cursor")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    )
                    .context("Failed to deserialize runbook_plan_cursor")?,
                    execution_log: serde_json::from_value(
                        extended_state
                            .get("execution_log")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!([])),
                    )
                    .context("Failed to deserialize execution_log")?,
                    created_at: r.try_get("created_at")?,
                    last_active_at: r.try_get("last_active_at")?,
                    // Transient field — the authoritative counter is on `runbook.next_version_counter`
                    // which is persisted in the runbook JSONB. We sync the legacy field below.
                    next_runbook_version: 0, // set below from persisted counter
                    tracing_suppressed: false,
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

                #[cfg(feature = "database")]
                {
                    session.trace =
                        crate::repl::trace_repository::SessionTraceRepository::load_trace(
                            &self.pool, session.id,
                        )
                        .await
                        .context("Failed to load session trace")?;
                    if let Some(last) = session.trace.last() {
                        session.trace_sequence = session.trace_sequence.max(last.sequence);
                    }
                }

                Ok(Some((session, r.try_get("version")?)))
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

fn runbook_plan_status_name(
    status: &crate::runbook::plan_types::RunbookPlanStatus,
) -> &'static str {
    match status {
        crate::runbook::plan_types::RunbookPlanStatus::Compiled => "compiled",
        crate::runbook::plan_types::RunbookPlanStatus::AwaitingApproval => "awaiting_approval",
        crate::runbook::plan_types::RunbookPlanStatus::Approved => "approved",
        crate::runbook::plan_types::RunbookPlanStatus::Executing { .. } => "executing",
        crate::runbook::plan_types::RunbookPlanStatus::Completed { .. } => "completed",
        crate::runbook::plan_types::RunbookPlanStatus::Failed { .. } => "failed",
        crate::runbook::plan_types::RunbookPlanStatus::Cancelled => "cancelled",
    }
}

//! Session Repository V2 — Database persistence for REPL v2 sessions.
//!
//! Sessions are persisted through orchestrator checkpoints. A normal REPL/Sage
//! turn checkpoints after `process()`, and critical gates (park, resume,
//! approve, reject, complete) require a successful checkpoint.

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

/// Compatibility handle for older call sites.
///
/// REPL workbook snapshots are append-only, so normal session persistence no
/// longer takes a database advisory lock. `release()` is intentionally a no-op.
pub struct SessionRecordLock {}

impl SessionRecordLock {
    /// Compatibility no-op.
    pub async fn release(self) -> Result<()> {
        Ok(())
    }
}

impl SessionRepositoryV2 {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Compatibility no-op. Session workbook writes are append-only snapshots.
    pub async fn acquire_session_record_lock(&self, session_id: Uuid) -> Result<SessionRecordLock> {
        let _ = session_id;
        Ok(SessionRecordLock {})
    }

    /// Save the current session header and append an immutable workbook snapshot.
    ///
    /// The `version` argument is retained for call-site compatibility; the save
    /// path no longer reads/increments a caller-side version and does not take a
    /// session advisory lock.
    #[allow(deprecated)] // Reads deprecated fields for DB persistence (migration compat)
    pub async fn save_session(&self, session: &ReplSessionV2, version: i64) -> Result<i64> {
        let _ = version; // version used by callers for stale detection; save is an atomic upsert
        self.save_session_inner(session).await
    }

    /// Check the current DB version for a session (for stale-write detection).
    pub async fn current_version(&self, session_id: Uuid) -> Result<Option<i64>> {
        sqlx::query_scalar(
            r#"SELECT version FROM "ob-poc".repl_sessions_v2 WHERE session_id = $1"#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to read session version")
    }

    /// Compatibility wrapper for older held-lock call sites.
    #[allow(deprecated)] // Reads deprecated fields for DB persistence (migration compat)
    pub async fn save_session_with_record_lock(
        &self,
        session: &ReplSessionV2,
        version: i64,
    ) -> Result<i64> {
        self.save_session(session, version).await
    }

    #[allow(deprecated)] // Reads deprecated fields for DB persistence (migration compat)
    async fn save_session_inner(&self, session: &ReplSessionV2) -> Result<i64> {
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
            "session_stack": session.session_stack,
            "bindings": session.bindings,
            "cbu_ids": session.cbu_ids,
            "name": session.name,
            "last_entity_resolution": session.last_entity_resolution,
        });
        let snapshot_id = Uuid::now_v7();
        let workbook_snapshot = serde_json::json!({
            "session_id": session.id,
            "state": state.clone(),
            "client_context": client_context.clone(),
            "journey_context": journey_context.clone(),
            "runbook": runbook.clone(),
            "messages": messages.clone(),
            "extended_state": extended_state.clone(),
            "created_at": session.created_at,
            "last_active_at": session.last_active_at,
        });

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin session save transaction")?;

        let new_version: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".repl_sessions_v2
                (session_id, state, client_context, journey_context, runbook, messages,
                 extended_state, created_at, last_active_at, version, current_snapshot_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, $10)
            ON CONFLICT (session_id) DO UPDATE
                SET state = $2,
                    client_context = $3,
                    journey_context = $4,
                    runbook = $5,
                    messages = $6,
                    extended_state = $7,
                    last_active_at = $9,
                    version = "ob-poc".repl_sessions_v2.version + 1,
                    current_snapshot_id = $10
            RETURNING version
            "#,
        )
        .bind(session.id)
        .bind(&state)
        .bind(&client_context)
        .bind(&journey_context)
        .bind(&runbook)
        .bind(&messages)
        .bind(&extended_state)
        .bind(session.created_at)
        .bind(session.last_active_at)
        .bind(snapshot_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to save session")?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".repl_session_workbook_snapshots
                (session_id, snapshot_id, session_version, state, client_context,
                 journey_context, runbook, messages, extended_state, workbook,
                 created_at, session_created_at, session_last_active_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), $11, $12)
            "#,
        )
        .bind(session.id)
        .bind(snapshot_id)
        .bind(new_version)
        .bind(&state)
        .bind(&client_context)
        .bind(&journey_context)
        .bind(&runbook)
        .bind(&messages)
        .bind(&extended_state)
        .bind(&workbook_snapshot)
        .bind(session.created_at)
        .bind(session.last_active_at)
        .execute(&mut *tx)
        .await
        .context("Failed to append session workbook snapshot")?;

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
            .execute(&mut *tx)
            .await
            .context("Failed to persist runbook plan")?;
        }

        tx.commit()
            .await
            .context("Failed to commit session save transaction")?;

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
                    last_entity_resolution: serde_json::from_value(
                        extended_state
                            .get("last_entity_resolution")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    )
                    .context("Failed to deserialize last_entity_resolution")?,
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
                    session_stack: serde_json::from_value(
                        extended_state
                            .get("session_stack")
                            .cloned()
                            .unwrap_or_else(|| {
                                serde_json::json!({
                                    "session_id": session_id,
                                })
                            }),
                    )
                    .context("Failed to deserialize session_stack")?,
                    bindings: serde_json::from_value(
                        extended_state
                            .get("bindings")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({})),
                    )
                    .context("Failed to deserialize bindings")?,
                    cbu_ids: serde_json::from_value(
                        extended_state
                            .get("cbu_ids")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!([])),
                    )
                    .context("Failed to deserialize cbu_ids")?,
                    name: serde_json::from_value(
                        extended_state
                            .get("name")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    )
                    .context("Failed to deserialize name")?,
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
                    match crate::repl::trace_repository::SessionTraceRepository::load_trace(
                        &self.pool, session.id,
                    )
                    .await
                    {
                        Ok(trace) => {
                            session.trace = trace;
                            if let Some(last) = session.trace.last() {
                                session.trace_sequence = session.trace_sequence.max(last.sequence);
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                session_id = %session.id,
                                error = %error,
                                "Failed to load session trace batch with session checkpoint"
                            );
                        }
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

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./test-migrations/session_repository")]
    async fn test_save_load_preserves_bindings_cbu_ids_name(pool: PgPool) {
        let repo = SessionRepositoryV2::new(pool);
        let mut session = ReplSessionV2::new();
        let cbu_a = Uuid::new_v4();
        let cbu_b = Uuid::new_v4();

        session
            .bindings
            .insert("@acme".into(), serde_json::json!("uuid-1"));
        session
            .bindings
            .insert("@lux".into(), serde_json::json!("uuid-2"));
        session.cbu_ids = vec![cbu_a, cbu_b];
        session.name = Some("Allianz Global Investors".into());

        let version = repo.save_session(&session, 0).await.unwrap();
        assert_eq!(version, 1);

        let (loaded, loaded_version) = repo.load_session(session.id).await.unwrap().unwrap();
        assert_eq!(loaded_version, 1);
        assert_eq!(loaded.bindings, session.bindings);
        assert_eq!(loaded.cbu_ids, session.cbu_ids);
        assert_eq!(loaded.name, session.name);
    }

    #[sqlx::test(migrations = "./test-migrations/session_repository")]
    async fn test_save_session_stack_is_not_aliased(pool: PgPool) {
        let repo = SessionRepositoryV2::new(pool);
        let mut session = ReplSessionV2::new();
        let original_scope_id = Uuid::new_v4();
        let mutated_scope_id = Uuid::new_v4();

        session.session_stack.scope = Some(ob_poc_types::session_stack::SessionScopeState {
            client_group_id: original_scope_id,
            client_group_name: Some("Original".into()),
        });
        session.session_stack.active_workspace =
            Some(ob_poc_types::session_stack::SessionWorkspaceKind::Cbu);
        session.session_stack.trace_sequence = 12;

        let version = repo.save_session(&session, 0).await.unwrap();
        assert_eq!(version, 1);

        session.session_stack.scope = Some(ob_poc_types::session_stack::SessionScopeState {
            client_group_id: mutated_scope_id,
            client_group_name: Some("Mutated".into()),
        });
        session.session_stack.active_workspace =
            Some(ob_poc_types::session_stack::SessionWorkspaceKind::Deal);
        session.session_stack.trace_sequence = 99;

        let (loaded, loaded_version) = repo.load_session(session.id).await.unwrap().unwrap();
        assert_eq!(loaded_version, 1);
        assert_eq!(
            loaded
                .session_stack
                .scope
                .as_ref()
                .map(|scope| scope.client_group_id),
            Some(original_scope_id)
        );
        assert_eq!(
            loaded.session_stack.active_workspace,
            Some(ob_poc_types::session_stack::SessionWorkspaceKind::Cbu)
        );
        assert_eq!(loaded.session_stack.trace_sequence, 12);
    }

    #[sqlx::test(migrations = "./test-migrations/session_repository")]
    async fn test_save_load_preserves_entity_resolution_session_state(pool: PgPool) {
        let repo = SessionRepositoryV2::new(pool);
        let entity_id = Uuid::new_v4();
        let lookup = crate::lookup::LookupResult {
            entity_snapshot: crate::lookup::EntitySnapshotMetadata {
                hash: "snapshot-hash".to_string(),
                version: 1,
                entity_count: 7,
            },
            verbs: Vec::new(),
            entities: vec![crate::entity_linking::EntityResolution {
                mention_span: (0, 7),
                mention_text: "Allianz".to_string(),
                candidates: vec![crate::entity_linking::EntityCandidate {
                    entity_id,
                    entity_kind: "cbu".to_string(),
                    canonical_name: "Allianz Fund".to_string(),
                    score: 0.92,
                    evidence: Vec::new(),
                }],
                selected: Some(entity_id),
                confidence: 0.92,
                evidence: Vec::new(),
            }],
            dominant_entity: Some(crate::lookup::service::DominantEntity {
                entity_id,
                canonical_name: "Allianz Fund".to_string(),
                entity_kind: "cbu".to_string(),
                confidence: 0.92,
                mention_span: (0, 7),
            }),
            expected_kinds: vec!["cbu".to_string()],
            concepts: Vec::new(),
            verb_matched: false,
            entities_resolved: true,
        };

        let mut session = ReplSessionV2::new();
        session.set_client_scope(Uuid::new_v4());
        session.set_workspace_root(crate::repl::types_v2::WorkspaceKind::Cbu);
        assert!(session.apply_lookup_result(&lookup));

        let version = repo.save_session(&session, 0).await.unwrap();
        assert_eq!(version, 1);

        let (loaded, loaded_version) = repo.load_session(session.id).await.unwrap().unwrap();
        assert_eq!(loaded_version, 1);
        assert_eq!(loaded.cbu_ids, vec![entity_id]);
        assert_eq!(
            loaded
                .last_entity_resolution
                .as_ref()
                .and_then(|resolution| resolution.dominant_entity.as_ref())
                .map(|entity| entity.entity_id),
            Some(entity_id)
        );
        assert_eq!(
            loaded
                .bindings
                .get("last_entity_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            Some(entity_id.to_string())
        );
        assert_eq!(
            loaded
                .workspace_stack
                .last()
                .and_then(|frame| frame.subject_id),
            Some(entity_id)
        );
        assert_eq!(
            loaded.session_stack.workspace_stack[0].subject_id,
            Some(entity_id)
        );
    }
}

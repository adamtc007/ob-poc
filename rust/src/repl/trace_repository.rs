//! Persistence layer for session trace entries.
//!
//! Provides `SessionTraceRepository` for batch-appending and querying
//! trace entries from the `session_traces` table.

use anyhow::{Context, Result};
use uuid::Uuid;

use super::session_trace::TraceEntry;

/// Repository for session trace persistence.
pub(crate) struct SessionTraceRepository;

impl SessionTraceRepository {
    /// Append only the trace entries not yet persisted for `session_id`.
    ///
    /// Reads the persisted high-water mark (`MAX(sequence)`) and inserts only
    /// entries above it, so a save path that passes the session's full
    /// in-memory trace on every checkpoint writes each entry exactly once.
    /// `append_batch`'s `ON CONFLICT DO NOTHING` (backed by the table's
    /// `(session_id, sequence)` primary key) remains the backstop against
    /// concurrent checkpoints racing past the high-water read.
    #[cfg(feature = "database")]
    pub(crate) async fn append_new(
        pool: &sqlx::PgPool,
        session_id: Uuid,
        entries: &[TraceEntry],
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let persisted_max: Option<i64> = sqlx::query_scalar(
            r#"SELECT MAX(sequence) FROM "ob-poc".session_traces WHERE session_id = $1"#,
        )
        .bind(session_id)
        .fetch_one(pool)
        .await
        .context("Failed to read persisted trace high-water mark")?;
        let persisted_max = u64::try_from(persisted_max.unwrap_or(0)).unwrap_or(0);

        // Entries are appended in ascending sequence order.
        let start = entries.partition_point(|e| e.sequence <= persisted_max);
        Self::append_batch(pool, &entries[start..]).await
    }

    /// Append a batch of trace entries to the database.
    #[cfg(feature = "database")]
    pub(crate) async fn append_batch(pool: &sqlx::PgPool, entries: &[TraceEntry]) -> Result<()> {
        for entry in entries {
            let op_json = serde_json::to_value(&entry.op)?;
            let stack_json = serde_json::to_value(&entry.stack_snapshot)?;
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".session_traces
                    (session_id, sequence, agent_mode, op, stack_snapshot, hydrated_snap, created_at,
                     verb_resolved, execution_result)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (session_id, sequence) DO NOTHING
                "#,
            )
            .bind(entry.session_id)
            .bind(entry.sequence as i64)
            .bind(match entry.agent_mode {
                super::types_v2::AgentMode::Sage => "sage",
                super::types_v2::AgentMode::Repl => "repl",
            })
            .bind(&op_json)
            .bind(&stack_json)
            .bind(&entry.snapshot)
            .bind(entry.timestamp)
            .bind(&entry.verb_resolved)
            .bind(&entry.execution_result)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    /// Load all trace entries for a session, ordered by sequence.
    #[cfg(feature = "database")]
    pub(crate) async fn load_trace(pool: &sqlx::PgPool, session_id: Uuid) -> Result<Vec<TraceEntry>> {
        let rows = sqlx::query_as::<_, TraceRow>(
            r#"
            SELECT session_id, sequence, agent_mode, op, stack_snapshot, hydrated_snap, created_at,
                   verb_resolved, execution_result
            FROM "ob-poc".session_traces
            WHERE session_id = $1
            ORDER BY sequence ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(TraceRow::into_entry).collect()
    }

    /// Load a single trace entry by session_id and sequence.
    #[cfg(feature = "database")]
    pub(crate) async fn load_entry(
        pool: &sqlx::PgPool,
        session_id: Uuid,
        sequence: u64,
    ) -> Result<Option<TraceEntry>> {
        let row = sqlx::query_as::<_, TraceRow>(
            r#"
            SELECT session_id, sequence, agent_mode, op, stack_snapshot, hydrated_snap, created_at,
                   verb_resolved, execution_result
            FROM "ob-poc".session_traces
            WHERE session_id = $1 AND sequence = $2
            "#,
        )
        .bind(session_id)
        .bind(sequence as i64)
        .fetch_optional(pool)
        .await?;

        row.map(TraceRow::into_entry).transpose()
    }
}

#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct TraceRow {
    session_id: Uuid,
    sequence: i64,
    agent_mode: String,
    op: serde_json::Value,
    stack_snapshot: Option<serde_json::Value>,
    hydrated_snap: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
    verb_resolved: Option<String>,
    execution_result: Option<serde_json::Value>,
}

#[cfg(feature = "database")]
impl TraceRow {
    fn into_entry(self) -> Result<TraceEntry> {
        use super::session_trace::{FrameRef, TraceOp};
        use super::types_v2::AgentMode;

        let agent_mode = match self.agent_mode.as_str() {
            "sage" => AgentMode::Sage,
            "repl" => AgentMode::Repl,
            other => anyhow::bail!("Unknown agent_mode: {other}"),
        };
        let op: TraceOp = serde_json::from_value(self.op)?;
        let stack_snapshot: Vec<FrameRef> = self
            .stack_snapshot
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();

        Ok(TraceEntry {
            session_id: self.session_id,
            sequence: u64::try_from(self.sequence)
                .context("trace sequence must be non-negative")?,
            timestamp: self.created_at,
            agent_mode,
            op,
            stack_snapshot,
            snapshot: self.hydrated_snap,
            session_feedback: None, // Not persisted — reconstructible from session state
            verb_resolved: self.verb_resolved,
            execution_result: self.execution_result,
        })
    }
}

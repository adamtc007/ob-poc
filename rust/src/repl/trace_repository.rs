//! Persistence layer for session trace entries.
//!
//! Provides `SessionTraceRepository` for batch-appending and querying
//! trace entries from the `session_traces` table.

use anyhow::Result;
use uuid::Uuid;

use super::session_trace::TraceEntry;

/// Repository for session trace persistence.
pub struct SessionTraceRepository;

impl SessionTraceRepository {
    /// Append a batch of trace entries to the database.
    #[cfg(feature = "database")]
    pub async fn append_batch(pool: &sqlx::PgPool, entries: &[TraceEntry]) -> Result<()> {
        for entry in entries {
            let op_json = serde_json::to_value(&entry.op)?;
            let stack_json = serde_json::to_value(&entry.stack_snapshot)?;
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".session_traces
                    (session_id, sequence, agent_mode, op, stack_snapshot, hydrated_snap, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (session_id, sequence) DO NOTHING
                "#,
            )
            .bind(entry.session_id)
            .bind(entry.sequence as i64)
            .bind(format!("{:?}", entry.agent_mode).to_lowercase())
            .bind(&op_json)
            .bind(&stack_json)
            .bind(&entry.snapshot)
            .bind(entry.timestamp)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    /// Load all trace entries for a session, ordered by sequence.
    #[cfg(feature = "database")]
    pub async fn load_trace(pool: &sqlx::PgPool, session_id: Uuid) -> Result<Vec<TraceEntry>> {
        let rows = sqlx::query_as::<_, TraceRow>(
            r#"
            SELECT session_id, sequence, agent_mode, op, stack_snapshot, hydrated_snap, created_at
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
    pub async fn load_entry(
        pool: &sqlx::PgPool,
        session_id: Uuid,
        sequence: u64,
    ) -> Result<Option<TraceEntry>> {
        let row = sqlx::query_as::<_, TraceRow>(
            r#"
            SELECT session_id, sequence, agent_mode, op, stack_snapshot, hydrated_snap, created_at
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
            .map(|v| serde_json::from_value(v))
            .transpose()?
            .unwrap_or_default();

        Ok(TraceEntry {
            session_id: self.session_id,
            sequence: self.sequence as u64,
            timestamp: self.created_at,
            agent_mode,
            op,
            stack_snapshot,
            snapshot: self.hydrated_snap,
            // Enrichment fields — not stored in separate DB columns,
            // they round-trip through the `op` JSONB or are populated
            // at query time from the hydrated_snap.
            session_feedback: None,
            verb_resolved: None,
            execution_result: None,
        })
    }
}

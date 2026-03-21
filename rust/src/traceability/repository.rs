//! Database repository for utterance trace persistence.

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::types::{
    NewUtteranceTrace, SurfaceVersions, TraceKind, TraceOutcome, UtteranceTraceRecord,
};

/// Repository for storing first-class utterance traces.
pub struct UtteranceTraceRepository {
    pool: PgPool,
}

impl UtteranceTraceRepository {
    /// Creates a repository from a Postgres pool.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::traceability::UtteranceTraceRepository;
    ///
    /// # async fn demo(pool: sqlx::PgPool) {
    /// let _repo = UtteranceTraceRepository::new(pool);
    /// # }
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Inserts a new utterance trace row.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::traceability::{NewUtteranceTrace, TraceKind, UtteranceTraceRepository};
    /// use uuid::Uuid;
    ///
    /// # async fn demo(repo: UtteranceTraceRepository) -> anyhow::Result<()> {
    /// let trace = NewUtteranceTrace::in_progress(
    ///     Uuid::new_v4(),
    ///     Uuid::new_v4(),
    ///     "show me the fund",
    ///     TraceKind::Original,
    /// );
    /// repo.insert(&trace).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert(&self, trace: &NewUtteranceTrace) -> Result<()> {
        let surface_versions =
            serde_json::to_value(&trace.surface_versions).context("serialize surface_versions")?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".utterance_traces (
                trace_id,
                utterance_id,
                session_id,
                correlation_id,
                trace_kind,
                parent_trace_id,
                timestamp,
                raw_utterance,
                outcome,
                halt_reason_code,
                halt_phase,
                resolved_verb,
                plane,
                polarity,
                execution_shape_kind,
                fallback_invoked,
                fallback_reason_code,
                situation_signature_hash,
                template_id,
                template_version,
                surface_versions,
                trace_payload
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
            )
            "#,
        )
        .bind(trace.trace_id)
        .bind(trace.utterance_id)
        .bind(trace.session_id)
        .bind(trace.correlation_id)
        .bind(trace.trace_kind.as_str())
        .bind(trace.parent_trace_id)
        .bind(trace.timestamp)
        .bind(&trace.raw_utterance)
        .bind(trace.outcome.as_str())
        .bind(&trace.halt_reason_code)
        .bind(trace.halt_phase)
        .bind(&trace.resolved_verb)
        .bind(&trace.plane)
        .bind(&trace.polarity)
        .bind(&trace.execution_shape_kind)
        .bind(trace.fallback_invoked)
        .bind(&trace.fallback_reason_code)
        .bind(trace.situation_signature_hash)
        .bind(&trace.template_id)
        .bind(&trace.template_version)
        .bind(surface_versions)
        .bind(&trace.trace_payload)
        .execute(&self.pool)
        .await
        .context("insert utterance trace")?;

        Ok(())
    }

    /// Updates an existing utterance trace row by `trace_id`.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::traceability::{NewUtteranceTrace, TraceKind, TraceOutcome, UtteranceTraceRepository};
    /// use uuid::Uuid;
    ///
    /// # async fn demo(repo: UtteranceTraceRepository) -> anyhow::Result<()> {
    /// let mut trace = NewUtteranceTrace::in_progress(
    ///     Uuid::new_v4(),
    ///     Uuid::new_v4(),
    ///     "show me the fund",
    ///     TraceKind::Original,
    /// );
    /// repo.insert(&trace).await?;
    /// trace.outcome = TraceOutcome::NoMatch;
    /// repo.update(&trace).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update(&self, trace: &NewUtteranceTrace) -> Result<()> {
        let surface_versions =
            serde_json::to_value(&trace.surface_versions).context("serialize surface_versions")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".utterance_traces
            SET
                utterance_id = $2,
                session_id = $3,
                correlation_id = $4,
                trace_kind = $5,
                parent_trace_id = $6,
                timestamp = $7,
                raw_utterance = $8,
                outcome = $9,
                halt_reason_code = $10,
                halt_phase = $11,
                resolved_verb = $12,
                plane = $13,
                polarity = $14,
                execution_shape_kind = $15,
                fallback_invoked = $16,
                fallback_reason_code = $17,
                situation_signature_hash = $18,
                template_id = $19,
                template_version = $20,
                surface_versions = $21,
                trace_payload = $22
            WHERE trace_id = $1
            "#,
        )
        .bind(trace.trace_id)
        .bind(trace.utterance_id)
        .bind(trace.session_id)
        .bind(trace.correlation_id)
        .bind(trace.trace_kind.as_str())
        .bind(trace.parent_trace_id)
        .bind(trace.timestamp)
        .bind(&trace.raw_utterance)
        .bind(trace.outcome.as_str())
        .bind(&trace.halt_reason_code)
        .bind(trace.halt_phase)
        .bind(&trace.resolved_verb)
        .bind(&trace.plane)
        .bind(&trace.polarity)
        .bind(&trace.execution_shape_kind)
        .bind(trace.fallback_invoked)
        .bind(&trace.fallback_reason_code)
        .bind(trace.situation_signature_hash)
        .bind(&trace.template_id)
        .bind(&trace.template_version)
        .bind(surface_versions)
        .bind(&trace.trace_payload)
        .execute(&self.pool)
        .await
        .context("update utterance trace")?;

        Ok(())
    }

    /// Loads a single utterance trace by ID.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::traceability::UtteranceTraceRepository;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(repo: UtteranceTraceRepository, trace_id: Uuid) -> anyhow::Result<()> {
    /// let _trace = repo.get(trace_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get(&self, trace_id: Uuid) -> Result<Option<UtteranceTraceRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                trace_id,
                utterance_id,
                session_id,
                correlation_id,
                trace_kind,
                parent_trace_id,
                timestamp,
                raw_utterance,
                outcome,
                halt_reason_code,
                halt_phase,
                resolved_verb,
                plane,
                polarity,
                execution_shape_kind,
                fallback_invoked,
                fallback_reason_code,
                situation_signature_hash,
                template_id,
                template_version,
                surface_versions,
                trace_payload
            FROM "ob-poc".utterance_traces
            WHERE trace_id = $1
            "#,
        )
        .bind(trace_id)
        .fetch_optional(&self.pool)
        .await
        .context("load utterance trace")?;

        row.map(decode_trace_row).transpose()
    }

    /// Loads recent utterance traces for a session ordered by timestamp ascending.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::traceability::UtteranceTraceRepository;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(repo: UtteranceTraceRepository, session_id: Uuid) -> anyhow::Result<()> {
    /// let _traces = repo.list_for_session(session_id, 50).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_for_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<UtteranceTraceRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                trace_id,
                utterance_id,
                session_id,
                correlation_id,
                trace_kind,
                parent_trace_id,
                timestamp,
                raw_utterance,
                outcome,
                halt_reason_code,
                halt_phase,
                resolved_verb,
                plane,
                polarity,
                execution_shape_kind,
                fallback_invoked,
                fallback_reason_code,
                situation_signature_hash,
                template_id,
                template_version,
                surface_versions,
                trace_payload
            FROM "ob-poc".utterance_traces
            WHERE session_id = $1
            ORDER BY timestamp ASC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("list utterance traces for session")?;

        rows.into_iter().map(decode_trace_row).collect()
    }
}

fn decode_trace_row(row: sqlx::postgres::PgRow) -> Result<UtteranceTraceRecord> {
    let trace_kind = match row.get::<String, _>("trace_kind").as_str() {
        "original" => TraceKind::Original,
        "clarification_prompt" => TraceKind::ClarificationPrompt,
        "clarification_response" => TraceKind::ClarificationResponse,
        "resumed_execution" => TraceKind::ResumedExecution,
        other => anyhow::bail!("unknown trace_kind '{other}'"),
    };
    let outcome = match row.get::<String, _>("outcome").as_str() {
        "in_progress" => TraceOutcome::InProgress,
        "executed_successfully" => TraceOutcome::ExecutedSuccessfully,
        "executed_with_correction" => TraceOutcome::ExecutedWithCorrection,
        "halted_at_phase" => TraceOutcome::HaltedAtPhase,
        "clarification_triggered" => TraceOutcome::ClarificationTriggered,
        "no_match" => TraceOutcome::NoMatch,
        other => anyhow::bail!("unknown outcome '{other}'"),
    };
    let surface_versions = serde_json::from_value::<SurfaceVersions>(
        row.get::<serde_json::Value, _>("surface_versions"),
    )
    .context("deserialize surface_versions")?;

    Ok(UtteranceTraceRecord {
        trace_id: row.get("trace_id"),
        utterance_id: row.get("utterance_id"),
        session_id: row.get("session_id"),
        correlation_id: row.get("correlation_id"),
        trace_kind,
        parent_trace_id: row.get("parent_trace_id"),
        timestamp: row.get("timestamp"),
        raw_utterance: row.get("raw_utterance"),
        outcome,
        halt_reason_code: row.get("halt_reason_code"),
        halt_phase: row.get("halt_phase"),
        resolved_verb: row.get("resolved_verb"),
        plane: row.get("plane"),
        polarity: row.get("polarity"),
        execution_shape_kind: row.get("execution_shape_kind"),
        fallback_invoked: row.get("fallback_invoked"),
        fallback_reason_code: row.get("fallback_reason_code"),
        situation_signature_hash: row.get("situation_signature_hash"),
        template_id: row.get("template_id"),
        template_version: row.get("template_version"),
        surface_versions,
        trace_payload: row.get("trace_payload"),
    })
}

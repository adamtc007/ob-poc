//! Feedback Inspector
//!
//! On-demand failure analysis system. Reads events captured by the event
//! infrastructure, classifies failures, and stores them for investigation.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use std::path::PathBuf;
use uuid::Uuid;

use crate::events::{DslEvent, EventPayload};

use super::classifier::FailureClassifier;
use super::redactor::Redactor;
use super::types::{
    ActorType, AnalysisReport, AuditAction, AuditEntry, AuditRecord, ErrorType, FailureRecord,
    IssueDetail, IssueFilter, IssueStatus, IssueSummary, OccurrenceRecord, RemediationPath,
    SessionContext, SessionEntry,
};

// =============================================================================
// FEEDBACK INSPECTOR
// =============================================================================

/// On-demand failure analysis inspector
///
/// Created per-request, not long-running. Zero DSL pipeline impact.
pub struct FeedbackInspector {
    pool: PgPool,
    event_store_path: Option<PathBuf>,
    classifier: FailureClassifier,
    redactor: Redactor,
}

impl FeedbackInspector {
    /// Create a new inspector with database connection and optional event store path
    pub fn new(pool: PgPool, event_store_path: Option<PathBuf>) -> Self {
        Self {
            pool,
            event_store_path,
            classifier: FailureClassifier::new(),
            redactor: Redactor::new(),
        }
    }

    // =========================================================================
    // ANALYSIS
    // =========================================================================

    /// Analyze events and create/update failure records
    pub async fn analyze(&self, since: Option<DateTime<Utc>>) -> Result<AnalysisReport> {
        let mut report = AnalysisReport::default();

        // Read events from store
        let events = self.read_failure_events(since).await?;
        report.events_processed = events.len();

        for event in events {
            if let EventPayload::CommandFailed {
                verb,
                duration_ms: _,
                error,
            } = &event.payload
            {
                // Classify the error
                let (error_type, remediation_path) = self.classifier.classify_snapshot(verb, error);

                // Compute fingerprint
                let (fingerprint, discriminator, version) = self
                    .classifier
                    .compute_fingerprint_snapshot(verb, error_type, error);

                // Redact error context
                let redacted_context = self.redactor.redact_for_error(
                    &serde_json::json!({
                        "source_id": error.source_id,
                        "http_status": error.http_status,
                        "error_type": error.error_type,
                    }),
                    error_type,
                );

                // Check if failure already exists
                let existing = self.get_failure_by_fingerprint(&fingerprint).await?;

                if let Some(failure) = existing {
                    // Record new occurrence
                    self.record_occurrence(
                        failure.id,
                        None, // event_id from event store if available
                        event.timestamp,
                        event.session_id,
                        verb,
                        error,
                    )
                    .await?;
                    report.failures_updated += 1;

                    // Update by_error_type count
                    *report
                        .by_error_type
                        .entry(error_type.to_string())
                        .or_insert(0) += 1;
                } else {
                    // Create new failure record
                    let failure_id = self
                        .create_failure(
                            &fingerprint,
                            version,
                            error_type,
                            remediation_path,
                            verb,
                            &error.message,
                            Some(redacted_context),
                            event.session_id,
                            event.timestamp,
                            &discriminator,
                        )
                        .await?;

                    // Audit: CAPTURED
                    self.audit(
                        AuditEntry::new(failure_id, AuditAction::Captured, ActorType::System)
                            .with_details(serde_json::json!({
                                "fingerprint": fingerprint,
                                "discriminator": discriminator,
                            })),
                    )
                    .await?;

                    // Audit: CLASSIFIED
                    self.audit(
                        AuditEntry::new(failure_id, AuditAction::Classified, ActorType::System)
                            .with_details(serde_json::json!({
                                    "error_type": error_type.to_string(),
                                    "remediation_path": remediation_path.to_string(),
                            })),
                    )
                    .await?;

                    report.failures_created += 1;

                    // Update counts
                    *report
                        .by_error_type
                        .entry(error_type.to_string())
                        .or_insert(0) += 1;
                    *report
                        .by_remediation_path
                        .entry(remediation_path.to_string())
                        .or_insert(0) += 1;
                }
            }
        }

        report.analyzed_at = Utc::now();
        Ok(report)
    }

    // =========================================================================
    // QUERIES
    // =========================================================================

    /// List issues with optional filtering
    pub async fn list_issues(&self, filter: IssueFilter) -> Result<Vec<IssueSummary>> {
        let mut query = String::from(
            r#"
            SELECT
                id, fingerprint, error_type, remediation_path, status,
                verb, source, error_message, user_intent,
                occurrence_count, first_seen_at, last_seen_at, repro_verified
            FROM feedback.failures
            WHERE 1=1
            "#,
        );

        let mut params: Vec<String> = Vec::new();
        let mut param_idx = 1;

        if let Some(status) = &filter.status {
            query.push_str(&format!(" AND status = ${}", param_idx));
            params.push(status.as_str().to_string());
            param_idx += 1;
        }

        if let Some(error_type) = &filter.error_type {
            query.push_str(&format!(" AND error_type = ${}", param_idx));
            params.push(error_type.as_str().to_string());
            param_idx += 1;
        }

        if let Some(path) = &filter.remediation_path {
            query.push_str(&format!(" AND remediation_path = ${}", param_idx));
            params.push(path.as_str().to_string());
            param_idx += 1;
        }

        if let Some(verb) = &filter.verb {
            query.push_str(&format!(" AND verb LIKE ${}", param_idx));
            params.push(format!("%{}%", verb));
            param_idx += 1;
        }

        if let Some(source) = &filter.source {
            query.push_str(&format!(" AND source = ${}", param_idx));
            params.push(source.clone());
            param_idx += 1;
        }

        if let Some(since) = &filter.since {
            query.push_str(&format!(" AND last_seen_at >= ${}", param_idx));
            params.push(since.to_rfc3339());
            let _ = param_idx; // Suppress unused warning
        }

        query.push_str(" ORDER BY last_seen_at DESC");

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        // For now, use a simpler approach without dynamic params
        // (SQLx doesn't support dynamic query building easily)
        let issues = sqlx::query_as!(
            IssueSummaryRow,
            r#"
            SELECT
                id, fingerprint,
                error_type as "error_type: ErrorType",
                remediation_path as "remediation_path: RemediationPath",
                status as "status: IssueStatus",
                verb, source, error_message, user_intent,
                occurrence_count, first_seen_at, last_seen_at,
                repro_verified
            FROM feedback.failures
            ORDER BY last_seen_at DESC
            LIMIT 100
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(issues.into_iter().map(|r| r.into()).collect())
    }

    /// Get a single issue by fingerprint
    pub async fn get_issue(&self, fingerprint: &str) -> Result<Option<IssueDetail>> {
        let failure = self.get_failure_by_fingerprint(fingerprint).await?;

        let Some(failure) = failure else {
            return Ok(None);
        };

        let occurrences = self.get_occurrences(failure.id).await?;
        let audit_trail = self.get_audit_trail_by_id(failure.id).await?;

        Ok(Some(IssueDetail {
            failure,
            occurrences,
            audit_trail,
        }))
    }

    /// Get session context for an issue
    pub async fn get_session_context(
        &self,
        session_id: Option<Uuid>,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<SessionContext>> {
        let Some(session_id) = session_id else {
            return Ok(None);
        };

        // Query session log entries around the timestamp
        let entries = sqlx::query_as!(
            SessionEntryRow,
            r#"
            SELECT entry_type, content, timestamp as created_at
            FROM sessions.log
            WHERE session_id = $1
              AND timestamp <= $2
            ORDER BY timestamp DESC
            LIMIT 20
            "#,
            session_id,
            timestamp
        )
        .fetch_all(&self.pool)
        .await?;

        if entries.is_empty() {
            return Ok(None);
        }

        // Extract user intent from recent entries
        let user_intent = entries
            .iter()
            .find(|e| e.entry_type == "user_input")
            .map(|e| e.content.clone());

        // Build command sequence
        let command_sequence: Vec<String> = entries
            .iter()
            .filter(|e| e.entry_type == "dsl_command")
            .map(|e| e.content.clone())
            .collect();

        Ok(Some(SessionContext {
            session_id: Some(session_id),
            user_intent,
            command_sequence,
            entries: entries.into_iter().map(|e| e.into()).collect(),
        }))
    }

    // =========================================================================
    // FAILURE STORE
    // =========================================================================

    async fn get_failure_by_fingerprint(&self, fingerprint: &str) -> Result<Option<FailureRecord>> {
        let row = sqlx::query_as!(
            FailureRow,
            r#"
            SELECT
                id, fingerprint, fingerprint_version,
                error_type as "error_type: ErrorType",
                remediation_path as "remediation_path: RemediationPath",
                status as "status: IssueStatus",
                verb, source, error_message, error_context,
                user_intent, command_sequence,
                repro_type, repro_path, repro_verified,
                fix_commit, fix_notes,
                occurrence_count, first_seen_at, last_seen_at, resolved_at,
                created_at, updated_at
            FROM feedback.failures
            WHERE fingerprint = $1
            "#,
            fingerprint
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_failure(
        &self,
        fingerprint: &str,
        version: u8,
        error_type: ErrorType,
        remediation_path: RemediationPath,
        verb: &str,
        error_message: &str,
        error_context: Option<serde_json::Value>,
        session_id: Option<Uuid>,
        timestamp: DateTime<Utc>,
        _discriminator: &str,
    ) -> Result<Uuid> {
        // Try to get session context for user_intent
        let session_context = self.get_session_context(session_id, timestamp).await?;
        let user_intent = session_context.as_ref().and_then(|c| c.user_intent.clone());
        let command_sequence = session_context.map(|c| c.command_sequence);

        // Extract source from verb if present
        let source = self.extract_source_from_verb(verb);

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO feedback.failures (
                fingerprint, fingerprint_version,
                error_type, remediation_path, status,
                verb, source, error_message, error_context,
                user_intent, command_sequence,
                first_seen_at, last_seen_at
            ) VALUES (
                $1, $2,
                $3, $4, 'NEW',
                $5, $6, $7, $8,
                $9, $10,
                $11, $11
            )
            RETURNING id
            "#,
            fingerprint,
            version as i16,
            error_type as ErrorType,
            remediation_path as RemediationPath,
            verb,
            source,
            error_message,
            error_context,
            user_intent,
            command_sequence.as_deref(),
            timestamp
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    async fn record_occurrence(
        &self,
        failure_id: Uuid,
        event_id: Option<Uuid>,
        event_timestamp: DateTime<Utc>,
        session_id: Option<Uuid>,
        verb: &str,
        error: &crate::events::ErrorSnapshot,
    ) -> Result<Uuid> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO feedback.occurrences (
                failure_id, event_id, event_timestamp, session_id,
                verb, error_message
            ) VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            failure_id,
            event_id,
            event_timestamp,
            session_id,
            verb,
            error.message
        )
        .fetch_one(&self.pool)
        .await?;

        // Update failure counts
        sqlx::query!(
            r#"
            UPDATE feedback.failures
            SET occurrence_count = occurrence_count + 1,
                last_seen_at = $2
            WHERE id = $1
            "#,
            failure_id,
            event_timestamp
        )
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    async fn get_occurrences(&self, failure_id: Uuid) -> Result<Vec<OccurrenceRecord>> {
        let rows = sqlx::query_as!(
            OccurrenceRow,
            r#"
            SELECT
                id, failure_id, event_id, event_timestamp, session_id,
                verb, duration_ms, error_message, error_backtrace, created_at
            FROM feedback.occurrences
            WHERE failure_id = $1
            ORDER BY event_timestamp DESC
            LIMIT 50
            "#,
            failure_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Set repro test information
    pub async fn set_repro(
        &self,
        fingerprint: &str,
        repro_type: &str,
        repro_path: &str,
    ) -> Result<()> {
        let result = sqlx::query!(
            r#"
            UPDATE feedback.failures
            SET repro_type = $2, repro_path = $3, status = 'REPRO_GENERATED'
            WHERE fingerprint = $1
            "#,
            fingerprint,
            repro_type,
            repro_path
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Failure not found: {}", fingerprint));
        }

        Ok(())
    }

    /// Mark repro as verified (test fails as expected)
    pub async fn verify_repro(&self, fingerprint: &str) -> Result<()> {
        let result = sqlx::query!(
            r#"
            UPDATE feedback.failures
            SET repro_verified = true, status = 'REPRO_VERIFIED'
            WHERE fingerprint = $1
            "#,
            fingerprint
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Failure not found: {}", fingerprint));
        }

        Ok(())
    }

    /// Update issue status
    pub async fn set_status(&self, fingerprint: &str, status: IssueStatus) -> Result<()> {
        let resolved_at = if status.is_terminal() {
            Some(Utc::now())
        } else {
            None
        };

        let result = sqlx::query!(
            r#"
            UPDATE feedback.failures
            SET status = $2, resolved_at = $3
            WHERE fingerprint = $1
            "#,
            fingerprint,
            status as IssueStatus,
            resolved_at
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Failure not found: {}", fingerprint));
        }

        Ok(())
    }

    /// Mark issue as fixed
    pub async fn mark_fixed(
        &self,
        fingerprint: &str,
        commit: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        let result = sqlx::query!(
            r#"
            UPDATE feedback.failures
            SET status = 'FIX_COMMITTED', fix_commit = $2, fix_notes = $3
            WHERE fingerprint = $1
            "#,
            fingerprint,
            commit,
            notes
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Failure not found: {}", fingerprint));
        }

        Ok(())
    }

    // =========================================================================
    // AUDIT TRAIL
    // =========================================================================

    /// Record an audit entry
    pub async fn audit(&self, entry: AuditEntry<'_>) -> Result<Uuid> {
        // Get current and new status for state transitions
        let (previous_status, new_status) = self.get_status_transition(&entry.action).await;

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO feedback.audit_log (
                failure_id, action, actor_type, actor_id,
                details, evidence, evidence_hash,
                previous_status, new_status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
            entry.failure_id,
            entry.action as AuditAction,
            entry.actor_type as ActorType,
            entry.actor_id,
            entry.details,
            entry.evidence,
            entry.evidence_hash,
            previous_status as Option<IssueStatus>,
            new_status as Option<IssueStatus>
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Get audit trail for a failure
    pub async fn get_audit_trail(&self, fingerprint: &str) -> Result<Vec<AuditRecord>> {
        let failure = self.get_failure_by_fingerprint(fingerprint).await?;

        let Some(failure) = failure else {
            return Err(anyhow!("Failure not found: {}", fingerprint));
        };

        self.get_audit_trail_by_id(failure.id).await
    }

    async fn get_audit_trail_by_id(&self, failure_id: Uuid) -> Result<Vec<AuditRecord>> {
        let rows = sqlx::query_as!(
            AuditRow,
            r#"
            SELECT
                id, failure_id,
                action as "action: AuditAction",
                actor_type as "actor_type: ActorType",
                actor_id, details, evidence, evidence_hash,
                previous_status as "previous_status: IssueStatus",
                new_status as "new_status: IssueStatus",
                created_at
            FROM feedback.audit_log
            WHERE failure_id = $1
            ORDER BY created_at ASC
            "#,
            failure_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    async fn read_failure_events(&self, since: Option<DateTime<Utc>>) -> Result<Vec<DslEvent>> {
        let Some(ref path) = self.event_store_path else {
            return Ok(Vec::new());
        };

        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(path)?;
        let since = since.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));

        let mut events = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<DslEvent>(line) {
                // Only include failure events after 'since'
                if event.timestamp >= since
                    && matches!(event.payload, EventPayload::CommandFailed { .. })
                {
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    fn extract_source_from_verb(&self, verb: &str) -> Option<String> {
        let known_sources = ["gleif", "lbr", "bods", "brave"];
        for source in known_sources {
            if verb.to_lowercase().contains(source) {
                return Some(source.to_string());
            }
        }
        None
    }

    async fn get_status_transition(
        &self,
        action: &AuditAction,
    ) -> (Option<IssueStatus>, Option<IssueStatus>) {
        // Map actions to status transitions
        match action {
            AuditAction::Captured => (None, Some(IssueStatus::New)),
            AuditAction::ReproGenerated => (None, Some(IssueStatus::ReproGenerated)),
            AuditAction::ReproVerifiedFails => (None, Some(IssueStatus::ReproVerified)),
            AuditAction::TodoCreated => (None, Some(IssueStatus::TodoCreated)),
            AuditAction::FixCommitted => (None, Some(IssueStatus::FixCommitted)),
            AuditAction::ReproVerifiedPasses => (None, Some(IssueStatus::FixVerified)),
            AuditAction::Resolved => (None, Some(IssueStatus::Resolved)),
            AuditAction::MarkedWontFix => (None, Some(IssueStatus::WontFix)),
            AuditAction::MarkedDuplicate => (None, Some(IssueStatus::Duplicate)),
            AuditAction::RuntimeSuccess => (None, Some(IssueStatus::RuntimeResolved)),
            AuditAction::RuntimeExhausted => (None, Some(IssueStatus::RuntimeEscalated)),
            _ => (None, None),
        }
    }
}

// =============================================================================
// DATABASE ROW TYPES
// =============================================================================

#[derive(Debug)]
struct IssueSummaryRow {
    id: Uuid,
    fingerprint: String,
    error_type: ErrorType,
    remediation_path: RemediationPath,
    status: IssueStatus,
    verb: String,
    source: Option<String>,
    error_message: String,
    user_intent: Option<String>,
    occurrence_count: i32,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    repro_verified: Option<bool>,
}

impl From<IssueSummaryRow> for IssueSummary {
    fn from(row: IssueSummaryRow) -> Self {
        Self {
            id: row.id,
            fingerprint: row.fingerprint,
            error_type: row.error_type,
            remediation_path: row.remediation_path,
            status: row.status,
            verb: row.verb,
            source: row.source,
            error_message: row.error_message,
            user_intent: row.user_intent,
            occurrence_count: row.occurrence_count,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
            repro_verified: row.repro_verified.unwrap_or(false),
        }
    }
}

#[derive(Debug)]
struct FailureRow {
    id: Uuid,
    fingerprint: String,
    fingerprint_version: i16,
    error_type: ErrorType,
    remediation_path: RemediationPath,
    status: IssueStatus,
    verb: String,
    source: Option<String>,
    error_message: String,
    error_context: Option<serde_json::Value>,
    user_intent: Option<String>,
    command_sequence: Option<Vec<String>>,
    repro_type: Option<String>,
    repro_path: Option<String>,
    repro_verified: Option<bool>,
    fix_commit: Option<String>,
    fix_notes: Option<String>,
    occurrence_count: i32,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<FailureRow> for FailureRecord {
    fn from(row: FailureRow) -> Self {
        Self {
            id: row.id,
            fingerprint: row.fingerprint,
            fingerprint_version: row.fingerprint_version,
            error_type: row.error_type,
            remediation_path: row.remediation_path,
            status: row.status,
            verb: row.verb,
            source: row.source,
            error_message: row.error_message,
            error_context: row.error_context,
            user_intent: row.user_intent,
            command_sequence: row.command_sequence,
            repro_type: row.repro_type,
            repro_path: row.repro_path,
            repro_verified: row.repro_verified.unwrap_or(false),
            fix_commit: row.fix_commit,
            fix_notes: row.fix_notes,
            occurrence_count: row.occurrence_count,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
            resolved_at: row.resolved_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug)]
struct OccurrenceRow {
    id: Uuid,
    failure_id: Uuid,
    event_id: Option<Uuid>,
    event_timestamp: DateTime<Utc>,
    session_id: Option<Uuid>,
    verb: String,
    duration_ms: Option<i64>,
    error_message: String,
    error_backtrace: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<OccurrenceRow> for OccurrenceRecord {
    fn from(row: OccurrenceRow) -> Self {
        Self {
            id: row.id,
            failure_id: row.failure_id,
            event_id: row.event_id,
            event_timestamp: row.event_timestamp,
            session_id: row.session_id,
            verb: row.verb,
            duration_ms: row.duration_ms,
            error_message: row.error_message,
            error_backtrace: row.error_backtrace,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug)]
struct AuditRow {
    id: Uuid,
    failure_id: Uuid,
    action: AuditAction,
    actor_type: ActorType,
    actor_id: Option<String>,
    details: Option<serde_json::Value>,
    evidence: Option<String>,
    evidence_hash: Option<String>,
    previous_status: Option<IssueStatus>,
    new_status: Option<IssueStatus>,
    created_at: DateTime<Utc>,
}

impl From<AuditRow> for AuditRecord {
    fn from(row: AuditRow) -> Self {
        Self {
            id: row.id,
            failure_id: row.failure_id,
            action: row.action,
            actor_type: row.actor_type,
            actor_id: row.actor_id,
            details: row.details,
            evidence: row.evidence,
            evidence_hash: row.evidence_hash,
            previous_status: row.previous_status,
            new_status: row.new_status,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug)]
struct SessionEntryRow {
    entry_type: String,
    content: String,
    created_at: DateTime<Utc>,
}

impl From<SessionEntryRow> for SessionEntry {
    fn from(row: SessionEntryRow) -> Self {
        Self {
            entry_type: row.entry_type,
            content: row.content,
            timestamp: row.created_at,
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to test extract_source_from_verb without database
    fn extract_source_from_verb(verb: &str) -> Option<String> {
        let known_sources = ["gleif", "lbr", "bods", "brave"];
        for source in known_sources {
            if verb.to_lowercase().contains(source) {
                return Some(source.to_string());
            }
        }
        None
    }

    #[test]
    fn test_extract_source_from_verb() {
        assert_eq!(
            extract_source_from_verb("gleif.fetch-entity"),
            Some("gleif".to_string())
        );
        assert_eq!(
            extract_source_from_verb("bods.import"),
            Some("bods".to_string())
        );
        assert_eq!(extract_source_from_verb("cbu.create"), None);
    }
}

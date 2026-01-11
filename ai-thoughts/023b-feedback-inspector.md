# 023b: Feedback Inspector

> **Status:** ✅ IMPLEMENTED
> **Implemented:** 2026-01-11 (see 026)
> **Part of:** Adaptive Feedback System (023a + 023b)
> **Depends on:** 023a Event Infrastructure

---

## Executive Summary

On-demand analysis of DSL execution failures. Reads from event store, joins session context, classifies, generates repro tests, creates TODOs.

**Key principle:** Spins up when needed, not always running. Event infrastructure (023a) captures; this analyzes.

---

## Part 1: Architecture

### 1.1 On-Demand Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  TRIGGER                            ACTION                                   │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                              │
│  `:feedback` in REPL                Spin up inspector, show failures        │
│  MCP tool call                      Spin up inspector, return results       │
│  Cron job (optional)                Periodic analysis, alerts               │
│  egui "Failures" panel              Inspector runs in background            │
│                                                                              │
│  Inspector lifetime: request-scoped (created, used, dropped)                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Data Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────────┐     │
│  │ Event Store  │     │ Session Log  │     │ Feedback DB              │     │
│  │ (023a)       │     │ (023a)       │     │ (this module)            │     │
│  │              │     │              │     │                          │     │
│  │ events.jsonl │     │ sessions.log │     │ feedback.failures        │     │
│  │ or events.log│     │              │     │ feedback.audit_log       │     │
│  └──────┬───────┘     └──────┬───────┘     └────────────┬─────────────┘     │
│         │                    │                          │                   │
│         │                    │                          │                   │
│         ▼                    ▼                          ▼                   │
│  ┌──────────────────────────────────────────────────────────────────┐       │
│  │                     FeedbackInspector                             │       │
│  │                                                                   │       │
│  │   1. Read failure events from event store                        │       │
│  │   2. Join session logs (what was user doing?)                    │       │
│  │   3. Classify (error type, remediation path)                     │       │
│  │   4. Fingerprint (dedupe)                                        │       │
│  │   5. Persist to feedback.failures (if not already there)         │       │
│  │   6. Generate repro tests                                        │       │
│  │   7. Create audit trail                                          │       │
│  │   8. Generate TODOs                                              │       │
│  │                                                                   │       │
│  └──────────────────────────────────────────────────────────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 2: Database Schema

### 2.1 Feedback Tables

```sql
-- migrations/023b_feedback.sql

CREATE SCHEMA IF NOT EXISTS feedback;

-- ============================================================================
-- ENUMS
-- ============================================================================

CREATE TYPE feedback.error_type AS ENUM (
    -- Transient (runtime path)
    'TIMEOUT',
    'RATE_LIMITED', 
    'CONNECTION_RESET',
    'SERVICE_UNAVAILABLE',
    'POOL_EXHAUSTED',
    
    -- Structural - PROVABLE DRIFT
    'ENUM_DRIFT',
    'SCHEMA_DRIFT',
    
    -- Structural - MIGHT BE OUR BUG
    'PARSE_ERROR',
    'HANDLER_PANIC',
    'HANDLER_ERROR',
    'DSL_PARSE_ERROR',
    'API_ENDPOINT_MOVED',
    'API_AUTH_CHANGED',
    'VALIDATION_FAILED',
    
    'UNKNOWN'
);

CREATE TYPE feedback.remediation_path AS ENUM (
    'RUNTIME',
    'CODE',
    'LOG_ONLY'
);

CREATE TYPE feedback.issue_status AS ENUM (
    'NEW',
    'RUNTIME_RESOLVED',
    'RUNTIME_ESCALATED',
    'REPRO_GENERATED',
    'REPRO_VERIFIED',
    'TODO_CREATED',
    'IN_PROGRESS',
    'FIX_COMMITTED',
    'FIX_VERIFIED',
    'DEPLOYED_STAGING',
    'DEPLOYED_PROD',
    'RESOLVED',
    'WONT_FIX',
    'DUPLICATE',
    'INVALID'
);

CREATE TYPE feedback.actor_type AS ENUM (
    'SYSTEM',
    'MCP_AGENT',
    'REPL_USER',
    'EGUI_USER',
    'CI_PIPELINE',
    'CLAUDE_CODE',
    'CRON_JOB'
);

CREATE TYPE feedback.audit_action AS ENUM (
    'CAPTURED',
    'CLASSIFIED',
    'DEDUPLICATED',
    'RUNTIME_ATTEMPT',
    'RUNTIME_SUCCESS',
    'RUNTIME_EXHAUSTED',
    'REPRO_GENERATED',
    'REPRO_VERIFIED_FAILS',
    'REPRO_VERIFICATION_FAILED',
    'TODO_CREATED',
    'TODO_ASSIGNED',
    'FIX_COMMITTED',
    'REPRO_VERIFIED_PASSES',
    'DEPLOYED',
    'SEMANTIC_REPLAY_PASSED',
    'SEMANTIC_REPLAY_FAILED',
    'RESOLVED',
    'MARKED_WONT_FIX',
    'MARKED_DUPLICATE',
    'REOPENED',
    'COMMENT_ADDED'
);

-- ============================================================================
-- FAILURES TABLE (deduplicated issues)
-- ============================================================================

CREATE TABLE feedback.failures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Identity
    fingerprint TEXT UNIQUE NOT NULL,
    fingerprint_key TEXT NOT NULL,
    fingerprint_version SMALLINT NOT NULL DEFAULT 1,
    
    -- Timing
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    occurrence_count INT NOT NULL DEFAULT 1,
    
    -- Error classification
    error_type feedback.error_type NOT NULL,
    path feedback.remediation_path NOT NULL,
    
    -- Error details
    verb TEXT NOT NULL,
    source_id TEXT,
    error_message TEXT NOT NULL,
    error_details JSONB,
    
    -- Sample data (from first occurrence, redacted)
    sample_raw_response JSONB,
    sample_http_status INT,
    sample_dsl_text TEXT,
    sample_args JSONB,
    
    -- Code context
    handler TEXT,
    file TEXT,
    line INT,
    
    -- Session context (from first occurrence)
    first_session_id UUID,
    user_context JSONB,  -- What user was doing (from session log)
    
    -- Classification
    suggested_action TEXT,
    
    -- Repro
    repro_type TEXT,
    repro_path TEXT,
    repro_pre_fix_verified BOOLEAN,
    repro_pre_fix_verified_at TIMESTAMPTZ,
    repro_post_fix_verified BOOLEAN,
    repro_post_fix_verified_at TIMESTAMPTZ,
    
    -- Workflow
    status feedback.issue_status NOT NULL DEFAULT 'NEW',
    todo_number INT,
    todo_path TEXT,
    fixed_in_commit TEXT,
    resolution_notes TEXT,
    
    -- Resolution tracking
    last_occurrence_at TIMESTAMPTZ,
    occurrence_count_since_fix INT DEFAULT 0,
    resolved_at TIMESTAMPTZ,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_failures_status ON feedback.failures(status, path);
CREATE INDEX idx_failures_fingerprint ON feedback.failures(fingerprint);
CREATE INDEX idx_failures_verb ON feedback.failures(verb);
CREATE INDEX idx_failures_error_type ON feedback.failures(error_type);
CREATE INDEX idx_failures_last_seen ON feedback.failures(last_seen_at DESC);

-- ============================================================================
-- OCCURRENCES TABLE (each time a fingerprint is seen)
-- ============================================================================

CREATE TABLE feedback.occurrences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    failure_id UUID NOT NULL REFERENCES feedback.failures(id),
    
    -- When
    occurred_at TIMESTAMPTZ NOT NULL,
    
    -- Context
    session_id UUID,
    http_status INT,
    duration_ms INT,
    
    -- Link to event store
    event_id BIGINT,
    
    -- Runtime handling
    runtime_handled BOOLEAN DEFAULT FALSE,
    runtime_action TEXT,
    runtime_attempts INT DEFAULT 0
);

CREATE INDEX idx_occurrences_failure ON feedback.occurrences(failure_id, occurred_at DESC);

-- ============================================================================
-- AUDIT LOG (full history)
-- ============================================================================

CREATE TABLE feedback.audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    failure_id UUID NOT NULL REFERENCES feedback.failures(id),
    
    -- What
    action feedback.audit_action NOT NULL,
    
    -- Who
    actor_type feedback.actor_type NOT NULL,
    actor_id TEXT,
    
    -- When
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Context
    details JSONB NOT NULL DEFAULT '{}',
    
    -- Evidence (for verification actions)
    evidence TEXT,
    evidence_hash TEXT
);

CREATE INDEX idx_audit_failure ON feedback.audit_log(failure_id, timestamp);
CREATE INDEX idx_audit_action ON feedback.audit_log(action, timestamp);
```

---

## Part 3: Core Implementation

### 3.1 Feedback Inspector

```rust
// rust/src/feedback/inspector.rs

use std::path::PathBuf;
use sqlx::PgPool;
use chrono::{DateTime, Utc, Duration};

/// On-demand feedback inspector
/// 
/// Created when needed, analyzes events, dropped when done
pub struct FeedbackInspector {
    pool: PgPool,
    event_store_path: PathBuf,
    classifier: FailureClassifier,
    redactor: Redactor,
}

impl FeedbackInspector {
    pub fn new(pool: PgPool, event_store_path: PathBuf) -> Self {
        Self {
            pool,
            event_store_path,
            classifier: FailureClassifier::new(),
            redactor: Redactor::default(),
        }
    }
    
    /// Analyze failures since timestamp (or last analysis)
    pub async fn analyze(&self, since: Option<DateTime<Utc>>) -> Result<AnalysisReport> {
        let since = since.unwrap_or_else(|| Utc::now() - Duration::hours(24));
        
        // 1. Read failure events from store
        let events = self.read_failure_events(since).await?;
        
        // 2. Process each event
        let mut new_issues = 0;
        let mut updated_issues = 0;
        
        for event in events {
            let result = self.process_event(&event).await?;
            match result {
                ProcessResult::NewIssue(_) => new_issues += 1,
                ProcessResult::ExistingIssue(_) => updated_issues += 1,
                ProcessResult::Skipped => {}
            }
        }
        
        // 3. Get current issue summary
        let issues = self.list_issues(IssueFilter::default()).await?;
        
        Ok(AnalysisReport {
            analyzed_since: since,
            events_processed: events.len(),
            new_issues,
            updated_issues,
            current_issues: issues,
        })
    }
    
    /// Process a single event
    async fn process_event(&self, event: &DslEvent) -> Result<ProcessResult> {
        let payload = match &event.payload {
            EventPayload::CommandFailed { error, verb, .. } => (verb, error),
            _ => return Ok(ProcessResult::Skipped),
        };
        
        let (verb, error) = payload;
        
        // Classify
        let (error_type, path) = self.classifier.classify_snapshot(verb, error);
        
        if path == RemediationPath::Skip {
            return Ok(ProcessResult::Skipped);
        }
        
        // Compute fingerprint
        let (key, hash, version) = self.classifier.compute_fingerprint_snapshot(verb, error_type, error);
        
        // Check if exists
        let existing = sqlx::query_scalar!(
            "SELECT id FROM feedback.failures WHERE fingerprint = $1",
            hash
        )
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(failure_id) = existing {
            // Update occurrence count
            self.record_occurrence(failure_id, event).await?;
            return Ok(ProcessResult::ExistingIssue(failure_id));
        }
        
        // New issue - enrich with session context
        let session_context = self.get_session_context(event.session_id, event.timestamp).await?;
        
        // Create failure record
        let failure_id = self.create_failure(
            &hash, &key, version,
            error_type, path,
            verb, error,
            event,
            session_context,
        ).await?;
        
        // Record occurrence
        self.record_occurrence(failure_id, event).await?;
        
        // Audit
        self.audit(failure_id, AuditAction::Captured, ActorType::System, json!({
            "event_timestamp": event.timestamp,
            "session_id": event.session_id,
        })).await?;
        
        Ok(ProcessResult::NewIssue(failure_id))
    }
    
    /// Get session context around a failure
    async fn get_session_context(
        &self,
        session_id: Option<Uuid>,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<SessionContext>> {
        let session_id = match session_id {
            Some(id) => id,
            None => return Ok(None),
        };
        
        // Get recent session entries before the failure
        let entries = sqlx::query_as!(
            SessionEntry,
            r#"
            SELECT entry_type, content, timestamp
            FROM sessions.log
            WHERE session_id = $1
              AND timestamp BETWEEN $2 AND $3
            ORDER BY timestamp DESC
            LIMIT 10
            "#,
            session_id,
            timestamp - Duration::minutes(5),
            timestamp + Duration::seconds(1),
        )
        .fetch_all(&self.pool)
        .await?;
        
        if entries.is_empty() {
            return Ok(None);
        }
        
        // Extract user intent (last user_input or agent_thought before failure)
        let user_intent = entries.iter()
            .rev()
            .find(|e| e.entry_type == "user_input" || e.entry_type == "agent_thought")
            .map(|e| e.content.clone());
        
        // Extract command sequence
        let command_sequence: Vec<String> = entries.iter()
            .rev()
            .filter(|e| e.entry_type == "dsl_command")
            .take(5)
            .map(|e| e.content.clone())
            .collect();
        
        Ok(Some(SessionContext {
            user_intent,
            command_sequence,
            entries: entries.into_iter().rev().collect(),
        }))
    }
    
    // ... more methods
}

#[derive(Debug)]
pub struct AnalysisReport {
    pub analyzed_since: DateTime<Utc>,
    pub events_processed: usize,
    pub new_issues: usize,
    pub updated_issues: usize,
    pub current_issues: Vec<IssueSummary>,
}

#[derive(Debug)]
pub struct SessionContext {
    pub user_intent: Option<String>,
    pub command_sequence: Vec<String>,
    pub entries: Vec<SessionEntry>,
}

#[derive(Debug)]
pub struct SessionEntry {
    pub entry_type: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

enum ProcessResult {
    NewIssue(Uuid),
    ExistingIssue(Uuid),
    Skipped,
}
```

### 3.2 Failure Store

```rust
// rust/src/feedback/store.rs

impl FeedbackInspector {
    async fn create_failure(
        &self,
        fingerprint: &str,
        fingerprint_key: &str,
        fingerprint_version: u8,
        error_type: ErrorType,
        path: RemediationPath,
        verb: &str,
        error: &ErrorSnapshot,
        event: &DslEvent,
        session_context: Option<SessionContext>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        
        // Redact raw response if present
        let redacted_response = event.raw_response()
            .map(|r| self.redactor.redact_for_error(r, error_type));
        
        sqlx::query!(
            r#"
            INSERT INTO feedback.failures (
                id, fingerprint, fingerprint_key, fingerprint_version,
                first_seen_at, last_seen_at,
                error_type, path,
                verb, source_id, error_message, error_details,
                sample_raw_response, sample_http_status, sample_dsl_text,
                handler, file, line,
                first_session_id, user_context,
                suggested_action
            ) VALUES (
                $1, $2, $3, $4,
                $5, $5,
                $6, $7,
                $8, $9, $10, $11,
                $12, $13, $14,
                $15, $16, $17,
                $18, $19,
                $20
            )
            "#,
            id,
            fingerprint,
            fingerprint_key,
            fingerprint_version as i16,
            event.timestamp,
            error_type as ErrorType,
            path as RemediationPath,
            verb,
            error.source_id.as_deref(),
            error.message,
            serde_json::to_value(&error).ok(),
            redacted_response,
            error.http_status.map(|s| s as i32),
            event.dsl_text(),
            error.handler.as_deref(),
            error.file.as_deref(),
            error.line.map(|l| l as i32),
            event.session_id,
            session_context.map(|c| serde_json::to_value(&c).unwrap_or_default()),
            self.classifier.suggest_action(error_type),
        )
        .execute(&self.pool)
        .await?;
        
        Ok(id)
    }
    
    async fn record_occurrence(&self, failure_id: Uuid, event: &DslEvent) -> Result<()> {
        // Insert occurrence
        sqlx::query!(
            r#"
            INSERT INTO feedback.occurrences (failure_id, occurred_at, session_id, http_status, duration_ms)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            failure_id,
            event.timestamp,
            event.session_id,
            event.http_status().map(|s| s as i32),
            event.duration_ms().map(|d| d as i32),
        )
        .execute(&self.pool)
        .await?;
        
        // Update failure stats
        sqlx::query!(
            r#"
            UPDATE feedback.failures
            SET occurrence_count = occurrence_count + 1,
                last_seen_at = $1,
                last_occurrence_at = $1,
                updated_at = NOW()
            WHERE id = $2
            "#,
            event.timestamp,
            failure_id,
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn list_issues(&self, filter: IssueFilter) -> Result<Vec<IssueSummary>> {
        sqlx::query_as!(
            IssueSummary,
            r#"
            SELECT 
                id,
                fingerprint,
                error_type as "error_type: ErrorType",
                path as "path: RemediationPath",
                verb,
                source_id,
                error_message,
                occurrence_count,
                first_seen_at,
                last_seen_at,
                status as "status: IssueStatus",
                suggested_action,
                user_context->>'user_intent' as user_intent
            FROM feedback.failures
            WHERE ($1::feedback.issue_status IS NULL OR status = $1)
              AND ($2::feedback.remediation_path IS NULL OR path = $2)
              AND ($3::text IS NULL OR verb LIKE '%' || $3 || '%')
            ORDER BY 
                CASE WHEN $4 = 'occurrence' THEN occurrence_count END DESC,
                CASE WHEN $4 = 'recent' THEN last_seen_at END DESC,
                last_seen_at DESC
            LIMIT $5
            "#,
            filter.status as Option<IssueStatus>,
            filter.path as Option<RemediationPath>,
            filter.verb_pattern,
            filter.order_by.unwrap_or("recent"),
            filter.limit.unwrap_or(20) as i32,
        )
        .fetch_all(&self.pool)
        .await
    }
    
    pub async fn get_issue(&self, fingerprint: &str) -> Result<Option<IssueDetail>> {
        let failure = sqlx::query_as!(
            FailureRecord,
            "SELECT * FROM feedback.failures WHERE fingerprint = $1",
            fingerprint
        )
        .fetch_optional(&self.pool)
        .await?;
        
        let failure = match failure {
            Some(f) => f,
            None => return Ok(None),
        };
        
        // Get recent occurrences
        let occurrences = sqlx::query_as!(
            OccurrenceRecord,
            r#"
            SELECT * FROM feedback.occurrences 
            WHERE failure_id = $1 
            ORDER BY occurred_at DESC 
            LIMIT 10
            "#,
            failure.id
        )
        .fetch_all(&self.pool)
        .await?;
        
        // Get audit trail
        let audit_trail = sqlx::query_as!(
            AuditRecord,
            r#"
            SELECT * FROM feedback.audit_log
            WHERE failure_id = $1
            ORDER BY timestamp DESC
            LIMIT 50
            "#,
            failure.id
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(Some(IssueDetail {
            failure,
            occurrences,
            audit_trail,
        }))
    }
}

#[derive(Debug, Default)]
pub struct IssueFilter {
    pub status: Option<IssueStatus>,
    pub path: Option<RemediationPath>,
    pub verb_pattern: Option<String>,
    pub order_by: Option<&'static str>,
    pub limit: Option<i32>,
}
```

### 3.3 Audit Trail

```rust
// rust/src/feedback/audit.rs

impl FeedbackInspector {
    pub async fn audit(
        &self,
        failure_id: Uuid,
        action: AuditAction,
        actor_type: ActorType,
        details: Value,
    ) -> Result<()> {
        self.audit_with_evidence(failure_id, action, actor_type, None, details, None, None).await
    }
    
    pub async fn audit_with_evidence(
        &self,
        failure_id: Uuid,
        action: AuditAction,
        actor_type: ActorType,
        actor_id: Option<&str>,
        details: Value,
        evidence: Option<&str>,
        evidence_hash: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO feedback.audit_log (
                failure_id, action, actor_type, actor_id, details, evidence, evidence_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            failure_id,
            action as AuditAction,
            actor_type as ActorType,
            actor_id,
            details,
            evidence,
            evidence_hash,
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_audit_trail(&self, fingerprint: &str) -> Result<Vec<AuditRecord>> {
        sqlx::query_as!(
            AuditRecord,
            r#"
            SELECT a.*
            FROM feedback.audit_log a
            JOIN feedback.failures f ON f.id = a.failure_id
            WHERE f.fingerprint = $1
            ORDER BY a.timestamp ASC
            "#,
            fingerprint
        )
        .fetch_all(&self.pool)
        .await
    }
}
```

---

## Part 4: Repro Test Generator

```rust
// rust/src/feedback/repro.rs

use std::path::PathBuf;
use tokio::fs;
use tokio::process::Command;

pub struct ReproGenerator {
    tests_dir: PathBuf,
}

impl ReproGenerator {
    pub fn new(tests_dir: PathBuf) -> Self {
        Self { tests_dir }
    }
    
    /// Generate repro test and verify it fails
    pub async fn generate_and_verify(
        &self,
        inspector: &FeedbackInspector,
        fingerprint: &str,
    ) -> Result<ReproResult> {
        let issue = inspector.get_issue(fingerprint).await?
            .ok_or_else(|| anyhow!("Issue not found: {}", fingerprint))?;
        
        // 1. Generate the test
        let artifact = self.generate(&issue.failure).await?;
        
        // 2. Update database
        inspector.set_repro(
            fingerprint,
            &artifact.repro_type,
            &artifact.repro_path,
        ).await?;
        
        // Audit
        inspector.audit(
            issue.failure.id,
            AuditAction::ReproGenerated,
            ActorType::System,
            json!({
                "repro_type": artifact.repro_type,
                "repro_path": artifact.repro_path,
            }),
        ).await?;
        
        // 3. Verify it fails (pre-fix)
        let verification = self.verify_fails(&artifact).await?;
        
        if verification.passed {
            // Test passed when it should fail - bad repro
            inspector.audit(
                issue.failure.id,
                AuditAction::ReproVerificationFailed,
                ActorType::System,
                json!({
                    "reason": "Test passed but should fail",
                    "output": verification.output,
                }),
            ).await?;
            
            return Ok(ReproResult {
                artifact,
                verified: false,
                verification_output: verification.output,
                error: Some("Test passed but should fail (pre-fix)".to_string()),
            });
        }
        
        // Test failed as expected - good repro
        inspector.audit_with_evidence(
            issue.failure.id,
            AuditAction::ReproVerifiedFails,
            ActorType::System,
            None,
            json!({}),
            Some(&verification.output),
            Some(&sha256_hash(&verification.output)),
        ).await?;
        
        // Update status
        inspector.set_status(fingerprint, IssueStatus::ReproVerified).await?;
        
        Ok(ReproResult {
            artifact,
            verified: true,
            verification_output: verification.output,
            error: None,
        })
    }
    
    async fn generate(&self, failure: &FailureRecord) -> Result<ReproArtifact> {
        match failure.error_type {
            ErrorType::EnumDrift | ErrorType::SchemaDrift | ErrorType::ParseError => {
                self.generate_golden_json_test(failure).await
            }
            ErrorType::HandlerPanic | ErrorType::HandlerError | ErrorType::DslParseError => {
                self.generate_dsl_scenario_test(failure).await
            }
            _ => self.generate_generic_test(failure).await,
        }
    }
    
    async fn verify_fails(&self, artifact: &ReproArtifact) -> Result<VerificationResult> {
        let output = Command::new("cargo")
            .args(["test", "--", &artifact.test_name, "--exact", "--nocapture"])
            .current_dir(&self.tests_dir.parent().unwrap())
            .output()
            .await?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);
        
        Ok(VerificationResult {
            passed: output.status.success(),
            output: combined,
        })
    }
    
    pub async fn verify_passes(&self, artifact: &ReproArtifact) -> Result<VerificationResult> {
        // Same as verify_fails, but we expect it to pass
        self.verify_fails(artifact).await
    }
    
    // ... generate_golden_json_test, generate_dsl_scenario_test, etc.
    // (Same as in original 023, moved here)
}

#[derive(Debug)]
pub struct ReproResult {
    pub artifact: ReproArtifact,
    pub verified: bool,
    pub verification_output: String,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct ReproArtifact {
    pub repro_type: String,
    pub repro_path: String,
    pub test_name: String,
    pub content: String,
}

#[derive(Debug)]
struct VerificationResult {
    passed: bool,
    output: String,
}
```

---

## Part 5: TODO Generator

```rust
// rust/src/feedback/todo.rs

impl FeedbackInspector {
    pub async fn generate_todo(
        &self,
        fingerprint: &str,
        todo_number: i32,
    ) -> Result<TodoResult> {
        let issue = self.get_issue(fingerprint).await?
            .ok_or_else(|| anyhow!("Issue not found"))?;
        
        // Check we have verified repro
        if !issue.failure.repro_pre_fix_verified.unwrap_or(false) {
            return Err(anyhow!(
                "Cannot create TODO without verified repro. Run generate_repro_test first."
            ));
        }
        
        let todo_path = format!(
            "ai-thoughts/{:03}-fix-{}.md",
            todo_number,
            slugify(&issue.failure.error_message, 40)
        );
        
        let todo_content = self.format_todo(&issue, todo_number);
        
        // Write file
        tokio::fs::write(&todo_path, &todo_content).await?;
        
        // Update database
        sqlx::query!(
            r#"
            UPDATE feedback.failures
            SET status = 'TODO_CREATED',
                todo_number = $1,
                todo_path = $2,
                updated_at = NOW()
            WHERE fingerprint = $3
            "#,
            todo_number,
            todo_path,
            fingerprint,
        )
        .execute(&self.pool)
        .await?;
        
        // Audit
        self.audit(
            issue.failure.id,
            AuditAction::TodoCreated,
            ActorType::McpAgent, // or whoever called this
            json!({
                "todo_number": todo_number,
                "todo_path": &todo_path,
            }),
        ).await?;
        
        Ok(TodoResult {
            path: todo_path,
            content: todo_content,
        })
    }
    
    fn format_todo(&self, issue: &IssueDetail, todo_number: i32) -> String {
        let f = &issue.failure;
        
        format!(r#"# {:03}: Fix {}

> **Status:** TODO
> **Fingerprint:** {}
> **Error Type:** {:?}
> **Verb:** {}
> **Occurrences:** {}
> **First Seen:** {}

---

## User Context

{}

---

## Reproduction Test

**Path:** `{}`
**Type:** {}

### Step 1: Verify test FAILS (before fix)

```bash
cargo test -- {} --exact
```

✅ Verified failing at: {}

---

## Error Details

```
{}
```

### DSL Command

```dsl
{}
```

{}

---

## Implementation

### Suggested Action: {}

### Files to Modify

- `{}`

### Tasks

- [x] Verify repro test FAILS (pre-fix)
- [ ] Implement fix
- [ ] Verify repro test PASSES (post-fix)
- [ ] Run full test suite
- [ ] Commit with message: "fix({}): {}"

### Step 2: Verify test PASSES (after fix)

```bash
cargo test -- {} --exact
```

---

## Audit Trail

{}

---

## Links

- Fingerprint: `{}`
"#,
            todo_number,
            truncate(&f.error_message, 50),
            f.fingerprint,
            f.error_type,
            f.verb,
            f.occurrence_count,
            f.first_seen_at.format("%Y-%m-%d %H:%M"),
            f.user_context.as_ref()
                .and_then(|c| c.get("user_intent"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown"),
            f.repro_path.as_deref().unwrap_or("Not generated"),
            f.repro_type.as_deref().unwrap_or("unknown"),
            f.repro_path.as_deref().unwrap_or("").replace("rust/tests/", ""),
            f.repro_pre_fix_verified_at
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Not verified".to_string()),
            f.error_message,
            f.sample_dsl_text.as_deref().unwrap_or(&f.verb),
            f.sample_raw_response.as_ref()
                .map(|r| format!("### Raw Response (Redacted)\n\n```json\n{}\n```", 
                    serde_json::to_string_pretty(r).unwrap_or_default()))
                .unwrap_or_default(),
            f.suggested_action.as_deref().unwrap_or("Investigate"),
            f.file.as_deref().unwrap_or("TBD"),
            f.verb.split('.').next().unwrap_or("dsl"),
            truncate(&f.error_message, 50),
            f.repro_path.as_deref().unwrap_or("").replace("rust/tests/", ""),
            issue.audit_trail.iter()
                .map(|a| format!("- {} [{}] {} - {}", 
                    a.timestamp.format("%m-%d %H:%M"),
                    a.actor_type,
                    a.action,
                    a.details))
                .collect::<Vec<_>>()
                .join("\n"),
            f.fingerprint,
        )
    }
}

#[derive(Debug)]
pub struct TodoResult {
    pub path: String,
    pub content: String,
}
```

---

## Part 6: MCP Server

```typescript
// mcp-servers/ob-feedback/src/index.ts

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import pg from "pg";

const server = new Server({
  name: "ob-feedback",
  version: "1.0.0"
}, {
  capabilities: { tools: {} }
});

server.setRequestHandler("tools/call", async (request) => {
  const { name, arguments: args } = request.params;
  
  // Create inspector for this request
  const inspector = await createInspector();
  
  try {
    switch (name) {
      case "analyze_failures": {
        const report = await inspector.analyze(args.since);
        return formatAnalysisReport(report);
      }
      
      case "list_failures": {
        const issues = await inspector.listIssues({
          status: args.status,
          path: args.path,
          verb: args.verb,
          limit: args.limit || 10,
        });
        return formatIssueList(issues);
      }
      
      case "get_failure": {
        const issue = await inspector.getIssue(args.fingerprint);
        return formatIssueDetail(issue);
      }
      
      case "get_failure_context": {
        const issue = await inspector.getIssue(args.fingerprint);
        // Includes full session context, audit trail
        return formatIssueWithContext(issue);
      }
      
      case "generate_repro_test": {
        const result = await inspector.generateAndVerifyRepro(args.fingerprint);
        return formatReproResult(result);
      }
      
      case "verify_repro_passes": {
        const result = await inspector.verifyReproPasses(args.fingerprint);
        return formatVerificationResult(result);
      }
      
      case "generate_todo": {
        const result = await inspector.generateTodo(args.fingerprint, args.todo_number);
        return formatTodoResult(result);
      }
      
      case "mark_fixed": {
        await inspector.markFixed(args.fingerprint, args.commit, args.notes);
        return { content: [{ type: "text", text: `Marked ${args.fingerprint} as fixed in ${args.commit}` }] };
      }
      
      case "get_audit_trail": {
        const trail = await inspector.getAuditTrail(args.fingerprint);
        return formatAuditTrail(trail);
      }
      
      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  } finally {
    await inspector.close();
  }
});

const transport = new StdioServerTransport();
server.connect(transport);
```

---

## Part 7: REPL Integration

```rust
// In REPL command handler

":feedback" | ":fb" => {
    let inspector = FeedbackInspector::new(pool.clone(), event_store_path.clone());
    
    // Run analysis
    let report = inspector.analyze(None).await?;
    
    println!("=== Feedback Analysis ===\n");
    println!("Analyzed since: {}", report.analyzed_since.format("%Y-%m-%d %H:%M"));
    println!("Events processed: {}", report.events_processed);
    println!("New issues: {}", report.new_issues);
    println!("Updated issues: {}", report.updated_issues);
    println!();
    
    if report.current_issues.is_empty() {
        println!("No issues requiring attention.");
    } else {
        println!("Issues needing attention:\n");
        for (i, issue) in report.current_issues.iter().enumerate() {
            println!("{}. {:?} - {}", i + 1, issue.error_type, truncate(&issue.error_message, 50));
            println!("   Verb: {} | Occurrences: {}", issue.verb, issue.occurrence_count);
            if let Some(intent) = &issue.user_intent {
                println!("   Context: {}", truncate(intent, 60));
            }
            println!("   Fingerprint: {}", issue.fingerprint);
            println!();
        }
    }
}

":fb <fingerprint>" => {
    let inspector = FeedbackInspector::new(pool.clone(), event_store_path.clone());
    
    let issue = inspector.get_issue(fingerprint).await?
        .ok_or_else(|| anyhow!("Issue not found"))?;
    
    println!("=== Issue: {} ===\n", fingerprint);
    println!("Error Type: {:?}", issue.failure.error_type);
    println!("Status: {:?}", issue.failure.status);
    println!("Verb: {}", issue.failure.verb);
    println!("Occurrences: {}", issue.failure.occurrence_count);
    println!();
    
    println!("Error: {}\n", issue.failure.error_message);
    
    if let Some(ctx) = &issue.failure.user_context {
        println!("User Context:");
        if let Some(intent) = ctx.get("user_intent").and_then(|v| v.as_str()) {
            println!("  Intent: {}", intent);
        }
        println!();
    }
    
    println!("Audit Trail:");
    for entry in &issue.audit_trail {
        println!("  {} [{:?}] {:?}", 
            entry.timestamp.format("%m-%d %H:%M"),
            entry.actor_type,
            entry.action);
    }
}

":fb repro <fingerprint>" => {
    let inspector = FeedbackInspector::new(pool.clone(), event_store_path.clone());
    let repro_gen = ReproGenerator::new(tests_dir.clone());
    
    println!("Generating repro test...");
    let result = repro_gen.generate_and_verify(&inspector, fingerprint).await?;
    
    if result.verified {
        println!("✅ Repro test generated and verified");
        println!("   Path: {}", result.artifact.repro_path);
        println!("   Test correctly FAILS (pre-fix)");
    } else {
        println!("❌ Repro generation failed");
        println!("   Error: {}", result.error.unwrap_or_default());
    }
}
```

---

## Part 8: Implementation Summary

### 8.1 Files to Create

```
rust/src/feedback/
├── mod.rs           # Public API
├── inspector.rs     # FeedbackInspector
├── classifier.rs    # FailureClassifier (from 023)
├── redactor.rs      # Redactor (from 023)
├── store.rs         # Database operations
├── audit.rs         # Audit trail
├── repro.rs         # ReproGenerator
└── todo.rs          # TODO generator

mcp-servers/ob-feedback/
├── package.json
├── tsconfig.json
└── src/
    └── index.ts

migrations/
└── 023b_feedback.sql
```

### 8.2 Effort Estimate

| Task | Hours |
|------|-------|
| Database schema | 2h |
| FeedbackInspector core | 4h |
| Classification/fingerprinting | 2h |
| Session context enrichment | 2h |
| Audit trail | 2h |
| Repro generator | 4h |
| Repro verification | 3h |
| TODO generator | 2h |
| MCP server | 4h |
| REPL integration | 2h |
| Tests | 4h |
| **Total** | **~31h** |

---

## Summary

**023a (Event Infrastructure):** Always on, ~500ns overhead, captures events + session context
**023b (Feedback Inspector):** On-demand, spins up when you call `:feedback` or MCP tools

Together they give you:
- Full audit trail from failure → repro → TODO → fix → verify → deploy
- Session context (what was user trying to do?)
- Verified repro tests (test FAILS before fix, PASSES after)
- Zero impact on DSL pipeline performance

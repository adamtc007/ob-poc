//! Core types for the Feedback Inspector
//!
//! These types mirror the SQL enums in feedback schema and provide
//! Rust representations for failure classification, status tracking,
//! and audit trail.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

// =============================================================================
// ERROR TYPE
// =============================================================================

/// Classification of error types for remediation routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "feedback.error_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorType {
    // Transient (runtime retry candidates)
    Timeout,
    RateLimited,
    ConnectionReset,
    ServiceUnavailable,
    PoolExhausted,

    // Schema/contract issues (code fix required)
    EnumDrift,
    SchemaDrift,

    // Code bugs (investigation needed)
    ParseError,
    HandlerPanic,
    HandlerError,
    DslParseError,

    // External API changes
    ApiEndpointMoved,
    ApiAuthChanged,
    ValidationFailed,

    // Catch-all
    Unknown,
}

impl ErrorType {
    /// Returns the string representation matching SQL enum
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Timeout => "TIMEOUT",
            Self::RateLimited => "RATE_LIMITED",
            Self::ConnectionReset => "CONNECTION_RESET",
            Self::ServiceUnavailable => "SERVICE_UNAVAILABLE",
            Self::PoolExhausted => "POOL_EXHAUSTED",
            Self::EnumDrift => "ENUM_DRIFT",
            Self::SchemaDrift => "SCHEMA_DRIFT",
            Self::ParseError => "PARSE_ERROR",
            Self::HandlerPanic => "HANDLER_PANIC",
            Self::HandlerError => "HANDLER_ERROR",
            Self::DslParseError => "DSL_PARSE_ERROR",
            Self::ApiEndpointMoved => "API_ENDPOINT_MOVED",
            Self::ApiAuthChanged => "API_AUTH_CHANGED",
            Self::ValidationFailed => "VALIDATION_FAILED",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Whether this error type is transient (can be retried)
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Timeout
                | Self::RateLimited
                | Self::ConnectionReset
                | Self::ServiceUnavailable
                | Self::PoolExhausted
        )
    }

    /// Whether this error type requires code changes
    pub fn requires_code_fix(&self) -> bool {
        matches!(
            self,
            Self::EnumDrift
                | Self::SchemaDrift
                | Self::ParseError
                | Self::HandlerPanic
                | Self::HandlerError
                | Self::DslParseError
                | Self::ApiEndpointMoved
                | Self::ApiAuthChanged
        )
    }
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// REMEDIATION PATH
// =============================================================================

/// How an error should be remediated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(
    type_name = "feedback.remediation_path",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum RemediationPath {
    /// Can be retried/recovered at runtime
    Runtime,
    /// Requires code change
    Code,
    /// Just log, no action needed
    LogOnly,
}

impl RemediationPath {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Runtime => "RUNTIME",
            Self::Code => "CODE",
            Self::LogOnly => "LOG_ONLY",
        }
    }
}

impl std::fmt::Display for RemediationPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// ISSUE STATUS
// =============================================================================

/// Lifecycle status of an issue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(
    type_name = "feedback.issue_status",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum IssueStatus {
    // Initial states
    New,
    RuntimeResolved,
    RuntimeEscalated,

    // Repro states
    ReproGenerated,
    ReproVerified,
    TodoCreated,

    // Fix states
    InProgress,
    FixCommitted,
    FixVerified,

    // Deployment states
    DeployedStaging,
    DeployedProd,
    Resolved,

    // Terminal states
    WontFix,
    Duplicate,
    Invalid,
}

impl IssueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::New => "NEW",
            Self::RuntimeResolved => "RUNTIME_RESOLVED",
            Self::RuntimeEscalated => "RUNTIME_ESCALATED",
            Self::ReproGenerated => "REPRO_GENERATED",
            Self::ReproVerified => "REPRO_VERIFIED",
            Self::TodoCreated => "TODO_CREATED",
            Self::InProgress => "IN_PROGRESS",
            Self::FixCommitted => "FIX_COMMITTED",
            Self::FixVerified => "FIX_VERIFIED",
            Self::DeployedStaging => "DEPLOYED_STAGING",
            Self::DeployedProd => "DEPLOYED_PROD",
            Self::Resolved => "RESOLVED",
            Self::WontFix => "WONT_FIX",
            Self::Duplicate => "DUPLICATE",
            Self::Invalid => "INVALID",
        }
    }

    /// Whether this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Resolved | Self::WontFix | Self::Duplicate | Self::Invalid
        )
    }

    /// Whether this issue is actively being worked on
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
            && !matches!(
                self,
                Self::RuntimeResolved | Self::DeployedProd | Self::DeployedStaging
            )
    }
}

impl std::fmt::Display for IssueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// ACTOR TYPE
// =============================================================================

/// Type of actor performing an action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "feedback.actor_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActorType {
    System,
    McpAgent,
    ReplUser,
    EguiUser,
    CiPipeline,
    ClaudeCode,
    CronJob,
}

impl ActorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "SYSTEM",
            Self::McpAgent => "MCP_AGENT",
            Self::ReplUser => "REPL_USER",
            Self::EguiUser => "EGUI_USER",
            Self::CiPipeline => "CI_PIPELINE",
            Self::ClaudeCode => "CLAUDE_CODE",
            Self::CronJob => "CRON_JOB",
        }
    }
}

impl std::fmt::Display for ActorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// AUDIT ACTION
// =============================================================================

/// Actions that can be recorded in the audit trail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(
    type_name = "feedback.audit_action",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum AuditAction {
    // Creation
    Captured,
    Classified,
    Deduplicated,

    // Runtime handling
    RuntimeAttempt,
    RuntimeSuccess,
    RuntimeExhausted,

    // Repro workflow
    ReproGenerated,
    ReproVerifiedFails,
    ReproVerificationFailed,

    // TODO workflow
    TodoCreated,
    TodoAssigned,
    FixCommitted,

    // Verification
    ReproVerifiedPasses,
    Deployed,
    SemanticReplayPassed,
    SemanticReplayFailed,

    // Terminal
    Resolved,
    MarkedWontFix,
    MarkedDuplicate,
    Reopened,
    CommentAdded,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Captured => "CAPTURED",
            Self::Classified => "CLASSIFIED",
            Self::Deduplicated => "DEDUPLICATED",
            Self::RuntimeAttempt => "RUNTIME_ATTEMPT",
            Self::RuntimeSuccess => "RUNTIME_SUCCESS",
            Self::RuntimeExhausted => "RUNTIME_EXHAUSTED",
            Self::ReproGenerated => "REPRO_GENERATED",
            Self::ReproVerifiedFails => "REPRO_VERIFIED_FAILS",
            Self::ReproVerificationFailed => "REPRO_VERIFICATION_FAILED",
            Self::TodoCreated => "TODO_CREATED",
            Self::TodoAssigned => "TODO_ASSIGNED",
            Self::FixCommitted => "FIX_COMMITTED",
            Self::ReproVerifiedPasses => "REPRO_VERIFIED_PASSES",
            Self::Deployed => "DEPLOYED",
            Self::SemanticReplayPassed => "SEMANTIC_REPLAY_PASSED",
            Self::SemanticReplayFailed => "SEMANTIC_REPLAY_FAILED",
            Self::Resolved => "RESOLVED",
            Self::MarkedWontFix => "MARKED_WONT_FIX",
            Self::MarkedDuplicate => "MARKED_DUPLICATE",
            Self::Reopened => "REOPENED",
            Self::CommentAdded => "COMMENT_ADDED",
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Summary of an issue for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub id: Uuid,
    pub fingerprint: String,
    pub error_type: ErrorType,
    pub remediation_path: RemediationPath,
    pub status: IssueStatus,
    pub verb: String,
    pub source: Option<String>,
    pub error_message: String,
    pub user_intent: Option<String>,
    pub occurrence_count: i32,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub repro_verified: bool,
}

/// Full detail of an issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDetail {
    pub failure: FailureRecord,
    pub occurrences: Vec<OccurrenceRecord>,
    pub audit_trail: Vec<AuditRecord>,
}

/// Full failure record from database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    pub id: Uuid,
    pub fingerprint: String,
    pub fingerprint_version: i16,
    pub error_type: ErrorType,
    pub remediation_path: RemediationPath,
    pub status: IssueStatus,
    pub verb: String,
    pub source: Option<String>,
    pub error_message: String,
    pub error_context: Option<serde_json::Value>,
    pub user_intent: Option<String>,
    pub command_sequence: Option<Vec<String>>,
    pub repro_type: Option<String>,
    pub repro_path: Option<String>,
    pub repro_verified: bool,
    pub fix_commit: Option<String>,
    pub fix_notes: Option<String>,
    pub occurrence_count: i32,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Individual occurrence record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OccurrenceRecord {
    pub id: Uuid,
    pub failure_id: Uuid,
    pub event_id: Option<Uuid>,
    pub event_timestamp: DateTime<Utc>,
    pub session_id: Option<Uuid>,
    pub verb: String,
    pub duration_ms: Option<i64>,
    pub error_message: String,
    pub error_backtrace: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Audit trail record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: Uuid,
    pub failure_id: Uuid,
    pub action: AuditAction,
    pub actor_type: ActorType,
    pub actor_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub evidence: Option<String>,
    pub evidence_hash: Option<String>,
    pub previous_status: Option<IssueStatus>,
    pub new_status: Option<IssueStatus>,
    pub created_at: DateTime<Utc>,
}

/// Session context for understanding what user was trying to do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: Option<Uuid>,
    pub user_intent: Option<String>,
    pub command_sequence: Vec<String>,
    pub entries: Vec<SessionEntry>,
}

/// Individual session log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub entry_type: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Input for creating an audit entry
#[derive(Debug, Clone)]
pub struct AuditEntry<'a> {
    pub failure_id: Uuid,
    pub action: AuditAction,
    pub actor_type: ActorType,
    pub actor_id: Option<&'a str>,
    pub details: Option<serde_json::Value>,
    pub evidence: Option<&'a str>,
    pub evidence_hash: Option<&'a str>,
}

impl<'a> AuditEntry<'a> {
    /// Create a new audit entry with required fields only
    pub fn new(failure_id: Uuid, action: AuditAction, actor_type: ActorType) -> Self {
        Self {
            failure_id,
            action,
            actor_type,
            actor_id: None,
            details: None,
            evidence: None,
            evidence_hash: None,
        }
    }

    /// Add optional details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Add optional actor ID
    pub fn with_actor_id(mut self, actor_id: &'a str) -> Self {
        self.actor_id = Some(actor_id);
        self
    }

    /// Add optional evidence
    pub fn with_evidence(mut self, evidence: &'a str, hash: &'a str) -> Self {
        self.evidence = Some(evidence);
        self.evidence_hash = Some(hash);
        self
    }
}

/// Filter for querying issues
#[derive(Debug, Clone, Default)]
pub struct IssueFilter {
    pub status: Option<IssueStatus>,
    pub error_type: Option<ErrorType>,
    pub remediation_path: Option<RemediationPath>,
    pub verb: Option<String>,
    pub source: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

/// Analysis report from inspector run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub events_processed: usize,
    pub failures_created: usize,
    pub failures_updated: usize,
    pub by_error_type: std::collections::HashMap<String, usize>,
    pub by_remediation_path: std::collections::HashMap<String, usize>,
    pub analyzed_at: DateTime<Utc>,
}

impl Default for AnalysisReport {
    fn default() -> Self {
        Self {
            events_processed: 0,
            failures_created: 0,
            failures_updated: 0,
            by_error_type: std::collections::HashMap::new(),
            by_remediation_path: std::collections::HashMap::new(),
            analyzed_at: Utc::now(),
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_type_is_transient() {
        assert!(ErrorType::Timeout.is_transient());
        assert!(ErrorType::RateLimited.is_transient());
        assert!(!ErrorType::EnumDrift.is_transient());
        assert!(!ErrorType::HandlerPanic.is_transient());
    }

    #[test]
    fn test_error_type_requires_code_fix() {
        assert!(ErrorType::EnumDrift.requires_code_fix());
        assert!(ErrorType::HandlerPanic.requires_code_fix());
        assert!(!ErrorType::Timeout.requires_code_fix());
        assert!(!ErrorType::Unknown.requires_code_fix());
    }

    #[test]
    fn test_issue_status_is_terminal() {
        assert!(IssueStatus::Resolved.is_terminal());
        assert!(IssueStatus::WontFix.is_terminal());
        assert!(!IssueStatus::New.is_terminal());
        assert!(!IssueStatus::InProgress.is_terminal());
    }

    #[test]
    fn test_issue_status_is_active() {
        assert!(IssueStatus::New.is_active());
        assert!(IssueStatus::InProgress.is_active());
        assert!(!IssueStatus::Resolved.is_active());
        assert!(!IssueStatus::RuntimeResolved.is_active());
    }

    #[test]
    fn test_error_type_display() {
        assert_eq!(ErrorType::Timeout.to_string(), "TIMEOUT");
        assert_eq!(ErrorType::EnumDrift.to_string(), "ENUM_DRIFT");
    }

    #[test]
    fn test_remediation_path_display() {
        assert_eq!(RemediationPath::Runtime.to_string(), "RUNTIME");
        assert_eq!(RemediationPath::Code.to_string(), "CODE");
    }
}

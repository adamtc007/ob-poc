//! Expansion Types
//!
//! Type definitions for template expansion, audit trails, and locking policy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// EXPANSION REPORT (Audit Trail)
// =============================================================================

/// Complete record of template expansion for audit/replay
///
/// Every expansion is captured in this report, which can be:
/// - Persisted to database for audit trail
/// - Used to replay "what did this template expand into?"
/// - Analyzed for debugging batch execution issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionReport {
    /// Unique identifier for this expansion
    pub expansion_id: Uuid,

    /// Hash of pre-expanded DSL (canonical, whitespace-normalized)
    pub source_digest: String,

    /// Hashes of each template definition used
    pub template_digests: Vec<TemplateDigest>,

    /// Details of each template invocation
    pub invocations: Vec<TemplateInvocationReport>,

    /// Total statements after expansion
    pub expanded_statement_count: usize,

    /// Hash of expanded DSL (canonical)
    pub expanded_dsl_digest: String,

    /// Locks inferred from metadata + args (sorted for deadlock prevention)
    pub derived_lock_set: Vec<LockKey>,

    /// Batch policy (atomic | best_effort)
    pub batch_policy: BatchPolicy,

    /// Warnings and errors during expansion
    pub diagnostics: Vec<ExpansionDiagnostic>,

    /// Timestamp of expansion
    pub expanded_at: DateTime<Utc>,
}

impl Default for ExpansionReport {
    fn default() -> Self {
        Self {
            expansion_id: Uuid::new_v4(),
            source_digest: String::new(),
            template_digests: Vec::new(),
            invocations: Vec::new(),
            expanded_statement_count: 0,
            expanded_dsl_digest: String::new(),
            derived_lock_set: Vec::new(),
            batch_policy: BatchPolicy::default(),
            diagnostics: Vec::new(),
            expanded_at: Utc::now(),
        }
    }
}

/// Hash of a template definition for audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDigest {
    /// Template name (e.g., "onboarding.research-group")
    pub name: String,
    /// Template version
    pub version: String,
    /// SHA-256 hash of template definition
    pub digest: String,
}

/// Details of a single template invocation within an expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInvocationReport {
    /// Template name
    pub name: String,
    /// Arguments passed to the template (as JSON)
    pub args_json: serde_json::Value,
    /// Policy for this template
    pub policy: TemplatePolicy,
    /// Source span of the invocation (if available)
    pub origin_span: Option<SpanRef>,
    /// Range of expanded statements this invocation produced
    pub expanded_range: ExpandedRange,
    /// Maps expanded statement index → template item index
    pub per_item_origins: Vec<PerItemOrigin>,
}

/// Reference to a span in source code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRef {
    /// Source file (if known)
    pub file: Option<String>,
    /// Start byte offset
    pub start: usize,
    /// End byte offset
    pub end: usize,
}

/// Range of expanded statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedRange {
    /// Start index (inclusive)
    pub start_index: usize,
    /// End index (exclusive)
    pub end_index_exclusive: usize,
}

/// Maps an expanded statement back to its template origin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerItemOrigin {
    /// Index in the expanded statement list
    pub expanded_statement_index: usize,
    /// Index in the template's item list
    pub template_item_index: usize,
}

// =============================================================================
// BATCH POLICY
// =============================================================================

/// Batch execution policy
///
/// Determines how failures are handled during batch execution:
/// - `Atomic`: All-or-nothing. Any failure rolls back entire batch.
/// - `BestEffort`: Partial success allowed. Failed items are skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BatchPolicy {
    /// All statements succeed or all are rolled back
    Atomic,
    /// Continue on failure, aggregate errors at the end
    #[default]
    BestEffort,
}

// =============================================================================
// RUNTIME POLICY (For Verbs)
// =============================================================================

/// Runtime policy for a verb, loaded from YAML metadata
///
/// This extends the VerbMetadata with execution-time policy decisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimePolicy {
    /// Batch execution policy
    #[serde(default)]
    pub batch: BatchPolicy,
    /// Locking policy (optional)
    #[serde(default)]
    pub locking: Option<LockingPolicy>,
}

// =============================================================================
// TEMPLATE POLICY
// =============================================================================

/// Policy for a template invocation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplatePolicy {
    /// Batch execution policy
    #[serde(default)]
    pub batch_policy: BatchPolicy,
    /// Locking configuration (optional)
    #[serde(default)]
    pub locking: Option<LockingPolicy>,
}

// =============================================================================
// LOCKING POLICY
// =============================================================================

/// Configuration for entity locking during batch execution
///
/// Locking prevents concurrent modification of entities during batch operations.
/// Without locking, a concurrent session could delete an entity mid-batch,
/// causing partial failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockingPolicy {
    /// Lock acquisition mode (not serialized — `Duration` is not serde-friendly;
    /// use `timeout_ms` to configure timeout from YAML/JSON).
    #[serde(skip, default)]
    pub mode: LockMode,
    /// Timeout in milliseconds (only used with `mode: block`)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Which arguments to lock
    pub targets: Vec<LockTarget>,
}

/// Lock acquisition mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LockMode {
    /// Non-blocking: fail immediately if lock unavailable
    #[default]
    Try,
    /// Blocking: wait for lock indefinitely
    Block,
    /// Blocking with timeout: wait up to `duration`, then fail with contention error.
    ///
    /// Implementation: `SET LOCAL statement_timeout = '<ms>'` before
    /// `pg_advisory_xact_lock()`, then `RESET statement_timeout` after.
    /// `SET LOCAL` scopes the timeout to the current transaction only —
    /// it does NOT leak to the connection pool. If the lock is not acquired
    /// within the duration, PostgreSQL raises error 57014 (query_canceled),
    /// which is caught and converted to `LockError::Contention`.
    Timeout(std::time::Duration),
}

/// Specifies which argument to lock and how
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockTarget {
    /// Argument name in verb call (e.g., "entity-id", "person-id")
    pub arg: String,
    /// Entity type for lock key (e.g., "person", "entity", "cbu")
    pub entity_type: String,
    /// Access type (read or write)
    #[serde(default)]
    pub access: LockAccess,
}

/// Lock access type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum LockAccess {
    /// Read lock - allows concurrent readers, blocks writers
    Read,
    /// Write lock - exclusive access
    #[default]
    Write,
}

// =============================================================================
// LOCK KEY (Runtime)
// =============================================================================

/// A concrete lock key derived from policy + runtime args
///
/// Lock keys are sorted before acquisition to prevent deadlocks.
/// The sort order is: (entity_type, entity_id, access).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LockKey {
    /// Entity type (e.g., "person", "entity", "cbu")
    pub entity_type: String,
    /// Entity UUID as string
    pub entity_id: String,
    /// Access type
    pub access: LockAccess,
}

impl LockKey {
    /// Create a new lock key
    pub fn new(
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        access: LockAccess,
    ) -> Self {
        Self {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            access,
        }
    }

    /// Create a write lock key
    pub fn write(entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        Self::new(entity_type, entity_id, LockAccess::Write)
    }

    /// Create a read lock key
    pub fn read(entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        Self::new(entity_type, entity_id, LockAccess::Read)
    }
}

impl PartialOrd for LockKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LockKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.entity_type, &self.entity_id, &self.access).cmp(&(
            &other.entity_type,
            &other.entity_id,
            &other.access,
        ))
    }
}

// =============================================================================
// DIAGNOSTICS
// =============================================================================

/// Diagnostic message from expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionDiagnostic {
    /// Severity level
    pub level: DiagnosticLevel,
    /// Human-readable message
    pub message: String,
    /// Path to the problematic element (e.g., "template.body[2].arg[0]")
    pub path: String,
}

/// Diagnostic severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    /// Expansion failed
    Error,
    /// Expansion succeeded but with warnings
    Warning,
    /// Informational message
    Info,
}

// NOTE: ErrorCause and CauseDetails moved to dsl_v2/errors.rs (Phase 4.1)

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_key_ordering() {
        let mut keys = [
            LockKey::write("person", "uuid-3"),
            LockKey::write("cbu", "uuid-1"),
            LockKey::read("person", "uuid-2"),
            LockKey::write("person", "uuid-2"),
        ];

        keys.sort();

        // Should be sorted by (entity_type, entity_id, access)
        assert_eq!(keys[0].entity_type, "cbu");
        assert_eq!(keys[1].entity_type, "person");
        assert_eq!(keys[1].entity_id, "uuid-2");
        assert_eq!(keys[2].entity_type, "person");
        assert_eq!(keys[2].entity_id, "uuid-2");
        assert_eq!(keys[3].entity_type, "person");
        assert_eq!(keys[3].entity_id, "uuid-3");
    }

    #[test]
    fn test_batch_policy_default() {
        let policy: BatchPolicy = Default::default();
        assert_eq!(policy, BatchPolicy::BestEffort);
    }

    #[test]
    fn test_batch_policy_serde() {
        let atomic: BatchPolicy = serde_json::from_str("\"atomic\"").unwrap();
        assert_eq!(atomic, BatchPolicy::Atomic);

        let best_effort: BatchPolicy = serde_json::from_str("\"best_effort\"").unwrap();
        assert_eq!(best_effort, BatchPolicy::BestEffort);
    }

    #[test]
    fn test_expansion_report_default() {
        let report = ExpansionReport::default();
        assert!(!report.expansion_id.is_nil());
        assert!(report.template_digests.is_empty());
        assert!(report.derived_lock_set.is_empty());
        assert_eq!(report.batch_policy, BatchPolicy::BestEffort);
    }

    #[test]
    fn test_lock_key_constructors() {
        let write_key = LockKey::write("person", "uuid-123");
        assert_eq!(write_key.access, LockAccess::Write);

        let read_key = LockKey::read("person", "uuid-123");
        assert_eq!(read_key.access, LockAccess::Read);
    }
}

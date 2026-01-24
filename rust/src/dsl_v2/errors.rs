//! Error Aggregation for DSL Execution
//!
//! Groups execution errors by root cause to avoid showing 50 separate
//! "entity deleted" errors when all 50 failed for the same reason.
//!
//! ## Key Features
//!
//! - Groups errors by root cause (EntityDeleted, EntityNotFound, etc.)
//! - Detects mid-execution timing (race condition detection)
//! - Provides human-readable summary
//! - Tracks affected verbs per cause

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// ERROR CAUSE
// =============================================================================

/// Root cause of an execution error
///
/// Errors are grouped by cause to avoid showing 50 separate "entity deleted"
/// errors when all 50 failed for the same reason.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorCause {
    /// Entity was deleted (possibly mid-batch)
    EntityDeleted { entity_id: String },
    /// Entity not found
    EntityNotFound { entity_id: String },
    /// Optimistic locking failure (version mismatch)
    VersionConflict { entity_id: String },
    /// Permission denied
    PermissionDenied { resource: String },
    /// Validation failed (constraint violation)
    ValidationFailed { rule: String },
    /// Foreign key constraint violation
    ForeignKeyViolation { constraint: String },
    /// Unique constraint violation
    UniqueViolation { constraint: String },
    /// Lock contention (could not acquire advisory lock)
    LockContention {
        entity_type: String,
        entity_id: String,
    },
    /// Database error
    DatabaseError { code: String },
    /// Other/unknown error
    Other { code: String },
}

impl ErrorCause {
    /// Create from an anyhow error, attempting to categorize it
    pub fn from_error(error: &anyhow::Error) -> Self {
        let msg = error.to_string().to_lowercase();

        // Try to extract entity ID from common patterns
        if msg.contains("not found") || msg.contains("does not exist") {
            if let Some(id) = extract_uuid_from_message(&msg) {
                return ErrorCause::EntityNotFound { entity_id: id };
            }
            return ErrorCause::EntityNotFound {
                entity_id: "unknown".to_string(),
            };
        }

        if msg.contains("deleted") {
            if let Some(id) = extract_uuid_from_message(&msg) {
                return ErrorCause::EntityDeleted { entity_id: id };
            }
            return ErrorCause::EntityDeleted {
                entity_id: "unknown".to_string(),
            };
        }

        if msg.contains("version") || msg.contains("optimistic") || msg.contains("concurrent") {
            if let Some(id) = extract_uuid_from_message(&msg) {
                return ErrorCause::VersionConflict { entity_id: id };
            }
            return ErrorCause::VersionConflict {
                entity_id: "unknown".to_string(),
            };
        }

        if msg.contains("permission") || msg.contains("denied") || msg.contains("unauthorized") {
            return ErrorCause::PermissionDenied {
                resource: extract_resource_from_message(&msg),
            };
        }

        if msg.contains("violates foreign key") {
            return ErrorCause::ForeignKeyViolation {
                constraint: extract_constraint_from_message(&msg),
            };
        }

        if msg.contains("violates unique") || msg.contains("duplicate key") {
            return ErrorCause::UniqueViolation {
                constraint: extract_constraint_from_message(&msg),
            };
        }

        if msg.contains("lock") && (msg.contains("contention") || msg.contains("timeout")) {
            return ErrorCause::LockContention {
                entity_type: "unknown".to_string(),
                entity_id: extract_uuid_from_message(&msg).unwrap_or_else(|| "unknown".to_string()),
            };
        }

        // Check for sqlx/database errors
        if msg.contains("sqlx") || msg.contains("postgres") || msg.contains("database") {
            return ErrorCause::DatabaseError {
                code: msg.chars().take(100).collect(),
            };
        }

        ErrorCause::Other {
            code: msg.chars().take(100).collect(),
        }
    }

    /// Get a short description of the cause
    pub fn short_description(&self) -> String {
        match self {
            ErrorCause::EntityDeleted { entity_id } => format!("Entity {} was deleted", entity_id),
            ErrorCause::EntityNotFound { entity_id } => format!("Entity {} not found", entity_id),
            ErrorCause::VersionConflict { entity_id } => {
                format!("Version conflict on entity {}", entity_id)
            }
            ErrorCause::PermissionDenied { resource } => format!("Permission denied: {}", resource),
            ErrorCause::ValidationFailed { rule } => format!("Validation failed: {}", rule),
            ErrorCause::ForeignKeyViolation { constraint } => {
                format!("Foreign key violation: {}", constraint)
            }
            ErrorCause::UniqueViolation { constraint } => {
                format!("Unique constraint violation: {}", constraint)
            }
            ErrorCause::LockContention { entity_type, .. } => {
                format!("Lock contention on {}", entity_type)
            }
            ErrorCause::DatabaseError { code } => format!("Database error: {}", code),
            ErrorCause::Other { code } => format!("Error: {}", code),
        }
    }

    /// Get the entity ID if this cause relates to a specific entity
    pub fn entity_id(&self) -> Option<&str> {
        match self {
            ErrorCause::EntityDeleted { entity_id }
            | ErrorCause::EntityNotFound { entity_id }
            | ErrorCause::VersionConflict { entity_id }
            | ErrorCause::LockContention { entity_id, .. } => Some(entity_id),
            _ => None,
        }
    }
}

// =============================================================================
// CAUSE DETAILS
// =============================================================================

/// Details about an error cause
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CauseDetails {
    /// Entity name (if known)
    pub entity_name: Option<String>,
    /// Who deleted the entity (if known)
    pub deleted_by: Option<String>,
    /// When the entity was deleted (if known)
    pub deleted_at: Option<DateTime<Utc>>,
    /// Human-readable hint for resolution
    #[serde(default)]
    pub hint: String,
    /// Whether this error is recoverable (can retry)
    #[serde(default)]
    pub recoverable: bool,
}

impl CauseDetails {
    /// Create details from an error cause
    pub fn from_cause(cause: &ErrorCause) -> Self {
        let (hint, recoverable) = match cause {
            ErrorCause::EntityDeleted { .. } => (
                "The entity was deleted. Check if another user or process deleted it.".to_string(),
                false,
            ),
            ErrorCause::EntityNotFound { .. } => (
                "The entity does not exist. Verify the ID is correct.".to_string(),
                false,
            ),
            ErrorCause::VersionConflict { .. } => (
                "The entity was modified by another process. Refresh and try again.".to_string(),
                true,
            ),
            ErrorCause::PermissionDenied { .. } => (
                "You don't have permission for this operation.".to_string(),
                false,
            ),
            ErrorCause::ValidationFailed { rule } => {
                (format!("Data validation failed: {}", rule), true)
            }
            ErrorCause::ForeignKeyViolation { .. } => {
                ("Referenced entity does not exist.".to_string(), false)
            }
            ErrorCause::UniqueViolation { .. } => {
                ("A record with this key already exists.".to_string(), false)
            }
            ErrorCause::LockContention { .. } => (
                "Another process is modifying this entity. Wait and try again.".to_string(),
                true,
            ),
            ErrorCause::DatabaseError { .. } => (
                "A database error occurred. Contact support if this persists.".to_string(),
                false,
            ),
            ErrorCause::Other { .. } => ("An unexpected error occurred.".to_string(), false),
        };

        Self {
            hint,
            recoverable,
            ..Default::default()
        }
    }
}

// =============================================================================
// AFFECTED VERB
// =============================================================================

/// Information about a verb that was affected by an error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedVerb {
    /// Index of the verb in the execution plan
    pub index: usize,
    /// Verb name (e.g., "create")
    pub verb: String,
    /// Domain name (e.g., "cbu")
    pub domain: String,
    /// Target entity (if applicable)
    pub target: Option<String>,
    /// The error message for this specific failure
    pub error_message: String,
}

// =============================================================================
// FAILURE TIMING
// =============================================================================

/// Detect if failure happened mid-execution (race condition)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FailureTiming {
    /// Entity was already deleted/missing when batch started
    PreExisting {
        /// When the entity was deleted
        deleted_at: Option<DateTime<Utc>>,
        /// When the batch started
        batch_started_at: DateTime<Utc>,
    },
    /// Entity was deleted/modified DURING batch execution
    MidExecution {
        /// When the entity was deleted/modified
        deleted_at: Option<DateTime<Utc>>,
        /// When first success occurred
        first_success_at: DateTime<Utc>,
        /// When first failure occurred
        first_failure_at: DateTime<Utc>,
        /// How many operations succeeded before the delete
        succeeded_before_delete: usize,
    },
}

// =============================================================================
// CAUSED ERRORS
// =============================================================================

/// Collection of errors sharing the same root cause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausedErrors {
    /// The root cause
    pub cause: ErrorCause,
    /// Details about the cause
    pub details: CauseDetails,
    /// List of affected verbs
    pub affected_verbs: Vec<AffectedVerb>,
    /// Total count of failures with this cause
    pub count: usize,
    /// Timing analysis (if available)
    pub timing: Option<FailureTiming>,
}

// =============================================================================
// EXECUTION ERRORS (AGGREGATED)
// =============================================================================

/// Aggregated execution errors grouped by root cause
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionErrors {
    /// Errors grouped by cause
    pub by_cause: HashMap<String, CausedErrors>,
    /// Total number of failed operations
    pub total_failed: usize,
    /// Total number of succeeded operations
    pub total_succeeded: usize,
}

impl ExecutionErrors {
    /// Create a new empty error collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful operation
    pub fn record_success(&mut self) {
        self.total_succeeded += 1;
    }

    /// Record a failed operation
    pub fn record_failure(
        &mut self,
        verb_index: usize,
        domain: &str,
        verb: &str,
        error: &anyhow::Error,
        target: Option<String>,
    ) {
        self.total_failed += 1;

        let cause = ErrorCause::from_error(error);
        let cause_key = format!("{:?}", cause); // Use debug representation as key

        let entry = self
            .by_cause
            .entry(cause_key)
            .or_insert_with(|| CausedErrors {
                cause: cause.clone(),
                details: CauseDetails::from_cause(&cause),
                affected_verbs: vec![],
                count: 0,
                timing: None,
            });

        entry.count += 1;
        entry.affected_verbs.push(AffectedVerb {
            index: verb_index,
            verb: verb.to_string(),
            domain: domain.to_string(),
            target,
            error_message: error.to_string(),
        });
    }

    /// Check if there are any errors
    pub fn is_empty(&self) -> bool {
        self.by_cause.is_empty()
    }

    /// Get the number of unique error causes
    pub fn cause_count(&self) -> usize {
        self.by_cause.len()
    }

    /// Generate human-readable summary
    pub fn summary(&self) -> String {
        if self.by_cause.is_empty() {
            return format!("✓ {} succeeded", self.total_succeeded);
        }

        let mut lines = vec![format!(
            "{} succeeded, {} failed ({} unique causes)",
            self.total_succeeded,
            self.total_failed,
            self.by_cause.len()
        )];

        for errors in self.by_cause.values() {
            lines.push(format!(
                "\n❌ {} operations failed: {}",
                errors.count, errors.details.hint
            ));

            // Show timing if mid-execution race detected
            if let Some(FailureTiming::MidExecution {
                succeeded_before_delete,
                ..
            }) = &errors.timing
            {
                lines.push(format!(
                    "   ⚠️ TIMING: Entity modified MID-EXECUTION ({} ops succeeded before failure)",
                    succeeded_before_delete
                ));
            }

            // Show first few affected verbs
            for verb in errors.affected_verbs.iter().take(3) {
                lines.push(format!(
                    "   • {}.{} (step {})",
                    verb.domain, verb.verb, verb.index
                ));
            }

            if errors.affected_verbs.len() > 3 {
                lines.push(format!("   ... +{} more", errors.affected_verbs.len() - 3));
            }
        }

        lines.join("\n")
    }

    /// Get all errors as a flat list
    pub fn all_errors(&self) -> Vec<&AffectedVerb> {
        self.by_cause
            .values()
            .flat_map(|e| e.affected_verbs.iter())
            .collect()
    }

    /// Check if any errors are recoverable
    pub fn has_recoverable(&self) -> bool {
        self.by_cause.values().any(|e| e.details.recoverable)
    }

    /// Get only recoverable errors
    pub fn recoverable_errors(&self) -> Vec<&CausedErrors> {
        self.by_cause
            .values()
            .filter(|e| e.details.recoverable)
            .collect()
    }

    /// Get only non-recoverable errors
    pub fn non_recoverable_errors(&self) -> Vec<&CausedErrors> {
        self.by_cause
            .values()
            .filter(|e| !e.details.recoverable)
            .collect()
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Extract a UUID from an error message
fn extract_uuid_from_message(msg: &str) -> Option<String> {
    // Look for UUID pattern: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    let re =
        regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").ok()?;
    re.find(msg).map(|m| m.as_str().to_string())
}

/// Extract a constraint name from an error message
fn extract_constraint_from_message(msg: &str) -> String {
    // Look for constraint name in quotes or after "constraint"
    if let Some(start) = msg.find('"') {
        if let Some(end) = msg[start + 1..].find('"') {
            return msg[start + 1..start + 1 + end].to_string();
        }
    }
    "unknown".to_string()
}

/// Extract a resource name from an error message
fn extract_resource_from_message(msg: &str) -> String {
    // Look for resource name in quotes
    if let Some(start) = msg.find('"') {
        if let Some(end) = msg[start + 1..].find('"') {
            return msg[start + 1..start + 1 + end].to_string();
        }
    }
    "unknown".to_string()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_cause_from_not_found() {
        let error = anyhow::anyhow!("Entity 550e8400-e29b-41d4-a716-446655440000 not found");
        let cause = ErrorCause::from_error(&error);

        match cause {
            ErrorCause::EntityNotFound { entity_id } => {
                assert_eq!(entity_id, "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("Expected EntityNotFound"),
        }
    }

    #[test]
    fn test_error_cause_from_deleted() {
        let error = anyhow::anyhow!("Entity 550e8400-e29b-41d4-a716-446655440000 was deleted");
        let cause = ErrorCause::from_error(&error);

        match cause {
            ErrorCause::EntityDeleted { entity_id } => {
                assert_eq!(entity_id, "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("Expected EntityDeleted"),
        }
    }

    #[test]
    fn test_error_cause_from_permission() {
        let error = anyhow::anyhow!("Permission denied for resource \"users\"");
        let cause = ErrorCause::from_error(&error);

        assert!(matches!(cause, ErrorCause::PermissionDenied { .. }));
    }

    #[test]
    fn test_execution_errors_aggregation() {
        let mut errors = ExecutionErrors::new();

        // Record some successes
        errors.record_success();
        errors.record_success();

        // Record failures with same cause
        let error1 = anyhow::anyhow!("Entity 550e8400-e29b-41d4-a716-446655440000 not found");
        let error2 = anyhow::anyhow!("Entity 550e8400-e29b-41d4-a716-446655440000 not found");

        errors.record_failure(0, "cbu", "create", &error1, None);
        errors.record_failure(1, "cbu", "create", &error2, None);

        // Should have 2 successes, 2 failures, 1 cause
        assert_eq!(errors.total_succeeded, 2);
        assert_eq!(errors.total_failed, 2);
        assert_eq!(errors.cause_count(), 1);

        // Summary should mention both failures
        let summary = errors.summary();
        assert!(summary.contains("2 succeeded"));
        assert!(summary.contains("2 failed"));
    }

    #[test]
    fn test_execution_errors_multiple_causes() {
        let mut errors = ExecutionErrors::new();

        let error1 = anyhow::anyhow!("Entity uuid-1 not found");
        let error2 = anyhow::anyhow!("Permission denied for resource \"admin\"");

        errors.record_failure(0, "cbu", "create", &error1, None);
        errors.record_failure(1, "user", "delete", &error2, None);

        // Should have 2 different causes
        assert_eq!(errors.cause_count(), 2);
    }

    #[test]
    fn test_empty_errors() {
        let errors = ExecutionErrors::new();
        assert!(errors.is_empty());
        assert_eq!(errors.summary(), "✓ 0 succeeded");
    }

    #[test]
    fn test_cause_details_recoverable() {
        let version_cause = ErrorCause::VersionConflict {
            entity_id: "test".to_string(),
        };
        let details = CauseDetails::from_cause(&version_cause);
        assert!(details.recoverable);

        let deleted_cause = ErrorCause::EntityDeleted {
            entity_id: "test".to_string(),
        };
        let details = CauseDetails::from_cause(&deleted_cause);
        assert!(!details.recoverable);
    }
}

//! DSL Sheet - Batch DSL execution with DAG-based phasing
//!
//! **DEPRECATED:** This module is superseded by `unified::RunSheet` and `unified::RunSheetEntry`.
//! New code should use `UnifiedSession.run_sheet` instead of `DslSheet`.
//!
//! Migration path:
//! - `DslSheet` → `UnifiedSession.run_sheet: RunSheet`
//! - `SessionDslStatement` → `RunSheetEntry`
//! - `StatementStatus` → `EntryStatus`
//! - `ExecutionPhase` → Use `RunSheet.by_phase(depth)`
//!
//! A DslSheet holds multiple DSL statements that form a "run sheet" for execution.
//! The sheet is analyzed for dependencies (DAG), split into phases, and executed
//! in order with symbol resolution between phases.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                          DSL Sheet Pipeline                                  │
//! │                                                                              │
//! │  Template DSL × Entity Set                                                   │
//! │         │                                                                    │
//! │         ▼                                                                    │
//! │  ┌─────────────┐                                                             │
//! │  │  GENERATE   │  Expand template for each entity in scope                   │
//! │  └─────────────┘                                                             │
//! │         │                                                                    │
//! │         ▼                                                                    │
//! │  ┌─────────────┐                                                             │
//! │  │    PARSE    │  Parse each statement, extract @symbols                     │
//! │  └─────────────┘                                                             │
//! │         │                                                                    │
//! │         ▼                                                                    │
//! │  ┌─────────────┐                                                             │
//! │  │  DAG SORT   │  Build dependency graph, compute phases                     │
//! │  └─────────────┘                                                             │
//! │         │                                                                    │
//! │         ▼                                                                    │
//! │  ┌─────────────┐  Phase 0: No dependencies (depth 0)                        │
//! │  │  EXECUTE    │  Phase 1: Depends on phase 0 outputs                        │
//! │  │  PHASED     │  Phase N: Depends on phase N-1 outputs                      │
//! │  └─────────────┘                                                             │
//! │         │                                                                    │
//! │         ▼                                                                    │
//! │  ┌─────────────┐                                                             │
//! │  │   RESULT    │  Per-statement status, returned PKs, error details          │
//! │  └─────────────┘                                                             │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// DSL SHEET
// =============================================================================

/// A batch of DSL statements to execute together
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSheet {
    /// Unique sheet ID
    pub id: Uuid,

    /// Session this sheet belongs to
    pub session_id: Uuid,

    /// Statements in submission order (before DAG reordering)
    pub statements: Vec<SessionDslStatement>,

    /// Computed execution phases (after DAG analysis)
    pub phases: Vec<ExecutionPhase>,

    /// Validation result from parser/compiler
    pub validation: Option<ValidationResult>,

    /// When the sheet was created
    pub created_at: DateTime<Utc>,
}

impl DslSheet {
    /// Create a new empty sheet
    pub fn new(session_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            statements: Vec::new(),
            phases: Vec::new(),
            validation: None,
            created_at: Utc::now(),
        }
    }

    /// Create sheet with statements
    pub fn with_statements(session_id: Uuid, statements: Vec<SessionDslStatement>) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            statements,
            phases: Vec::new(),
            validation: None,
            created_at: Utc::now(),
        }
    }

    /// Get statement count
    pub fn statement_count(&self) -> usize {
        self.statements.len()
    }

    /// Get phase count
    pub fn phase_count(&self) -> usize {
        self.phases.len()
    }

    /// Check if all statements are resolved (no unresolved symbols)
    pub fn is_fully_resolved(&self) -> bool {
        self.statements.iter().all(|s| {
            matches!(
                s.status,
                StatementStatus::Resolved | StatementStatus::Success
            )
        })
    }

    /// Get statements that need resolution
    pub fn unresolved_statements(&self) -> Vec<(usize, &SessionDslStatement)> {
        self.statements
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.status, StatementStatus::Parsed))
            .collect()
    }

    /// Get count of unresolved statements
    pub fn unresolved_count(&self) -> usize {
        self.statements
            .iter()
            .filter(|s| matches!(s.status, StatementStatus::Parsed))
            .count()
    }
}

// =============================================================================
// STATEMENT
// =============================================================================

/// A single DSL statement within a sheet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDslStatement {
    /// Index in the original submission order
    pub index: usize,

    /// The DSL source (may contain @symbol placeholders)
    pub source: String,

    /// DAG depth (0 = no deps, 1 = depends on depth 0, etc.)
    pub dag_depth: usize,

    /// Symbol this statement produces (if any) - extracted from `:as @name`
    pub produces: Option<String>,

    /// Symbols this statement consumes - extracted from `@name` references
    pub consumes: Vec<String>,

    /// Resolved argument values (symbol → UUID)
    pub resolved_args: HashMap<String, Uuid>,

    /// Primary key returned by execution (for dependent statements)
    pub returned_pk: Option<Uuid>,

    /// Current status
    pub status: StatementStatus,
}

impl SessionDslStatement {
    /// Create a new pending statement
    pub fn new(index: usize, source: String) -> Self {
        Self {
            index,
            source,
            dag_depth: 0,
            produces: None,
            consumes: Vec::new(),
            resolved_args: HashMap::new(),
            returned_pk: None,
            status: StatementStatus::Pending,
        }
    }

    /// Check if statement is ready for execution
    pub fn is_ready(&self) -> bool {
        matches!(self.status, StatementStatus::Resolved)
    }

    /// Check if statement has completed (success or failure)
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            StatementStatus::Success
                | StatementStatus::Failed { .. }
                | StatementStatus::Skipped { .. }
        )
    }

    /// Get the fully resolved source (symbols replaced with UUIDs)
    pub fn resolved_source(&self) -> Result<String, String> {
        let mut result = self.source.clone();
        for (symbol, uuid) in &self.resolved_args {
            let pattern = format!("@{}", symbol);
            result = result.replace(&pattern, &format!("\"{}\"", uuid));
        }
        // Check for any remaining unresolved @symbols
        if let Some(pos) = result.find('@') {
            let remaining: String = result[pos..].chars().take(20).collect();
            return Err(format!(
                "Unresolved symbol at position {}: {}",
                pos, remaining
            ));
        }
        Ok(result)
    }
}

// =============================================================================
// STATEMENT STATUS
// =============================================================================

/// Status of a statement in the execution pipeline
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StatementStatus {
    /// Not yet processed
    Pending,

    /// Parsed but has unresolved entity references
    Parsed,

    /// All references resolved, ready to execute
    Resolved,

    /// Currently executing
    Executing,

    /// Executed successfully
    Success,

    /// Execution failed
    Failed {
        /// Error message
        error: String,
        /// Error code for categorization
        code: ErrorCode,
    },

    /// Skipped because a dependency failed
    Skipped {
        /// Index of the statement that failed
        blocked_by: usize,
    },
}

impl StatementStatus {
    /// Create a failed status
    pub fn failed(error: impl Into<String>, code: ErrorCode) -> Self {
        Self::Failed {
            error: error.into(),
            code,
        }
    }

    /// Create a skipped status
    pub fn skipped(blocked_by: usize) -> Self {
        Self::Skipped { blocked_by }
    }

    /// Check if this is a terminal status (no further transitions)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Success | Self::Failed { .. } | Self::Skipped { .. }
        )
    }
}

// =============================================================================
// EXECUTION PHASE
// =============================================================================

/// A group of statements that can be executed in parallel (same DAG depth)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPhase {
    /// Phase depth (0 = first phase, no dependencies)
    pub depth: usize,

    /// Indices of statements in this phase
    pub statement_indices: Vec<usize>,

    /// Symbols produced by this phase
    pub produces: Vec<String>,

    /// Symbols consumed by this phase (must be produced by earlier phases)
    pub consumes: Vec<String>,
}

impl ExecutionPhase {
    /// Create a new empty phase
    pub fn new(depth: usize) -> Self {
        Self {
            depth,
            statement_indices: Vec::new(),
            produces: Vec::new(),
            consumes: Vec::new(),
        }
    }

    /// Get count of statements in this phase
    pub fn statement_count(&self) -> usize {
        self.statement_indices.len()
    }
}

// =============================================================================
// VALIDATION RESULT
// =============================================================================

/// Result of parsing/validating a sheet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether validation passed
    pub valid: bool,

    /// Syntax errors
    pub syntax_errors: Vec<ValidationError>,

    /// Semantic warnings
    pub warnings: Vec<ValidationWarning>,

    /// Unresolved entity references that need user input
    pub unresolved_refs: Vec<UnresolvedReference>,

    /// Cyclic dependencies detected (sheet cannot be executed)
    pub cyclic_dependencies: Vec<CyclicDependency>,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success() -> Self {
        Self {
            valid: true,
            syntax_errors: Vec::new(),
            warnings: Vec::new(),
            unresolved_refs: Vec::new(),
            cyclic_dependencies: Vec::new(),
        }
    }

    /// Create a failed validation result
    pub fn failed(errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            syntax_errors: errors,
            warnings: Vec::new(),
            unresolved_refs: Vec::new(),
            cyclic_dependencies: Vec::new(),
        }
    }

    /// Check if there are any blocking issues
    pub fn has_blocking_issues(&self) -> bool {
        !self.valid || !self.cyclic_dependencies.is_empty()
    }
}

/// A syntax or semantic error in validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Statement index
    pub statement_index: usize,

    /// Error message
    pub message: String,

    /// Source span (line, column, length)
    pub span: Option<SourceSpan>,

    /// Error code
    pub code: ErrorCode,
}

/// A warning (non-blocking)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Statement index
    pub statement_index: usize,

    /// Warning message
    pub message: String,

    /// Warning code
    pub code: String,
}

/// An unresolved entity reference needing user input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedReference {
    /// Statement index
    pub statement_index: usize,

    /// The reference text (e.g., "Goldman Sachs")
    pub reference_text: String,

    /// Expected entity type
    pub expected_type: Option<String>,

    /// Candidate matches from search
    pub candidates: Vec<EntityCandidate>,
}

/// A candidate entity match for resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidate {
    /// Entity ID
    pub entity_id: Uuid,

    /// Entity name
    pub name: String,

    /// Entity type
    pub entity_type: String,

    /// Match confidence (0.0 - 1.0)
    pub confidence: f64,
}

/// Cyclic dependency detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyclicDependency {
    /// Statements involved in the cycle
    pub statement_indices: Vec<usize>,

    /// Description of the cycle
    pub description: String,
}

/// Source span for error highlighting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Line number (1-indexed)
    pub line: usize,

    /// Column number (1-indexed)
    pub column: usize,

    /// Length in characters
    pub length: usize,
}

// =============================================================================
// ERROR CODES
// =============================================================================

/// Categorized error codes for DSL execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorCode {
    // Syntax errors
    SyntaxError,
    InvalidVerb,
    InvalidArgument,
    MissingRequired,

    // Resolution errors
    UnresolvedSymbol,
    AmbiguousEntity,
    EntityNotFound,
    TypeMismatch,

    // Execution errors
    DbConstraint,
    DbConnection,
    Timeout,
    PermissionDenied,

    // Dependency errors
    Blocked,
    CyclicDependency,

    // Internal errors
    InternalError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxError => write!(f, "SYNTAX_ERROR"),
            Self::InvalidVerb => write!(f, "INVALID_VERB"),
            Self::InvalidArgument => write!(f, "INVALID_ARGUMENT"),
            Self::MissingRequired => write!(f, "MISSING_REQUIRED"),
            Self::UnresolvedSymbol => write!(f, "UNRESOLVED_SYMBOL"),
            Self::AmbiguousEntity => write!(f, "AMBIGUOUS_ENTITY"),
            Self::EntityNotFound => write!(f, "ENTITY_NOT_FOUND"),
            Self::TypeMismatch => write!(f, "TYPE_MISMATCH"),
            Self::DbConstraint => write!(f, "DB_CONSTRAINT"),
            Self::DbConnection => write!(f, "DB_CONNECTION"),
            Self::Timeout => write!(f, "TIMEOUT"),
            Self::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            Self::Blocked => write!(f, "BLOCKED"),
            Self::CyclicDependency => write!(f, "CYCLIC_DEPENDENCY"),
            Self::InternalError => write!(f, "INTERNAL_ERROR"),
        }
    }
}

// =============================================================================
// EXECUTION RESULT
// =============================================================================

/// Result of executing a sheet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetExecutionResult {
    /// Session ID
    pub session_id: Uuid,

    /// Sheet ID
    pub sheet_id: Uuid,

    /// Overall status
    pub overall_status: SheetStatus,

    /// Phases completed before stopping
    pub phases_completed: usize,

    /// Total phases
    pub phases_total: usize,

    /// Per-statement results
    pub statements: Vec<StatementResult>,

    /// Execution start time
    pub started_at: DateTime<Utc>,

    /// Execution end time
    pub completed_at: DateTime<Utc>,

    /// Total execution time in milliseconds
    pub duration_ms: u64,
}

impl SheetExecutionResult {
    /// Get count of successful statements
    pub fn success_count(&self) -> usize {
        self.statements
            .iter()
            .filter(|s| s.status == StatementStatus::Success)
            .count()
    }

    /// Get count of failed statements
    pub fn failed_count(&self) -> usize {
        self.statements
            .iter()
            .filter(|s| matches!(s.status, StatementStatus::Failed { .. }))
            .count()
    }

    /// Get count of skipped statements
    pub fn skipped_count(&self) -> usize {
        self.statements
            .iter()
            .filter(|s| matches!(s.status, StatementStatus::Skipped { .. }))
            .count()
    }

    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.overall_status == SheetStatus::Success
    }
}

/// Overall sheet execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SheetStatus {
    /// All statements executed successfully
    Success,

    /// At least one statement failed, transaction rolled back
    Failed,

    /// Execution was rolled back (explicit or due to error)
    RolledBack,
}

impl SheetStatus {
    /// Get status as string for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::RolledBack => "rolled_back",
        }
    }
}

/// Per-statement execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementResult {
    /// Statement index
    pub index: usize,

    /// DAG depth
    pub dag_depth: usize,

    /// Original source
    pub source: String,

    /// Resolved source (with UUIDs)
    pub resolved_source: Option<String>,

    /// Final status
    pub status: StatementStatus,

    /// Error details (if failed)
    pub error: Option<StatementError>,

    /// Returned primary key (if any)
    pub returned_pk: Option<Uuid>,

    /// Execution time in milliseconds
    pub execution_time_ms: Option<u64>,
}

/// Detailed error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementError {
    /// Error code
    pub code: ErrorCode,

    /// Error message
    pub message: String,

    /// Additional detail
    pub detail: Option<String>,

    /// Source span for highlighting
    pub span: Option<SourceSpan>,

    /// Index of statement that blocked this one (if Blocked)
    pub blocked_by: Option<usize>,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_sheet() {
        let session_id = Uuid::new_v4();
        let sheet = DslSheet::new(session_id);
        assert_eq!(sheet.session_id, session_id);
        assert!(sheet.statements.is_empty());
        assert!(sheet.phases.is_empty());
    }

    #[test]
    fn test_statement_resolved_source() {
        let mut stmt = SessionDslStatement::new(0, "(entity.create :name @cbu)".to_string());
        stmt.resolved_args.insert(
            "cbu".to_string(),
            Uuid::parse_str("12345678-1234-1234-1234-123456789012").unwrap(),
        );

        let resolved = stmt.resolved_source().unwrap();
        assert!(resolved.contains("12345678-1234-1234-1234-123456789012"));
        assert!(!resolved.contains("@cbu"));
    }

    #[test]
    fn test_statement_unresolved_symbol() {
        let stmt = SessionDslStatement::new(0, "(entity.create :cbu @missing)".to_string());
        let result = stmt.resolved_source();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unresolved symbol"));
    }

    #[test]
    fn test_statement_status_terminal() {
        assert!(StatementStatus::Success.is_terminal());
        assert!(StatementStatus::failed("error", ErrorCode::SyntaxError).is_terminal());
        assert!(StatementStatus::skipped(0).is_terminal());
        assert!(!StatementStatus::Pending.is_terminal());
        assert!(!StatementStatus::Executing.is_terminal());
    }

    #[test]
    fn test_validation_result() {
        let result = ValidationResult::success();
        assert!(result.valid);
        assert!(!result.has_blocking_issues());

        let result = ValidationResult::failed(vec![ValidationError {
            statement_index: 0,
            message: "test".to_string(),
            span: None,
            code: ErrorCode::SyntaxError,
        }]);
        assert!(!result.valid);
        assert!(result.has_blocking_issues());
    }
}

//! Domain models for the DSL architecture
//!
//! This module defines the core data structures that represent DSL domains,
//! versions, and execution state in the database.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use uuid::Uuid;

/// DSL Domain representation
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslDomain {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub description: Option<String>,
    pub base_grammar_version: String,
    pub vocabulary_version: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// DSL Version representation with sequential versioning
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslVersion {
    pub version_id: Uuid,
    pub domain_id: Uuid,
    pub version_number: i32,
    pub functional_state: Option<String>,
    pub dsl_source_code: String,
    pub compilation_status: CompilationStatus,
    pub change_description: Option<String>,
    pub parent_version_id: Option<Uuid>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub compiled_at: Option<DateTime<Utc>>,
    pub activated_at: Option<DateTime<Utc>>,
}

/// Parsed AST storage with metadata
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ParsedAst {
    pub ast_id: Uuid,
    pub version_id: Uuid,
    pub ast_json: serde_json::Value,
    pub parse_metadata: Option<serde_json::Value>,
    pub grammar_version: String,
    pub parser_version: String,
    pub ast_hash: Option<String>,
    pub node_count: Option<i32>,
    pub complexity_score: Option<rust_decimal::Decimal>,
    pub parsed_at: DateTime<Utc>,
    pub invalidated_at: Option<DateTime<Utc>>,
}

/// DSL execution log entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslExecutionLog {
    pub execution_id: Uuid,
    pub version_id: Uuid,
    pub cbu_id: Option<String>,
    pub execution_phase: ExecutionPhase,
    pub status: ExecutionStatus,
    pub result_data: Option<serde_json::Value>,
    pub error_details: Option<serde_json::Value>,
    pub performance_metrics: Option<serde_json::Value>,
    pub executed_by: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,
}

/// Compilation status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "compilation_status", rename_all = "UPPERCASE")]
pub enum CompilationStatus {
    Draft,
    Compiling,
    Compiled,
    Active,
    Deprecated,
    Error,
}

/// Execution phase enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "execution_phase", rename_all = "UPPERCASE")]
pub enum ExecutionPhase {
    Parse,
    Compile,
    Validate,
    Execute,
    Complete,
}

/// Execution status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "execution_status", rename_all = "UPPERCASE")]
pub enum ExecutionStatus {
    Success,
    Failed,
    InProgress,
    Cancelled,
}

/// Request to create a new DSL version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDslVersion {
    pub domain_name: String,
    pub functional_state: Option<String>,
    pub dsl_source_code: String,
    pub change_description: Option<String>,
    pub parent_version_id: Option<Uuid>,
    pub created_by: Option<String>,
}

/// Request to create a new parsed AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewParsedAst {
    pub version_id: Uuid,
    pub ast_json: serde_json::Value,
    pub parse_metadata: Option<serde_json::Value>,
    pub grammar_version: String,
    pub parser_version: String,
    pub ast_hash: Option<String>,
    pub node_count: Option<i32>,
    pub complexity_score: Option<rust_decimal::Decimal>,
}

/// Latest version view representation
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslLatestVersion {
    pub domain_name: String,
    pub domain_description: Option<String>,
    pub version_id: Uuid,
    pub version_number: i32,
    pub functional_state: Option<String>,
    pub compilation_status: CompilationStatus,
    pub change_description: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub has_compiled_ast: bool,
}

/// Execution summary view representation
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslExecutionSummary {
    pub domain_name: String,
    pub version_number: i32,
    pub compilation_status: CompilationStatus,
    pub total_executions: Option<i64>,
    pub successful_executions: Option<i64>,
    pub failed_executions: Option<i64>,
    pub avg_duration_ms: Option<rust_decimal::Decimal>,
    pub last_execution_at: Option<DateTime<Utc>>,
}

/// Domain statistics for monitoring and reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainStatistics {
    pub domain_name: String,
    pub total_versions: i32,
    pub active_versions: i32,
    pub compiled_versions: i32,
    pub total_executions: i64,
    pub success_rate: f64,
    pub avg_compilation_time_ms: Option<i32>,
    pub avg_execution_time_ms: Option<i32>,
    pub last_activity: Option<DateTime<Utc>>,
}

/// Version history entry for change tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionHistoryEntry {
    pub version_number: i32,
    pub change_description: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub compilation_status: CompilationStatus,
    pub lines_added: Option<i32>,
    pub lines_deleted: Option<i32>,
    pub complexity_delta: Option<rust_decimal::Decimal>,
}

impl DslDomain {
    /// Check if domain is active and can accept new versions
    pub fn can_accept_versions(&self) -> bool {
        self.active
    }

    /// Get domain identifier for logging/display
    pub fn identifier(&self) -> String {
        format!("{}[{}]", self.domain_name, self.domain_id)
    }
}

impl DslVersion {
    /// Check if this version is ready for compilation
    pub fn can_compile(&self) -> bool {
        matches!(
            self.compilation_status,
            CompilationStatus::Draft | CompilationStatus::Error
        )
    }

    /// Check if this version is compiled and ready for execution
    pub fn can_execute(&self) -> bool {
        matches!(
            self.compilation_status,
            CompilationStatus::Compiled | CompilationStatus::Active
        )
    }

    /// Get version identifier for logging/display
    pub fn identifier(&self) -> String {
        format!("v{}[{}]", self.version_number, self.version_id)
    }

    /// Calculate age of this version
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }
}

impl ParsedAst {
    /// Check if AST is valid and not invalidated
    pub fn is_valid(&self) -> bool {
        self.invalidated_at.is_none()
    }

    /// Get AST age for cache management
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.parsed_at
    }

    /// Calculate AST size estimate in bytes
    pub fn estimated_size_bytes(&self) -> usize {
        serde_json::to_string(&self.ast_json)
            .map(|s| s.len())
            .unwrap_or(0)
    }
}

impl DslExecutionLog {
    /// Check if execution is still running
    pub fn is_running(&self) -> bool {
        matches!(self.status, ExecutionStatus::InProgress)
    }

    /// Check if execution completed successfully
    pub fn is_successful(&self) -> bool {
        matches!(self.status, ExecutionStatus::Success)
    }

    /// Get execution duration if completed
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.completed_at
            .map(|completed| completed - self.started_at)
    }
}

impl Default for CompilationStatus {
    fn default() -> Self {
        CompilationStatus::Draft
    }
}

impl std::fmt::Display for CompilationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilationStatus::Draft => write!(f, "DRAFT"),
            CompilationStatus::Compiling => write!(f, "COMPILING"),
            CompilationStatus::Compiled => write!(f, "COMPILED"),
            CompilationStatus::Active => write!(f, "ACTIVE"),
            CompilationStatus::Deprecated => write!(f, "DEPRECATED"),
            CompilationStatus::Error => write!(f, "ERROR"),
        }
    }
}

impl std::fmt::Display for ExecutionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionPhase::Parse => write!(f, "PARSE"),
            ExecutionPhase::Compile => write!(f, "COMPILE"),
            ExecutionPhase::Validate => write!(f, "VALIDATE"),
            ExecutionPhase::Execute => write!(f, "EXECUTE"),
            ExecutionPhase::Complete => write!(f, "COMPLETE"),
        }
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Success => write!(f, "SUCCESS"),
            ExecutionStatus::Failed => write!(f, "FAILED"),
            ExecutionStatus::InProgress => write!(f, "IN_PROGRESS"),
            ExecutionStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compilation_status_display() {
        assert_eq!(CompilationStatus::Draft.to_string(), "DRAFT");
        assert_eq!(CompilationStatus::Compiled.to_string(), "COMPILED");
    }

    #[test]
    fn test_dsl_version_can_compile() {
        let mut version = DslVersion {
            version_id: Uuid::new_v4(),
            domain_id: Uuid::new_v4(),
            version_number: 1,
            functional_state: None,
            dsl_source_code: "test".to_string(),
            compilation_status: CompilationStatus::Draft,
            change_description: None,
            parent_version_id: None,
            created_by: None,
            created_at: Utc::now(),
            compiled_at: None,
            activated_at: None,
        };

        assert!(version.can_compile());

        version.compilation_status = CompilationStatus::Compiled;
        assert!(!version.can_compile());
    }

    #[test]
    fn test_parsed_ast_validity() {
        let ast = ParsedAst {
            ast_id: Uuid::new_v4(),
            version_id: Uuid::new_v4(),
            ast_json: serde_json::json!({"type": "test"}),
            parse_metadata: None,
            grammar_version: "1.0.0".to_string(),
            parser_version: "1.0.0".to_string(),
            ast_hash: None,
            node_count: None,
            complexity_score: None,
            parsed_at: Utc::now(),
            invalidated_at: None,
        };

        assert!(ast.is_valid());
    }
}

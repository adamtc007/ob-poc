//! DSL Manager - Central Gateway for All DSL Operations
//!
//! This module provides the unified entry point for all DSL-related operations
//! including parsing, normalization (v3.3 → v3.1), AST building, validation,
//! compilation, state management, and backend integration.
//!
//! ## Architecture Principle
//! **ALL DSL operations MUST flow through the DSL Manager**
//!
//! The DSL Manager serves as the single source of truth for:
//! - DSL parsing with automatic v3.3 delta normalization
//! - AST construction and validation
//! - State change management and audit trails
//! - Backend database operations
//! - Compilation and execution pipelines
//!
//! ## Key Components
//! - `core`: Core DSL manager implementation
//! - `pipeline`: Processing pipeline stages
//! - `state`: DSL state management
//! - `backend`: Database and storage operations
//! - `validation`: Comprehensive validation engine
//! - `compiler`: DSL compilation and execution

pub mod backend;
pub mod compiler;
pub mod core;
pub mod pipeline;
pub mod state;
pub mod validation;

// Re-export main types
pub use backend::{BackendOperation, BackendResult, DslBackend};
pub use compiler::{CompilationResult, DslCompiler, ExecutionContext};
pub use core::{
    AgenticCrudRequest, AiOnboardingRequest, AiOnboardingResponse, AiValidationResult,
    CanonicalDslResponse, CbuGenerator, ComprehensiveHealthStatus, DslInstanceSummary, DslManager,
    DslManagerConfig, ExecutionDetails, HealthMetrics,
};
pub use pipeline::{DslPipeline, DslPipelineStage, PipelineResult};
pub use state::{DslState, DslStateManager, StateChangeEvent};
pub use validation::{DslValidationEngine, ValidationLevel, ValidationReport};

/// DSL Manager error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum DslManagerError {
    #[error("Parsing failed: {message}")]
    ParsingError { message: String },

    #[error("Normalization failed: {message}")]
    NormalizationError { message: String },

    #[error("AST validation failed: {message}")]
    AstValidationError { message: String },

    #[error("Compilation failed: {message}")]
    CompilationError { message: String },

    #[error("State management error: {message}")]
    StateError { message: String },

    #[error("Backend operation failed: {message}")]
    BackendError { message: String },

    #[error("Pipeline stage '{stage}' failed: {reason}")]
    PipelineError { stage: String, reason: String },

    #[error("Invalid DSL version: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },

    #[error("Unauthorized operation: {operation}")]
    UnauthorizedOperation { operation: String },

    #[error("Resource not found: {resource_id}")]
    ResourceNotFound { resource_id: String },

    #[error("Concurrency conflict: {details}")]
    ConcurrencyConflict { details: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
}

/// Result type for DSL Manager operations
pub type DslManagerResult<T> = Result<T, DslManagerError>;

/// Operation types for DSL Manager operations
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    Create,
    Read,
    Update,
    Delete,
}

/// DSL operation types that flow through the manager
#[derive(Debug, Clone, PartialEq)]
pub enum DslOperation {
    /// Parse raw DSL text into AST
    Parse {
        dsl_text: String,
        apply_normalization: bool,
    },

    /// Validate parsed AST
    Validate {
        ast: crate::Program,
        validation_level: ValidationLevel,
    },

    /// Compile AST to executable form
    Compile {
        ast: crate::Program,
        execution_context: ExecutionContext,
    },

    /// Execute compiled DSL
    Execute {
        compiled_dsl: CompilationResult,
        dry_run: bool,
    },

    /// Create new DSL instance
    CreateInstance {
        initial_dsl: String,
        domain: String,
        metadata: std::collections::HashMap<String, String>,
    },

    /// Update existing DSL instance
    UpdateInstance {
        instance_id: uuid::Uuid,
        dsl_increment: String,
        change_description: Option<String>,
    },

    /// Query DSL state
    QueryState {
        instance_id: uuid::Uuid,
        version: Option<u64>,
    },

    /// Get DSL history
    GetHistory {
        instance_id: uuid::Uuid,
        limit: Option<u64>,
    },

    /// Rollback to previous version
    Rollback {
        instance_id: uuid::Uuid,
        target_version: u64,
    },

    /// Batch operations
    Batch {
        operations: Vec<DslOperation>,
        transaction_mode: TransactionMode,
    },
}

/// Transaction modes for batch operations
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionMode {
    /// All operations must succeed or all fail
    Atomic,
    /// Continue processing even if some operations fail
    Sequential,
    /// Validate all operations without execution
    DryRun,
}

/// DSL processing context
#[derive(Debug, Clone)]
pub struct DslContext {
    /// Request ID for tracking
    pub request_id: String,
    /// User performing the operation
    pub user_id: String,
    /// Domain context
    pub domain: String,
    /// Processing options
    pub options: DslProcessingOptions,
    /// Audit metadata
    pub audit_metadata: std::collections::HashMap<String, String>,
}

/// DSL Manager factory for creating configured instances
pub struct DslManagerFactory;

impl DslManagerFactory {
    /// Create a new DSL Manager with default configuration
    pub fn new() -> DslManager {
        DslManager::new(DslManagerConfig::default())
    }

    /// Create a DSL Manager with custom configuration
    pub fn with_config(config: DslManagerConfig) -> DslManager {
        DslManager::new(config)
    }

    /// Create a DSL Manager with database backend
    #[cfg(feature = "database")]
    pub async fn with_database(
        config: DslManagerConfig,
        database_url: &str,
    ) -> DslManagerResult<DslManager> {
        use crate::database::DatabaseManager;

        let db_config = crate::database::DatabaseConfig {
            database_url: database_url.to_string(),
            max_connections: 10,
            connection_timeout: std::time::Duration::from_secs(30),
            idle_timeout: Some(std::time::Duration::from_secs(600)),
            max_lifetime: Some(std::time::Duration::from_secs(1800)),
        };
        let db_manager =
            DatabaseManager::new(db_config)
                .await
                .map_err(|e| DslManagerError::BackendError {
                    message: format!("Database connection failed: {}", e),
                })?;

        let mut manager = DslManager::new(config);
        manager.set_backend(Box::new(backend::DatabaseBackend::new(db_manager)));
        Ok(manager)
    }

    /// Create a DSL Manager for testing with mock backend
    pub fn for_testing() -> DslManager {
        let mut config = DslManagerConfig::default();
        config.enable_strict_validation = true;
        config.enable_metrics = true;

        let mut manager = DslManager::new(config);
        manager.set_backend(Box::new(backend::MockBackend::new()));
        manager
    }
}

impl Default for DslContext {
    fn default() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id: "default".to_string(),
            domain: "default".to_string(),
            options: DslProcessingOptions::default(),
            audit_metadata: std::collections::HashMap::new(),
        }
    }
}

/// DSL processing options
#[derive(Debug, Clone)]
pub struct DslProcessingOptions {
    /// Apply v3.3 → v3.1 normalization
    pub apply_normalization: bool,
    /// Validation level
    pub validation_level: ValidationLevel,
    /// Enable detailed error reporting
    pub detailed_errors: bool,
    /// Enable performance metrics
    pub enable_metrics: bool,
    /// Timeout for operations (in seconds)
    pub timeout_seconds: Option<u64>,
}

impl Default for DslProcessingOptions {
    fn default() -> Self {
        Self {
            apply_normalization: true, // Always apply v3.3 delta by default
            validation_level: ValidationLevel::Standard,
            detailed_errors: true,
            enable_metrics: false,
            timeout_seconds: Some(30),
        }
    }
}

/// Unified DSL processing result
#[derive(Debug)]
pub struct DslProcessingResult {
    /// Processing success status
    pub success: bool,
    /// Parsed and normalized AST (if successful)
    pub ast: Option<crate::Program>,
    /// Validation report
    pub validation_report: ValidationReport,
    /// Compilation result (if compiled)
    pub compilation_result: Option<CompilationResult>,
    /// Execution result (if executed)
    pub execution_result: Option<BackendResult>,
    /// Processing metrics
    pub metrics: ProcessingMetrics,
    /// Any errors encountered
    pub errors: Vec<DslManagerError>,
    /// Warnings
    pub warnings: Vec<String>,
}

/// Processing performance metrics
#[derive(Debug, Default)]
pub struct ProcessingMetrics {
    /// Total processing time in milliseconds
    pub total_time_ms: u64,
    /// Parsing time
    pub parse_time_ms: u64,
    /// Normalization time
    pub normalization_time_ms: u64,
    /// Validation time
    pub validation_time_ms: u64,
    /// Compilation time
    pub compilation_time_ms: u64,
    /// Execution time
    pub execution_time_ms: u64,
    /// Backend operation time
    pub backend_time_ms: u64,
}

// DslManagerFactory moved to avoid duplication - using the one defined above at line 189

/// Convenience macros for DSL Manager operations
#[macro_export]
macro_rules! dsl_parse {
    ($manager:expr, $dsl:expr) => {
        $manager.process_operation(
            $crate::dsl_manager::DslOperation::Parse {
                dsl_text: $dsl.to_string(),
                apply_normalization: true,
            },
            $crate::dsl_manager::DslContext::default(),
        )
    };
}

#[macro_export]
macro_rules! dsl_execute {
    ($manager:expr, $dsl:expr, $context:expr) => {
        $manager.execute_dsl($dsl, $context)
    };
}

/// Integration tests for DSL Manager
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_dsl_manager_v33_normalization() {
        let manager = DslManagerFactory::for_testing();

        // Test v3.3 legacy syntax gets normalized to v3.1
        let legacy_dsl = r#"
            (kyc.start_case :case_id "test-001" :case_type "KYC_CASE")
            (ubo.link_ownership :from_entity "P1" :to_entity "E1" :percent 100.0)
        "#;

        let context = DslContext::default();
        let result = manager
            .process_operation(
                DslOperation::Parse {
                    dsl_text: legacy_dsl.to_string(),
                    apply_normalization: true,
                },
                context,
            )
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.ast.is_some());

        // Verify normalization occurred
        let _ast = result.ast.unwrap();
        // Should contain canonical forms: case.create, entity.link
        // TODO: Add specific AST verification
    }

    #[tokio::test]
    async fn test_agentic_crud_integration() {
        let manager = DslManagerFactory::for_testing();

        // Test agentic CRUD DSL processing
        let crud_dsl = r#"(data.create :asset "cbu" :values {:name "Test CBU"})"#;

        let context = DslContext::default();
        let result = manager.execute_dsl(crud_dsl, context).await;

        assert!(result.is_ok());
        // Verify it goes through proper pipeline
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let manager = DslManagerFactory::for_testing();

        let batch_op = DslOperation::Batch {
            operations: vec![
                DslOperation::Parse {
                    dsl_text: "(case.create :case-id \"test\")".to_string(),
                    apply_normalization: true,
                },
                DslOperation::Parse {
                    dsl_text: "(entity.register :entity-id \"E1\")".to_string(),
                    apply_normalization: true,
                },
            ],
            transaction_mode: TransactionMode::Atomic,
        };

        let result = manager
            .process_operation(batch_op, DslContext::default())
            .await;
        assert!(result.is_ok());
    }
}

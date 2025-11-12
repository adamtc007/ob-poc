//! Enhanced error handling for DSL operations
//!
//! Provides comprehensive error types for database operations, document processing,
//! and DSL management with proper serialization support.

use thiserror::Error;

/// Database operation errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum DatabaseError {
    #[error("Connection failed: {message}")]
    ConnectionError { message: String },

    #[error("Query failed: {message}")]
    QueryError { message: String },

    #[error("Transaction failed: {message}")]
    TransactionError { message: String },

    #[error("Migration failed: {message}")]
    MigrationError { message: String },

    #[error("Constraint violation: {message}")]
    ConstraintViolation { message: String },

    #[error("Record not found: {message}")]
    NotFound { message: String },

    #[error("Duplicate record: {message}")]
    Duplicate { message: String },

    #[error("Timeout: {message}")]
    Timeout { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("SQLX error: {0}")]
    SqlxError(String),
}

impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        DatabaseError::SqlxError(err.to_string())
    }
}

/// Document processing errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub(crate) enum DocumentProcessingError {
    #[error("Invalid document type: {document_type}")]
    InvalidDocumentType { document_type: String },

    #[error("Extraction failed for attribute {attribute}: {reason}")]
    ExtractionFailed { attribute: String, reason: String },

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Cross-document validation failed for {attribute}: inconsistent values")]
    CrossValidationFailed { attribute: String },

    #[error("Required attribute missing: {attribute}")]
    MissingAttribute { attribute: String },

    #[error("AI processing failed: {reason}")]
    AiProcessingFailed { reason: String },

    #[error("Template not found: {template_id}")]
    TemplateNotFound { template_id: String },

    #[error("Content parsing failed: {reason}")]
    ContentParsingFailed { reason: String },

    #[error("File operation failed: {operation}")]
    FileOperationFailed { operation: String },

    #[error("Database error occurred")]
    DatabaseError,
}

/// DSL manager specific errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum DslManagerError {
    #[error("DSL compilation failed: {reason}")]
    CompilationFailed { reason: String },

    #[error("DSL instance not found: {instance_id}")]
    InstanceNotFound { instance_id: String },

    #[error("DSL version not found: {version_id}")]
    VersionNotFound { version_id: String },

    #[error("Invalid DSL syntax: {message}")]
    InvalidSyntax { message: String },

    #[error("Template generation failed: {template}")]
    TemplateGenerationFailed { template: String },

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Workflow execution failed: {stage}")]
    WorkflowExecutionFailed { stage: String },

    #[error("State transition invalid: from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Resource conflict: {resource}")]
    ResourceConflict { resource: String },

    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },

    #[error("External service error: {service}")]
    ExternalServiceError { service: String },

    #[error("Timeout: operation {operation} exceeded {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Database error occurred")]
    DatabaseError,

    #[error("Document processing error occurred")]
    DocumentProcessingError,
}

impl From<DatabaseError> for DslManagerError {
    fn from(_err: DatabaseError) -> Self {
        DslManagerError::DatabaseError
    }
}

impl From<DocumentProcessingError> for DslManagerError {
    fn from(_err: DocumentProcessingError) -> Self {
        DslManagerError::DocumentProcessingError
    }
}

/// Consolidated error type for all operations
#[derive(Error, Debug)]
pub(crate) enum EnhancedError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Document processing error: {0}")]
    DocumentProcessing(#[from] DocumentProcessingError),

    #[error("DSL manager error: {0}")]
    DslManager(#[from] DslManagerError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Result type aliases for convenience
pub(crate) type DatabaseResult<T> = Result<T, DatabaseError>;
pub type DocumentResult<T> = Result<T, DocumentProcessingError>;
pub type DslManagerResult<T> = Result<T, DslManagerError>;
pub(crate) type EnhancedResult<T> = Result<T, EnhancedError>;

/// Helper functions for error creation
impl DatabaseError {
    pub(crate) fn connection_failed(message: impl Into<String>) -> Self {
        Self::ConnectionError {
            message: message.into(),
        }
    }

    pub(crate) fn query_failed(message: impl Into<String>) -> Self {
        Self::QueryError {
            message: message.into(),
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
        }
    }

    pub(crate) fn constraint_violation(message: impl Into<String>) -> Self {
        Self::ConstraintViolation {
            message: message.into(),
        }
    }
}

impl DocumentProcessingError {
    pub(crate) fn invalid_document_type(document_type: impl Into<String>) -> Self {
        Self::InvalidDocumentType {
            document_type: document_type.into(),
        }
    }

    pub(crate) fn extraction_failed(attribute: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ExtractionFailed {
            attribute: attribute.into(),
            reason: reason.into(),
        }
    }

    pub(crate) fn missing_attribute(attribute: impl Into<String>) -> Self {
        Self::MissingAttribute {
            attribute: attribute.into(),
        }
    }

    pub(crate) fn ai_processing_failed(reason: impl Into<String>) -> Self {
        Self::AiProcessingFailed {
            reason: reason.into(),
        }
    }
}

impl DslManagerError {
    pub(crate) fn compilation_failed(reason: impl Into<String>) -> Self {
        Self::CompilationFailed {
            reason: reason.into(),
        }
    }

    pub(crate) fn instance_not_found(instance_id: impl Into<String>) -> Self {
        Self::InstanceNotFound {
            instance_id: instance_id.into(),
        }
    }

    pub(crate) fn invalid_syntax(message: impl Into<String>) -> Self {
        Self::InvalidSyntax {
            message: message.into(),
        }
    }

    pub(crate) fn workflow_execution_failed(stage: impl Into<String>) -> Self {
        Self::WorkflowExecutionFailed {
            stage: stage.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_error_creation() {
        let err = DatabaseError::connection_failed("Connection timeout");
        assert!(matches!(err, DatabaseError::ConnectionError { .. }));
    }

    #[test]
    fn test_document_error_creation() {
        let err = DocumentProcessingError::invalid_document_type("passport");
        assert!(matches!(
            err,
            DocumentProcessingError::InvalidDocumentType { .. }
        ));
    }

    #[test]
    fn test_dsl_manager_error_creation() {
        let err = DslManagerError::compilation_failed("Parse error");
        assert!(matches!(err, DslManagerError::CompilationFailed { .. }));
    }

    #[test]
    fn test_error_conversion() {
        let db_err = DatabaseError::connection_failed("Test");
        let manager_err: DslManagerError = db_err.into();
        assert!(matches!(manager_err, DslManagerError::DatabaseError));
    }
}

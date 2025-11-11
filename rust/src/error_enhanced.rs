//! Enhanced error handling for DSL system with document-attribute bridge
//!
//! This module extends the error system to support document processing,
//! database operations, and the enhanced DSL manager functionality.

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

    #[error("Constraint violation: {constraint}")]
    ConstraintViolation { constraint: String },

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Duplicate entity: {entity_type} with key {key}")]
    Duplicate { entity_type: String, key: String },

    #[error("Migration failed: {version}")]
    MigrationFailed { version: i32 },

    #[error("Serialization error: {message}")]
    SerializationError(String),

    #[error("SQLX error: {0}")]
    SqlxError(#[from] sqlx::Error),
}

/// Document processing errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum DocumentProcessingError {
    #[error("Document type not supported: {document_type}")]
    UnsupportedDocumentType { document_type: String },

    #[error("Extraction failed for attribute {attribute}: {reason}")]
    ExtractionFailed { attribute: String, reason: String },

    #[error("Validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Cross-document validation failed for {attribute}: inconsistent values")]
    CrossValidationFailed { attribute: String },

    #[error("Required attribute missing: {attribute}")]
    RequiredAttributeMissing { attribute: String },

    #[error("Invalid document format: {reason}")]
    InvalidFormat { reason: String },

    #[error("Document processing timeout after {duration_ms}ms")]
    ProcessingTimeout { duration_ms: u64 },

    #[error("AI extraction service unavailable")]
    ExtractionServiceUnavailable,

    #[error("Insufficient confidence score: {score} < {threshold}")]
    InsufficientConfidence { score: f64, threshold: f64 },

    #[error("Privacy classification violation: attempted to extract {classification} data")]
    PrivacyViolation { classification: String },
}

/// Enhanced DSL manager errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum DslManagerError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error("Document processing error: {0}")]
    DocumentProcessingError(#[from] DocumentProcessingError),

    #[error("Validation error: {message}")]
    ValidationError(String),

    #[error("Configuration error: {message}")]
    ConfigurationError(String),

    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },

    #[error("Template not found: {template_id}")]
    TemplateNotFound { template_id: String },

    #[error("Attribute mapping not found: {document_type} -> {attribute}")]
    AttributeMappingNotFound {
        document_type: String,
        attribute: String,
    },

    #[error("DSL generation failed: {reason}")]
    DslGenerationFailed { reason: String },

    #[error("Cross-document consistency check failed")]
    ConsistencyCheckFailed,

    #[error("Unauthorized operation: {operation}")]
    Unauthorized { operation: String },

    #[error("Rate limit exceeded: {limit} requests per {window}")]
    RateLimitExceeded { limit: u32, window: String },

    #[error("Internal error: {message}")]
    InternalError(String),
}

/// Attribute validation errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum AttributeValidationError {
    #[error("Invalid attribute value for {attribute_code}: {message}")]
    InvalidValue {
        attribute_code: String,
        message: String,
    },

    #[error("Attribute format validation failed for {attribute_code}: expected {expected_format}")]
    FormatValidation {
        attribute_code: String,
        expected_format: String,
    },

    #[error(
        "Privacy classification mismatch for {attribute_code}: expected {expected}, found {found}"
    )]
    PrivacyClassification {
        attribute_code: String,
        expected: String,
        found: String,
    },

    #[error("Cross-reference validation failed for {attribute_code}: {reason}")]
    CrossReferenceValidation {
        attribute_code: String,
        reason: String,
    },

    #[error("Business rule violation for {attribute_code}: {rule}")]
    BusinessRuleViolation {
        attribute_code: String,
        rule: String,
    },

    #[error("Regulatory compliance violation for {attribute_code}: {regulation}")]
    ComplianceViolation {
        attribute_code: String,
        regulation: String,
    },
}

/// Extraction errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum ExtractionError {
    #[error("Pattern matching failed: no patterns matched for {attribute}")]
    PatternMatchingFailed { attribute: String },

    #[error("AI extraction failed: {reason}")]
    AiExtractionFailed { reason: String },

    #[error("OCR processing failed: {reason}")]
    OcrFailed { reason: String },

    #[error("Text preprocessing failed: {reason}")]
    PreprocessingFailed { reason: String },

    #[error("Confidence score too low: {score} < {threshold}")]
    LowConfidence { score: f64, threshold: f64 },

    #[error("Multiple conflicting values found for {attribute}")]
    ConflictingValues { attribute: String },

    #[error(
        "Data type conversion failed for {attribute}: cannot convert {value} to {target_type}"
    )]
    TypeConversionFailed {
        attribute: String,
        value: String,
        target_type: String,
    },

    #[error("Field location not found: {field_hint}")]
    FieldLocationNotFound { field_hint: String },
}

/// Cross-document validation errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum CrossDocumentError {
    #[error("Inconsistent values across documents for {attribute}: {details}")]
    InconsistentValues { attribute: String, details: String },

    #[error("Missing reference document: {document_type} required for {attribute}")]
    MissingReferenceDocument {
        document_type: String,
        attribute: String,
    },

    #[error("Validation rule not found for {attribute}")]
    ValidationRuleNotFound { attribute: String },

    #[error("Entity identifier mismatch: {expected} != {found}")]
    EntityMismatch { expected: String, found: String },

    #[error("Temporal inconsistency: {attribute} values have incompatible dates")]
    TemporalInconsistency { attribute: String },

    #[error("Jurisdiction conflict: {attribute} values conflict across jurisdictions")]
    JurisdictionConflict { attribute: String },
}

/// Template processing errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum TemplateError {
    #[error("Template not found: {template_id}")]
    TemplateNotFound { template_id: String },

    #[error("Template compilation failed: {reason}")]
    CompilationFailed { reason: String },

    #[error("Template variable not found: {variable}")]
    VariableNotFound { variable: String },

    #[error("Template syntax error: {message} at line {line}")]
    SyntaxError { message: String, line: u32 },

    #[error("Template rendering failed: {reason}")]
    RenderingFailed { reason: String },

    #[error("Circular template dependency detected: {templates:?}")]
    CircularDependency { templates: Vec<String> },

    #[error("Template version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },
}

/// Repository operation errors
#[derive(Error, Debug, Clone, serde::Serialize)]
pub enum RepositoryError {
    #[error("Connection pool exhausted")]
    ConnectionPoolExhausted,

    #[error("Transaction timeout after {timeout_ms}ms")]
    TransactionTimeout { timeout_ms: u64 },

    #[error("Optimistic locking failed: entity was modified by another transaction")]
    OptimisticLockingFailed,

    #[error("Foreign key constraint violation: {constraint}")]
    ForeignKeyViolation { constraint: String },

    #[error("Unique constraint violation: {field}")]
    UniqueConstraintViolation { field: String },

    #[error("Invalid query parameters: {reason}")]
    InvalidQueryParameters { reason: String },

    #[error("Batch operation failed: {failed_count}/{total_count} operations failed")]
    BatchOperationFailed { failed_count: u32, total_count: u32 },

    #[error("Schema validation failed: {reason}")]
    SchemaValidationFailed { reason: String },
}

/// Result type aliases for enhanced error handling
pub type DatabaseResult<T> = Result<T, DatabaseError>;
pub type DocumentProcessingResult<T> = Result<T, DocumentProcessingError>;
pub type DslManagerResult<T> = Result<T, DslManagerError>;
pub type AttributeValidationResult<T> = Result<T, AttributeValidationError>;
pub type ExtractionResult<T> = Result<T, ExtractionError>;
pub type CrossDocumentResult<T> = Result<T, CrossDocumentError>;
pub type TemplateResult<T> = Result<T, TemplateError>;
pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Error severity levels for enhanced error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ErrorSeverity {
    /// Informational - operation succeeded with notes
    Info,
    /// Warning - operation succeeded but with concerns
    Warning,
    /// Error - operation failed but recoverable
    Error,
    /// Critical - operation failed with system impact
    Critical,
    /// Fatal - operation failed with unrecoverable state
    Fatal,
}

/// Enhanced error context with additional metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnhancedErrorContext {
    pub error_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub severity: ErrorSeverity,
    pub operation: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub user_id: Option<String>,
    pub correlation_id: Option<String>,
    pub additional_context: std::collections::HashMap<String, serde_json::Value>,
}

impl EnhancedErrorContext {
    pub fn new(operation: String, severity: ErrorSeverity) -> Self {
        Self {
            error_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            severity,
            operation,
            entity_type: None,
            entity_id: None,
            user_id: None,
            correlation_id: None,
            additional_context: std::collections::HashMap::new(),
        }
    }

    pub fn with_entity(mut self, entity_type: String, entity_id: String) -> Self {
        self.entity_type = Some(entity_type);
        self.entity_id = Some(entity_id);
        self
    }

    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_context(mut self, key: String, value: serde_json::Value) -> Self {
        self.additional_context.insert(key, value);
        self
    }
}

/// Contextual error wrapper for enhanced error reporting
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextualError<T> {
    pub error: T,
    pub context: EnhancedErrorContext,
    pub chain: Vec<String>,
}

impl<T> ContextualError<T> {
    pub fn new(error: T, context: EnhancedErrorContext) -> Self {
        Self {
            error,
            context,
            chain: Vec::new(),
        }
    }

    pub fn with_chain_error(mut self, error_message: String) -> Self {
        self.chain.push(error_message);
        self
    }
}

impl<T: std::fmt::Display> std::fmt::Display for ContextualError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (operation: {})", self.error, self.context.operation)?;
        if !self.chain.is_empty() {
            write!(f, " | Chain: {}", self.chain.join(" -> "))?;
        }
        Ok(())
    }
}

impl<T: std::error::Error + 'static> std::error::Error for ContextualError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Trait for adding enhanced error context
pub trait WithEnhancedContext<T> {
    fn with_context(self, operation: String, severity: ErrorSeverity) -> ContextualError<T>;
    fn with_entity_context(
        self,
        operation: String,
        severity: ErrorSeverity,
        entity_type: String,
        entity_id: String,
    ) -> ContextualError<T>;
}

impl<T, E> WithEnhancedContext<E> for Result<T, E> {
    fn with_context(self, operation: String, severity: ErrorSeverity) -> ContextualError<E> {
        match self {
            Ok(_) => panic!("Cannot add context to successful result"),
            Err(error) => {
                let context = EnhancedErrorContext::new(operation, severity);
                ContextualError::new(error, context)
            }
        }
    }

    fn with_entity_context(
        self,
        operation: String,
        severity: ErrorSeverity,
        entity_type: String,
        entity_id: String,
    ) -> ContextualError<E> {
        match self {
            Ok(_) => panic!("Cannot add context to successful result"),
            Err(error) => {
                let context = EnhancedErrorContext::new(operation, severity)
                    .with_entity(entity_type, entity_id);
                ContextualError::new(error, context)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_serialization() {
        let error = DatabaseError::ConnectionError {
            message: "Connection failed".to_string(),
        };

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("Connection failed"));
    }

    #[test]
    fn test_contextual_error() {
        let error = DocumentProcessingError::ExtractionFailed {
            attribute: "entity.legal_name".to_string(),
            reason: "Pattern not found".to_string(),
        };

        let context =
            EnhancedErrorContext::new("document_processing".to_string(), ErrorSeverity::Error)
                .with_entity("document".to_string(), "doc-123".to_string());

        let contextual = ContextualError::new(error, context);
        assert_eq!(contextual.context.entity_id, Some("doc-123".to_string()));
    }

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Fatal > ErrorSeverity::Critical);
        assert!(ErrorSeverity::Critical > ErrorSeverity::Error);
        assert!(ErrorSeverity::Error > ErrorSeverity::Warning);
        assert!(ErrorSeverity::Warning > ErrorSeverity::Info);
    }

    #[test]
    fn test_enhanced_context_builder() {
        let context =
            EnhancedErrorContext::new("test_operation".to_string(), ErrorSeverity::Warning)
                .with_user("user-123".to_string())
                .with_correlation_id("corr-456".to_string())
                .with_context("key".to_string(), serde_json::json!("value"));

        assert_eq!(context.user_id, Some("user-123".to_string()));
        assert_eq!(context.correlation_id, Some("corr-456".to_string()));
        assert_eq!(context.additional_context.len(), 1);
    }
}

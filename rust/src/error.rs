//! Comprehensive error handling for the UBO DSL system
//!
//! This module provides idiomatic Rust error types using thiserror for
//! better error messages and proper error chain handling.

use std::fmt;

use nom::error::VerboseError;
use thiserror::Error;

/// Main error type for the DSL system
#[derive(Error, Debug)]
pub enum DSLError {
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Grammar error: {0}")]
    Grammar(#[from] GrammarError),

    #[error("Vocabulary error: {0}")]
    Vocabulary(#[from] VocabularyError),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Runtime error: {0}")]
    Runtime(#[from] RuntimeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Parse errors from nom-based parsers
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Syntax error at position {position}: {message}")]
    Syntax { position: usize, message: String },

    #[error("Unexpected token '{token}' at position {position}, expected {expected}")]
    UnexpectedToken {
        token: String,
        position: usize,
        expected: String,
    },

    #[error("Incomplete input: expected more content")]
    Incomplete,

    #[error("Invalid character encoding")]
    Encoding,

    #[error("Parser internal error: {message}")]
    Internal { message: String },
}

impl From<VerboseError<&str>> for ParseError {
    fn from(error: VerboseError<&str>) -> Self {
        // Convert nom's VerboseError to our ParseError
        if let Some((input, _)) = error.errors.first() {
            let position = input.as_ptr() as usize; // Rough approximation
            let message = format!("Parsing failed: {:?}", error);
            ParseError::Syntax { position, message }
        } else {
            ParseError::Internal {
                message: "Unknown parsing error".to_string(),
            }
        }
    }
}

impl From<String> for ParseError {
    fn from(message: String) -> Self {
        ParseError::Internal { message }
    }
}

/// Grammar-related errors
#[derive(Error, Debug)]
pub enum GrammarError {
    #[error("Rule '{rule}' not found")]
    RuleNotFound { rule: String },

    #[error("Circular dependency detected: {chain}")]
    CircularDependency { chain: String },

    #[error("Invalid rule definition for '{rule}': {reason}")]
    InvalidRule { rule: String, reason: String },

    #[error("Grammar compilation failed: {message}")]
    CompilationError { message: String },

    #[error("Undefined non-terminal '{name}' referenced in rule '{rule}'")]
    UndefinedReference { name: String, rule: String },

    #[error("Ambiguous grammar: multiple interpretations possible")]
    Ambiguous,
}

/// Vocabulary and domain-specific errors
#[derive(Error, Debug)]
pub enum VocabularyError {
    #[error("Unknown verb '{verb}' in domain '{domain}'")]
    UnknownVerb { verb: String, domain: String },

    #[error("Invalid verb signature for '{verb}': expected {expected}, found {found}")]
    InvalidSignature {
        verb: String,
        expected: String,
        found: String,
    },

    #[error("Invalid verb format for '{verb}': expected {expected}")]
    InvalidVerbFormat { verb: String, expected: String },

    #[error("Verb conflict: '{verb}' already exists in domain '{existing_domain}', cannot register in '{new_domain}'")]
    VerbConflict {
        verb: String,
        existing_domain: String,
        new_domain: String,
    },

    #[error("Verb '{verb}' not found")]
    VerbNotFound { verb: String },

    #[error("Domain '{domain}' not registered")]
    DomainNotFound { domain: String },

    #[error("Vocabulary validation failed: {message}")]
    ValidationFailed { message: String },

    #[error(
        "Version conflict: verb '{verb}' requires version {required}, but {found} is available"
    )]
    VersionConflict {
        verb: String,
        required: String,
        found: String,
    },
}

/// Semantic validation errors
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Type mismatch: expected {expected}, found {found} at {location}")]
    TypeMismatch {
        expected: String,
        found: String,
        location: String,
    },

    #[error("Undefined variable '{name}' at {location}")]
    UndefinedVariable { name: String, location: String },

    #[error("Variable '{name}' already defined in this scope at {location}")]
    DuplicateVariable { name: String, location: String },

    #[error("Invalid operation '{operation}' for type {type_name}")]
    InvalidOperation {
        operation: String,
        type_name: String,
    },

    #[error("Constraint violation: {constraint} at {location}")]
    ConstraintViolation {
        constraint: String,
        location: String,
    },

    #[error("Missing required property '{property}' for {entity_type}")]
    MissingProperty {
        property: String,
        entity_type: String,
    },

    #[error("Workflow validation failed: {message}")]
    WorkflowError { message: String },
}

/// Runtime execution errors
#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Execution failed at statement {statement}: {message}")]
    ExecutionFailed { statement: String, message: String },

    #[error("Resource not available: {resource}")]
    ResourceUnavailable { resource: String },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("Network error: {message}")]
    Network { message: String },

    #[error("Timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },
}

/// Result type aliases for convenience
pub type DSLResult<T> = Result<T, DSLError>;
pub type ParseResult<T> = Result<T, ParseError>;
pub type GrammarResult<T> = Result<T, GrammarError>;
pub type VocabularyResult<T> = Result<T, VocabularyError>;
pub type ValidationResult<T> = Result<T, ValidationError>;
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Source location information for errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: Option<String>,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl SourceLocation {
    pub fn new(file: Option<String>, line: usize, column: usize, offset: usize) -> Self {
        Self {
            file,
            line,
            column,
            offset,
        }
    }

    pub fn unknown() -> Self {
        Self {
            file: None,
            line: 0,
            column: 0,
            offset: 0,
        }
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.file {
            Some(file) => write!(f, "{}:{}:{}", file, self.line, self.column),
            None => write!(f, "{}:{}", self.line, self.column),
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Info => write!(f, "INFO"),
            ErrorSeverity::Warning => write!(f, "WARNING"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Structured error with additional context
#[derive(Debug)]
pub struct ContextualError {
    pub error: DSLError,
    pub location: SourceLocation,
    pub severity: ErrorSeverity,
    pub context: Vec<String>,
}

impl ContextualError {
    pub fn new(error: DSLError, location: SourceLocation, severity: ErrorSeverity) -> Self {
        Self {
            error,
            location,
            severity,
            context: Vec::new(),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context.push(context.into());
        self
    }

    pub fn add_context(&mut self, context: impl Into<String>) {
        self.context.push(context.into());
    }
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[{}] {} at {}", self.severity, self.error, self.location)?;

        for (i, ctx) in self.context.iter().enumerate() {
            writeln!(f, "  {}: {}", i + 1, ctx)?;
        }

        Ok(())
    }
}

impl std::error::Error for ContextualError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Error collector for batch validation
#[derive(Debug, Default)]
pub struct ErrorCollector {
    pub errors: Vec<ContextualError>,
}

impl ErrorCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_error(&mut self, error: ContextualError) {
        self.errors.push(error);
    }

    pub fn add_simple_error(
        &mut self,
        error: DSLError,
        location: SourceLocation,
        severity: ErrorSeverity,
    ) {
        self.errors
            .push(ContextualError::new(error, location, severity));
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_fatal_errors(&self) -> bool {
        self.errors
            .iter()
            .any(|e| e.severity == ErrorSeverity::Fatal)
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn warning_count(&self) -> usize {
        self.errors
            .iter()
            .filter(|e| e.severity == ErrorSeverity::Warning)
            .count()
    }

    pub fn fatal_error_count(&self) -> usize {
        self.errors
            .iter()
            .filter(|e| e.severity == ErrorSeverity::Fatal)
            .count()
    }

    pub fn into_result<T>(self, value: T) -> Result<T, Vec<ContextualError>> {
        if self.has_errors() {
            Err(self.errors)
        } else {
            Ok(value)
        }
    }

    pub fn clear(&mut self) {
        self.errors.clear();
    }
}

impl fmt::Display for ErrorCollector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.errors.is_empty() {
            writeln!(f, "No errors")?;
        } else {
            writeln!(f, "Found {} error(s):", self.errors.len())?;
            for (i, error) in self.errors.iter().enumerate() {
                writeln!(f, "{}. {}", i + 1, error)?;
            }
        }
        Ok(())
    }
}

/// Helper macros for error construction
#[macro_export]
macro_rules! parse_error {
    ($msg:expr) => {
        $crate::error::ParseError::Internal {
            message: $msg.to_string(),
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ParseError::Internal {
            message: format!($fmt, $($arg)*),
        }
    };
}

#[macro_export]
macro_rules! grammar_error {
    ($variant:ident { $($field:ident: $value:expr),* $(,)? }) => {
        $crate::error::GrammarError::$variant { $($field: $value),* }
    };
}

#[macro_export]
macro_rules! validation_error {
    ($variant:ident { $($field:ident: $value:expr),* $(,)? }) => {
        $crate::error::ValidationError::$variant { $($field: $value),* }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_construction() {
        let parse_err = ParseError::Syntax {
            position: 10,
            message: "Expected identifier".to_string(),
        };

        let dsl_err = DSLError::Parse(parse_err);
        assert!(matches!(dsl_err, DSLError::Parse(_)));
    }

    #[test]
    fn test_error_collector() {
        let mut collector = ErrorCollector::new();
        assert!(!collector.has_errors());

        collector.add_simple_error(
            DSLError::Parse(ParseError::Incomplete),
            SourceLocation::unknown(),
            ErrorSeverity::Error,
        );

        assert!(collector.has_errors());
        assert_eq!(collector.error_count(), 1);
    }

    #[test]
    fn test_contextual_error() {
        let error = ContextualError::new(
            DSLError::Parse(ParseError::Incomplete),
            SourceLocation::new(Some("test.dsl".to_string()), 1, 1, 0),
            ErrorSeverity::Error,
        )
        .with_context("While parsing workflow");

        assert_eq!(error.context.len(), 1);
        assert_eq!(error.context[0], "While parsing workflow");
    }

    #[test]
    fn test_source_location_display() {
        let loc = SourceLocation::new(Some("test.dsl".to_string()), 10, 5, 100);
        assert_eq!(format!("{}", loc), "test.dsl:10:5");

        let loc_no_file = SourceLocation::new(None, 10, 5, 100);
        assert_eq!(format!("{}", loc_no_file), "10:5");
    }
}

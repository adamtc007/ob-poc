//! Comprehensive error handling for the UBO DSL system
//!
//! This module provides idiomatic Rust error types using thiserror for
//! better error messages and proper error chain handling.

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

    #[cfg(feature = "database")]
    #[error("Database error: {0}")]
    Database(String),

    #[cfg(feature = "database")]
    #[error("Document processing error: {0}")]
    DocumentProcessing(String),

    #[cfg(feature = "database")]
    #[error("DSL Manager error: {0}")]
    DslManager(String),
}

impl serde::Serialize for DSLError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        match self {
            DSLError::Parse(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Parse")?;
                state.serialize_field("error", err)?;
                state.end()
            }
            DSLError::Grammar(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Grammar")?;
                state.serialize_field("error", err)?;
                state.end()
            }
            DSLError::Vocabulary(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Vocabulary")?;
                state.serialize_field("error", err)?;
                state.end()
            }
            DSLError::Validation(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Validation")?;
                state.serialize_field("error", err)?;
                state.end()
            }
            DSLError::Runtime(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Runtime")?;
                state.serialize_field("error", err)?;
                state.end()
            }
            DSLError::Io(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Io")?;
                state.serialize_field("error", &err.to_string())?;
                state.end()
            }
            DSLError::Serialization(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Serialization")?;
                state.serialize_field("error", &err.to_string())?;
                state.end()
            }
            #[cfg(feature = "database")]
            DSLError::Database(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "Database")?;
                state.serialize_field("error", err)?;
                state.end()
            }
            #[cfg(feature = "database")]
            DSLError::DocumentProcessing(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "DocumentProcessing")?;
                state.serialize_field("error", err)?;
                state.end()
            }
            #[cfg(feature = "database")]
            DSLError::DslManager(err) => {
                let mut state = serializer.serialize_struct("DSLError", 2)?;
                state.serialize_field("type", "DslManager")?;
                state.serialize_field("error", err)?;
                state.end()
            }
        }
    }
}

/// Parse errors from nom-based parsers
#[derive(Error, Debug, serde::Serialize)]
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
#[derive(Error, Debug, serde::Serialize)]
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
#[derive(Error, Debug, serde::Serialize)]
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
#[derive(Error, Debug, serde::Serialize)]
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
#[derive(Error, Debug, serde::Serialize)]
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

// Conversions from Forth engine errors to DSLError
impl From<crate::forth_engine::errors::EngineError> for DSLError {
    fn from(err: crate::forth_engine::errors::EngineError) -> Self {
        use crate::forth_engine::errors::EngineError;
        match err {
            EngineError::Parse(msg) => DSLError::Parse(ParseError::Internal { message: msg }),
            EngineError::Compile(e) => DSLError::Runtime(RuntimeError::ExecutionFailed {
                statement: "compile".to_string(),
                message: e.to_string(),
            }),
            EngineError::Vm(e) => DSLError::Runtime(RuntimeError::ExecutionFailed {
                statement: "vm".to_string(),
                message: e.to_string(),
            }),
            EngineError::Database(msg) => {
                #[cfg(feature = "database")]
                {
                    DSLError::Database(msg)
                }
                #[cfg(not(feature = "database"))]
                {
                    DSLError::Runtime(RuntimeError::Database { message: msg })
                }
            }
        }
    }
}

impl From<crate::forth_engine::errors::VmError> for DSLError {
    fn from(err: crate::forth_engine::errors::VmError) -> Self {
        DSLError::Runtime(RuntimeError::ExecutionFailed {
            statement: "vm".to_string(),
            message: err.to_string(),
        })
    }
}

impl From<crate::forth_engine::errors::CompileError> for DSLError {
    fn from(err: crate::forth_engine::errors::CompileError) -> Self {
        DSLError::Runtime(RuntimeError::ExecutionFailed {
            statement: "compile".to_string(),
            message: err.to_string(),
        })
    }
}

/// Result type aliases for convenience
pub(crate) type DSLResult<T> = Result<T, DSLError>;
pub type ParseResult<T> = Result<T, ParseError>;
pub type ValidationResult<T> = Result<T, ValidationError>;

// SourceLocation moved to dsl_types crate - import from there

// Error severity levels
// ErrorSeverity moved to dsl_types crate - import from there

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
    use crate::forth_engine::errors::{CompileError, EngineError, VmError};

    #[test]
    fn test_vm_error_to_dsl_error() {
        let vm_err = VmError::StackUnderflow {
            expected: 2,
            found: 0,
        };
        let dsl_err: DSLError = vm_err.into();
        match dsl_err {
            DSLError::Runtime(RuntimeError::ExecutionFailed { statement, .. }) => {
                assert_eq!(statement, "vm");
            }
            _ => panic!("Expected Runtime error"),
        }
    }

    #[test]
    fn test_compile_error_to_dsl_error() {
        let compile_err = CompileError::UnknownWord("unknown.verb".to_string());
        let dsl_err: DSLError = compile_err.into();
        match dsl_err {
            DSLError::Runtime(RuntimeError::ExecutionFailed { statement, .. }) => {
                assert_eq!(statement, "compile");
            }
            _ => panic!("Expected Runtime error"),
        }
    }

    #[test]
    fn test_engine_error_parse_to_dsl_error() {
        let engine_err = EngineError::Parse("syntax error".to_string());
        let dsl_err: DSLError = engine_err.into();
        match dsl_err {
            DSLError::Parse(ParseError::Internal { message }) => {
                assert_eq!(message, "syntax error");
            }
            _ => panic!("Expected Parse error"),
        }
    }

    #[test]
    fn test_engine_error_vm_to_dsl_error() {
        let vm_err = VmError::TypeError {
            expected: "String".to_string(),
            found: "Int".to_string(),
        };
        let engine_err = EngineError::Vm(vm_err);
        let dsl_err: DSLError = engine_err.into();
        match dsl_err {
            DSLError::Runtime(RuntimeError::ExecutionFailed { statement, message }) => {
                assert_eq!(statement, "vm");
                assert!(message.contains("String"));
            }
            _ => panic!("Expected Runtime error"),
        }
    }

    #[test]
    fn test_parse_error_creation() {
        let err = ParseError::Syntax {
            position: 10,
            message: "unexpected token".to_string(),
        };
        assert!(err.to_string().contains("unexpected token"));
    }

    #[test]
    fn test_validation_error_creation() {
        let err = ValidationError::TypeMismatch {
            expected: "String".to_string(),
            found: "Integer".to_string(),
            location: "line 5".to_string(),
        };
        assert!(err.to_string().contains("String"));
        assert!(err.to_string().contains("Integer"));
    }

    #[test]
    fn test_runtime_error_creation() {
        let err = RuntimeError::Database {
            message: "connection failed".to_string(),
        };
        assert!(err.to_string().contains("connection failed"));
    }

    #[test]
    fn test_vocabulary_error_creation() {
        let err = VocabularyError::UnknownVerb {
            verb: "unknown.verb".to_string(),
            domain: "unknown".to_string(),
        };
        assert!(err.to_string().contains("unknown.verb"));
    }
}

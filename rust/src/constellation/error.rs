use thiserror::Error;

/// Constellation library error type.
#[derive(Debug, Error)]
pub enum ConstellationError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("execution error: {0}")]
    Execution(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Standard result type for constellation operations.
pub type ConstellationResult<T> = Result<T, ConstellationError>;

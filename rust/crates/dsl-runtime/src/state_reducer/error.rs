use thiserror::Error;

/// Reducer library error type.
#[derive(Debug, Error)]
pub enum ReducerError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("evaluation error: {0}")]
    Evaluation(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Standard result type for reducer operations.
pub type ReducerResult<T> = Result<T, ReducerError>;

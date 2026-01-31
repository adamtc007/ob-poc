//! Input error types.

use thiserror::Error;

/// Errors that can occur during input processing.
#[derive(Debug, Clone, Error)]
pub enum InputError {
    /// Key code not recognized.
    #[error("unknown key code: {0}")]
    UnknownKeyCode(String),

    /// No binding for this input.
    #[error("no binding for input")]
    NoBinding,

    /// Gesture recognition failed.
    #[error("gesture recognition failed: {0}")]
    GestureError(String),

    /// Invalid configuration.
    #[error("invalid input config: {0}")]
    InvalidConfig(String),

    /// Input sequence incomplete (e.g., chord not finished).
    #[error("input sequence incomplete")]
    IncompleteSequence,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = InputError::UnknownKeyCode("XYZ".to_string());
        assert!(err.to_string().contains("XYZ"));

        let err = InputError::NoBinding;
        assert!(!err.to_string().is_empty());
    }
}

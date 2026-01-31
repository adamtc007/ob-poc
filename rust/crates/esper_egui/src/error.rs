//! Render error types.

use thiserror::Error;

/// Errors that can occur during rendering.
#[derive(Debug, Clone, Error)]
pub enum RenderError {
    /// Invalid camera state.
    #[error("invalid camera state: {0}")]
    InvalidCamera(String),

    /// Entity not found for rendering.
    #[error("entity not found: {0}")]
    EntityNotFound(u64),

    /// Chamber not loaded.
    #[error("chamber not loaded: {0}")]
    ChamberNotLoaded(u32),

    /// Invalid render configuration.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// Texture or resource missing.
    #[error("resource missing: {0}")]
    ResourceMissing(String),

    /// Layout calculation failed.
    #[error("layout error: {0}")]
    LayoutError(String),
}

/// Result type for render operations.
pub type RenderResult<T> = Result<T, RenderError>;

impl RenderError {
    /// Check if this error is recoverable (can continue rendering).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RenderError::EntityNotFound(_) | RenderError::ResourceMissing(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = RenderError::EntityNotFound(123);
        assert!(err.to_string().contains("123"));
    }

    #[test]
    fn error_recoverable() {
        assert!(RenderError::EntityNotFound(1).is_recoverable());
        assert!(RenderError::ResourceMissing("tex".into()).is_recoverable());
        assert!(!RenderError::InvalidCamera("bad".into()).is_recoverable());
    }
}

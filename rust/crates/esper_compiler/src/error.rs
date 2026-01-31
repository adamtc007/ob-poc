//! Compiler error types.

use thiserror::Error;

/// Errors that can occur during snapshot compilation.
#[derive(Debug, Clone, Error)]
pub enum CompilerError {
    /// No entities in input.
    #[error("empty graph: no entities to compile")]
    EmptyGraph,

    /// Entity not found.
    #[error("entity not found: {0}")]
    EntityNotFound(u64),

    /// Invalid edge (endpoint not found).
    #[error("invalid edge: entity {0} not found")]
    InvalidEdge(u64),

    /// Layout algorithm failed.
    #[error("layout failed: {0}")]
    LayoutFailed(String),

    /// Chamber capacity exceeded.
    #[error("chamber capacity exceeded: {count} entities (max: {max})")]
    ChamberCapacityExceeded { count: usize, max: usize },

    /// Cycle detected in hierarchy.
    #[error("cycle detected in entity hierarchy")]
    CycleDetected,

    /// Invalid configuration.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// Cache error.
    #[error("cache error: {0}")]
    CacheError(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

impl CompilerError {
    /// Check if this error is recoverable (can retry with different config).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CompilerError::LayoutFailed(_)
                | CompilerError::ChamberCapacityExceeded { .. }
                | CompilerError::CacheError(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = CompilerError::EntityNotFound(42);
        assert!(err.to_string().contains("42"));

        let err = CompilerError::ChamberCapacityExceeded {
            count: 15000,
            max: 10000,
        };
        assert!(err.to_string().contains("15000"));
        assert!(err.to_string().contains("10000"));
    }

    #[test]
    fn error_recoverable() {
        assert!(CompilerError::LayoutFailed("test".into()).is_recoverable());
        assert!(!CompilerError::EmptyGraph.is_recoverable());
        assert!(!CompilerError::CycleDetected.is_recoverable());
    }
}

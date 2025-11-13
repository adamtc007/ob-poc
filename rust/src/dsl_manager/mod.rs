//! DSL Manager - Clean Gateway for DSL Operations
//!
//! This module provides the simplified DSL Manager implementation following the
//! proven call chain architecture from the independent implementation blueprint.
//!
//! ## Clean Architecture
//! DSL Manager → DSL Mod → DB State Manager → DSL Visualizer

pub mod clean_manager;

// Public re-exports for external API
pub use clean_manager::{
    AiResult, CallChainResult, CallChainSteps, CleanDslManager, CleanManagerConfig,
    IncrementalResult, ValidationResult,
};

/// DSL Manager error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum DslManagerError {
    #[error("Processing failed: {message}")]
    ProcessingError { message: String },

    #[error("Validation failed: {message}")]
    ValidationError { message: String },

    #[error("State error: {message}")]
    StateError { message: String },
}

/// Result type for DSL Manager operations
pub type DslManagerResult<T> = Result<T, DslManagerError>;

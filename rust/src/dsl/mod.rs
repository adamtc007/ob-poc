//! Central DSL Management Module
//!
//! This module implements the centralized DSL editing and management system
//! following the "ONE EBNF, ONE DSL vocab, ONE data dictionary" architecture.
//!
//! ## Key Components:
//! - `central_editor`: Core DSL editing engine with domain context switching
//! - `domain_context`: Domain context system for operation routing
//! - `domain_registry`: Registry and trait system for domain handlers
//! - `operations`: Standard DSL operation types and transformations
//!
//! ## Architecture:
//! The centralized approach replaces domain-specific edit functions with a
//! unified system that uses domain context to route operations while maintaining
//! shared grammar, vocabulary, and dictionary resources.

pub mod central_editor;
pub mod domain_context;
pub mod domain_registry;
pub mod operations;

// Re-export main types for convenience
pub use central_editor::CentralDslEditor;
pub use domain_context::{DomainContext, OperationMetadata, StateRequirements};
pub use domain_registry::{DomainHandler, DomainRegistry};
pub use operations::DslOperation;

/// Central DSL editing error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum DslEditError {
    #[error("Domain '{0}' not found in registry")]
    DomainNotFound(String),

    #[error("Operation '{0}' not supported by domain '{1}'")]
    UnsupportedOperation(String, String),

    #[error("Domain validation failed: {0}")]
    DomainValidationError(String),

    #[error("Grammar validation failed: {0}")]
    GrammarValidationError(String),

    #[error("Dictionary validation failed: {0}")]
    DictionaryValidationError(String),

    #[error("State transition not allowed: {from} -> {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Business rule violation: {0}")]
    BusinessRuleViolation(String),

    #[error("DSL compilation error: {0}")]
    CompilationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Result type for DSL editing operations
pub type DslEditResult<T> = Result<T, DslEditError>;

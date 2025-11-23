//! Typed Attribute System
//!
//! This module implements the complete AttributeID-as-Type pattern,
//! providing compile-time type safety for all attributes used in the DSL.

pub mod builder;
pub mod execution_context;
pub mod kyc;
pub mod resolver;
pub mod sources;
pub mod types;
pub mod uuid_constants;

// Re-export key types for convenience
pub use types::{
    AttributeCategory, AttributeMetadata, AttributeType, DataType, TypedAttributeRef,
    ValidationError as TypeValidationError, ValidationErrorType, ValidationRules,
};

// Re-export resolver (Phase 2)
pub use resolver::{AttributeResolver, ResolutionError, ResolutionResult};

// Re-export execution context (Phase 3)
pub use execution_context::{ExecutionContext, ValueSource};

// Re-export builder
pub use builder::{entity_set_attribute, validate_attribute, DslBuilder, DslValue};

// Re-export all KYC attributes
pub use kyc::*;

//! Services module for core business logic implementations
//!
//! This module contains the service implementations that provide
//! business logic and external interfaces for the DSL engine functionality.
//!
//! NOTE: All services now delegate to DSL Manager as the single entry point
//! for DSL operations. This ensures proper lifecycle management and consistency.

pub mod ai_dsl_service;

// Re-export service types for backwards compatibility
pub use ai_dsl_service::{
    AiDslService, AiOnboardingRequest, AiOnboardingResponse, CbuGenerator, DslInstanceSummary,
    ExecutionDetails, HealthCheckResult, ValidationResult,
};

// Re-export DSL Manager types for direct access when needed
pub use crate::dsl_manager::{DslContext, DslManager, DslManagerFactory, DslProcessingOptions};

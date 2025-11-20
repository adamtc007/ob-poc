//! Services module for core business logic implementations
//!
//! This module contains the service implementations that provide
//! business logic and external interfaces for the DSL engine functionality.
//!
//! NOTE: All services now delegate to DSL Manager as the single entry point
//! for DSL operations. This ensures proper lifecycle management and consistency.
//!
//! ## Architecture Update (November 2025)
//! Legacy services (ai_dsl_service, dsl_ast_sync, dsl_lifecycle) have been
//! removed in favor of the new Forth-engine architecture. All DSL operations
//! now flow through the Forth stack-based execution engine.

// Re-export DSL Manager types for direct access when needed
pub use crate::dsl_manager::{
    CallChainResult, CleanDslManager, DslManagerError, IncrementalResult,
};

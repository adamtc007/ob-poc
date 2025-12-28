//! Core execution types for ob-poc DSL execution layer
//!
//! This crate provides the foundational types used across the execution pipeline:
//! - `ExecutionContext` - Symbol table and execution state
//! - `ExecutionResult` - Result of executing a single verb
//! - `StepResult` - Detailed result of executing a single step
//! - `ExecutionResults` - Accumulated results from executing a plan
//!
//! These types are extracted to a separate crate to:
//! 1. Break circular dependencies between execution and templates
//! 2. Allow templates and workflow to reference execution types without importing the full executor
//! 3. Keep the core types minimal and focused

mod context;
mod result;
mod step_result;

pub use context::ExecutionContext;
pub use result::ExecutionResult;
pub use step_result::{ExecutionResults, StepResult};

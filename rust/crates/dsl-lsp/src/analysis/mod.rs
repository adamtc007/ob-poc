//! Document analysis for the DSL Language Server.
//!
//! Handles parsing, symbol tracking, and semantic analysis.

pub mod document;
mod symbols;
mod context;

pub use document::DocumentState;
pub use symbols::SymbolTable;
pub use context::{CompletionContext, detect_completion_context};

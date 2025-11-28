//! Document analysis for the DSL Language Server.
//!
//! Handles parsing, symbol tracking, and semantic analysis.

mod context;
pub mod document;
mod symbols;
mod v2_adapter;

pub use context::{detect_completion_context, CompletionContext};
pub use document::DocumentState;
pub use symbols::SymbolTable;
pub use v2_adapter::parse_with_v2;

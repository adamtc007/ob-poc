//! Document analysis for the DSL Language Server.
//!
//! Handles parsing, symbol tracking, and semantic analysis.

mod context;
pub mod document;
mod symbols;
mod v2_adapter;

pub(crate) use context::{detect_completion_context, CompletionContext};
pub(crate) use document::DocumentState;
pub(crate) use symbols::SymbolTable;
pub(crate) use v2_adapter::parse_with_v2;

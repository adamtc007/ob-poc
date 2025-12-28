//! dsl-core: Core DSL parser, AST, and types for OB-POC
//!
//! This crate contains the pure DSL logic with NO database dependencies:
//! - AST types (Program, Statement, VerbCall, AstNode, etc.)
//! - Nom-based S-expression parser
//! - Binding context for symbol resolution
//! - Diagnostic types for error reporting
//! - Op enum for primitive operations
//! - DAG builder and topological sort
//! - YAML configuration types and loader
//!
//! The execution layer (generic_executor, custom_ops) remains in ob-poc
//! as it requires database access.

pub mod ast;
pub mod binding_context;
pub mod compiler;
pub mod config;
pub mod dag;
pub mod diagnostics;
pub mod ops;
pub mod parser;

// Re-export commonly used types
pub use ast::{AstNode, Program, Span, Statement, VerbCall};
pub use binding_context::BindingContext;
pub use config::loader::ConfigLoader;
pub use config::types::*;
pub use diagnostics::{Diagnostic, DiagnosticCode, Severity, SourceSpan};
pub use parser::parse_program;

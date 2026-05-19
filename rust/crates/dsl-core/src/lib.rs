//! dsl-core: Core DSL parser, AST, and types for OB-POC
//!
//! This crate contains the pure DSL logic with NO database dependencies:
//! - AST types (Program, Statement, VerbCall, AstNode, etc.)
//! - Nom-based S-expression parser
//! - Binding context for symbol resolution
//! - Diagnostic types for error reporting
//! - Op-free compiler (compile_to_steps, CompileStep)
//! - YAML configuration types and loader
//!
//! The Op enum and DAG builder were removed in Phase 3 CR A4.
//! The execution layer (generic_executor, custom_ops) remains in ob-poc
//! as it requires database access.

pub mod ast;
pub mod binding_context;
pub mod compiler;
pub mod config;
pub mod diagnostics;
pub mod frontier;
pub mod parser;
pub mod resolver;
pub(crate) mod viewport_parser;

// Re-export commonly used types
pub use ast::{AstNode, Program, Span, Statement, VerbCall};

// Re-export viewport verb types
pub use ast::{
    ConfidenceZone, EnhanceArg, ExportFormat, FocusTarget, NavDirection, NavTarget, ViewType,
    ViewportVerb,
};
pub use binding_context::BindingContext;
pub use config::loader::ConfigLoader;
pub use config::types::{
    ArgConfig, ArgType, CrudConfig, CrudOperation, DomainConfig, LookupConfig, ReturnTypeConfig,
    ReturnsConfig, SearchKeyConfig, VerbBehavior, VerbConfig, VerbConsumes, VerbLifecycle,
    VerbMetadata, VerbOutputConfig, VerbProduces, VerbsConfig,
};
pub use diagnostics::{Diagnostic, DiagnosticCode, Severity, SourceSpan};
pub use parser::parse_program;

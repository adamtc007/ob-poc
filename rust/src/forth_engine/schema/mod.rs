//! Verb Schema System for the DSL.
//!
//! This module provides formal verb schemas that drive:
//! - Argument validation
//! - Type checking  
//! - LSP completions
//! - Documentation generation
//! - LLM context export
//!
//! # Architecture
//!
//! The schema system follows a three-phase validation pipeline:
//!
//! 1. **Parse Phase**: Nom parser produces RawAst with source spans
//! 2. **Validation Phase**: SchemaValidator checks RawAst against VerbDef schemas
//! 3. **Execution Phase**: ValidatedAst is executed with type-safe values
//!
//! # Example
//!
//! ```ignore
//! use forth_engine::schema::{SchemaValidator, SchemaCache, ValidationContext};
//!
//! let cache = Arc::new(SchemaCache::with_defaults());
//! let validator = SchemaValidator::new(cache);
//! let context = ValidationContext::new();
//!
//! let result = validator.validate(&raw_ast, &context);
//! match result {
//!     Ok(validated) => { /* execute */ },
//!     Err(report) => println!("{}", report.format(&source, "file.dsl")),
//! }
//! ```

pub mod types;
pub mod registry;
pub mod verbs;
pub mod ast;
pub mod cache;
pub mod validator;
pub mod validation_errors;
pub mod llm_export;

// Re-export main types
pub use types::*;
pub use registry::{VerbRegistry, VERB_REGISTRY};
pub use ast::{
    Span, Spanned,
    RawAst, RawExpr, RawExprKind, RawArg, RawValue,
    ValidatedAst, ValidatedExpr, ValidatedExprKind, TypedValue,
    SymbolTable, SymbolInfo, SymbolError,
};
pub use cache::{SchemaCache, LookupEntry};
pub use validator::{SchemaValidator, ValidationContext};
pub use validation_errors::{ValidationReport, ValidationError, ErrorKind};

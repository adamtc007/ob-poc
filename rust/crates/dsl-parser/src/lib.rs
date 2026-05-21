//! dsl-parser: S-expression lexer and parser for the unified DSL v0.1.
//!
//! Produces a raw untyped parse tree (`SourceFile`) from DSL source text.
//! Kind classification and typed AST construction happen in `dsl-ast`.
//!
//! # Entry point
//!
//! ```rust,ignore
//! let (source_file, diagnostics) = dsl_parser::parse(src);
//! if diagnostics.has_errors() {
//!     // handle errors
//! }
//! ```

pub mod lexer;
pub mod parser;
pub mod raw_ast;

pub use parser::parse;
pub use raw_ast::{RawAtom, RawValue, SourceFile};

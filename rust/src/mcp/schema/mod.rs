//! Schema-guided DSL parsing infrastructure
//!
//! This module provides:
//! - VerbSpec and ArgSchema types for verb definitions
//! - S-expression tokenizer with source spans
//! - Schema-guided parser that validates against verb specs
//! - Canonicalizer that normalizes parsed ASTs to keyword form
//! - Registry that loads and resolves verb schemas
//!
//! # Architecture
//!
//! ```text
//! Input: "(drill \"Allianz\")" or "(view.drill :entity \"Allianz\")"
//!            │
//!            ▼
//!     ┌──────────────┐
//!     │  Tokenizer   │  → Vec<Token> with spans
//!     └──────────────┘
//!            │
//!            ▼
//!     ┌──────────────┐
//!     │   Parser     │  → ParsedExpr (aliases resolved)
//!     └──────────────┘
//!            │
//!            ▼
//!     ┌──────────────┐
//!     │ Canonicalizer│  → CanonicalAst (keyword form)
//!     └──────────────┘
//!            │
//!            ▼
//!     Ready for executor
//! ```

mod canonicalizer;
mod parser;
mod registry;
mod tokenizer;
mod types;

pub use canonicalizer::{canonicalize, CanonicalArg, CanonicalAst};
pub use parser::{ParseError, ParseResult, ParsedExpr, Parser};
pub use registry::{HeadResolution, VerbRegistry};
pub use tokenizer::{Span, Token, TokenKind, Tokenizer};
pub use types::*;

#[cfg(test)]
mod tests;

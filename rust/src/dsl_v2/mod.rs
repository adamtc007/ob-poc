//! DSL v2 - Unified S-Expression DSL
//!
//! This module implements a complete refactoring of the ob-poc DSL system
//! from the dual-grammar, vocab-sprawl architecture to a unified, data-driven design.
//!
//! ## Architecture
//!
//! ```text
//! DSL Source Text → Parser (Nom) → AST → Executor → Database
//! ```
//!
//! ## Key Features
//!
//! - **Single Grammar**: One S-expression syntax: `(domain.verb :key value ...)`
//! - **Data-Driven Execution**: 90% of verbs defined as static data, not code
//! - **Explicit Custom Operations**: 10% truly custom operations with mandatory rationale
//! - **Document↔Attribute Integration**: Bidirectional mapping via `document_type_attributes`
//!
//! ## Modules
//!
//! - `ast`: AST type definitions
//! - `parser`: Nom-based parser
//! - `verbs`: Standard verb definitions (Tier 1)
//! - `mappings`: Column mappings (DSL key → DB column)
//! - `executor`: DslExecutor + generic CRUD functions
//! - `custom_ops`: Custom operation trait and implementations (Tier 2)

pub mod ast;
pub mod parser;
pub mod verbs;
pub mod mappings;
pub mod executor;
pub mod custom_ops;

// Re-export key types for convenience
pub use ast::{Program, Statement, VerbCall, Argument, Key, Value, Span};
pub use parser::{parse_program, parse_single_verb};
pub use verbs::{VerbDef, Behavior, find_verb, verbs_for_domain, domains, verb_count, STANDARD_VERBS};
pub use mappings::{DbType, ColumnMapping, TableMappings, get_table_mappings, resolve_column};
pub use executor::{DslExecutor, ExecutionContext, ExecutionResult, ReturnType};

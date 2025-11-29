//! DSL v2 - Unified S-Expression DSL
//!
//! This module implements a complete refactoring of the ob-poc DSL system
//! from the dual-grammar, vocab-sprawl architecture to a unified, data-driven design.
//!
//! ## Architecture
//!
//! ```text
//! DSL Source Text → Parser (Nom) → AST → Compiler → Plan → Executor → Database
//! ```
//!
//! ## Execution Model
//!
//! The DSL supports **declarative nested structures** that are compiled into
//! a linear execution plan with dependency injection:
//!
//! ```text
//! (cbu.create :name "Fund"           →  Step 0: cbu.create → $0
//!   :roles [                            Step 1: assign-role($0, aviva)
//!     (cbu.assign-role :entity-id       Step 2: assign-role($0, bob)
//!       @aviva :role "Mgr")
//!   ])
//! ```
//!
//! ## Key Features
//!
//! - **Single Grammar**: One S-expression syntax: `(domain.verb :key value ...)`
//! - **Nested Operations**: Child verb calls compiled with parent dependency injection
//! - **Data-Driven Execution**: 90% of verbs defined as static data, not code
//! - **Explicit Custom Operations**: 10% truly custom operations with mandatory rationale
//! - **Document↔Attribute Integration**: Bidirectional mapping via `document_type_attributes`
//!
//! ## Modules
//!
//! - `ast`: AST type definitions
//! - `parser`: Nom-based parser
//! - `execution_plan`: Compiler and execution plan types
//! - `verbs`: Standard verb definitions (Tier 1)
//! - `mappings`: Column mappings (DSL key → DB column)
//! - `executor`: DslExecutor + generic CRUD functions
//! - `custom_ops`: Custom operation trait and implementations (Tier 2)

pub mod applicability_rules;
pub mod assembly;
pub mod ast;
pub mod csg_linter;
pub mod custom_ops;
pub mod execution_plan;
pub mod executor;
pub mod mappings;
pub mod parser;
#[cfg(feature = "database")]
pub mod ref_resolver;
pub mod semantic_context;
pub mod semantic_intent;
#[cfg(feature = "database")]
pub mod semantic_validator;
pub mod validation;
pub mod verb_schema;
pub mod verbs;

// Re-export key types for convenience
pub use applicability_rules::{ApplicabilityRules, AttributeApplicability, DocumentApplicability};
pub use ast::{Argument, Key, Program, Span, Statement, Value, VerbCall};
pub use csg_linter::{CsgLinter, InferredContext, LintResult};
pub use execution_plan::{compile, CompileError, ExecutionPlan, ExecutionStep, Injection};
pub use executor::{DslExecutor, ExecutionContext, ExecutionResult, ReturnType};
pub use mappings::{get_table_mappings, resolve_column, ColumnMapping, DbType, TableMappings};
pub use parser::{parse_program, parse_single_verb};
pub use semantic_context::SemanticContextStore;
pub use verbs::{
    domains, find_verb, verb_count, verbs_for_domain, Behavior, VerbDef, STANDARD_VERBS,
};

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
//! - **YAML-Driven Execution**: Verbs defined in `config/verbs.yaml`
//! - **Explicit Custom Operations**: Plugins for complex logic (external APIs, etc.)
//!
//! ## Modules
//!
//! - `ast`: AST type definitions
//! - `parser`: Nom-based parser
//! - `execution_plan`: Compiler and execution plan types
//! - `config`: YAML configuration loading
//! - `runtime_registry`: Runtime verb registry from YAML
//! - `generic_executor`: YAML-driven CRUD executor
//! - `executor`: DslExecutor orchestration
//! - `custom_ops`: Plugin trait and implementations

pub mod applicability_rules;
pub mod assembly;
pub mod ast;
pub mod config;
pub mod csg_linter;
pub mod custom_ops;
pub mod execution_plan;
pub mod executor;
#[cfg(feature = "database")]
pub mod gateway_resolver;
#[cfg(feature = "database")]
pub mod generic_executor;
#[cfg(feature = "database")]
pub mod idempotency;
pub mod parser;
#[cfg(feature = "database")]
pub mod ref_resolver;
pub mod runtime_registry;
pub mod semantic_context;
pub mod semantic_intent;
#[cfg(feature = "database")]
pub mod semantic_validator;
pub mod validation;
pub mod verb_registry;
pub mod verb_schema;

// Re-export key types for convenience
pub use applicability_rules::{ApplicabilityRules, AttributeApplicability, DocumentApplicability};
pub use ast::{Argument, Key, Program, Span, Statement, Value, VerbCall};
pub use config::types::LookupConfig;
pub use csg_linter::{CsgLinter, InferredContext, LintResult};
pub use execution_plan::{compile, CompileError, ExecutionPlan, ExecutionStep, Injection};
pub use executor::{DslExecutor, ExecutionContext, ExecutionResult, ReturnType};
#[cfg(feature = "database")]
pub use gateway_resolver::GatewayRefResolver;
#[cfg(feature = "database")]
pub use generic_executor::{GenericCrudExecutor, GenericExecutionResult};
#[cfg(feature = "database")]
pub use idempotency::{compute_idempotency_key, IdempotencyManager};
pub use parser::{parse_program, parse_single_verb};
#[cfg(feature = "database")]
pub use ref_resolver::RefResolver;
pub use runtime_registry::{runtime_registry, RuntimeVerbRegistry};
pub use semantic_context::SemanticContextStore;
#[cfg(feature = "database")]
pub use semantic_validator::{validate_dsl, validate_dsl_with_csg, SemanticValidator};
pub use verb_registry::{
    find_unified_verb, registry, verb_exists, ArgDef, UnifiedVerbDef, UnifiedVerbRegistry,
    VerbBehavior,
};

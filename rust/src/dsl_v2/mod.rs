//! DSL v2 - Unified S-Expression DSL
//!
//! Single pipeline from source text to execution.
//!
//! ## Pipeline
//!
//! ```text
//! Source → Parser → Raw AST → Enrichment → Enriched AST → Compiler → Plan → Executor
//!                                 ↓
//!                          YAML verb defs
//! ```
//!
//! ## AST Node Types
//!
//! - `Literal`: Terminal values (strings, numbers, booleans)
//! - `SymbolRef`: `@name` bindings resolved at execution time
//! - `EntityRef`: External references resolved via EntityGateway
//!
//! ## Key Files
//!
//! - `ast.rs`: Self-describing AST types
//! - `parser.rs`: Nom-based S-expression parser
//! - `enrichment.rs`: Converts raw AST strings to EntityRefs using YAML config
//! - `execution_plan.rs`: Compiles AST to dependency-sorted execution plan
//! - `executor.rs`: Executes plan against database

pub mod applicability_rules;
pub mod ast;
pub mod binding_context;
pub mod config;
pub mod csg_linter;
pub mod custom_ops;
pub mod enrichment;
pub mod entity_deps;
pub mod execution_plan;
pub mod executor;
#[cfg(feature = "database")]
pub mod gateway_resolver;
#[cfg(feature = "database")]
pub mod generic_executor;
#[cfg(feature = "database")]
pub mod idempotency;
pub mod intent;
pub mod intent_extractor;
#[cfg(feature = "database")]
pub mod lsp_validator;
pub mod parser;
#[cfg(feature = "database")]
pub mod ref_resolver;
pub mod runtime_registry;
pub mod semantic_context;
#[cfg(feature = "database")]
pub mod semantic_validator;
pub mod suggestions;
pub mod topo_sort;
pub mod validation;
pub mod verb_registry;

// Re-export key types for convenience
pub use applicability_rules::{ApplicabilityRules, AttributeApplicability, DocumentApplicability};
pub use ast::{
    count_entity_refs, Argument, AstNode, EntityRefStats, Literal, Program, Span, Statement,
    VerbCall,
};
pub use binding_context::{BindingContext, BindingInfo};
pub use config::types::LookupConfig;
pub use csg_linter::{CsgLinter, InferredContext, LintResult};
pub use enrichment::{enrich_program, EnrichmentError, EnrichmentResult};
#[cfg(feature = "database")]
pub use entity_deps::init_entity_deps;
pub use entity_deps::{
    entity_deps, topological_sort_unified, DependencyKind, EntityDep, EntityDependencyRegistry,
    EntityInstance, EntityTypeKey, TopoSortUnifiedError, TopoSortUnifiedResult,
};
pub use execution_plan::{
    compile, compile_with_planning, BindingInfo as PlanningBindingInfo, CompileError,
    ExecutionPlan, ExecutionStep, Injection, PlannerDiagnostic, PlanningContext, PlanningResult,
    SyntheticStep,
};
pub use executor::{DslExecutor, ExecutionContext, ExecutionResult, ReturnType};
#[cfg(feature = "database")]
pub use gateway_resolver::GatewayRefResolver;
#[cfg(feature = "database")]
pub use generic_executor::{GenericCrudExecutor, GenericExecutionResult};
#[cfg(feature = "database")]
pub use idempotency::{compute_idempotency_key, IdempotencyManager};
pub use intent::{ArgIntent, DslIntent, DslIntentBatch, ResolvedArg};
pub use intent_extractor::IntentExtractor;
#[cfg(feature = "database")]
pub use lsp_validator::LspValidator;
pub use parser::{parse_program, parse_single_verb};
#[cfg(feature = "database")]
pub use ref_resolver::RefResolver;
pub use runtime_registry::{runtime_registry, RuntimeVerbRegistry};
pub use semantic_context::SemanticContextStore;
#[cfg(feature = "database")]
pub use semantic_validator::{validate_dsl, validate_dsl_with_csg, SemanticValidator};
pub use topo_sort::{
    emit_dsl, topological_sort, topological_sort_with_lifecycle, TopoSortError, TopoSortResult,
};
pub use verb_registry::{
    find_unified_verb, registry, verb_exists, ArgDef, UnifiedVerbDef, UnifiedVerbRegistry,
    VerbBehavior,
};

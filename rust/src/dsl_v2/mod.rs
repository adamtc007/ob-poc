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
//! - `ast.rs`: Self-describing AST types (in dsl-core)
//! - `parser.rs`: Nom-based S-expression parser (in dsl-core)
//! - `enrichment.rs`: Converts raw AST strings to EntityRefs using YAML config
//! - `execution_plan.rs`: Compiles AST to dependency-sorted execution plan
//! - `executor.rs`: Executes plan against database

// =============================================================================
// Re-export core types from dsl-core crate
// =============================================================================

// AST types
pub use dsl_core::ast;
pub use dsl_core::ast::{
    count_entity_refs, Argument, AstNode, EntityRefStats, Literal, Program, Span, Statement,
    VerbCall,
};

// Parser
pub use dsl_core::parser;
pub use dsl_core::parser::{parse_program, parse_single_verb};

// Binding context
pub use dsl_core::binding_context;
pub use dsl_core::binding_context::{BindingContext, BindingInfo};

// Config types
pub use dsl_core::config;
pub use dsl_core::config::types::LookupConfig;
pub use dsl_core::config::ConfigLoader;

// Diagnostics
pub use dsl_core::diagnostics;
pub use dsl_core::diagnostics::{
    cycle_error, implicit_create_hint, missing_arg_error, undefined_symbol_error,
    unknown_verb_error, Diagnostic, DiagnosticCode, RelatedInfo, Severity, SourceSpan,
    SuggestedFix,
};

// Ops and DAG
pub use dsl_core::dag;
pub use dsl_core::dag::{
    build_execution_plan, collect_external_refs, describe_plan, CycleError, ExecutionPhase,
    ExecutionPlan as DagExecutionPlan,
};
pub use dsl_core::ops;
pub use dsl_core::ops::{DocKey, EntityKey, Op, OpRef};

// Compiler
pub use dsl_core::compiler;
pub use dsl_core::compiler::{compile_to_ops, CompileError as OpCompileError, CompiledProgram};

// =============================================================================
// Local modules (require database or other dependencies not in dsl-core)
// =============================================================================

pub mod applicability_rules;
#[cfg(feature = "database")]
pub mod batch_executor;
pub mod csg_linter;
pub mod custom_ops;
pub mod domain_context;
pub mod enrichment;
pub mod entity_deps;
pub mod execution_plan;
pub mod execution_result;
pub mod executor;
#[cfg(feature = "database")]
pub mod gateway_resolver;
#[cfg(feature = "database")]
pub mod generic_executor;
#[cfg(feature = "database")]
pub mod graph_executor;
#[cfg(feature = "database")]
pub mod idempotency;
pub mod intent;
pub mod intent_extractor;
#[cfg(feature = "database")]
pub mod lsp_validator;
pub mod planning_facade;
#[cfg(feature = "database")]
pub mod ref_resolver;
pub mod repl_session;
pub mod runtime_registry;
pub mod semantic_context;
#[cfg(feature = "database")]
pub mod semantic_validator;
pub mod submission;
pub mod suggestions;
pub mod topo_sort;
pub mod validation;
pub mod verb_registry;

// Re-export local module types
pub use applicability_rules::{ApplicabilityRules, AttributeApplicability, DocumentApplicability};
#[cfg(feature = "database")]
pub use batch_executor::{
    BatchExecutionResult, BatchExecutor, BatchResultAccumulator, OnErrorMode,
};
pub use csg_linter::{CsgLinter, InferredContext, LintResult};
pub use domain_context::{ActiveDomain, DomainContext, IterationContext};
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
pub use execution_result::{ExecutionResults, StepResult};
pub use executor::{
    DagExecutionResult, DslExecutor, ExecutionContext, ExecutionResult, OpExecutionResult,
    ReturnType,
};
#[cfg(feature = "database")]
pub use executor::{IterationResult, SubmissionResult};
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
pub use planning_facade::{
    analyse_and_plan, quick_validate, ImplicitCreateMode, PlannedExecution, PlanningInput,
    PlanningOutput, SyntheticStep as FacadeSyntheticStep,
};
#[cfg(feature = "database")]
pub use ref_resolver::RefResolver;
pub use repl_session::{ExecutedBlock, ReplSession};
pub use runtime_registry::{runtime_registry, runtime_registry_arc, RuntimeVerbRegistry};
pub use semantic_context::SemanticContextStore;
#[cfg(feature = "database")]
pub use semantic_validator::{validate_dsl, validate_dsl_with_csg, SemanticValidator};
pub use submission::{
    DslSubmission, ExpandedSubmission, IterationKey, IterationStatements, SubmissionError,
    SubmissionLimits, SubmissionState, SymbolBinding,
};
pub use topo_sort::{
    emit_dsl, topological_sort, topological_sort_with_lifecycle, TopoSortError, TopoSortResult,
};
pub use verb_registry::{
    find_unified_verb, registry, verb_exists, ArgDef, UnifiedVerbDef, UnifiedVerbRegistry,
    VerbBehavior,
};

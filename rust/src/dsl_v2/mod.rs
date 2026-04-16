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
pub mod cardinality;
pub mod csg_linter;
pub mod display_nouns;

// Macro expansion (operator vocabulary layer)
pub mod domain_context;
pub mod enrichment;
pub(crate) mod entity_deps;
pub mod errors;
pub(crate) mod execution_plan;
pub mod execution_result;
pub(crate) mod executor;
pub(crate) mod expansion;
#[cfg(feature = "database")]
pub(crate) mod gateway_resolver;
#[cfg(feature = "database")]
pub(crate) mod generic_executor;
#[cfg(feature = "database")]
pub mod graph_executor;
#[cfg(feature = "database")]
pub mod idempotency;
pub mod intent;
pub mod intent_tiers;
#[cfg(feature = "database")]
pub(crate) mod lsp_validator;
pub(crate) mod macros;
pub mod operator_types;
pub(crate) mod planning_facade;
#[cfg(feature = "database")]
pub mod ref_resolver;
pub mod repl_session;
pub(crate) mod runtime_registry;
#[cfg(feature = "database")]
pub(crate) mod semantic_validator;
#[cfg(feature = "database")]
pub mod sheet_executor;
pub mod submission;
pub mod suggestions;
pub mod topo_sort;
pub(crate) mod validation;
pub mod verb_registry;
pub mod verb_taxonomy;

// Re-export local module types
pub use applicability_rules::{ApplicabilityRules, AttributeApplicability, DocumentApplicability};
#[cfg(feature = "database")]
pub use batch_executor::{
    BatchExecutionResult, BatchExecutor, BatchResultAccumulator, OnErrorMode,
};
pub use cardinality::{
    Cardinality, CardinalityConfig, CardinalityDiagnostic, CardinalityError, CardinalityRegistry,
    CardinalityValidator, RoleCardinalityDef,
};
pub use csg_linter::{CsgLinter, InferredContext, LintResult};
pub use display_nouns::{
    contains_forbidden_token, display_noun, display_noun_or_self, has_display_noun, pluralize,
    FORBIDDEN_UI_TOKENS,
};
pub use domain_context::{ActiveDomain, DomainContext, IterationContext};
pub use enrichment::{enrich_program, EnrichmentError, EnrichmentResult};
pub use execution_result::{ExecutionResults, StepResult};
#[cfg(feature = "database")]
pub use idempotency::{compute_idempotency_key, IdempotencyManager};
pub use intent::{ArgIntent, DslIntent, DslIntentBatch, ResolvedArg};
pub use operator_types::{OperatorRole, OperatorType};
#[cfg(feature = "database")]
pub use ref_resolver::RefResolver;
pub use repl_session::{ExecutedBlock, ReplSession};
#[cfg(feature = "database")]
pub use sheet_executor::SheetExecutor;
pub use submission::{
    DslSubmission, ExpandedSubmission, IterationKey, IterationStatements, SubmissionError,
    SubmissionLimits, SubmissionState, SymbolBinding,
};
pub use topo_sort::{
    emit_dsl, topological_sort, topological_sort_with_lifecycle,
    ExecutionPhase as TopoExecutionPhase, TopoSortError, TopoSortResult,
};

pub use verb_taxonomy::{
    verb_taxonomy, DomainSummary, TaxonomyCategory, TaxonomyDomain, VerbLocation, VerbTaxonomy,
};

// Re-export expansion module types (consumed externally)
#[allow(unused_imports)]
pub use expansion::{
    expand_templates, expand_templates_simple, BatchPolicy, ExpansionReport, LockAccess, LockKey,
    LockMode,
};

// Re-export error aggregation types
pub use errors::{
    AffectedVerb, CauseDetails, CausedErrors, ErrorCause, ExecutionErrors, FailureTiming,
};

// Re-export macro expansion types (consumed externally)
pub use macros::{
    load_macro_registry, load_macro_registry_from_dir, MacroRegistry,
};

/// Syntax-facing DSL seam: parse input and inspect AST/bindings.
pub mod syntax {
    pub use super::{
        parse_program, parse_single_verb, Argument, AstNode, BindingContext, BindingInfo,
        EntityRefStats, Literal, Program, Span, Statement, VerbCall,
    };
}

/// Planning-facing DSL seam: compile, analyse, and inspect dependency/planning output.
pub mod planning {
    pub use super::entity_deps::{
        entity_deps, topological_sort_unified, DependencyKind, EntityDep, EntityDependencyRegistry,
        EntityInstance, EntityTypeKey, TopoSortUnifiedError, TopoSortUnifiedResult,
    };
    pub use super::execution_plan::{
        compile, compile_with_planning, BindingInfo as PlanningBindingInfo, CompileError,
        ExecutionPlan, ExecutionStep, Injection, PlannerDiagnostic, PlanningContext,
        PlanningResult, SyntheticStep,
    };
    pub use super::planning_facade::{
        analyse_and_plan, quick_validate, ImplicitCreateMode, PlannedExecution, PlanningInput,
        PlanningOutput, SyntheticStep as FacadeSyntheticStep,
    };
}

/// Execution-facing DSL seam: execute compiled/planned work and access runtime registries.
pub mod execution {
    #[cfg(feature = "database")]
    pub use super::executor::{
        AtomicExecutionResult, BatchStatus, BestEffortExecutionResult, DslExecutor,
        ExecutionContext, ExecutionResult, IterationResult, SubmissionResult,
    };
    #[cfg(not(feature = "database"))]
    pub use super::executor::{DslExecutor, ExecutionContext, ExecutionResult};

    pub use super::executor::{DagExecutionResult, OpExecutionResult, ReturnType};
    #[cfg(feature = "database")]
    pub use super::gateway_resolver::{gateway_addr, GatewayRefResolver};
    #[cfg(feature = "database")]
    pub use super::generic_executor::{GenericCrudExecutor, GenericExecutionResult};
    pub use super::runtime_registry::{
        runtime_registry, runtime_registry_arc, RuntimeArg, RuntimeBatchPolicy, RuntimeBehavior,
        RuntimeCrudConfig, RuntimeDurableConfig, RuntimeGraphQueryConfig, RuntimeLockAccess,
        RuntimeLockMode, RuntimeLockTarget, RuntimePolicyConfig, RuntimeReturn, RuntimeVerb,
        RuntimeVerbRegistry,
    };
}

/// Tooling-facing DSL seam: diagnostics, validation, planning, and editor support.
pub mod tooling {
    pub use super::planning_facade::{
        analyse_and_plan, PlanningInput, PlanningOutput, SyntheticStep as PlanningSyntheticStep,
    };
    pub use super::runtime_registry::{RuntimeBehavior, RuntimeVerb, RuntimeVerbRegistry};
    pub use super::validation::{
        ClientType as ValidationClientType, Diagnostic as SemanticDiagnostic,
        DiagnosticCode as ValidationDiagnosticCode, Intent as ValidationIntent, RefType,
        ResolvedArg as ValidationResolvedArg, RustStyleFormatter as ValidationRustStyleFormatter,
        Severity, SourceSpan, Suggestion, ValidatedProgram, ValidatedStatement, ValidationContext,
        ValidationRequest, ValidationResult,
    };
    pub use super::verb_registry::{
        find_unified_verb, registry, verb_exists, ArgDef, UnifiedVerbDef, UnifiedVerbRegistry,
        VerbBehavior,
    };

    #[cfg(feature = "database")]
    pub use super::gateway_resolver::{gateway_addr, GatewayRefResolver};
    #[cfg(feature = "database")]
    pub use super::lsp_validator::LspValidator;
    #[cfg(feature = "database")]
    pub use super::semantic_validator::{validate_dsl, validate_dsl_with_csg, SemanticValidator};
    #[cfg(feature = "database")]
    pub use super::validation::{
        BindingInfo as ValidationBindingInfo, Diagnostic as ValidationDiagnostic,
        Severity as ValidationSeverity, SourceSpan as ValidationSourceSpan,
    };
}

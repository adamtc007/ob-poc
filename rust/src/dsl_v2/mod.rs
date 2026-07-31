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
pub use dsl_core::{
    count_entity_refs, Argument, AstNode, EntityRefStats, Literal, Program, Span, Statement,
    VerbCall,
};

pub mod ast {
    pub use dsl_core::{
        find_unresolved_ref_locations, Argument, AstNode, Literal, Program, Span, Statement,
        UnresolvedRefLocation, VerbCall,
    };
}

// Parser
pub use dsl_core::{parse_program, parse_single_verb};

pub mod parser {
    pub use dsl_core::parse_program;
}

// Binding context
pub use dsl_core::{BindingContext, BindingInfo};

pub mod binding_context {
    pub use dsl_core::{BindingContext, BindingInfo};
}

// Diagnostics
pub use dsl_core::{
    cycle_error, implicit_create_hint, missing_arg_error, undefined_symbol_error,
    unknown_verb_error, Diagnostic, DiagnosticCode, RelatedInfo, Severity, SourceSpan,
    SuggestedFix,
};

pub mod diagnostics {
    pub use dsl_core::{Diagnostic, DiagnosticCode, Severity, SourceSpan};
}

// Config types
pub use dsl_core::LookupConfig;
pub use dsl_core::{set_phrase_gen_nouns, ConfigLoader, PhraseGenNouns};

pub mod config {
    pub use dsl_core::{
        collect_declared_fqns, flatten_pack_entries, load_packs_from_dir, validate_pack_fqns,
        validate_verbs_config, wiring_check, ConfigLoader, DagWarning, StructuralError,
        ValidationContext,
    };
    pub mod types {
        pub use dsl_core::{
            ArgConfig, ArgType, CrudConfig, CrudOperation, DomainConfig, DurableConfig,
            GraphQueryOperation, HarmClass, LookupConfig, ResolutionMode, ReturnTypeConfig,
            SearchKeyConfig, VerbBehavior, VerbConfig, VerbsConfig,
        };
    }
    pub mod loader {
        pub use dsl_core::ConfigLoader;
    }
}

// Compiler — Op-free path (Phase 3 CR A4; Op enum, DAG, and VerbHandler removed)
pub mod compiler {
    pub use dsl_core::{
        compile_to_steps, CompileError as OpCompileError, CompileStep, CompiledSteps,
    };
}
pub use compiler::{compile_to_steps, CompileStep, CompiledSteps, OpCompileError};

// =============================================================================
// Local modules (require database or other dependencies not in dsl-core)
// =============================================================================

pub mod applicability_rules;
#[cfg(feature = "database")]
pub mod batch_executor;
pub mod csg_linter;

// Macro expansion (operator vocabulary layer)
pub mod domain_context;
pub mod enrichment;
pub mod errors;
pub(crate) mod execution_plan;
pub mod execution_result;
pub(crate) mod executor;
pub(crate) mod expansion;
// §9 item 9 slice 6 (2026-05-13): gateway_resolver relocated to dsl-runtime.
pub(crate) use dsl_analysis::gateway_resolver;
#[cfg(feature = "database")]
pub(crate) mod generic_executor;
#[cfg(feature = "database")]
pub mod idempotency;
// §9 item 9 slice 6 (2026-05-13): lsp_validator relocated to dsl-runtime.
pub(crate) use dsl_analysis::lsp_validator;
pub(crate) mod macros;
// §9 item 9 slice 5 (2026-05-13): planning_facade relocated to dsl-runtime.
pub(crate) use dsl_analysis::planning_facade;
// §9 item 9 slice 6 (2026-05-13): ref_resolver relocated to dsl-runtime.
pub use dsl_analysis::ref_resolver;
// §9 item 9 slice 1 (2026-05-13): runtime_registry relocated to
// dsl-runtime. Compat re-export keeps `super::runtime_registry::*`
// paths (used by the tooling + execution submodules below) and
// existing `crate::dsl_v2::runtime_registry::*` callers working.
pub(crate) use dsl_analysis::runtime_registry;
#[cfg(feature = "database")]
pub(crate) mod semantic_validator;
pub mod submission;
// §9 item 9 slice 3 (2026-05-13): suggestions relocated to dsl-runtime.
pub use dsl_analysis::suggestions;
pub mod topo_sort;
// §9 item 9 slice 4 (2026-05-13): validation relocated to dsl-runtime.
pub(crate) use dsl_analysis::validation;
// §9 item 9 slice 2 (2026-05-13): verb_registry relocated to dsl-runtime.
// Compat re-export preserves `super::verb_registry::*` (used by the
// tooling submodule) and `crate::dsl_v2::verb_registry::*` callers.
pub use dsl_analysis::verb_registry;

// Re-export local module types
// `pub` (not `pub(crate)`): the `dsl_cli` binary (`src/bin/dsl_cli.rs`) is
// a separate compilation unit and needs to name these across the crate
// boundary.
pub use csg_linter::{CsgLinter, LintResult};
pub(crate) use enrichment::enrich_program;
pub use execution_result::{StepResult};
#[cfg(feature = "database")]
pub use ref_resolver::RefResolver;
pub(crate) use submission::{DslSubmission, SubmissionLimits, SubmissionState, SymbolBinding};
// `emit_dsl`/`topological_sort` are `pub` (not `pub(crate)`): the `dsl_cli`
// binary (`src/bin/dsl_cli.rs`) is a separate compilation unit and needs to
// name them across the crate boundary.
pub use topo_sort::{emit_dsl, topological_sort};

// Re-export expansion module types (consumed externally)
#[allow(unused_imports)]
pub(crate) use expansion::{
    expand_templates, expand_templates_simple, BatchPolicy, ExpansionReport, LockAccess, LockKey,
    LockMode,
};


// Re-export macro expansion types (consumed externally)
pub use macros::{load_macro_registry, load_macro_registry_from_dir, MacroRegistry};

/// Syntax-facing DSL seam: parse input and inspect AST/bindings.
pub mod syntax {
    pub use super::{
        parse_program, parse_single_verb, Argument, AstNode, BindingContext, BindingInfo,
        EntityRefStats, Literal, Program, Span, Statement, VerbCall,
    };
}

/// Planning-facing DSL seam: compile, analyse, and inspect dependency/planning output.
pub mod planning {
    pub use super::execution_plan::{compile};
    pub(crate) use super::execution_plan::ExecutionPlan;
    // `compile_with_planning`/`PlanningBindingInfo`/`PlanningContext` are
    // `pub` (not `pub(crate)`): the `dsl_cli` binary (`src/bin/dsl_cli.rs`)
    // is a separate compilation unit and needs to name them across the
    // crate boundary.
    pub use super::execution_plan::{
        compile_with_planning, BindingInfo as PlanningBindingInfo, PlanningContext,
    };
    pub use super::planning_facade::{
        analyse_and_plan, quick_validate, ImplicitCreateMode, PlannedExecution, PlanningInput,
        PlanningOutput, SyntheticStep as FacadeSyntheticStep,
    };
}

/// Execution-facing DSL seam: execute compiled/planned work and access runtime registries.
pub mod execution {
    #[cfg(feature = "database")]
    pub use super::executor::{DslExecutor, ExecutionContext, ExecutionResult};
    pub(crate) use super::executor::{AtomicExecutionResult, BestEffortExecutionResult};
    #[cfg(not(feature = "database"))]
    pub use super::executor::{DslExecutor, ExecutionContext, ExecutionResult};

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
    pub use super::semantic_validator::{validate_dsl, validate_dsl_with_csg};
pub(crate) use super::semantic_validator::{SemanticValidator};
    #[cfg(feature = "database")]
    pub use super::validation::{
        BindingInfo as ValidationBindingInfo, Diagnostic as ValidationDiagnostic,
        Severity as ValidationSeverity, SourceSpan as ValidationSourceSpan,
    };
}

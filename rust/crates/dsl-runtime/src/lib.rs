//! DSL runtime — the execution plane of the three-plane architecture.
//!
//! See `docs/backlog/three-plane-architecture-v0.3.md` §7.1 for the scope
//! split between this crate, `sem_os_*` (control plane), and `ob-poc`
//! (composition plane).
//!
//! # Current state
//!
//! Per `docs/backlog/three-plane-architecture-implementation-plan-v0.1.md`
//! §3 Phase 2, this crate owns:
//!
//! - `VerbExecutionPort` trait (moved from sem_os_core in Phase 1).
//! - `VerbExecutionContext`, `VerbExecutionOutcome`, `VerbSideEffects`,
//!   `VerbExecutionResult` (moved from `sem_os_core::execution` in Phase 2).
//! - `CrudExecutionPort` (moved from `sem_os_core::execution` in Phase 2 to
//!   avoid a crate-graph cycle — it references `VerbExecutionContext`).
//! - `Result<T>` alias over `SemOsError` (moved from `sem_os_core::execution`).
//!
//! `sem_os_core::execution` is now an empty placeholder module; callers
//! migrate imports:
//!
//! ```text
//! // before
//! use sem_os_core::execution::VerbExecutionContext;
//! // after
//! use dsl_runtime::VerbExecutionContext;
//! ```
//!
//! # Transitional dep
//!
//! `dsl-runtime` still depends on `sem_os_core` for `Principal`,
//! `SemOsError`, and `VerbContractBody` (the CRUD port's contract
//! metadata). A future slice inverts this by either moving `Principal`
//! into a shared lower crate or introducing a dsl-runtime-local error
//! type. Phase 2 does not gate on inversion.
//!
//! # Phase 5c-migrate slice #80 cleanup
//!
//! The former `CustomOperation` trait, `CustomOpFactory`,
//! `CustomOperationRegistry`, and the scaffold `VerbRegistrar` trait were
//! all deleted once every plugin op had migrated to
//! `sem_os_postgres::ops::SemOsVerbOp`. The proc-macro crate
//! `dsl-runtime-macros` was removed in the same slice.
//!
//! # Visibility policy
//!
//! Explicit allowlist only — no wildcard `pub use`. Every new public
//! surface is added here deliberately so the plane boundary is reviewable
//! at a glance.

pub mod bods;
pub mod cross_workspace;
pub mod crud_executor;
pub mod document_bundles;
pub mod document_requirements;
pub mod domain_ops;
pub mod execution;
pub mod placeholder;
pub mod port;
// dsl-runtime-split v1 Phase 3 (2026-05-14): registry cluster relocated
// to `dsl-analysis` — verb_registry, runtime_registry, catalogue_loader,
// entity_kind. Compat re-exports keep external + intra-crate
// `crate::*` / `dsl_runtime::*` paths resolving. Removed in Phase 11.
pub use dsl_analysis::catalogue_loader;
pub use dsl_analysis::entity_kind;
pub use dsl_analysis::runtime_registry;
pub use dsl_analysis::verb_registry;
// §9 item 9 slice 3 (2026-05-13): suggestions::predict_next_steps
// relocated from rust/src/dsl_v2/. Frontier-derived "what verb makes
// sense next" recommender; pure-Rust over the verb registry.
pub mod suggestions;
// dsl-runtime-split v1 Phase 2 (2026-05-14): validation relocated to
// `dsl-analysis`. Pure-types module (Diagnostic, Severity, SourceSpan,
// ValidationContext, ValidationResult, Suggestion, ValidatedProgram,
// ValidatedStatement). Compat re-export keeps `dsl_runtime::validation::*`
// resolving for external consumers (ob-poc, dsl-lsp) and intra-crate
// `crate::validation::*` paths in gateway_resolver/ref_resolver/lsp_validator.
// Removed in Phase 11.
pub use dsl_analysis::validation;
// §9 item 9 slice 5 (2026-05-13): planning_facade relocated from
// rust/src/dsl_v2/. The analyse-and-plan orchestrator that parses
// DSL text, compiles to ops, runs DAG planning, and returns both
// diagnostics + executable plan. analyse_and_plan, PlanningInput,
// PlanningOutput, SyntheticStep, quick_validate, ImplicitCreateMode.
pub mod planning_facade;
// dsl-runtime-split v1 Phase 5 (2026-05-14): ref_resolver +
// gateway_resolver relocated to `dsl-analysis`. lsp_validator stays
// here for Phase 6; reaches the resolver pair via the compat re-exports
// below. Removed in Phase 11.
pub use dsl_analysis::gateway_resolver;
pub use dsl_analysis::ref_resolver;
pub mod lsp_validator;
// dsl-runtime-split v1 Phase 4 (2026-05-14): macros registry subset
// (schema + registry + conditions + variable + scope) relocated to
// `dsl-analysis`. Expander remains in `ob-poc`. Compat re-export
// preserves `dsl_runtime::macros::*` for dsl-lsp + ob-poc::dsl_v2::macros.
// Removed in Phase 11.
pub use dsl_analysis::macros;
pub mod service_traits;
pub mod services;
pub mod state_reducer;
pub mod stategraph;
pub mod tx;
pub mod verification;

// Explicit re-exports — do NOT add `pub use module::*`.
pub use crud_executor::PgCrudExecutor;
pub use execution::{
    Result, VerbExecutionContext, VerbExecutionOutcome, VerbExecutionResult, VerbSideEffects,
};
pub use port::{CrudExecutionPort, VerbExecutionPort};
pub use services::{ServiceRegistry, ServiceRegistryBuilder};

#[cfg(test)]
pub use port::test_support;

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
// dsl-runtime-split v1 Phase 2 (2026-05-14): validation relocated to
// `dsl-analysis` (pure-types module). Compat re-export keeps
// `dsl_runtime::validation::*` resolving. Removed in Phase 11.
pub use dsl_analysis::validation;
// dsl-runtime-split v1 Phases 5–7 (2026-05-14): resolver cluster +
// lsp_validator + suggestions + planning_facade relocated to
// `dsl-analysis`. Compat re-exports preserve all `dsl_runtime::*`
// paths for dsl-lsp and ob-poc::dsl_v2. Removed in Phase 11.
pub use dsl_analysis::gateway_resolver;
pub use dsl_analysis::lsp_validator;
pub use dsl_analysis::planning_facade;
pub use dsl_analysis::ref_resolver;
pub use dsl_analysis::suggestions;
// dsl-runtime-split v1 Phase 4 (2026-05-14): macros registry subset
// (schema + registry + conditions + variable + scope) relocated to
// `dsl-analysis`. Expander remains in `ob-poc`. Compat re-export
// preserves `dsl_runtime::macros::*` for dsl-lsp + ob-poc::dsl_v2::macros.
// Removed in Phase 11.
pub use dsl_analysis::macros;
pub mod service_traits;
pub mod services;
pub mod state_reducer;
pub mod tx;
// dsl-runtime-split v1 Phase 8 (2026-05-14): stategraph + verification
// relocated to `dsl-analysis`. Compat re-exports preserve
// `dsl_runtime::{stategraph, verification}` for sem_os_postgres::ops::
// {discovery, verify}. Removed in Phase 11.
pub use dsl_analysis::stategraph;
pub use dsl_analysis::verification;

// Explicit re-exports — do NOT add `pub use module::*`.
pub use crud_executor::PgCrudExecutor;
pub use execution::{
    Result, VerbExecutionContext, VerbExecutionOutcome, VerbExecutionResult, VerbSideEffects,
};
pub use port::{CrudExecutionPort, VerbExecutionPort};
pub use services::{ServiceRegistry, ServiceRegistryBuilder};

#[cfg(test)]
pub use port::test_support;

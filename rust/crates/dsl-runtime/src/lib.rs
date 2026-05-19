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

// dsl-runtime is now the data plane only. The 13 analyser-tier modules
// (validation, verb_registry, runtime_registry, catalogue_loader,
// entity_kind, macros, ref_resolver, gateway_resolver, lsp_validator,
// suggestions, planning_facade, stategraph, verification) live in
// `dsl-analysis`. See docs/todo/dsl-runtime-split-v1.md.
pub mod bods;
pub mod coordination;
pub mod cross_workspace;
pub mod frame;
pub mod crud_executor;
pub mod document_bundles;
pub mod document_requirements;
pub mod domain_ops;
pub mod execution;
pub mod placeholder;
pub mod port;
pub mod service_traits;
pub mod services;
pub mod state_reducer;
pub mod tx;

// Explicit re-exports — do NOT add `pub use module::*`.
pub use crud_executor::PgCrudExecutor;
pub use execution::{
    Result, VerbExecutionContext, VerbExecutionOutcome, VerbExecutionResult, VerbSideEffects,
};
pub use port::{CrudExecutionPort, VerbExecutionPort};
pub use services::{ServiceRegistry, ServiceRegistryBuilder};

#[cfg(test)]
pub use port::test_support;

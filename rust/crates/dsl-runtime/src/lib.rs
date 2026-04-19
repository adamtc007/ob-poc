//! DSL runtime — the execution plane of the three-plane architecture.
//!
//! See `docs/todo/three-plane-architecture-v0.3.md` §7.1 for the scope
//! split between this crate, `sem_os_*` (control plane), and `ob-poc`
//! (composition plane).
//!
//! # Phase 2 state
//!
//! Per `docs/todo/three-plane-architecture-implementation-plan-v0.1.md`
//! §3 Phase 2, this crate now owns:
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
//! Phase 2c will add `CustomOperation`, `CustomOperationRegistry`,
//! `CustomOpFactory`, and `VerbRegistrar`, plus the `#[register_custom_op]`
//! macro (relocating to a sibling `dsl-runtime-macros` crate).
//!
//! # Visibility policy
//!
//! Explicit allowlist only — no wildcard `pub use`. Every new public
//! surface is added here deliberately so the plane boundary is reviewable
//! at a glance.

pub mod execution;
pub mod port;
pub mod registrar;

// Explicit re-exports — do NOT add `pub use port::*` or `pub use execution::*`.
pub use execution::{
    Result, VerbExecutionContext, VerbExecutionOutcome, VerbExecutionResult, VerbSideEffects,
};
pub use port::{CrudExecutionPort, VerbExecutionPort};
pub use registrar::VerbRegistrar;

#[cfg(test)]
pub use port::test_support;

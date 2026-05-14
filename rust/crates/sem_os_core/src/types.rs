//! sem_os_core::types compat shim.
//!
//! The real definitions live in `sem_os_types`. This shim preserves
//! `sem_os_core::types::*` paths for the duration of the
//! sem_os_core-split v1 migration (Phase 2.5, 2026-05-14). It can
//! drop in Phase 12 if no consumer still uses the `sem_os_core::types`
//! path by then.

pub use sem_os_types::*;

// Back-compat re-export — the canonical 9-state ChangeSetStatus
// (Draft, UnderReview, Approved, Validated, Rejected, DryRunPassed,
// DryRunFailed, Published, Superseded) lives in the policy-tier
// `authoring` module. sem_os_types cannot reach up into authoring,
// so this re-export stays at the sem_os_core shim layer.
pub use crate::authoring::types::ChangeSetStatus as ChangesetStatus;

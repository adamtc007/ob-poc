//! Test-only fixtures for the Phase 5c-migrate / 5d arc.
//!
//! Today the codebase has no plugin op that produces a non-empty
//! [`ob_poc_types::PendingStateAdvance`]. The §9a atomic-commit
//! invariant gate ("rollback if apply-`PendingStateAdvance` fails")
//! cannot be exercised without one. These fixtures supply the
//! missing producer surface so harness-level tests can drive the
//! full stage-8 → stage-9a path before any production op grows the
//! capability.
//!
//! # Why a separate `test_fixtures` module instead of `#[cfg(test)]`
//!
//! `#[cfg(test)]` only compiles inside the crate's own test binary.
//! Phase 5d (TOCTOU recheck) and Phase 5e (replay-safety) want to
//! exercise the producer path from integration tests in **ob-poc**'s
//! `tests/` directory. A pub module gated by a `test-fixtures`
//! feature would be cleaner long-term; for now the module is plain
//! `pub` and the fixtures opt out of the dead-code lint when
//! production builds don't reference them.
//!
//! # Status (Phase 5c-migrate Session 1)
//!
//! Session 1 ships the **producer-side data shape only** — a typed
//! factory that returns a non-empty `PendingStateAdvance` and a
//! tagged DAG node id for assertion. The wiring into a synthetic
//! `CustomOperation` impl lands in Session 6 alongside the stage-9a
//! commit-invariant test fixture; the producer surface here is what
//! that fixture will adopt.

pub mod pending_state_advance;

pub use pending_state_advance::{
    fixture_pending_state_advance, FIXTURE_DAG_NODE_ID, FIXTURE_ENTITY_ID,
};

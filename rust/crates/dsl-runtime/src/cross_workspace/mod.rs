//! Cross-workspace state consistency — shared atom registry, staleness propagation,
//! constellation replay, remediation lifecycle, external call idempotency, and
//! platform DAG derivation.
//!
//! See: `docs/architecture/cross-workspace-state-consistency-v0.4.md`
//!
//! Relocated from ob-poc to dsl-runtime in Phase 5a composite-blocker #2
//! (2026-04-20). The ob-poc-side `crate::repl::types_v2::WorkspaceKind`
//! dependency was widened to `String` (same snake_case serde repr) to
//! keep this module plane-neutral. dsl-runtime has sqlx unconditionally,
//! so the `#[cfg(feature = "database")]` gates around DB-backed
//! submodules (legacy from ob-poc's conditional feature) are dropped.

pub mod compensation;
pub mod fact_refs;
pub mod fact_versions;
pub mod idempotency;
pub mod platform_dag;
pub mod providers;
pub mod remediation;
pub mod replay;
pub mod repository;
pub mod types;

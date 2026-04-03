//! Cross-workspace state consistency — shared atom registry, staleness propagation,
//! constellation replay, remediation lifecycle, external call idempotency, and
//! platform DAG derivation.
//!
//! See: `docs/architecture/cross-workspace-state-consistency-v0.4.md`

pub mod types;

#[cfg(feature = "database")]
pub mod repository;

#[cfg(feature = "database")]
pub mod fact_versions;

#[cfg(feature = "database")]
pub mod fact_refs;

pub mod replay;

#[cfg(feature = "database")]
pub mod remediation;

#[cfg(feature = "database")]
pub mod idempotency;

#[cfg(feature = "database")]
pub mod providers;

#[cfg(feature = "database")]
pub mod compensation;

pub mod platform_dag;

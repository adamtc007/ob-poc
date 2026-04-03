//! Cross-workspace state consistency — shared atom registry, staleness propagation,
//! constellation replay, and remediation lifecycle.
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

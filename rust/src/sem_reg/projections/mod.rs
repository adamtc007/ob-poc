//! Phase 9 — Lineage, Embeddings, and Coverage Metrics
//!
//! Derived projections for impact analysis, semantic search, and governance dashboards.
//! All records in this module are append-only / versioned — no in-place updates.

pub mod embeddings;
pub mod lineage;
pub mod metrics;

#[cfg(test)]
pub(crate) use lineage::LineageStore;
pub use metrics::{CoverageReport, MetricsStore};

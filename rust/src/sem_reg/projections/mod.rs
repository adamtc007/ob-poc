//! Phase 9 — Lineage, Embeddings, and Coverage Metrics
//!
//! Derived projections for impact analysis, semantic search, and governance dashboards.
//! All records in this module are append-only / versioned — no in-place updates.

pub mod embeddings;
pub mod lineage;
pub mod metrics;

pub use embeddings::{EmbeddingRecord, EmbeddingStore, SemanticText};
pub use lineage::{DerivationEdge, LineageDirection, LineageStore, RunRecord};
pub use metrics::{CoverageReport, MetricsStore, TierDistribution};

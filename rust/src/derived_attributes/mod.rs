//! Canonical persistence plane for below-the-line derived attributes.
//!
//! This module owns the durable write/read contract for derived values and
//! their lineage dependencies.

pub mod repository;

pub use repository::{
    acquire_derivation_lock, compute_content_hash, get_current, get_direct_dependencies,
    get_entity_scoped_impact, get_latest, get_recompute_queue, get_reverse_impact,
    get_transitive_closure, insert_dependencies, insert_derived_value, mark_stale,
    mark_stale_by_input, mark_stale_by_spec, supersede_current, BatchRecomputeResult,
    ContentHashInput, DependencyRow, DependencyRowInput, DerivedValueRow, DerivedValueRowInput,
};

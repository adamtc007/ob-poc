//! Pure seed data builders for the semantic registry.
//!
//! These modules produce typed seed data from pure functions (no DB, no I/O).
//! The DB-publishing orchestrators remain in `ob-poc/src/sem_reg/seeds/`.

pub mod derivation_seeds;
pub mod policy_seeds;
pub mod taxonomy_seeds;
pub mod view_seeds;

pub use derivation_seeds::core_derivation_specs;
pub use policy_seeds::core_policies;
pub use taxonomy_seeds::core_taxonomies;
pub use view_seeds::core_views;

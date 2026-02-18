//! Bootstrap seeds for taxonomy definitions, view definitions, policy rules,
//! and derivation specs.
//!
//! These seed functions are called by the scanner to populate the semantic registry
//! with core taxonomies, views, policies, and derivation specs derived from the
//! existing domain structure and governance patterns.

pub mod derivation_seeds;
pub mod policy_seeds;
pub mod taxonomy_seeds;
pub mod view_seeds;

pub use derivation_seeds::{seed_derivation_specs, DerivationSeedReport};
pub use policy_seeds::{seed_policies, PolicySeedReport};
pub use taxonomy_seeds::{seed_taxonomies, TaxonomySeedReport};
pub use view_seeds::{seed_views, ViewSeedReport};

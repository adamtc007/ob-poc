//! Bootstrap seeds for taxonomy definitions, view definitions, policy rules,
//! and derivation specs.
//!
//! These seed functions are called by the scanner to populate the semantic registry
//! with core taxonomies, views, policies, and derivation specs derived from the
//! existing domain structure and governance patterns.

pub mod derivation_seeds;
pub mod membership_rule_seeds;
pub mod policy_seeds;
pub mod taxonomy_seeds;
pub mod view_seeds;

pub use derivation_seeds::{seed_derivation_specs};
pub(crate) use derivation_seeds::{DerivationSeedReport};
pub use membership_rule_seeds::{seed_kyc_membership_rules};
pub(crate) use membership_rule_seeds::{MembershipRuleSeedReport};
pub use policy_seeds::{seed_policies};
pub(crate) use policy_seeds::{PolicySeedReport};
pub use taxonomy_seeds::{seed_taxonomies};
pub(crate) use taxonomy_seeds::{TaxonomySeedReport};
pub use view_seeds::{seed_views};
pub(crate) use view_seeds::{ViewSeedReport};

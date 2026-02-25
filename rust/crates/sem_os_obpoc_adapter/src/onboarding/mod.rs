//! Pure onboarding validation and default generation.
//!
//! These modules provide structural validation and default generation
//! for the onboarding pipeline. The DB-dependent pipeline orchestrator
//! remains in `ob-poc/src/sem_reg/onboarding/pipeline.rs`.

pub mod defaults;
pub mod validators;

pub use defaults::{
    columns_for_entity_in_view, default_attributes_for_entity_type,
    default_taxonomy_fqns_for_entity_type, default_verb_contracts_for_entity_type,
    default_view_fqns_for_entity_type, membership_rule_for_entity_in_taxonomy,
};
pub use validators::{validate_request, OnboardingRequest};

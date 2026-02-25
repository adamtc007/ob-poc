//! sem_os_obpoc_adapter — bridges ob-poc config to Semantic OS seed bundles.
//!
//! Pure conversion layer: takes ob-poc verb YAML config and produces a
//! `SeedBundle` that the Semantic OS core can bootstrap from.
//!
//! - `scanner` — verb-first YAML scanner (pure conversion functions)
//! - `seeds` — taxonomy, view, policy, derivation spec seed builders
//! - `onboarding` — request validation and default generation

pub mod onboarding;
pub mod scanner;
pub mod seeds;

use dsl_core::config::types::VerbsConfig;
use sem_os_core::seeds::{
    AttributeSeed, EntityTypeSeed, PolicySeed, SeedBundle, TaxonomySeed, VerbContractSeed, ViewSeed,
};

/// Build a complete `SeedBundle` from ob-poc verb configuration.
///
/// This is the main entry point for the adapter. It:
/// 1. Scans verb YAML to extract verb contracts, inferred entity types, and attributes
/// 2. Collects taxonomy, policy, view, and derivation spec seeds from pure builders
/// 3. Serializes everything into seed DTOs with `serde_json::to_value`
/// 4. Computes a deterministic SHA-256 bundle hash for idempotent bootstrap
pub fn build_seed_bundle(verbs_config: &VerbsConfig) -> SeedBundle {
    // 1. Scan verb configs → typed bodies
    let verb_contract_bodies = scanner::scan_verb_contracts(verbs_config);
    let attribute_bodies = scanner::infer_attributes_from_verbs(verbs_config);
    let entity_type_bodies = scanner::infer_entity_types_from_verbs(verbs_config);

    // 2. Collect seed data from pure builders
    let taxonomy_pairs = seeds::core_taxonomies();
    let policy_bodies = seeds::core_policies();
    let view_bodies = seeds::core_views();
    // Note: derivation specs are included as policies (they have their own object type
    // in the registry but are not a separate SeedBundle field). They will be handled
    // by the bootstrap endpoint on the server side. For now we skip them in the bundle
    // since SeedBundle doesn't have a derivations field.

    // 3. Serialize into seed DTOs
    let verb_contracts: Vec<VerbContractSeed> = verb_contract_bodies
        .iter()
        .map(|body| VerbContractSeed {
            fqn: body.fqn.clone(),
            payload: serde_json::to_value(body).expect("VerbContractBody must serialize"),
        })
        .collect();

    let attributes: Vec<AttributeSeed> = attribute_bodies
        .iter()
        .map(|body| AttributeSeed {
            fqn: body.fqn.clone(),
            payload: serde_json::to_value(body).expect("AttributeDefBody must serialize"),
        })
        .collect();

    let entity_types: Vec<EntityTypeSeed> = entity_type_bodies
        .iter()
        .map(|body| EntityTypeSeed {
            fqn: body.fqn.clone(),
            payload: serde_json::to_value(body).expect("EntityTypeDefBody must serialize"),
        })
        .collect();

    let taxonomies: Vec<TaxonomySeed> = taxonomy_pairs
        .iter()
        .map(|(tax_body, _nodes)| TaxonomySeed {
            fqn: tax_body.fqn.clone(),
            payload: serde_json::to_value(tax_body).expect("TaxonomyDefBody must serialize"),
        })
        .collect();

    let policies: Vec<PolicySeed> = policy_bodies
        .iter()
        .map(|body| PolicySeed {
            fqn: body.fqn.clone(),
            payload: serde_json::to_value(body).expect("PolicyRuleBody must serialize"),
        })
        .collect();

    let views: Vec<ViewSeed> = view_bodies
        .iter()
        .map(|body| ViewSeed {
            fqn: body.fqn.clone(),
            payload: serde_json::to_value(body).expect("ViewDefBody must serialize"),
        })
        .collect();

    // 4. Compute deterministic bundle hash
    let bundle_hash = SeedBundle::compute_hash(
        &verb_contracts,
        &attributes,
        &entity_types,
        &taxonomies,
        &policies,
        &views,
    )
    .expect("SeedBundle canonical JSON serialization should never fail");

    SeedBundle {
        bundle_hash,
        verb_contracts,
        attributes,
        entity_types,
        taxonomies,
        policies,
        views,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::config::types::*;
    use std::collections::HashMap;

    fn minimal_verbs_config() -> VerbsConfig {
        let mut domains = HashMap::new();
        domains.insert(
            "cbu".into(),
            DomainConfig {
                description: "CBU domain".into(),
                verbs: {
                    let mut v = HashMap::new();
                    v.insert(
                        "create".into(),
                        VerbConfig {
                            description: "Create a CBU".into(),
                            behavior: VerbBehavior::Plugin,
                            crud: None,
                            handler: Some("CbuCreateOp".into()),
                            graph_query: None,
                            durable: None,
                            args: vec![ArgConfig {
                                name: "name".into(),
                                arg_type: ArgType::String,
                                required: true,
                                maps_to: Some("name".into()),
                                lookup: None,
                                valid_values: None,
                                default: None,
                                description: Some("CBU name".into()),
                                validation: None,
                                fuzzy_check: None,
                                slot_type: None,
                                preferred_roles: vec![],
                            }],
                            returns: None,
                            produces: Some(VerbProduces {
                                produced_type: "cbu".into(),
                                subtype: None,
                                subtype_from_arg: None,
                                resolved: false,
                                initial_state: None,
                            }),
                            consumes: vec![],
                            lifecycle: None,
                            metadata: None,
                            invocation_phrases: vec!["create cbu".into()],
                            policy: None,
                            sentences: None,
                            confirm_policy: None,
                        },
                    );
                    v
                },
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );

        VerbsConfig {
            version: "1.0".into(),
            domains,
        }
    }

    #[test]
    fn test_build_seed_bundle_produces_non_empty() {
        let config = minimal_verbs_config();
        let bundle = build_seed_bundle(&config);

        assert!(!bundle.bundle_hash.is_empty());
        assert!(bundle.bundle_hash.starts_with("v1:"));
        assert!(!bundle.verb_contracts.is_empty());
        assert!(!bundle.attributes.is_empty());
        assert!(!bundle.taxonomies.is_empty());
        assert!(!bundle.policies.is_empty());
        assert!(!bundle.views.is_empty());
    }

    #[test]
    fn test_build_seed_bundle_deterministic() {
        let config = minimal_verbs_config();
        let bundle1 = build_seed_bundle(&config);
        let bundle2 = build_seed_bundle(&config);

        assert_eq!(bundle1.bundle_hash, bundle2.bundle_hash);
        assert_eq!(bundle1.verb_contracts.len(), bundle2.verb_contracts.len());
    }

    #[test]
    fn test_verb_contracts_have_valid_fqns() {
        let config = minimal_verbs_config();
        let bundle = build_seed_bundle(&config);

        for vc in &bundle.verb_contracts {
            assert!(
                vc.fqn.contains('.'),
                "FQN must be domain.action: {}",
                vc.fqn
            );
        }
    }

    #[test]
    fn test_taxonomy_seeds_are_present() {
        let config = minimal_verbs_config();
        let bundle = build_seed_bundle(&config);

        // Should have 9 taxonomies from core_taxonomies()
        assert_eq!(bundle.taxonomies.len(), 9);
        let fqns: Vec<&str> = bundle.taxonomies.iter().map(|t| t.fqn.as_str()).collect();
        assert!(fqns.contains(&"taxonomy.entity-classification"));
        assert!(fqns.contains(&"taxonomy.jurisdiction"));
        assert!(fqns.contains(&"taxonomy.instrument-class"));
    }

    #[test]
    fn test_policy_seeds_are_present() {
        let config = minimal_verbs_config();
        let bundle = build_seed_bundle(&config);

        // Should have 8 policies from core_policies()
        assert_eq!(bundle.policies.len(), 8);
    }

    #[test]
    fn test_view_seeds_are_present() {
        let config = minimal_verbs_config();
        let bundle = build_seed_bundle(&config);

        // Should have 4 views from core_views()
        assert_eq!(bundle.views.len(), 4);
    }
}

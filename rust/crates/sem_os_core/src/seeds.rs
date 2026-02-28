//! SeedBundle — canonical, hashable bootstrap payload.
//! The adapter (sem_os_obpoc_adapter) builds a SeedBundle from ob-poc config;
//! the core crate owns the type and the hashing logic.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedBundle {
    /// SHA-256 of the canonical JSON serialisation of this bundle (fields sorted, deterministic).
    /// Computed by the adapter via `SeedBundle::compute_hash()`.
    /// Used as the idempotency key on POST /bootstrap/seed_bundle.
    /// Prefixed with "v1:" to allow future hash algorithm migration.
    pub bundle_hash: String,
    pub verb_contracts: Vec<VerbContractSeed>,
    pub attributes: Vec<AttributeSeed>,
    pub entity_types: Vec<EntityTypeSeed>,
    pub taxonomies: Vec<TaxonomySeed>,
    pub policies: Vec<PolicySeed>,
    pub views: Vec<ViewSeed>,
    #[serde(default)]
    pub derivation_specs: Vec<DerivationSpecSeed>,
}

impl SeedBundle {
    /// Compute a stable, version-prefixed SHA-256 hash of the bundle contents.
    /// Sort all vecs by their FQN field before hashing to ensure determinism
    /// regardless of source ordering.
    ///
    /// Returns `Err` if canonical JSON serialization fails (should not happen
    /// in practice, but avoids a panic in production).
    pub fn compute_hash(
        verb_contracts: &[VerbContractSeed],
        attributes: &[AttributeSeed],
        entity_types: &[EntityTypeSeed],
        taxonomies: &[TaxonomySeed],
        policies: &[PolicySeed],
        views: &[ViewSeed],
        derivation_specs: &[DerivationSpecSeed],
    ) -> std::result::Result<String, serde_json::Error> {
        #[derive(Serialize)]
        struct Canonical<'a> {
            verb_contracts: Vec<&'a VerbContractSeed>,
            attributes: Vec<&'a AttributeSeed>,
            entity_types: Vec<&'a EntityTypeSeed>,
            taxonomies: Vec<&'a TaxonomySeed>,
            policies: Vec<&'a PolicySeed>,
            views: Vec<&'a ViewSeed>,
            derivation_specs: Vec<&'a DerivationSpecSeed>,
        }

        let mut vc: Vec<&VerbContractSeed> = verb_contracts.iter().collect();
        vc.sort_by_key(|s| &s.fqn);

        let mut at: Vec<&AttributeSeed> = attributes.iter().collect();
        at.sort_by_key(|s| &s.fqn);

        let mut et: Vec<&EntityTypeSeed> = entity_types.iter().collect();
        et.sort_by_key(|s| &s.fqn);

        let mut tx: Vec<&TaxonomySeed> = taxonomies.iter().collect();
        tx.sort_by_key(|s| &s.fqn);

        let mut po: Vec<&PolicySeed> = policies.iter().collect();
        po.sort_by_key(|s| &s.fqn);

        let mut vi: Vec<&ViewSeed> = views.iter().collect();
        vi.sort_by_key(|s| &s.fqn);

        let mut ds: Vec<&DerivationSpecSeed> = derivation_specs.iter().collect();
        ds.sort_by_key(|s| &s.fqn);

        let canonical = Canonical {
            verb_contracts: vc,
            attributes: at,
            entity_types: et,
            taxonomies: tx,
            policies: po,
            views: vi,
            derivation_specs: ds,
        };
        let json = serde_json::to_string(&canonical)?;
        let hash = Sha256::digest(json.as_bytes());
        Ok(format!("v1:{}", hex::encode(hash)))
    }
}

// Seed DTOs — pure data, no SQLx derives, no ob-poc config types.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContractSeed {
    pub fqn: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSeed {
    pub fqn: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeSeed {
    pub fqn: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomySeed {
    pub fqn: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySeed {
    pub fqn: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSeed {
    pub fqn: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationSpecSeed {
    pub fqn: String,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_verb(fqn: &str) -> VerbContractSeed {
        VerbContractSeed {
            fqn: fqn.into(),
            payload: json!({"desc": fqn}),
        }
    }

    fn make_attr(fqn: &str) -> AttributeSeed {
        AttributeSeed {
            fqn: fqn.into(),
            payload: json!({"type": "string"}),
        }
    }

    fn empty_hash(verbs: &[VerbContractSeed], attrs: &[AttributeSeed]) -> String {
        SeedBundle::compute_hash(verbs, attrs, &[], &[], &[], &[], &[]).unwrap()
    }

    #[test]
    fn compute_hash_determinism() {
        let verbs = vec![make_verb("cbu.create"), make_verb("cbu.delete")];
        let attrs = vec![make_attr("cbu.name")];
        let h1 = empty_hash(&verbs, &attrs);
        let h2 = empty_hash(&verbs, &attrs);
        assert_eq!(h1, h2);
    }

    #[test]
    fn compute_hash_order_independence() {
        let v_ab = vec![make_verb("a.verb"), make_verb("b.verb")];
        let v_ba = vec![make_verb("b.verb"), make_verb("a.verb")];
        let h_ab = empty_hash(&v_ab, &[]);
        let h_ba = empty_hash(&v_ba, &[]);
        assert_eq!(h_ab, h_ba, "hash must be order-independent (sorted by fqn)");
    }

    #[test]
    fn compute_hash_has_v1_prefix() {
        let h = empty_hash(&[], &[]);
        assert!(h.starts_with("v1:"), "expected v1: prefix, got: {h}");
        // SHA-256 hex = 64 chars, plus "v1:" = 67
        assert_eq!(h.len(), 67);
    }

    #[test]
    fn compute_hash_changes_with_content() {
        let h_empty = empty_hash(&[], &[]);
        let h_one = empty_hash(&[make_verb("x.y")], &[]);
        assert_ne!(
            h_empty, h_one,
            "different content must produce different hash"
        );
    }

    #[test]
    fn seed_bundle_serde_round_trip() {
        let bundle = SeedBundle {
            bundle_hash: "v1:abc123".into(),
            verb_contracts: vec![make_verb("cbu.create")],
            attributes: vec![make_attr("cbu.name")],
            entity_types: vec![EntityTypeSeed {
                fqn: "cbu".into(),
                payload: json!({}),
            }],
            taxonomies: vec![TaxonomySeed {
                fqn: "domain.kyc".into(),
                payload: json!({}),
            }],
            policies: vec![PolicySeed {
                fqn: "p.rule".into(),
                payload: json!({}),
            }],
            views: vec![ViewSeed {
                fqn: "v.default".into(),
                payload: json!({}),
            }],
            derivation_specs: vec![DerivationSpecSeed {
                fqn: "d.spec".into(),
                payload: json!({}),
            }],
        };
        let json_str = serde_json::to_string(&bundle).unwrap();
        let restored: SeedBundle = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.bundle_hash, bundle.bundle_hash);
        assert_eq!(restored.verb_contracts.len(), 1);
        assert_eq!(restored.attributes.len(), 1);
        assert_eq!(restored.entity_types.len(), 1);
        assert_eq!(restored.taxonomies.len(), 1);
        assert_eq!(restored.policies.len(), 1);
        assert_eq!(restored.views.len(), 1);
        assert_eq!(restored.derivation_specs.len(), 1);
    }

    #[test]
    fn individual_seed_types_serde_round_trip() {
        let seeds: Vec<serde_json::Value> = vec![
            serde_json::to_value(VerbContractSeed {
                fqn: "v.c".into(),
                payload: json!(1),
            })
            .unwrap(),
            serde_json::to_value(AttributeSeed {
                fqn: "a.s".into(),
                payload: json!(2),
            })
            .unwrap(),
            serde_json::to_value(EntityTypeSeed {
                fqn: "e.t".into(),
                payload: json!(3),
            })
            .unwrap(),
            serde_json::to_value(TaxonomySeed {
                fqn: "t.x".into(),
                payload: json!(4),
            })
            .unwrap(),
            serde_json::to_value(PolicySeed {
                fqn: "p.r".into(),
                payload: json!(5),
            })
            .unwrap(),
            serde_json::to_value(ViewSeed {
                fqn: "v.d".into(),
                payload: json!(6),
            })
            .unwrap(),
            serde_json::to_value(DerivationSpecSeed {
                fqn: "d.s".into(),
                payload: json!(7),
            })
            .unwrap(),
        ];
        // Deserialize each back to its concrete type
        let _: VerbContractSeed = serde_json::from_value(seeds[0].clone()).unwrap();
        let _: AttributeSeed = serde_json::from_value(seeds[1].clone()).unwrap();
        let _: EntityTypeSeed = serde_json::from_value(seeds[2].clone()).unwrap();
        let _: TaxonomySeed = serde_json::from_value(seeds[3].clone()).unwrap();
        let _: PolicySeed = serde_json::from_value(seeds[4].clone()).unwrap();
        let _: ViewSeed = serde_json::from_value(seeds[5].clone()).unwrap();
        let _: DerivationSpecSeed = serde_json::from_value(seeds[6].clone()).unwrap();
    }
}

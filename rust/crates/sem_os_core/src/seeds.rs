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
    ) -> std::result::Result<String, serde_json::Error> {
        #[derive(Serialize)]
        struct Canonical<'a> {
            verb_contracts: Vec<&'a VerbContractSeed>,
            attributes: Vec<&'a AttributeSeed>,
            entity_types: Vec<&'a EntityTypeSeed>,
            taxonomies: Vec<&'a TaxonomySeed>,
            policies: Vec<&'a PolicySeed>,
            views: Vec<&'a ViewSeed>,
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

        let canonical = Canonical {
            verb_contracts: vc,
            attributes: at,
            entity_types: et,
            taxonomies: tx,
            policies: po,
            views: vi,
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

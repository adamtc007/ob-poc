//! Deterministic ACP pack context envelope v2.
//!
//! Gate D starts here: this module turns the read-only Slice 1 registry
//! projection into a per-pack envelope that can be rebuilt byte-for-byte,
//! budget checked, signed, and verified without touching runtime execution.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::acp_registry_projection::{
    AcpRegistryPackProjection, AcpRegistryProjection, AcpVerbEffectProjection,
    AcpWorkbookPlanProjection,
};

/// Schema version for deterministic ACP pack context envelopes.
///
/// **v3** (R2a, 2026-05-11): replaces the three sections
/// `verb_bindings` / `verb_effects` / `macro_tiers` with one unified
/// `dsl_atoms` section. See `r1-schema-parity-adr.md` for the
/// architectural framing — macros and verbs are peers on the Sage/ACP
/// visibility surface (planning + compilation only); REPL execution
/// remains verb-only.
pub const ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION: &str = "acp_pack_context_envelope_v3";

/// Schema version for deterministic ACP pack context envelope bundles.
pub const ACP_PACK_CONTEXT_ENVELOPE_V2_BUNDLE_SCHEMA_VERSION: &str =
    "acp_pack_context_envelope_v3_bundle";

/// Schema version for persisted ACP pack context registry state.
pub const ACP_PACK_CONTEXT_REGISTRY_STATE_V2_SCHEMA_VERSION: &str =
    "acp_pack_context_registry_state_v3";

/// Builder version pinned into every envelope build.
pub const ACP_PACK_CONTEXT_ENVELOPE_BUILDER_VERSION: &str = "gate-d-builder-v0.2-r2a";

/// Deterministic development signer key id.
pub const ACP_PACK_CONTEXT_DEV_SIGNING_KEY_ID: &str = "acp-pack-context-dev-key-v1";

/// ACP pack context signature algorithm.
pub const ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM: &str = "hmac-sha256-v1";

/// Environment variable for the production ACP pack context signing key id.
pub const ACP_PACK_CONTEXT_SIGNING_KEY_ID_ENV: &str = "ACP_PACK_CONTEXT_SIGNING_KEY_ID";

/// Environment variable for the production ACP pack context signing key bytes as hex.
pub const ACP_PACK_CONTEXT_SIGNING_KEY_HEX_ENV: &str = "ACP_PACK_CONTEXT_SIGNING_KEY_HEX";

/// Lifecycle state for an ACP pack context envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpPackLifecycleState {
    Draft,
    Active,
    Deprecated,
    Retired,
}

/// Top-level deterministic ACP pack context envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextEnvelopeV2 {
    pub schema_version: String,
    pub envelope_hash: String,
    pub signature: Option<AcpPackContextEnvelopeSignature>,
    pub body: AcpPackContextEnvelopeBodyV2,
}

/// Deterministic bundle of ACP pack context envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextEnvelopeBundleV2 {
    pub schema_version: String,
    pub pack_count: usize,
    pub envelopes: Vec<AcpPackContextEnvelopeV2>,
}

/// Persisted registry state for signed ACP pack context envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextRegistryStateV2 {
    pub schema_version: String,
    pub source_projection_hash: String,
    pub pack_count: usize,
    pub envelopes: Vec<AcpPackContextEnvelopeV2>,
}

/// Registry state load mode for online verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpPackContextRegistryLoadMode {
    Development,
    Production,
}

/// Options for loading ACP pack context registry state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPackContextRegistryLoadOptions {
    pub mode: AcpPackContextRegistryLoadMode,
    pub state_path: Option<PathBuf>,
}

impl AcpPackContextRegistryLoadOptions {
    /// Build development load options.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::{
    ///     AcpPackContextRegistryLoadMode, AcpPackContextRegistryLoadOptions,
    /// };
    ///
    /// let options = AcpPackContextRegistryLoadOptions::development();
    /// assert_eq!(options.mode, AcpPackContextRegistryLoadMode::Development);
    /// assert!(options.state_path.is_none());
    /// ```
    pub fn development() -> Self {
        Self {
            mode: AcpPackContextRegistryLoadMode::Development,
            state_path: None,
        }
    }

    /// Build production load options.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::{
    ///     AcpPackContextRegistryLoadMode, AcpPackContextRegistryLoadOptions,
    /// };
    ///
    /// let options = AcpPackContextRegistryLoadOptions::production("/tmp/acp-pack-state.json");
    /// assert_eq!(options.mode, AcpPackContextRegistryLoadMode::Production);
    /// assert!(options.state_path.is_some());
    /// ```
    pub fn production(path: impl Into<PathBuf>) -> Self {
        Self {
            mode: AcpPackContextRegistryLoadMode::Production,
            state_path: Some(path.into()),
        }
    }
}

/// Signed, hashable envelope body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextEnvelopeBodyV2 {
    pub builder_version: String,
    pub lifecycle: AcpPackLifecycleState,
    pub pack_id: String,
    pub pack_name: String,
    pub pack_version: String,
    pub manifest_hash: String,
    pub build_inputs: AcpPackContextBuildInputs,
    pub budget: AcpPackContextBudgetReport,
    pub sections: AcpPackContextEnvelopeSections,
    pub section_hashes: BTreeMap<String, String>,
    pub content_hash_chain: Vec<String>,
}

/// Pinned build input hashes used to prove deterministic rebuild inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextBuildInputs {
    pub source_projection_schema: String,
    pub source_projection_hash: String,
    pub semos_dsl_hash: String,
    pub governed_config_artifact_hash: String,
    pub registered_fixture_hash: String,
    pub builder_lockfile: String,
}

/// Budget report for envelope and section byte/token limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextBudgetReport {
    pub envelope_byte_limit: usize,
    pub envelope_token_limit: usize,
    pub envelope_bytes: usize,
    pub envelope_token_estimate: usize,
    pub section_reports: Vec<AcpPackContextSectionBudgetReport>,
    pub omitted: Vec<AcpPackContextOmission>,
}

/// Budget report for one envelope section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextSectionBudgetReport {
    pub section: String,
    pub byte_limit: usize,
    pub token_limit: usize,
    pub bytes: usize,
    pub token_estimate: usize,
    pub omitted: bool,
}

/// Deterministic omission record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextOmission {
    pub section: String,
    pub reason: String,
    pub byte_limit: usize,
    pub actual_bytes: usize,
}

/// Envelope sections split for budget accounting and future context assembly.
///
/// **v3 (R2a):** `verb_bindings`, `verb_effects`, and `macro_tiers` were
/// replaced by a single `dsl_atoms` section carrying the kind-agnostic
/// visibility projection. See `r1-schema-parity-adr.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextEnvelopeSections {
    pub pack_summary: serde_json::Value,
    /// R2a unified visibility surface — verbs + macros as DSL atoms.
    /// Replaces v2's separate verb_bindings / verb_effects / macro_tiers
    /// sections. Each atom carries a `dispatch_type: verb | macro`
    /// discriminator. The full ordered macro `expands_to` body is NOT
    /// projected here; see `AcpDslAtomExpansionSummary` for the
    /// redacted summary the agent reads.
    pub dsl_atoms: serde_json::Value,
    pub production_contracts: serde_json::Value,
    pub workbook_plans: serde_json::Value,
    pub diagnostic_taxonomy: serde_json::Value,
    /// R2b — v0.5 §14 neighbour hints (this pack's outbound edges only).
    pub pack_neighbours: serde_json::Value,
    /// R2b — v0.5 §14 known collision routing policy.
    pub known_collision_policy: serde_json::Value,
    /// R2b — v0.5 §7.8 cross-DAG handoff refs (outbound from this pack only).
    pub cross_dag_handoffs: serde_json::Value,
    /// R2b — v0.5 §15 canonical example utterances (positive + negative
    /// shapes), filtered to this pack.
    pub example_utterances: serde_json::Value,
}

/// Normalized return/produce contract derived from verb effect metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackProductionContract {
    pub verb: String,
    pub exposure: String,
    pub return_type: Option<String>,
    pub produces_entity_grain: Option<String>,
    pub read_entity_grains: Vec<String>,
    pub write_entity_grains: Vec<String>,
    pub side_effects: Option<String>,
    pub policy_grade: String,
    pub contract_hash: String,
}

/// Signature attached to an envelope body hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextEnvelopeSignature {
    pub algorithm: String,
    pub key_id: String,
    pub signed_hash: String,
    pub signature: String,
}

/// Key material used to sign or verify ACP pack context envelopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPackContextSigningKey {
    key_id: String,
    algorithm: String,
    key_material: Vec<u8>,
}

impl AcpPackContextSigningKey {
    /// Build a signing key from explicit key material bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::{
    ///     AcpPackContextSigningKey, ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
    /// };
    ///
    /// let key = AcpPackContextSigningKey::new(
    ///     "acp-pack-context-prod-key-v1",
    ///     ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
    ///     b"fixture-key-material",
    /// );
    /// assert_eq!(key.key_id(), "acp-pack-context-prod-key-v1");
    /// ```
    pub fn new(
        key_id: impl Into<String>,
        algorithm: impl Into<String>,
        key_material: impl AsRef<[u8]>,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            algorithm: algorithm.into(),
            key_material: key_material.as_ref().to_vec(),
        }
    }

    /// Return the key id.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::AcpPackContextSigningKey;
    ///
    /// let key = AcpPackContextSigningKey::new("key-1", "hmac-sha256-v1", b"material");
    /// assert_eq!(key.key_id(), "key-1");
    /// ```
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Return the signing algorithm.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::AcpPackContextSigningKey;
    ///
    /// let key = AcpPackContextSigningKey::new("key-1", "hmac-sha256-v1", b"material");
    /// assert_eq!(key.algorithm(), "hmac-sha256-v1");
    /// ```
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }
}

/// Verification keyring for ACP pack context envelope signatures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPackContextSigningKeyring {
    keys: Vec<AcpPackContextSigningKey>,
}

impl AcpPackContextSigningKeyring {
    /// Build a keyring from explicit signing keys.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::{
    ///     AcpPackContextSigningKey, AcpPackContextSigningKeyring,
    /// };
    ///
    /// let keyring = AcpPackContextSigningKeyring::new(vec![
    ///     AcpPackContextSigningKey::new("key-1", "hmac-sha256-v1", b"material"),
    /// ]);
    /// assert_eq!(keyring.key_count(), 1);
    /// ```
    pub fn new(keys: Vec<AcpPackContextSigningKey>) -> Self {
        Self { keys }
    }

    /// Build an empty keyring.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::AcpPackContextSigningKeyring;
    ///
    /// let keyring = AcpPackContextSigningKeyring::empty();
    /// assert_eq!(keyring.key_count(), 0);
    /// ```
    pub fn empty() -> Self {
        Self { keys: Vec::new() }
    }

    /// Build the deterministic development fixture keyring.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::AcpPackContextSigningKeyring;
    ///
    /// let keyring = AcpPackContextSigningKeyring::development_fixture();
    /// assert_eq!(keyring.key_count(), 1);
    /// ```
    pub fn development_fixture() -> Self {
        Self {
            keys: vec![development_signing_key()],
        }
    }

    /// Build a production keyring from environment variables.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ob_poc::acp_pack_context_envelope_v2::AcpPackContextSigningKeyring;
    ///
    /// let keyring = AcpPackContextSigningKeyring::production_from_env().unwrap();
    /// assert_eq!(keyring.key_count(), 1);
    /// ```
    pub fn production_from_env() -> Result<Self> {
        let key_id = std::env::var(ACP_PACK_CONTEXT_SIGNING_KEY_ID_ENV)
            .with_context(|| format!("{ACP_PACK_CONTEXT_SIGNING_KEY_ID_ENV} is not set"))?;
        let key_hex = std::env::var(ACP_PACK_CONTEXT_SIGNING_KEY_HEX_ENV)
            .with_context(|| format!("{ACP_PACK_CONTEXT_SIGNING_KEY_HEX_ENV} is not set"))?;
        let key_material = hex::decode(&key_hex)
            .with_context(|| format!("{ACP_PACK_CONTEXT_SIGNING_KEY_HEX_ENV} is not valid hex"))?;
        Ok(Self::new(vec![AcpPackContextSigningKey::new(
            key_id,
            ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
            key_material,
        )]))
    }

    /// Return the number of keys in the keyring.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ob_poc::acp_pack_context_envelope_v2::AcpPackContextSigningKeyring;
    ///
    /// assert_eq!(AcpPackContextSigningKeyring::empty().key_count(), 0);
    /// ```
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }
}

/// Structured refusal raised when an envelope fails verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPackContextVerificationRefusal {
    pub code: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct SectionBudget {
    name: &'static str,
    byte_limit: usize,
}

const ENVELOPE_BYTE_LIMIT: usize = 512 * 1024;
const TOKEN_BYTE_RATIO: usize = 4;
const SECTION_BUDGETS: &[SectionBudget] = &[
    SectionBudget {
        name: "pack_summary",
        byte_limit: 24 * 1024,
    },
    // R2a: dsl_atoms replaces verb_bindings + verb_effects + macro_tiers.
    // Combined v2 budget was 128+160+96 = 384 KiB; the unified surface
    // carries the same content density so the budget is retained.
    SectionBudget {
        name: "dsl_atoms",
        byte_limit: 384 * 1024,
    },
    SectionBudget {
        name: "production_contracts",
        byte_limit: 96 * 1024,
    },
    SectionBudget {
        name: "workbook_plans",
        byte_limit: 80 * 1024,
    },
    SectionBudget {
        name: "diagnostic_taxonomy",
        byte_limit: 16 * 1024,
    },
    // R2b — v0.5 §8 / §14 / §15 new top-level sections. Conservative
    // budgets sized for Slice 1 content; per-section omission triggers
    // a structured diagnostic per §8.1 budget policy.
    SectionBudget {
        name: "pack_neighbours",
        byte_limit: 12 * 1024,
    },
    SectionBudget {
        name: "known_collision_policy",
        byte_limit: 12 * 1024,
    },
    SectionBudget {
        name: "cross_dag_handoffs",
        byte_limit: 8 * 1024,
    },
    SectionBudget {
        name: "example_utterances",
        byte_limit: 32 * 1024,
    },
];

/// Build and sign a deterministic ACP pack context envelope v2.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::build_acp_pack_context_envelope_v2;
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let envelope = build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", &config_root).unwrap();
/// assert_eq!(envelope.schema_version, "acp_pack_context_envelope_v2");
/// ```
pub fn build_acp_pack_context_envelope_v2(
    projection: &AcpRegistryProjection,
    pack_id: &str,
    config_root: impl AsRef<Path>,
) -> Result<AcpPackContextEnvelopeV2> {
    build_acp_pack_context_envelope_v2_with_signing_key(
        projection,
        pack_id,
        config_root,
        &development_signing_key(),
    )
}

/// Build and sign a deterministic ACP pack context envelope v2 with explicit key material.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_envelope_v2_with_signing_key, AcpPackContextSigningKey,
///     ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let key = AcpPackContextSigningKey::new(
///     "acp-pack-context-prod-key-v1",
///     ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
///     b"fixture-key-material",
/// );
/// let envelope = build_acp_pack_context_envelope_v2_with_signing_key(
///     &projection,
///     "cbu-maintenance",
///     &config_root,
///     &key,
/// )
/// .unwrap();
/// assert_eq!(envelope.signature.as_ref().unwrap().key_id, key.key_id());
/// ```
pub fn build_acp_pack_context_envelope_v2_with_signing_key(
    projection: &AcpRegistryProjection,
    pack_id: &str,
    config_root: impl AsRef<Path>,
    signing_key: &AcpPackContextSigningKey,
) -> Result<AcpPackContextEnvelopeV2> {
    let config_root = config_root.as_ref();
    let pack = projection
        .packs
        .iter()
        .find(|pack| pack.pack_id == pack_id)
        .with_context(|| format!("pack {pack_id} is not present in projection"))?;
    let build_inputs = build_inputs(projection, config_root)?;
    let sections = envelope_sections(pack, projection)?;
    let (section_hashes, budget) = budget_and_hash_sections(&sections)?;
    let content_hash_chain = content_hash_chain(&section_hashes);

    let body_without_size = AcpPackContextEnvelopeBodyV2 {
        builder_version: ACP_PACK_CONTEXT_ENVELOPE_BUILDER_VERSION.to_string(),
        lifecycle: AcpPackLifecycleState::Draft,
        pack_id: pack.pack_id.clone(),
        pack_name: pack.pack_name.clone(),
        pack_version: pack.pack_version.clone(),
        manifest_hash: pack.manifest_hash.clone(),
        build_inputs,
        budget,
        sections,
        section_hashes,
        content_hash_chain,
    };
    let mut envelope = sign_body_with_key(body_without_size, signing_key)?;
    let envelope_bytes = deterministic_json_bytes(&envelope)?;
    envelope.body.budget.envelope_bytes = envelope_bytes.len();
    envelope.body.budget.envelope_token_estimate = token_estimate(envelope_bytes.len());
    if envelope.body.budget.envelope_bytes > envelope.body.budget.envelope_byte_limit {
        envelope.body.budget.omitted.push(AcpPackContextOmission {
            section: "envelope".to_string(),
            reason: "envelope_byte_limit_exceeded".to_string(),
            byte_limit: envelope.body.budget.envelope_byte_limit,
            actual_bytes: envelope.body.budget.envelope_bytes,
        });
    }
    sign_body_with_key(envelope.body, signing_key)
}

/// Build a deterministic bundle containing every projected pack envelope.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::build_acp_pack_context_envelope_v2_bundle;
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let bundle = build_acp_pack_context_envelope_v2_bundle(&projection, &config_root).unwrap();
/// assert_eq!(bundle.pack_count, projection.pack_count);
/// ```
pub fn build_acp_pack_context_envelope_v2_bundle(
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
) -> Result<AcpPackContextEnvelopeBundleV2> {
    let config_root = config_root.as_ref();
    let envelopes = projection
        .packs
        .iter()
        .map(|pack| build_acp_pack_context_envelope_v2(projection, &pack.pack_id, config_root))
        .collect::<Result<Vec<_>>>()?;
    Ok(AcpPackContextEnvelopeBundleV2 {
        schema_version: ACP_PACK_CONTEXT_ENVELOPE_V2_BUNDLE_SCHEMA_VERSION.to_string(),
        pack_count: envelopes.len(),
        envelopes,
    })
}

/// Build persisted registry state with every projected pack sealed as active.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::build_active_acp_pack_context_registry_state_v2;
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let state = build_active_acp_pack_context_registry_state_v2(&projection, &config_root).unwrap();
/// assert_eq!(state.pack_count, projection.pack_count);
/// ```
pub fn build_active_acp_pack_context_registry_state_v2(
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
) -> Result<AcpPackContextRegistryStateV2> {
    build_active_acp_pack_context_registry_state_v2_with_signing_key(
        projection,
        config_root,
        &development_signing_key(),
    )
}

/// Build persisted registry state with every projected pack sealed by explicit key material.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_active_acp_pack_context_registry_state_v2_with_signing_key,
///     AcpPackContextSigningKey, ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let key = AcpPackContextSigningKey::new(
///     "acp-pack-context-prod-key-v1",
///     ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
///     b"fixture-key-material",
/// );
/// let state = build_active_acp_pack_context_registry_state_v2_with_signing_key(
///     &projection,
///     &config_root,
///     &key,
/// )
/// .unwrap();
/// assert_eq!(state.pack_count, projection.pack_count);
/// ```
pub fn build_active_acp_pack_context_registry_state_v2_with_signing_key(
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
    signing_key: &AcpPackContextSigningKey,
) -> Result<AcpPackContextRegistryStateV2> {
    let config_root = config_root.as_ref();
    let mut envelopes = Vec::new();
    for pack in &projection.packs {
        let envelope = build_acp_pack_context_envelope_v2_with_signing_key(
            projection,
            &pack.pack_id,
            config_root,
            signing_key,
        )?;
        let active = transition_acp_pack_lifecycle_v2_with_signing_key(
            &envelope,
            AcpPackLifecycleState::Active,
            signing_key,
        )
        .map_err(|refusal| {
            anyhow::anyhow!(
                "activating deterministic ACP pack context envelope {} failed: {}",
                pack.pack_id,
                refusal.code
            )
        })?;
        envelopes.push(active);
    }
    Ok(AcpPackContextRegistryStateV2 {
        schema_version: ACP_PACK_CONTEXT_REGISTRY_STATE_V2_SCHEMA_VERSION.to_string(),
        source_projection_hash: projection.projection_hash.clone(),
        pack_count: envelopes.len(),
        envelopes,
    })
}

/// Persist ACP pack context registry state as deterministic pretty JSON.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_active_acp_pack_context_registry_state_v2,
///     write_acp_pack_context_registry_state_v2,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let state = build_active_acp_pack_context_registry_state_v2(&projection, &config_root).unwrap();
/// let output = std::path::Path::new("/tmp/acp-pack-context-registry-state-v2.json");
/// write_acp_pack_context_registry_state_v2(output, &state).unwrap();
/// ```
pub fn write_acp_pack_context_registry_state_v2(
    path: impl AsRef<Path>,
    state: &AcpPackContextRegistryStateV2,
) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating registry state directory {}", parent.display()))?;
    }
    fs::write(path, deterministic_pretty_json_bytes(state)?)
        .with_context(|| format!("writing ACP pack context registry state {}", path.display()))
}

/// Load persisted registry state and verify active-pack immutability.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_active_acp_pack_context_registry_state_v2,
///     load_acp_pack_context_registry_state_v2,
///     write_acp_pack_context_registry_state_v2,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let state = build_active_acp_pack_context_registry_state_v2(&projection, &config_root).unwrap();
/// let path = std::path::Path::new("/tmp/acp-pack-context-registry-state-v2.json");
/// write_acp_pack_context_registry_state_v2(path, &state).unwrap();
/// load_acp_pack_context_registry_state_v2(path, &projection, &config_root).unwrap();
/// ```
pub fn load_acp_pack_context_registry_state_v2(
    path: impl AsRef<Path>,
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
) -> std::result::Result<AcpPackContextRegistryStateV2, AcpPackContextVerificationRefusal> {
    load_acp_pack_context_registry_state_v2_with_keyring(
        path,
        projection,
        config_root,
        &AcpPackContextSigningKeyring::development_fixture(),
    )
}

/// Load persisted registry state and verify active-pack immutability with explicit keys.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_active_acp_pack_context_registry_state_v2,
///     load_acp_pack_context_registry_state_v2_with_keyring,
///     write_acp_pack_context_registry_state_v2,
///     AcpPackContextSigningKeyring,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let state = build_active_acp_pack_context_registry_state_v2(&projection, &config_root).unwrap();
/// let path = std::path::Path::new("/tmp/acp-pack-context-registry-state-v2.json");
/// write_acp_pack_context_registry_state_v2(path, &state).unwrap();
/// let keyring = AcpPackContextSigningKeyring::development_fixture();
/// load_acp_pack_context_registry_state_v2_with_keyring(path, &projection, &config_root, &keyring).unwrap();
/// ```
pub fn load_acp_pack_context_registry_state_v2_with_keyring(
    path: impl AsRef<Path>,
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
    keyring: &AcpPackContextSigningKeyring,
) -> std::result::Result<AcpPackContextRegistryStateV2, AcpPackContextVerificationRefusal> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        refusal(
            "acp_registry_state_load_failed",
            "ACP pack context registry state could not be loaded",
            None,
            Some(format!("{}: {error}", path.display())),
        )
    })?;
    let state: AcpPackContextRegistryStateV2 = serde_json::from_slice(&bytes).map_err(|error| {
        refusal(
            "acp_registry_state_parse_failed",
            "ACP pack context registry state is not valid JSON for schema v2",
            None,
            Some(error.to_string()),
        )
    })?;
    verify_acp_pack_context_registry_state_v2_with_keyring(
        &state,
        projection,
        config_root,
        keyring,
    )?;
    Ok(state)
}

/// Load registry state for a development or production online verification path.
///
/// Development mode may synthesize active state from the current projection when no
/// state path is configured. Production mode must load a persisted state artifact.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     load_online_acp_pack_context_registry_state_v2, AcpPackContextRegistryLoadOptions,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let state = load_online_acp_pack_context_registry_state_v2(
///     &projection,
///     &config_root,
///     AcpPackContextRegistryLoadOptions::development(),
/// )
/// .unwrap();
/// assert_eq!(state.pack_count, projection.pack_count);
/// ```
pub fn load_online_acp_pack_context_registry_state_v2(
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
    options: AcpPackContextRegistryLoadOptions,
) -> std::result::Result<AcpPackContextRegistryStateV2, AcpPackContextVerificationRefusal> {
    load_online_acp_pack_context_registry_state_v2_with_keyring(
        projection,
        config_root,
        options,
        &AcpPackContextSigningKeyring::development_fixture(),
    )
}

/// Load registry state online with explicit verification key material.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     load_online_acp_pack_context_registry_state_v2_with_keyring,
///     AcpPackContextRegistryLoadOptions, AcpPackContextSigningKeyring,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let keyring = AcpPackContextSigningKeyring::development_fixture();
/// let state = load_online_acp_pack_context_registry_state_v2_with_keyring(
///     &projection,
///     &config_root,
///     AcpPackContextRegistryLoadOptions::development(),
///     &keyring,
/// )
/// .unwrap();
/// assert_eq!(state.pack_count, projection.pack_count);
/// ```
pub fn load_online_acp_pack_context_registry_state_v2_with_keyring(
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
    options: AcpPackContextRegistryLoadOptions,
    keyring: &AcpPackContextSigningKeyring,
) -> std::result::Result<AcpPackContextRegistryStateV2, AcpPackContextVerificationRefusal> {
    let config_root = config_root.as_ref();
    match (options.mode, options.state_path.as_deref()) {
        (AcpPackContextRegistryLoadMode::Development, Some(path)) => {
            load_acp_pack_context_registry_state_v2_with_keyring(
                path,
                projection,
                config_root,
                keyring,
            )
        }
        (AcpPackContextRegistryLoadMode::Development, None) => {
            let signing_key = primary_signing_key(keyring).ok_or_else(|| {
                refusal(
                    "acp_registry_signing_key_required",
                    "Development ACP pack context registry load requires signing key material",
                    Some("at least one signing key".to_string()),
                    None,
                )
            })?;
            let state = build_active_acp_pack_context_registry_state_v2_with_signing_key(
                projection,
                config_root,
                signing_key,
            )
            .map_err(|error| {
                refusal(
                    "acp_registry_state_development_build_failed",
                    "Development ACP pack context registry state could not be built",
                    None,
                    Some(error.to_string()),
                )
            })?;
            verify_acp_pack_context_registry_state_v2_with_keyring(
                &state,
                projection,
                config_root,
                keyring,
            )?;
            Ok(state)
        }
        (AcpPackContextRegistryLoadMode::Production, Some(path)) => {
            load_acp_pack_context_registry_state_v2_with_keyring(
                path,
                projection,
                config_root,
                keyring,
            )
        }
        (AcpPackContextRegistryLoadMode::Production, None) => Err(refusal(
            "acp_registry_state_required",
            "Production ACP pack context registry load requires a persisted registry state path",
            Some("persisted registry state path".to_string()),
            None,
        )),
    }
}

/// Verify persisted registry state and active-pack rebuild immutability.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_active_acp_pack_context_registry_state_v2,
///     verify_acp_pack_context_registry_state_v2,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let state = build_active_acp_pack_context_registry_state_v2(&projection, &config_root).unwrap();
/// verify_acp_pack_context_registry_state_v2(&state, &projection, &config_root).unwrap();
/// ```
pub fn verify_acp_pack_context_registry_state_v2(
    state: &AcpPackContextRegistryStateV2,
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
) -> std::result::Result<(), AcpPackContextVerificationRefusal> {
    verify_acp_pack_context_registry_state_v2_with_keyring(
        state,
        projection,
        config_root,
        &AcpPackContextSigningKeyring::development_fixture(),
    )
}

/// Verify persisted registry state and active-pack rebuild immutability with explicit keys.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_active_acp_pack_context_registry_state_v2,
///     verify_acp_pack_context_registry_state_v2_with_keyring,
///     AcpPackContextSigningKeyring,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let state = build_active_acp_pack_context_registry_state_v2(&projection, &config_root).unwrap();
/// let keyring = AcpPackContextSigningKeyring::development_fixture();
/// verify_acp_pack_context_registry_state_v2_with_keyring(
///     &state,
///     &projection,
///     &config_root,
///     &keyring,
/// )
/// .unwrap();
/// ```
pub fn verify_acp_pack_context_registry_state_v2_with_keyring(
    state: &AcpPackContextRegistryStateV2,
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
    keyring: &AcpPackContextSigningKeyring,
) -> std::result::Result<(), AcpPackContextVerificationRefusal> {
    if state.schema_version != ACP_PACK_CONTEXT_REGISTRY_STATE_V2_SCHEMA_VERSION {
        return Err(refusal(
            "acp_registry_state_schema_mismatch",
            "ACP pack context registry state schema version is not supported",
            Some(ACP_PACK_CONTEXT_REGISTRY_STATE_V2_SCHEMA_VERSION.to_string()),
            Some(state.schema_version.clone()),
        ));
    }
    if state.source_projection_hash != projection.projection_hash {
        return Err(refusal(
            "acp_registry_state_projection_hash_mismatch",
            "ACP pack context registry state was built from a different registry projection",
            Some(projection.projection_hash.clone()),
            Some(state.source_projection_hash.clone()),
        ));
    }
    if state.pack_count != state.envelopes.len() {
        return Err(refusal(
            "acp_registry_state_pack_count_mismatch",
            "ACP pack context registry state pack count does not match persisted envelopes",
            Some(state.pack_count.to_string()),
            Some(state.envelopes.len().to_string()),
        ));
    }
    if state.pack_count != projection.pack_count {
        return Err(refusal(
            "acp_registry_state_pack_count_mismatch",
            "ACP pack context registry state pack count does not match projection pack count",
            Some(projection.pack_count.to_string()),
            Some(state.pack_count.to_string()),
        ));
    }

    let config_root = config_root.as_ref();
    let expected_pack_ids = projection
        .packs
        .iter()
        .map(|pack| pack.pack_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_pack_ids = BTreeSet::new();
    let signing_key = primary_signing_key(keyring).ok_or_else(|| {
        refusal(
            "acp_registry_signing_key_required",
            "ACP pack context registry verification requires signing key material",
            Some("at least one signing key".to_string()),
            None,
        )
    })?;
    for envelope in &state.envelopes {
        verify_acp_pack_context_envelope_v2_with_keyring(envelope, keyring)?;
        if !seen_pack_ids.insert(envelope.body.pack_id.as_str()) {
            return Err(refusal(
                "acp_registry_state_duplicate_pack",
                "ACP pack context registry state contains a duplicate pack envelope",
                None,
                Some(envelope.body.pack_id.clone()),
            ));
        }
        if !expected_pack_ids.contains(envelope.body.pack_id.as_str()) {
            return Err(refusal(
                "acp_registry_state_unknown_pack",
                "ACP pack context registry state contains an envelope for an unknown pack",
                Some(
                    expected_pack_ids
                        .iter()
                        .copied()
                        .collect::<Vec<_>>()
                        .join(","),
                ),
                Some(envelope.body.pack_id.clone()),
            ));
        }
        let rebuilt = build_acp_pack_context_envelope_v2_with_signing_key(
            projection,
            &envelope.body.pack_id,
            config_root,
            signing_key,
        )
        .map_err(|error| {
            refusal(
                "acp_registry_state_rebuild_failed",
                "ACP pack context envelope could not be rebuilt during registry load",
                Some(envelope.body.pack_id.clone()),
                Some(error.to_string()),
            )
        })?;
        verify_acp_pack_context_rebuild_v2_with_keyring(envelope, &rebuilt, keyring)?;
    }
    if seen_pack_ids != expected_pack_ids {
        return Err(refusal(
            "acp_registry_state_pack_set_mismatch",
            "ACP pack context registry state pack set does not match the current projection",
            Some(
                expected_pack_ids
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            Some(seen_pack_ids.iter().copied().collect::<Vec<_>>().join(",")),
        ));
    }
    Ok(())
}

/// Build deterministic pretty JSON artifact bytes for a pack or the full bundle.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::build_acp_pack_context_artifact_bytes_v2;
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let bytes = build_acp_pack_context_artifact_bytes_v2(&projection, &config_root, "all").unwrap();
/// assert!(bytes.ends_with(b"\n"));
/// ```
pub fn build_acp_pack_context_artifact_bytes_v2(
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
    pack_id: &str,
) -> Result<Vec<u8>> {
    let config_root = config_root.as_ref();
    let output = if pack_id == "all" {
        serde_json::to_value(build_acp_pack_context_envelope_v2_bundle(
            projection,
            config_root,
        )?)
        .context("serializing ACP pack context envelope bundle")?
    } else {
        serde_json::to_value(build_acp_pack_context_envelope_v2(
            projection,
            pack_id,
            config_root,
        )?)
        .context("serializing ACP pack context envelope")?
    };
    deterministic_pretty_json_bytes(&output)
}

/// Verify that expected artifact bytes match a deterministic rebuild.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_artifact_bytes_v2, verify_acp_pack_context_artifact_rebuild_v2,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let bytes = build_acp_pack_context_artifact_bytes_v2(&projection, &config_root, "cbu-maintenance").unwrap();
/// verify_acp_pack_context_artifact_rebuild_v2(&bytes, &projection, &config_root, "cbu-maintenance").unwrap();
/// ```
pub fn verify_acp_pack_context_artifact_rebuild_v2(
    expected_bytes: &[u8],
    projection: &AcpRegistryProjection,
    config_root: impl AsRef<Path>,
    pack_id: &str,
) -> std::result::Result<(), AcpPackContextVerificationRefusal> {
    let actual_bytes = build_acp_pack_context_artifact_bytes_v2(projection, config_root, pack_id)
        .map_err(|error| {
        refusal(
            "acp_envelope_artifact_rebuild_failed",
            "ACP pack context artifact could not be rebuilt",
            None,
            Some(error.to_string()),
        )
    })?;
    if expected_bytes != actual_bytes {
        return Err(refusal(
            "acp_envelope_artifact_rebuild_mismatch",
            "ACP pack context artifact bytes do not match deterministic rebuild output",
            Some(format!("{} bytes", expected_bytes.len())),
            Some(format!("{} bytes", actual_bytes.len())),
        ));
    }
    Ok(())
}

/// Verify an ACP pack context envelope v2.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_envelope_v2, verify_acp_pack_context_envelope_v2,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let envelope = build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", &config_root).unwrap();
/// verify_acp_pack_context_envelope_v2(&envelope).unwrap();
/// ```
pub fn verify_acp_pack_context_envelope_v2(
    envelope: &AcpPackContextEnvelopeV2,
) -> Result<(), AcpPackContextVerificationRefusal> {
    verify_acp_pack_context_envelope_v2_with_keyring(
        envelope,
        &AcpPackContextSigningKeyring::development_fixture(),
    )
}

/// Verify an ACP pack context envelope v2 with explicit signing key material.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_envelope_v2, verify_acp_pack_context_envelope_v2_with_keyring,
///     AcpPackContextSigningKeyring,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let envelope = build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", &config_root).unwrap();
/// let keyring = AcpPackContextSigningKeyring::development_fixture();
/// verify_acp_pack_context_envelope_v2_with_keyring(&envelope, &keyring).unwrap();
/// ```
pub fn verify_acp_pack_context_envelope_v2_with_keyring(
    envelope: &AcpPackContextEnvelopeV2,
    keyring: &AcpPackContextSigningKeyring,
) -> Result<(), AcpPackContextVerificationRefusal> {
    if envelope.schema_version != ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION {
        return Err(refusal(
            "acp_envelope_schema_mismatch",
            "Envelope schema version is not supported",
            Some(ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION.to_string()),
            Some(envelope.schema_version.to_string()),
        ));
    }
    let actual_hash =
        prefixed_hash(&deterministic_json_bytes(&envelope.body).map_err(|error| {
            refusal(
                "acp_envelope_serialization_failed",
                "Envelope body could not be serialized deterministically",
                None,
                Some(error.to_string()),
            )
        })?);
    if envelope.envelope_hash != actual_hash {
        return Err(refusal(
            "acp_envelope_hash_mismatch",
            "Envelope body hash does not match the declared hash",
            Some(envelope.envelope_hash.clone()),
            Some(actual_hash),
        ));
    }
    let signature = envelope.signature.as_ref().ok_or_else(|| {
        refusal(
            "acp_envelope_unsigned",
            "Envelope is missing a signature",
            None,
            None,
        )
    })?;
    if signature.signed_hash != envelope.envelope_hash {
        return Err(refusal(
            "acp_envelope_signature_hash_mismatch",
            "Envelope signature does not cover the declared hash",
            Some(envelope.envelope_hash.clone()),
            Some(signature.signed_hash.clone()),
        ));
    }
    let signing_key = signing_key_for_signature(keyring, signature).ok_or_else(|| {
        refusal(
            "acp_envelope_signature_key_untrusted",
            "Envelope signature key is not registered for ACP pack context verification",
            Some(registered_signing_key_ids(keyring).join(",")),
            Some(signature.key_id.clone()),
        )
    })?;
    if signature.algorithm != signing_key.algorithm() {
        return Err(refusal(
            "acp_envelope_signature_algorithm_unsupported",
            "Envelope signature algorithm is not registered for the signing key",
            Some(signing_key.algorithm().to_string()),
            Some(signature.algorithm.clone()),
        ));
    }
    let expected_signature = key_material_signature(signing_key, &signature.signed_hash);
    if signature.signature != expected_signature {
        return Err(refusal(
            "acp_envelope_signature_invalid",
            "Envelope signature is invalid",
            Some(expected_signature),
            Some(signature.signature.clone()),
        ));
    }
    if envelope.body.budget.envelope_bytes > envelope.body.budget.envelope_byte_limit {
        return Err(refusal(
            "acp_envelope_budget_exceeded",
            "Envelope exceeds the configured byte budget",
            Some(envelope.body.budget.envelope_byte_limit.to_string()),
            Some(envelope.body.budget.envelope_bytes.to_string()),
        ));
    }
    Ok(())
}

/// Re-seal an envelope with a permitted lifecycle transition.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_envelope_v2, transition_acp_pack_lifecycle_v2,
///     AcpPackLifecycleState,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let envelope = build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", &config_root).unwrap();
/// let active = transition_acp_pack_lifecycle_v2(&envelope, AcpPackLifecycleState::Active).unwrap();
/// assert_eq!(active.body.lifecycle, AcpPackLifecycleState::Active);
/// ```
pub fn transition_acp_pack_lifecycle_v2(
    envelope: &AcpPackContextEnvelopeV2,
    to: AcpPackLifecycleState,
) -> std::result::Result<AcpPackContextEnvelopeV2, AcpPackContextVerificationRefusal> {
    transition_acp_pack_lifecycle_v2_with_signing_key(envelope, to, &development_signing_key())
}

/// Re-seal an envelope with a permitted lifecycle transition using explicit key material.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_envelope_v2_with_signing_key,
///     transition_acp_pack_lifecycle_v2_with_signing_key, AcpPackContextSigningKey,
///     AcpPackLifecycleState, ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let key = AcpPackContextSigningKey::new("key-1", ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM, b"material");
/// let envelope = build_acp_pack_context_envelope_v2_with_signing_key(
///     &projection,
///     "cbu-maintenance",
///     &config_root,
///     &key,
/// )
/// .unwrap();
/// let active = transition_acp_pack_lifecycle_v2_with_signing_key(
///     &envelope,
///     AcpPackLifecycleState::Active,
///     &key,
/// )
/// .unwrap();
/// assert_eq!(active.body.lifecycle, AcpPackLifecycleState::Active);
/// ```
pub fn transition_acp_pack_lifecycle_v2_with_signing_key(
    envelope: &AcpPackContextEnvelopeV2,
    to: AcpPackLifecycleState,
    signing_key: &AcpPackContextSigningKey,
) -> std::result::Result<AcpPackContextEnvelopeV2, AcpPackContextVerificationRefusal> {
    verify_acp_pack_context_envelope_v2_with_keyring(
        envelope,
        &AcpPackContextSigningKeyring::new(vec![signing_key.clone()]),
    )?;
    let from = envelope.body.lifecycle;
    if from == to {
        return Ok(envelope.clone());
    }
    if !acp_pack_lifecycle_transition_allowed(from, to) {
        return Err(refusal(
            "acp_pack_lifecycle_transition_refused",
            "Envelope lifecycle transition is not permitted",
            Some(format!("{from:?} -> permitted forward transition")),
            Some(format!("{from:?} -> {to:?}")),
        ));
    }
    let mut body = envelope.body.clone();
    body.lifecycle = to;
    sign_body_with_key(body, signing_key).map_err(|error| {
        refusal(
            "acp_envelope_serialization_failed",
            "Envelope body could not be serialized after lifecycle transition",
            None,
            Some(error.to_string()),
        )
    })
}

/// Verify that an active registered envelope matches a deterministic rebuild.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_envelope_v2, transition_acp_pack_lifecycle_v2,
///     verify_acp_pack_context_rebuild_v2, AcpPackLifecycleState,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let rebuilt = build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", &config_root).unwrap();
/// let registered = transition_acp_pack_lifecycle_v2(&rebuilt, AcpPackLifecycleState::Active).unwrap();
/// verify_acp_pack_context_rebuild_v2(&registered, &rebuilt).unwrap();
/// ```
pub fn verify_acp_pack_context_rebuild_v2(
    registered: &AcpPackContextEnvelopeV2,
    rebuilt: &AcpPackContextEnvelopeV2,
) -> std::result::Result<(), AcpPackContextVerificationRefusal> {
    verify_acp_pack_context_rebuild_v2_with_keyring(
        registered,
        rebuilt,
        &AcpPackContextSigningKeyring::development_fixture(),
    )
}

/// Verify active registered envelope rebuild parity with explicit signing keys.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc::acp_pack_context_envelope_v2::{
///     build_acp_pack_context_envelope_v2, transition_acp_pack_lifecycle_v2,
///     verify_acp_pack_context_rebuild_v2_with_keyring, AcpPackContextSigningKeyring,
///     AcpPackLifecycleState,
/// };
/// use ob_poc::acp_registry_projection::build_slice1_acp_registry_projection;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let projection = build_slice1_acp_registry_projection(&config_root).unwrap();
/// let rebuilt = build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", &config_root).unwrap();
/// let registered = transition_acp_pack_lifecycle_v2(&rebuilt, AcpPackLifecycleState::Active).unwrap();
/// let keyring = AcpPackContextSigningKeyring::development_fixture();
/// verify_acp_pack_context_rebuild_v2_with_keyring(&registered, &rebuilt, &keyring).unwrap();
/// ```
pub fn verify_acp_pack_context_rebuild_v2_with_keyring(
    registered: &AcpPackContextEnvelopeV2,
    rebuilt: &AcpPackContextEnvelopeV2,
    keyring: &AcpPackContextSigningKeyring,
) -> std::result::Result<(), AcpPackContextVerificationRefusal> {
    let signing_key = primary_signing_key(keyring).ok_or_else(|| {
        refusal(
            "acp_registry_signing_key_required",
            "ACP pack context rebuild verification requires signing key material",
            Some("at least one signing key".to_string()),
            None,
        )
    })?;
    verify_acp_pack_context_envelope_v2_with_keyring(registered, keyring)?;
    verify_acp_pack_context_envelope_v2_with_keyring(rebuilt, keyring)?;
    if registered.body.pack_id != rebuilt.body.pack_id {
        return Err(refusal(
            "acp_envelope_pack_mismatch",
            "Registered and rebuilt envelopes refer to different packs",
            Some(registered.body.pack_id.clone()),
            Some(rebuilt.body.pack_id.clone()),
        ));
    }

    match registered.body.lifecycle {
        AcpPackLifecycleState::Draft => Ok(()),
        AcpPackLifecycleState::Active => {
            let comparable_rebuild = transition_acp_pack_lifecycle_v2_with_signing_key(
                rebuilt,
                AcpPackLifecycleState::Active,
                signing_key,
            )?;
            if registered.envelope_hash != comparable_rebuild.envelope_hash {
                return Err(refusal(
                    "acp_active_pack_rebuild_mismatch",
                    "Active registered envelope does not match deterministic rebuild output",
                    Some(registered.envelope_hash.clone()),
                    Some(comparable_rebuild.envelope_hash),
                ));
            }
            Ok(())
        }
        AcpPackLifecycleState::Deprecated => Ok(()),
        AcpPackLifecycleState::Retired => Ok(()),
    }
}

/// Return whether a lifecycle transition is permitted.
///
/// # Examples
///
/// ```rust
/// use ob_poc::acp_pack_context_envelope_v2::{
///     acp_pack_lifecycle_transition_allowed, AcpPackLifecycleState,
/// };
///
/// assert!(acp_pack_lifecycle_transition_allowed(
///     AcpPackLifecycleState::Draft,
///     AcpPackLifecycleState::Active,
/// ));
/// assert!(!acp_pack_lifecycle_transition_allowed(
///     AcpPackLifecycleState::Active,
///     AcpPackLifecycleState::Draft,
/// ));
/// ```
pub fn acp_pack_lifecycle_transition_allowed(
    from: AcpPackLifecycleState,
    to: AcpPackLifecycleState,
) -> bool {
    matches!(
        (from, to),
        (AcpPackLifecycleState::Draft, AcpPackLifecycleState::Active)
            | (
                AcpPackLifecycleState::Active,
                AcpPackLifecycleState::Deprecated
            )
            | (
                AcpPackLifecycleState::Deprecated,
                AcpPackLifecycleState::Retired
            )
    )
}

#[cfg(test)]
fn sign_body(body: AcpPackContextEnvelopeBodyV2) -> Result<AcpPackContextEnvelopeV2> {
    sign_body_with_key(body, &development_signing_key())
}

fn sign_body_with_key(
    body: AcpPackContextEnvelopeBodyV2,
    signing_key: &AcpPackContextSigningKey,
) -> Result<AcpPackContextEnvelopeV2> {
    let body_hash = prefixed_hash(&deterministic_json_bytes(&body)?);
    Ok(AcpPackContextEnvelopeV2 {
        schema_version: ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION.to_string(),
        envelope_hash: body_hash.clone(),
        signature: Some(AcpPackContextEnvelopeSignature {
            algorithm: signing_key.algorithm().to_string(),
            key_id: signing_key.key_id().to_string(),
            signed_hash: body_hash.clone(),
            signature: key_material_signature(signing_key, &body_hash),
        }),
        body,
    })
}

fn build_inputs(
    projection: &AcpRegistryProjection,
    config_root: &Path,
) -> Result<AcpPackContextBuildInputs> {
    Ok(AcpPackContextBuildInputs {
        source_projection_schema: projection.schema_version.to_string(),
        source_projection_hash: projection.projection_hash.clone(),
        semos_dsl_hash: hash_config_dirs(config_root, &["verbs"])?,
        governed_config_artifact_hash: hash_config_dirs(
            config_root,
            &["packs", "verb_schemas/macros"],
        )?,
        registered_fixture_hash: hash_config_dirs(
            config_root,
            &["stategraphs", "sem_os_seeds", "workflows"],
        )?,
        builder_lockfile: format!(
            "{}:{}",
            ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION, ACP_PACK_CONTEXT_ENVELOPE_BUILDER_VERSION
        ),
    })
}

fn envelope_sections(
    pack: &AcpRegistryPackProjection,
    projection: &AcpRegistryProjection,
) -> Result<AcpPackContextEnvelopeSections> {
    // R2b: per-pack slices of the top-level §8/§14/§15 sections.
    let pack_id = pack.pack_id.as_str();

    let pack_neighbours = projection
        .pack_neighbours
        .iter()
        .find(|edge| edge.from_pack_id == pack_id)
        .map(|edge| edge.neighbours.clone())
        .unwrap_or_default();

    let known_collision_policy: Vec<_> = projection
        .known_collision_policy
        .iter()
        .filter(|c| c.winner_pack_id == pack_id || c.loser_pack_ids.iter().any(|p| p == pack_id))
        .cloned()
        .collect();

    let cross_dag_handoffs: Vec<_> = projection
        .cross_dag_handoffs
        .iter()
        .filter(|h| h.from_pack_id == pack_id)
        .cloned()
        .collect();

    let example_utterances: Vec<_> = projection
        .example_utterances
        .iter()
        .filter(|ex| {
            ex.pack_id == pack_id || ex.expected_pack_id.as_deref() == Some(pack_id)
        })
        .cloned()
        .collect();

    Ok(AcpPackContextEnvelopeSections {
        pack_summary: serde_json::json!({
            "pack_id": pack.pack_id,
            "pack_name": pack.pack_name,
            "pack_version": pack.pack_version,
            "invocation_phrases": pack.invocation_phrases,
            "workspaces": pack.workspaces,
            "required_context": pack.required_context,
            "optional_context": pack.optional_context,
            "allowed_verbs": pack.allowed_verbs,
            "forbidden_verbs": pack.forbidden_verbs,
            "risk_policy": pack.risk_policy,
            "required_questions": pack.required_questions,
            "optional_questions": pack.optional_questions,
        }),
        dsl_atoms: serde_json::to_value(&pack.dsl_atoms)
            .context("serializing dsl_atoms section")?,
        production_contracts: serde_json::to_value(production_contracts(&pack.verb_effects)?)
            .context("serializing production contract section")?,
        workbook_plans: serde_json::to_value(workbook_plan_summaries(&pack.workbook_plans))
            .context("serializing workbook plan section")?,
        diagnostic_taxonomy: serde_json::to_value(&projection.diagnostic_taxonomy)
            .context("serializing diagnostic taxonomy section")?,
        pack_neighbours: serde_json::to_value(pack_neighbours)
            .context("serializing pack_neighbours section")?,
        known_collision_policy: serde_json::to_value(known_collision_policy)
            .context("serializing known_collision_policy section")?,
        cross_dag_handoffs: serde_json::to_value(cross_dag_handoffs)
            .context("serializing cross_dag_handoffs section")?,
        example_utterances: serde_json::to_value(example_utterances)
            .context("serializing example_utterances section")?,
    })
}

#[derive(Serialize)]
struct ProductionContractHashMaterial<'a> {
    verb: &'a str,
    exposure: &'a str,
    return_type: &'a Option<String>,
    produces_entity_grain: &'a Option<String>,
    read_entity_grains: &'a [String],
    write_entity_grains: &'a [String],
    side_effects: &'a Option<String>,
    policy_grade: &'a str,
}

fn production_contracts(
    effects: &[AcpVerbEffectProjection],
) -> Result<Vec<AcpPackProductionContract>> {
    let mut contracts = effects
        .iter()
        .filter(|effect| effect.return_type.is_some() || effect.produces_entity_grain.is_some())
        .map(|effect| {
            let contract_hash = deterministic_hash(&ProductionContractHashMaterial {
                verb: &effect.verb,
                exposure: &effect.exposure,
                return_type: &effect.return_type,
                produces_entity_grain: &effect.produces_entity_grain,
                read_entity_grains: &effect.read_entity_grains,
                write_entity_grains: &effect.write_entity_grains,
                side_effects: &effect.side_effects,
                policy_grade: &effect.policy.policy_grade,
            })?;
            Ok(AcpPackProductionContract {
                verb: effect.verb.clone(),
                exposure: effect.exposure.clone(),
                return_type: effect.return_type.clone(),
                produces_entity_grain: effect.produces_entity_grain.clone(),
                read_entity_grains: effect.read_entity_grains.clone(),
                write_entity_grains: effect.write_entity_grains.clone(),
                side_effects: effect.side_effects.clone(),
                policy_grade: effect.policy.policy_grade.clone(),
                contract_hash,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    contracts.sort_by(|left, right| {
        left.exposure
            .cmp(&right.exposure)
            .then(left.verb.cmp(&right.verb))
    });
    Ok(contracts)
}

fn workbook_plan_summaries(plans: &[AcpWorkbookPlanProjection]) -> Vec<serde_json::Value> {
    plans
        .iter()
        .map(|plan| {
            serde_json::json!({
                "plan_id": plan.plan_id,
                "template_id": plan.template_id,
                "pack_id": plan.pack_id,
                "plan_hash": plan.plan_hash,
                "trigger_phrases": plan.trigger_phrases,
                "required_bindings": plan.required_bindings,
                "optional_bindings": plan.optional_bindings,
                "steps": plan.steps,
                "risk_policy": plan.risk_policy,
                "state_effects": plan.state_effects,
                "refusal_conditions": plan.refusal_conditions,
                "policy": plan.policy,
            })
        })
        .collect()
}

fn budget_and_hash_sections(
    sections: &AcpPackContextEnvelopeSections,
) -> Result<(BTreeMap<String, String>, AcpPackContextBudgetReport)> {
    budget_and_hash_sections_with_budgets(sections, SECTION_BUDGETS)
}

fn budget_and_hash_sections_with_budgets(
    sections: &AcpPackContextEnvelopeSections,
    budgets: &[SectionBudget],
) -> Result<(BTreeMap<String, String>, AcpPackContextBudgetReport)> {
    let mut section_hashes = BTreeMap::new();
    let mut section_reports = Vec::new();
    let mut omitted = Vec::new();
    for budget in budgets {
        let bytes = deterministic_json_bytes(section_value(sections, budget.name))?;
        let byte_count = bytes.len();
        let omitted_section = byte_count > budget.byte_limit;
        if omitted_section {
            omitted.push(AcpPackContextOmission {
                section: budget.name.to_string(),
                reason: "section_byte_limit_exceeded".to_string(),
                byte_limit: budget.byte_limit,
                actual_bytes: byte_count,
            });
        }
        section_reports.push(AcpPackContextSectionBudgetReport {
            section: budget.name.to_string(),
            byte_limit: budget.byte_limit,
            token_limit: token_estimate(budget.byte_limit),
            bytes: byte_count,
            token_estimate: token_estimate(byte_count),
            omitted: omitted_section,
        });
        section_hashes.insert(budget.name.to_string(), prefixed_hash(&bytes));
    }
    Ok((
        section_hashes,
        AcpPackContextBudgetReport {
            envelope_byte_limit: ENVELOPE_BYTE_LIMIT,
            envelope_token_limit: token_estimate(ENVELOPE_BYTE_LIMIT),
            envelope_bytes: 0,
            envelope_token_estimate: 0,
            section_reports,
            omitted,
        },
    ))
}

fn section_value<'a>(
    sections: &'a AcpPackContextEnvelopeSections,
    name: &str,
) -> &'a serde_json::Value {
    match name {
        "pack_summary" => &sections.pack_summary,
        "dsl_atoms" => &sections.dsl_atoms,
        "production_contracts" => &sections.production_contracts,
        "workbook_plans" => &sections.workbook_plans,
        "diagnostic_taxonomy" => &sections.diagnostic_taxonomy,
        "pack_neighbours" => &sections.pack_neighbours,
        "known_collision_policy" => &sections.known_collision_policy,
        "cross_dag_handoffs" => &sections.cross_dag_handoffs,
        "example_utterances" => &sections.example_utterances,
        _ => &serde_json::Value::Null,
    }
}

fn content_hash_chain(section_hashes: &BTreeMap<String, String>) -> Vec<String> {
    let mut previous = "sha256:root".to_string();
    let mut chain = Vec::new();
    for (name, hash) in section_hashes {
        let material = format!("{previous}|{name}|{hash}");
        previous = prefixed_hash(material.as_bytes());
        chain.push(previous.clone());
    }
    chain
}

fn hash_config_dirs(config_root: &Path, dirs: &[&str]) -> Result<String> {
    let mut files = Vec::new();
    for dir in dirs {
        let path = config_root.join(dir);
        if path.exists() {
            files.extend(walk_files(&path)?);
        }
    }
    files.sort();
    let mut hasher = Sha256::new();
    for file in files {
        let relative = file.strip_prefix(config_root).unwrap_or(&file);
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(
            fs::read(&file).with_context(|| format!("reading build input {}", file.display()))?,
        );
        hasher.update([0]);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn walk_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(current) = dirs.pop() {
        for entry in
            fs::read_dir(&current).with_context(|| format!("reading {}", current.display()))?
        {
            let path = entry
                .with_context(|| format!("reading entry in {}", current.display()))?
                .path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn deterministic_json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).context("serializing deterministic envelope JSON")
}

fn deterministic_pretty_json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .context("serializing deterministic pretty envelope JSON")?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn deterministic_hash(value: &impl Serialize) -> Result<String> {
    Ok(prefixed_hash(&deterministic_json_bytes(value)?))
}

fn prefixed_hash(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn development_signing_key() -> AcpPackContextSigningKey {
    AcpPackContextSigningKey::new(
        ACP_PACK_CONTEXT_DEV_SIGNING_KEY_ID,
        ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
        b"acp-pack-context-development-fixture-key-v1",
    )
}

fn signing_key_for_signature<'a>(
    keyring: &'a AcpPackContextSigningKeyring,
    signature: &AcpPackContextEnvelopeSignature,
) -> Option<&'a AcpPackContextSigningKey> {
    keyring
        .keys
        .iter()
        .find(|key| key.key_id() == signature.key_id)
}

fn registered_signing_key_ids(keyring: &AcpPackContextSigningKeyring) -> Vec<String> {
    keyring
        .keys
        .iter()
        .map(|key| key.key_id().to_string())
        .collect()
}

fn primary_signing_key(
    keyring: &AcpPackContextSigningKeyring,
) -> Option<&AcpPackContextSigningKey> {
    keyring.keys.first()
}

fn key_material_signature(signing_key: &AcpPackContextSigningKey, signed_hash: &str) -> String {
    let signature = hmac_sha256(&signing_key.key_material, signed_hash.as_bytes());
    format!("sha256:{}", hex::encode(signature))
}

fn hmac_sha256(key_material: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0_u8; BLOCK_SIZE];
    if key_material.len() > BLOCK_SIZE {
        key_block[..32].copy_from_slice(&Sha256::digest(key_material));
    } else {
        key_block[..key_material.len()].copy_from_slice(key_material);
    }

    let mut outer_pad = [0x5c_u8; BLOCK_SIZE];
    let mut inner_pad = [0x36_u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        outer_pad[index] ^= key_block[index];
        inner_pad[index] ^= key_block[index];
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_hash);
    outer.finalize().into()
}

fn token_estimate(bytes: usize) -> usize {
    bytes.div_ceil(TOKEN_BYTE_RATIO)
}

fn refusal(
    code: &str,
    message: &str,
    expected: Option<String>,
    actual: Option<String>,
) -> AcpPackContextVerificationRefusal {
    AcpPackContextVerificationRefusal {
        code: code.to_string(),
        message: message.to_string(),
        expected,
        actual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp_registry_projection::build_slice1_acp_registry_projection;

    fn repo_config_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("config")
    }

    fn projection() -> AcpRegistryProjection {
        build_slice1_acp_registry_projection(repo_config_root()).unwrap()
    }

    fn production_fixture_signing_key() -> AcpPackContextSigningKey {
        AcpPackContextSigningKey::new(
            "acp-pack-context-prod-key-test",
            ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM,
            b"production-fixture-key-material",
        )
    }

    #[test]
    fn envelope_v2_builds_and_verifies_for_slice1_pack() {
        let projection = projection();
        let envelope =
            build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", repo_config_root())
                .unwrap();

        assert_eq!(
            envelope.schema_version,
            ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION
        );
        assert_eq!(envelope.body.lifecycle, AcpPackLifecycleState::Draft);
        assert_eq!(envelope.body.pack_id, "cbu-maintenance");
        assert!(envelope.envelope_hash.starts_with("sha256:"));
        assert!(envelope.signature.is_some());
        assert!(envelope.body.budget.omitted.is_empty());
        verify_acp_pack_context_envelope_v2(&envelope).unwrap();
    }

    #[test]
    fn envelope_v2_bytes_are_stable_for_same_inputs() {
        let projection = projection();
        let first = build_acp_pack_context_envelope_v2(
            &projection,
            "product-service-taxonomy",
            repo_config_root(),
        )
        .unwrap();
        let second = build_acp_pack_context_envelope_v2(
            &projection,
            "product-service-taxonomy",
            repo_config_root(),
        )
        .unwrap();

        assert_eq!(
            deterministic_json_bytes(&first).unwrap(),
            deterministic_json_bytes(&second).unwrap()
        );
        assert_eq!(first.envelope_hash, second.envelope_hash);
    }

    #[test]
    fn artifact_rebuild_verification_accepts_byte_identical_output() {
        let projection = projection();
        let expected = build_acp_pack_context_artifact_bytes_v2(
            &projection,
            repo_config_root(),
            "cbu-maintenance",
        )
        .unwrap();

        verify_acp_pack_context_artifact_rebuild_v2(
            &expected,
            &projection,
            repo_config_root(),
            "cbu-maintenance",
        )
        .unwrap();
    }

    #[test]
    fn artifact_rebuild_verification_refuses_byte_mismatch() {
        let projection = projection();
        let mut expected =
            build_acp_pack_context_artifact_bytes_v2(&projection, repo_config_root(), "all")
                .unwrap();
        expected.push(b' ');

        let refusal = verify_acp_pack_context_artifact_rebuild_v2(
            &expected,
            &projection,
            repo_config_root(),
            "all",
        )
        .unwrap_err();
        assert_eq!(refusal.code, "acp_envelope_artifact_rebuild_mismatch");
    }

    #[test]
    fn artifact_bundle_bytes_use_declared_schema_and_pack_count() {
        let projection = projection();
        let bytes =
            build_acp_pack_context_artifact_bytes_v2(&projection, repo_config_root(), "all")
                .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(
            value["schema_version"],
            ACP_PACK_CONTEXT_ENVELOPE_V2_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(value["pack_count"], projection.pack_count);
        assert_eq!(
            value["envelopes"].as_array().unwrap().len(),
            projection.pack_count
        );
        assert!(bytes.ends_with(b"\n"));
    }

    #[test]
    fn envelope_v2_includes_normalized_production_contracts() {
        let projection = projection();
        let cbu_pack = projection
            .packs
            .iter()
            .find(|pack| pack.pack_id == "cbu-maintenance")
            .unwrap();
        let contracts = production_contracts(&cbu_pack.verb_effects).unwrap();

        let create = contracts
            .iter()
            .find(|contract| contract.verb == "cbu.create" && contract.exposure == "allowed")
            .unwrap();
        assert_eq!(create.produces_entity_grain.as_deref(), Some("cbu"));
        assert!(create.write_entity_grains.contains(&"cbu".to_string()));
        assert_eq!(create.side_effects.as_deref(), Some("state_write"));
        assert!(create.contract_hash.starts_with("sha256:"));

        let envelope =
            build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", repo_config_root())
                .unwrap();
        let section = envelope
            .body
            .sections
            .production_contracts
            .as_array()
            .unwrap();
        assert_eq!(section.len(), contracts.len());
        assert!(envelope
            .body
            .section_hashes
            .contains_key("production_contracts"));
        assert!(envelope
            .body
            .budget
            .section_reports
            .iter()
            .any(|report| report.section == "production_contracts"));
    }

    #[test]
    fn production_contract_hashes_are_stable_for_same_inputs() {
        let projection = projection();
        let taxonomy_pack = projection
            .packs
            .iter()
            .find(|pack| pack.pack_id == "product-service-taxonomy")
            .unwrap();

        let first = production_contracts(&taxonomy_pack.verb_effects).unwrap();
        let second = production_contracts(&taxonomy_pack.verb_effects).unwrap();

        assert_eq!(first, second);
        assert!(first.iter().any(|contract| {
            contract.verb == "cbu.create"
                && contract.exposure == "forbidden"
                && contract.produces_entity_grain.as_deref() == Some("cbu")
        }));
    }

    #[test]
    fn envelope_v2_refuses_unsigned_envelope() {
        let projection = projection();
        let mut envelope = build_acp_pack_context_envelope_v2(
            &projection,
            "onboarding-request",
            repo_config_root(),
        )
        .unwrap();
        envelope.signature = None;

        let refusal = verify_acp_pack_context_envelope_v2(&envelope).unwrap_err();
        assert_eq!(refusal.code, "acp_envelope_unsigned");
    }

    #[test]
    fn envelope_v2_refuses_hash_mismatch() {
        let projection = projection();
        let mut envelope = build_acp_pack_context_envelope_v2(
            &projection,
            "onboarding-request",
            repo_config_root(),
        )
        .unwrap();
        envelope.body.pack_name = "tampered".to_string();

        let refusal = verify_acp_pack_context_envelope_v2(&envelope).unwrap_err();
        assert_eq!(refusal.code, "acp_envelope_hash_mismatch");
    }

    #[test]
    fn envelope_v2_refuses_unregistered_signature_key() {
        let projection = projection();
        let mut envelope = build_acp_pack_context_envelope_v2(
            &projection,
            "onboarding-request",
            repo_config_root(),
        )
        .unwrap();
        envelope.signature.as_mut().unwrap().key_id = "unregistered-key".to_string();

        let refusal = verify_acp_pack_context_envelope_v2(&envelope).unwrap_err();
        assert_eq!(refusal.code, "acp_envelope_signature_key_untrusted");
    }

    #[test]
    fn envelope_v2_refuses_unregistered_signature_algorithm() {
        let projection = projection();
        let mut envelope = build_acp_pack_context_envelope_v2(
            &projection,
            "onboarding-request",
            repo_config_root(),
        )
        .unwrap();
        envelope.signature.as_mut().unwrap().algorithm = "ed25519-test".to_string();

        let refusal = verify_acp_pack_context_envelope_v2(&envelope).unwrap_err();
        assert_eq!(refusal.code, "acp_envelope_signature_algorithm_unsupported");
    }

    #[test]
    fn lifecycle_transition_reseals_active_envelope() {
        let projection = projection();
        let envelope =
            build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", repo_config_root())
                .unwrap();
        let active =
            transition_acp_pack_lifecycle_v2(&envelope, AcpPackLifecycleState::Active).unwrap();

        assert_eq!(active.body.lifecycle, AcpPackLifecycleState::Active);
        assert_ne!(active.envelope_hash, envelope.envelope_hash);
        assert_eq!(
            active.signature.as_ref().unwrap().signed_hash,
            active.envelope_hash
        );
        verify_acp_pack_context_envelope_v2(&active).unwrap();
    }

    #[test]
    fn active_pack_rebuild_verification_accepts_equivalent_rebuild() {
        let projection = projection();
        let rebuilt =
            build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", repo_config_root())
                .unwrap();
        let registered =
            transition_acp_pack_lifecycle_v2(&rebuilt, AcpPackLifecycleState::Active).unwrap();

        verify_acp_pack_context_rebuild_v2(&registered, &rebuilt).unwrap();
    }

    #[test]
    fn active_pack_rebuild_verification_refuses_signed_drift() {
        let projection = projection();
        let rebuilt =
            build_acp_pack_context_envelope_v2(&projection, "cbu-maintenance", repo_config_root())
                .unwrap();
        let registered =
            transition_acp_pack_lifecycle_v2(&rebuilt, AcpPackLifecycleState::Active).unwrap();
        let mut drifted_body = rebuilt.body;
        drifted_body.pack_name = "changed name".to_string();
        let drifted_rebuild = sign_body(drifted_body).unwrap();

        let refusal =
            verify_acp_pack_context_rebuild_v2(&registered, &drifted_rebuild).unwrap_err();
        assert_eq!(refusal.code, "acp_active_pack_rebuild_mismatch");
    }

    #[test]
    fn registry_state_load_accepts_persisted_active_rebuilds() {
        let projection = projection();
        let state =
            build_active_acp_pack_context_registry_state_v2(&projection, repo_config_root())
                .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("acp-pack-context-registry-state-v2.json");
        write_acp_pack_context_registry_state_v2(&path, &state).unwrap();

        let loaded =
            load_acp_pack_context_registry_state_v2(&path, &projection, repo_config_root())
                .unwrap();

        assert_eq!(
            loaded.schema_version,
            ACP_PACK_CONTEXT_REGISTRY_STATE_V2_SCHEMA_VERSION
        );
        assert_eq!(loaded.source_projection_hash, projection.projection_hash);
        assert_eq!(loaded.pack_count, projection.pack_count);
        assert!(loaded
            .envelopes
            .iter()
            .all(|envelope| envelope.body.lifecycle == AcpPackLifecycleState::Active));
    }

    #[test]
    fn registry_state_load_refuses_active_signed_drift() {
        let projection = projection();
        let mut state =
            build_active_acp_pack_context_registry_state_v2(&projection, repo_config_root())
                .unwrap();
        let mut drifted_body = state.envelopes[0].body.clone();
        drifted_body.pack_name = "drifted active pack name".to_string();
        state.envelopes[0] = sign_body(drifted_body).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("acp-pack-context-registry-state-v2.json");
        write_acp_pack_context_registry_state_v2(&path, &state).unwrap();

        let refusal =
            load_acp_pack_context_registry_state_v2(&path, &projection, repo_config_root())
                .unwrap_err();

        assert_eq!(refusal.code, "acp_active_pack_rebuild_mismatch");
    }

    #[test]
    fn online_registry_development_load_builds_verified_state() {
        let projection = projection();

        let state = load_online_acp_pack_context_registry_state_v2(
            &projection,
            repo_config_root(),
            AcpPackContextRegistryLoadOptions::development(),
        )
        .unwrap();

        assert_eq!(state.pack_count, projection.pack_count);
        assert_eq!(state.source_projection_hash, projection.projection_hash);
        assert!(state
            .envelopes
            .iter()
            .all(|envelope| envelope.body.lifecycle == AcpPackLifecycleState::Active));
    }

    #[test]
    fn online_registry_production_load_requires_persisted_state_path() {
        let projection = projection();

        let refusal = load_online_acp_pack_context_registry_state_v2(
            &projection,
            repo_config_root(),
            AcpPackContextRegistryLoadOptions {
                mode: AcpPackContextRegistryLoadMode::Production,
                state_path: None,
            },
        )
        .unwrap_err();

        assert_eq!(refusal.code, "acp_registry_state_required");
    }

    #[test]
    fn online_registry_production_load_accepts_verified_persisted_state() {
        let projection = projection();
        let state =
            build_active_acp_pack_context_registry_state_v2(&projection, repo_config_root())
                .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("acp-pack-context-registry-state-v2.json");
        write_acp_pack_context_registry_state_v2(&path, &state).unwrap();

        let loaded = load_online_acp_pack_context_registry_state_v2(
            &projection,
            repo_config_root(),
            AcpPackContextRegistryLoadOptions::production(&path),
        )
        .unwrap();

        assert_eq!(loaded, state);
    }

    #[test]
    fn production_key_material_signs_and_verifies_persisted_state() {
        let projection = projection();
        let signing_key = production_fixture_signing_key();
        let keyring = AcpPackContextSigningKeyring::new(vec![signing_key.clone()]);
        let state = build_active_acp_pack_context_registry_state_v2_with_signing_key(
            &projection,
            repo_config_root(),
            &signing_key,
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("acp-pack-context-prod-registry-state-v2.json");
        write_acp_pack_context_registry_state_v2(&path, &state).unwrap();

        let loaded = load_online_acp_pack_context_registry_state_v2_with_keyring(
            &projection,
            repo_config_root(),
            AcpPackContextRegistryLoadOptions::production(&path),
            &keyring,
        )
        .unwrap();

        assert_eq!(loaded, state);
        assert!(loaded.envelopes.iter().all(|envelope| {
            let signature = envelope.signature.as_ref().unwrap();
            signature.key_id == signing_key.key_id()
                && signature.algorithm == ACP_PACK_CONTEXT_SIGNATURE_ALGORITHM
        }));
    }

    #[test]
    fn production_key_material_refuses_signature_with_wrong_material() {
        let projection = projection();
        let signing_key = production_fixture_signing_key();
        let state = build_active_acp_pack_context_registry_state_v2_with_signing_key(
            &projection,
            repo_config_root(),
            &signing_key,
        )
        .unwrap();
        let wrong_key = AcpPackContextSigningKey::new(
            signing_key.key_id(),
            signing_key.algorithm(),
            b"different-production-fixture-key-material",
        );
        let wrong_keyring = AcpPackContextSigningKeyring::new(vec![wrong_key]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("acp-pack-context-prod-registry-state-v2.json");
        write_acp_pack_context_registry_state_v2(&path, &state).unwrap();

        let refusal = load_online_acp_pack_context_registry_state_v2_with_keyring(
            &projection,
            repo_config_root(),
            AcpPackContextRegistryLoadOptions::production(&path),
            &wrong_keyring,
        )
        .unwrap_err();

        assert_eq!(refusal.code, "acp_envelope_signature_invalid");
    }

    #[test]
    fn section_budget_overflow_records_deterministic_omission() {
        let projection = projection();
        let pack = projection
            .packs
            .iter()
            .find(|pack| pack.pack_id == "cbu-maintenance")
            .unwrap();
        let sections = envelope_sections(pack, &projection).unwrap();
        let budgets = [
            SectionBudget {
                name: "pack_summary",
                byte_limit: 1,
            },
            SectionBudget {
                name: "verb_bindings",
                byte_limit: 10 * 1024 * 1024,
            },
            SectionBudget {
                name: "verb_effects",
                byte_limit: 10 * 1024 * 1024,
            },
            SectionBudget {
                name: "macro_tiers",
                byte_limit: 10 * 1024 * 1024,
            },
            SectionBudget {
                name: "workbook_plans",
                byte_limit: 10 * 1024 * 1024,
            },
            SectionBudget {
                name: "diagnostic_taxonomy",
                byte_limit: 10 * 1024 * 1024,
            },
        ];

        let (first_hashes, first_budget) =
            budget_and_hash_sections_with_budgets(&sections, &budgets).unwrap();
        let (second_hashes, second_budget) =
            budget_and_hash_sections_with_budgets(&sections, &budgets).unwrap();

        assert_eq!(first_hashes, second_hashes);
        assert_eq!(first_budget, second_budget);
        assert_eq!(first_budget.omitted.len(), 1);
        assert_eq!(first_budget.omitted[0].section, "pack_summary");
        assert_eq!(
            first_budget.omitted[0].reason,
            "section_byte_limit_exceeded"
        );
        assert_eq!(first_budget.omitted[0].byte_limit, 1);
        assert!(first_budget.omitted[0].actual_bytes > 1);
        let pack_summary_report = first_budget
            .section_reports
            .iter()
            .find(|report| report.section == "pack_summary")
            .unwrap();
        assert!(pack_summary_report.omitted);
        assert_eq!(pack_summary_report.token_limit, 1);
    }

    #[test]
    fn lifecycle_fsm_only_moves_forward() {
        assert!(acp_pack_lifecycle_transition_allowed(
            AcpPackLifecycleState::Draft,
            AcpPackLifecycleState::Active
        ));
        assert!(acp_pack_lifecycle_transition_allowed(
            AcpPackLifecycleState::Active,
            AcpPackLifecycleState::Deprecated
        ));
        assert!(acp_pack_lifecycle_transition_allowed(
            AcpPackLifecycleState::Deprecated,
            AcpPackLifecycleState::Retired
        ));
        assert!(!acp_pack_lifecycle_transition_allowed(
            AcpPackLifecycleState::Active,
            AcpPackLifecycleState::Draft
        ));
        assert!(!acp_pack_lifecycle_transition_allowed(
            AcpPackLifecycleState::Retired,
            AcpPackLifecycleState::Active
        ));
    }
}

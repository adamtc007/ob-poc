//! Pack projection — boundary's typed projection of the pack catalogue
//! for ACP discovery.
//!
//! The pack catalogue's authoritative source is SemOS. Today it's fed
//! from disk via `ob-poc-journey::pack::load_packs_from_dir` and the
//! ob-poc integrator registers the provider below. Tomorrow it will be
//! fed via MCP from a SemOS-served catalogue API. The provider hook is
//! the seam where SemOS plugs in.
//!
//! What lives in this module:
//! - `PackProjection` — the projection boundary stores per pack.
//!   `indexing` carries everything needed for the ACP semantic-pack-
//!   selection algorithm; `context` is the typed ACP discovery payload
//!   that gets handed back to editors verbatim.
//! - `PackIndexing` — id/name/hash plus the pre-computed phrase set and
//!   allowed-verb set. Pre-computed at projection time so the runtime
//!   pack-selection loop never reaches into `AcpDagSemanticPackContext`.
//! - `set_pack_projection_provider` — function-pointer hook for the
//!   ob-poc integrator. Registered once at app startup.
//! - `get_pack_projection_provider` — called by `acp_dag_semantic`'s
//!   `semantic_index()` static initializer. Returns the registered
//!   provider, or an error if no provider has been registered.
//!
//! What does NOT live here:
//! - `PackManifest` (the YAML-loaded type) — that's an `ob-poc-journey`
//!   concern. Boundary never sees it.
//! - The projection function `&PackManifest -> PackProjection` — lives
//!   in ob-poc (the integrator), because it's the only crate that needs
//!   to know both the source manifest type and boundary's projection
//!   shape.
//! - Per-utterance state (selected pack, score, matched phrase) — those
//!   are computed in `acp_dag_semantic` from the indexing + context
//!   stored here.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::acp_dag_semantic::AcpDagSemanticPackContext;

/// Boundary's typed projection of one pack for ACP discovery.
///
/// Produced by the registered provider (see [`set_pack_projection_provider`])
/// and consumed by the ACP semantic-pack-selection algorithm in
/// `acp_dag_semantic`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackProjection {
    /// Indexing fields — what the pack-selection algorithm reads.
    pub indexing: PackIndexing,
    /// ACP discovery context — what editors receive verbatim when a pack
    /// is selected. Built once at projection time; the runtime selection
    /// loop never re-reads upstream manifest fields. `score` and
    /// `matched_phrase` are placeholders here (the per-utterance values
    /// are filled in by `acp_dag_semantic::pack_context_from_scored`).
    pub context: AcpDagSemanticPackContext,
}

/// Pre-computed indexing fields for one pack.
///
/// Everything the selection algorithm needs to match an utterance to a
/// pack and to filter verbs by pack membership. All computation happens
/// once at projection time; the runtime loop reads these directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackIndexing {
    /// Pack ID (matches `AcpDagSemanticPackContext::pack_id`).
    pub id: String,
    /// Human-readable pack name.
    pub name: String,
    /// Content hash of the source manifest (rendered by ob-poc-journey's
    /// loader). Surfaced to editors as `pack_hash` for cache invalidation.
    pub hash: String,
    /// Indexable phrases used by the pack-selection algorithm:
    /// the pack ID, the pack name, all `invocation_phrases` from the
    /// source manifest, plus workspace names rendered as space-separated.
    pub phrases: Vec<String>,
    /// Allowed-verb set, used to filter the verb-row scorer to verbs
    /// the active pack permits.
    pub allowed_verbs: BTreeSet<String>,
}

// ---------------------------------------------------------------------------
// Provider hook
// ---------------------------------------------------------------------------

/// A registered provider of pack projections.
///
/// The function pointer (`fn` not `Box<dyn Fn>`) keeps the hook
/// allocation-free and the contract sharp: the projection logic itself
/// lives in static code in the ob-poc integrator, not in a captured
/// closure.
pub type PackProjectionProvider = fn() -> Result<Vec<PackProjection>, String>;

static PROVIDER: OnceLock<PackProjectionProvider> = OnceLock::new();

/// Register the pack-projection provider. Idempotent — calling more than
/// once after the first set has no effect (the first registration wins,
/// matching `OnceLock` semantics).
///
/// Called once at app startup by the ob-poc integrator. The provider
/// closure is invoked lazily by [`get_pack_projection_provider`] the
/// first time `acp_dag_semantic`'s `semantic_index()` is queried.
///
/// Returns `Ok(())` if the registration took, `Err(())` if a provider
/// was already registered (caller can decide whether to log/panic).
pub fn set_pack_projection_provider(provider: PackProjectionProvider) -> Result<(), ()> {
    PROVIDER.set(provider).map_err(|_| ())
}

/// Fetch the registered pack-projection provider.
///
/// Returns `Err` if no provider has been registered. This is a hard
/// error from the caller's perspective — `acp_dag_semantic`'s
/// `semantic_index()` will surface it as a build-time failure of the
/// static index, which means the ob-poc integrator forgot to register
/// the provider during startup.
pub fn get_pack_projection_provider() -> Result<PackProjectionProvider, &'static str> {
    PROVIDER
        .get()
        .copied()
        .ok_or("pack-projection provider not registered (ob-poc app startup must call set_pack_projection_provider)")
}

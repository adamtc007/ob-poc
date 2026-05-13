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
use std::path::Path;
use std::sync::OnceLock;

use ob_poc_types::journey::pack_types::PackManifest;
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

// ---------------------------------------------------------------------------
// Pack-manifest provider (Phase 3C-prep, 2026-05-13)
// ---------------------------------------------------------------------------
//
// `acp_registry_projection` builds a rich registry projection (verb
// bindings, macro tiers, workbook plans) that reads `PackManifest`
// field-by-field, so it can't run off the already-projected
// `PackProjection`. Boundary still must not depend on `ob-poc-journey`
// (plan §6 decision 2), so the disk-loading path-to-manifests function
// is provided through a separate hook below.
//
// The integrator (ob-poc) registers both providers at startup; they
// share the same underlying `load_packs_from_dir` call internally.

/// A registered provider that loads pack manifests from a config root.
///
/// Function-pointer hook (allocation-free) matching the
/// [`PackProjectionProvider`] pattern. The integrator's implementation
/// calls `ob-poc-journey::pack::load_packs_from_dir(&path.join("packs"))`
/// and translates the loader error into a string.
pub type PackManifestProvider = fn(&Path) -> Result<Vec<(PackManifest, String)>, String>;

static MANIFEST_PROVIDER: OnceLock<PackManifestProvider> = OnceLock::new();

/// Register the pack-manifest provider. Idempotent — `OnceLock::set`
/// semantics; the first registration wins.
pub fn set_pack_manifest_provider(provider: PackManifestProvider) -> Result<(), ()> {
    MANIFEST_PROVIDER.set(provider).map_err(|_| ())
}

/// Fetch the registered pack-manifest provider.
///
/// Returns `Err` if no provider has been registered. Surfaced by
/// `acp_registry_projection::build_slice1_acp_registry_projection` so a
/// missing integrator registration becomes a visible projection-time
/// failure, not silent.
pub fn get_pack_manifest_provider() -> Result<PackManifestProvider, &'static str> {
    MANIFEST_PROVIDER
        .get()
        .copied()
        .ok_or("pack-manifest provider not registered (ob-poc app startup must call set_pack_manifest_provider)")
}

// ---------------------------------------------------------------------------
// Test-only fixture provider
// ---------------------------------------------------------------------------
//
// Phase 3 of the capability-crate restructure splits pack-catalogue
// loading (ob-poc-journey owns it) from pack-projection (ob-poc-boundary
// owns it). The production projection function lives in the ob-poc
// integrator (Phase 3D). Boundary's tests still need to exercise the
// semantic-pack-selection algorithm against real pack data, so this
// `#[cfg(test)]` block carries a parallel projection function that
// mirrors what ob-poc will do at startup. Duplication is intentional:
// it keeps boundary's tests standalone and provides a concrete spec for
// what the ob-poc projection must produce. If the production projection
// diverges from this fixture, the tests will catch the drift.

#[cfg(test)]
pub(crate) fn ensure_test_provider_registered() {
    // Idempotent — both hooks are `OnceLock::set`, so first registration
    // wins; concurrent test calls are safe.
    let _ = set_pack_projection_provider(load_test_pack_projections);
    let _ = set_pack_manifest_provider(load_test_pack_manifests);
}

#[cfg(test)]
fn load_test_pack_manifests(
    config_root: &Path,
) -> Result<Vec<(PackManifest, String)>, String> {
    use ob_poc_journey::pack::load_packs_from_dir;
    let packs_dir = config_root.join("packs");
    load_packs_from_dir(&packs_dir).map_err(|error| error.to_string())
}

#[cfg(test)]
fn load_test_pack_projections() -> Result<Vec<PackProjection>, String> {
    use ob_poc_journey::pack::load_packs_from_dir;
    use std::path::Path;
    // CARGO_MANIFEST_DIR resolves to repo/rust/crates/ob-poc-boundary; the
    // shared config tree lives at repo/rust/config (two levels up).
    let packs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/packs");
    let packs = load_packs_from_dir(&packs_dir).map_err(|error| error.to_string())?;
    Ok(packs.into_iter().map(test_project_pack).collect())
}

#[cfg(test)]
fn test_project_pack(
    (manifest, hash): (ob_poc_types::journey::pack_types::PackManifest, String),
) -> PackProjection {
    use crate::acp_dag_semantic::workspace_context_name;
    use crate::acp_dag_semantic::{
        AcpDagSemanticPackContext, AcpDagSemanticPackProgressSignal, AcpDagSemanticPackQuestion,
        AcpDagSemanticPackRiskPolicy, AcpDagSemanticPackSection, AcpDagSemanticPackTemplate,
        AcpDagSemanticPackTemplateStep,
    };

    // Build indexing — id/name/hash + phrase set + allowed-verb set.
    let mut phrases = std::collections::BTreeSet::new();
    phrases.insert(manifest.id.clone());
    phrases.insert(manifest.name.clone());
    for phrase in &manifest.invocation_phrases {
        phrases.insert(phrase.clone());
    }
    for workspace in &manifest.workspaces {
        phrases.insert(workspace_context_name(workspace).replace('_', " "));
    }
    let indexing = PackIndexing {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        hash: hash.clone(),
        phrases: phrases.into_iter().collect(),
        allowed_verbs: manifest.allowed_verbs.iter().cloned().collect(),
    };

    // Build context — the full ACP discovery payload. Score and
    // matched_phrase are placeholders; `pack_context_from_scored` patches
    // them in per-utterance.
    let context = AcpDagSemanticPackContext {
        pack_id: manifest.id.clone(),
        pack_name: manifest.name.clone(),
        pack_version: manifest.version.clone(),
        pack_hash: hash,
        score: 0.0,
        matched_phrase: None,
        description: manifest.description.clone(),
        invocation_phrases: manifest.invocation_phrases.clone(),
        workspaces: manifest
            .workspaces
            .iter()
            .map(workspace_context_name)
            .collect(),
        required_context: manifest.required_context.clone(),
        optional_context: manifest.optional_context.clone(),
        allowed_verbs: manifest.allowed_verbs.clone(),
        allowed_verb_count: manifest.allowed_verbs.len(),
        forbidden_verbs: manifest.forbidden_verbs.clone(),
        risk_policy: AcpDagSemanticPackRiskPolicy {
            require_confirm_before_execute: manifest.risk_policy.require_confirm_before_execute,
            max_steps_without_confirm: manifest.risk_policy.max_steps_without_confirm,
        },
        required_questions: manifest
            .required_questions
            .iter()
            .map(|question| AcpDagSemanticPackQuestion {
                field: question.field.clone(),
                prompt: question.prompt.clone(),
                answer_kind: format!("{:?}", question.answer_kind),
                options_source: question.options_source.clone(),
                default: question.default.clone(),
                ask_when: question.ask_when.clone(),
            })
            .collect(),
        optional_questions: manifest
            .optional_questions
            .iter()
            .map(|question| AcpDagSemanticPackQuestion {
                field: question.field.clone(),
                prompt: question.prompt.clone(),
                answer_kind: format!("{:?}", question.answer_kind),
                options_source: question.options_source.clone(),
                default: question.default.clone(),
                ask_when: question.ask_when.clone(),
            })
            .collect(),
        stop_rules: manifest.stop_rules.clone(),
        templates: manifest
            .templates
            .iter()
            .map(|template| AcpDagSemanticPackTemplate {
                template_id: template.template_id.clone(),
                when_to_use: template.when_to_use.clone(),
                steps: template
                    .steps
                    .iter()
                    .map(|step| AcpDagSemanticPackTemplateStep {
                        verb: step.verb.clone(),
                        args: step
                            .args
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect(),
                        repeat_for: step.repeat_for.clone(),
                        when: step.when.clone(),
                        execution_mode: step.execution_mode.clone(),
                    })
                    .collect(),
            })
            .collect(),
        pack_summary_template: manifest.pack_summary_template.clone(),
        section_layout: manifest
            .section_layout
            .iter()
            .map(|section| AcpDagSemanticPackSection {
                title: section.title.clone(),
                verb_prefixes: section.verb_prefixes.clone(),
            })
            .collect(),
        definition_of_done: manifest.definition_of_done.clone(),
        progress_signals: manifest
            .progress_signals
            .iter()
            .map(|signal| AcpDagSemanticPackProgressSignal {
                signal: signal.signal.clone(),
                description: signal.description.clone(),
            })
            .collect(),
        handoff_target: manifest.handoff_target.clone(),
    };

    PackProjection { indexing, context }
}

//! In-memory SemOS index loader for a Sage ACP session.
//!
//! Phase 2.5 of the Sage ACP capability plan. The agent holds
//! SemOS-resident knowledge in agent memory so the planning loop can
//! ground a draft without round-tripping every prompt against the
//! substrate. For the spike this is one pack manifest + the verb
//! allowlist/denylist it declares. Phase 4 extends the index to the
//! full Motivated Sage surface (scoped verb surface, AffinityGraph,
//! NOM equivalent, FSM transitions, constellation walk) and swaps the
//! disk-backed loader for a `sem_os_mcp`-backed one.
//!
//! ## Design
//!
//! - [`SessionIndex`] — the read-only snapshot the planning loop sees.
//!   Loaded once at session start, refreshed on dirty-flag propagation
//!   (Phase 4 wiring).
//! - [`IndexLoader`] — async trait that produces a `SessionIndex` for
//!   a `(workspace, pack_id)` pair. Lets the spike use a disk loader
//!   while the productionised path uses an MCP loader, without
//!   changing the planning loop.
//! - [`DiskPackIndexLoader`] — concrete loader that reads pack
//!   manifests from a directory using
//!   `ob_poc_journey::pack::load_packs_from_dir`. Filters by pack id
//!   and workspace. Used by the spike binary at startup.
//!
//! The spike intentionally loads only ONE pack (the one the editor
//! requested in `initialize`). Loading the full catalogue is a Phase 4
//! concern once the dirty-flag refresh is wired.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ob_poc_journey::pack::{load_packs_from_dir, PackLoadError, PackManifest};
use ob_poc_types::session::kinds::WorkspaceKind;
use thiserror::Error;

/// The in-memory knowledge snapshot for a single Sage ACP session.
///
/// The Sage planning loop reads this to ground its draft proposals
/// against the SemOS-sanctioned surface. Constrained composition
/// guarantee: the LLM may only select from
/// [`SessionIndex::allowed_verbs`] minus [`SessionIndex::forbidden_verbs`].
///
/// Phase 4 will extend this with the scoped verb surface (from
/// `ValidVerbSetEngine`), the constellation map, and the macro/pack
/// catalogue for compound-intent matching.
#[derive(Debug, Clone)]
pub struct SessionIndex {
    /// The pack manifest the session is anchored to.
    pub pack: PackManifest,
    /// SHA-256 of the raw YAML bytes — surfaces in the audit record so
    /// post-hoc replay can verify the agent saw the exact manifest the
    /// editor approved.
    pub pack_hash: String,
    /// Workspace this session belongs to. Drives ABAC / scope
    /// filtering when the index is widened.
    pub workspace: WorkspaceKind,
    /// When this index was loaded. Used by the dirty-flag refresh
    /// path to skip reloads when nothing has changed.
    pub loaded_at: DateTime<Utc>,
}

impl SessionIndex {
    /// Verbs the pack is sanctioned to use. The planning loop must
    /// confirm an LLM draft's verb FQN appears here before any draft
    /// is submitted to the LSP-shaped REPL channel.
    pub fn allowed_verbs(&self) -> &[String] {
        &self.pack.allowed_verbs
    }

    /// Verbs the pack must never use. Always checked after
    /// `allowed_verbs` — explicit denials override any allowlist hit.
    pub fn forbidden_verbs(&self) -> &[String] {
        &self.pack.forbidden_verbs
    }

    /// Whether `verb_fqn` is sanctioned for the current session. Both
    /// the allowlist hit and the denylist miss must agree.
    pub fn is_verb_sanctioned(&self, verb_fqn: &str) -> bool {
        if self.forbidden_verbs().iter().any(|v| v == verb_fqn) {
            return false;
        }
        self.allowed_verbs().iter().any(|v| v == verb_fqn)
    }
}

/// What `IndexLoader::load` needs to know to populate a session index.
#[derive(Debug, Clone)]
pub struct IndexLoadRequest {
    /// Workspace the editor session targets.
    pub workspace: WorkspaceKind,
    /// Pack id (e.g. `"book-setup"`) the editor selected in
    /// `initialize`. Phase 2 loads exactly one pack per session.
    pub pack_id: String,
}

/// Async loader producing a [`SessionIndex`] for a session.
///
/// Two impls planned:
/// - [`DiskPackIndexLoader`] — reads manifests from a YAML directory.
///   Used by the spike binary (no MCP server required to ship a
///   demonstrable Phase 2 slice).
/// - `SemOsMcpIndexLoader` (Phase 4) — calls
///   `sem_os_mcp::pack_catalogue` over MCP. The disk loader becomes a
///   developer convenience; production reads from the substrate.
#[async_trait]
pub trait IndexLoader: Send + Sync {
    async fn load(&self, request: &IndexLoadRequest) -> Result<SessionIndex, IndexLoadError>;
}

/// Disk-backed loader that reads pack manifests from a directory.
///
/// The spike binary constructs one of these pointed at
/// `rust/config/packs/`. Phase 4 retires this in favour of the MCP
/// loader once `sem_os_mcp` lands.
#[derive(Debug, Clone)]
pub struct DiskPackIndexLoader {
    packs_dir: PathBuf,
}

impl DiskPackIndexLoader {
    /// Construct a loader rooted at `packs_dir`. The directory must
    /// contain `*.yaml` / `*.yml` files conforming to the
    /// `PackManifest` schema. Non-existence is deferred to load time.
    pub fn new(packs_dir: impl AsRef<Path>) -> Self {
        Self {
            packs_dir: packs_dir.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl IndexLoader for DiskPackIndexLoader {
    async fn load(&self, request: &IndexLoadRequest) -> Result<SessionIndex, IndexLoadError> {
        let packs = load_packs_from_dir(&self.packs_dir).map_err(IndexLoadError::Pack)?;
        let (manifest, hash) = packs
            .into_iter()
            .find(|(manifest, _)| manifest.id == request.pack_id)
            .ok_or_else(|| IndexLoadError::PackNotFound {
                pack_id: request.pack_id.clone(),
                packs_dir: self.packs_dir.display().to_string(),
            })?;

        if !manifest.workspaces.contains(&request.workspace) {
            return Err(IndexLoadError::WorkspaceMismatch {
                pack_id: request.pack_id.clone(),
                requested: request.workspace.clone(),
                manifest_workspaces: manifest.workspaces.clone(),
            });
        }

        Ok(SessionIndex {
            pack: manifest,
            pack_hash: hash,
            workspace: request.workspace.clone(),
            loaded_at: Utc::now(),
        })
    }
}

/// Errors produced when populating a [`SessionIndex`].
#[derive(Debug, Error)]
pub enum IndexLoadError {
    #[error("pack load failed: {0:?}")]
    Pack(PackLoadError),
    #[error("pack '{pack_id}' not found in {packs_dir}")]
    PackNotFound { pack_id: String, packs_dir: String },
    #[error(
        "pack '{pack_id}' does not declare workspace {requested:?}; declared workspaces: \
         {manifest_workspaces:?}"
    )]
    WorkspaceMismatch {
        pack_id: String,
        requested: WorkspaceKind,
        manifest_workspaces: Vec<WorkspaceKind>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_journey::pack::load_pack_from_bytes;

    fn book_setup_manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: Spike fixture for ob-poc-agent index loader tests.
invocation_phrases:
  - "set up book for"
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs:
  - cbu.create
  - cbu.attach-product
forbidden_verbs:
  - cbu.delete
required_questions: []
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done: []
progress_signals: []
"#
    }

    #[test]
    fn session_index_verb_sanction_check() {
        let (manifest, hash) = load_pack_from_bytes(book_setup_manifest_yaml())
            .expect("fixture parses");
        let index = SessionIndex {
            pack: manifest,
            pack_hash: hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        };

        assert!(index.is_verb_sanctioned("cbu.create"));
        assert!(index.is_verb_sanctioned("cbu.attach-product"));
        // Denylist overrides anything else.
        assert!(!index.is_verb_sanctioned("cbu.delete"));
        // Unlisted verbs are not sanctioned.
        assert!(!index.is_verb_sanctioned("cbu.archive"));
    }
}

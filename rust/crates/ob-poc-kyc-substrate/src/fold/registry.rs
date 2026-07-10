//! FoldRegistry: content-addressed version-dispatch for replay-faithfulness.
//!
//! D2 of EOP-DD-KYCUBO-002: "semantic change = new hash = new verb identity;
//! stream migration is a governed re-fold, never in-place reinterpretation."
//!
//! # INVARIANT
//!
//! **Dispatch is TOTAL.** An unregistered `lexicon_hash` is a hard error
//! (`KycError::UnregisteredLexiconHash`), never a silent fallback to the
//! "current/latest" version.  A fallback silently destroys replay-faithfulness
//! (Q7, K-18/K-31) — that is the failure mode D2 exists to prevent.
//!
//! **Registry is BTreeMap-deterministic.** No HashMap; consistent with the
//! closed determinism class enforced across the fold path.
//!
//! **No fallback fold.** There is deliberately no `get_or_default`, no
//! `resolve_latest`, no implicit version coercion.  If a fold version needs to
//! be added, register it explicitly under its content-addressed hash.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::error::KycError;
use crate::event::IntentEvent;
use crate::fold::control::{apply_one_control_event, ControlState};
use crate::fold::obligation::{apply_one_obligation_event, ObligationState};
use crate::types::Hash;

// ── Trait ────────────────────────────────────────────────────────────────────

/// Fold behaviour for one lexicon version.
///
/// Implementing types are registered in `FoldRegistry` under a lexicon manifest
/// hash.  Each `IntentEvent` carries the hash it was written against; the
/// registry dispatches per-event, so a stream written under vN replays under
/// vN's impl even after vN+1 is registered.
pub trait FoldImpl: Send + Sync {
    /// Apply one event's contribution to the control graph state.
    fn apply_control(&self, state: ControlState, event: &IntentEvent) -> ControlState;
    /// Apply one event's contribution to the obligation graph state.
    fn apply_obligation(&self, state: ObligationState, event: &IntentEvent) -> ObligationState;
}

// ── V1 implementation (the current lexicon) ───────────────────────────────────

/// `FoldImpl` for the initial lexicon version (the `phase1_lexicon()` manifest).
///
/// Wraps `apply_one_control_event` and `apply_one_obligation_event` — the
/// authoritative v1 fold logic extracted from `fold/control.rs` and
/// `fold/obligation.rs`.
pub struct V1FoldImpl;

impl FoldImpl for V1FoldImpl {
    fn apply_control(&self, state: ControlState, event: &IntentEvent) -> ControlState {
        apply_one_control_event(state, event)
    }

    fn apply_obligation(&self, state: ObligationState, event: &IntentEvent) -> ObligationState {
        apply_one_obligation_event(state, event)
    }
}

// ── Registry ──────────────────────────────────────────────────────────────────

/// Content-addressed fold registry: `lexicon_manifest_hash → Arc<dyn FoldImpl>`.
///
/// Register with [`FoldRegistry::register`].  Resolve with [`FoldRegistry::get`]
/// — hard error if absent, no fallback.
#[derive(Default)]
pub struct FoldRegistry {
    /// BTreeMap for determinism (no HashMap — determinism class invariant).
    impls: BTreeMap<Hash, Arc<dyn FoldImpl>>,
}

impl FoldRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            impls: BTreeMap::new(),
        }
    }

    /// Register a fold implementation under `hash`.
    ///
    /// Registering the same `hash` twice overwrites silently (second registration
    /// wins).  In practice each hash is unique by content-address construction.
    pub fn register(&mut self, hash: Hash, impl_: Arc<dyn FoldImpl>) {
        self.impls.insert(hash, impl_);
    }

    /// Resolve the fold implementation for `hash`.
    ///
    /// # Errors
    /// `KycError::UnregisteredLexiconHash` — **no fallback**.  An unknown hash
    /// is a replay-integrity error; the caller must register the version before
    /// replaying events written against it.
    pub fn get(&self, hash: &Hash) -> Result<&Arc<dyn FoldImpl>, KycError> {
        self.impls
            .get(hash)
            .ok_or(KycError::UnregisteredLexiconHash(*hash))
    }

    /// Number of registered versions.
    pub fn len(&self) -> usize {
        self.impls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.impls.is_empty()
    }
}

// ── Versioned fold entry points ───────────────────────────────────────────────

/// Fold an event stream's control state, dispatching each event through the
/// registry on `event.lexicon_hash`.
///
/// # Errors
/// Propagates `KycError::UnregisteredLexiconHash` on the first event whose
/// `lexicon_hash` is not registered — the fold halts, no partial state is
/// returned.
pub fn fold_control_versioned(
    events: &[&IntentEvent],
    registry: &FoldRegistry,
) -> Result<ControlState, KycError> {
    let mut state = ControlState::default();
    for event in events {
        let impl_ = registry.get(&event.lexicon_hash)?;
        state = impl_.apply_control(state, event);
    }
    Ok(state)
}

/// Fold an event stream's obligation state, dispatching each event through the
/// registry on `event.lexicon_hash`.
///
/// # Errors
/// Propagates `KycError::UnregisteredLexiconHash` on the first unregistered hash.
pub fn fold_obligations_versioned(
    events: &[&IntentEvent],
    registry: &FoldRegistry,
) -> Result<ObligationState, KycError> {
    let mut state = ObligationState::default();
    for event in events {
        let impl_ = registry.get(&event.lexicon_hash)?;
        state = impl_.apply_obligation(state, event);
    }
    Ok(state)
}

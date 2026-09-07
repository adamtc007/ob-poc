//! Preview / lookahead — EOP-PLAN-KYCUBO-KIT-001 T3 (closes KIT-7).
//!
//! `preview(committed, candidates, lexicon) -> Result<(ControlState,
//! TypeRegistryState), KycError>` folds `committed ++ candidates`, zero
//! store involvement. Chain preview
//! mirrors bpmn-lite's `resolve_hypothetical_chain` semantics
//! (`utterance-engine/src/bpmn_board.rs`, commit `68723b9`) — per-step
//! admission: candidate *n* validates against the state folded from
//! `committed ++ candidates[..n-1]` — without cloning a DAG, because the
//! control fold is already callable store-free over `&[&IntentEvent]`.
//!
//! **Commit-by-replay only, by construction.** This module has no append,
//! no store handle, nothing to commit *to* — the only way a previewed line
//! becomes real is replaying its candidate events through the real governed
//! append path (T1's `source_text` capture + the store's own precondition
//! check). There is no speculative-commit path to accidentally reach for.

use crate::error::KycError;
use crate::event::IntentEvent;
use crate::fold::control::{apply_one_control_event, check_preconditions, fold_control, ControlState};
use crate::fold::type_registry::{fold_type_registry, TypeRegistryState};
use crate::lexicon::LexiconManifest;

/// Fold `committed ++ candidates` into the resulting `(ControlState,
/// TypeRegistryState)` pair, admitting each candidate in turn against the
/// state folded from everything before it (committed history, then prior
/// candidates in the chain). Rejects at the first candidate whose lexicon
/// preconditions fail — steps before it are never applied to the returned
/// state, and nothing is written anywhere (pure function, no store
/// dependency).
///
/// Was a 3-tuple with `ObligationState` (T6.1(a), "the unified checker") —
/// the obligation fold was removed (EOP-DD-UBO-CLEANOUT-001 T6 P2,
/// 2026-09-07); see `lib.rs`'s module doc for the full reasoning.
pub fn preview(
    committed: &[IntentEvent],
    candidates: &[IntentEvent],
    lexicon: &LexiconManifest,
) -> Result<(ControlState, TypeRegistryState), KycError> {
    let committed_refs: Vec<&IntentEvent> = committed.iter().collect();
    let mut control = fold_control(&committed_refs);
    // No incremental `apply_one_type_registry_event` exists — refold the
    // growing prefix each step.
    let mut refs_so_far: Vec<&IntentEvent> = committed_refs;
    let mut type_registry = fold_type_registry(&refs_so_far);

    for candidate in candidates {
        let entry = lexicon
            .get(candidate.verb_fqn.as_str())
            .ok_or_else(|| KycError::UnknownVerb(candidate.verb_fqn.clone()))?;
        check_preconditions(entry, &control, &type_registry, candidate)?;
        control = apply_one_control_event(control, candidate);
        refs_so_far.push(candidate);
        type_registry = fold_type_registry(&refs_so_far);
    }

    Ok((control, type_registry))
}

//! Preview / lookahead — EOP-PLAN-KYCUBO-KIT-001 T3 (closes KIT-7).
//!
//! `preview(committed, candidates, lexicon) -> Result<ControlState, KycError>`
//! folds `committed ++ candidates`, zero store involvement. Chain preview
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
use crate::fold::control::{apply_one_control_event, check_control_preconditions, fold_control, ControlState};
use crate::lexicon::LexiconManifest;

/// Fold `committed ++ candidates` into the resulting `ControlState`,
/// admitting each candidate in turn against the state folded from
/// everything before it (committed history, then prior candidates in the
/// chain). Rejects at the first candidate whose lexicon preconditions fail
/// — steps before it are never applied to the returned state, and nothing
/// is written anywhere (pure function, no store dependency).
pub fn preview(
    committed: &[IntentEvent],
    candidates: &[IntentEvent],
    lexicon: &LexiconManifest,
) -> Result<ControlState, KycError> {
    let committed_refs: Vec<&IntentEvent> = committed.iter().collect();
    let mut state = fold_control(&committed_refs);

    for candidate in candidates {
        let entry = lexicon
            .get(candidate.verb_fqn.as_str())
            .ok_or_else(|| KycError::UnknownVerb(candidate.verb_fqn.clone()))?;
        check_control_preconditions(entry, &state, candidate)?;
        state = apply_one_control_event(state, candidate);
    }

    Ok(state)
}

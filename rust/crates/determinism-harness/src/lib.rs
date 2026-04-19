//! Determinism harness — Phase 0e skeleton.
//!
//! Implements the v0.3 §9.4 contract: runs the full Sequencer pipeline
//! twice per fixture and byte-compares stage outputs 4, 5, 6, 7, 8 plus
//! stage-9a `PendingStateAdvance` plus the outbox row set.
//!
//! # Phase 0e scope
//!
//! This crate is a **scaffold**. The concrete comparison APIs are sized
//! here so Phases 5b–5e can populate fixtures and comparison modes against
//! a stable interface.
//!
//! # Non-goals
//!
//! - No CLI. Drivers live in `xtask` (added in Phase 0h later slice).
//! - No actual Sequencer implementation (that's Phase 5b).
//! - No fixture DB orchestration (Phase 5e).

use ob_poc_types::{
    GatedOutcome, GatedVerbEnvelope, OutboxDraft, PendingStateAdvance,
};
use serde::{Deserialize, Serialize};

/// A single stage-capture point. Each run emits one of these per stage that
/// the harness is configured to compare.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StageCapture {
    /// Stage 4: SessionVerbSurface output — the derived candidate verb set.
    /// Phase 5b: populated from `sequencer::stage_4_disclose_surface`.
    VerbSurface { verbs: Vec<String> },

    /// Stage 5: NLP match output — selected verb + arg binding.
    NlpMatch { selected: String, args_json: String },

    /// Stage 6: gate decision — emitted envelope or rejection.
    GateDecision { envelope: Option<GatedVerbEnvelope> },

    /// Stage 7: compiled runbook — ordered list of pre-gated envelopes.
    RunbookCompile {
        step_envelopes: Vec<GatedVerbEnvelope>,
    },

    /// Stage 8 inner-loop outcome per step.
    DispatchOutcome {
        step_seq: usize,
        outcome: GatedOutcome,
    },

    /// Stage 9a applied PendingStateAdvance per step.
    StateAdvanceApplied {
        step_seq: usize,
        advance: PendingStateAdvance,
    },

    /// Stage 8 outbox row set written inside the txn (pre-commit).
    OutboxRows { drafts: Vec<OutboxDraft> },
}

impl StageCapture {
    /// Compute a byte-identity fingerprint for this capture. Two runs that
    /// produce the same `StageCapture` MUST produce the same hash.
    pub fn fingerprint(&self) -> [u8; 32] {
        // Serialise through `serde_json` in a canonical mode — the same
        // value produces byte-identical JSON across runs because field
        // order is fixed by struct-derived Serialize impl.
        let bytes =
            serde_json::to_vec(self).expect("StageCapture must serialise deterministically");
        *blake3::hash(&bytes).as_bytes()
    }
}

/// One complete harness run — ordered captures across all stages. Intended
/// to be stored as JSON alongside its input fixture and diffed across
/// back-to-back runs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessRun {
    pub fixture_id: String,
    pub captures: Vec<StageCapture>,
}

impl HarnessRun {
    /// Compare two runs produced from the same fixture. Returns the first
    /// divergent stage, or `None` if byte-identical across all captures.
    ///
    /// This is the primary determinism assertion in the v0.3 §9.1 sense:
    /// given fixed snapshot + utterance, stage outputs 4..=8 +
    /// PendingStateAdvance + outbox rows must match byte-for-byte.
    pub fn first_divergence<'a>(
        lhs: &'a HarnessRun,
        rhs: &'a HarnessRun,
    ) -> Option<StageDivergence<'a>> {
        if lhs.fixture_id != rhs.fixture_id {
            return Some(StageDivergence::FixtureMismatch {
                lhs: &lhs.fixture_id,
                rhs: &rhs.fixture_id,
            });
        }
        if lhs.captures.len() != rhs.captures.len() {
            return Some(StageDivergence::LengthMismatch {
                lhs_len: lhs.captures.len(),
                rhs_len: rhs.captures.len(),
            });
        }
        for (i, (l, r)) in lhs.captures.iter().zip(rhs.captures.iter()).enumerate() {
            if l != r {
                return Some(StageDivergence::StageDiffers {
                    index: i,
                    lhs: l,
                    rhs: r,
                });
            }
        }
        None
    }
}

/// A divergence reason returned by [`HarnessRun::first_divergence`].
#[derive(Debug)]
pub enum StageDivergence<'a> {
    FixtureMismatch { lhs: &'a str, rhs: &'a str },
    LengthMismatch { lhs_len: usize, rhs_len: usize },
    StageDiffers {
        index: usize,
        lhs: &'a StageCapture,
        rhs: &'a StageCapture,
    },
}

// ---------------------------------------------------------------------------
// Tests — meta-tests verifying the harness itself is deterministic.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_capture() -> StageCapture {
        StageCapture::VerbSurface {
            verbs: vec!["cbu.ensure".into(), "cbu.inspect".into()],
        }
    }

    #[test]
    fn fingerprint_is_deterministic_across_calls() {
        let a = sample_capture();
        let f1 = a.fingerprint();
        let f2 = a.fingerprint();
        assert_eq!(f1, f2);
    }

    #[test]
    fn equal_captures_equal_fingerprints() {
        let a = sample_capture();
        let b = sample_capture();
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn different_captures_different_fingerprints() {
        let a = sample_capture();
        let b = StageCapture::VerbSurface {
            verbs: vec!["cbu.ensure".into()],
        };
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn harness_run_identical_reports_no_divergence() {
        let run = HarnessRun {
            fixture_id: "fixture-001".into(),
            captures: vec![sample_capture()],
        };
        assert!(HarnessRun::first_divergence(&run, &run).is_none());
    }

    #[test]
    fn harness_run_different_fixture_ids_reported() {
        let a = HarnessRun {
            fixture_id: "a".into(),
            captures: vec![],
        };
        let b = HarnessRun {
            fixture_id: "b".into(),
            captures: vec![],
        };
        assert!(matches!(
            HarnessRun::first_divergence(&a, &b),
            Some(StageDivergence::FixtureMismatch { .. })
        ));
    }

    #[test]
    fn harness_run_detects_stage_diff() {
        let a = HarnessRun {
            fixture_id: "x".into(),
            captures: vec![StageCapture::VerbSurface { verbs: vec![] }],
        };
        let b = HarnessRun {
            fixture_id: "x".into(),
            captures: vec![StageCapture::VerbSurface {
                verbs: vec!["cbu.ensure".into()],
            }],
        };
        assert!(matches!(
            HarnessRun::first_divergence(&a, &b),
            Some(StageDivergence::StageDiffers { index: 0, .. })
        ));
    }
}

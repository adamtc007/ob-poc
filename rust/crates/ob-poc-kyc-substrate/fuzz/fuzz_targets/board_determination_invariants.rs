//! Board-move fuzzing (EOP-VS-UBO-GAME-001 §3.4 P3 — determination
//! invariants, no oracle) over `board::gen_board_sequence`. Dispatches the
//! subject's `EntityType` to the same `DeterminationStrategy`
//! `kyc_ubo.decide.determination.freeze` would use, and checks only
//! invariants a determination must satisfy regardless of the actual
//! answer: terminates, and never names an unpierced nominee as a UBO.
//!
//! **Finding #3 (2026-09-08), re-characterised and closed
//! (EOP-DD-UBO-BASES-001 §6):** this target used to also assert that no
//! candidate's `effective_ownership_pct` exceeds 100, under the name
//! "control never multiplied along a chain". That framing was wrong on
//! both counts — the minimised crash was a single hop (no chain), and
//! `effective_ownership_pct` is never `Some` on a control-axis candidate
//! at all (K-3). The real defect was an unbounded `percentage` input at
//! `connect`'s write path, now closed by `Precondition::
//! PercentageIsBounded` (see `tests/kyc_bases_001_percentage.rs`) — a
//! write-path precondition, not a fold/determination invariant, so this
//! pure-fold property no longer asserts it. See `properties::check_p3`'s
//! doc for the full reasoning.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate_fuzz::properties::check_p3;
use ob_poc_kyc_substrate_fuzz::Tape;

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    check_p3(&mut tape);
});

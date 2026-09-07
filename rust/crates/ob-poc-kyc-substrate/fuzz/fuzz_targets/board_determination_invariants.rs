//! Board-move fuzzing (EOP-VS-UBO-GAME-001 §3.4 P3 — determination
//! invariants, no oracle) over `board::gen_board_sequence`. Dispatches the
//! subject's `EntityType` to the same `DeterminationStrategy`
//! `kyc_ubo.decide.determination.freeze` would use, and checks only
//! invariants a determination must satisfy regardless of the actual
//! answer: terminates, never names an unpierced nominee as a UBO, and
//! never returns a resolved percentage above 100 (no chain multiplication).
//! See `properties::check_p3`'s doc for the full reasoning.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate_fuzz::properties::check_p3;
use ob_poc_kyc_substrate_fuzz::Tape;

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    check_p3(&mut tape);
});

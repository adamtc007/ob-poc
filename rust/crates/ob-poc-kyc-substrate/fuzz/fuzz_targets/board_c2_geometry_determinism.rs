//! Board-move fuzzing (EOP-VS-UBO-GAME-001 §3.4 C2 + geometry closure +
//! determinism) over `board::gen_board_sequence`. See `properties::check_p2`'s
//! doc for the exact sub-claims — C2 offered<=>admitted (positive half
//! checked unconditionally inside the generator itself; negative half
//! probed here), geometry closure over the live type registry, and
//! double-fold / fold-to-prefix determinism.
//!
//! **KNOWN, REPORTED FINDING (#2, not a harness bug):** geometry closure is
//! violated by connecting to/from a withdrawn entity that is later
//! re-placed with a different type — see `properties::check_p2`'s doc and
//! the P4 close-out report for the mechanism and an 11-event minimized
//! reproduction. This target is expected to find that violation quickly.
//! Do not "fix" by loosening the assertion in `properties.rs` — report
//! reproductions and let Adam rule on sequencing.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate_fuzz::properties::check_p2;
use ob_poc_kyc_substrate_fuzz::Tape;

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    check_p2(&mut tape);
});

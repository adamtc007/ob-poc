//! Board-move fuzzing (EOP-VS-UBO-GAME-001 §3.4 C2 + geometry closure +
//! determinism) over `board::gen_board_sequence`. See `properties::check_p2`'s
//! doc for the exact sub-claims — C2 offered<=>admitted (positive half
//! checked unconditionally inside the generator itself; negative half
//! probed here), geometry closure over the live type registry, and
//! double-fold / fold-to-prefix determinism.
//!
//! **FINDING #2 CLOSED (2026-09-08):** geometry closure used to be
//! violated by connecting to/from a withdrawn entity that was later
//! re-placed with a different type. RULED by Adam 2026-09-07: forbid the
//! connection — `Precondition::ConnectEndpointsNotWithdrawn` now refuses
//! any `connect` naming a withdrawn endpoint (a never-placed endpoint is
//! unaffected — R5/R6/CTN-2e, and the K-8 nominee mechanism, unchanged).
//! See `properties::check_p2`'s
//! doc for the mechanism and the committed regression corpus entry
//! (`fuzz/regressions/board_c2_geometry_determinism/`) for a minimized
//! reproduction that this target must no longer crash on.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate_fuzz::properties::check_p2;
use ob_poc_kyc_substrate_fuzz::Tape;

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    check_p2(&mut tape);
});

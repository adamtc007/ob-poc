//! Board-move fuzzing (EOP-VS-UBO-GAME-001 §3.4 R8): a legal move sequence
//! generated purely from `enumerate_placement_set`'s real offered moves —
//! see `board::gen_board_sequence`. Property: every prior board shape is
//! reachable again by forward moves — checked as "no orphan links, no
//! stranded placements" after fully emptying the board, plus "`remove`
//! prunes ONLY touching links." See `properties::check_r8`'s doc for the
//! exact sub-claims.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate_fuzz::properties::check_r8;
use ob_poc_kyc_substrate_fuzz::Tape;

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    check_r8(&mut tape);
});

//! Board-move fuzzing (EOP-DD-UBO-BASES-001 §4/§7 survival property) over
//! `board::gen_board_sequence`. Property: a person admitted by N
//! independent bases survives the removal of any N-1 of them, and the
//! surviving candidate still names exactly the one basis left standing.
//! See `properties::check_p4_basis_survival`'s doc for the exact mechanics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate_fuzz::properties::check_p4_basis_survival;
use ob_poc_kyc_substrate_fuzz::Tape;

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    check_p4_basis_survival(&mut tape);
});

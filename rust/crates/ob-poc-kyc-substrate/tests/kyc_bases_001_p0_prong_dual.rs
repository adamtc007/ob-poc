//! EOP-DD-UBO-BASES-001 §7 `prong_dual_is_gone` — RED via compiler error,
//! same discipline `tests/kyc_t4_dispatch.rs` documents for forward-looking
//! API ("first a compile error... then a real assertion failure"). An
//! exhaustive match with no catch-all and no `Dual` arm is a compile error
//! TODAY (E0004, non-exhaustive — `Dual` still exists in the enum) and
//! becomes valid, passing code once §4 retires the variant.

use ob_poc_kyc_substrate::Prong;

fn three_variants_only(p: Prong) -> &'static str {
    match p {
        Prong::OwnershipProng => "ownership",
        Prong::ControlByOtherMeans => "control",
        Prong::SmoFallback => "smo",
    }
}

#[test]
fn prong_dual_is_gone() {
    assert_eq!(three_variants_only(Prong::OwnershipProng), "ownership");
    assert_eq!(three_variants_only(Prong::ControlByOtherMeans), "control");
    assert_eq!(three_variants_only(Prong::SmoFallback), "smo");
}

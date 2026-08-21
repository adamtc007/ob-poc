//! Flagship target (EOP-FUZZ-KYCUBO-001 F2, §4 P0): O1 (no-panic), O2 (fold
//! determinism), O6 (cross-module smo_person_id/smo_event_id invariant).
//!
//! `ubo.determination.freeze` — the event a KYC case is approved against
//! (K-23) — folds over exactly `fold_control` + `fold_obligations`. If
//! either panics, or produces different output on two calls over the
//! identical event slice, that's a correctness-of-record defect, not a
//! cosmetic bug.
//!
//! O6 note: `determination.rs:798-801`'s `panic!("fold invariant violated:
//! smo_person_id is Some but smo_event_id is None")` is reached through
//! `recover_determination_at`, which this target calls on every generated
//! sequence. `apply_one_control_event`'s `apply-smo-fallback` arm always
//! sets both fields from the *same* event
//! (`fold/control.rs:489-492`), so this panic should be structurally
//! unreachable via any sequence `gen_events` can produce — that
//! unreachability is exactly what O6 claims, and what this target spends
//! its cycles trying to falsify rather than asserting by inspection alone.
#![no_main]

use std::collections::BTreeSet;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate::{
    check_preconditions, fold_control, fold_obligations, fold_type_registry,
    natural_persons_from_events, phase1_lexicon, recover_determination_at, Hash, LexiconManifest,
    OwnershipProngStrategy, RecoveryPin,
};
use ob_poc_kyc_substrate_fuzz::{gen_events, Tape};
use uuid::Uuid;

fn lexicon() -> &'static LexiconManifest {
    static MANIFEST: OnceLock<LexiconManifest> = OnceLock::new();
    MANIFEST.get_or_init(phase1_lexicon)
}

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    let events = gen_events(&mut tape);
    let refs: Vec<&_> = events.iter().collect();

    // O1 + O2: both folds, called twice over the identical slice.
    let control1 = fold_control(&refs);
    let control2 = fold_control(&refs);
    assert_eq!(
        format!("{control1:?}"),
        format!("{control2:?}"),
        "fold_control produced different output on two calls over the identical event slice"
    );

    let obligations1 = fold_obligations(&refs);
    let obligations2 = fold_obligations(&refs);
    assert_eq!(
        format!("{obligations1:?}"),
        format!("{obligations2:?}"),
        "fold_obligations produced different output on two calls over the identical event slice"
    );

    // O1: check_preconditions against the last generated event, matched to
    // its real lexicon entry when the generator picked a known verb FQN.
    if let Some(last) = events.last() {
        if let Some(entry) = lexicon().entries.get(last.verb_fqn.as_str()) {
            let type_registry1 = fold_type_registry(&refs);
            let _ = check_preconditions(entry, &control1, &obligations1, &type_registry1, last);
        }
    }

    // O6: `recover_determination_at` reaches the smo pairing check via
    // `fold_control` internally — a real `panic!` if the invariant ever
    // breaks, which libFuzzer reports as a crash like any other.
    let natural_persons = natural_persons_from_events(&refs);
    let strategy = OwnershipProngStrategy;
    let threshold_pct = tape.percentage();
    let pin = RecoveryPin {
        policy_version: "fuzz",
        lexicon_manifest_hash: Hash::of(b"fuzz-manifest"),
        reference_snapshot_id: Uuid::nil(),
        import_run_ids: BTreeSet::new(),
        viewer: None,
    };
    let _ = recover_determination_at(&refs, &strategy, &natural_persons, threshold_pct, pin);
});

//! R6 structural gate — EOP-VS-UBO-GAME-001 §3.4, T1 P5.
//!
//! "Every surface builds the same event. One function maps declared
//! arguments to a stored event; every surface calls it. A second
//! constructor is how two surfaces come to disagree, and it is forbidden
//! rather than merely discouraged."
//!
//! Source-level scan, `include_str!`-based, mirroring
//! `runbook::invariant_tests`' pattern (internal test, crate test-boundary
//! rule — these reference sibling source files by relative path so cannot
//! be external harnesses). This is tier-1 static analysis in the CLAUDE.md
//! sense: it proves a property of the *source text* the compiler already
//! type-checked, not a property of runtime behavior — cheap, and it cannot
//! silently stop catching regressions the way a behavior-only test could if
//! someone routed a *new* call around the assertion without editing this
//! file. There is no `dyn`-dispatch blindness here (op registration is a
//! separate, already-covered concern — `cargo x registry-graph`); this gate
//! is purely "does op X's/the workbook's source text call the one function."

/// The 13 assembly-tier (board-buildable) verb FQNs `canonical_event_shape`
/// covers. Excludes `kyc_ubo.decide.determination.freeze` (§3.2 — computes a
/// verdict from live state, not declared args) and the three
/// `kyc_ubo.decide.{subject.approve,subject.reject,obligation.waiver}`
/// verdicts (TS.6 P2 — never build an `IntentEvent` at all, live in
/// `ob-poc-kyc-decide`, not `kyc_stream_ops.rs`).
///
/// T2 (EOP-VS-UBO-GAME-001, 2026-08-27, §8 Q1): `register`+`type` MERGED
/// into `place` (15→14, one FQN for two); `member-withdrawal` renamed
/// `remove` (no count change); `type-correction` DISSOLVED (14→13, no
/// replacement FQN — see `lexicon.rs`'s retirement comment).
const ASSEMBLY_VERB_FQNS: &[&str] = &[
    "kyc_ubo.assert.edge.control",
    "kyc_ubo.assert.edge.economic-interest",
    "kyc_ubo.assert.edge.evidence",
    "kyc_ubo.assert.edge.verification",
    "kyc_ubo.assert.edge.supersession",
    "kyc_ubo.assert.edge.reconciliation",
    "kyc_ubo.assert.subject.place",
    "kyc_ubo.assert.subject.structure-class",
    "kyc_ubo.assert.subject.remove",
    "kyc_ubo.assert.subject.enquiry",
    "kyc_ubo.assert.entity.identity",
    "kyc_ubo.assert.entity.screening",
    "kyc_ubo.assert.entity.risk",
];

/// Named, documented exemptions: direct `TargetBinding` construction sites
/// in `kyc_stream_ops.rs` that do NOT go through `canonical_event_shape`.
/// Exactly two, both explained in `canonical.rs`'s own module doc and
/// CLAUDE.md's KYC/UBO section:
/// - `freeze` — computes a verdict from live `ControlState`/`ObligationState`,
///   not from declared arguments; calling `canonical_event_shape` with this
///   FQN is itself a caller bug (it `bail!`s).
/// - `apply_screening_outcome_to_obligations` — an internal system fan-out
///   hook that synthesizes `kyc_ubo.assert.entity.screening` events from
///   live fold-derived `obligation_ids`, never from a caller's declared
///   arguments. Not a "surface" in R6's sense.
///
/// A third site appearing here means either a new second constructor snuck
/// in, or a new legitimate exemption exists that must be named here (and in
/// `canonical.rs`'s doc) before this count changes.
const OP_LAYER_DOCUMENTED_EXEMPTIONS: usize = 2;

#[test]
fn one_event_constructor_op_layer_covers_every_assembly_verb() {
    let src = include_str!("kyc_stream_ops.rs");
    for fqn in ASSEMBLY_VERB_FQNS {
        let needle = format!("canonical_event_shape(\"{fqn}\"");
        assert!(
            src.contains(&needle),
            "R6: kyc_stream_ops.rs's op for {fqn} must call canonical_event_shape — \
             no second constructor may build this verb's (target, payload) shape"
        );
    }
}

#[test]
fn one_event_constructor_op_layer_has_no_undocumented_second_constructor() {
    let src = include_str!("kyc_stream_ops.rs");

    let canonical_calls = src.matches("canonical_event_shape(").count();
    assert_eq!(
        canonical_calls,
        ASSEMBLY_VERB_FQNS.len(),
        "R6: kyc_stream_ops.rs has {canonical_calls} canonical_event_shape call(s), \
         expected exactly {} (one per assembly verb) — a mismatch means either a verb \
         lost its call, or a duplicate/second call was added for one verb",
        ASSEMBLY_VERB_FQNS.len()
    );

    let direct_target_binding = src.matches("TargetBinding::for_subject(subject)").count()
        + src.matches("TargetBinding::for_edge(").count()
        + src.matches("TargetBinding {").count();
    assert_eq!(
        direct_target_binding, OP_LAYER_DOCUMENTED_EXEMPTIONS,
        "R6: kyc_stream_ops.rs has {direct_target_binding} direct TargetBinding \
         construction site(s) outside canonical_event_shape, expected exactly \
         {OP_LAYER_DOCUMENTED_EXEMPTIONS} (freeze + apply_screening_outcome_to_obligations, \
         both named exemptions in this file's doc comment). If you added a legitimate \
         third exemption, name and document it there and in canonical.rs, then update \
         OP_LAYER_DOCUMENTED_EXEMPTIONS here. If not, this is a new undocumented second \
         constructor — route it through canonical_event_shape instead."
    );
}

#[test]
fn one_event_constructor_workbook_layer_has_exactly_one_call_and_no_second_constructor() {
    let src = include_str!("kyc_workbook.rs");

    let canonical_calls = src.matches("canonical_event_shape(").count();
    assert_eq!(
        canonical_calls, 1,
        "R6: kyc_workbook.rs must call canonical_event_shape exactly once, in \
         sexpr_to_parsed_move — found {canonical_calls}"
    );

    let direct_target_binding = src.matches("TargetBinding::for_subject(").count()
        + src.matches("TargetBinding::for_edge(").count()
        + src.matches("TargetBinding {").count();
    assert_eq!(
        direct_target_binding, 0,
        "R6: kyc_workbook.rs must not construct TargetBinding directly anywhere — \
         found {direct_target_binding} site(s); every shape decision must route \
         through canonical_event_shape, which sexpr_to_parsed_move already calls"
    );
}

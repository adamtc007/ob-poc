//! T0.3 pack closure audit — teeth (EOP-PLAN-KYCUBO-KIT-001 T0.3;
//! docs/todo/EOP-DD-KYCUBO-KIT-T0.3_Pack-Closure-Audit_v0.1.md).
//!
//! Pure — no `DATABASE_URL`. Seven tests statically enumerate declarations
//! (verb YAML, the DAG's `stream_governed` block, op registration, the
//! lexicon manifest, precondition/strategy coverage, entry sanity, and fold
//! match arms) via source scanning, matching this repo's own doctrine that
//! closed/match-dispatched questions belong to static analysis, not a
//! runtime harness (CLAUDE.md "Verification Strategy"). One test
//! (`edge_status_lifecycle_is_fully_reachable`) is a real behavioral
//! fold-chain check.
//!
//! Each test pins one row of the audit's gap register. A passing test means
//! "matches the audited state", not "no gap exists" — several of these
//! assert a known-open gap set is *exactly* what the audit found, not that
//! it's empty. Widening or shrinking a pinned set must touch this file
//! consciously, in either direction.
//!
//! The 9th tooth (EOP-PLAN-KYCUBO-KIT-001 Part A, the "checker-reach" tooth)
//! is `every_precondition_carrying_verb_is_reached_by_the_checker` +
//! `precondition_carrying_verbs_actually_enforce_their_stud` — two tests
//! answering two different halves of one question, counted as the plan's
//! single 9th tooth (a wiring proof needs a semantic-correctness proof
//! alongside it or a "wired to nothing meaningful" checker could pass it
//! vacuously). This is a hybrid:
//! `check_preconditions` cannot be driven through the REAL op `execute()` in
//! this file without `DATABASE_URL` (every `dsl.kyc` op takes a
//! `&mut dyn TransactionScope` backed by Postgres — there is no in-memory
//! substitute), so a live-DB drive-through would break this file's pure
//! discipline for what is otherwise a fast, no-DB pack. Per the tooth's own
//! fallback clause, the wiring question ("is `check_preconditions`/
//! `check_control_preconditions` genuinely reached when this verb's op runs")
//! is answered by source-level dispatch-site enumeration instead — scanning
//! `kyc_stream_ops.rs` for the two real call shapes (`stream_append`'s
//! `validate_entry_fqn` argument, and the two inline
//! `lexicon.get(fqn) → check_preconditions(entry, ...)` sites) — while the
//! *semantic* question ("does the checker actually enforce what the entry
//! declares") is answered behaviorally, reusing the same
//! `fold_control_versioned` + `check_preconditions` composition as
//! `edge_status_lifecycle_is_fully_reachable`, immediately below it. Together
//! they close the gap a pure-checker-only test would miss: a checker that is
//! semantically correct but never actually called from the real op (the
//! historical defect — freeze's dead `None` precondition, pre-DD-003) fails
//! the wiring half; a checker that is called but semantically wrong fails the
//! behavioral half.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::{TimeZone, Utc};

use ob_poc_kyc_substrate::{
    check_control_preconditions, check_preconditions, fold_control_versioned, phase1_lexicon,
    AuthorityRef, ControlState, EdgeId, EdgeStatus, EntityId, FoldRegistry, Hash, IntentEvent,
    ObligationState, Precondition, Principal, StructureClass, SubjectId, TargetBinding,
    V1FoldImpl,
};

const DSL_KYC_YAML: &str = include_str!("../config/verbs/kyc/dsl-kyc.yaml");
const DSL_KYC_OBLIGATION_YAML: &str = include_str!("../config/verbs/kyc/dsl-kyc-obligation.yaml");
const KYC_DAG_YAML: &str = include_str!("../config/sem_os_seeds/dag_taxonomies/kyc_dag.yaml");
const KYC_STREAM_OPS_SRC: &str = include_str!("../src/domain_ops/kyc_stream_ops.rs");
const CONTROL_FOLD_SRC: &str = include_str!("../crates/ob-poc-kyc-substrate/src/fold/control.rs");
const OBLIGATION_FOLD_SRC: &str =
    include_str!("../crates/ob-poc-kyc-substrate/src/fold/obligation.rs");

// ── Declaration extraction ──────────────────────────────────────────────────

/// Parse `domains: { <domain>: { verbs: { <verb>: ... } } }` from a dsl.kyc
/// verb YAML file — a plain line-scan (2-space domain keys, 6-space verb
/// keys) rather than a full YAML parse, matching the fixed hand-authored
/// indentation of these two files (verified: exactly 2 domain lines + 12 verb
/// lines in dsl-kyc.yaml, 1 domain line + 8 verb lines in
/// dsl-kyc-obligation.yaml — 20 total, post K-G7 retirement of
/// kyc.role.assign/withdraw, 2026-08-12).
fn extract_verb_fqns(yaml: &str) -> BTreeSet<String> {
    let mut fqns = BTreeSet::new();
    let mut domain: Option<&str> = None;
    for line in yaml.lines() {
        let is_2space = line.starts_with("  ") && !line.starts_with("   ");
        let is_6space = line.starts_with("      ") && !line.starts_with("       ");
        if is_2space && line.trim_end().ends_with(':') {
            domain = Some(line.trim().trim_end_matches(':'));
        } else if is_6space && line.trim_end().ends_with(':') {
            if let Some(d) = domain {
                fqns.insert(format!("{d}.{}", line.trim().trim_end_matches(':')));
            }
        }
    }
    fqns
}

fn declared_verb_universe() -> BTreeSet<String> {
    let mut fqns = extract_verb_fqns(DSL_KYC_YAML);
    fqns.extend(extract_verb_fqns(DSL_KYC_OBLIGATION_YAML));
    fqns
}

/// `verb_families:` glob list from `kyc_dag.yaml`'s `stream_governed` block.
fn stream_governed_globs() -> Vec<String> {
    let marker = "verb_families:";
    let start = KYC_DAG_YAML
        .find(marker)
        .expect("kyc_dag.yaml must declare stream_governed.verb_families");
    let mut globs = Vec::new();
    for line in KYC_DAG_YAML[start + marker.len()..].lines() {
        let trimmed = line.trim();
        match trimmed.strip_prefix("- ") {
            Some(g) => globs.push(g.trim().to_string()),
            None if !trimmed.is_empty() => break, // first non-list line ends the block
            None => {}
        }
    }
    globs
}

/// `fn fqn(&self) -> &str { "..." }` string literals from kyc_stream_ops.rs.
fn registered_op_fqns() -> BTreeSet<String> {
    let marker = "fn fqn(&self) -> &str {";
    let mut fqns = BTreeSet::new();
    let mut lines = KYC_STREAM_OPS_SRC.lines();
    while let Some(line) = lines.next() {
        if line.trim() == marker {
            if let Some(next) = lines.next() {
                let trimmed = next.trim();
                if let Some(literal) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
                {
                    fqns.insert(literal.to_string());
                }
            }
        }
    }
    fqns
}

/// `"literal" => ...` match-arm string literals from a fold source file.
///
/// Deliberately broad (also catches payload-value match arms inside helper
/// functions like `structure_class_from_payload`, e.g. `"private_company" =>
/// ...`) — safe because callers only ever probe membership for the 22 known
/// verb FQNs, which never collide with those payload-value literals, rather
/// than asserting on the raw set size.
fn fold_match_arms(src: &str) -> BTreeSet<String> {
    let mut arms = BTreeSet::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('"') {
            continue;
        }
        let mut parts = trimmed.splitn(3, '"');
        let _before = parts.next();
        let (Some(literal), Some(rest)) = (parts.next(), parts.next()) else {
            continue;
        };
        if rest.trim_start().starts_with("=>") {
            arms.insert(literal.to_string());
        }
    }
    arms
}

// ── §0 scope ─────────────────────────────────────────────────────────────

#[test]
fn verb_universe_is_exactly_20() {
    let fqns = declared_verb_universe();
    assert_eq!(
        fqns.len(),
        20,
        "dsl.kyc verb count drifted from the post-retirement 20 (K-G7: \
         kyc.role.assign/withdraw retired 2026-08-12) — update the T0.3 \
         audit and every other pinned test in this file, not just this \
         assertion: {fqns:#?}"
    );
}

// ── K-G3: unmapped move ─────────────────────────────────────────────────────

#[test]
fn stream_governed_family_covers_every_declared_verb() {
    let globs = stream_governed_globs();
    assert_eq!(
        globs.len(),
        6,
        "stream_governed.verb_families count drifted from the audited 6: {globs:#?}"
    );
    // The kyc.role.* glob is vacuous after the K-G7 retirement (no live verb
    // matches it) — left in kyc_dag.yaml deliberately: governance-block
    // edits are a separate conscious change from verb retirement, and the
    // glob costs nothing to leave (it covers zero verbs, not a wrong verb).

    for fqn in declared_verb_universe() {
        let covered = globs.iter().any(|g| match g.strip_suffix(".*") {
            Some(prefix) => fqn.starts_with(prefix) && fqn[prefix.len()..].starts_with('.'),
            None => false,
        });
        assert!(
            covered,
            "{fqn} is not covered by any stream_governed family glob {globs:#?} (K-G3)"
        );
    }
}

// ── K-G4: phantom move (and its inverse: unregistered live verb) ───────────

#[test]
fn every_declared_verb_has_a_registered_op() {
    let declared = declared_verb_universe();
    let registered = registered_op_fqns();
    assert_eq!(
        registered.len(),
        20,
        "registered dsl.kyc op count drifted from the post-retirement 20: {registered:#?}"
    );

    let missing_ops: Vec<_> = declared.difference(&registered).collect();
    let phantom_ops: Vec<_> = registered.difference(&declared).collect();
    assert!(
        missing_ops.is_empty(),
        "declared verbs with no registered op (K-G4): {missing_ops:#?}"
    );
    assert!(
        phantom_ops.is_empty(),
        "registered ops with no YAML declaration: {phantom_ops:#?}"
    );
}

// ── K-G6: lexicon-manifest coverage drift ───────────────────────────────────

#[test]
fn lexicon_manifest_coverage_gap_is_exactly_known() {
    // T6.0 closed this gap 2026-08-12: the 8 kyc.obligation.*/kyc.person.*
    // entries were authored (entries only — no preconditions, that stays
    // T6.1+). This row flips from pinning a RED-honest open gap to pinning
    // genuine closure — an empty uncovered set is now the correct, not the
    // aspirational, assertion.
    let lexicon = phase1_lexicon();
    let uncovered: BTreeSet<String> = declared_verb_universe()
        .into_iter()
        .filter(|fqn| lexicon.get(fqn).is_none())
        .collect();

    assert!(
        uncovered.is_empty(),
        "K-G6 lexicon-manifest coverage gap reopened — expected empty \
         (closed at T6.0), found: {uncovered:#?}"
    );
}

/// Entry-sanity check, one verb per newly-covered family (T6.0): the
/// authored `LexiconEntry`'s own `fqn` field matches its map key (catches a
/// hand-authored copy-paste mismatch — `render_intent_event_to_sexpr`'s
/// `debug_assert_eq!` on this exact equality was previously never exercised
/// for these FQNs, since `render_entry` was unconditionally `None` before
/// T6.0), governing_taxonomy is Obligation, and rendering with the entry
/// present does not panic. NOT a render-text-delta check: `LexiconEntry` is
/// only consulted for that debug assertion in `render_intent_event_to_sexpr`
/// — entry presence changes fold-precondition-eligibility, K-30 lint
/// coverage, and manifest hash (Q7) membership, never the rendered text
/// itself (confirmed by reading render.rs; the original T0.3 audit's
/// "rendering loses whatever the entry would have added" framing does not
/// hold against the real implementation, corrected here rather than
/// asserted-around).
#[test]
fn newly_covered_entries_are_fqn_correct_and_render_safe() {
    let lexicon = phase1_lexicon();
    for fqn in ["kyc.obligation.create", "kyc.person.approve"] {
        let entry = lexicon
            .get(fqn)
            .unwrap_or_else(|| panic!("{fqn} must be lexicon-covered after T6.0"));
        assert_eq!(entry.fqn.0, fqn, "entry fqn must match its manifest key");
        assert_eq!(
            entry.governing_taxonomy,
            ob_poc_kyc_substrate::Taxonomy::Obligation,
            "{fqn} must govern the Obligation taxonomy"
        );

        let subject = SubjectId(uuid::Uuid::new_v4());
        let event = make_event(
            subject,
            fqn,
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            lexicon.hash,
        );
        // Must not panic (exercises render's debug_assert_eq! for real).
        let _ = ob_poc_kyc_substrate::render_intent_event_to_sexpr(&event, Some(entry));
    }
}

// ── K-G7: fold-blind write (the audit's new finding) ────────────────────────

#[test]
fn fold_blind_verbs_are_exactly_known() {
    let control_arms = fold_match_arms(CONTROL_FOLD_SRC);
    let obligation_arms = fold_match_arms(OBLIGATION_FOLD_SRC);

    let fold_blind: BTreeSet<String> = declared_verb_universe()
        .into_iter()
        .filter(|fqn| !control_arms.contains(fqn) && !obligation_arms.contains(fqn))
        .collect();

    // kyc.role.assign/withdraw (the K-G7 finding) were retired 2026-08-12
    // rather than wired in — see dsl-kyc-obligation.yaml's retirement
    // comment. ubo.determination.compute-fold remains the sole allow-listed
    // fold-inert verb (effect_class: read_snapshot — it projects, it does
    // not mutate; fold-blind by design, not by omission). The open K-G7 gap
    // is now empty — a genuine closure, not a pinned-open gap like K-G6.
    let expected: BTreeSet<String> = ["ubo.determination.compute-fold"]
        .into_iter()
        .map(String::from)
        .collect();

    assert_eq!(
        fold_blind, expected,
        "K-G7 fold-blind verb set changed — this set is expected EMPTY of \
         open gaps post-retirement (only the by-design compute-fold entry \
         remains); any other member is a regression"
    );
}

// ── K-G5: precondition + strategy coverage (the seventh tooth) ─────────────

/// `"literal" => &Ident` arms inside freeze's `match strategy_name { ... }`
/// dispatch block — bounded to that one match (unlike `fold_match_arms`,
/// which is deliberately broad; this one must be exact since it pins a
/// literal count, not just membership).
fn freeze_strategy_arms() -> BTreeSet<String> {
    let marker = "let strategy: &dyn DeterminationStrategy = match strategy_name {";
    let start = KYC_STREAM_OPS_SRC
        .find(marker)
        .expect("freeze's strategy dispatch match must exist in kyc_stream_ops.rs");
    let mut arms = BTreeSet::new();
    for line in KYC_STREAM_OPS_SRC[start + marker.len()..].lines() {
        let trimmed = line.trim();
        if trimmed == "};" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix('"') {
            if let Some((literal, tail)) = rest.split_once('"') {
                if tail.trim_start().starts_with("=> &") {
                    arms.insert(literal.to_string());
                }
            }
        }
    }
    arms
}

/// `structure-class`'s `valid_values: [...]` bracketed list from dsl-kyc.yaml.
fn structure_class_valid_values() -> Vec<String> {
    let marker_line = DSL_KYC_YAML
        .lines()
        .find(|l| l.trim_start().starts_with("valid_values:") && l.contains("private_company"))
        .expect("structure-class valid_values line must exist in dsl-kyc.yaml");
    let inner = marker_line
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("valid_values must be a bracketed list");
    inner.split(',').map(|s| s.trim().to_string()).collect()
}

/// RED-honest pin (K-G5): passes because the gap is exactly as documented —
/// 3 of 20 lexicon-covered verbs carry a precondition (post-T6.0: all 20
/// dsl.kyc verbs are now lexicon-covered, K-G6 closed), and 1 of 11
/// structure classes (Nominee, TS.4) has no strategy behind it. Authoring a T6.1+
/// precondition, or a new `DeterminationStrategy`, is a CONSCIOUS edit here,
/// not a silent pass or a silent break.
#[test]
fn precondition_and_strategy_coverage_is_exactly_known() {
    let lexicon = phase1_lexicon();
    let actual: BTreeMap<String, Vec<Precondition>> = lexicon
        .entries
        .values()
        .map(|e| (e.fqn.0.clone(), e.preconditions.clone()))
        .collect();

    let mut expected: BTreeMap<String, Vec<Precondition>> = BTreeMap::new();
    expected.insert(
        "ubo.edge.verify".to_string(),
        vec![Precondition::EvidenceCited],
    );
    expected.insert(
        "ubo.determination.compute-fold".to_string(),
        vec![
            Precondition::ReconciledProjection,
            Precondition::StrategySelected,
        ],
    );
    // T6.1(c) (2026-08-12): the fail-closed strategy guard (matrix rows
    // 6a/8a) — `select-strategy` gets `StructureClassSupported`; `freeze`
    // gets it ADDED to its existing two (defense in depth — the guard holds
    // even if select-strategy is bypassed). The strategy-COUNT pin below (2)
    // is unchanged — this is a precondition-map change, not a new
    // `DeterminationStrategy`.
    // T6.3 row 6 (2026-08-12) widens select-strategy's sole 6a precondition
    // into the full documented order: subject registered -> structure
    // classified -> structure class supported.
    expected.insert(
        "ubo.determination.select-strategy".to_string(),
        vec![
            Precondition::SubjectRegistered,
            Precondition::StructureClassified,
            Precondition::StructureClassSupported,
        ],
    );
    expected.insert(
        "ubo.determination.freeze".to_string(),
        vec![
            Precondition::ReconciledProjection,
            Precondition::StrategySelected,
            Precondition::StructureClassSupported,
        ],
    );
    // T6.2 (2026-08-12, edge family — EOP-DD-KYCUBO-KIT-T6 matrix rows 1-5):
    // 5 more verbs go from geometry-free to studded. Rows 1/2 share the same
    // two studs (registration + no-duplicate-active-edge); rows 3/4 share
    // the same two (edge exists + active); row 5 is registration alone,
    // deliberately WITHOUT the optional "≥1 active economic edge" amendment
    // (kept callable early, per the ratified matrix note).
    expected.insert(
        "ubo.edge.assert-control".to_string(),
        vec![
            Precondition::SubjectRegistered,
            Precondition::NoDuplicateActiveEdge,
        ],
    );
    expected.insert(
        "ubo.edge.assert-economic-interest".to_string(),
        vec![
            Precondition::SubjectRegistered,
            Precondition::NoDuplicateActiveEdge,
        ],
    );
    expected.insert(
        "ubo.edge.attach-evidence".to_string(),
        vec![Precondition::EdgeExists, Precondition::EdgeActive],
    );
    expected.insert(
        "ubo.edge.supersede".to_string(),
        vec![Precondition::EdgeExists, Precondition::EdgeActive],
    );
    expected.insert(
        "ubo.edge.reconcile-conflict".to_string(),
        vec![Precondition::SubjectRegistered],
    );

    // T6.3 (2026-08-12, determination-family remainder — EOP-DD-KYCUBO-KIT-T6
    // matrix rows 7, 9, 10; row 6 is folded into select-strategy above; row 8
    // is the unchanged 8a freeze guard, asserted below).
    expected.insert(
        "ubo.determination.apply-smo-fallback".to_string(),
        vec![
            Precondition::ReconciledProjection,
            Precondition::StrategySelected,
        ],
    );
    // row 9 (`kyc.subject.register`) HALTED, not shipped (see the lexicon
    // entry's own comment): `NotAlreadyRegistered` is incompatible with
    // this verb's real multi-call-per-stream usage (one call per
    // natural-person candidate under one subject_root). Stays at its
    // pre-T6.3 empty precondition list — the ONE of the 20 dsl.kyc verbs
    // that remains geometry-free after T6.4.
    expected.insert("kyc.subject.register".to_string(), vec![]);
    expected.insert(
        "kyc.subject.classify-structure".to_string(),
        vec![Precondition::SubjectRegistered],
    );

    // T6.4 (2026-08-12, obligation/person family — EOP-DD-KYCUBO-KIT-T6
    // matrix rows 11-18, every row cross-fold via the T6.1 unified checker).
    expected.insert(
        "kyc.obligation.create".to_string(),
        vec![Precondition::SubjectRegistered],
    );
    for fqn in [
        "kyc.obligation.update-identity",
        "kyc.obligation.update-screening",
        "kyc.obligation.update-risk",
        "kyc.obligation.satisfy",
        "kyc.obligation.waive",
    ] {
        expected.insert(
            fqn.to_string(),
            vec![
                Precondition::ObligationExists,
                Precondition::SubjectNotDecided,
            ],
        );
    }
    expected.insert(
        "kyc.person.approve".to_string(),
        vec![
            Precondition::SubjectAllTerminal,
            Precondition::SubjectNotDecided,
        ],
    );
    expected.insert(
        "kyc.person.reject".to_string(),
        vec![Precondition::SubjectNotDecided],
    );

    assert_eq!(
        actual.len(),
        20,
        "phase1_lexicon() entry count drifted from the post-T6.0 20 \
         lexicon-covered verbs"
    );
    assert_eq!(
        actual, expected,
        "K-G5 precondition map changed — as of T6.4, every one of the 20 dsl.kyc verbs except \
         select-strategy/freeze's own 8a guard interaction now carries a stud (verify, \
         compute-fold, select-strategy, freeze, the 5 edge-family verbs, apply-smo-fallback, \
         register, classify-structure, and all 8 obligation/person verbs); any other change \
         is either T6.3/T6.4 progress (update the T0.3 audit) or a regression"
    );

    let classes = structure_class_valid_values();
    assert_eq!(
        classes.len(),
        11,
        "kyc.subject.classify-structure's structure-class valid_values count \
         drifted from the audited 11: {classes:#?}"
    );

    let strategies = freeze_strategy_arms();
    assert_eq!(
        strategies.len(),
        7,
        "freeze's implemented DeterminationStrategy count drifted from the \
         TS.3 7 (ownership_prong_strategy, control_prong_strategy, \
         trust_role_strategy, fund_control_strategy, \
         foundation_council_strategy, state_owned_strategy, \
         cooperative_member_strategy) — a new strategy is a CONSCIOUS pin \
         edit here AND in the TS.1 split pin below: {strategies:#?}"
    );
}

// ── TS.1 §1c — the two new closure-tooth pins (EOP-DD-KYCUBO-KIT-TS0) ──────

/// `kind`'s `valid_values: [...]` bracketed list from `assert-control` in
/// dsl-kyc.yaml — identified by membership ("voting_rights" appears only in
/// this list), same mechanism as `structure_class_valid_values()`.
fn assert_control_kind_valid_values() -> BTreeSet<String> {
    let marker_line = DSL_KYC_YAML
        .lines()
        .find(|l| l.trim_start().starts_with("valid_values:") && l.contains("voting_rights"))
        .expect("assert-control kind valid_values line must exist in dsl-kyc.yaml");
    let inner = marker_line
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("valid_values must be a bracketed list");
    inner.split(',').map(|s| s.trim().to_string()).collect()
}

/// TS.1 wire-value pin (closes R4 fact 5's "EdgeKind is unguarded"): the
/// YAML `valid_values`, the substrate's canonical `EDGE_KIND_WIRE_VALUES`
/// const, and the pinned 11-value set here must be EXACTLY the same set.
/// Any future `EdgeKind` wire addition is a conscious dual (YAML + const)
/// edit plus this pin — never a silent widening (and never a fold-arm that
/// exists without a wire value, or vice versa: the const's own doc binds it
/// to `edge_kind_from_payload`'s arms).
#[test]
fn edge_kind_wire_values_are_exactly_known() {
    let expected: BTreeSet<String> = [
        "economic_interest",
        "voting_rights",
        "board_appointment",
        "gp_statutory",
        "designated_member",
        "trust_settlor",
        "trust_trustee",
        "trust_protector",
        "trust_beneficiary",
        "nominee",
        "dominant_influence",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let from_const: BTreeSet<String> = ob_poc_kyc_substrate::EDGE_KIND_WIRE_VALUES
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        from_const, expected,
        "EDGE_KIND_WIRE_VALUES drifted from the pinned TS.1 11-value set — \
         adding/removing a wire value must consciously touch the const, the \
         fold's edge_kind_from_payload arms, the YAML valid_values, AND this pin"
    );

    let from_yaml = assert_control_kind_valid_values();
    assert_eq!(
        from_yaml, expected,
        "assert-control's kind valid_values (dsl-kyc.yaml) drifted from the \
         pinned TS.1 11-value set — the YAML and EDGE_KIND_WIRE_VALUES must \
         stay in lockstep (one source of truth, two declared surfaces)"
    );
}

/// TS.1 split pin (closes R4 fact 5's second half): the freeze dispatch's
/// strategy arms and `IMPLEMENTED_STRATEGY_CLASSES` must stay in lockstep
/// via an explicit arm → classes-served mapping. Widening
/// `IMPLEMENTED_STRATEGY_CLASSES` without a real strategy arm landing (or
/// landing an arm that serves no class) fails here: every implemented class
/// must be served by exactly the strategy this map says, and every arm must
/// serve at least one class.
#[test]
fn implemented_class_split_matches_strategy_arms() {
    use ob_poc_kyc_substrate::IMPLEMENTED_STRATEGY_CLASSES;

    // The ruled mapping (EOP-DD-KYCUBO-KIT-TS0 §2 + kyc_stream_ops dispatch):
    // which structure classes each freeze-dispatch arm serves.
    let classes_served: BTreeMap<&str, Vec<StructureClass>> = BTreeMap::from([
        (
            "ownership_prong_strategy",
            vec![
                StructureClass::PrivateCompany,
                StructureClass::MultiTierHoldingGroup,
                StructureClass::ListedEntity,
            ],
        ),
        (
            "control_prong_strategy",
            vec![
                StructureClass::LimitedPartnershipFund,
                StructureClass::Llp,
            ],
        ),
        ("trust_role_strategy", vec![StructureClass::Trust]),
        // TS.2 (EOP-DD-KYCUBO-KIT-TS0 §2.2/§2.3): fund control sits with the
        // manager; foundation control sits with the council.
        ("fund_control_strategy", vec![StructureClass::InvestmentFund]),
        (
            "foundation_council_strategy",
            vec![StructureClass::Foundation],
        ),
        // TS.3 (EOP-DD-KYCUBO-KIT-TS0 §2.4/§2.5): state-owned control is
        // usually SMO (the strategy legitimizes the fallback route);
        // cooperative control arises from office, never membership.
        ("state_owned_strategy", vec![StructureClass::StateOwned]),
        (
            "cooperative_member_strategy",
            vec![StructureClass::Cooperative],
        ),
    ]);

    // Every dispatch arm appears in the mapping and vice versa.
    let arms = freeze_strategy_arms();
    let mapped: BTreeSet<String> = classes_served.keys().map(|s| s.to_string()).collect();
    assert_eq!(
        arms, mapped,
        "freeze's dispatch arms and the arm→classes-served mapping diverged — \
         a new DeterminationStrategy arm must land WITH the classes it serves \
         (and a mapping entry must never exist without a real arm): lockstep \
         rule, EOP-DD-KYCUBO-KIT-TS0 §1c"
    );

    // Every arm serves at least one class; the union of served classes is
    // EXACTLY IMPLEMENTED_STRATEGY_CLASSES (no class implemented without a
    // strategy, no strategy arm serving a class outside the guard set).
    let mut served: BTreeSet<String> = BTreeSet::new();
    for (arm, classes) in &classes_served {
        assert!(
            !classes.is_empty(),
            "strategy arm {arm} serves no structure class — an arm with no \
             class behind it is dead dispatch (lockstep rule)"
        );
        served.extend(classes.iter().map(|c| format!("{c:?}")));
    }
    let implemented: BTreeSet<String> = IMPLEMENTED_STRATEGY_CLASSES
        .iter()
        .map(|c| format!("{c:?}"))
        .collect();
    assert_eq!(
        served, implemented,
        "IMPLEMENTED_STRATEGY_CLASSES and the arm→classes-served mapping \
         diverged — widening the guard set without a strategy arm (or vice \
         versa) is exactly what this pin fail-closes; after TS.3 the split \
         is 7 arms serving 10 classes (lockstep rule, EOP-DD-KYCUBO-KIT-TS0 §1c)"
    );
}

// ── K-G1/K-G2: EdgeStatus reachability (real fold-chain behavior) ─────────

fn registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

#[allow(clippy::too_many_arguments)]
fn make_event(
    subject: SubjectId,
    verb_fqn: &str,
    target: TargetBinding,
    payload: serde_json::Value,
    as_of: chrono::DateTime<Utc>,
    lexicon_hash: Hash,
) -> IntentEvent {
    IntentEvent::new(
        subject,
        verb_fqn,
        Principal::new(uuid::Uuid::nil(), "analyst"),
        AuthorityRef("test.fixture".into()),
        target,
        payload,
        as_of,
    )
    .with_lexicon_hash(lexicon_hash)
}

#[test]
fn edge_status_lifecycle_is_fully_reachable() {
    let subject = SubjectId(uuid::Uuid::new_v4());
    let from = EntityId(uuid::Uuid::new_v4());
    let to = EntityId(uuid::Uuid::new_v4());
    let edge = EdgeId(uuid::Uuid::new_v4());
    let as_of = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let lexicon_hash = phase1_lexicon().hash;
    let reg = registry();

    let assert_event = make_event(
        subject,
        "ubo.edge.assert-control",
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({
            "from_entity_id": from.0,
            "to_entity_id": to.0,
            "kind": "voting_rights",
            "edge_id": edge.0,
        }),
        as_of,
        lexicon_hash,
    );
    let state = fold_control_versioned(&[&assert_event], &reg).expect("fold ok");
    assert_eq!(state.edges.get(&edge).unwrap().status, EdgeStatus::Asserted);

    let evidence_event = make_event(
        subject,
        "ubo.edge.attach-evidence",
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({}),
        as_of,
        lexicon_hash,
    );
    let state =
        fold_control_versioned(&[&assert_event, &evidence_event], &reg).expect("fold ok");
    assert_eq!(state.edges.get(&edge).unwrap().status, EdgeStatus::Evidenced);

    let verify_event = make_event(
        subject,
        "ubo.edge.verify",
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({}),
        as_of,
        lexicon_hash,
    );
    let state = fold_control_versioned(&[&assert_event, &evidence_event, &verify_event], &reg)
        .expect("fold ok");
    assert_eq!(state.edges.get(&edge).unwrap().status, EdgeStatus::Verified);

    // Supersede-from-Verified — proves Superseded is reachable from every
    // prior status, not just Asserted (K-G1/K-G2: no dead end mid-lifecycle).
    let supersede_event = make_event(
        subject,
        "ubo.edge.supersede",
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({}),
        as_of,
        lexicon_hash,
    );
    let state = fold_control_versioned(
        &[&assert_event, &evidence_event, &verify_event, &supersede_event],
        &reg,
    )
    .expect("fold ok");
    assert_eq!(state.edges.get(&edge).unwrap().status, EdgeStatus::Superseded);
}

// ── Part A (EOP-PLAN-KYCUBO-KIT-001) — the checker-reach tooth ─────────────

/// Extract `stream_append(fqn, subject, target, payload, authority,
/// validate_entry_fqn, ctx, scope)` call sites from `kyc_stream_ops.rs`.
/// Every call site in the file (verified 2026-08-12) lays out its 8
/// arguments one per line in this fixed order, so a 6th-line read is exact
/// rather than a heuristic guess — brittle to reformatting, but this file's
/// whole M.O. (`fold_match_arms`, `registered_op_fqns`, `extract_verb_fqns`,
/// above) is pinned source-shape scanning, not a real parser.
///
/// Returns `fqn -> validate_entry_fqn` where the value is `None` when the
/// call site passes literal `None`, or `Some(x)` when it passes
/// `Some("x")`.
fn stream_append_wiring(src: &str) -> BTreeMap<String, Option<String>> {
    // `= stream_append(` (a call site) rather than bare `stream_append(`,
    // which would also match the `async fn stream_append(` definition
    // itself.
    let marker = "= stream_append(";
    let mut out = BTreeMap::new();
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(marker) {
        let start = search_from + rel + marker.len();
        let mut lines = src[start..].lines().filter(|l| !l.trim().is_empty());
        let arg = |l: &str| l.trim().trim_end_matches(',').to_string();
        let Some(fqn_line) = lines.next() else { break };
        let fqn = arg(fqn_line).trim_matches('"').to_string();
        // Skip subject, target, payload, authority (lines 2-5); line 6 is
        // validate_entry_fqn.
        for _ in 0..4 {
            if lines.next().is_none() {
                break;
            }
        }
        let validate = lines.next().map(arg).unwrap_or_default();
        let value = if validate == "None" {
            None
        } else if let Some(inner) = validate
            .strip_prefix("Some(\"")
            .and_then(|s| s.strip_suffix("\")"))
        {
            Some(inner.to_string())
        } else {
            // Unrecognised shape (e.g. `Some(some_variable)`) — record it
            // verbatim so a drifted call site fails loudly (as "wired to
            // <garbage>", not silently treated as unwired) rather than
            // being missed by this scanner.
            Some(format!("UNRECOGNISED[{validate}]"))
        };
        out.insert(fqn, value);
        search_from = start;
    }
    out
}

/// Extract fqns wired via the inline `let entry = lexicon.get("<fqn>")` →
/// `check_preconditions(entry, ...)` / `check_control_preconditions(entry,
/// ...)` shape (`ubo.edge.assert-control`'s hand-rolled `append_in_scope`
/// closure, and `ubo.determination.compute-fold`'s read-path precondition
/// check — the Part A A3 fix). Looks ahead a bounded window (25 lines, well
/// past both real call sites' distance) for the checker call, stopping
/// early at the next `lexicon.get(` so a miss can never be misattributed to
/// the wrong fqn.
fn inline_lexicon_get_wiring(src: &str) -> BTreeSet<String> {
    let marker = ".get(\"";
    let mut out = BTreeSet::new();
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(marker) {
        let start = search_from + rel + marker.len();
        let Some(end) = src[start..].find('"') else {
            break;
        };
        let fqn = src[start..start + end].to_string();
        let window_end = (start + end + 4000).min(src.len());
        let window = &src[start + end..window_end];
        let next_get = window[1..].find(".get(\"").unwrap_or(usize::MAX);
        let checker_hit = window
            .find("check_preconditions(entry")
            .or_else(|| window.find("check_control_preconditions(entry"));
        if let Some(hit) = checker_hit {
            if hit < next_get {
                out.insert(fqn);
            }
        }
        search_from = start + end;
    }
    out
}

/// The wiring half of the tooth: every lexicon entry with a non-empty
/// `preconditions` list must have a genuine call-site in `kyc_stream_ops.rs`
/// that reaches `check_preconditions`/`check_control_preconditions` for that
/// exact fqn when the verb's real op runs — not just a correct checker
/// function in the abstract (see module doc for why this can't be a live-DB
/// drive-through in this pure file).
#[test]
fn every_precondition_carrying_verb_is_reached_by_the_checker() {
    let lexicon = phase1_lexicon();
    let precondition_carrying: BTreeSet<String> = lexicon
        .entries
        .values()
        .filter(|e| !e.preconditions.is_empty())
        .map(|e| e.fqn.0.clone())
        .collect();

    let stream_wired = stream_append_wiring(KYC_STREAM_OPS_SRC);
    let inline_wired = inline_lexicon_get_wiring(KYC_STREAM_OPS_SRC);

    let mut unreached: Vec<String> = Vec::new();
    for fqn in &precondition_carrying {
        let via_stream_append = stream_wired.get(fqn).map(|v| v.as_deref()) == Some(Some(fqn));
        let via_inline = inline_wired.contains(fqn);
        if !via_stream_append && !via_inline {
            unreached.push(fqn.clone());
        }
    }

    assert!(
        unreached.is_empty(),
        "declared precondition(s) on {unreached:#?} are never reached by the checker at the \
         real op call site (stream_append wiring: {stream_wired:#?}; inline wiring: \
         {inline_wired:#?}) — this is the exact defect class of freeze's pre-DD-003 dead \
         precondition and the pre-T6.1(c) select-strategy gap"
    );

    // Sanity: the scanner itself must not be vacuously trivial — it must
    // have found at least the 4 pre-T6.2 wired verbs (verify, select-strategy,
    // freeze, and compute-fold post the A3 fix) as a floor, or a scanner bug
    // (not a real gap) could be silently passing this test by finding nothing
    // to check in the first place.
    for known_wired in [
        "ubo.edge.verify",
        "ubo.determination.select-strategy",
        "ubo.determination.freeze",
        "ubo.determination.compute-fold",
    ] {
        assert!(
            precondition_carrying.contains(known_wired),
            "scanner sanity: {known_wired} must still carry a precondition in the pinned \
             lexicon — if not, this floor check needs a conscious update"
        );
    }
}

/// The semantic half: for each of today's precondition-carrying verbs, a
/// state that violates its stud must produce the real `KycError`, and a
/// state that satisfies it must succeed — driven through the same
/// `check_preconditions`/`check_control_preconditions` composition the real
/// op calls (proven wired by the sibling test above), not a bespoke
/// re-implementation.
#[test]
fn precondition_carrying_verbs_actually_enforce_their_stud() {
    let lexicon = phase1_lexicon();
    let subject = SubjectId(uuid::Uuid::new_v4());
    let as_of = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let empty_obligation = ObligationState::default();

    let probe = |verb_fqn: &str, target: TargetBinding| {
        IntentEvent::new(
            subject,
            verb_fqn,
            Principal::test_analyst(),
            AuthorityRef("closure-tooth-probe".into()),
            target,
            serde_json::Value::Null,
            as_of,
        )
    };

    // ubo.edge.verify — EvidenceCited: an Asserted (not-yet-evidenced) edge
    // must block; an Evidenced edge must admit.
    let edge = EdgeId(uuid::Uuid::new_v4());
    let verify_entry = lexicon.get("ubo.edge.verify").unwrap();
    let mut asserted_only = ControlState::default();
    asserted_only.edges.insert(
        edge,
        ob_poc_kyc_substrate::EdgeState {
            id: edge,
            kind: ob_poc_kyc_substrate::EdgeKind::VotingRights,
            from: EntityId(uuid::Uuid::new_v4()),
            to: EntityId(uuid::Uuid::new_v4()),
            percentage: None,
            status: EdgeStatus::Asserted,
            evidence_event_id: None,
            originating_event_id: ob_poc_kyc_substrate::EventId::new(),
            trust_revocable: None,
        },
    );
    assert!(
        check_preconditions(
            verify_entry,
            &asserted_only,
            &empty_obligation,
            &probe("ubo.edge.verify", TargetBinding::for_edge(subject, edge)),
        )
        .is_err(),
        "verify must block an edge with no evidence attached"
    );
    let mut evidenced = asserted_only.clone();
    evidenced.edges.get_mut(&edge).unwrap().status = EdgeStatus::Evidenced;
    assert!(
        check_preconditions(
            verify_entry,
            &evidenced,
            &empty_obligation,
            &probe("ubo.edge.verify", TargetBinding::for_edge(subject, edge)),
        )
        .is_ok(),
        "verify must admit an edge with evidence attached"
    );

    // ubo.determination.select-strategy / freeze — StructureClassSupported:
    // an unsupported class must block; a supported one must admit.
    // T6.3 row 6: select-strategy also carries SubjectRegistered +
    // StructureClassified now, so both fixtures below must set
    // `registered: true` too — the Trust case still blocks (on
    // StructureClassSupported, checked last), the PrivateCompany case would
    // otherwise incorrectly block on SubjectRegistered instead of proving
    // the guard this sub-test targets.
    // TS.3 fixture fix: StateOwned + Cooperative joined the implemented set
    // (StateOwnedStrategy/CooperativeMemberStrategy), so the fail-closed
    // exemplar class becomes Nominee — the LAST strategy-less class (moves
    // again at TS.4, to a taxonomy widening or the pin's retirement).
    let select_entry = lexicon.get("ubo.determination.select-strategy").unwrap();
    let unsupported = ControlState {
        registered: true,
        structure_class: Some(StructureClass::Nominee),
        ..Default::default()
    };
    assert!(
        check_preconditions(
            select_entry,
            &unsupported,
            &empty_obligation,
            &probe("ubo.determination.select-strategy", TargetBinding::for_subject(subject)),
        )
        .is_err(),
        "select-strategy must block an unsupported structure class (Nominee)"
    );
    let supported = ControlState {
        registered: true,
        structure_class: Some(StructureClass::PrivateCompany),
        ..Default::default()
    };
    assert!(
        check_preconditions(
            select_entry,
            &supported,
            &empty_obligation,
            &probe("ubo.determination.select-strategy", TargetBinding::for_subject(subject)),
        )
        .is_ok(),
        "select-strategy must admit a supported structure class (PrivateCompany)"
    );

    // ubo.determination.compute-fold — ReconciledProjection + StrategySelected
    // (the A3 fix): an unreconciled/unstrategized state must block; a
    // reconciled+strategized one must admit.
    let compute_fold_entry = lexicon.get("ubo.determination.compute-fold").unwrap();
    let not_ready = ControlState::default();
    assert!(
        check_control_preconditions(
            compute_fold_entry,
            &not_ready,
            &probe("ubo.determination.compute-fold", TargetBinding::for_subject(subject)),
        )
        .is_err(),
        "compute-fold must block before reconcile-conflict + select-strategy have fired (K-14)"
    );
    let ready = ControlState {
        reconciliation_event_id: Some(ob_poc_kyc_substrate::EventId::new()),
        selected_strategy: Some("ownership_prong_strategy".to_string()),
        ..Default::default()
    };
    assert!(
        check_control_preconditions(
            compute_fold_entry,
            &ready,
            &probe("ubo.determination.compute-fold", TargetBinding::for_subject(subject)),
        )
        .is_ok(),
        "compute-fold must admit once reconcile-conflict + select-strategy have fired"
    );
}

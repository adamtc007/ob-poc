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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::{TimeZone, Utc};

use ob_poc_kyc_substrate::{
    fold_control_versioned, phase1_lexicon, AuthorityRef, EdgeId, EdgeStatus, EntityId,
    FoldRegistry, Hash, IntentEvent, Precondition, Principal, SubjectId, TargetBinding,
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
/// dsl.kyc verbs are now lexicon-covered, K-G6 closed), and 6 of 11
/// structure classes have no strategy behind them. Authoring a T6.1+
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

    let mut expected: BTreeMap<String, Vec<Precondition>> = [
        "kyc.subject.register",
        "kyc.subject.classify-structure",
        "ubo.edge.assert-control",
        "ubo.edge.assert-economic-interest",
        "ubo.edge.attach-evidence",
        "ubo.edge.supersede",
        "ubo.edge.reconcile-conflict",
        "ubo.determination.select-strategy",
        "ubo.determination.apply-smo-fallback",
        // T6.0 (2026-08-12): entries only, no preconditions authored here —
        // that stays T6.1+.
        "kyc.obligation.create",
        "kyc.obligation.update-identity",
        "kyc.obligation.update-screening",
        "kyc.obligation.update-risk",
        "kyc.obligation.satisfy",
        "kyc.obligation.waive",
        "kyc.person.approve",
        "kyc.person.reject",
    ]
    .into_iter()
    .map(|fqn| (fqn.to_string(), Vec::new()))
    .collect();
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
    expected.insert(
        "ubo.determination.freeze".to_string(),
        vec![
            Precondition::ReconciledProjection,
            Precondition::StrategySelected,
        ],
    );

    assert_eq!(
        actual.len(),
        20,
        "phase1_lexicon() entry count drifted from the post-T6.0 20 \
         lexicon-covered verbs"
    );
    assert_eq!(
        actual, expected,
        "K-G5 precondition map changed — today only verify/compute-fold/freeze \
         carry a precondition; any other change is either T6 progress (update \
         the T0.3 audit) or a regression"
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
        2,
        "freeze's implemented DeterminationStrategy count drifted from the \
         audited 2 (ownership_prong_strategy, control_prong_strategy): {strategies:#?}"
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

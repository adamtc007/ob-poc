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
    check_control_preconditions, check_preconditions, fold_control_versioned, assembly_lexicon,
    evaluation_lexicon,
    AuthorityRef, ControlProngStrategy, ControlState, CooperativeMemberStrategy,
    DeterminationStrategy, EdgeId, EdgeKind, EdgeStatus, EntityId, EventId, FoldRegistry,
    FoundationCouncilStrategy, FundControlStrategy, Hash, IntentEvent, NomineePierceStrategy,
    ObligationState, OwnershipProngStrategy, PersonId, Precondition, Principal, Prong,
    StateOwnedStrategy, StructureClass, SubjectId, TargetBinding, TrustRoleKind, TrustRoleStrategy,
    TypeRegistryState, V1FoldImpl,
};

const DSL_KYC_YAML: &str = include_str!("../config/verbs/kyc/dsl-kyc.yaml");
const DSL_KYC_OBLIGATION_YAML: &str = include_str!("../config/verbs/kyc/dsl-kyc-obligation.yaml");
const SCREENING_YAML: &str = include_str!("../config/verbs/screening.yaml");
const KYC_DAG_YAML: &str = include_str!("../config/sem_os_seeds/dag_taxonomies/kyc_dag.yaml");
const KYC_STREAM_OPS_SRC: &str = include_str!("../src/domain_ops/kyc_stream_ops.rs");
// TS.6 P1/P2: decide.approve/decide.reject moved to ob-poc-kyc-decide — a
// second source file this test's `registered_op_fqns()` must scan, same
// widening principle as `kyc_stream_ops_declared_universe()`'s screening.yaml
// intersection below (a declared verb registered somewhere this test didn't
// used to look must not manufacture a false K-G4 "missing_ops" red).
const KYC_DECIDE_OPS_SRC: &str = include_str!("../crates/ob-poc-kyc-decide/src/lib.rs");
const KYC_STORE_SRC: &str = include_str!("../crates/ob-poc-kyc-store/src/store.rs");
const KYC_PROJECTION_SRC: &str = include_str!("../crates/ob-poc-kyc-store/src/projection.rs");
const KYC_STORE_LIB_SRC: &str = include_str!("../crates/ob-poc-kyc-store/src/lib.rs");
const LEXICON_SRC: &str = include_str!("../crates/ob-poc-kyc-substrate/src/lexicon.rs");
const CONTROL_FOLD_SRC: &str = include_str!("../crates/ob-poc-kyc-substrate/src/fold/control.rs");
const OBLIGATION_FOLD_SRC: &str =
    include_str!("../crates/ob-poc-kyc-substrate/src/fold/obligation.rs");
const TYPE_REGISTRY_FOLD_SRC: &str =
    include_str!("../crates/ob-poc-kyc-substrate/src/fold/type_registry.rs");

// ── Declaration extraction ──────────────────────────────────────────────────

/// Parse `domains: { <domain>: { verbs: { <verb>: ... } } }` from a dsl.kyc
/// verb YAML file — a plain line-scan (2-space domain keys, 6-space verb
/// keys) rather than a full YAML parse, matching the fixed hand-authored
/// indentation of these two files (verified: exactly 2 domain lines + 16 verb
/// lines in dsl-kyc.yaml, 1 domain line + 8 verb lines in
/// dsl-kyc-obligation.yaml — 24 total, post K-G7 retirement of
/// kyc.role.assign/withdraw (2026-08-12), the TS.4 `ubo.edge.pierce-nominee`
/// addition (K-8, full-kit-citizenship reintroduction discipline), the D1
/// `kyc.subject.{assert-type,correct-type,withdraw-member,record-enquiry}`
/// addition (EOP-DD-KYCUBO-TS.1 §3 moves 2/6/7/8), and the TS.6 P2 retirement
/// of `ubo.determination.select-strategy`).
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

/// The dsl.kyc verb family (`declared_verb_universe()`) widened by whichever
/// `screening.yaml` verbs are ALSO registered as `SemOsVerbOp`s inside
/// `kyc_stream_ops.rs` (today: `screening.complete`, `screening.review-hit`
/// — the W5 screening hook, which fans out `assert.screening`
/// events from inside the same file). Deliberately NOT a blind union of all
/// of `screening.yaml`: most of its verbs (`pep`, `sanctions`,
/// `adverse-media`, `bulk-refresh`, `await-aggregate-outcome`) are also
/// `behavior: plugin`, but registered elsewhere, outside this file's
/// visibility — unioning them in would manufacture false K-G4 "missing_ops"
/// reds for ops this test never claims to see. The intersection with
/// `registered_op_fqns()` is not circular: it only restricts WHICH
/// screening.yaml declarations are eligible to satisfy K-G4, it does not
/// assume they are correct — a rogue registered fqn with no matching
/// declaration anywhere still surfaces as `phantom_ops` below, and a
/// declaration whose op is deleted from `kyc_stream_ops.rs` still surfaces
/// too (the entry drops out of the intersection, `declared_verb_universe()`'s
/// base 22 is untouched, so it becomes a `phantom_ops` orphan). K-G3
/// (`stream_governed_family_covers_every_declared_verb`) and
/// `verb_universe_is_exactly_22` intentionally keep using the narrower
/// `declared_verb_universe()` — they measure the dsl.kyc stream-governed
/// family specifically, not "every op this file happens to host".
fn kyc_stream_ops_declared_universe() -> BTreeSet<String> {
    let mut fqns = declared_verb_universe();
    let screening_declared = extract_verb_fqns(SCREENING_YAML);
    let registered = registered_op_fqns();
    fqns.extend(screening_declared.intersection(&registered).cloned());
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

/// `fn fqn(&self) -> &str { "..." }` string literals from `src` (a source
/// file's text, scanned line-by-line).
fn fqns_from_op_src(src: &str) -> BTreeSet<String> {
    let marker = "fn fqn(&self) -> &str {";
    let mut fqns = BTreeSet::new();
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        if line.trim() == marker {
            if let Some(next) = lines.next() {
                let trimmed = next.trim();
                if let Some(literal) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                    fqns.insert(literal.to_string());
                }
            }
        }
    }
    fqns
}

/// Registered `SemOsVerbOp` FQNs across BOTH kyc_stream_ops.rs and
/// ob-poc-kyc-decide (TS.6 P1/P2 — decide.approve/decide.reject moved out of
/// the former into the latter).
fn registered_op_fqns() -> BTreeSet<String> {
    let mut fqns = fqns_from_op_src(KYC_STREAM_OPS_SRC);
    fqns.extend(fqns_from_op_src(KYC_DECIDE_OPS_SRC));
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
fn verb_universe_is_exactly_21() {
    // TS.6 P1/P2: kyc.person.approve/.reject renamed decide.approve/.reject
    // and moved to a new `decide:` domain block in the SAME YAML file
    // (dsl-kyc-obligation.yaml) — a rename+relocation, not a retirement, so
    // this count is unchanged at 22 (declared_verb_universe() scans both
    // files' domains, not just `kyc:`).
    let fqns = declared_verb_universe();
    assert_eq!(
        fqns.len(),
        21,
        "dsl.kyc verb count drifted from the post-TS.6-§5 21 (25 post-D1, \
         minus the retired select-strategy, compute-fold, and pierce-nominee \
         verb declarations) — update the T0.3 audit and every other pinned \
         test in this file, not just this assertion: {fqns:#?}"
    );
}

// ── TS.6 §8: pack-membership, boundary, and retirement gates ───────────────

#[test]
fn assembly_pack_is_exactly_known() {
    // TS.6 §3/§4: the 20 `assembly_lexicon()` entries — 22 declared dsl.kyc
    // verbs minus the 2 evaluation-pack verdicts. Includes `kyc.obligation.
    // waive`, deferred here pending D2.0 (TS.6 §3's amended row 13) rather
    // than renamed to `decide.waive`.
    let expected: BTreeSet<String> = [
        "kyc.subject.register",
        "kyc.subject.classify-structure",
        "kyc.subject.assert-type",
        "kyc.subject.correct-type",
        "kyc.subject.withdraw-member",
        "kyc.subject.record-enquiry",
        "ubo.edge.assert-control",
        "ubo.edge.assert-economic-interest",
        "ubo.edge.attach-evidence",
        "ubo.edge.verify",
        "ubo.edge.supersede",
        "ubo.edge.reconcile-conflict",
        "ubo.determination.freeze",
        "kyc.obligation.create",
        "assert.identity",
        "assert.screening",
        "assert.risk",
        "kyc.obligation.satisfy",
        "kyc.obligation.waive",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let actual: BTreeSet<String> = assembly_lexicon().entries.keys().cloned().collect();
    assert_eq!(
        actual, expected,
        "assembly_lexicon() membership drifted (TS.6 §8 assembly_pack_is_exactly_known)"
    );
}

#[test]
fn evaluation_pack_is_exactly_known() {
    // TS.6 §4: only the 2 landed verdicts. `decide.waive` is NOT here —
    // `kyc.obligation.waive` stays in assembly_lexicon() pending D2.0.
    let expected: BTreeSet<String> = ["decide.approve", "decide.reject"]
        .into_iter()
        .map(String::from)
        .collect();
    let actual: BTreeSet<String> = evaluation_lexicon().entries.keys().cloned().collect();
    assert_eq!(
        actual, expected,
        "evaluation_lexicon() membership drifted (TS.6 §8 evaluation_pack_is_exactly_known)"
    );
}

#[test]
fn relocated_facts_are_in_assembly() {
    // TS.6 §3: three obligation-namespace verbs are observations about the
    // world, not work-item updates — the quiet win of the pack split. Must
    // live in Assembly and never appear in Evaluation.
    let assembly = assembly_lexicon();
    let evaluation = evaluation_lexicon();
    for fqn in [
        "assert.screening",
        "assert.identity",
        "assert.risk",
    ] {
        assert!(
            assembly.get(fqn).is_some(),
            "{fqn} must be a member of assembly_lexicon() (TS.6 §3 relocation)"
        );
        assert!(
            evaluation.get(fqn).is_none(),
            "{fqn} must not be a member of evaluation_lexicon()"
        );
    }
}

#[test]
fn evaluation_pack_cannot_write_facts() {
    // TS.6 §8's structural gate, made a discoverable `cargo test` rather than
    // only a hand-run shell script (`scripts/check_kyc_decide_deps.sh`,
    // which this duplicates in Rust so CI's normal test run catches drift):
    // `ob-poc-kyc-decide` must have zero dependency, direct or transitive, on
    // ANY crate exposing a dsl.kyc fact-stream append.
    //
    // WIDENED 2026-08-22. The original list named only `ob-poc-kyc-seam` —
    // the sole GOVERNED chokepoint, but NOT the only crate-level write path.
    // `ob-poc-kyc-store::PgKycEventStore::append` is `pub` and is the real
    // append (stream lock, seq allocation, INSERT into kyc_intent_events).
    // The reconciliation proved the gap by execution: a probe inside the
    // evaluation pack appended a real event with `ob-poc-kyc-seam` nowhere in
    // its tree, while this test still passed. The gate forbade the wrong edge.
    //
    // The evaluation pack now depends on `ob-poc-kyc-read`, which contains no
    // append at all — so the property holds by construction, and this test is
    // FALSIFIABLE: re-add `ob-poc-kyc-store` to
    // `crates/ob-poc-kyc-decide/Cargo.toml` and it goes red (RED-proven
    // 2026-08-22).
    let output = std::process::Command::new("cargo")
        .args(["tree", "-p", "ob-poc-kyc-decide"])
        .output()
        .expect("run cargo tree");
    assert!(
        output.status.success(),
        "cargo tree -p ob-poc-kyc-decide failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    for forbidden in [
        "ob-poc-kyc-seam",
        "ob_poc_kyc_seam",
        "ob-poc-kyc-store",
        "ob_poc_kyc_store",
    ] {
        assert!(
            !tree.contains(forbidden),
            "ob-poc-kyc-decide must never depend on {forbidden} — it exposes a \
             fact-stream append (TS.6 §8 evaluation_pack_cannot_write_facts). \
             Depend on ob-poc-kyc-read instead. Dep tree:\n{tree}"
        );
    }
}

#[test]
fn retired_verbs_are_gone() {
    // K-G7 grep-proof pattern: no live YAML declaration, op registration, or
    // LexiconEntry remains for any TS.6-retired verb.
    //
    // YAML verb keys are written relative to their domain (e.g. `ubo:` +
    // `determination.select-strategy:`, `kyc:` + `person.approve:`), so a
    // bare-key substring check is correct here and doesn't need the domain
    // prefix reconstructed.
    for needle in [
        "person.approve:",
        "person.reject:",
        "determination.select-strategy:",
        "determination.compute-fold:",
        "edge.pierce-nominee:",
    ] {
        assert!(
            !DSL_KYC_YAML.contains(needle),
            "retired verb declaration {needle} still present in dsl-kyc.yaml"
        );
        assert!(
            !DSL_KYC_OBLIGATION_YAML.contains(needle),
            "retired verb declaration {needle} still present in dsl-kyc-obligation.yaml"
        );
    }

    // Struct-definition form, not a bare name substring — `kyc_stream_ops.rs`
    // deliberately keeps a retirement comment mentioning `KycPersonApprove`/
    // `KycPersonReject` by name (mirroring `lexicon.rs`'s pierce-nominee
    // commentary), which a bare substring check would false-fail against.
    for needle in [
        "struct KycPersonApprove",
        "struct KycPersonReject",
        "struct UboDeterminationSelectStrategy",
        "struct UboDeterminationComputeFold",
        "struct UboEdgePierceNominee",
    ] {
        assert!(
            !KYC_STREAM_OPS_SRC.contains(needle),
            "retired op struct {needle} still present in kyc_stream_ops.rs"
        );
    }

    // Quoted-literal check (not bare substring) so this doesn't false-fail
    // against `pierce-nominee`'s own retirement commentary, which
    // deliberately mentions its retired FQN in prose (lexicon.rs ~line 415).
    // select-strategy/compute-fold have no such retained prose mention as a
    // double-quoted Rust string literal, so checking for the exact
    // `LexiconEntry::build(...)` first-argument form is safe.
    for needle in [
        "\"ubo.determination.select-strategy\"",
        "\"ubo.determination.compute-fold\"",
    ] {
        assert!(
            !LEXICON_SRC.contains(needle),
            "retired LexiconEntry {needle} still present in lexicon.rs"
        );
    }
}

// ── K-G3: unmapped move ─────────────────────────────────────────────────────

#[test]
fn stream_governed_family_covers_every_declared_verb() {
    let globs = stream_governed_globs();
    // 6 → 7 (TS.6 §3 relocation, 2026-08-22): `assert.*` added for
    // assert.screening/.identity/.risk, renamed out of `kyc.obligation.*`.
    // They remain stream-governed — only the namespace moved, not the
    // behaviour — so the family list must cover the new prefix or they would
    // read as ungoverned (K-G3).
    assert_eq!(
        globs.len(),
        7,
        "stream_governed.verb_families count drifted from the audited 7: {globs:#?}"
    );
    // The kyc.role.* glob is vacuous after the K-G7 retirement (no live verb
    // matches it) — left in kyc_dag.yaml deliberately: governance-block
    // edits are a separate conscious change from verb retirement, and the
    // glob costs nothing to leave (it covers zero verbs, not a wrong verb).
    // kyc.person.* is likewise now vacuous (TS.6 P1/P2): decide.approve/
    // decide.reject don't match it and are deliberately excluded from this
    // loop below — they are NOT stream-governed any more (ob-poc-kyc-decide
    // never writes to the fact stream at all), so "must be covered by a
    // stream_governed family glob" does not apply to them by design.

    for fqn in declared_verb_universe() {
        if fqn == "decide.approve" || fqn == "decide.reject" {
            continue;
        }
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
    // Tree-cleanup tranche (2026-08-21) traced the prior "27 vs 25" red to an
    // under-scoped tooth, not a real defect: `screening.complete` and
    // `screening.review-hit` are correctly declared (`screening.yaml`) and
    // correctly registered (`kyc_stream_ops.rs`), but this test's declared
    // side never read `screening.yaml`. See `kyc_stream_ops_declared_universe`.
    // TS.6 P2 (K-G7) retired `UboDeterminationSelectStrategy`'s registration
    // (27→26), then `UboDeterminationComputeFold`'s (26→25), then
    // `UboEdgePierceNominee`'s (25→24 — folded into a macro composing
    // `UboEdgeAssertControl` + `UboEdgeSupersede`, both already registered).
    // TS.6 P1/P2 then moved `KycPersonApprove`/`KycPersonReject` OUT of this
    // file entirely (renamed `DecideApprove`/`DecideReject`, registered in
    // `ob-poc-kyc-decide` instead) — `registered_op_fqns()` now scans both
    // source files, so the total count is unchanged at 24 (a relocation,
    // not a net registration change).
    let declared = kyc_stream_ops_declared_universe();
    let registered = registered_op_fqns();
    assert_eq!(
        registered.len(),
        23,
        "registered dsl.kyc-adjacent op count (incl. the 2 W5 screening-hook \
         ops co-hosted in kyc_stream_ops.rs, and decide.approve/decide.reject \
         in ob-poc-kyc-decide) drifted from 23: {registered:#?}"
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
    //
    // TS.6 P1/P2: `kyc.person.approve`/`.reject` (renamed `decide.approve`/
    // `.reject`) moved out of `assembly_lexicon()` into `evaluation_lexicon()`
    // — still declared in the same two YAML files `declared_verb_universe()`
    // scans, so coverage is checked against the union of both lexicons, not
    // `assembly_lexicon()` alone (a verb belongs to exactly one pack; "not in
    // Assembly" is correct for these two, not a gap).
    let assembly = assembly_lexicon();
    let evaluation = evaluation_lexicon();
    let uncovered: BTreeSet<String> = declared_verb_universe()
        .into_iter()
        .filter(|fqn| assembly.get(fqn).is_none() && evaluation.get(fqn).is_none())
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
    // `kyc.person.approve` renamed `decide.approve` TS.6 P2 — its entry now
    // lives in `evaluation_lexicon()`, not `assembly_lexicon()` (it no
    // longer reaches the fact stream at all, so there is nothing for
    // `render_intent_event_to_sexpr` to render in practice — this loop
    // still proves the entry itself is fqn-correct and render-safe, which
    // is all it ever asserted).
    let assembly = assembly_lexicon();
    let evaluation = evaluation_lexicon();
    for (fqn, lexicon) in [
        ("kyc.obligation.create", &assembly),
        ("decide.approve", &evaluation),
    ] {
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
    // D1 (EOP-DD-KYCUBO-TS.1): a THIRD fold axis exists
    // (`fold::type_registry::TypeRegistryState`) — the 4 new type-registry
    // moves fold there, not in control.rs/obligation.rs, so this scan must
    // include it or they'd be misreported as fold-blind when they are not.
    let type_registry_arms = fold_match_arms(TYPE_REGISTRY_FOLD_SRC);

    let fold_blind: BTreeSet<String> = declared_verb_universe()
        .into_iter()
        // decide.approve/decide.reject (TS.6 P1/P2) are not fold-blind in
        // the K-G7 sense — that finding is about a verb that DOES append to
        // the fact stream but whose event no fold ever reads back (a
        // defect: a write that silently goes nowhere). decide.* verbs never
        // append to the fact stream AT ALL (ob-poc-kyc-decide has no
        // dependency on ob-poc-kyc-seam) — "fold-blind" doesn't apply to a
        // verb that was never fold-eligible to begin with, so they're
        // excluded from this universe rather than allow-listed as an
        // exception to a defect class they don't belong to.
        .filter(|fqn| fqn != "decide.approve" && fqn != "decide.reject")
        .filter(|fqn| {
            !control_arms.contains(fqn)
                && !obligation_arms.contains(fqn)
                && !type_registry_arms.contains(fqn)
        })
        .collect();

    // kyc.role.assign/withdraw (the K-G7 finding) were retired 2026-08-12
    // rather than wired in — see dsl-kyc-obligation.yaml's retirement
    // comment. `ubo.determination.compute-fold` — the one allow-listed
    // fold-inert verb (effect_class: read_snapshot, fold-blind by design) —
    // was itself retired TS.6 P2 (K-G7): it was a derivation dressed as a
    // verb, carrying no precondition `freeze` doesn't already declare
    // independently, so there was nothing to preserve by keeping it. The
    // set is now empty with no by-design exception left to allow-list.
    let expected: BTreeSet<String> = BTreeSet::new();

    assert_eq!(
        fold_blind, expected,
        "K-G7 fold-blind verb set changed — expected fully empty post-\
         compute-fold-retirement (TS.6 P2); any member is a regression"
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

/// Pin (K-G5): as of the T6 row-9 fix (2026-08-17) the map was fully
/// studded — every stud-bearing verb, including `kyc.subject.register`,
/// carries a stud — and ALL 11 structure classes have a strategy behind
/// them (the guard set is total). **Updated by TS.5 (2026-08-22):**
/// `ubo.edge.assert-control`/`assert-economic-interest` each gain
/// `Precondition::TypeGeometryPermits` — TS.1 §1's type-geometry layer was
/// declared but never actually reachable from this checker until TS.5; this
/// pin's widening records that, not a drift. (`ubo.edge.pierce-nominee` also
/// carried this at the time — its own entry was retired TS.6 P2, folded
/// into the two verbs already listed here.)
/// Authoring a precondition, or a new `DeterminationStrategy`, is a CONSCIOUS
/// edit here, not a silent pass or a silent break.
#[test]
fn precondition_and_strategy_coverage_is_exactly_known() {
    let lexicon = assembly_lexicon();
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
    // T6.1(c) (2026-08-12): the fail-closed strategy guard (matrix rows
    // 6a/8a), originally split across `select-strategy` (6a) and `freeze`
    // (8a, defense in depth). TS.6 P2 (K-G7) RETIRED `select-strategy` —
    // the strategy is now DERIVED from `structure_class`
    // (`strategy_for_structure_class`), never separately asserted, so
    // `StructureClassSupported` alone gates readiness on `freeze`.
    // `ubo.determination.compute-fold` (which also carried this pair) was
    // ITSELF retired TS.6 P2 as a derivation dressed as a verb, redundant
    // with `freeze`'s own independent declaration — no entry survives it
    // here. The strategy-COUNT pin below (2) is unchanged — this is a
    // precondition-map change, not a new `DeterminationStrategy`. The
    // `StructureClassified`/bare `SubjectRegistered` variants that used to
    // attach to `select-strategy` remain evaluable T6.1(b) machinery,
    // unattached to any entry today.
    expected.insert(
        "ubo.determination.freeze".to_string(),
        vec![
            Precondition::ReconciledProjection,
            Precondition::StructureClassSupported,
        ],
    );
    // T6.2 (2026-08-12, edge family — EOP-DD-KYCUBO-KIT-T6 matrix rows 1-5):
    // 5 more verbs go from geometry-free to studded. Rows 1/2 share the same
    // two studs (registration + no-duplicate-active-edge); rows 3/4 share
    // the same two (edge exists + active); row 5 is registration alone,
    // deliberately WITHOUT the optional "≥1 active economic edge" amendment
    // (kept callable early, per the ratified matrix note).
    // TS.5 R1 (2026-08-22): `TypeGeometryPermits` added to both
    // edge-asserting verbs — TS.1 §1's FIRST constraint layer (the type
    // geometry) was never actually reachable from this checker before TS.5;
    // it is now, and this is a conscious widening of the pin, not a drift.
    expected.insert(
        "ubo.edge.assert-control".to_string(),
        vec![
            Precondition::SubjectRegistered,
            Precondition::NoDuplicateActiveEdge,
            Precondition::TypeGeometryPermits,
        ],
    );
    expected.insert(
        "ubo.edge.assert-economic-interest".to_string(),
        vec![
            Precondition::SubjectRegistered,
            Precondition::NoDuplicateActiveEdge,
            Precondition::TypeGeometryPermits,
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
    // "ubo.edge.pierce-nominee" LexiconEntry RETIRED (TS.6 P2, K-G7,
    // 2026-08-22) — no entry survives it here. Its precondition set (subject
    // registered + target edge exists + active + geometry, TS.5 §6 Q2)
    // decomposes exactly across `ubo.edge.assert-control`'s entry (above:
    // SubjectRegistered, NoDuplicateActiveEdge, TypeGeometryPermits) and
    // `ubo.edge.supersede`'s entry (below: EdgeExists, EdgeActive); the
    // "target is actually a nominee edge" check (no precondition primitive)
    // moved to assert-control's op-layer, gated on its new `pierced-from`
    // arg.

    // T6.3 (2026-08-12, determination-family remainder — EOP-DD-KYCUBO-KIT-T6
    // matrix rows 7, 9, 10; row 6 (`select-strategy`) RETIRED by TS.6 P2 and
    // row 7 (`apply-smo-fallback`) by TS.6 §5 — see the note above `freeze`,
    // which carries row 7's identical precondition pair; row 8 is the
    // unchanged 8a freeze guard, asserted above).
    // row 9 (`kyc.subject.register`) CLOSED (2026-08-17, corrects
    // EOP-DD-KYCUBO-KIT-T6 §5 — see its §6 amendment and the lexicon
    // entry's own comment): `NotAlreadyRegistered` is now keyed off the
    // event's `entity_id` (`ControlState.registered_entity_ids`), so it no
    // longer conflicts with this verb's real multi-call-per-stream usage
    // (one call per natural-person candidate under one subject_root).
    expected.insert(
        "kyc.subject.register".to_string(),
        vec![Precondition::NotAlreadyRegistered],
    );
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
    // `Precondition::SubjectNotDecided` retired TS.6 P2 (K-G7) alongside
    // `kyc.person.approve`/`.reject` (renamed `decide.approve`/`.reject`,
    // moved to `ob-poc-kyc-decide` — no lexicon entry here any more, no
    // substrate Precondition either; their finality guard now queries
    // `kyc_decision_records` directly). The 5 obligation verbs below keep
    // only `ObligationExists`.
    for fqn in [
        "assert.identity",
        "assert.screening",
        "assert.risk",
        "kyc.obligation.satisfy",
        "kyc.obligation.waive",
    ] {
        expected.insert(fqn.to_string(), vec![Precondition::ObligationExists]);
    }

    // D1 (EOP-DD-KYCUBO-TS.1 §3, moves 2/6/7/8): assert-type carries only
    // EntityRegistered; withdraw-member/correct-type additionally carry
    // their Phase 2-promoted positional studs (MembershipActive,
    // PriorTypeAsserted — tree-cleanup follow-up tranche, EOP-STATE-
    // KYCUBO-D1 §4/§7); record-enquiry carries none (row 8: "group exists"
    // is true by construction).
    expected.insert("kyc.subject.assert-type".to_string(), vec![Precondition::EntityRegistered]);
    expected.insert(
        "kyc.subject.correct-type".to_string(),
        vec![Precondition::EntityRegistered, Precondition::PriorTypeAsserted],
    );
    expected.insert(
        "kyc.subject.withdraw-member".to_string(),
        vec![Precondition::EntityRegistered, Precondition::MembershipActive],
    );
    expected.insert("kyc.subject.record-enquiry".to_string(), vec![]);

    assert_eq!(
        actual.len(),
        19,
        "assembly_lexicon() entry count drifted from the post-TS.6-P1 20 \
         lexicon-covered verbs (25 post-D1, minus the retired select-strategy, \
         compute-fold, and pierce-nominee entries, minus decide.approve/decide.reject \
         moved to evaluation_lexicon())"
    );
    assert_eq!(
        actual, expected,
        "K-G5 precondition map changed — as of TS.6 P1/P2 (2026-08-22, select-strategy, \
         compute-fold, and pierce-nominee's own entry all retired; kyc.person.approve/.reject \
         moved out to evaluation_lexicon() as decide.approve/.reject), every one of the 16 \
         remaining dsl.kyc verbs with a stud carries it (verify, freeze, the 5 edge-family \
         verbs, apply-smo-fallback, register, classify-structure, and the 6 obligation \
         verbs — piercing's precondition pair now reaches the stream via assert-control's \
         and supersede's own entries); any other change is either matrix progress (update \
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
        8,
        "freeze's implemented DeterminationStrategy count drifted from the \
         TS.4 8 (ownership_prong_strategy, control_prong_strategy, \
         trust_role_strategy, fund_control_strategy, \
         foundation_council_strategy, state_owned_strategy, \
         cooperative_member_strategy, nominee_pierce_strategy) — a new \
         strategy is a CONSCIOUS pin edit here AND in the TS.1 split pin \
         below: {strategies:#?}"
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
        // TS.2 §5 growth (D1 Part B) — closes `every_geometry_pipe_is_assertable`.
        "officer_appointment",
        "management_mandate",
        "membership_rights",
        "statutory_authority",
        "employment",
        "containment",
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
            vec![StructureClass::LimitedPartnershipFund, StructureClass::Llp],
        ),
        ("trust_role_strategy", vec![StructureClass::Trust]),
        // TS.2 (EOP-DD-KYCUBO-KIT-TS0 §2.2/§2.3): fund control sits with the
        // manager; foundation control sits with the council.
        (
            "fund_control_strategy",
            vec![StructureClass::InvestmentFund],
        ),
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
        // TS.4 (EOP-DD-KYCUBO-KIT-TS0 §2.6): post-piercing the subject
        // resolves by the UNDERLYING structure — the strategy delegates to
        // the control prong; the "no unpierced nominee edges" guard lives at
        // the freeze dispatch site (resolve() cannot error). The guard set
        // is now TOTAL: 11/11 classes served, none fail-closed.
        ("nominee_pierce_strategy", vec![StructureClass::Nominee]),
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
         versa) is exactly what this pin fail-closes; after TS.4 the split \
         is 8 arms serving all 11 classes — TOTAL (lockstep rule, \
         EOP-DD-KYCUBO-KIT-TS0 §1c)"
    );
}

// ── K-G1/K-G2: EdgeStatus reachability (real fold-chain behavior) ─────────

fn registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
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
    let lexicon_hash = assembly_lexicon().hash;
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
    let state = fold_control_versioned(&[&assert_event, &evidence_event], &reg).expect("fold ok");
    assert_eq!(
        state.edges.get(&edge).unwrap().status,
        EdgeStatus::Evidenced
    );

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
        &[
            &assert_event,
            &evidence_event,
            &verify_event,
            &supersede_event,
        ],
        &reg,
    )
    .expect("fold ok");
    assert_eq!(
        state.edges.get(&edge).unwrap().status,
        EdgeStatus::Superseded
    );
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
/// closure — the Part A A3 fix; `ubo.determination.compute-fold`'s own such
/// call site was retired along with the verb, TS.6 P2). Looks ahead a
/// bounded window (25 lines, well past the real call site's distance) for
/// the checker call, stopping early at the next `lexicon.get(` so a miss
/// can never be misattributed to the wrong fqn.
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
    let lexicon = assembly_lexicon();
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
    // have found at least the pre-T6.2 wired verbs as a floor, or a scanner
    // bug (not a real gap) could be silently pass this test by finding
    // nothing to check in the first place. (`select-strategy`/`compute-fold`
    // retired TS.6 P2, `apply-smo-fallback` TS.6 §5 — all dropped from this
    // floor list; `freeze` still carries the identical precondition pair the
    // last two of those declared.)
    for known_wired in ["ubo.edge.verify", "ubo.determination.freeze"] {
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
    let lexicon = assembly_lexicon();
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
            superseded_by: None,
            pierced_from: None,
        },
    );
    assert!(
        check_preconditions(
            verify_entry,
            &asserted_only,
            &empty_obligation,
            &TypeRegistryState::default(),
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
            &TypeRegistryState::default(),
            &probe("ubo.edge.verify", TargetBinding::for_edge(subject, edge)),
        )
        .is_ok(),
        "verify must admit an edge with evidence attached"
    );

    // ubo.determination.freeze — ReconciledProjection + StructureClassSupported.
    // TS.6 P2 (K-G7) retired `select-strategy` AND `compute-fold`;
    // `StructureClassSupported` (the T6.1(c)/TS.4 fail-closed guard) now
    // lives solely on `freeze`. This sub-test pins (i) totality — every one
    // of the 11 StructureClass variants is admitted (given reconciliation
    // too, since freeze also carries `ReconciledProjection`) — and
    // (ii) the fail-closed floor — a subject with NO recorded class (which
    // is exactly what an unknown/garbage wire string folds to,
    // `structure_class_from_payload` → None) still blocks.
    let freeze_entry_for_totality = lexicon.get("ubo.determination.freeze").unwrap();
    let all_classes = [
        StructureClass::PrivateCompany,
        StructureClass::MultiTierHoldingGroup,
        StructureClass::ListedEntity,
        StructureClass::LimitedPartnershipFund,
        StructureClass::Llp,
        StructureClass::Trust,
        StructureClass::Foundation,
        StructureClass::InvestmentFund,
        StructureClass::StateOwned,
        StructureClass::Cooperative,
        StructureClass::Nominee,
    ];
    assert_eq!(
        all_classes.len(),
        ob_poc_kyc_substrate::IMPLEMENTED_STRATEGY_CLASSES.len(),
        "the implemented set must be TOTAL post-TS.4 (11/11 classes)"
    );
    for class in all_classes {
        let supported = ControlState {
            registered: true,
            reconciliation_event_id: Some(ob_poc_kyc_substrate::EventId::new()),
            structure_class: Some(class.clone()),
            ..Default::default()
        };
        assert!(
            check_control_preconditions(
                freeze_entry_for_totality,
                &supported,
                &TypeRegistryState::default(),
                &probe(
                    "ubo.determination.freeze",
                    TargetBinding::for_subject(subject)
                ),
            )
            .is_ok(),
            "freeze must admit every implemented structure class \
             post-TS.4 (totality); {class:?} was blocked"
        );
    }
    // The fail-closed floor: no class (= what a garbage wire string folds
    // to) still blocks, even with registration+reconciliation satisfied.
    let unclassified = ControlState {
        registered: true,
        reconciliation_event_id: Some(ob_poc_kyc_substrate::EventId::new()),
        structure_class: None,
        ..Default::default()
    };
    assert!(
        check_control_preconditions(
            freeze_entry_for_totality,
            &unclassified,
            &TypeRegistryState::default(),
            &probe(
                "ubo.determination.freeze",
                TargetBinding::for_subject(subject)
            ),
        )
        .is_err(),
        "freeze must still fail-close on a subject with no recorded \
         structure class (unknown wire strings fold to None)"
    );

    // ubo.determination.freeze — ReconciledProjection alone: an
    // unreconciled state must block even with a supported class set.
    let not_ready = ControlState::default();
    assert!(
        check_control_preconditions(
            freeze_entry_for_totality,
            &not_ready,
            &TypeRegistryState::default(),
            &probe(
                "ubo.determination.freeze",
                TargetBinding::for_subject(subject)
            ),
        )
        .is_err(),
        "freeze must block before reconcile-conflict + a supported structure class have \
         fired (K-14)"
    );
}

// ── D1 corrective tranche Item 2 — traversal-blind tooth (TS.2 EdgeKind growth) ─

/// TS.2 §5 grew `EdgeKind` by 6 variants (`OfficerAppointment`,
/// `ManagementMandate`, `MembershipRights`, `StatutoryAuthority`,
/// `Employment`, `Containment`) so every geometry-declared pipe became
/// assertable (`every_geometry_pipe_is_assertable`,
/// `ts2_pipe_convergence.rs`). Assertable is not the same question as
/// consumed by a *determination* — this tooth answers that second
/// question directly and behaviorally, driving every real
/// `DeterminationStrategy::resolve()` impl with a synthetic lone edge,
/// rather than trusting doc comments (several of which — `ControlProng
/// Strategy`'s in particular — name a CLOSED list of kinds it "traverses"
/// that is, in fact, stale: the real code has no kind filter at all
/// beyond excluding `EconomicInterest`/`Nominee`).
///
/// **Finding, reported rather than silently absorbed: the corrective
/// tranche's own brief — "no determination traversal consumes them" — is
/// FALSE for half the strategy set.** `control_prong_strategy`,
/// `fund_control_strategy` (a thin delegate), `state_owned_strategy`, and
/// `nominee_pierce_strategy` (also a thin delegate) all build their
/// adjacency from `reconciled_control_edges()`, which admits every active
/// non-economic, non-nominee edge with NO kind filter — so an
/// `Employment` or `MembershipRights` edge is walked exactly like a
/// `VotingRights` edge, undifferentiated, and surfaces a
/// `Prong::ControlByOtherMeans` candidate from it today. The premise IS
/// true for the three structure-class-specific strategies that DO
/// kind-filter (`trust_role_strategy`, `foundation_council_strategy`,
/// `cooperative_member_strategy`, each a closed `matches!` list) and for
/// `ownership_prong_strategy` (economic axis only, by construction).
/// `Nominee` is the one kind traversed by NOTHING — by design, guarded
/// away before any strategy runs (K-8 pierce-first, at the freeze
/// dispatch site).
///
/// This is arguably a WORSE gap than blindness — 4 of 8 strategies
/// silently over-admit rather than silently ignore — but wiring a fix
/// touches `DeterminationStrategy` selection/traversal, which the D1
/// corrective tranche's own SCOPE FENCE excludes. This tooth exists so
/// the true shape cannot be forgotten or silently narrowed back to the
/// false "just unconsumed" framing: assertable does not mean
/// *deliberately* consumed — for 4 of 8 strategies it means
/// *accidentally* consumed, undifferentiated from every other control
/// kind.
#[test]
fn edge_kind_strategy_admission_is_exactly_known() {
    let kinds: &[(&str, EdgeKind)] = &[
        ("economic_interest", EdgeKind::EconomicInterest),
        ("voting_rights", EdgeKind::VotingRights),
        ("board_appointment", EdgeKind::BoardAppointment),
        ("gp_statutory", EdgeKind::GpStatutory),
        ("designated_member", EdgeKind::DesignatedMember),
        ("trust_settlor", EdgeKind::TrustRole(TrustRoleKind::Settlor)),
        ("trust_trustee", EdgeKind::TrustRole(TrustRoleKind::Trustee)),
        ("trust_protector", EdgeKind::TrustRole(TrustRoleKind::Protector)),
        ("trust_beneficiary", EdgeKind::TrustRole(TrustRoleKind::Beneficiary)),
        ("nominee", EdgeKind::Nominee),
        ("dominant_influence", EdgeKind::DominantInfluence),
        ("officer_appointment", EdgeKind::OfficerAppointment),
        ("management_mandate", EdgeKind::ManagementMandate),
        ("membership_rights", EdgeKind::MembershipRights),
        ("statutory_authority", EdgeKind::StatutoryAuthority),
        ("employment", EdgeKind::Employment),
        ("containment", EdgeKind::Containment),
    ];
    assert_eq!(
        kinds.len(),
        17,
        "TS.0 §3's pipe vocabulary is 17-wide; EdgeKind mirrors it 1:1 including \
         all 4 TrustRole sub-kinds"
    );

    let strategies: &[(&str, &dyn DeterminationStrategy)] = &[
        ("ownership_prong_strategy", &OwnershipProngStrategy),
        ("control_prong_strategy", &ControlProngStrategy),
        ("trust_role_strategy", &TrustRoleStrategy),
        ("fund_control_strategy", &FundControlStrategy),
        ("foundation_council_strategy", &FoundationCouncilStrategy),
        ("state_owned_strategy", &StateOwnedStrategy),
        ("cooperative_member_strategy", &CooperativeMemberStrategy),
        ("nominee_pierce_strategy", &NomineePierceStrategy),
    ];
    // Not 1:1 with `IMPLEMENTED_STRATEGY_CLASSES` (11 classes) — several
    // classes share a strategy (e.g. the ownership-axis classes all use
    // `ownership_prong_strategy`); 8 is the count of distinct
    // `DeterminationStrategy` impls (`determination.rs`), independently
    // re-verified here.
    assert_eq!(strategies.len(), 8, "8 distinct DeterminationStrategy impls exist today");

    let subject_entity = EntityId(uuid::Uuid::new_v4());
    let person = EntityId(uuid::Uuid::new_v4());
    let natural_persons: BTreeSet<PersonId> = [PersonId(person.0)].into_iter().collect();

    // Behaviorally computed, not asserted from a doc read: for every
    // (kind, strategy) pair, does a LONE such edge (natural person ->
    // subject) surface a candidate?
    let mut actual: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (kind_name, kind) in kinds {
        let mut state = ControlState::default();
        let edge_id = EdgeId(uuid::Uuid::new_v4());
        state.edges.insert(
            edge_id,
            ob_poc_kyc_substrate::EdgeState {
                id: edge_id,
                kind: kind.clone(),
                from: person,
                to: subject_entity,
                percentage: matches!(kind, EdgeKind::EconomicInterest).then_some(100.0),
                status: EdgeStatus::Asserted,
                evidence_event_id: None,
                originating_event_id: EventId::new(),
                trust_revocable: None,
                superseded_by: None,
                pierced_from: None,
            },
        );
        for (strategy_name, strategy) in strategies {
            let candidates = strategy.resolve(&state, subject_entity, &natural_persons, 25.0);
            if !candidates.is_empty() {
                actual.entry(*strategy_name).or_default().insert(*kind_name);
            }
        }
    }

    // EOP-DD-KYCUBO-TS.3 (RATIFIED 2026-08-21) reconciled the whitelist
    // against V&S v0.6 §6.4's control-axis column: `management_mandate`
    // and `membership_rights` are named control axes for funds/cooperatives
    // respectively and move from excluded to admitted (`Traverse`,
    // `control_admission`, fold/control.rs). `broad_control` now pins the
    // 11-kind Traverse set the full-walk strategies share; `cooperative_
    // member_strategy` additionally gains `membership_rights` on top of its
    // own narrower 3-kind filter (TS.3 §3: the co-op control axis).
    let broad_control: BTreeSet<&str> = [
        "voting_rights",
        "board_appointment",
        "gp_statutory",
        "designated_member",
        "trust_settlor",
        "trust_trustee",
        "trust_protector",
        "trust_beneficiary",
        "dominant_influence",
        "management_mandate",
        "membership_rights",
    ]
    .into_iter()
    .collect();
    let mut expected: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    expected.insert("ownership_prong_strategy", ["economic_interest"].into_iter().collect());
    expected.insert("control_prong_strategy", broad_control.clone());
    expected.insert("fund_control_strategy", broad_control.clone());
    expected.insert("state_owned_strategy", broad_control.clone());
    expected.insert("nominee_pierce_strategy", broad_control);
    expected.insert(
        "trust_role_strategy",
        ["trust_settlor", "trust_trustee", "trust_protector"].into_iter().collect(),
    );
    expected.insert(
        "foundation_council_strategy",
        ["board_appointment", "dominant_influence"].into_iter().collect(),
    );
    expected.insert(
        "cooperative_member_strategy",
        ["voting_rights", "board_appointment", "dominant_influence", "membership_rights"]
            .into_iter()
            .collect(),
    );

    assert_eq!(
        actual, expected,
        "strategy edge-kind admission drifted — a strategy started (or stopped) \
         traversing a kind it didn't (or did) before; this is a conscious-edit \
         pin, not a guess (TS.3 four-way classification)"
    );

    // TS.3 §4: the admission function is now a four-way class
    // (`Traverse | Stop | NotControl | Pierce`), not a boolean. `nominee`
    // (Pierce) and `statutory_authority` (Stop — recorded via
    // `detect_statutory_stops`, not walked) are excluded from every
    // strategy's WALK by design, same as before. `officer_appointment`
    // (NotControl — pulled on exhaustion by `pull_smo_on_exhaustion`,
    // §4a, never pushed by admission), `employment`, `containment` stay
    // NotControl per §3's citations. This is a deliberate admission
    // boundary, not a gap.
    let all_traversed_by_someone: BTreeSet<&str> =
        actual.values().flat_map(|s| s.iter().copied()).collect();
    let untraversed_by_anyone: BTreeSet<&str> = kinds
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !all_traversed_by_someone.contains(name))
        .collect();
    assert_eq!(
        untraversed_by_anyone,
        ["nominee", "officer_appointment", "statutory_authority", "employment", "containment"]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "control_admission's non-Traverse set must be exactly these 5 kinds — `nominee` \
         (Pierce, K-8), `statutory_authority` (Stop, TS.3 §3), `officer_appointment` \
         (NotControl, pulled on exhaustion only), `employment`/`containment` (NotControl, \
         obligation basis / structural scoping); if this set grew or shrank, that is a \
         real admission-boundary change and must be reported, not silently re-absorbed"
    );
}

// ── Phase 1 (whitelist tranche, EOP-STATE-KYCUBO-D1 §4, 2026-08-21) ────────

/// SAFETY proof, not a semantics ruling: for every `EdgeKind` the exclusion
/// filter admitted BEFORE the whitelist tranche (`c7ef69ca`) — and which
/// TS.3 (RATIFIED 2026-08-21, `control_admission`, fold/control.rs) ALSO
/// still admits — the full `ProngCandidate` a lone edge produces — person,
/// prong, effective ownership %, chain — is bit-identical.
///
/// **Scope note (TS.3):** this test deliberately covers only the ORIGINAL
/// 9 kinds, not the 2 TS.3 newly admits (`management_mandate`,
/// `membership_rights`) — those are exactly where TS.3 intends a real
/// difference (`fund_with_manco_now_resolves`,
/// `cooperative_resolves_via_membership`), so pinning them here as
/// "unchanged" would be the WRONG assertion for this tranche. Equivalence
/// for the untouched 9, difference for the newly-admitted 2 — both
/// correct, for different reasons.
///
/// The four strategies checked here
/// (`control_prong_strategy` directly; `fund_control_strategy` and
/// `nominee_pierce_strategy`, which thin-delegate to it; `state_owned_strategy`,
/// which re-implements the identical full-admission walk) are exactly the
/// ones whose behaviour Phase 1 COULD have changed — they carried no
/// kind-filter of their own before this tranche, unlike
/// `foundation_council_strategy`/`cooperative_member_strategy`, which are
/// unaffected by construction (checked separately, structurally, in
/// `edge_kind_strategy_admission_is_exactly_known`).
///
/// The pin below is re-derived BY RUNNING against the (already whitelisted)
/// production code — its value is proving today's determinations are
/// unchanged, not in guessing the numbers by hand.
///
/// **Bite proof (perturb the whitelist, observe red, restore, observe
/// green — pasted in the tranche receipt, not just asserted here):**
/// temporarily move any one of `is_admitted_as_control`'s 9 true-arm kinds
/// into the false arm. This test goes red immediately — the perturbed
/// kind's candidate list drops from one person to empty, failing the
/// equality assertion below. Every one of the 9 kinds is load-bearing to
/// this test, not decorative coverage.
#[test]
fn determination_is_unchanged_by_whitelisting() {
    let subject_entity = EntityId(uuid::Uuid::from_u128(0xD1_5E1F_0000_0000_0000_0000_0001));
    let person = EntityId(uuid::Uuid::from_u128(0xD1_5E1F_0000_0000_0000_0000_0002));
    let natural_persons: BTreeSet<PersonId> = [PersonId(person.0)].into_iter().collect();
    let fixed_event = EventId(uuid::Uuid::from_u128(0xD1_5E1F_0000_0000_0000_0000_00E1));

    // The 9 kinds admitted both before (exclusion filter) and after
    // (whitelist) Phase 1 — everything `edge_kind_strategy_admission_is_-
    // exactly_known`'s `broad_control` set pins today.
    let admitted_kinds: &[(&str, EdgeKind)] = &[
        ("voting_rights", EdgeKind::VotingRights),
        ("board_appointment", EdgeKind::BoardAppointment),
        ("gp_statutory", EdgeKind::GpStatutory),
        ("designated_member", EdgeKind::DesignatedMember),
        ("trust_settlor", EdgeKind::TrustRole(TrustRoleKind::Settlor)),
        ("trust_trustee", EdgeKind::TrustRole(TrustRoleKind::Trustee)),
        ("trust_protector", EdgeKind::TrustRole(TrustRoleKind::Protector)),
        ("trust_beneficiary", EdgeKind::TrustRole(TrustRoleKind::Beneficiary)),
        ("dominant_influence", EdgeKind::DominantInfluence),
    ];

    let strategies: &[(&str, &dyn DeterminationStrategy)] = &[
        ("control_prong_strategy", &ControlProngStrategy),
        ("fund_control_strategy", &FundControlStrategy),
        ("state_owned_strategy", &StateOwnedStrategy),
        ("nominee_pierce_strategy", &NomineePierceStrategy),
    ];

    let mut actual: BTreeMap<(&str, &str), Vec<(PersonId, Prong, Option<u64>, Vec<EntityId>, EventId)>> =
        BTreeMap::new();
    for (kind_name, kind) in admitted_kinds {
        let mut state = ControlState::default();
        let edge_id = EdgeId(uuid::Uuid::from_u128(0xD1_5E1F_0000_0000_0000_0000_0003));
        state.edges.insert(
            edge_id,
            ob_poc_kyc_substrate::EdgeState {
                id: edge_id,
                kind: kind.clone(),
                from: person,
                to: subject_entity,
                percentage: None,
                status: EdgeStatus::Asserted,
                evidence_event_id: None,
                originating_event_id: fixed_event,
                trust_revocable: None,
                superseded_by: None,
                pierced_from: None,
            },
        );
        for (strategy_name, strategy) in strategies {
            let candidates = strategy.resolve(&state, subject_entity, &natural_persons, 25.0);
            let summary = candidates
                .into_iter()
                .map(|c| {
                    (
                        c.person_id,
                        c.prong,
                        c.effective_ownership_pct.map(|p| p.to_bits()),
                        c.ownership_chain,
                        c.originating_event_id,
                    )
                })
                .collect();
            actual.insert((kind_name, strategy_name), summary);
        }
    }

    let candidate = vec![(
        PersonId(person.0),
        Prong::ControlByOtherMeans,
        None,
        vec![subject_entity],
        fixed_event,
    )];
    let mut expected: BTreeMap<(&str, &str), Vec<(PersonId, Prong, Option<u64>, Vec<EntityId>, EventId)>> =
        BTreeMap::new();
    for (kind_name, _) in admitted_kinds {
        for (strategy_name, _) in strategies {
            expected.insert((kind_name, strategy_name), candidate.clone());
        }
    }

    assert_eq!(
        actual, expected,
        "determination_is_unchanged_by_whitelisting: a still-admitted kind's \
         determination diverged from the pre-Phase-1 golden baseline — the \
         whitelist must change ONLY which kinds count as control, never the \
         resolved candidate for a kind it still admits"
    );
}

/// Companion to `determination_is_unchanged_by_whitelisting`: pins that the
/// remaining excluded kinds are provably excluded from EVERY strategy's
/// walk, with the reason recorded inline rather than left to be
/// rediscovered. TS.3 (RATIFIED 2026-08-21) closed the ratification
/// question for `management_mandate`/`membership_rights` (now admitted —
/// see `edge_kind_strategy_admission_is_exactly_known`,
/// `fund_with_manco_now_resolves`, `cooperative_resolves_via_membership`);
/// this test now covers the FOUR kinds TS.3 §3 keeps out of the walk on
/// separate, distinct grounds: `officer_appointment` (NotControl — pulled
/// on exhaustion only, `pull_smo_on_exhaustion`), `statutory_authority`
/// (Stop, not a walk exclusion — see `statutory_authority_stops_with_
/// reason`; still produces zero *candidates* from any strategy, which is
/// what this test checks), `employment`/`containment` (NotControl,
/// obligation basis / structural scoping per V&S citations).
#[test]
fn new_edge_kinds_are_not_traversed_as_control() {
    let subject_entity = EntityId(uuid::Uuid::from_u128(0xD1_5E1F_0000_0000_0000_0000_0004));
    let person = EntityId(uuid::Uuid::from_u128(0xD1_5E1F_0000_0000_0000_0000_0005));
    let natural_persons: BTreeSet<PersonId> = [PersonId(person.0)].into_iter().collect();

    // TS.3 §3/§4: `NotControl`- and `Stop`-classified kinds — never part of
    // the WALK (a `Stop` still produces zero candidates; it just also
    // records why, tested separately).
    let excluded_kinds: &[(&str, EdgeKind)] = &[
        ("officer_appointment", EdgeKind::OfficerAppointment),
        ("statutory_authority", EdgeKind::StatutoryAuthority),
        ("employment", EdgeKind::Employment),
        ("containment", EdgeKind::Containment),
    ];

    let strategies: &[(&str, &dyn DeterminationStrategy)] = &[
        ("ownership_prong_strategy", &OwnershipProngStrategy),
        ("control_prong_strategy", &ControlProngStrategy),
        ("trust_role_strategy", &TrustRoleStrategy),
        ("fund_control_strategy", &FundControlStrategy),
        ("foundation_council_strategy", &FoundationCouncilStrategy),
        ("state_owned_strategy", &StateOwnedStrategy),
        ("cooperative_member_strategy", &CooperativeMemberStrategy),
        ("nominee_pierce_strategy", &NomineePierceStrategy),
    ];

    for (kind_name, kind) in excluded_kinds {
        let mut state = ControlState::default();
        let edge_id = EdgeId(uuid::Uuid::from_u128(0xD1_5E1F_0000_0000_0000_0000_0006));
        state.edges.insert(
            edge_id,
            ob_poc_kyc_substrate::EdgeState {
                id: edge_id,
                kind: kind.clone(),
                from: person,
                to: subject_entity,
                percentage: None,
                status: EdgeStatus::Asserted,
                evidence_event_id: None,
                originating_event_id: EventId::new(),
                trust_revocable: None,
                superseded_by: None,
                pierced_from: None,
            },
        );
        for (strategy_name, strategy) in strategies {
            let candidates = strategy.resolve(&state, subject_entity, &natural_persons, 25.0);
            assert!(
                candidates.is_empty(),
                "{kind_name} must not be traversed as control by {strategy_name} \
                 (excluded pending a TS ratification, not forgotten): got {candidates:#?}"
            );
        }
    }
}

// ── D1 corrective tranche Item 3, closed by Phase 2 (tree-cleanup follow-up,
//    EOP-STATE-KYCUBO-D1 §4/§7) — op-layer-only studs promoted ────────────

/// D1's Item 3 pinned TWO op-layer-only studs (`kyc.subject.withdraw-member`'s
/// "membership must be active", `kyc.subject.correct-type`'s "a type was
/// already asserted") that had no `Precondition` primitive and were
/// hand-duplicated in the op layer and the board preview, because the
/// checker's signature could not see `TypeRegistryState` at all.
///
/// Phase 2 widened `check_preconditions`/`check_control_preconditions` to
/// accept `&TypeRegistryState` (threaded through the real append chokepoint,
/// `ob-poc-kyc-store::PgKycEventStore::append`, and every other caller),
/// promoted both studs to real `Precondition` variants
/// (`MembershipActive`, `PriorTypeAsserted`), and deleted both hand-rolled
/// copies. This tooth now pins the CLOSED state: the pinned op-layer-only
/// set is EMPTY, not two. `no_stud_is_duplicated` (below) is the structural
/// companion — it pins the absence of the deleted hand-rolled markers.
#[test]
fn op_layer_only_studs_are_exactly_known() {
    // (a) Both lexicon entries now declare the promoted Preconditions — the
    // studs are lexicon/Precondition-declared, no longer op-layer-only.
    let lexicon = assembly_lexicon();
    let withdraw_entry = lexicon
        .get("kyc.subject.withdraw-member")
        .expect("kyc.subject.withdraw-member must be in assembly_lexicon()");
    assert_eq!(
        withdraw_entry.preconditions,
        vec![Precondition::EntityRegistered, Precondition::MembershipActive],
        "withdraw-member must declare the promoted MembershipActive precondition \
         alongside EntityRegistered — if this reverts to just EntityRegistered, \
         the stud silently went back to being op-layer-only"
    );
    let correct_entry = lexicon
        .get("kyc.subject.correct-type")
        .expect("kyc.subject.correct-type must be in assembly_lexicon()");
    assert_eq!(
        correct_entry.preconditions,
        vec![Precondition::EntityRegistered, Precondition::PriorTypeAsserted],
        "correct-type must declare the promoted PriorTypeAsserted precondition \
         alongside EntityRegistered — if this reverts to just EntityRegistered, \
         the stud silently went back to being op-layer-only"
    );

    // (b) The checker's own signature now DOES accept TypeRegistryState —
    // the structural precondition for both promotions above.
    let sig_marker = "pub fn check_preconditions(";
    let sig_start = CONTROL_FOLD_SRC
        .find(sig_marker)
        .expect("check_preconditions signature must exist in fold/control.rs");
    let sig_end = CONTROL_FOLD_SRC[sig_start..]
        .find(") -> Result<(), KycError> {")
        .map(|i| sig_start + i)
        .expect("check_preconditions signature must close with its return type");
    let signature = &CONTROL_FOLD_SRC[sig_start..sig_end];
    assert!(
        signature.contains("TypeRegistryState"),
        "check_preconditions must accept TypeRegistryState now that both former \
         op-layer-only studs are promoted through it. Signature was: {signature}"
    );
}

/// Structural companion to `op_layer_only_studs_are_exactly_known`: pins
/// that NO hand-rolled `TypeRegistryState` precondition logic for
/// withdraw-member/correct-type remains in the op layer
/// (`src/domain_ops/kyc_stream_ops.rs`) or the board preview
/// (`crates/ob-poc-kyc-substrate/src/placement.rs`) — both now consult the
/// single `check_preconditions`/`check_control_preconditions` checker only.
/// Source-scanned, not semantic: if either of these markers reappears
/// verbatim, the duplication this whole tooth family exists to prevent has
/// silently come back.
#[test]
fn no_stud_is_duplicated() {
    let op_markers = [
        ("kyc_stream_ops.rs", "type_registry.is_withdrawn(entity)"),
        ("kyc_stream_ops.rs", "type_registry.type_of(entity).is_none()"),
    ];
    for (label, marker) in op_markers {
        assert!(
            !KYC_STREAM_OPS_SRC.contains(marker),
            "{label}: found the deleted hand-rolled check {marker:?} — the \
             withdraw-member/correct-type stud duplication has come back; both \
             must be enforced solely by check_preconditions inside stream_append"
        );
    }

    // Scoped to the withdraw-member/correct-type blocks specifically —
    // `attach-evidence`'s type-scoped half (a legitimate, DIFFERENT,
    // never-duplicated positional check) uses the same
    // `type_registry.is_withdrawn`/`type_registry.type_of` calls in its own
    // right and must not false-positive this tooth.
    let placement_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/ob-poc-kyc-substrate/src/placement.rs"
    ))
    .expect("read placement.rs");
    let withdraw_block_start = placement_src
        .find("if let Some(entry) = withdraw_member_entry {")
        .expect("withdraw_member_entry block must exist in placement.rs");
    let correct_block_start = placement_src
        .find("if let Some(entry) = correct_type_entry {")
        .expect("correct_type_entry block must exist in placement.rs");
    let attach_evidence_start = placement_src
        .find("// attach-evidence, type-scoped half")
        .expect("attach-evidence type-scoped comment must exist in placement.rs");
    let withdraw_block = &placement_src[withdraw_block_start..correct_block_start];
    let correct_block = &placement_src[correct_block_start..attach_evidence_start];
    assert!(
        !withdraw_block.contains("is_withdrawn"),
        "placement.rs::type_registry_candidates (withdraw-member block): found a \
         hand-rolled is_withdrawn check — the board-preview side of the stud \
         duplication has come back; it must consult check_control_preconditions only"
    );
    assert!(
        !correct_block.contains("type_of(entity)"),
        "placement.rs::type_registry_candidates (correct-type block): found a \
         hand-rolled type_of check — the board-preview side of the stud \
         duplication has come back; it must consult check_control_preconditions only"
    );
}

/// TS.6 / K-G7 — **the search index**, not just source files.
///
/// `retired_verbs_are_gone` above scans YAML declarations, op structs and
/// `LexiconEntry`s — source only. That is exactly why four retired FQNs
/// (`kyc.role.assign`, `kyc.role.withdraw`,
/// `ubo.determination.select-strategy`, `ubo.determination.compute-fold`)
/// survived every sweep as live rows in `verb_pattern_embeddings` with zero
/// `dsl_verbs` backing: an operator utterance still resolved to a dead verb
/// while "grep-proven gone" reported clean (found 2026-08-22).
///
/// An orphan is an embedding whose FQN is neither a live verb NOR a live
/// macro. Macros are deliberately absent from `dsl_verbs` — they live in
/// `config/verb_schemas/macros/` and are expanded before execution — so the
/// naive "no dsl_verbs row" test would wrongly flag every macro
/// (`ubo.edge.pierce-nominee` is the live example).
///
/// Live-DB. Ignored by default so the pure pack stays runnable without a
/// database; run it in the DB lane.
#[tokio::test]
#[ignore = "needs DATABASE_URL"]
async fn retired_verbs_are_gone_from_the_search_index() {
    use std::collections::BTreeSet;

    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect");

    let orphans: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT e.verb_name, count(*)::bigint
           FROM "ob-poc".verb_pattern_embeddings e
           LEFT JOIN "ob-poc".dsl_verbs d ON d.full_name = e.verb_name
           WHERE d.full_name IS NULL
           GROUP BY e.verb_name ORDER BY e.verb_name"#,
    )
    .fetch_all(&pool)
    .await
    .expect("query orphans");

    // Every FQN declared as a macro anywhere under config/verb_schemas/macros/.
    let mut macro_fqns: BTreeSet<String> = BTreeSet::new();
    let dir = std::path::Path::new("config/verb_schemas/macros");
    for entry in std::fs::read_dir(dir).expect("read macros dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read macro yaml");
        for line in src.lines() {
            // Top-level (column-0) `some.macro.fqn:` keys are macro names.
            if let Some(name) = line.strip_suffix(':') {
                if !name.is_empty()
                    && !name.starts_with(char::is_whitespace)
                    && !name.starts_with('#')
                    && name.contains('.')
                {
                    macro_fqns.insert(name.to_string());
                }
            }
        }
    }
    assert!(
        macro_fqns.contains("ubo.edge.pierce-nominee"),
        "macro scan is broken — it must find the known live macro"
    );

    // KYC/UBO scope: this pack's own retirements. `bpmn.*` orphans are a
    // separate, pre-existing finding outside this tranche's fence.
    let kyc_orphans: Vec<String> = orphans
        .iter()
        .filter(|(fqn, _)| fqn.starts_with("kyc.") || fqn.starts_with("ubo.") || fqn.starts_with("assert.") || fqn.starts_with("decide."))
        .filter(|(fqn, _)| !macro_fqns.contains(fqn))
        .map(|(fqn, n)| format!("{fqn} ({n} rows)"))
        .collect();

    assert!(
        kyc_orphans.is_empty(),
        "retired KYC/UBO verbs still discoverable in the search index — \
         prune verb_pattern_embeddings + verb_centroids for: {kyc_orphans:#?}"
    );
}

/// The projection queue is gone (2026-08-22 ruling): no drainer type, no
/// outbox effect-kind constant, no fan-out from `append`.
///
/// Grep-proof rather than compile-proof on purpose. `dead_code = "deny"`
/// cannot see a re-introduced drainer that a test calls, and the K-G7 lesson
/// (and the TS.6 Item-4 embedding-orphan lesson right after it) is that a
/// retired thing comes back through whatever surface nobody is scanning. The
/// KEPT half — `rebuild_control_edges`/`rebuild_obligations`, the K-34
/// fold-whole-stream + full-replace machinery — is asserted PRESENT here, so
/// this gate cannot be satisfied by deleting the projector too.
#[test]
fn projection_queue_is_gone() {
    const BANNED: &[(&str, &str)] = &[
        ("PgKycProjectionDrainer", "control-edge drainer type"),
        ("PgKycObligationDrainer", "obligation drainer type"),
        ("CONTROL_EDGE_PROJECTION_EFFECT", "control-edge effect kind"),
        ("OBLIGATION_PROJECTION_EFFECT", "obligation effect kind"),
        ("PROJECTION_EFFECT_KINDS", "the fan-out's effect-kind list"),
        ("enqueue_projection_effects", "the append fan-out itself"),
        ("kyc.projection.control_edges", "control-edge effect-kind string"),
        ("kyc.projection.obligations", "obligation effect-kind string"),
    ];
    let sources: &[(&str, &str)] = &[
        ("store.rs", KYC_STORE_SRC),
        ("projection.rs", KYC_PROJECTION_SRC),
        ("lib.rs", KYC_STORE_LIB_SRC),
    ];

    let mut found: Vec<String> = Vec::new();
    for (file, src) in sources {
        for line in src.lines() {
            // Retirement notes are allowed to NAME what they retired; only
            // live code counts. (Same carve-out shape as `retired_verbs_are_gone`.)
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            for (sym, what) in BANNED {
                if line.contains(sym) {
                    found.push(format!("{file}: {what} (`{sym}`) — {}", line.trim()));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "the projection queue was removed 2026-08-22; these symbols are back \
         in live code: {found:#?}"
    );

    // The KEPT half must still be there — this gate must not be satisfiable
    // by deleting the projector along with its trigger.
    for kept in [
        "pub struct PgKycProjector",
        "pub async fn rebuild_control_edges",
        "pub struct PgKycObligationProjector",
        "pub async fn rebuild_obligations",
    ] {
        assert!(
            KYC_PROJECTION_SRC.contains(kept),
            "K-34 machinery must survive the queue removal; missing: {kept}"
        );
    }
    // Full-replace semantics, specifically: the rebuild DELETEs the subject's
    // rows before re-inserting from the fold.
    assert_eq!(
        KYC_PROJECTION_SRC.matches("DELETE FROM").count(),
        3,
        "full-replace: one DELETE per projected table (control edges, \
         obligations, subject rollup)"
    );
}

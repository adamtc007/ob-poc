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
//! `check_preconditions` genuinely reached when this verb's op runs")
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
    check_preconditions, fold_control_versioned, assembly_lexicon,
    evaluation_lexicon,
    AuthorityRef, ControlProngStrategy, ControlState, CooperativeMemberStrategy,
    DeterminationDispatch, DeterminationStrategy, EdgeId, EdgeKind, EdgeStatus, EntityId,
    EntityType, EntityTypeRecord, EventId, FoldRegistry, FoundationCouncilStrategy,
    FundControlStrategy, Hash, IntentEvent, NomineePierceStrategy,
    OwnershipProngStrategy, PersonId, Precondition, Principal, Prong, ProofKind, ProofRecord,
    StateOwnedStrategy, SubjectId, TargetBinding, TrustRoleKind, TrustRoleStrategy,
    TypeRegistryState, V1FoldImpl,
};

const DSL_KYC_YAML: &str = include_str!("../config/verbs/kyc/dsl-kyc.yaml");
const DSL_KYC_OBLIGATION_YAML: &str = include_str!("../config/verbs/kyc/dsl-kyc-obligation.yaml");
const SCREENING_YAML: &str = include_str!("../config/verbs/screening.yaml");
const KYC_DAG_YAML: &str = include_str!("../config/sem_os_seeds/dag_taxonomies/kyc_dag.yaml");
const KYC_STREAM_OPS_SRC: &str = include_str!("../src/domain_ops/kyc_stream_ops.rs");
// TS.6 P1/P2: kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject moved to ob-poc-kyc-decide — a
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
const TYPE_REGISTRY_FOLD_SRC: &str =
    include_str!("../crates/ob-poc-kyc-substrate/src/fold/type_registry.rs");

// ── Declaration extraction ──────────────────────────────────────────────────

/// Parse `domains: { <domain>: { verbs: { <verb>: ... } } }` from a dsl.kyc
/// verb YAML file — a plain line-scan (2-space domain keys, 6-space verb
/// keys) rather than a full YAML parse, matching the fixed hand-authored
/// indentation of these two files (verified: exactly 2 domain lines + 16 verb
/// lines in dsl-kyc.yaml, 1 domain line + 8 verb lines in
/// dsl-kyc-obligation.yaml — 24 total, post K-G7 retirement of
/// kyc.role.assign/withdraw (2026-08-12), the TS.4 `kyc_ubo.assert.edge.nominee-piercing`
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
/// — the W5 screening hook, which fans out `kyc_ubo.assert.entity.screening`
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
/// ob-poc-kyc-decide (TS.6 P1/P2 — kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject moved out of
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
fn verb_universe_is_exactly_14() {
    // TS.6 P1/P2: kyc.person.approve/.reject renamed kyc_ubo.decide.subject.approve/.reject
    // and moved to a new `decide:` domain block in the SAME YAML file
    // (dsl-kyc-obligation.yaml) — a rename+relocation, not a retirement, so
    // this count is unchanged at 22 (declared_verb_universe() scans both
    // files' domains, not just `kyc:`).
    //
    // D2.0 §5 (2026-08-22): `creation`/`satisfaction` DISSOLVED (21→19).
    // `waiver` renamed+relocated to `kyc_ubo.decide.obligation.waiver` in
    // the same YAML file — a relocation, not a count change.
    //
    // T2 (2026-08-27, EOP-VS-UBO-GAME-001 §8 Q1): `register` + `type` MERGED
    // into `place` (one verb absorbs two declarations, 19→18, P1);
    // `member-withdrawal` renamed `remove` — a rename, not a count change
    // (P2). `type-correction` DISSOLVED (18→17, P3) — no replacement.
    //
    // T3 (2026-08-27, §3.2/§3.3): `control` + `economic-interest` MERGED
    // into `connect` (one verb absorbs two declarations, 17→16, P1);
    // `supersession` renamed `disconnect` — a rename, not a count change
    // (P2, R8's fold-time-pruning correction touches `remove`'s behavior,
    // not the verb count). `reconciliation` DISSOLVED (16→15, P3) — no
    // replacement.
    //
    // T4 (EOP-DD-UBO-DISPATCH-001, 2026-08-28): `structure-class` RETIRED
    // (15→14) — no replacement.
    let fqns = declared_verb_universe();
    assert_eq!(
        fqns.len(),
        14,
        "dsl.kyc verb count drifted from the post-T4 14 (post-T3 15, minus \
         structure-class RETIRED) (17 post-T2, \
         minus control+economic-interest MERGED into connect, minus \
         reconciliation DISSOLVED; supersession renamed disconnect is net \
         zero) (18 post-P1/P2, minus type-correction DISSOLVED) (19 \
         post-D2.0, minus register+type MERGED into place) (21 \
         post-TS.6-§5, minus creation/satisfaction DISSOLVED) (25 post-D1, \
         minus the retired select-strategy, compute-fold, and pierce-nominee \
         verb declarations) — update the T0.3 audit and every other pinned \
         test in this file, not just this assertion: {fqns:#?}"
    );
}

// ── TS.6 §8: pack-membership, boundary, and retirement gates ───────────────

#[test]
fn assembly_pack_is_exactly_known() {
    // D2.0 §5 (2026-08-22): the 16 `assembly_lexicon()` entries — `creation`/
    // `satisfaction` DISSOLVED, `waiver` MOVED to `evaluation_lexicon()`.
    // T2 (2026-08-27, §8 Q1): `register` + `type` MERGED into `place`
    // (16→15, P1); `member-withdrawal` renamed `remove` (P2).
    // `type-correction` DISSOLVED (15→14, P3) — no replacement entry.
    // T3 (2026-08-27, §3.2/§3.3): `control` + `economic-interest` MERGED
    // into `connect` (14→13, P1); `supersession` renamed `disconnect`
    // (net zero, P2); `reconciliation` DISSOLVED (13→12, P3) — no
    // replacement entry.
    // T4 (EOP-DD-UBO-DISPATCH-001, 2026-08-28): `structure-class` RETIRED
    // (12→11) — the strategy is now derived from EntityType directly, no
    // replacement entry.
    // T5 (EOP-DD-UBO-PROOF-001, 2026-08-28): `verification` RETIRED
    // (K-G7, 0 real committed events) — "the board collects facts; the
    // policy rules on adequacy," so there is no ratchet left to verify
    // into. `retract` ADDED (net zero, 11→11) — withdraws a previously
    // logged proof by citation id.
    let expected: BTreeSet<String> = [
        "kyc_ubo.assert.subject.place",
        "kyc_ubo.assert.subject.remove",
        "kyc_ubo.assert.subject.enquiry",
        "kyc_ubo.assert.edge.connect",
        "kyc_ubo.assert.edge.evidence",
        "kyc_ubo.assert.edge.retract",
        "kyc_ubo.assert.edge.disconnect",
        "kyc_ubo.decide.determination.freeze",
        "kyc_ubo.assert.entity.identity",
        "kyc_ubo.assert.entity.screening",
        "kyc_ubo.assert.entity.risk",
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
    // D2.0 §5 (2026-08-22): the 2 landed verdicts plus `kyc_ubo.decide.
    // obligation.waiver`, moved here from assembly_lexicon().
    let expected: BTreeSet<String> = [
        "kyc_ubo.decide.subject.approve",
        "kyc_ubo.decide.subject.reject",
        "kyc_ubo.decide.obligation.waiver",
    ]
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
        "kyc_ubo.assert.entity.screening",
        "kyc_ubo.assert.entity.identity",
        "kyc_ubo.assert.entity.risk",
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
fn evaluation_pack_dependency_graph_excludes_the_append_chokepoint() {
    // RENAMED from `evaluation_pack_cannot_write_facts` (EOP-DD-KYCUBO-D2.1
    // §6/§7 Q1, 2026-08-23). The old name asserted the evaluation pack
    // "cannot write facts" — a claim the 2026-08-23 reconciliation disproved
    // BY EXECUTION for the second time: `ob-poc-kyc-decide` holds an `sqlx`
    // dependency and a live `&mut PgConnection` via `scope.executor()`, so a
    // probe using raw SQL against `"ob-poc".kyc_intent_events` compiled,
    // ran, and inserted a real fact-stream row while this dep-tree check
    // still reported PASS. A crate-dependency check cannot express "must
    // never write to table X" when the crate holds a SQL driver and a
    // connection — no widening of the forbidden-crate list closes that gap,
    // because the reachable path was never a crate dependency at all.
    //
    // D2.1 §7 Q1 (RULED): the two-pack SPLIT is the design and is correct —
    // the evaluation pack is read-only on the UBO board and writes only its
    // own run sheet, and no Sage session can execute evaluation verbs
    // against the assembly pack. What a crate can technically reach with a
    // raw SQL driver is an implementation property of the crate, not a
    // defect in that design. So the gate is corrected to assert what is
    // actually TRUE and load-bearing — the dependency graph excludes the
    // GOVERNED append chokepoint (`ob-poc-kyc-seam::append_in_scope`, the
    // sole path every real dsl.kyc verb dispatches through) and its
    // underlying store (`ob-poc-kyc-store::PgKycEventStore::append`) —
    // rather than the stronger, now twice-disproven claim that no fact can
    // reach the table by any means. A gate asserting something false is
    // worse than a gate that cannot fail.
    //
    // Still falsifiable the same way as before: re-add `ob-poc-kyc-store`
    // (or `ob-poc-kyc-seam`) to `crates/ob-poc-kyc-decide/Cargo.toml` and
    // this goes red.
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
            "ob-poc-kyc-decide must never depend on {forbidden} — it is the governed \
             dsl.kyc fact-stream append chokepoint (D2.1 §7 Q1 \
             evaluation_pack_dependency_graph_excludes_the_append_chokepoint). \
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
        "structure-class:",
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
        "struct KycSubjectClassifyStructure",
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
        "\"kyc_ubo.assert.subject.structure-class\"",
    ] {
        assert!(
            !LEXICON_SRC.contains(needle),
            "retired LexiconEntry {needle} still present in lexicon.rs"
        );
    }
}

/// T2 (EOP-VS-UBO-GAME-001, 2026-08-27, §8 Q1, P3) — K-G7 grep-proof for
/// `kyc_ubo.assert.subject.type-correction`'s DISSOLUTION, mirroring
/// `retired_verbs_are_gone`'s pattern but scoped to this tranche's own
/// retirement rather than TS.6's. This one had 0 real committed events
/// (confirmed by DB query before deletion) — a full deletion, not a
/// fold-arm-kept historical retirement, so it must be gone from every
/// declaring surface with no exceptions (unlike `register`/`type`/
/// `member-withdrawal`, whose fold arms deliberately remain for replay).
/// The search-index half of this K-G7 proof is the generic orphan detector
/// `retired_verbs_are_gone_from_the_search_index` below (left-joins
/// `verb_pattern_embeddings` against `dsl_verbs`) — no bespoke FQN list
/// needed there, since a stale `type-correction` embedding row will have no
/// matching `dsl_verbs` row once `cargo x verbs compile` runs.
#[test]
fn type_correction_is_gone() {
    assert!(
        !DSL_KYC_YAML.contains("type-correction:"),
        "retired verb declaration type-correction: still present in dsl-kyc.yaml"
    );
    assert!(
        !KYC_STREAM_OPS_SRC.contains("struct KycSubjectCorrectType"),
        "retired op struct KycSubjectCorrectType still present in kyc_stream_ops.rs"
    );
    assert!(
        !LEXICON_SRC.contains("\"kyc_ubo.assert.subject.type-correction\""),
        "retired LexiconEntry kyc_ubo.assert.subject.type-correction still present in lexicon.rs"
    );
    assert!(
        !TYPE_REGISTRY_FOLD_SRC.contains("\"kyc_ubo.assert.subject.type-correction\" =>"),
        "retired fold match arm for kyc_ubo.assert.subject.type-correction still present \
         in fold/type_registry.rs"
    );
    assert!(
        !TYPE_REGISTRY_FOLD_SRC.contains("fn edges_invalidated_by_correction"),
        "retired cascade function edges_invalidated_by_correction still present in \
         fold/type_registry.rs"
    );
}

// ── K-G3: unmapped move ─────────────────────────────────────────────────────

#[test]
fn stream_governed_family_covers_every_declared_verb() {
    let globs = stream_governed_globs();
    // 7 → 5 (four-segment rename, RATIFIED 2026-08-22) → 4 (D2.0 §5,
    // 2026-08-22: `kyc_ubo.assert.obligation.*` removed outright, not just
    // left vacuous, once it was actually EMPTY — `.creation`/`.satisfaction`
    // dissolved (K-G7) and `.waiver` moved to the Evaluation-plane
    // `kyc_ubo.decide.obligation.waiver`, so no verb could ever match this
    // glob again). The two-glob drop from 7→5 was a conscious keep-vacuous
    // move (see the `kyc.role.*`/`kyc.person.*` note below); this 5→4 drop
    // is a conscious REMOVE, because the same domain segment
    // (`assert.obligation`) has zero remaining stream-governed members —
    // there is nothing left to leave in place.
    assert_eq!(
        globs.len(),
        4,
        "stream_governed.verb_families count drifted from the audited 7: {globs:#?}"
    );
    // The kyc.role.* glob is vacuous after the K-G7 retirement (no live verb
    // matches it) — left in kyc_dag.yaml deliberately: governance-block
    // edits are a separate conscious change from verb retirement, and the
    // glob costs nothing to leave (it covers zero verbs, not a wrong verb).
    // kyc.person.* is likewise now vacuous (TS.6 P1/P2): kyc_ubo.decide.subject.approve/
    // kyc_ubo.decide.subject.reject don't match it and are deliberately excluded from this
    // loop below — they are NOT stream-governed any more (ob-poc-kyc-decide
    // never writes to the fact stream at all), so "must be covered by a
    // stream_governed family glob" does not apply to them by design.
    // kyc_ubo.decide.obligation.waiver (D2.0 §5, moved here 2026-08-22) is
    // excluded for the identical reason — same crate, same non-write.

    for fqn in declared_verb_universe() {
        if fqn == "kyc_ubo.decide.subject.approve"
            || fqn == "kyc_ubo.decide.subject.reject"
            || fqn == "kyc_ubo.decide.obligation.waiver"
        {
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
    //
    // D2.0 §5 (2026-08-22): `KycObligationCreate`/`KycObligationSatisfy`
    // DISSOLVED (23→21). `KycObligationWaive` MOVED to `ob-poc-kyc-decide`
    // as `DecideObligationWaive` — still counted once, so no further change.
    //
    // T2 (2026-08-27, §8 Q1): `KycSubjectRegister` + `KycSubjectAssertType`
    // MERGED into one `KycSubjectPlace` op (21→20, P1). `KycSubjectWithdrawMember`
    // renamed `KycSubjectRemove` — still counted once, no further change (P2).
    // `KycSubjectCorrectType` DELETED (20→19, P3) — no op survives it.
    //
    // T3 (2026-08-27, §3.2/§3.3): `UboEdgeAssertControl` + `UboEdgeAssertEconomicInterest`
    // MERGED into one `UboEdgeConnect` op (19→18, P1). `UboEdgeSupersede`
    // renamed `UboEdgeDisconnect` — still counted once, no further change
    // (P2). `UboEdgeReconcileConflict` DELETED (18→17, P3) — no op
    // survives it.
    //
    // T4 (EOP-DD-UBO-DISPATCH-001, 2026-08-28): `KycSubjectClassifyStructure`
    // DELETED (17→16) — no op survives it.
    let declared = kyc_stream_ops_declared_universe();
    let registered = registered_op_fqns();
    assert_eq!(
        registered.len(),
        16,
        "registered dsl.kyc-adjacent op count (incl. the 2 W5 screening-hook \
         ops co-hosted in kyc_stream_ops.rs, and kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject/ \
         kyc_ubo.decide.obligation.waiver in ob-poc-kyc-decide) drifted from 16: {registered:#?}"
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
    // TS.6 P1/P2: `kyc.person.approve`/`.reject` (renamed `kyc_ubo.decide.subject.approve`/
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
    // `kyc.person.approve` renamed `kyc_ubo.decide.subject.approve` TS.6 P2 — its entry now
    // lives in `evaluation_lexicon()`, not `assembly_lexicon()` (it no
    // longer reaches the fact stream at all, so there is nothing for
    // `render_intent_event_to_sexpr` to render in practice — this loop
    // still proves the entry itself is fqn-correct and render-safe, which
    // is all it ever asserted).
    let assembly = assembly_lexicon();
    let evaluation = evaluation_lexicon();
    for (fqn, lexicon) in [
        ("kyc_ubo.assert.entity.identity", &assembly),
        ("kyc_ubo.decide.subject.approve", &evaluation),
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
    // D1 (EOP-DD-KYCUBO-TS.1): a THIRD fold axis exists
    // (`fold::type_registry::TypeRegistryState`) — the 4 new type-registry
    // moves fold there, not in control.rs/obligation.rs, so this scan must
    // include it or they'd be misreported as fold-blind when they are not.
    let type_registry_arms = fold_match_arms(TYPE_REGISTRY_FOLD_SRC);

    let fold_blind: BTreeSet<String> = declared_verb_universe()
        .into_iter()
        // kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject (TS.6 P1/P2) are not fold-blind in
        // the K-G7 sense — that finding is about a verb that DOES append to
        // the fact stream but whose event no fold ever reads back (a
        // defect: a write that silently goes nowhere). decide.* verbs never
        // append to the fact stream AT ALL (ob-poc-kyc-decide has no
        // dependency on ob-poc-kyc-seam) — "fold-blind" doesn't apply to a
        // verb that was never fold-eligible to begin with, so they're
        // excluded from this universe rather than allow-listed as an
        // exception to a defect class they don't belong to.
        // kyc_ubo.decide.obligation.waiver (D2.0 §5, 2026-08-22) is excluded
        // for the identical reason — moved to the same never-appends crate.
        .filter(|fqn| {
            fqn != "kyc_ubo.decide.subject.approve"
                && fqn != "kyc_ubo.decide.subject.reject"
                && fqn != "kyc_ubo.decide.obligation.waiver"
        })
        .filter(|fqn| {
            !control_arms.contains(fqn)
                && !type_registry_arms.contains(fqn)
        })
        .collect();

    // kyc.role.assign/withdraw (the K-G7 finding) were retired 2026-08-12
    // rather than wired in — see dsl-kyc-obligation.yaml's retirement
    // comment. `ubo.determination.compute-fold` — the one allow-listed
    // fold-inert verb (effect_class: read_snapshot, fold-blind by design) —
    // was itself retired TS.6 P2 (K-G7): it was a derivation dressed as a
    // verb, carrying no precondition `freeze` doesn't already declare
    // independently, so there was nothing to preserve by keeping it.
    //
    // `kyc_ubo.assert.entity.{identity,screening,risk}` NEWLY fold-blind
    // (EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07): `fold/obligation.rs` —
    // their only reader — is deleted. This is not a new defect this
    // tranche introduces; the op layer had already stopped calling
    // `check_preconditions` for these three before this tranche
    // (`validate_entry_fqn: None`, `kyc_stream_ops.rs`, T6.4 rows 12-14),
    // so a landed event for any of them was already going nowhere in
    // practice — the fold's deletion just makes that structurally
    // explicit instead of merely true-in-practice. Allow-listed here
    // rather than hidden: a real, disclosed K-G7 member, not silently
    // patched away.
    //
    // `kyc_ubo.decide.determination.freeze` NEWLY fold-blind (same tranche,
    // same deletion): `fold/obligation.rs` carried a no-op arm,
    // `"kyc_ubo.decide.determination.freeze" => {}`, whose only purpose was
    // to satisfy this scanner — it did no fold work. Freeze's own read path
    // (`ob-poc-kyc-store::cross_stream::prior_freeze_persons`) queries the
    // `outbox` table directly, never `fold_control`/`fold_type_registry`, so
    // deleting the stub changes nothing real; it only removes the
    // appeasement arm and lets the scanner see what was already true.
    let expected: BTreeSet<String> = [
        "kyc_ubo.assert.entity.identity",
        "kyc_ubo.assert.entity.screening",
        "kyc_ubo.assert.entity.risk",
        "kyc_ubo.decide.determination.freeze",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    assert_eq!(
        fold_blind, expected,
        "K-G7 fold-blind verb set changed — expected exactly the 3 allow-listed \
         identity/screening/risk verbs (fold-blind since fold/obligation.rs's removal, \
         EOP-DD-UBO-CLEANOUT-001 T6 P2); any OTHER member is a regression"
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

/// Pin (K-G5): as of the T6 row-9 fix (2026-08-17) the map was fully
/// studded — every stud-bearing verb, including `kyc_ubo.assert.subject.register`,
/// carries a stud — and ALL 11 structure classes have a strategy behind
/// them (the guard set is total). **Updated by TS.5 (2026-08-22):**
/// `kyc_ubo.assert.edge.control`/`assert-economic-interest` each gain
/// `Precondition::TypeGeometryPermits` — TS.1 §1's type-geometry layer was
/// declared but never actually reachable from this checker until TS.5; this
/// pin's widening records that, not a drift. (`kyc_ubo.assert.edge.nominee-piercing` also
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
    // T5 (EOP-DD-UBO-PROOF-001, 2026-08-28): `verification`/`EvidenceCited`
    // RETIRED together (K-G7, 0 real committed events) — the ratchet they
    // gated no longer exists. `retract` carries no preconditions: a
    // citation either exists to be withdrawn or the removal is a no-op
    // (§4 — "the set shrinks and nothing else happens").
    expected.insert("kyc_ubo.assert.edge.retract".to_string(), vec![]);
    // T6.1(c) (2026-08-12): the fail-closed strategy guard (matrix rows
    // 6a/8a), originally split across `select-strategy` (6a) and `freeze`
    // (8a, defense in depth). TS.6 P2 (K-G7) RETIRED `select-strategy` —
    // the strategy was DERIVED from `structure_class`, never separately
    // asserted. EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28) RETIRED
    // `structure_class`/`StructureClassSupported` in turn — the strategy
    // is now derived directly from the subject's `EntityType`
    // (`dispatch_for_entity_type`, §2's ratified 21-type/7-strategy
    // mapping), gated by `EntityTypeSupportsStrategy`.
    // `ubo.determination.compute-fold` (which also carried this pair) was
    // ITSELF retired TS.6 P2 as a derivation dressed as a verb, redundant
    // with `freeze`'s own independent declaration — no entry survives it
    // here. EOP-VS-UBO-GAME-001 T3 (§3.3, 2026-08-27) removed the
    // `ReconciledProjection` half of the pair entirely, alongside
    // `reconciliation` itself — `EntityTypeSupportsStrategy` is now
    // freeze's sole precondition. The strategy-COUNT pin below (2) is
    // unchanged — this is a precondition-map change, not a new
    // `DeterminationStrategy`. `StructureClassified`/`StructureClassSupported`
    // are DELETED (T4) — nothing left to keep them declared for; a
    // precondition has no replay role the way the fold arm does.
    // 2026-09-07 (audit item 2, P2): NoUnpiercedNomineeEdges hoisted from
    // freeze's op-layer-only hand check (TS.4 §3 Ruling B, K-8) — the RULE
    // is now declared here too, even though freeze has exactly one live
    // surface (`canonical_event_shape` bails on this FQN by design, §3.2)
    // and this promotion doesn't close a two-surface disagreement the way
    // `PiercedFromIsActiveNominee` (below, on `connect`) does.
    expected.insert(
        "kyc_ubo.decide.determination.freeze".to_string(),
        vec![
            Precondition::EntityTypeSupportsStrategy,
            Precondition::NoUnpiercedNomineeEdges,
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
    //
    // EOP-VS-UBO-GAME-001 T3 (§3.2, 2026-08-27): `control` + `economic-
    // interest` MERGED into `connect` — the merge test (P0b) confirmed
    // their precondition sets were already identical, so this row is
    // unchanged in substance, just one name now covering both former rows.
    // 2026-09-07 (audit item 2, P1): PiercedFromIsActiveNominee hoisted
    // from `UboEdgeConnect::execute`'s op-layer-only hand check — the
    // workbook path never ran it, a real R7 defect closed by this
    // promotion (K-8, §3.4 R7).
    // 2026-09-08 (fuzz-harness geometry-closure finding #2, RULED by Adam
    // 2026-09-07, scope confirmed 2026-09-08): ConnectEndpointsNotWithdrawn
    // — neither endpoint of a `connect` may be currently WITHDRAWN; closes
    // the window a withdrawn-then-retyped entity left permanently
    // geometry-stale (no forward move could ever revisit an edge asserted
    // in that window). Deliberately narrower than "must be registered" —
    // a never-placed endpoint is untouched (stays Unevaluable/admit at
    // TypeGeometryPermits, R5/R6/CTN-2e), preserving the K-8 nominee-pierce
    // mechanism (its on-paper holder is never placed by design).
    expected.insert(
        "kyc_ubo.assert.edge.connect".to_string(),
        vec![
            Precondition::SubjectRegistered,
            Precondition::NoDuplicateActiveEdge,
            Precondition::TypeGeometryPermits,
            Precondition::PiercedFromIsActiveNominee,
            Precondition::ConnectEndpointsNotWithdrawn,
        ],
    );
    expected.insert(
        // 2026-08-24 corrective tranche Item 3a: dual-target verb — edge
        // pair vacuous when entity-scoped, entity pair vacuous when
        // edge-scoped (each half's arm checks `event.target.*_id`, `None`
        // is a no-op). Wires the `entity-id` half of TS.1 §3 row 4
        // ("evidence a TYPE OR A LINKAGE — one verb, two possible
        // targets") that was previously undispatchable.
        "kyc_ubo.assert.edge.evidence".to_string(),
        vec![
            Precondition::EdgeExists,
            Precondition::EdgeActive,
            Precondition::PriorTypeAsserted,
            Precondition::MembershipActive,
        ],
    );
    // T3 (§3.2): `supersession` renamed `disconnect` — precondition pair
    // unchanged.
    expected.insert(
        "kyc_ubo.assert.edge.disconnect".to_string(),
        vec![Precondition::EdgeExists, Precondition::EdgeActive],
    );
    // T3 (§3.3): `reconciliation` DISSOLVED — no entry survives it here.
    // "kyc_ubo.assert.edge.nominee-piercing" LexiconEntry RETIRED (TS.6 P2, K-G7,
    // 2026-08-22) — no entry survives it here. Its precondition set (subject
    // registered + target edge exists + active + geometry, TS.5 §6 Q2)
    // decomposes exactly across `kyc_ubo.assert.edge.connect`'s entry (above:
    // SubjectRegistered, NoDuplicateActiveEdge, TypeGeometryPermits) and
    // `kyc_ubo.assert.edge.disconnect`'s entry (above: EdgeExists, EdgeActive); the
    // "target is actually a nominee edge" check, gated on connect's
    // `pierced-from` arg, is `Precondition::PiercedFromIsActiveNominee`
    // as of 2026-09-07 (audit item 2, P1) — no longer op-layer-only.

    // T6.3 (2026-08-12, determination-family remainder — EOP-DD-KYCUBO-KIT-T6
    // matrix rows 7, 9, 10; row 6 (`select-strategy`) RETIRED by TS.6 P2 and
    // row 7 (`apply-smo-fallback`) by TS.6 §5 — see the note above `freeze`,
    // which carries row 7's identical precondition pair; row 8 is the
    // unchanged 8a freeze guard, asserted above).
    // row 9 (`kyc_ubo.assert.subject.register`) CLOSED (2026-08-17, corrects
    // EOP-DD-KYCUBO-KIT-T6 §5 — see its §6 amendment and the lexicon
    // entry's own comment): `NotAlreadyRegistered` is now keyed off the
    // event's `entity_id` (`ControlState.registered_entity_ids`), so it no
    // longer conflicts with this verb's real multi-call-per-stream usage
    // (one call per natural-person candidate under one subject_root).
    //
    // T2 (2026-08-27, §8 Q1): `register` MERGED into `place`, which carries
    // `NotCurrentlyPlaced` instead of `NotAlreadyRegistered` — place never
    // carries update semantics, so the guard is "not currently placed", not
    // "never before registered" (an already-withdrawn entity CAN be placed
    // again).
    expected.insert(
        "kyc_ubo.assert.subject.place".to_string(),
        vec![Precondition::NotCurrentlyPlaced],
    );
    // `kyc_ubo.assert.subject.structure-class` entry RETIRED
    // (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) — no entry survives it here,
    // same as `type-correction`/`reconciliation` above.

    // T6.4 (2026-08-12, obligation/person family — EOP-DD-KYCUBO-KIT-T6
    // matrix rows 11-18, every row cross-fold via the T6.1 unified checker).
    // D2.0 §5 (2026-08-22): `creation` (row 11) DISSOLVED — no lexicon entry
    // left to pin here. `satisfaction` (row 15) DISSOLVED; `waiver` (row 16)
    // MOVED to `evaluation_lexicon()` (`kyc_ubo.decide.obligation.waiver`,
    // `preconditions: []` — see `evaluation_pack_is_exactly_known`'s note on
    // why Evaluation entries carry none). The 3 remaining Assembly
    // obligation-fact verbs below carried `ObligationExists` until
    // EOP-DD-UBO-CLEANOUT-001 T6 P2 (2026-09-07) removed it —
    // `fold/obligation.rs`/`ObligationState` deleted (it was already dead
    // at the real write path, `validate_entry_fqn: None`,
    // `kyc_stream_ops.rs`) — so all three now carry `vec![]`, same as
    // `evaluation_lexicon()`'s decide.* entries.
    //
    // `Precondition::SubjectNotDecided` retired TS.6 P2 (K-G7) alongside
    // `kyc.person.approve`/`.reject` (renamed `kyc_ubo.decide.subject.approve`/`.reject`,
    // moved to `ob-poc-kyc-decide` — no lexicon entry here any more, no
    // substrate Precondition either; their finality guard now queries
    // `kyc_decision_records` directly).
    for fqn in [
        "kyc_ubo.assert.entity.identity",
        "kyc_ubo.assert.entity.screening",
        "kyc_ubo.assert.entity.risk",
    ] {
        expected.insert(fqn.to_string(), vec![]);
    }

    // D1 (EOP-DD-KYCUBO-TS.1 §3, moves 2/6/7/8): assert-type carries only
    // EntityRegistered; withdraw-member/correct-type additionally carry
    // their Phase 2-promoted positional studs (MembershipActive,
    // PriorTypeAsserted — tree-cleanup follow-up tranche, EOP-STATE-
    // KYCUBO-D1 §4/§7); record-enquiry carries none (row 8: "group exists"
    // is true by construction).
    //
    // T2 (2026-08-27, §8 Q1): standalone `type` MERGED into `place` (above);
    // `member-withdrawal` renamed `remove`. `type-correction` DISSOLVED
    // (P3) — no entry survives it here.
    // T3 (§3.4 R8, corrected 2026-08-27): `remove`'s precondition set is
    // UNCHANGED from T2 — it is never refused for touching links. Instead
    // it PRUNES them as a fold-time side effect (see
    // `fold::control::apply_one_control_event`'s `remove` arm); a
    // precondition-based refusal was tried and superseded before this
    // tranche closed, see EOP-STATE-KYCUBO-D1's T3 section.
    expected.insert(
        "kyc_ubo.assert.subject.remove".to_string(),
        vec![
            Precondition::EntityRegistered,
            Precondition::MembershipActive,
        ],
    );
    expected.insert("kyc_ubo.assert.subject.enquiry".to_string(), vec![]);

    assert_eq!(
        actual.len(),
        11,
        "assembly_lexicon() entry count drifted from the post-T5 11 \
         (post-T4 11, minus verification RETIRED, plus retract ADDED — \
         EOP-DD-UBO-PROOF-001, 2026-08-28, net zero) \
         (post-T3 12, minus structure-class RETIRED — EOP-DD-UBO-DISPATCH-001, \
         2026-08-28, the strategy is now derived from EntityType directly) \
         (14 post-T2, minus control+economic-interest MERGED into connect, \
         minus reconciliation DISSOLVED; supersession renamed disconnect is \
         net zero) (15 post-P1/P2, minus type-correction DISSOLVED) \
         (16 post-D2.0, minus register+type MERGED into place) \
         lexicon-covered verbs (19 post-TS.6-P1, minus creation/satisfaction \
         DISSOLVED and waiver MOVED to evaluation_lexicon(), D2.0 §5) \
         (25 post-D1, minus the retired select-strategy, \
         compute-fold, and pierce-nominee entries, minus kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject \
         moved to evaluation_lexicon())"
    );
    assert_eq!(
        actual, expected,
        "K-G5 precondition map changed — as of T5 (2026-08-28, \
         EOP-DD-UBO-PROOF-001: verification RETIRED taking EvidenceCited \
         with it — no ratchet left to gate; retract ADDED with no \
         preconditions); T4 (2026-08-28, EOP-DD-UBO-DISPATCH-001: \
         structure-class RETIRED, freeze's StructureClassSupported \
         superseded by EntityTypeSupportsStrategy; T3, 2026-08-27, \
         §3.2/§3.3/§3.4 R8: control+economic-interest MERGED \
         into connect, supersession renamed disconnect, reconciliation \
         DISSOLVED taking freeze's ReconciledProjection with it; remove's \
         precondition set is unchanged — R8 is enforced by fold-time \
         pruning, not a precondition refusal), every one of the 11 \
         remaining dsl.kyc verbs carries its correct stud set \
         (freeze, connect, evidence, disconnect, place, remove, \
         and the 3 obligation verbs; retract and enquiry carry none — \
         piercing's precondition pair now reaches the stream via connect's \
         and disconnect's own entries); \
         any other change is either matrix progress (update the T0.3 \
         audit) or a regression"
    );

    // `structure_class_valid_values()`'s 11-count check RETIRED
    // (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) alongside the
    // `kyc_ubo.assert.subject.structure-class` verb's YAML declaration —
    // there is no longer a structure-class valid_values list to audit.
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

// `implemented_class_split_matches_strategy_arms` DELETED (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) — it pinned the retired 11-class/8-arm `IMPLEMENTED_STRATEGY_CLASSES` lockstep, which no longer exists (StructureClass dispatch retired). `kyc_t4_dispatch.rs`'s `mapping_matches_the_ratified_table` is its successor: an independent hand-authored copy of §2's 21-type/7-strategy mapping, diffed against `dispatch_for_entity_type` (D4 — data with a test, not trust in the function's own arms).

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
        "kyc_ubo.assert.edge.connect",
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
    assert!(!state.edges.get(&edge).unwrap().has_proof());

    // EOP-DD-UBO-PROOF-001 §4 (T5, 2026-08-28): `EdgeStatus::Evidenced`/
    // `Verified` are gone — the ratchet they formed is replaced by a
    // citation set. An `evidence` event with a real kind logs a proof;
    // `status` stays `Asserted` throughout (only `disconnect` moves it).
    let evidence_event = make_event(
        subject,
        "kyc_ubo.assert.edge.evidence",
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({
            "kind": "filed-document",
            "source": "test fixture",
            "date": "2026-01-01",
        }),
        as_of,
        lexicon_hash,
    );
    let state = fold_control_versioned(&[&assert_event, &evidence_event], &reg).expect("fold ok");
    let edge_state = state.edges.get(&edge).unwrap();
    assert_eq!(edge_state.status, EdgeStatus::Asserted, "status is unaffected by evidence");
    assert!(edge_state.has_proof());
    assert_eq!(edge_state.proofs.len(), 1);

    // Supersede-from-proof-bearing — proves Superseded is reachable
    // regardless of the edge's citation set, not just from a bare-Asserted
    // edge (K-G1/K-G2: no dead end mid-lifecycle).
    let supersede_event = make_event(
        subject,
        "kyc_ubo.assert.edge.disconnect",
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({}),
        as_of,
        lexicon_hash,
    );
    let state = fold_control_versioned(&[&assert_event, &evidence_event, &supersede_event], &reg)
        .expect("fold ok");
    let edge_state = state.edges.get(&edge).unwrap();
    assert_eq!(edge_state.status, EdgeStatus::Superseded);
    assert!(edge_state.has_proof(), "supersede never touches the citation set (K-13)");
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
/// `check_preconditions(entry, ...)` / `check_preconditions(entry,
/// ...)` shape (`kyc_ubo.assert.edge.control`'s hand-rolled `append_in_scope`
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
            .or_else(|| window.find("check_preconditions(entry"));
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
/// that reaches `check_preconditions`/`check_preconditions` for that
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
    for known_wired in ["kyc_ubo.assert.edge.evidence", "kyc_ubo.decide.determination.freeze"] {
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
/// `check_preconditions`/`check_preconditions` composition the real
/// op calls (proven wired by the sibling test above), not a bespoke
/// re-implementation.
#[test]
fn precondition_carrying_verbs_actually_enforce_their_stud() {
    let lexicon = assembly_lexicon();
    let subject = SubjectId(uuid::Uuid::new_v4());
    let as_of = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

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

    // `kyc_ubo.assert.edge.verification` — EvidenceCited sub-test RETIRED
    // (EOP-DD-UBO-PROOF-001 §3/§4, T5, 2026-08-28) alongside the verb and
    // its precondition (K-G7: 0 real committed events). `retract`, its
    // replacement in the assembly pack, carries no preconditions — there
    // is no successor stud for this sub-test to exercise.

    // kyc_ubo.decide.determination.freeze — EntityTypeSupportsStrategy.
    // EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28): `StructureClassSupported`
    // retired, superseded by `EntityTypeSupportsStrategy` — the exhaustive
    // 21-type totality + fail-closed-floor proof now lives in
    // `kyc_t4_dispatch.rs` (`every_entity_type_has_a_ruling`,
    // `unmapped_type_refuses_freeze_by_name`, `terminal_types_are_explicit`,
    // `dispatch_is_exhaustive`). This sub-test's job is narrower and
    // distinct: does the REAL `LexiconEntry` (`assembly_lexicon()`, not a
    // hand-built one) actually enforce, end-to-end through
    // `check_preconditions` — one admitted type, one terminal type,
    // one untyped subject.
    let freeze_entry_for_totality = lexicon.get("kyc_ubo.decide.determination.freeze").unwrap();
    let typed = |t: EntityType| -> TypeRegistryState {
        let mut tr = TypeRegistryState::default();
        let citing = EventId::new();
        let mut proofs = BTreeMap::new();
        proofs.insert(
            citing,
            ProofRecord {
                kind: ProofKind::FiledDocument,
                source: "test fixture".to_string(),
                date: "2026-08-28".to_string(),
                event_id: citing,
            },
        );
        tr.types.insert(
            EntityId(subject.0),
            EntityTypeRecord { entity_type: t, originating_event_id: EventId::new(), proofs },
        );
        tr
    };
    let registered = ControlState { registered: true, ..Default::default() };

    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::PrivateLimitedCompany),
        DeterminationDispatch::Strategy("ownership_prong_strategy"),
        "sanity: PrivateLimitedCompany must be a live dispatch target for the admit case below"
    );
    assert!(
        check_preconditions(
            freeze_entry_for_totality,
            &registered,
            &typed(EntityType::PrivateLimitedCompany),
            &probe(
                "kyc_ubo.decide.determination.freeze",
                TargetBinding::for_subject(subject)
            ),
        )
        .is_ok(),
        "freeze must admit a subject whose EntityType dispatches to a live strategy"
    );

    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::NaturalPerson),
        DeterminationDispatch::NotADeterminationSubject,
        "sanity: NaturalPerson must be terminal for the refuse case below"
    );
    assert!(
        check_preconditions(
            freeze_entry_for_totality,
            &registered,
            &typed(EntityType::NaturalPerson),
            &probe(
                "kyc_ubo.decide.determination.freeze",
                TargetBinding::for_subject(subject)
            ),
        )
        .is_err(),
        "freeze must fail-close on a terminal (NotADeterminationSubject) EntityType"
    );

    // The fail-closed floor: no recorded type at all still blocks.
    assert!(
        check_preconditions(
            freeze_entry_for_totality,
            &registered,
            &TypeRegistryState::default(),
            &probe(
                "kyc_ubo.decide.determination.freeze",
                TargetBinding::for_subject(subject)
            ),
        )
        .is_err(),
        "freeze must still fail-close on a subject with no recorded EntityType"
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
                proofs: BTreeMap::new(),
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
    // EOP-DD-UBO-DISPATCH-001 Foundation ruling (T4-close, 2026-08-28): was
    // ["board_appointment", "dominant_influence"] — TS.1 geometry never
    // permitted either kind onto a real Foundation, so the filter was
    // corrected to TrustRole-kind edges (`reconciled_trust_edges` +
    // `TrustRoleStrategy::edge_qualifies`, reused directly) — the exact
    // set `trust_role_strategy` admits, `trust_beneficiary` excluded by
    // the same per-role rule (never a candidate) both here.
    expected.insert(
        "foundation_council_strategy",
        ["trust_settlor", "trust_trustee", "trust_protector"].into_iter().collect(),
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
                proofs: BTreeMap::new(),
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
                proofs: BTreeMap::new(),
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

/// D1's Item 3 pinned TWO op-layer-only studs (`kyc_ubo.assert.subject.member-withdrawal`'s
/// "membership must be active", `kyc_ubo.assert.subject.type-correction`'s "a type was
/// already asserted") that had no `Precondition` primitive and were
/// hand-duplicated in the op layer and the board preview, because the
/// checker's signature could not see `TypeRegistryState` at all.
///
/// Phase 2 widened `check_preconditions`/`check_preconditions` to
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
        .get("kyc_ubo.assert.subject.remove")
        .expect("kyc_ubo.assert.subject.remove must be in assembly_lexicon()");
    assert_eq!(
        withdraw_entry.preconditions,
        vec![Precondition::EntityRegistered, Precondition::MembershipActive],
        "remove (T2, formerly member-withdrawal) must declare the promoted \
         MembershipActive precondition alongside EntityRegistered — if this \
         reverts to just EntityRegistered, the stud silently went back to \
         being op-layer-only"
    );
    // `correct_entry` (`kyc_ubo.assert.subject.type-correction`'s promoted
    // PriorTypeAsserted stud) REMOVED — EOP-VS-UBO-GAME-001 T2 (§8 Q1,
    // 2026-08-27, P3) DISSOLVED the verb; no lexicon entry survives it to
    // assert against. `Precondition::PriorTypeAsserted` itself survives —
    // it is also declared by `kyc_ubo.assert.edge.evidence`'s entity-scoped
    // half (see `precondition_and_strategy_coverage_is_exactly_known`).

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
/// place/remove/correct-type remains in the op layer
/// (`src/domain_ops/kyc_stream_ops.rs`) or the board preview
/// (`crates/ob-poc-kyc-substrate/src/placement.rs`) — both now consult the
/// single `check_preconditions`/`check_preconditions` checker only.
/// Source-scanned, not semantic: if either of these markers reappears
/// verbatim, the duplication this whole tooth family exists to prevent has
/// silently come back.
///
/// T2 (2026-08-27, §8 Q1) widened this tooth: `place_and_remove_candidates`
/// (formerly `type_registry_candidates`, renamed when `register`+`type`
/// merged into `place` and `member-withdrawal` renamed `remove`) used to
/// hand-roll an `is_withdrawn` gate to decide which candidates to enumerate
/// for both `place` and `remove` — a duplicate of the `NotCurrentlyPlaced`/
/// `MembershipActive` precondition arms `check_preconditions`
/// already enforces on every probed candidate. Removed: `place` now always
/// offers the subject's own entity as a candidate (the checker refuses it
/// if already placed), and `remove` now always offers every registered
/// entity (the checker refuses it if already withdrawn).
///
/// 2026-09-07 (audit item 3, P2): `place` stopped probing
/// `check_preconditions` at enumeration entirely — it offers entity
/// TYPES now (`EOP-VS-UBO-GAME-001` §3.4 R9), computed from type geometry
/// against currently-active board members, not a per-entity-id oracle
/// probe (there is no id yet for a brand-new entity). Its `is_withdrawn`
/// call is therefore a NEW, legitimate, structurally different thing from
/// the T2-era duplication this tooth guards against: it filters WHICH
/// TYPED MEMBERS ARE CURRENTLY ACTIVE for geometry narrowing, never a
/// per-candidate "is THIS entity already placed" refusal (that check moved
/// entirely to `KycWorkbook::stage()`, run once against the caller's real
/// id — see that method's doc comment). The `NotCurrentlyPlaced` stud
/// itself has exactly one caller left: `check_preconditions`. This tooth
/// stays scoped to `remove`, which is unchanged and must still consult the
/// checker only.
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
             place/remove stud duplication has come back; both \
             must be enforced solely by check_preconditions inside stream_append"
        );
    }

    // Scoped to the place/remove blocks specifically — `attach-evidence`'s
    // type-scoped half (a legitimate, DIFFERENT, never-duplicated
    // positional check) uses the same `type_registry.is_withdrawn`/
    // `type_registry.type_of` calls in its own right and must not
    // false-positive this tooth. `correct-type`'s own block (T2 P3,
    // 2026-08-27, §8 Q1) DISSOLVED — the verb it belonged to no longer
    // exists, so there is nothing left between `remove`'s block and
    // `attach-evidence` for a third block to bound.
    let placement_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/ob-poc-kyc-substrate/src/placement.rs"
    ))
    .expect("read placement.rs");
    let place_block_start = placement_src
        .find("if lexicon.get(PLACE).is_some() {")
        .expect("place type-level block must exist in placement.rs");
    // Bounded by the REMOVE COMMENT, not the `if let` line — the comment
    // sits between the place block's closing brace and the remove code and
    // itself mentions `check_preconditions` (describing REMOVE's
    // own oracle use), which would otherwise leak into `place_block` and
    // false-positive the assertion below.
    let remove_comment_start = placement_src
        .find("// remove: every registered entity")
        .expect("remove comment must exist in placement.rs");
    let remove_block_start = placement_src
        .find("if let Some(entry) = remove_entry {")
        .expect("remove_entry block must exist in placement.rs");
    let attach_evidence_start = placement_src
        .find("// attach-evidence, type-scoped half")
        .expect("attach-evidence type-scoped comment must exist in placement.rs");
    let place_block = &placement_src[place_block_start..remove_comment_start];
    let remove_block = &placement_src[remove_block_start..attach_evidence_start];
    // `place` no longer probes the oracle at all (2026-09-07, see this
    // test's doc comment) — pin that directly, rather than banning
    // `is_withdrawn` (which the block now legitimately calls for geometry
    // membership, not stud duplication).
    assert!(
        !place_block.contains("check_preconditions") && !place_block.contains("check_preconditions"),
        "placement.rs::place_and_remove_candidates (place block): place must never probe the \
         precondition oracle at enumeration — it offers TYPES (EOP-VS-UBO-GAME-001 §3.4 R9), \
         evaluated by type geometry only; the real NotCurrentlyPlaced check runs once, later, \
         against the caller's actual id in KycWorkbook::stage()"
    );
    assert!(
        !remove_block.contains("is_withdrawn"),
        "placement.rs::place_and_remove_candidates (remove block): found a \
         hand-rolled is_withdrawn check — the board-preview side of the stud \
         duplication has come back; it must consult check_preconditions only"
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
/// (`kyc_ubo.assert.edge.nominee-piercing` is the live example).
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
        macro_fqns.contains("kyc_ubo.assert.edge.nominee-piercing"),
        "macro scan is broken — it must find the known live macro"
    );

    // KYC/UBO scope: this pack's own retirements. `bpmn.*` orphans are a
    // separate, pre-existing finding outside this tranche's fence.
    // T4 (EOP-DD-UBO-DISPATCH-001, 2026-08-28): added `kyc_ubo.` — the
    // RATIFIED 2026-08-22 four-segment prefix this filter predated, so it
    // silently let every dsl.kyc verb retired since the rename (this
    // tranche's `structure-class`) through unchecked.
    let kyc_orphans: Vec<String> = orphans
        .iter()
        .filter(|(fqn, _)| fqn.starts_with("kyc.") || fqn.starts_with("kyc_ubo.") || fqn.starts_with("ubo.") || fqn.starts_with("assert.") || fqn.starts_with("decide."))
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
    //
    // `PgKycObligationProjector`/`rebuild_obligations` REMOVED (D2.0 §5,
    // 2026-08-22, distinct from the queue removal this test otherwise
    // guards): "the outbox removal deleted the queue and drainers but left
    // the obligation projection standing... it goes with obligations."
    // `creation` — the only writer of a new `ObligationTracks` entry — is
    // dissolved, so that projector had nothing left to project.
    for kept in ["pub struct PgKycProjector", "pub async fn rebuild_control_edges"] {
        assert!(
            KYC_PROJECTION_SRC.contains(kept),
            "K-34 machinery must survive the queue removal; missing: {kept}"
        );
    }
    for gone in ["pub struct PgKycObligationProjector", "pub async fn rebuild_obligations"] {
        assert!(
            !KYC_PROJECTION_SRC.contains(gone),
            "D2.0 §5 removed the obligation projection; found it back: {gone}"
        );
    }
    // Full-replace semantics, specifically: the rebuild DELETEs the subject's
    // rows before re-inserting from the fold. Only the control-edge
    // projection survives D2.0 §5, so exactly one DELETE remains.
    assert_eq!(
        KYC_PROJECTION_SRC.matches("DELETE FROM").count(),
        1,
        "full-replace: one DELETE for the surviving control-edge projection"
    );
}

// ── Four-segment verb naming (RATIFIED 2026-08-22) ──────────────────────────
//
// `kyc_ubo.<capability>.<domain>.<intent>`. `kyc_ubo` is kept as a root so a
// future cbu/deal build gets its own namespace without collision. The intent
// segment is a NOUN, never a repeat of the capability. Capability follows PACK
// MEMBERSHIP, not spelling.

/// Every FQN this rename retired. Kept as data, not as a regex, so the gate
/// names exactly what it forbids and cannot silently narrow.
const LEGACY_FQNS: &[&str] = &[
    "ubo.edge.assert-control",
    "ubo.edge.assert-economic-interest",
    "ubo.edge.attach-evidence",
    "ubo.edge.verify",
    "ubo.edge.supersede",
    "ubo.edge.reconcile-conflict",
    "ubo.edge.pierce-nominee",
    "ubo.determination.freeze",
    "kyc.subject.register",
    "kyc.subject.classify-structure",
    "kyc.subject.assert-type",
    "kyc.subject.correct-type",
    "kyc.subject.withdraw-member",
    "kyc.subject.record-enquiry",
    "kyc.obligation.create",
    "kyc.obligation.satisfy",
    "kyc.obligation.waive",
    "assert.screening",
    "assert.identity",
    "assert.risk",
    "decide.approve",
    "decide.reject",
];

/// No legacy FQN survives in the declaring surfaces.
///
/// Source half. The search-index half is the `#[ignore]` live-DB gate below —
/// split because this one must run without a database, and the embedding-orphan
/// lesson (TS.6 Item 4) is precisely that a source-only scan passes GREEN while
/// dead names stay discoverable. Neither half alone is the gate.
#[test]
fn no_legacy_fqns_remain() {
    let sources: &[(&str, &str)] = &[
        ("dsl-kyc.yaml", DSL_KYC_YAML),
        ("dsl-kyc-obligation.yaml", DSL_KYC_OBLIGATION_YAML),
        ("kyc_dag.yaml", KYC_DAG_YAML),
        ("lexicon.rs", LEXICON_SRC),
        ("fold/control.rs", CONTROL_FOLD_SRC),
        ("kyc_stream_ops.rs", KYC_STREAM_OPS_SRC),
        ("kyc-decide/lib.rs", KYC_DECIDE_OPS_SRC),
    ];
    let mut found: Vec<String> = Vec::new();
    for (file, src) in sources {
        for (lineno, line) in src.lines().enumerate() {
            // A retirement note may NAME what it retired — only live
            // declarations count. Same carve-out as `retired_verbs_are_gone`.
            if line.trim_start().starts_with("//") || line.trim_start().starts_with('#') {
                continue;
            }
            for legacy in LEGACY_FQNS {
                if line.contains(legacy) {
                    found.push(format!("{file}:{}: `{legacy}`", lineno + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "legacy FQNs survive the four-segment rename (RATIFIED 2026-08-22): {found:#?}"
    );
}

/// Every entry in BOTH packs is exactly four dot-separated segments, rooted at
/// `kyc_ubo`, with a capability of `assert` / `assess` / `decide`.
#[test]
fn fqn_shape_is_exactly_four_segments() {
    let mut bad: Vec<String> = Vec::new();
    for fqn in assembly_lexicon()
        .entries
        .keys()
        .chain(evaluation_lexicon().entries.keys())
    {
        let seg: Vec<&str> = fqn.split('.').collect();
        if seg.len() != 4 {
            bad.push(format!("{fqn}: {} segments, want 4", seg.len()));
            continue;
        }
        if seg[0] != "kyc_ubo" {
            bad.push(format!("{fqn}: root `{}`, want `kyc_ubo`", seg[0]));
        }
        if !matches!(seg[1], "assert" | "assess" | "decide") {
            bad.push(format!("{fqn}: capability `{}` is not assert|assess|decide", seg[1]));
        }
        if seg[3].is_empty() || seg[2].is_empty() {
            bad.push(format!("{fqn}: empty domain or intent segment"));
        }
    }
    assert!(bad.is_empty(), "four-segment shape violated: {bad:#?}");
}

/// Capability follows PACK MEMBERSHIP, which is the whole TS.6 §1 ruling made
/// checkable — "a pack is a capability".
///
/// Evaluation verbs must be `decide.*`. Assembly verbs are `assert.*` EXCEPT a
/// pinned verdict set. The verdict set is data, not a rule, so widening it is a
/// visible edit to this line rather than a quiet relaxation: `freeze` commits to
/// a computed answer as the one of record — a disposition, nothing was handed to
/// us. `verify` and `reconcile-conflict` are deliberately NOT verdicts (they
/// record a human act and assert the reconciled position respectively), which
/// keeps `decide` purely Evaluation apart from this one exception.
/// `waiver` stays `assert` until D2.0 unblocks its move.
#[test]
fn capability_segment_matches_pack() {
    const ASSEMBLY_VERDICTS: &[&str] = &["kyc_ubo.decide.determination.freeze"];

    let mut bad: Vec<String> = Vec::new();
    for fqn in evaluation_lexicon().entries.keys() {
        if !fqn.starts_with("kyc_ubo.decide.") {
            bad.push(format!("Evaluation verb `{fqn}` must be `kyc_ubo.decide.*`"));
        }
    }
    for fqn in assembly_lexicon().entries.keys() {
        let is_verdict = ASSEMBLY_VERDICTS.contains(&fqn.as_str());
        if is_verdict {
            if !fqn.starts_with("kyc_ubo.decide.") {
                bad.push(format!("pinned Assembly verdict `{fqn}` must be `kyc_ubo.decide.*`"));
            }
        } else if !fqn.starts_with("kyc_ubo.assert.") {
            bad.push(format!(
                "Assembly verb `{fqn}` must be `kyc_ubo.assert.*` (it is not in the \
                 pinned verdict set {ASSEMBLY_VERDICTS:?})"
            ));
        }
    }
    // The verdict set must not name a verb that has left the Assembly pack.
    for v in ASSEMBLY_VERDICTS {
        assert!(
            assembly_lexicon().entries.contains_key(*v),
            "pinned Assembly verdict `{v}` is not in the Assembly pack"
        );
    }
    assert!(bad.is_empty(), "capability does not match pack: {bad:#?}");
}

/// The parser finding, pinned rather than remembered: `dsl_parser::parse` — the
/// governed KYC workbook path (`src/domain_ops/kyc_workbook.rs`) — accepts a
/// four-segment FQN with an underscore in its root, and preserves the dotted
/// path WHOLE rather than splitting it.
///
/// This is here because the four-segment scheme was once believed blocked: the
/// wrong parser (`dsl-core`, which takes exactly two segments) had been tested
/// for the question. `dsl-core`'s limit is a separate pre-existing finding
/// affecting 108 verbs across ten domains, tracked on its own ticket — this
/// gate deliberately asserts nothing about it.
#[test]
fn four_segment_fqn_parses() {
    for fqn in assembly_lexicon()
        .entries
        .keys()
        .chain(evaluation_lexicon().entries.keys())
    {
        let src = format!(r#"({fqn} :subject-id "00000000-0000-0000-0000-000000000000")"#);
        let (sf, diags) = dsl_parser::parse(&src);
        assert!(
            !diags.has_errors(),
            "governed authoring path must parse `{fqn}`: {diags:?}"
        );
        assert_eq!(sf.atoms.len(), 1, "`{fqn}` must parse to exactly one atom");
        assert_eq!(
            sf.atoms[0].kind, *fqn,
            "the dotted path must survive parsing whole, not be split"
        );
    }
}

/// The search-index half of `no_legacy_fqns_remain`.
///
/// Separate from the source half because that one must run without a database,
/// and this is the half that actually catches the TS.6 Item-4 defect: a
/// source-only scan goes GREEN while the retired name stays discoverable in
/// `verb_pattern_embeddings` / `verb_centroids`, so Sage can still surface and
/// bind an FQN that no longer exists.
#[tokio::test]
#[ignore = "needs DATABASE_URL"]
async fn no_legacy_fqns_remain_in_the_search_index() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect");

    let legacy: Vec<String> = LEGACY_FQNS.iter().map(|s| s.to_string()).collect();

    let patterns: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT verb_name, count(*)::bigint FROM "ob-poc".verb_pattern_embeddings
           WHERE verb_name = ANY($1) GROUP BY verb_name ORDER BY verb_name"#,
    )
    .bind(&legacy)
    .fetch_all(&pool)
    .await
    .expect("query verb_pattern_embeddings");
    assert!(
        patterns.is_empty(),
        "legacy FQNs are still discoverable in verb_pattern_embeddings — the \
         rename left search-index orphans: {patterns:#?}"
    );

    let centroids: Vec<(String,)> = sqlx::query_as(
        r#"SELECT verb_name FROM "ob-poc".verb_centroids
           WHERE verb_name = ANY($1) ORDER BY verb_name"#,
    )
    .bind(&legacy)
    .fetch_all(&pool)
    .await
    .expect("query verb_centroids");
    assert!(
        centroids.is_empty(),
        "legacy FQNs are still discoverable in verb_centroids: {centroids:#?}"
    );

    // And the positive half — the new names ARE indexed, so this gate cannot be
    // satisfied by an empty index.
    let indexed: i64 = sqlx::query_scalar(
        r#"SELECT count(DISTINCT verb_name)::bigint FROM "ob-poc".verb_pattern_embeddings
           WHERE verb_name LIKE 'kyc\_ubo.%'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("count new");
    // D2.0 §5 (2026-08-22): creation/satisfaction DISSOLVED (21→19) — gone
    // from the index entirely, not renamed. waiver's new FQN is still
    // counted once.
    // T2 (2026-08-27, §8 Q1): register+type MERGED into place (19→18, P1) —
    // gone from the index entirely, not renamed (member-withdrawal→remove
    // IS a rename, still counted once, P2). type-correction DISSOLVED
    // (18→17, P3) — gone from the index entirely, no replacement.
    // T3 (2026-08-27, §3.1-§3.4): control+economic-interest MERGED into
    // connect (17→16); reconciliation DISSOLVED, gone entirely (16→15);
    // supersession MERGED into disconnect, no net count change (rename).
    // T4-close (2026-08-28, EOP-DD-UBO-DISPATCH-001): structure-class
    // DISSOLVED, gone entirely (15→14). T5 (2026-08-28,
    // EOP-DD-UBO-PROOF-001): verification DISSOLVED, absorbed into evidence
    // — no net count change (evidence already counted). Landed at 14,
    // matching the live 14-verb lexicon (`assembly_lexicon()` +
    // `evaluation_lexicon()`); this pin was stale from T3/T4/T5 (all landed
    // before EOP-DD-UBO-CLEANOUT-001 T6) until corrected here (T6 P4,
    // 2026-09-07) — the embeddings table itself was already correctly at
    // 14, only this hardcoded assertion had drifted.
    assert_eq!(
        indexed, 14,
        "the 14 surviving verbs must be discoverable under the current scheme"
    );
}

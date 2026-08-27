//! `cargo x kyc-alignment` — read-only join of every place "what KYC/UBO
//! moves exist" is declared, built for the EOP-VS-UBO-GAME-001
//! rip-and-replace tranche plan (T0 of that plan).
//!
//! ANALYSIS ONLY. This module reads verb YAML, the real Rust-side lexicon
//! and board enumerator (not a text scan — a hand-regexed grep over
//! `lexicon.rs` during scoping missed 2 of 19 verbs, `verification` and
//! `freeze`, because their `LexiconEntry::build(` call is formatted
//! differently from the other 17), the SemOsVerbOp registry, the journey
//! pack, every domain pack's `owned_verb_prefixes`, the macro catalogue,
//! and the database (`dsl_verbs`, `verb_pattern_embeddings`,
//! `verb_centroids`). It writes nothing anywhere — no YAML, no packs, no
//! lexicon entries, no ops, no embeddings, no DB rows.
//!
//! Nineteen verbs have five independent declaration sites today and no
//! join between them (see `docs/eop/EOP-VS-UBO-GAME-001_The-UBO-Game_v0.1.md`
//! §7 and the tranche-plan review that preceded this tool). The one tool
//! that could see the gaps — `cargo x reconcile hygiene-report` — is
//! prefix-blind: `"kyc_ubo.assert…".starts_with("kyc.")` is `false` (an
//! underscore, not a dot, follows `kyc`), so all 19 verbs were unowned by
//! any domain pack and the report showed zero of them. This view exists so
//! that fact — and the other five known misalignments — are a standing,
//! re-runnable measurement instead of an ad-hoc grep.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_core::collect_declared_fqns;

use ob_poc_kyc_substrate::{
    assembly_lexicon, enumerate_placement_set, evaluation_lexicon, fold_control_versioned,
    fold_obligations_versioned, fold_type_registry, AuthorityRef, ControlState, EdgeId, EntityId,
    FoldRegistry, IntentEvent, ObligationState, Principal, SubjectId, TargetBinding,
    TypeRegistryState, V1FoldImpl,
};

use crate::reconcile::{
    collect_macro_fqns, load_catalogue, load_domain_pack_ownership, DomainPackOwnership,
};

const KYC_PREFIX: &str = "kyc_ubo.";

/// Which lexicon (if either) declares this verb — the build game
/// (`assembly_lexicon`) or the inspect game (`evaluation_lexicon`, D2.0).
/// A verdict verb (`approve`/`reject`/`waiver`) is legitimately `Eval`, not
/// board-enumerable, and that is not a bug — `enumerate_placement_set`
/// only ever takes the assembly manifest (§3 vs §4 of the target design).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexiconMembership {
    Assembly,
    Evaluation,
    None,
}

impl std::fmt::Display for LexiconMembership {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Assembly => write!(f, "assembly"),
            Self::Evaluation => write!(f, "eval"),
            Self::None => write!(f, "-"),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct AlignmentRow {
    fqn: String,
    yaml: bool,
    lexicon: Option<LexiconMembership>,
    macro_claim: bool,
    ops: bool,
    pack: bool,
    dp_prefix: Vec<String>,
    board: Option<bool>, // None = not applicable (verdict verb, not a build move)
    dsl_verbs: bool,
    lexicon_hash_nonnull: bool,
    embeddings: i64,
    centroid: bool,
}

impl AlignmentRow {
    fn new(fqn: String) -> Self {
        Self {
            fqn,
            lexicon: Some(LexiconMembership::None),
            ..Default::default()
        }
    }

    /// A row is a "declared thing" if any of the three primary declaration
    /// sites (YAML, lexicon, ops registry) knows about it.
    fn is_declared(&self) -> bool {
        self.yaml || !matches!(self.lexicon, Some(LexiconMembership::None) | None) || self.ops
    }

    /// A row is "claimed" if a consumer site (pack, domain-pack prefix,
    /// macro) references it — independent of whether it is declared.
    fn is_claimed(&self) -> bool {
        self.pack || !self.dp_prefix.is_empty() || self.macro_claim
    }
}

pub(crate) async fn run() -> Result<()> {
    println!("=====================================================");
    println!("  cargo x kyc-alignment — KYC/UBO vocabulary join");
    println!("=====================================================\n");

    let cfg = load_catalogue()?;
    let declared = collect_declared_fqns(&cfg);
    let macros = collect_macro_fqns();

    let assembly = assembly_lexicon();
    let evaluation = evaluation_lexicon();

    let mut registry = sem_os_postgres::ops::build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);
    let registered: HashSet<String> = registry.manifest().into_iter().collect();

    let packs_dir = std::path::PathBuf::from("config/packs");
    let loaded_packs = dsl_core::load_packs_from_dir(&packs_dir)
        .context("loading config/packs/ for kyc-alignment")?;
    let kyc_case_pack_verbs: HashSet<String> = loaded_packs
        .get("kyc-case")
        .map(|p| p.allowed_verbs.iter().cloned().collect())
        .unwrap_or_default();

    let dp_ownership = load_domain_pack_ownership()?;

    let board = board_enumerable_fqns()?;

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    let pool = PgPool::connect(&database_url)
        .await
        .context("connecting to DATABASE_URL for kyc-alignment DB columns")?;
    let db = DbSnapshot::load(&pool).await?;

    // ── Universe: union of every FQN mentioned by any site ─────────────────
    let mut universe: BTreeSet<String> = BTreeSet::new();
    universe.extend(
        declared
            .iter()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );
    universe.extend(
        assembly
            .entries
            .keys()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );
    universe.extend(
        evaluation
            .entries
            .keys()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );
    universe.extend(
        registered
            .iter()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );
    universe.extend(
        kyc_case_pack_verbs
            .iter()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );
    universe.extend(macros.iter().filter(|f| f.starts_with(KYC_PREFIX)).cloned());
    universe.extend(
        db.dsl_verbs
            .keys()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );
    universe.extend(
        db.embeddings
            .keys()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );
    universe.extend(
        db.centroids
            .iter()
            .filter(|f| f.starts_with(KYC_PREFIX))
            .cloned(),
    );

    let mut rows: BTreeMap<String, AlignmentRow> = BTreeMap::new();
    for fqn in &universe {
        let mut row = AlignmentRow::new(fqn.clone());
        row.yaml = declared.contains(fqn);
        row.lexicon = Some(if assembly.entries.contains_key(fqn) {
            LexiconMembership::Assembly
        } else if evaluation.entries.contains_key(fqn) {
            LexiconMembership::Evaluation
        } else {
            LexiconMembership::None
        });
        row.macro_claim = macros.contains(fqn);
        row.ops = registered.contains(fqn);
        row.pack = kyc_case_pack_verbs.contains(fqn);
        row.dp_prefix = dp_ownership
            .iter()
            .filter(|p| {
                p.owned_verb_prefixes
                    .iter()
                    .any(|pre| fqn.starts_with(pre.as_str()))
            })
            .map(|p| p.pack_id.clone())
            .collect();
        row.board = if matches!(row.lexicon, Some(LexiconMembership::Assembly)) {
            Some(board.contains(fqn))
        } else {
            None
        };
        row.dsl_verbs = db.dsl_verbs.contains_key(fqn);
        row.lexicon_hash_nonnull = db.dsl_verbs.get(fqn).copied().unwrap_or(false);
        row.embeddings = db.embeddings.get(fqn).copied().unwrap_or(0);
        row.centroid = db.centroids.contains(fqn);
        rows.insert(fqn.clone(), row);
    }

    print_table(&rows);
    print_inverse_findings(&rows);
    run_validations(&rows, &dp_ownership);

    println!("\nReport-only: this command exits zero by design; nothing was written.");
    Ok(())
}

struct DbSnapshot {
    /// full_name -> lexicon_hash IS NOT NULL
    dsl_verbs: BTreeMap<String, bool>,
    /// verb_name -> embedding row count
    embeddings: BTreeMap<String, i64>,
    centroids: BTreeSet<String>,
}

impl DbSnapshot {
    async fn load(pool: &PgPool) -> Result<Self> {
        let dsl_verbs_rows: Vec<(String, Option<String>)> = sqlx::query_as(
            r#"SELECT full_name, lexicon_hash FROM "ob-poc".dsl_verbs WHERE full_name LIKE 'kyc\_ubo.%'"#,
        )
        .fetch_all(pool)
        .await
        .context("querying ob-poc.dsl_verbs")?;
        let dsl_verbs = dsl_verbs_rows
            .into_iter()
            .map(|(fqn, hash)| (fqn, hash.is_some()))
            .collect();

        let embedding_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT verb_name, count(*) FROM "ob-poc".verb_pattern_embeddings
               WHERE verb_name LIKE 'kyc\_ubo.%' GROUP BY 1"#,
        )
        .fetch_all(pool)
        .await
        .context("querying ob-poc.verb_pattern_embeddings")?;
        let embeddings = embedding_rows.into_iter().collect();

        let centroid_rows: Vec<(String,)> = sqlx::query_as(
            r#"SELECT verb_name FROM "ob-poc".verb_centroids WHERE verb_name LIKE 'kyc\_ubo.%'"#,
        )
        .fetch_all(pool)
        .await
        .context("querying ob-poc.verb_centroids")?;
        let centroids = centroid_rows.into_iter().map(|(v,)| v).collect();

        Ok(Self {
            dsl_verbs,
            embeddings,
            centroids,
        })
    }
}

/// Board-enumerable FQNs — the union of `enumerate_placement_set`'s output
/// across TWO synthetic board states, folded in-memory (no DB, no write
/// path): an empty board, and a board pushed as deep as the live
/// precondition chain allows — registered+typed entities, an evidenced and
/// verified control edge, a supported structure class, and a reconciliation
/// marker (unlocking `verification`'s `EvidenceCited` and `freeze`'s
/// `ReconciledProjection`/`StructureClassSupported`). This mirrors exactly
/// what the real board machinery does; it does not reimplement it.
///
/// `kyc_ubo.assert.entity.{identity,screening,risk}` never appear in either
/// state, and no third state would change that: their sole precondition is
/// `ObligationExists`, and D2.0 §5 dissolved `assert.obligation.creation` —
/// the only verb that ever created an `ObligationTracks` entry. The
/// substrate's own comment names this exactly
/// (`fold/obligation.rs`, near the dissolved-verb note): "with no path left
/// to satisfy it, those three verbs are now permanently refused at append
/// time." So `board=N` for those three is not a probe limitation — it is a
/// confirmed, permanent unreachability in the live vocabulary, surfaced
/// here rather than hidden by a deeper probe that could never exist.
fn board_enumerable_fqns() -> Result<HashSet<String>> {
    let lexicon = assembly_lexicon();
    let mut registry = FoldRegistry::new();
    registry.register(lexicon.hash, Arc::new(V1FoldImpl));

    let subject = SubjectId(Uuid::new_v4());
    let as_of = Utc::now();
    let mut found = HashSet::new();

    // State 1: empty board.
    {
        let control = ControlState::default();
        let obligation = ObligationState::default();
        let type_registry = TypeRegistryState::default();
        let set = enumerate_placement_set(subject, &control, &obligation, &type_registry, &lexicon);
        found.extend(set.moves.iter().map(|m| m.verb_fqn.as_str().to_string()));
    }

    // State 2: pushed as deep as the live precondition chain allows.
    {
        let person = EntityId(Uuid::new_v4());
        let company = EntityId(Uuid::new_v4());
        let edge = Uuid::new_v4();
        let mk = |verb: &str, target: TargetBinding, payload: serde_json::Value| {
            IntentEvent::new(
                subject,
                verb,
                Principal::test_analyst(),
                AuthorityRef("kyc-alignment.probe".into()),
                target,
                payload,
                as_of,
            )
            .with_lexicon_hash(lexicon.hash)
        };
        let subj_target = TargetBinding::for_subject(subject);
        let events = vec![
            mk(
                "kyc_ubo.assert.subject.register",
                subj_target.clone(),
                serde_json::json!({"entity_id": person.0}),
            ),
            mk(
                "kyc_ubo.assert.subject.register",
                subj_target.clone(),
                serde_json::json!({"entity_id": company.0}),
            ),
            mk(
                "kyc_ubo.assert.subject.type",
                subj_target.clone(),
                serde_json::json!({"entity_id": person.0, "entity_type": "natural_person"}),
            ),
            mk(
                "kyc_ubo.assert.subject.type",
                subj_target.clone(),
                serde_json::json!({"entity_id": company.0, "entity_type": "private_limited_company"}),
            ),
            mk(
                "kyc_ubo.assert.edge.control",
                subj_target.clone(),
                serde_json::json!({
                    "edge_id": edge,
                    "from_entity_id": person.0,
                    "to_entity_id": company.0,
                    "kind": "board_appointment",
                }),
            ),
            // Edge-scoped: target carries the edge id, not the payload
            // (`edge_id_from_target` reads `event.target.edge_id` — see
            // `fold/control.rs`'s evidence/verify arms).
            mk(
                "kyc_ubo.assert.edge.evidence",
                TargetBinding::for_edge(subject, EdgeId(edge)),
                serde_json::json!({}),
            ),
            mk(
                "kyc_ubo.assert.edge.verification",
                TargetBinding::for_edge(subject, EdgeId(edge)),
                serde_json::json!({}),
            ),
            mk(
                "kyc_ubo.assert.subject.structure-class",
                subj_target.clone(),
                serde_json::json!({"entity_id": company.0, "structure_class": "private_company"}),
            ),
            mk(
                "kyc_ubo.assert.edge.reconciliation",
                subj_target.clone(),
                serde_json::json!({}),
            ),
        ];
        let refs: Vec<&IntentEvent> = events.iter().collect();
        let control = fold_control_versioned(&refs, &registry)
            .context("folding synthetic control state for board enumeration")?;
        let obligation = fold_obligations_versioned(&refs, &registry)
            .context("folding synthetic obligation state for board enumeration")?;
        let type_registry = fold_type_registry(&refs);
        let set = enumerate_placement_set(subject, &control, &obligation, &type_registry, &lexicon);
        found.extend(set.moves.iter().map(|m| m.verb_fqn.as_str().to_string()));
    }

    Ok(found)
}

fn yn(b: bool) -> &'static str {
    if b {
        "Y"
    } else {
        "."
    }
}

fn print_table(rows: &BTreeMap<String, AlignmentRow>) {
    println!(
        "{:<44} {:>4} {:>9} {:>5} {:>3} {:>4} {:>9} {:>6} {:>3} {:>4} {:>4} {:>3}",
        "FQN",
        "yaml",
        "lexicon",
        "macro",
        "ops",
        "pack",
        "dp_pfx",
        "board",
        "dsl",
        "hash",
        "embed",
        "cent"
    );
    println!("{}", "-".repeat(140));
    for row in rows.values() {
        let board_s = match row.board {
            Some(true) => "Y".to_string(),
            Some(false) => "N".to_string(),
            None => "-".to_string(),
        };
        println!(
            "{:<44} {:>4} {:>9} {:>5} {:>3} {:>4} {:>9} {:>6} {:>3} {:>4} {:>4} {:>3}",
            row.fqn.trim_start_matches(KYC_PREFIX),
            yn(row.yaml),
            row.lexicon.unwrap_or(LexiconMembership::None),
            yn(row.macro_claim),
            yn(row.ops),
            yn(row.pack),
            if row.dp_prefix.is_empty() {
                ".".to_string()
            } else {
                row.dp_prefix.join(",")
            },
            board_s,
            yn(row.dsl_verbs),
            yn(row.lexicon_hash_nonnull),
            row.embeddings,
            yn(row.centroid),
        );
    }
    println!("\nTOTAL rows: {}", rows.len());
}

/// The inverse direction: anything CLAIMED (pack / domain-pack prefix /
/// macro) that does not exist in ANY declaration site (YAML, lexicon,
/// ops). This is the `nominee-piercing` shape — a live macro and a live
/// pack entry pointing at a verb that was retired everywhere else.
fn print_inverse_findings(rows: &BTreeMap<String, AlignmentRow>) {
    println!("\nClaimed by a consumer site but declared nowhere (yaml/lexicon/ops all absent):");
    let mut found = false;
    for row in rows.values() {
        if row.is_claimed() && !row.is_declared() {
            found = true;
            let mut claimants = Vec::new();
            if row.pack {
                claimants.push("pack".to_string());
            }
            if !row.dp_prefix.is_empty() {
                claimants.push(format!("domain-pack-prefix({})", row.dp_prefix.join(",")));
            }
            if row.macro_claim {
                claimants.push("macro".to_string());
            }
            println!("  - {}: claimed by {}", row.fqn, claimants.join(", "));
        }
    }
    if !found {
        println!("  none");
    }
}

fn run_validations(rows: &BTreeMap<String, AlignmentRow>, dp_ownership: &[DomainPackOwnership]) {
    println!("\n=====================================================");
    println!("  Six known misalignments — must fall out, unforced");
    println!("=====================================================");

    // 1. 19/19 verbs unowned by any domain pack.
    let total = rows.len();
    let unowned = rows.values().filter(|r| r.dp_prefix.is_empty()).count();
    let v1 = unowned == total && total > 0;
    println!(
        "\n[{}] V1 — all verbs unowned by any domain pack ({}/{} unowned)",
        if v1 { "PASS" } else { "FAIL" },
        unowned,
        total
    );

    // 2. 4 board moves in no journey pack.
    let expect_no_pack = [
        "kyc_ubo.assert.subject.type",
        "kyc_ubo.assert.subject.type-correction",
        "kyc_ubo.assert.subject.member-withdrawal",
        "kyc_ubo.assert.subject.enquiry",
    ];
    let mut v2_detail = Vec::new();
    let mut v2 = true;
    for fqn in expect_no_pack {
        match rows.get(fqn) {
            Some(r) if !r.pack => v2_detail.push(format!("{fqn}: pack=N (expected)")),
            Some(_) => {
                v2 = false;
                v2_detail.push(format!("{fqn}: pack=Y (UNEXPECTED — should be absent)"));
            }
            None => {
                v2 = false;
                v2_detail.push(format!("{fqn}: not in universe at all"));
            }
        }
    }
    println!(
        "\n[{}] V2 — 4 board moves absent from the journey pack",
        if v2 { "PASS" } else { "FAIL" }
    );
    for d in &v2_detail {
        println!("       {d}");
    }

    // 3. nominee-piercing claimed by kyc-case.yaml, retired as a verb.
    let fqn = "kyc_ubo.assert.edge.nominee-piercing";
    let v3 = match rows.get(fqn) {
        Some(r) => {
            r.pack
                && r.macro_claim
                && !r.yaml
                && !r.ops
                && matches!(r.lexicon, Some(LexiconMembership::None))
        }
        None => false,
    };
    println!(
        "\n[{}] V3 — nominee-piercing: pack claims it, no yaml/lexicon/ops verb exists",
        if v3 { "PASS" } else { "FAIL" }
    );
    if let Some(r) = rows.get(fqn) {
        println!(
            "       pack={} macro={} yaml={} ops={} lexicon={}",
            yn(r.pack),
            yn(r.macro_claim),
            yn(r.yaml),
            yn(r.ops),
            r.lexicon.unwrap_or(LexiconMembership::None)
        );
    } else {
        println!("       {fqn} not found in universe at all");
    }

    // 4. 3 verbs with NULL lexicon_hash in dsl_verbs: waiver, approve, reject.
    let expect_null_hash = [
        "kyc_ubo.decide.obligation.waiver",
        "kyc_ubo.decide.subject.approve",
        "kyc_ubo.decide.subject.reject",
    ];
    let mut v4 = true;
    let mut v4_detail = Vec::new();
    for fqn in expect_null_hash {
        match rows.get(fqn) {
            Some(r) if r.dsl_verbs && !r.lexicon_hash_nonnull => {
                v4_detail.push(format!(
                    "{fqn}: dsl_verbs row present, lexicon_hash NULL (expected)"
                ));
            }
            Some(r) => {
                v4 = false;
                v4_detail.push(format!(
                    "{fqn}: dsl_verbs={} lexicon_hash_nonnull={} (UNEXPECTED)",
                    yn(r.dsl_verbs),
                    yn(r.lexicon_hash_nonnull)
                ));
            }
            None => {
                v4 = false;
                v4_detail.push(format!("{fqn}: not in universe at all"));
            }
        }
    }
    println!(
        "\n[{}] V4 — 3 verdict verbs have NULL lexicon_hash in dsl_verbs",
        if v4 { "PASS" } else { "FAIL" }
    );
    for d in &v4_detail {
        println!("       {d}");
    }

    // 5. lexicon/pack count mismatch (16 vs 16 with different membership).
    let lexicon_assembly_fqns: BTreeSet<&str> = rows
        .values()
        .filter(|r| matches!(r.lexicon, Some(LexiconMembership::Assembly)))
        .map(|r| r.fqn.as_str())
        .collect();
    let pack_fqns: BTreeSet<&str> = rows
        .values()
        .filter(|r| r.pack)
        .map(|r| r.fqn.as_str())
        .collect();
    let same_count = lexicon_assembly_fqns.len() == pack_fqns.len();
    let same_membership = lexicon_assembly_fqns == pack_fqns;
    let v5 = same_count && !same_membership;
    println!(
        "\n[{}] V5 — lexicon(assembly) and pack have equal counts ({} vs {}) but different membership",
        if v5 { "PASS" } else { "FAIL" },
        lexicon_assembly_fqns.len(),
        pack_fqns.len()
    );
    let only_lexicon: Vec<&str> = lexicon_assembly_fqns
        .difference(&pack_fqns)
        .copied()
        .collect();
    let only_pack: Vec<&str> = pack_fqns
        .difference(&lexicon_assembly_fqns)
        .copied()
        .collect();
    println!("       in lexicon(assembly) but not pack: {only_lexicon:?}");
    println!("       in pack but not lexicon(assembly): {only_pack:?}");

    // 6. owned_verb_prefixes lists three-segment prefixes while comments
    //    beside them name four-segment FQNs (a source-text check, since
    //    this is specifically about the declared DATA vs the comments
    //    sitting next to it, not something the row join can see).
    let kyc_pack = dp_ownership.iter().find(|p| p.pack_id == "ob-poc.kyc");
    let v6 = match kyc_pack {
        Some(p) => {
            let three_segment = ["kyc.", "ubo.", "decide.", "assert."];
            three_segment
                .iter()
                .all(|pre| p.owned_verb_prefixes.iter().any(|x| x == pre))
                && !p.owned_verb_prefixes.iter().any(|x| x == KYC_PREFIX)
        }
        None => false,
    };
    println!(
        "\n[{}] V6 — ob-poc.kyc declares three-segment prefixes {{kyc.,ubo.,decide.,assert.}}, none matches kyc_ubo.*",
        if v6 { "PASS" } else { "FAIL" }
    );
    if let Some(p) = kyc_pack {
        println!("       owned_verb_prefixes: {:?}", p.owned_verb_prefixes);
    } else {
        println!("       ob-poc.kyc domain pack not found");
    }

    let all_pass = v1 && v2 && v3 && v4 && v5 && v6;
    println!(
        "\n=====================================================\n  {}\n=====================================================",
        if all_pass {
            "ALL SIX VALIDATIONS PASS — the view reproduces every known misalignment unforced"
        } else {
            "AT LEAST ONE VALIDATION FAILED — the view is wrong, per its own spec"
        }
    );
}

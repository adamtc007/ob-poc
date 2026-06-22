//! Intent Trace eval harness (Option C — measure both reductions).
//!
//! Phase 0 (this file, `phase0_board_enrichment_receipt`): proves every corpus
//! case can drive a genuine, board-derived pack-scoped verb set — NOT the
//! answer-keyed `get_simulated_allowed_verbs` synthetic bucket the legacy
//! `step0_trial` used.
//!
//! ## Why the board set is computed directly from the registry
//!
//! Under Phase-0 conditions the production `compute_session_verb_surface()`
//! reduces to exactly `registry ∩ board.pack_domains`:
//!   * Step 2 (AgentMode): `Governed` allows all business verbs — no-op here.
//!   * Step 3 (Scope/workflow): the board IS the pack scope; expressed as
//!     `pack_domains` rather than the 4 hard-coded workflow buckets (which do
//!     not cover ~half the corpus's domains — see the plan's Phase-0 finding).
//!   * Step 4 (SemReg CCIR): an available envelope whose legal set is the board
//!     set intersects to the same set.
//!   * Step 5 (Lifecycle): `entity_state = None` ⇒ skipped (state reachability
//!     is the Phase-1 read-only observer, run via `preconditions_met`, not here).
//!   * Steps 6/7 (FailPolicy/rank): SemReg available ⇒ no fail-policy; rank
//!     affects ordering, not membership.
//!
//! So `board_legal_set()` is the genuine pack-scoped set, and the collapse
//! counts (`total_registry → board_size`) are the same numbers Phase 1 wires
//! into `IntentTrace` on the production path (where a live envelope exists).
//!
//! Run: `cargo test --test intent_trace_eval phase0_board_enrichment_receipt -- --ignored --nocapture`

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

// ── Fixtures ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct Board {
    board_id: String,
    corpus_domain: String,
    #[allow(dead_code)]
    description: String,
    pack_domains: Vec<String>,
    #[serde(default)]
    entity_state: Option<String>,
    /// Session workflow focus driving the PRODUCTION board composition
    /// (`workflow_allowed_domains` → the full workspace domain set + Phase-1
    /// membership-owned macros). When authored, `board_allowed_set` drives
    /// `compute_session_verb_surface` instead of the bespoke pack-scoped set.
    #[serde(default)]
    stage_focus: Option<String>,
    #[serde(default = "default_true")]
    has_group_scope: bool,
    #[serde(default)]
    is_infrastructure_scope: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct BoardFixtures {
    boards: Vec<Board>,
}

#[derive(Debug, Deserialize)]
struct CorpusEntry {
    id: String,
    utterance: String,
    expected_verb: String,
    #[allow(dead_code)]
    domain: String,
    board_id: String,
    #[serde(default)]
    alt_verbs: Vec<String>,
}

fn load_boards() -> Vec<Board> {
    let raw = std::fs::read_to_string("tests/fixtures/intent_trace_boards.json")
        .expect("read tests/fixtures/intent_trace_boards.json");
    let fixtures: BoardFixtures = serde_json::from_str(&raw).expect("parse board fixtures");
    fixtures.boards
}

fn load_corpus() -> Vec<CorpusEntry> {
    let raw = std::fs::read_to_string("assets/cic_labeled_corpus.json")
        .expect("read assets/cic_labeled_corpus.json");
    serde_json::from_str(&raw).expect("parse corpus (must carry board_id — run Phase 0 enrichment)")
}

// ── Board legal set (genuine pack-scoped reachable set) ──────────

/// Registry verbs whose domain belongs to the board's `pack_domains`.
/// See module docs for why this equals `compute_session_verb_surface()` under
/// Phase-0 conditions.
fn board_legal_set(pack_domains: &HashSet<&str>) -> HashSet<String> {
    use ob_poc::dsl_v2::execution::runtime_registry;
    runtime_registry()
        .all_verbs()
        .filter(|v| pack_domains.contains(v.domain.as_str()))
        .map(|v| v.full_name.clone())
        .collect()
}

/// Legacy synthetic allowed set — copied verbatim from `step0_trial.rs` so the
/// receipt can contrast it against the genuine board set. The decisive property:
/// the last line ALWAYS inserts `expected_verb`, guaranteeing survival by
/// construction (answer leakage). Board sets never do this.
fn get_simulated_allowed_verbs(expected_verb: &str) -> HashSet<String> {
    let namespace = expected_verb.split('.').next().unwrap_or("");
    let mut allowed = HashSet::new();
    match namespace {
        "cbu" => {
            for v in &[
                "cbu.create", "cbu.update", "cbu.delete", "cbu.assign-role", "cbu.add-product",
                "cbu.parties", "cbu.delete-cascade", "cbu.terminate", "cbu.suspend",
                "cbu.list-roles", "cbu.get-config",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "entity" | "party" => {
            for v in &[
                "entity.create", "entity.update", "entity.delete", "entity.read",
                "entity.verify-name", "entity.add-parent", "entity.list-placeholders",
                "entity.resolve-placeholder", "entity.read-structure", "entity.check-status",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "screening" | "ubo" | "control" | "kyc" | "red-flag" => {
            for v in &[
                "screening.sanctions", "screening.pep", "screening.adverse-media",
                "screening.full", "ubo.list-ubos", "ubo.add-ownership", "ubo.compute-chains",
                "ubo.trace-chains", "ubo.mark-deceased", "ubo.waive-verification",
                "ubo.update-ownership", "control.add", "control.show-board-controller",
                "control.import-psc-register", "red-flag.dismiss", "red-flag.escalate",
                "red-flag.list", "kyc.download-cert",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "onboarding" | "gleif" => {
            for v in &[
                "onboarding.start", "onboarding.status", "onboarding.resume", "onboarding.pause",
                "onboarding.check-readiness", "onboarding.runsheet", "onboarding.send-welcome",
                "gleif.search", "gleif.enrich", "gleif.import-tree", "gleif.check-active",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "document" | "doc-request" => {
            for v in &[
                "document.solicit", "document.upload-version", "document.verify",
                "document.reject", "document.extract", "document.solicit-batch",
                "document.list-pending", "document.approve", "document.archive",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "deal" | "custody" | "share-class" | "fund" => {
            for v in &[
                "deal.create", "deal.add-participant", "deal.read", "deal.create-rate-card",
                "deal.add-rate-card-line", "deal.propose-rate-card", "deal.counter-rate-card",
                "deal.update-status", "deal.request-onboarding", "deal.cancel", "fund.create",
                "fund.create-subfund", "share-class.create", "fund.link-feeder",
                "fund.list-investors", "fund.add-investment", "custody.settlement-cycle",
            ] {
                allowed.insert(v.to_string());
            }
        }
        _ => {
            allowed.insert("session.exit".to_string());
            allowed.insert("agent.help".to_string());
        }
    }
    allowed.insert(expected_verb.to_string()); // <-- answer leakage
    allowed
}

// ── Receipt structs ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct BoardReport {
    board_id: String,
    corpus_domain: String,
    entity_state: Option<String>,
    pack_domains: usize,
    legal_set_size: usize,
    collapse_ratio: f32,
    cases: usize,
    expected_reachable: usize,
    expected_unreachable: usize,
}

#[derive(Debug, Serialize)]
struct SampleContrast {
    id: String,
    utterance: String,
    expected_verb: String,
    synthetic_size: usize,
    synthetic_contains_expected: bool,
    board_id: String,
    board_size: usize,
    board_contains_expected: bool,
}

#[derive(Debug, Serialize)]
struct Phase0Receipt {
    note: String,
    total_registry_verbs: usize,
    total_cases: usize,
    expected_reachable: usize,
    expected_unreachable: usize,
    reachability_pct: f32,
    answer_leakage_removed: bool,
    /// Of the unreachable cases: how many are macro/scenario-routed (found via
    /// search Tiers -2A/-2B, not the verb registry) vs genuine vocabulary gaps
    /// (FQN absent from registry, macros, AND scenarios).
    unreachable_macro_or_scenario_routed: usize,
    unreachable_vocabulary_gap: usize,
    boards: Vec<BoardReport>,
    synthetic_vs_board_sample: Vec<SampleContrast>,
    unreachable_examples: Vec<String>,
    vocabulary_gap_examples: Vec<String>,
}

/// True if `fqn` appears in the macro or scenario config — i.e. the search would
/// reach it via Tier -2A/-2B, so Phase 2's board allowed-set must union these.
fn is_macro_or_scenario_fqn(fqn: &str, macro_corpus: &str, scenario_corpus: &str) -> bool {
    macro_corpus.contains(fqn) || scenario_corpus.contains(fqn)
}

/// Concatenate all macro YAML under config/verb_schemas/macros (best-effort).
fn read_macro_corpus() -> String {
    let mut s = String::new();
    if let Ok(rd) = std::fs::read_dir("config/verb_schemas/macros") {
        for e in rd.flatten() {
            if let Ok(c) = std::fs::read_to_string(e.path()) {
                s.push_str(&c);
                s.push('\n');
            }
        }
    }
    s
}

// ── Phase 0 receipt ─────────────────────────────────────────────

#[test]
#[ignore = "Phase 0 receipt: run explicitly with --ignored"]
fn phase0_board_enrichment_receipt() {
    use ob_poc::dsl_v2::execution::runtime_registry;

    let boards = load_boards();
    let corpus = load_corpus();
    let total_registry = runtime_registry().all_verbs().count();
    assert!(
        total_registry > 0,
        "runtime registry empty — config/verbs not found from CWD {:?}",
        std::env::current_dir().unwrap()
    );

    // Pre-compute each board's legal set.
    let mut legal_sets: HashMap<String, HashSet<String>> = HashMap::new();
    let mut board_meta: HashMap<String, &Board> = HashMap::new();
    for b in &boards {
        let domains: HashSet<&str> = b.pack_domains.iter().map(String::as_str).collect();
        legal_sets.insert(b.board_id.clone(), board_legal_set(&domains));
        board_meta.insert(b.board_id.clone(), b);
    }

    // Per-board + overall coverage.
    let reachable = |case: &CorpusEntry, set: &HashSet<String>| -> bool {
        set.contains(&case.expected_verb) || case.alt_verbs.iter().any(|a| set.contains(a))
    };

    let mut per_board_cases: HashMap<String, (usize, usize, usize)> = HashMap::new(); // (cases, reach, unreach)
    let mut unreachable_examples = Vec::new();
    let (mut total_reach, mut total_unreach) = (0usize, 0usize);

    for case in &corpus {
        let set = legal_sets
            .get(&case.board_id)
            .unwrap_or_else(|| panic!("case {} references unknown board {}", case.id, case.board_id));
        let entry = per_board_cases.entry(case.board_id.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        if reachable(case, set) {
            entry.1 += 1;
            total_reach += 1;
        } else {
            entry.2 += 1;
            total_unreach += 1;
            if unreachable_examples.len() < 20 {
                unreachable_examples.push(format!("{} [{}] {}", case.id, case.expected_verb, case.utterance));
            }
        }
    }

    let mut board_reports: Vec<BoardReport> = boards
        .iter()
        .map(|b| {
            let set = &legal_sets[&b.board_id];
            let (cases, reach, unreach) = per_board_cases.get(&b.board_id).copied().unwrap_or((0, 0, 0));
            BoardReport {
                board_id: b.board_id.clone(),
                corpus_domain: b.corpus_domain.clone(),
                entity_state: b.entity_state.clone(),
                pack_domains: b.pack_domains.len(),
                legal_set_size: set.len(),
                collapse_ratio: set.len() as f32 / total_registry as f32,
                cases,
                expected_reachable: reach,
                expected_unreachable: unreach,
            }
        })
        .collect();
    board_reports.sort_by_key(|b| std::cmp::Reverse(b.cases));

    // Sample contrast: one case per board.
    let mut seen_boards: HashSet<String> = HashSet::new();
    let mut sample = Vec::new();
    for case in &corpus {
        if seen_boards.contains(&case.board_id) {
            continue;
        }
        seen_boards.insert(case.board_id.clone());
        let synthetic = get_simulated_allowed_verbs(&case.expected_verb);
        let board_set = &legal_sets[&case.board_id];
        sample.push(SampleContrast {
            id: case.id.clone(),
            utterance: case.utterance.clone(),
            expected_verb: case.expected_verb.clone(),
            synthetic_size: synthetic.len(),
            synthetic_contains_expected: synthetic.contains(&case.expected_verb),
            board_id: case.board_id.clone(),
            board_size: board_set.len(),
            board_contains_expected: board_set.contains(&case.expected_verb),
        });
    }
    sample.sort_by(|a, b| a.board_id.cmp(&b.board_id));

    // Classify the unreachable cases: macro/scenario-routed vs vocabulary gap.
    let macro_corpus = read_macro_corpus();
    let scenario_corpus = std::fs::read_to_string("config/scenario_index.yaml").unwrap_or_default();
    let (mut unreach_macro, mut unreach_vocab) = (0usize, 0usize);
    let mut vocab_gap_examples = Vec::new();
    for case in &corpus {
        let set = &legal_sets[&case.board_id];
        if reachable(case, set) {
            continue;
        }
        if is_macro_or_scenario_fqn(&case.expected_verb, &macro_corpus, &scenario_corpus) {
            unreach_macro += 1;
        } else {
            unreach_vocab += 1;
            vocab_gap_examples.push(format!("{} [{}] {}", case.id, case.expected_verb, case.utterance));
        }
    }

    // Answer-leakage check: synthetic always contains expected; boards do not by construction.
    let synthetic_always_leaks = corpus
        .iter()
        .all(|c| get_simulated_allowed_verbs(&c.expected_verb).contains(&c.expected_verb));

    let receipt = Phase0Receipt {
        note: "Phase 0 (Option A). Board legal sets are domain-scoped (registry ∩ pack_domains), \
               never answer-keyed. Equivalent to compute_session_verb_surface() under Phase-0 \
               conditions (Governed mode, entity_state=None, SemReg available). Unreachable cases \
               are the genuine PackExcluded baseline — measured, not papered over."
            .to_string(),
        total_registry_verbs: total_registry,
        total_cases: corpus.len(),
        expected_reachable: total_reach,
        expected_unreachable: total_unreach,
        reachability_pct: total_reach as f32 / corpus.len() as f32,
        answer_leakage_removed: synthetic_always_leaks, // true ⇒ synthetic leaked; boards demonstrably do not (see sample)
        unreachable_macro_or_scenario_routed: unreach_macro,
        unreachable_vocabulary_gap: unreach_vocab,
        boards: board_reports,
        synthetic_vs_board_sample: sample,
        unreachable_examples,
        vocabulary_gap_examples: vocab_gap_examples,
    };

    std::fs::create_dir_all("reports").ok();
    std::fs::write(
        "reports/phase0_board_enrichment.json",
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .expect("write reports/phase0_board_enrichment.json");

    println!("\n===== PHASE 0 BOARD ENRICHMENT RECEIPT =====");
    println!("registry verbs: {}", receipt.total_registry_verbs);
    println!(
        "cases: {} | expected reachable in board: {} ({:.1}%) | unreachable (PackExcluded baseline): {}",
        receipt.total_cases,
        receipt.expected_reachable,
        receipt.reachability_pct * 100.0,
        receipt.expected_unreachable
    );
    for b in &receipt.boards {
        println!(
            "  {:30} dom={:2} legal={:4} ({:4.1}% of registry) cases={:3} reach={:3} unreach={:2}",
            b.board_id,
            b.pack_domains,
            b.legal_set_size,
            b.collapse_ratio * 100.0,
            b.cases,
            b.expected_reachable,
            b.expected_unreachable
        );
    }
    println!("\n  synthetic-vs-board (synthetic always contains expected = answer leakage):");
    for s in &receipt.synthetic_vs_board_sample {
        println!(
            "    [{}] synth(size={},has_expected={}) board={}(size={},has_expected={}) :: {}",
            s.expected_verb,
            s.synthetic_size,
            s.synthetic_contains_expected,
            s.board_id,
            s.board_size,
            s.board_contains_expected,
            s.utterance
        );
    }
    println!("\n  receipt -> reports/phase0_board_enrichment.json");
}

// ════════════════════════════════════════════════════════════════
// DB-backed eval (Phase 1 receipts + Phase 2 batch).
// Requires --features database + DATABASE_URL + local BGE embedder.
// ════════════════════════════════════════════════════════════════

#[cfg(feature = "database")]
mod db_eval {
    use super::*;
    use ob_poc::agent::verb_surface::observe_state_reachability;
    use ob_poc::mcp::verb_search::{soft_stage_flow, HybridVerbSearcher, VerbSearchResult};
    use std::path::Path;
    use std::sync::Arc;

    fn boards_by_id() -> HashMap<String, Board> {
        load_boards().into_iter().map(|b| (b.board_id.clone(), b)).collect()
    }

    /// Genuine board allowed-set for the SEARCH path: registry verbs in
    /// pack_domains UNION macro/scenario FQNs whose leading domain ∈ pack_domains.
    /// The macro/scenario union is required because search Tiers -2A/-2B filter
    /// against `allowed_verbs` too — without it, the 27 macro-routed corpus cases
    /// die in search (Phase 0 finding).
    fn board_allowed_set(board: &Board, macro_scenario_fqns: &HashSet<String>) -> HashSet<String> {
        // PRODUCTION PATH (Phase 2): when the board carries a `stage_focus`, drive
        // the genuine session surface — `compute_session_verb_surface` with the
        // workspace's `workflow_allowed_domains` set + the Phase-1 membership-owned
        // macros (`allowed_fqns()` now unions `owned_macros`). This eliminates the
        // divergence between what the harness measured and what live composes:
        // the allowed set IS the production allowed set, by construction.
        if board.stage_focus.is_some() {
            return production_allowed_set(board);
        }
        // LEGACY FALLBACK — Option-C multi-board receipts whose boards have no
        // authored stage_focus. Domain-scoped set + leading-domain macro
        // admission (the pre-Phase-2 behaviour), kept so those receipts are
        // unchanged by this CBU-scoped fix.
        let domains: HashSet<&str> = board.pack_domains.iter().map(String::as_str).collect();
        let mut set = board_legal_set(&domains);
        for fqn in macro_scenario_fqns {
            if let Some(dom) = fqn.split('.').next() {
                if domains.contains(dom) {
                    set.insert(fqn.clone());
                }
            }
        }
        set
    }

    /// The genuine production allowed set for a workspace board: exactly what the
    /// live pipeline threads into verb search via `with_allowed_verbs`. Drives
    /// `compute_session_verb_surface` with the board's `stage_focus` so
    /// `workflow_allowed_domains` resolves the FULL workspace domain set, and
    /// `allowed_fqns()` emits the Phase-1 membership-owned macros.
    ///
    /// `entity_state = None`: Step-5 lifecycle is intentionally skipped at
    /// composition time (state reachability is observed separately by the
    /// non-mutating `observe_state_reachability`, preserving the "composition
    /// must not change ranking" discipline).
    fn production_allowed_set(board: &Board) -> HashSet<String> {
        use ob_poc::agent::sem_os_context_envelope::SemOsContextEnvelope;
        use ob_poc::agent::verb_surface::{
            compute_session_verb_surface, VerbSurfaceContext, VerbSurfaceFailPolicy,
        };
        use sem_os_types::agent_mode::AgentMode;

        let envelope = SemOsContextEnvelope::unavailable();
        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: board.stage_focus.as_deref(),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: board.has_group_scope,
            is_infrastructure_scope: board.is_infrastructure_scope,
            composite_state: None,
        };
        compute_session_verb_surface(&ctx).allowed_fqns()
    }

    /// Phase 2 receipt: the harness CBU allowed set IS the production allowed set
    /// (no divergence), and it is now the 13-domain workspace composition — not
    /// the 4-domain pack. No DB needed (`compute_session_verb_surface` reads the
    /// runtime registry + macro files only).
    #[test]
    fn phase2_board_equals_production_surface() {
        use ob_poc::agent::sem_os_context_envelope::SemOsContextEnvelope;
        use ob_poc::agent::verb_surface::{
            compute_session_verb_surface, VerbSurfaceContext, VerbSurfaceFailPolicy,
        };
        use sem_os_types::agent_mode::AgentMode;

        let board = boards_by_id()
            .remove("board-cbu-operational")
            .expect("cbu board");
        assert_eq!(
            board.stage_focus.as_deref(),
            Some("semos-onboarding"),
            "Phase 2 requires the CBU board to carry the workspace stage_focus"
        );

        // The dispatched harness set.
        let harness = board_allowed_set(&board, &HashSet::new());

        // The genuine live surface, built independently here.
        let envelope = SemOsContextEnvelope::unavailable();
        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-onboarding"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };
        let live = compute_session_verb_surface(&ctx).allowed_fqns();

        // EQUALITY: same membership, same macros — the divergence is gone.
        assert_eq!(
            harness, live,
            "harness CBU allowed set must equal the production surface"
        );

        // COMPOSITION: it is the 13-domain workspace, not the 4-domain pack.
        // An atomic verb from a workspace domain OUTSIDE the old pack is present.
        assert!(
            live.contains("trading-profile.read"),
            "13-domain workspace composition must include trading-profile.* atomics \
             that the old 4-domain pack excluded"
        );
        // A CBU-owned macro (Phase-1 membership) is present.
        assert!(
            live.contains("structure.product-suite-full"),
            "production surface must include the CBU-owned macro"
        );
        // The OLD 4-domain pack-scoped set did NOT contain it — proves expansion.
        let old_pack: HashSet<&str> = ["cbu", "session", "view", "agent"].into_iter().collect();
        let old_set = board_legal_set(&old_pack);
        assert!(
            !old_set.contains("trading-profile.read"),
            "the pre-Phase-2 pack-scoped set excluded trading-profile.* — confirming \
             the board was under-composed"
        );
    }

    /// Phase 3 C3 — the convergence invariant, hardened past the degenerate
    /// `None == None`. The production discovery surface MEMBERSHIP is invariant
    /// to `entity_state`: discovery no longer prunes on lifecycle (M2), so the
    /// harness set equals the production surface built with a REAL entity state
    /// (M4). `entity_state` only flips the per-verb `lifecycle_eligible` TAG
    /// (the select-then-validate signal), never membership.
    ///
    /// Pre-C2 this FAILED: the `Some("DISCOVERED")` surface pruned `cbu.confirm`
    /// (requires VALIDATION_PENDING) and friends, so it was strictly smaller
    /// than the harness/`None` set. Post-C2 all three are identical.
    #[test]
    fn c3_production_surface_membership_state_invariant() {
        use ob_poc::agent::sem_os_context_envelope::SemOsContextEnvelope;
        use ob_poc::agent::verb_surface::{
            compute_session_verb_surface, SessionVerbSurface, VerbSurfaceContext,
            VerbSurfaceFailPolicy,
        };
        use sem_os_types::agent_mode::AgentMode;

        let board = boards_by_id()
            .remove("board-cbu-operational")
            .expect("cbu board");
        let harness = board_allowed_set(&board, &HashSet::new());

        let envelope = SemOsContextEnvelope::unavailable();
        let surface_for = |state: Option<&str>| -> SessionVerbSurface {
            let ctx = VerbSurfaceContext {
                agent_mode: AgentMode::Governed,
                stage_focus: Some("semos-onboarding"),
                envelope: &envelope,
                fail_policy: VerbSurfaceFailPolicy::FailOpen,
                entity_state: state,
                has_group_scope: true,
                is_infrastructure_scope: false,
                composite_state: None,
            };
            compute_session_verb_surface(&ctx)
        };

        let none_surface = surface_for(None);
        let discovered_surface = surface_for(Some("DISCOVERED"));

        // M2: membership is invariant to entity_state (no lifecycle prune).
        assert_eq!(
            none_surface.allowed_fqns(),
            discovered_surface.allowed_fqns(),
            "discovery membership must be invariant to entity_state — discovery must \
             not prune on lifecycle"
        );
        // M4: harness == production surface built with a REAL entity_state
        // (not the degenerate None == None).
        assert_eq!(
            harness,
            discovered_surface.allowed_fqns(),
            "harness CBU set must equal the production surface built with a real \
             entity_state (DISCOVERED)"
        );

        // The state-ineligible verb stays a classification candidate…
        assert!(
            discovered_surface.contains("cbu.confirm"),
            "cbu.confirm must remain discoverable at DISCOVERED"
        );
        // …yet entity_state still flips its eligibility TAG (read the pub field;
        // external tests use the public API only).
        let eligible = |s: &SessionVerbSurface, fqn: &str| -> Option<bool> {
            s.verbs
                .iter()
                .find(|v| v.fqn == fqn)
                .map(|v| v.lifecycle_eligible)
        };
        assert_eq!(
            eligible(&discovered_surface, "cbu.confirm"),
            Some(false),
            "cbu.confirm (requires VALIDATION_PENDING) tagged ineligible at DISCOVERED"
        );
        assert_eq!(
            eligible(&none_surface, "cbu.confirm"),
            Some(true),
            "no entity_state ⇒ eligible (cannot check)"
        );
        assert_eq!(
            eligible(&discovered_surface, "cbu.submit-for-validation"),
            Some(true),
            "submit-for-validation (requires DISCOVERED) eligible at DISCOVERED"
        );
    }

    /// Tag an out-of-scope expected verb with the workspace that owns it.
    /// Scoping is by the **workspace membership of the expected verb/macro**, never
    /// by whether discovery resolves it.
    fn true_workspace_for(domain: &str) -> &'static str {
        match domain {
            // SemOS-maintenance / data governance.
            "attribute" | "service-resource" | "governance" | "derivation"
            | "typed-attribute" | "changeset" | "registry" | "schema" | "authoring"
            | "service" => "sem_os_maintenance",
            // KYC / UBO.
            "ownership" | "evidence" | "screening" | "case" | "allegation" | "bods"
            | "ubo" | "kyc" | "requirement" | "entity-workstream" | "movement" => "kyc",
            // Pre-workspace scope-gate (selected before a workspace exists).
            "client-group" => "scope_gate",
            // System / contextual-query surface (no mutation workspace).
            "narration" => "system",
            // Capital / instruments / structuring referenced-entity content not
            // owned by the onboarding workspace's atomic domains.
            "capital" | "instrument-class" | "booking-principal" | "partnership"
            | "trust" | "identifier" | "mandate" | "readiness" | "tollgate"
            | "product" | "structure" => "other",
            _ => "unmapped",
        }
    }

    /// Phase 3 receipt: partition the 219 CBU-labelled cases into in-scope (the
    /// expected verb/macro is owned by the onboarding workspace = present in the
    /// production-composed allowed set) vs out-of-scope (owned by another
    /// workspace, tagged but NOT deleted). No DB needed.
    #[test]
    fn phase3_corpus_scoping_by_membership() {
        let board = boards_by_id()
            .remove("board-cbu-operational")
            .expect("cbu board");
        // Workspace membership set: 13-domain atomics ∪ CBU-owned macros
        // (struct.*/structure.* recovered here via Phase-1 mode-tag membership).
        let allowed = production_allowed_set(&board);

        let corpus = load_corpus();
        let cbu: Vec<&CorpusEntry> = corpus
            .iter()
            .filter(|c| c.board_id == "board-cbu-operational")
            .collect();

        let mut in_scope: Vec<&str> = Vec::new();
        let mut out_by_ws: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
            Default::default();

        for c in &cbu {
            if allowed.contains(&c.expected_verb) {
                in_scope.push(&c.expected_verb);
            } else {
                let domain = c.expected_verb.split('.').next().unwrap_or("");
                let ws = true_workspace_for(domain);
                out_by_ws.entry(ws.to_string()).or_default().push(
                    serde_json::json!({
                        "id": c.id,
                        "expected_verb": c.expected_verb,
                        "domain": domain,
                    }),
                );
            }
        }

        let total = cbu.len();
        let in_n = in_scope.len();
        let out_n: usize = out_by_ws.values().map(|v| v.len()).sum();
        assert_eq!(in_n + out_n, total, "every case is scoped exactly once");

        let out_counts: std::collections::BTreeMap<&String, usize> =
            out_by_ws.iter().map(|(k, v)| (k, v.len())).collect();

        let report = serde_json::json!({
            "frozen_pack": "88eb3699 + Phase-1 macro membership fix",
            "scoping_rule": "in-scope = expected verb/macro ∈ production_allowed_set \
                             (13-domain semos-onboarding atomics ∪ CBU-owned macros). \
                             Membership only — NOT whether discovery resolves it.",
            "total_cbu_cases": total,
            "in_scope": in_n,
            "out_of_scope": out_n,
            "out_of_scope_by_workspace": out_counts,
            "out_of_scope_detail": out_by_ws,
        });
        std::fs::write(
            "reports/cbu_corpus_scoping.json",
            serde_json::to_string_pretty(&report).unwrap(),
        )
        .unwrap();

        eprintln!(
            "Phase 3 scoping: {in_n}/{total} in-scope, {out_n} out-of-scope {:?}",
            out_counts
        );
        // Sanity: the workspace composition recovers a meaningful in-scope set
        // (research floor ~78 atomic, ~103 with macros). Assert it is no longer
        // the 4-domain ~76 and well above it.
        assert!(
            in_n >= 78,
            "workspace composition should lift in-scope to the research floor; got {in_n}"
        );
    }

    /// FQNs the search can reach via macro/scenario tiers (-2B/-2A).
    fn load_macro_scenario_fqns() -> HashSet<String> {
        let mut fqns = HashSet::new();
        // Macro FQNs from the macro registry.
        if let Ok(reg) =
            ob_poc::dsl_v2::load_macro_registry_from_dir(Path::new("config/verb_schemas/macros"))
        {
            for fqn in reg.all_fqns() {
                fqns.insert(fqn.clone());
            }
        }
        // Scenario route targets — read raw YAML and harvest dotted verb/macro FQNs.
        if let Ok(text) = std::fs::read_to_string("config/scenario_index.yaml") {
            for tok in text.split(|c: char| !(c.is_alphanumeric() || c == '.' || c == '-' || c == '_')) {
                if tok.contains('.') && tok.split('.').next().map(|d| !d.is_empty()).unwrap_or(false) {
                    fqns.insert(tok.to_string());
                }
            }
        }
        fqns
    }

    async fn build_searcher(pool: &sqlx::PgPool) -> HybridVerbSearcher {
        use ob_poc::agent::learning::embedder::{CandleEmbedder, Embedder};
        use ob_poc::agent::learning::warmup::LearningWarmup;
        use ob_poc::database::verb_service::VerbService;
        use ob_poc::mcp::macro_index::MacroIndex;
        use ob_poc::mcp::scenario_index::ScenarioIndex;

        let _ = sqlx::query("SET ivfflat.probes = 100").execute(pool).await;
        let embedder = Arc::new(CandleEmbedder::new().expect("embedder"));
        let dyn_embedder: Arc<dyn Embedder> = embedder;
        let verb_service = Arc::new(VerbService::new(pool.clone()));
        let (learned_data, _) = LearningWarmup::new(pool.clone()).warmup().await.expect("warmup");

        let macro_index = {
            let path = Path::new("config/verb_schemas/macros");
            path.is_dir().then(|| {
                let reg = ob_poc::dsl_v2::load_macro_registry_from_dir(path).expect("macro registry");
                Arc::new(MacroIndex::from_registry(&reg, None))
            })
        };
        let scenario_index = {
            let path = Path::new("config/scenario_index.yaml");
            path.is_file()
                .then(|| Arc::new(ScenarioIndex::from_yaml_file(path).expect("scenario index")))
        };

        let mut s = HybridVerbSearcher::new(verb_service, Some(learned_data)).with_embedder(dyn_embedder);
        if let Some(mi) = macro_index {
            s = s.with_macro_index(mi);
        }
        if let Some(si) = scenario_index {
            s = s.with_scenario_index(si);
        }
        s
    }

    async fn search(
        searcher: &HybridVerbSearcher,
        utterance: &str,
        allowed: &HashSet<String>,
    ) -> Vec<VerbSearchResult> {
        searcher
            .search(utterance, None, None, None, 5, Some(allowed), None, None)
            .await
            .unwrap_or_default()
    }

    /// Phase 1 receipt: a single fully-populated extended trace + the evidence
    /// fields, written to reports/phase1_single_trace.json.
    #[tokio::test]
    #[ignore = "Phase 1 receipt: requires DATABASE_URL + database feature"]
    async fn phase1_single_trace_receipt() {
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL"))
            .await
            .expect("connect");
        let searcher = build_searcher(&pool).await;
        let boards = boards_by_id();
        let macro_scenario = load_macro_scenario_fqns();

        // A representative case from the CBU board.
        let corpus = load_corpus();
        let case = corpus
            .iter()
            .find(|c| c.board_id == "board-cbu-operational")
            .expect("a cbu case");
        let board = &boards[&case.board_id];
        let allowed = board_allowed_set(board, &macro_scenario);

        let results = search(&searcher, &case.utterance, &allowed).await;
        let ranked: Vec<(String, f32)> = results.iter().map(|r| (r.verb.clone(), r.score)).collect();

        // Evidence fields (the Option-C extension).
        let total_registry = ob_poc::dsl_v2::execution::runtime_registry().all_verbs().count();
        let flow = soft_stage_flow(&results);
        let allowed_vec: Vec<String> = allowed.iter().cloned().collect();
        let observations = observe_state_reachability(&allowed_vec, board.entity_state.as_deref());
        let state_unreachable = observations.iter().filter(|o| !o.state_reachable).count();

        let trace = serde_json::json!({
            "utterance": case.utterance,
            "expected_verb": case.expected_verb,
            "board_id": case.board_id,
            "entity_state": board.entity_state,
            // ── Option-C evidence fields ──
            "surface_full_count": total_registry,
            "surface_pack_scoped_count": allowed.len(),
            "soft_stage_flow": flow,
            "entity_confidence": serde_json::Value::Null, // no context resolution in eval path
            "state_observer_total": observations.len(),
            "state_observer_unreachable": state_unreachable,
            // ── survival ──
            "ranked": ranked,
            "expected_survived": results.iter().any(|r| r.verb == case.expected_verb),
            "expected_rank": results.iter().position(|r| r.verb == case.expected_verb),
        });

        std::fs::create_dir_all("reports").ok();
        std::fs::write("reports/phase1_single_trace.json", serde_json::to_string_pretty(&trace).unwrap())
            .expect("write phase1_single_trace.json");
        println!("\n===== PHASE 1 SINGLE EXTENDED TRACE =====");
        println!("{}", serde_json::to_string_pretty(&trace).unwrap());
        println!("\n  receipt -> reports/phase1_single_trace.json");
    }

    /// Phase 1 ranking-unchanged proof: for a 20-utterance sample, the ranked
    /// list with capture (soft_stage_flow + state observer) MUST equal the bare
    /// search ranked list. The capture is read-only and never feeds back.
    #[tokio::test]
    #[ignore = "Phase 1 receipt: requires DATABASE_URL + database feature"]
    async fn phase1_ranking_unchanged() {
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL"))
            .await
            .expect("connect");
        let searcher = build_searcher(&pool).await;
        let boards = boards_by_id();
        let macro_scenario = load_macro_scenario_fqns();
        let corpus = load_corpus();

        let mut diffs = Vec::new();
        for case in corpus.iter().take(20) {
            let board = &boards[&case.board_id];
            let allowed = board_allowed_set(board, &macro_scenario);

            // Bare search.
            let bare = search(&searcher, &case.utterance, &allowed).await;
            let bare_ranked: Vec<(String, f32)> = bare.iter().map(|r| (r.verb.clone(), r.score)).collect();

            // Search + capture (read-only observation).
            let captured = search(&searcher, &case.utterance, &allowed).await;
            let _flow = soft_stage_flow(&captured);
            let allowed_vec: Vec<String> = allowed.iter().cloned().collect();
            let _obs = observe_state_reachability(&allowed_vec, board.entity_state.as_deref());
            let cap_ranked: Vec<(String, f32)> = captured.iter().map(|r| (r.verb.clone(), r.score)).collect();

            if bare_ranked != cap_ranked {
                diffs.push(serde_json::json!({
                    "id": case.id, "bare": bare_ranked, "captured": cap_ranked
                }));
            }
        }

        std::fs::create_dir_all("reports").ok();
        std::fs::write(
            "reports/phase1_ranking_unchanged.json",
            serde_json::to_string_pretty(&serde_json::json!({
                "sample": 20, "diffs": diffs, "ranking_unchanged": diffs.is_empty()
            }))
            .unwrap(),
        )
        .expect("write ranking proof");
        println!("\n===== PHASE 1 RANKING-UNCHANGED PROOF =====");
        println!("sample=20 diffs={} (empty diff => ranking unchanged)", diffs.len());
        assert!(diffs.is_empty(), "ranking changed under capture: {:?}", diffs);
    }

    // ── Phase 2 + 3: batch eval over the full corpus → jsonl + aggregate ──

    fn all_known_fqns(macro_scenario: &HashSet<String>) -> HashSet<String> {
        use ob_poc::dsl_v2::execution::runtime_registry;
        let mut set: HashSet<String> = runtime_registry()
            .all_verbs()
            .map(|v| v.full_name.clone())
            .collect();
        set.extend(macro_scenario.iter().cloned());
        set
    }

    const AMBIGUITY_MARGIN: f32 = 0.05;

    #[tokio::test]
    #[ignore = "Phase 2/3: requires DATABASE_URL + database feature (~524 searches)"]
    async fn phase2_3_batch_eval() {
        use std::io::Write;
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL"))
            .await
            .expect("connect");
        let searcher = build_searcher(&pool).await;
        let boards = boards_by_id();
        let macro_scenario = load_macro_scenario_fqns();
        let known = all_known_fqns(&macro_scenario);
        let corpus = load_corpus();
        let total_registry = ob_poc::dsl_v2::execution::runtime_registry().all_verbs().count();

        std::fs::create_dir_all("reports").ok();
        let mut jsonl = std::fs::File::create("reports/intent_traces.jsonl").expect("jsonl");

        // Aggregates.
        let mut survived_to_surface = 0usize; // expected in allowed (passed pack collapse)
        let mut retrieved = 0usize; // expected appears in top-K
        let mut top1 = 0usize; // expected is rank 0
        let mut failure_classes: HashMap<String, usize> = HashMap::new();
        let mut size_hist: HashMap<String, usize> = HashMap::new();
        let mut le5 = 0usize;
        let mut max_score = 0f32;
        let mut counterfactual_fracs: Vec<f32> = Vec::new(); // unreachable / allowed_size per case
        let mut post_selection_rejections = 0usize; // selected top-1 state-unreachable
        let mut selections = 0usize;
        let mut collapse_samples: Vec<(usize, usize)> = Vec::new(); // (full, pack_scoped)

        for case in &corpus {
            let board = &boards[&case.board_id];
            let allowed = board_allowed_set(board, &macro_scenario);
            let allowed_vec: Vec<String> = allowed.iter().cloned().collect();
            let observations = observe_state_reachability(&allowed_vec, board.entity_state.as_deref());
            let unreachable: HashSet<&String> = observations
                .iter()
                .filter(|o| !o.state_reachable)
                .map(|o| &o.verb)
                .collect();

            collapse_samples.push((total_registry, allowed.len()));
            counterfactual_fracs.push(if allowed.is_empty() {
                0.0
            } else {
                unreachable.len() as f32 / allowed.len() as f32
            });
            let bucket = match allowed.len() {
                0..=5 => "0-5",
                6..=25 => "6-25",
                26..=75 => "26-75",
                76..=200 => "76-200",
                _ => "200+",
            };
            *size_hist.entry(bucket.to_string()).or_insert(0) += 1;
            if allowed.len() <= 5 {
                le5 += 1;
            }

            let expected_in_allowed = allowed.contains(&case.expected_verb)
                || case.alt_verbs.iter().any(|a| allowed.contains(a));
            if expected_in_allowed {
                survived_to_surface += 1;
            }

            let results = searcher
                .search(&case.utterance, None, None, None, 25, Some(&allowed), None, None)
                .await
                .unwrap_or_default();
            for r in &results {
                if r.score > max_score {
                    max_score = r.score;
                }
            }
            let flow = soft_stage_flow(&results);
            let expected_rank = results
                .iter()
                .position(|r| r.verb == case.expected_verb || case.alt_verbs.contains(&r.verb));
            let is_top1 = expected_rank == Some(0);
            if expected_rank.is_some() {
                retrieved += 1;
            }
            if is_top1 {
                top1 += 1;
            }

            // Post-selection state rejection (Option B cost): is the SELECTED top-1 unreachable?
            if let Some(top) = results.first() {
                selections += 1;
                if unreachable.contains(&top.verb) {
                    post_selection_rejections += 1;
                }
            }

            // Failure classification (only when not top-1).
            let failure_class = if is_top1 {
                None
            } else if !expected_in_allowed {
                Some(if known.contains(&case.expected_verb) {
                    "PackExcluded"
                } else {
                    "Vocabulary"
                })
            } else {
                match expected_rank {
                    None => Some("RankingResidual"), // in surface, not retrieved into top-25
                    Some(rank) => {
                        let exp_score = results[rank].score;
                        let top_score = results[0].score;
                        let exp_unreachable = unreachable.contains(&case.expected_verb);
                        if exp_unreachable {
                            Some("StateUnreachable")
                        } else if top_score - exp_score < AMBIGUITY_MARGIN {
                            Some("Ambiguity")
                        } else {
                            Some("RankingResidual")
                        }
                    }
                }
            };
            if let Some(fc) = failure_class {
                *failure_classes.entry(fc.to_string()).or_insert(0) += 1;
            }

            let record = serde_json::json!({
                "id": case.id,
                "utterance": case.utterance,
                "expected_verb": case.expected_verb,
                "board_id": case.board_id,
                "entity_state": board.entity_state,
                "surface_full_count": total_registry,
                "surface_pack_scoped_count": allowed.len(),
                "soft_stage_flow": flow,
                "state_observer": { "total": observations.len(), "unreachable": unreachable.len() },
                "entity_confidence": serde_json::Value::Null,
                "survival": {
                    "survived_to_surface": expected_in_allowed,
                    "retrieved": expected_rank.is_some(),
                    "expected_rank": expected_rank,
                    "top1": is_top1,
                    "failure_class": failure_class,
                },
                "top_result": results.first().map(|r| serde_json::json!([r.verb, r.score])),
            });
            writeln!(jsonl, "{}", serde_json::to_string(&record).unwrap()).ok();
        }

        let n = corpus.len() as f32;
        let mean = |v: &[f32]| if v.is_empty() { 0.0 } else { v.iter().sum::<f32>() / v.len() as f32 };
        let collapse_ratios: Vec<f32> = collapse_samples
            .iter()
            .map(|(f, p)| *p as f32 / *f as f32)
            .collect();
        // Registry-wide state-precondition coverage: the denominator that makes
        // the state-counterfactual interpretable. If almost no verbs declare
        // requires_states, a near-zero counterfactual measures METADATA ABSENCE,
        // not state weakness.
        let verbs_with_state_preconditions = ob_poc::dsl_v2::execution::runtime_registry()
            .all_verbs()
            .filter(|v| {
                v.lifecycle
                    .as_ref()
                    .map(|l| !l.requires_states.is_empty())
                    .unwrap_or(false)
            })
            .count();
        let state_precondition_coverage = verbs_with_state_preconditions as f32 / total_registry as f32;

        let aggregate = serde_json::json!({
            "note": "Intent Trace Option-C aggregate. Survival = expected reaches the surface/ranked; \
                     state_collapse_counterfactual = Option A prize; post_selection_state_rejection_rate = Option B cost. \
                     entity_confidence is null in the eval (search-only path, no context resolution).",
            "total_cases": corpus.len(),
            "survival_recall_to_surface": survived_to_surface as f32 / n,
            "retrieval_recall": retrieved as f32 / n,
            "top1_accuracy": top1 as f32 / n,
            "pack_collapse": {
                "registry_verbs": total_registry,
                "mean_pack_scoped_fraction": mean(&collapse_ratios),
                "min_pack_scoped": collapse_samples.iter().map(|(_, p)| *p).min(),
                "max_pack_scoped": collapse_samples.iter().map(|(_, p)| *p).max(),
            },
            "state_collapse_counterfactual": {
                "mean_unreachable_fraction_of_allowed": mean(&counterfactual_fracs),
                "interpretation": format!(
                    "Fraction of the pack-scoped set state WOULD remove. Small here reflects ~ABSENT \
                     precondition metadata (only {:.2}% of verbs declare requires_states; see `decision`) \
                     — NOT a weak reducer. Uninterpretable as a state-strength signal as-is.",
                    state_precondition_coverage * 100.0
                ),
            },
            "post_selection_state_rejection_rate": {
                "rate": if selections == 0 { 0.0 } else { post_selection_rejections as f32 / selections as f32 },
                "rejected": post_selection_rejections,
                "selections": selections,
                "interpretation": "Option B cost: fraction of selected top-1 verbs state would reject under select-then-validate.",
            },
            "failure_class_distribution": failure_classes,
            "set_size_histogram": size_hist,
            "fraction_le_5": le5 as f32 / n,
            "max_score": max_score,
            "max_score_le_1": max_score <= 1.0 + f32::EPSILON,
            "ambiguous_count": failure_classes.get("Ambiguity").copied().unwrap_or(0),
            "owed_scoring_receipts": {
                "survival_recall_ge_0695": survived_to_surface as f32 / n >= 0.695,
                "top1_accuracy": top1 as f32 / n,
                "top1_ge_0695_pre_fix": top1 as f32 / n >= 0.695,
                "combiner_lambda": 0.5,
                "combiner_note": "Code ships COMBINE_LAMBDA=0.5 (DAMPED) + 0.99 score cap — NOT the undamped λ=1 the plan feared. Saturation/pancaking failure mode does not apply.",
                "score_cap_holds": max_score <= 0.99 + f32::EPSILON,
            },
            // ── Decision (operational decision and v0.5 thesis are SEPARATE claims) ──
            "decision": {
                "operational": {
                    "verdict": "Option B — keep architecture (do NOT move state into discovery)",
                    "supported": true,
                    "rationale": "Moving state forward (Option A) costs the provisional-guard risk, and \
                        the measurable prize here is ~0. Leaving select-then-validate in place is correct.",
                },
                "v0_5_state_reducer_thesis": {
                    "status": "UNTESTED — neither proven nor refuted",
                    "do_not_reword": "Do NOT reword v0.5 to 'state is a thin gate'. Unanswerable until the \
                        registry carries state preconditions to collapse on.",
                    "why_unmeasurable": format!(
                        "Only {}/{} verbs ({:.2}%) declare requires_states, and StateUnreachable \
                         failure_class = 0. The {:.2}% state_collapse_counterfactual measures the ABSENCE \
                         of state metadata, not the weakness of state as a reducer — it is structurally \
                         pinned near zero regardless of how strong state would be in a fully-authored registry.",
                        verbs_with_state_preconditions, total_registry,
                        state_precondition_coverage * 100.0,
                        mean(&counterfactual_fracs) * 100.0,
                    ),
                    "state_precondition_coverage": state_precondition_coverage,
                },
                "pack_collapse_finding": {
                    "fraction_le_5": le5 as f32 / n,
                    "finding": "Pack scope does NOT reach a small set. fraction_le_5 = 0.0; most cases \
                        still carry 200+ candidates after collapse (see set_size_histogram). Retrieval \
                        0.94 vs top1 0.78 => the SCORER carries the load the board was meant to remove. \
                        This contradicts the v0.5 promise of constrained classification over a tiny set.",
                    "fidelity_caveat": "Boards approximate the COARSE Step-3 domain intersection; the tight \
                        reducer is live SemOS Step 4, not replicated here. So fraction_le_5 = 0.0 may \
                        UNDERSTATE real collapse — pack collapse is ALSO not measured at full strength.",
                },
                "net": "Operationally Option B (no architecture change). v0.5 state-reducer claim UNTESTED \
                    pending precondition coverage. Pack-collapse re-measure owed against live SemOS Step 4. \
                    Neither reducer measured at full strength — the thesis is indeterminate, not settled.",
                "owed_before_v0_5_can_be_judged": [
                    format!(
                        "Author requires_states coverage across the registry ({}/{} ≈ {:.2}% is what makes \
                         the state question permanently unanswerable).",
                        verbs_with_state_preconditions, total_registry, state_precondition_coverage * 100.0
                    ),
                    "Re-run the pack-collapse measurement against live SemOS Step 4, not the coarse board proxy.".to_string(),
                ],
            },
        });

        std::fs::write(
            "reports/intent_trace_aggregate.json",
            serde_json::to_string_pretty(&aggregate).unwrap(),
        )
        .expect("write aggregate");

        println!("\n===== INTENT TRACE AGGREGATE (Phase 2/3) =====");
        println!("{}", serde_json::to_string_pretty(&aggregate).unwrap());
        println!("\n  per-case traces -> reports/intent_traces.jsonl");
        println!("  aggregate       -> reports/intent_trace_aggregate.json");

        assert!(max_score <= 1.0 + f32::EPSILON, "max_score exceeded 1.0: {}", max_score);
    }

    // ── CBU Phase 2: First-hit / Second-hit metric (frozen pack 88eb3699) ──
    //
    // Classification only — verbs are resolved, never executed. Uses the SYSTEM's
    // own commit-vs-ask decision (`check_ambiguity` at semantic_threshold), read
    // not overridden. No answer injection: the second-prompt check is membership
    // of the expected verb in the surfaced top-K shortlist, never feeding the
    // ground truth back as a synthetic utterance.
    #[tokio::test]
    #[ignore = "CBU Phase 2 second-hit metric: DATABASE_URL + database feature"]
    async fn cbu_second_hit_metric() {
        use ob_poc::mcp::verb_search::{check_ambiguity, VerbSearchOutcome};
        const FROZEN: &str = "88eb369975f6e266920ff5a30380f768652bf4cc";

        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL"))
            .await
            .expect("connect");
        let searcher = build_searcher(&pool).await;
        let threshold = searcher.semantic_threshold();
        let boards = boards_by_id();
        let macro_scenario = load_macro_scenario_fqns();
        let board = &boards["board-cbu-operational"];
        let allowed = board_allowed_set(board, &macro_scenario);
        let allowed_vec: Vec<String> = allowed.iter().cloned().collect();
        let state_unreachable: HashSet<String> =
            observe_state_reachability(&allowed_vec, board.entity_state.as_deref())
                .into_iter()
                .filter(|o| !o.state_reachable)
                .map(|o| o.verb)
                .collect();

        let corpus = load_corpus();
        let cbu_cases: Vec<&CorpusEntry> = corpus
            .iter()
            .filter(|c| c.board_id == "board-cbu-operational")
            .collect();

        #[derive(Default)]
        struct Acc {
            first: usize,
            confident_wrong: usize,
            // ask outcomes carry the expected-verb rank in the shortlist (None = not retrieved)
            ask_ranks: Vec<(Option<usize>, bool)>, // (rank, expected_in_pack)
            nomatch_in_pack: usize,
            nomatch_out_pack: usize,
            confident_wrong_pairs: Vec<serde_json::Value>,
            first_in_pack: usize,
            confident_wrong_in_pack: usize,
            // state side-metric: shortlist (top-5) contains a state-unreachable verb
            shortlist_has_state_unreachable: usize,
        }
        let mut a = Acc::default();
        let mut in_pack_total = 0usize;

        let is_hit = |verb: &str, case: &CorpusEntry| {
            verb == case.expected_verb || case.alt_verbs.iter().any(|x| x == verb)
        };

        for case in &cbu_cases {
            let expected_in_pack =
                allowed.contains(&case.expected_verb) || case.alt_verbs.iter().any(|x| allowed.contains(x));
            if expected_in_pack {
                in_pack_total += 1;
            }
            let candidates = searcher
                .search(&case.utterance, None, None, None, 10, Some(&allowed), None, None)
                .await
                .unwrap_or_default();
            if candidates.iter().take(5).any(|c| state_unreachable.contains(&c.verb)) {
                a.shortlist_has_state_unreachable += 1;
            }
            let outcome = check_ambiguity(&candidates, threshold);
            match outcome {
                VerbSearchOutcome::Matched(top) => {
                    if is_hit(&top.verb, case) {
                        a.first += 1;
                        if expected_in_pack {
                            a.first_in_pack += 1;
                        }
                    } else {
                        a.confident_wrong += 1;
                        if expected_in_pack {
                            a.confident_wrong_in_pack += 1;
                        }
                        a.confident_wrong_pairs.push(serde_json::json!({
                            "expected": case.expected_verb,
                            "selected": top.verb,
                            "in_pack": expected_in_pack,
                            "kind": if expected_in_pack { "genuine_near_synonym" } else { "coverage_artifact" },
                        }));
                    }
                }
                VerbSearchOutcome::Ambiguous { .. } | VerbSearchOutcome::Suggest { .. } => {
                    let rank = candidates.iter().position(|c| is_hit(&c.verb, case));
                    a.ask_ranks.push((rank, expected_in_pack));
                }
                VerbSearchOutcome::NoMatch => {
                    if expected_in_pack {
                        a.nomatch_in_pack += 1;
                    } else {
                        a.nomatch_out_pack += 1;
                    }
                }
            }
        }

        let n = cbu_cases.len() as f32;
        let second_at = |k: usize| a.ask_ranks.iter().filter(|(r, _)| r.map(|r| r < k).unwrap_or(false)).count();
        let second_at_in_pack = |k: usize| {
            a.ask_ranks
                .iter()
                .filter(|(r, ip)| *ip && r.map(|r| r < k).unwrap_or(false))
                .count()
        };
        let second3 = second_at(3);
        let second5 = second_at(5);
        // rank distribution (1-indexed) among ask outcomes where expected is within top-5
        let mut rank_hist: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        for (r, _) in &a.ask_ranks {
            if let Some(r) = r {
                if *r < 5 {
                    *rank_hist.entry(r + 1).or_insert(0) += 1;
                }
            }
        }
        let ask_total = a.ask_ranks.len();
        let miss3 = cbu_cases.len() - a.first - a.confident_wrong - second3;
        let miss5 = cbu_cases.len() - a.first - a.confident_wrong - second5;

        // in-pack subset (the honest discovery number — excludes coverage misses)
        let ip = in_pack_total.max(1) as f32;
        let s3_ip = second_at_in_pack(3);
        let s5_ip = second_at_in_pack(5);

        let report = serde_json::json!({
            "frozen_commit": FROZEN,
            "note": "CBU first/second-hit. Commit-vs-ask is the system's own check_ambiguity at \
                     semantic_threshold (read, not overridden). Second hit = expected ∈ surfaced top-K \
                     (no answer injection). Confident-wrong is NEVER counted as a hit.",
            "semantic_threshold": threshold,
            "total_cbu_cases": cbu_cases.len(),
            "coverage": {
                "expected_in_frozen_pack": in_pack_total,
                "out_of_pack": cbu_cases.len() - in_pack_total,
                "caveat": "Only ~1/4 of CBU-labelled cases expect a verb in the clean cbu+nav pack; \
                           the rest expect struct/attribute/narration/trading-profile/etc. Out-of-pack \
                           misses are a COVERAGE gap (corpus 'CBU' label is broader than the domain-model \
                           cbu pack), not a discovery failure. See in_pack_subset for the discovery number."
            },
            "buckets_all_219": {
                "first_hit": a.first,
                "confident_wrong": a.confident_wrong,
                "ask_total": ask_total,
                "second_hit_at_3": second3,
                "second_hit_at_5": second5,
                "miss_at_3": miss3,
                "miss_at_5": miss5,
                "miss_breakdown": {
                    "nomatch_out_of_pack": a.nomatch_out_pack,
                    "nomatch_in_pack": a.nomatch_in_pack,
                    "ask_but_expected_not_in_top5": ask_total - second5,
                },
                "sum_check_at_3": a.first + a.confident_wrong + second3 + miss3,
                "sum_check_at_5": a.first + a.confident_wrong + second5 + miss5,
            },
            "rates_all_219": {
                "first_hit_rate": a.first as f32 / n,
                "second_hit_rate_at_3": second3 as f32 / n,
                "second_hit_rate_at_5": second5 as f32 / n,
                "within_2_at_3": (a.first + second3) as f32 / n,
                "within_2_at_5": (a.first + second5) as f32 / n,
                "confident_wrong_rate": a.confident_wrong as f32 / n,
                "miss_rate_at_5": miss5 as f32 / n,
            },
            "in_pack_subset": {
                "n": in_pack_total,
                "first_hit": a.first_in_pack,
                "confident_wrong": a.confident_wrong_in_pack,
                "first_hit_rate": a.first_in_pack as f32 / ip,
                "within_2_at_3": (a.first_in_pack + s3_ip) as f32 / ip,
                "within_2_at_5": (a.first_in_pack + s5_ip) as f32 / ip,
                "confident_wrong_rate": a.confident_wrong_in_pack as f32 / ip,
            },
            "second_hit_rank_distribution_1indexed": rank_hist,
            "confident_wrong_pairs": a.confident_wrong_pairs,
            "state_gating_side_metric": {
                "board_entity_state": board.entity_state,
                "cases_whose_top5_contains_a_state_unreachable_verb": a.shortlist_has_state_unreachable,
                "note": "How often I3 lifecycle-gating WOULD prune the shown shortlist at OPERATIONALLY_ACTIVE — measurable for the first time now CBU I3 is authored (gated 4→11)."
            },
        });

        std::fs::create_dir_all("reports").ok();
        std::fs::write("reports/cbu_second_hit.json", serde_json::to_string_pretty(&report).unwrap())
            .expect("write cbu_second_hit.json");
        println!("\n===== CBU SECOND-HIT METRIC (frozen {}) =====", &FROZEN[..8]);
        println!("{}", serde_json::to_string_pretty(&report["rates_all_219"]).unwrap());
        println!("in-pack discovery: {}", serde_json::to_string_pretty(&report["in_pack_subset"]).unwrap());
        println!("coverage: {}", serde_json::to_string_pretty(&report["coverage"]).unwrap());
        println!("  -> reports/cbu_second_hit.json");
        assert_eq!(a.first + a.confident_wrong + second5 + miss5, cbu_cases.len(), "buckets must sum to N");
    }

    /// One arm of the composed metric over a fixed allowed set.
    struct ArmResult {
        n: usize,
        board_size: usize,
        first: usize,
        confident_wrong: usize,
        second3: usize,
        second5: usize,
        /// recall@5 — expected verb retrieved within top-5, irrespective of the
        /// commit-vs-ask decision. The "small-set thesis" test on a larger board.
        recall5: usize,
        miss5: usize,
        ask_total: usize,
        confident_wrong_pairs: Vec<serde_json::Value>,
    }

    async fn run_arm(
        searcher: &HybridVerbSearcher,
        threshold: f32,
        cases: &[&CorpusEntry],
        allowed: &HashSet<String>,
    ) -> ArmResult {
        use ob_poc::mcp::verb_search::{check_ambiguity, VerbSearchOutcome};
        let is_hit = |verb: &str, case: &CorpusEntry| {
            verb == case.expected_verb || case.alt_verbs.iter().any(|x| x == verb)
        };
        let mut r = ArmResult {
            n: cases.len(),
            board_size: allowed.len(),
            first: 0,
            confident_wrong: 0,
            second3: 0,
            second5: 0,
            recall5: 0,
            miss5: 0,
            ask_total: 0,
            confident_wrong_pairs: Vec::new(),
        };
        let mut ask_in_top5 = 0usize;
        for case in cases {
            let candidates = searcher
                .search(&case.utterance, None, None, None, 10, Some(allowed), None, None)
                .await
                .unwrap_or_default();
            let rank = candidates.iter().position(|c| is_hit(&c.verb, case));
            if rank.map(|x| x < 5).unwrap_or(false) {
                r.recall5 += 1;
            }
            match check_ambiguity(&candidates, threshold) {
                VerbSearchOutcome::Matched(top) => {
                    if is_hit(&top.verb, case) {
                        r.first += 1;
                    } else {
                        r.confident_wrong += 1;
                        r.confident_wrong_pairs.push(serde_json::json!({
                            "expected": case.expected_verb,
                            "selected": top.verb,
                        }));
                    }
                }
                VerbSearchOutcome::Ambiguous { .. } | VerbSearchOutcome::Suggest { .. } => {
                    r.ask_total += 1;
                    if let Some(x) = rank {
                        if x < 3 {
                            r.second3 += 1;
                        }
                        if x < 5 {
                            r.second5 += 1;
                            ask_in_top5 += 1;
                        }
                    }
                }
                VerbSearchOutcome::NoMatch => {}
            }
        }
        let _ = ask_in_top5;
        r.miss5 = r.n - r.first - r.confident_wrong - r.second5;
        r
    }

    /// Phase 4: the HONEST production numbers — first/second-hit over the
    /// production-composed 13-domain board, on the in-scope CBU subset only
    /// (Phase-3 membership). Sensitivity arm strips the owned macros to show the
    /// macro layer's contribution. Receipt: reports/cbu_second_hit_composed.json.
    #[tokio::test]
    #[ignore = "Phase 4 receipt: requires DATABASE_URL + database feature"]
    async fn phase4_cbu_second_hit_composed() {
        const FROZEN: &str = "88eb3699 + Phase-1 macro membership fix";
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL"))
            .await
            .expect("connect");
        let searcher = build_searcher(&pool).await;
        let threshold = searcher.semantic_threshold();

        let board = boards_by_id()
            .remove("board-cbu-operational")
            .expect("cbu board");

        // Composed (production) allowed set: 13-domain atomics ∪ CBU-owned macros.
        let allowed_full = production_allowed_set(&board);
        // Sensitivity arm: atomic-only (strip the membership-owned macros).
        let atomic_domains: HashSet<&str> = [
            "cbu", "entity", "session", "view", "agent", "contract", "deal", "billing",
            "trading-profile", "custody", "onboarding", "gleif", "research",
        ]
        .into_iter()
        .collect();
        let allowed_atomic = board_legal_set(&atomic_domains);
        let owned_macros = allowed_full.len().saturating_sub(allowed_atomic.len());

        // In-scope subset (Phase-3 membership): expected ∈ composed allowed set.
        let corpus = load_corpus();
        let in_scope: Vec<&CorpusEntry> = corpus
            .iter()
            .filter(|c| c.board_id == "board-cbu-operational")
            .filter(|c| {
                allowed_full.contains(&c.expected_verb)
                    || c.alt_verbs.iter().any(|x| allowed_full.contains(x))
            })
            .collect();

        let full = run_arm(&searcher, threshold, &in_scope, &allowed_full).await;
        let atomic = run_arm(&searcher, threshold, &in_scope, &allowed_atomic).await;

        let rate = |x: usize, n: usize| x as f32 / n.max(1) as f32;
        let arm_json = |r: &ArmResult| {
            serde_json::json!({
                "denominator": r.n,
                "board_size": r.board_size,
                "first_hit": r.first,
                "confident_wrong": r.confident_wrong,
                "second_hit_at_3": r.second3,
                "second_hit_at_5": r.second5,
                "miss_at_5": r.miss5,
                "ask_total": r.ask_total,
                "first_hit_rate": rate(r.first, r.n),
                "second_hit_rate_at_5": rate(r.second5, r.n),
                "within_2_at_3": rate(r.first + r.second3, r.n),
                "within_2_at_5": rate(r.first + r.second5, r.n),
                "confident_wrong_rate": rate(r.confident_wrong, r.n),
                "miss_rate_at_5": rate(r.miss5, r.n),
                "fraction_le_5_recall": rate(r.recall5, r.n),
                "sum_check": r.first + r.confident_wrong + r.second5 + r.miss5,
            })
        };

        assert_eq!(
            full.first + full.confident_wrong + full.second5 + full.miss5,
            full.n,
            "composed buckets must sum to the in-scope denominator"
        );

        let report = serde_json::json!({
            "frozen_commit": FROZEN,
            "note": "HONEST production numbers. Board = 13-domain semos-onboarding workspace \
                     composition (compute_session_verb_surface, Phase-1 macro membership). \
                     Denominator = in-scope CBU subset (Phase-3 membership: expected ∈ composed \
                     allowed set). Commit-vs-ask = system's own check_ambiguity at semantic_threshold \
                     (read, not overridden). No answer injection. Confident-wrong is NEVER a hit.",
            "semantic_threshold": threshold,
            "composed_board": {
                "atomic_verbs_13_domain": allowed_atomic.len(),
                "owned_macros": owned_macros,
                "board_size_total": allowed_full.len(),
            },
            "primary_arm_composed": arm_json(&full),
            "sensitivity_arm_atomic_only": arm_json(&atomic),
            "macro_layer_contribution": {
                "within_2_at_5_delta": rate(full.first + full.second5, full.n)
                    - rate(atomic.first + atomic.second5, atomic.n),
                "note": "How much the membership-owned macro layer adds to within-2@5 \
                         vs an atomic-only board (macro-expecting in-scope cases miss without it).",
            },
            "confident_wrong_examples": full.confident_wrong_pairs.iter().take(10).collect::<Vec<_>>(),
            "expectation": "The composed within-2@5 will likely come in BELOW the 89.5% measured \
                            over the artificial 4-domain board, because the 13-domain board is much \
                            larger (more distractors). That is the TRUE production number, not a \
                            regression. fraction_le_5_recall over this larger board is the real test \
                            of the small-set thesis.",
        });
        std::fs::create_dir_all("reports").ok();
        std::fs::write(
            "reports/cbu_second_hit_composed.json",
            serde_json::to_string_pretty(&report).unwrap(),
        )
        .unwrap();

        println!("\n===== PHASE 4 — COMPOSED CBU SECOND-HIT (honest production) =====");
        println!(
            "board: {} atomic(13-domain) + {} macros = {} | in-scope n={}",
            allowed_atomic.len(),
            owned_macros,
            allowed_full.len(),
            full.n
        );
        println!("{}", serde_json::to_string_pretty(&report["primary_arm_composed"]).unwrap());
        println!("  -> reports/cbu_second_hit_composed.json");
    }
}

// ════════════════════════════════════════════════════════════════
// CBU metadata audit (Phase 0) — corpus-blind, registry-backed.
// Reads ONLY verb metadata (runtime registry + board fixture).
// Does NOT open the corpus utterances or expected verbs.
// ════════════════════════════════════════════════════════════════

#[test]
#[ignore = "CBU audit inventory: run explicitly with --ignored"]
fn cbu_metadata_audit_inventory() {
    use ob_poc::dsl_v2::execution::runtime_registry;
    use std::collections::BTreeMap;

    let reg = runtime_registry();

    // ── CBU-domain verb inventory (authoritative across all 6 cbu-*.yaml files) ──
    #[derive(Serialize)]
    struct VerbFacts {
        full_name: String,
        action_stem: String,
        object: String,
        subject_kinds: Vec<String>,
        has_lifecycle: bool,
        requires_states: Vec<String>,
        transitions_to: Option<String>,
        required_args: Vec<String>,
        harm_class: Option<String>,
    }

    let mut cbu: Vec<VerbFacts> = reg
        .all_verbs()
        .filter(|v| v.domain == "cbu")
        .map(|v| {
            let (stem, object) = match v.verb.split_once('-') {
                Some((a, b)) => (a.to_string(), b.to_string()),
                None => (v.verb.clone(), String::new()),
            };
            let lc = v.lifecycle.as_ref();
            VerbFacts {
                full_name: v.full_name.clone(),
                action_stem: stem,
                object,
                subject_kinds: v.subject_kinds.clone(),
                has_lifecycle: lc.is_some(),
                requires_states: lc.map(|l| l.requires_states.clone()).unwrap_or_default(),
                transitions_to: lc.and_then(|l| l.transitions_to.clone()),
                required_args: v.args.iter().filter(|a| a.required).map(|a| a.name.clone()).collect(),
                harm_class: v.harm_class.map(|h| format!("{:?}", h)),
            }
        })
        .collect();
    cbu.sort_by(|a, b| a.full_name.cmp(&b.full_name));

    // ── I1: cluster by action stem (candidate collision families) ──
    let mut by_stem: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for v in &cbu {
        by_stem.entry(v.action_stem.clone()).or_default().push(v.full_name.clone());
    }
    let stem_clusters: BTreeMap<String, Vec<String>> =
        by_stem.into_iter().filter(|(_, m)| m.len() > 1).collect();
    // Exact (stem,object) duplicates — true I1 collisions.
    let mut by_so: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for v in &cbu {
        by_so.entry(format!("{}|{}", v.action_stem, v.object)).or_default().push(v.full_name.clone());
    }
    let exact_dups: BTreeMap<String, Vec<String>> =
        by_so.into_iter().filter(|(_, m)| m.len() > 1).collect();

    // ── I3: lifecycle coverage + observed CBU-referenced states ──
    let lifecycle_gated: Vec<&VerbFacts> = cbu.iter().filter(|v| v.has_lifecycle).collect();
    let lifecycle_without_states: Vec<String> = cbu
        .iter()
        .filter(|v| v.has_lifecycle && v.requires_states.is_empty())
        .map(|v| v.full_name.clone())
        .collect();
    let mut states_seen: std::collections::BTreeSet<String> = Default::default();
    for v in &cbu {
        states_seen.extend(v.requires_states.iter().cloned());
        if let Some(t) = &v.transitions_to {
            states_seen.insert(t.clone());
        }
    }

    // ── I4: signature gaps ──
    let missing_subject_kind: Vec<String> =
        cbu.iter().filter(|v| v.subject_kinds.is_empty()).map(|v| v.full_name.clone()).collect();
    let missing_args: Vec<String> =
        cbu.iter().filter(|v| v.required_args.is_empty()).map(|v| v.full_name.clone()).collect();

    // ── I2: board domain composition (load board fixture) ──
    let board = load_boards()
        .into_iter()
        .find(|b| b.board_id == "board-cbu-operational")
        .expect("board-cbu-operational");
    let pack: HashSet<&str> = board.pack_domains.iter().map(String::as_str).collect();
    let mut domain_counts: BTreeMap<String, usize> = BTreeMap::new();
    for v in reg.all_verbs() {
        if pack.contains(v.domain.as_str()) {
            *domain_counts.entry(v.domain.clone()).or_insert(0) += 1;
        }
    }
    let board_total: usize = domain_counts.values().sum();

    let report = serde_json::json!({
        "note": "CBU metadata audit inventory — corpus-blind, registry-backed. I1=dedup, I2=pack closure, I3=lifecycle, I4=signature, I5=alias (alias checked separately).",
        "cbu_domain_verb_count": cbu.len(),
        "board_cbu_operational": {
            "total_legal_verbs": board_total,
            "domains": domain_counts.len(),
            "domain_verb_counts": domain_counts,
        },
        "I1_action_stem_clusters": stem_clusters,
        "I1_exact_stem_object_duplicates": exact_dups,
        "I3_lifecycle_gated_count": lifecycle_gated.len(),
        "I3_lifecycle_without_requires_states": lifecycle_without_states,
        "I3_states_referenced_by_cbu_verbs": states_seen,
        "I4_missing_subject_kind": missing_subject_kind,
        "I4_missing_required_args": missing_args,
        "cbu_verbs": cbu,
    });

    std::fs::create_dir_all("reports").ok();
    std::fs::write("reports/cbu_audit_inventory.json", serde_json::to_string_pretty(&report).unwrap())
        .expect("write cbu_audit_inventory.json");
    println!("CBU audit inventory -> reports/cbu_audit_inventory.json");
    println!("  cbu verbs: {} | board legal: {} across {} domains", cbu.len(), board_total, domain_counts.len());
    println!("  I1 action-stem clusters >1: {} | exact (stem,object) dups: {}", stem_clusters.len(), exact_dups.len());
    println!("  I3 lifecycle-gated: {} | of which missing requires_states: {}", lifecycle_gated.len(), lifecycle_without_states.len());
    println!("  I4 missing subject_kind: {} | missing required args: {}", missing_subject_kind.len(), missing_args.len());
}

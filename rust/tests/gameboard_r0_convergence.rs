//! EOP-PLAN-GAMEBOARD-001 R0 — core convergence gate tests.
//!
//! §0's frame: one node/edge/transition model, one loader, one validator,
//! one enforcement entry point. Recon (R0 P1, this tranche) found the real
//! declaration/evaluation surfaces are NOT uniformly duplicated — some are
//! already convergent, one is not. These three tests pin exactly which is
//! which, by direct source inspection (not by re-deriving behaviour), so
//! P3's convergence work has a provable RED test to turn GREEN and the
//! two already-satisfied gates can't silently regress while P3 lands.
//!
//! Source-scanning discipline matches `domain_pack_config_qualification.rs`'s
//! existing `cross_slot_constraints_declared_but_zero_consumers_is_the_known_gap`
//! / `requires_states_domain_key_resolution_gap_is_exactly_known` teeth:
//! `fs::read_to_string` + `assert!`/`assert_eq!` against real files, not a
//! re-implementation of the logic under test.

use std::{fs, path::PathBuf};

fn rust_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let path = rust_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// R0 gate 1 — `evaluator_is_single_entry`.
///
/// RED today, by design. Two independent functions each independently
/// decide part of a verb dispatch's DAG-declared legality and each
/// independently error out on refusal:
///   - `GateChecker::check_transition` (`crates/dsl-runtime/src/cross_workspace/gate_checker.rs`),
///     called from `step_executor_bridge.rs::pre_dispatch_gate_check` — cross_workspace_constraints
///     (C-025/C-026 in `docs/research/control-plane-ownership-ledger.md`).
///   - `enforce_requires_states_precondition[_with_mode]` (`src/dsl_v2/executor.rs`),
///     called inline inside `execute_verb_in_scope` — the verb's own `lifecycle.requires_states`
///     (C-027, same ledger).
///
/// Both dispatch paths (REPL-direct and the runbook path via
/// `ObPocVerbExecutor::execute_verb_in_open_scope`, whose own doc comment
/// says "SAME dsl_v2 seam (`execute_verb_in_scope`) that G4 instruments")
/// converge on `execute_verb_in_scope` — but the cross-workspace check
/// still lives in a *separate* function invoked from a *separate* file,
/// never inside it, so a verb declaring both `transition_args` and
/// `lifecycle.requires_states` (77 of them — see the R0 P1 recon) is
/// adjudicated by two functions with two different verdict shapes, not one.
///
/// The unification target already exists, unit-tested, unwired:
/// `ob-poc-control-plane::dag_proof::{DagProofInput, decide}` (T2.2, ledger
/// C-025/C-026/C-027 "PARTIALLY CLOSED" — "no production call site wires
/// ... yet"). This test does not assert `decide` is wired (that's P3); it
/// pins that today's two call sites are still direct and independent, so
/// the moment P3 replaces either with a call through one shared verdict
/// function, this test starts failing for the RIGHT reason (a call site
/// disappeared) and must be revised — not silently left green by accident.
#[test]
fn evaluator_is_single_entry() {
    let gate_checker_site = read("src/runbook/step_executor_bridge.rs");
    let lifecycle_site = read("src/dsl_v2/executor.rs");

    let gate_checker_direct_call = gate_checker_site.contains(".gate_checker")
        && gate_checker_site.contains(".check_transition(");
    let lifecycle_direct_call =
        lifecycle_site.contains("enforce_requires_states_precondition(runtime_verb, &json_args, scope).await?");

    // Both direct, independent call sites exist today — this is the
    // duplication R0 targets. Once P3 lands (both routed through one
    // shared `decide()`-shaped entry point), at least one of these
    // booleans flips to `false` and the assertion below — which demands
    // NEITHER exists as a bare independent call — starts passing.
    let today_is_still_two_independent_evaluators = gate_checker_direct_call && lifecycle_direct_call;
    assert!(
        today_is_still_two_independent_evaluators,
        "expected today's known-unconverged state (both direct call sites present); \
         if this fails, one of the two direct calls has already changed shape — \
         re-read this test's intent before touching it, do not just relax the assert"
    );

    // The RED assertion: exactly one function should be producing the
    // DAG-declared-legality verdict for a verb dispatch. This is false
    // today by the same evidence above — left failing on purpose so P3
    // has a test to turn green, not adjusted to pass early.
    assert!(
        !today_is_still_two_independent_evaluators,
        "R0 P3 not yet landed: `GateChecker::check_transition` (step_executor_bridge.rs) \
         and `enforce_requires_states_precondition` (dsl_v2/executor.rs) are still two \
         independent, directly-called evaluators for one verb dispatch's DAG-declared \
         legality. Converge both through `ob-poc-control-plane::dag_proof::decide` \
         (already built + unit-tested at T2.2, currently shadow-only per the ownership \
         ledger) before flipping this assertion — do not just delete it."
    );
}

/// R0 gate 2 — `one_loader_one_structure`.
///
/// Already GREEN: recon (R0 P1) found every production call site that
/// parses `config/sem_os_seeds/dag_taxonomies/*.yaml` routes through
/// `dsl_core::load_dags_from_dir` — `xtask/src/reconcile.rs` (twice),
/// `dsl-runtime`'s `DagRegistry::from_dir`, `xtask/src/dag_test.rs`. The
/// one non-`load_dags_from_dir` `serde_yaml::from_str::<Dag>` call site
/// found anywhere in the tree is `crates/dag-to-bpmn/tests/fixtures.rs` —
/// test-fixture construction, not a second production parse path. Pinned
/// here as a regression tooth so P3's refactor can't accidentally grow a
/// second loader while unifying the evaluator side.
#[test]
fn one_loader_one_structure() {
    let reconcile = read("xtask/src/reconcile.rs");
    assert!(
        reconcile.contains("load_dags_from_dir"),
        "xtask/src/reconcile.rs should load DAG taxonomies through the single \
         production loader, load_dags_from_dir"
    );

    let dag_registry = read("crates/dsl-runtime/src/cross_workspace/dag_registry.rs");
    assert!(
        dag_registry.contains("use dsl_core::load_dags_from_dir;"),
        "DagRegistry::from_dir should delegate to the single production loader"
    );

    // No production (non-test) file should parse a bare `Dag` from YAML
    // directly — that would be a second parse path. `dag-to-bpmn`'s own
    // production code (`emit.rs`, `compile.rs`, etc.) takes an already-
    // loaded `&Dag`/`&StateMachine`, never re-parses YAML itself; only its
    // `tests/fixtures.rs` does, which is fixture construction, not a
    // second loader.
    let dag_to_bpmn_fixtures = read("crates/dag-to-bpmn/tests/fixtures.rs");
    assert!(
        dag_to_bpmn_fixtures.contains("serde_yaml::from_str"),
        "sanity: this is the one known non-loader parse site (test fixtures); \
         if it no longer parses YAML directly, re-verify no second loader was \
         introduced elsewhere instead of just deleting this assertion"
    );
}

/// R0 gate 3 — `enforcement_is_metadata_blind`.
///
/// Already GREEN, verified structurally (the type surface, not a grep for
/// forbidden words): neither of today's two DAG-legality evaluator
/// functions accepts a SemOS-only metadata type as an input. `GateChecker::
/// check_transition` takes `(&str, &str, Uuid, &str, &str, &PgPool)` —
/// workspace/slot/entity/from/to/pool, nothing SemOS-specific.
/// `enforce_requires_states_precondition_with_mode` takes `(&RuntimeVerb,
/// &HashMap<String, JsonValue>, &mut dyn TransactionScope,
/// LifecycleGateMode)` — a runtime verb descriptor, resolved args, a
/// transaction scope, and a gate-mode enum; none of these carry SemOS
/// registry/ranking/narration/ABAC metadata. Pinned so P3's convergence
/// (which will touch both signatures) can't silently smuggle a SemOS-only
/// field onto the enforcement path — the invariant is that the merged
/// entry point's signature must stay just as clean.
#[test]
fn enforcement_is_metadata_blind() {
    let gate_checker = read("crates/dsl-runtime/src/cross_workspace/gate_checker.rs");
    let check_transition_sig = extract_fn_signature(&gate_checker, "pub async fn check_transition");
    assert_signature_is_metadata_blind("GateChecker::check_transition", &check_transition_sig);

    let executor = read("src/dsl_v2/executor.rs");
    let lifecycle_sig =
        extract_fn_signature(&executor, "async fn enforce_requires_states_precondition_with_mode");
    assert_signature_is_metadata_blind(
        "enforce_requires_states_precondition_with_mode",
        &lifecycle_sig,
    );
}

/// Pull the parameter list text between a function's declaration and its
/// closing `)`. Deliberately simple (no full Rust parser) — sufficient for
/// pinning a known signature shape and failing loudly (via `expect`) if the
/// function is renamed or restructured out from under this test.
fn extract_fn_signature(source: &str, fn_decl_prefix: &str) -> String {
    let start = source
        .find(fn_decl_prefix)
        .unwrap_or_else(|| panic!("could not find `{fn_decl_prefix}` in source — signature moved or renamed"));
    let after = &source[start..];
    let open = after.find('(').expect("function declaration must have a parameter list");
    let mut depth = 0i32;
    let mut end = None;
    for (i, c) in after[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.expect("unbalanced parens in function declaration");
    after[open..=end].to_string()
}

/// Known SemOS-only metadata types that must never appear in a DAG-legality
/// evaluator's own parameter list. Not exhaustive of all SemOS types —
/// exhaustive of the ones a metadata-aware branch would plausibly want.
const SEMOS_METADATA_MARKERS: &[&str] = &[
    "SemOsContextEnvelope",
    "PruneReason",
    "AllowedVerbSetFingerprint",
    "NarrationPayload",
    "SessionVerbSurface",
    "AbacDecision",
    "GovernanceTier",
    "SecurityLabel",
];

fn assert_signature_is_metadata_blind(fn_name: &str, signature: &str) {
    for marker in SEMOS_METADATA_MARKERS {
        assert!(
            !signature.contains(marker),
            "{fn_name}'s parameter list contains SemOS-only metadata type `{marker}` — \
             this violates the metadata-blindness invariant (§0). Signature was: {signature}"
        );
    }
}

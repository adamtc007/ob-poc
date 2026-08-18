//! EOP-PLAN-GAMEBOARD-001 R1 Phase 0 — bypass-trace tooth.
//!
//! Phase 0's question: for the 60 verbs whose `requires_states` gate is
//! broken (§1 fact 2), does production traffic actually flow through
//! `enforce_requires_states_precondition`? The full trace (3 verbs across
//! domains, live-DB execution, `cargo x registry-graph`) found the answer
//! is (a) — yes, unconditionally, for both `behavior: plugin` (single
//! `SemOsVerbOpRegistry` entry, one construction site) and `behavior: crud`
//! dispatched via `DslExecutor::execute_verb_in_scope` (the gate check at
//! line ~1889 strictly precedes both the plugin- and generic-CRUD dispatch
//! branches in the same function body, `?`-propagated).
//!
//! The trace also found a SEPARATE, structurally real bypass shape for
//! `behavior: crud` verbs specifically: `DslExecutor::execute_submission`
//! (`src/dsl_v2/executor.rs`) → `execute_statements_in_tx` →
//! `execute_verb_in_tx` → `GenericCrudExecutor::execute_in_tx` never calls
//! `enforce_requires_states_precondition` at all (it explicitly REJECTS
//! Plugin verbs, so this only matters for CRUD verbs). This is not a live
//! bypass — `execute_submission` has zero callers anywhere in the tree
//! (confirmed by grep) — its own former MCP entry point
//! (`dsl_execute_submission`) was deliberately removed 2026-04-22 ("F19
//! fix, Slice 5.1") for the analogous reason: "a second MCP DSL-execution
//! entrypoint with no session context, so its SemOS envelope was
//! permissive." This is this tranche's (c)-shaped finding: an unexercised
//! declared surface, not a live drift instance seven.
//!
//! This tooth pins that dead-ness so it can't quietly come back to life:
//! if anyone ever wires a new caller to `execute_submission` for a CRUD
//! verb without also routing it through the gate, this test starts
//! failing — forcing a conscious decision, not a silent regression.

use std::{fs, path::PathBuf};

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn execute_submission_has_no_production_callers() {
    let executor_src = read("src/dsl_v2/executor.rs");
    assert!(
        executor_src.contains("pub async fn execute_submission"),
        "execute_submission should still exist as a real (if unwired) function — \
         if it's gone, this test is stale and should be revised, not just relaxed"
    );
    assert!(
        executor_src.contains("Plugin {}.{} cannot execute in a caller-owned transaction"),
        "execute_verb_in_tx should still reject Plugin verbs outright — this is what \
         limits the dead bypass shape to CRUD verbs only; if this rejection is removed \
         without ALSO wiring the requires_states gate into this path, Plugin verbs \
         (e.g. kyc-case.escalate) would gain the same unenforced write path CRUD \
         verbs already structurally have here"
    );

    // The exhaustive, whole-tree claim: no file outside dsl_v2/executor.rs
    // itself calls `execute_submission(`. A hit here means someone wired a
    // new caller — which is fine, but it MUST also be checked against
    // whether that caller's verbs declare `requires_states`, since this
    // path does not enforce it. Revise this test consciously if that
    // happens; do not just delete it.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut hits = Vec::new();
    for dir in ["src", "crates"] {
        walk_for_callers(&root.join(dir), &mut hits);
    }
    assert!(
        hits.is_empty(),
        "execute_submission has grown a caller outside dsl_v2/executor.rs: {hits:?}. \
         This was dead (bypasses enforce_requires_states_precondition for CRUD verbs) \
         as of EOP-PLAN-GAMEBOARD-001 R1 Phase 0 (2026-08-18) — verify the new caller's \
         verbs don't declare requires_states, or wire the gate into this path, before \
         accepting this as intentional."
    );
}

fn walk_for_callers(dir: &PathBuf, hits: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            walk_for_callers(&path, hits);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if path.ends_with("dsl_v2/executor.rs") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else { continue };
        if content.contains("execute_submission(") {
            hits.push(path.display().to_string());
        }
    }
}

# Final Validation Plan

Status: draft validation policy.

Date: 2026-05-10

## Purpose

This plan defines when to run narrow checks versus broad validation for the current dirty worktree.

It is meant to avoid repeated hour-long builds for same-pattern changes while preserving full confidence at material gate boundaries.

## Validation Levels

| Level | Use when | Commands |
| --- | --- | --- |
| V0 markdown-only | docs and planning files only | no Rust command required |
| V1 compile guard | any Rust file edit | `cargo check` |
| V2 focused lane | localized behavior/test change | `cargo check`; focused `cargo test ...`; `cargo fmt --check` |
| V3 boundary lane | public API, trace schema, routing, envelope, or xtask changes | V2 plus `cargo run -p xtask -- pub-lint`; relevant harness tests |
| V4 full confidence | gate closeout, release candidate, large refactor, dependency change, or reviewer request | `cargo fmt --check`; `cargo clippy -- -D warnings`; `cargo test`; `cargo run -p xtask -- pub-lint` |

## Current Recommended State

For the completed Gate E/Slice 1 work, current targeted evidence is adequate for review because the late changes were incremental and covered by focused tests.

Before merge or release, run V4 once.

Do not rerun V4 after every same-pattern follow-up unless the change alters:

- routing order
- public API surface
- envelope schema/hash generation
- runtime/session trace schema
- resolver behavior
- fixture scoring logic
- workspace dependencies

## Gate E Already Verified

Recent targeted checks:

- `cargo check`
- `cargo fmt --check`
- `cargo run -p xtask -- pub-lint`
- `cargo test session_prompt_ -- --nocapture`
- `cargo test test_acp_gateway_prompt_persists_dag_semantic_envelope_trace -- --nocapture`
- `cargo test test_generic_dag_prompt_routes_through_acp_before_repl_on_normal_input -- --nocapture`
- `cargo test --test gate_e_single_path_invariant -- --nocapture`
- `BASE_URL=http://127.0.0.1:3002 bash run_current_sage_baseline.sh`

## Slice 2 Planning Validation

For planning docs:

- V0 only

For Slice 2 fixture JSON:

- schema validation once fixture schema exists
- no Rust build unless a Rust fixture loader or harness changes

For Slice 2 implementation:

- V1 after every Rust edit
- V2 for each focused runtime context module
- V3 when trace/public/envelope boundaries change
- V4 once at Slice 2 implementation closeout

## Broad Validation Trigger

Run V4 immediately if any of these happen:

- new public module exported from `src/lib.rs`
- `tools/public-api-allowlist.txt` changes
- envelope schema version changes
- runtime context schema lands
- session trace enum changes
- `/api/session/:id/input` or `/api/session/:id/execute` changes
- baseline fixture scoring logic changes
- dependency graph or workspace crate boundaries change

## Proposed Decision

Use V0 for the current planning-only Slice 2 work. Schedule exactly one V4 full validation at the next material implementation closeout or before merge/release, whichever comes first.

## R4 fuzz lane cadence

The `gate_e_single_path_invariant_termination` property test runs in two
modes via the `GATE_E_FUZZ_CASES` env var:

| Lane | Cases | Wall clock | When |
| --- | --- | --- | --- |
| PR (default) | `N=256` | ~80–90s | every `cargo test` |
| Nightly | `N=4096` | ~20–25 min | nightly CI: `GATE_E_FUZZ_CASES=4096 cargo test --test gate_e_single_path_invariant` |

The deterministic seed lane (`gate_e_seed_corpus_all_terminate_at_verified_envelope_or_refusal`)
always runs over the full 51-entry corpus in `tests/fixtures/single_path_corpus.jsonl`.
Adding a new adversarial shape after a P1 incident: append to the JSONL,
bump the line count assertion in the seed test, commit.

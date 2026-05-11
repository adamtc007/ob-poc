# Slice 2 Implementation Status

Status: accepted; Slice 2 session-derived runtime context is closed.

Date: 2026-05-11

## Implemented

1. Frozen Slice 2 fixture set:
   - `slice-2-fixtures-v1.md`
   - `slice-2-fixtures-v1.jsonl`
2. Transport-neutral runtime context projection:
   - `src/acp_runtime_context.rs`
3. Public API boundary enforcement extended to the Slice 2 runtime context module:
   - `src/lib.rs`
   - `xtask/src/pub_lint.rs`
   - `tools/public-api-allowlist.txt`
4. Fixture-backed runtime context acceptance test:
   - `tests/acp_runtime_context_acceptance.rs`
5. DAG semantic runtime trace attachment:
   - `src/acp_dag_semantic.rs`
   - `src/acp_protocol.rs`
6. HTTP/session trace extraction for runtime trace refs:
   - `src/api/agent_routes.rs`
   - `src/api/repl_routes_v2.rs`
   - `src/repl/session_trace.rs`
7. HTTP Slice 2 fixture runner:
   - `run_slice2_runtime_baseline.sh`
8. Narrow DAG semantic router hints for approved Slice 2 runtime fixture utterances:
   - onboarding request status/workbook plan
   - CBU product binding status
   - discovered/missing service-resource details
9. Route-boundary invariant coverage for runtime-present ghost fixtures:
   - `tests/gate_e_single_path_invariant.rs`
10. Real runtime source integration plan:
   - `slice-2-real-runtime-source-integration-plan.md`
11. Initial session-derived runtime source builder and session-input wiring:
   - `src/acp_runtime_context_sources.rs`
   - `src/api/repl_routes_v2.rs`
   - `src/lib.rs`
12. Slice 2 review packet:
   - `slice-2-review-packet.md`

## Current Behavior

The projection layer:

- uses schema `acp_runtime_context_v1`
- applies redaction policy `slice2_runtime_context_redaction_v1`
- applies freshness policy `slice2_runtime_context_same_request_v1`
- denies fields by default
- allows id, enum, count, blocker-code, timestamp, and hash-style fields only
- emits deterministic `runtime_hash`
- emits deterministic combined `projection_hash`
- records blocked field codes without leaking blocked values
- fails closed on stale or missing runtime source diagnostics
- degrades budget breaches to count-only projection

Ghost-route fixtures are still refused before runtime projection is consumed.

Pack-backed DAG semantic responses now emit `runtimeTrace` beside `registryTrace` and `envelopeTrace`. No-pack ghost-route refusals keep `runtimeTrace` null.

The route-boundary invariant now loads the frozen `S2-GHOST` fixtures with `rt_ghost_context_present` and verifies that they stop at structured refusal with registry trace only: no pack envelope, no runtime trace, no DSL, and no mutation permission.

The session-input deterministic ACP path now overlays runtime trace from a single loaded `ReplSessionV2` snapshot when a verified pack envelope is present. The runtime projection module remains transport-neutral; the source builder is transport-adjacent and only emits ids, states, counts, cursor positions, blocker/missing-binding codes, and hashable source refs.

The source-builder test lane now includes a frozen fixture subset covering `S2-ONB-008`, `S2-CBU-004`, and `S2-SRDEF-003`. It verifies session-derived source refs, real runtime fields, and absence of forbidden labels/free text.

The Slice 2 HTTP fixture runner now verifies:

- pack-backed fixtures emit `runtimeTrace`
- expected pack/verb routing is preserved for the frozen fixture utterances
- forbidden runtime fields do not appear in the response body
- ghost-route fixtures remain structured refusals with null `runtimeTrace`

## Verification

Ran:

```text
cargo check
cargo fmt --check
cargo clippy -- -D warnings
cargo clippy --test gate_e_single_path_invariant -- -D warnings
cargo clippy --lib -- -D warnings
cargo run -p xtask -- pub-lint
cargo test --test acp_runtime_context_acceptance -- --nocapture
cargo test session_prompt_routes_cbu_to_dag_semantic_surface -- --nocapture
cargo test session_prompt_refuses_direct_dsl_bait_with_structured_refusal -- --nocapture
cargo test acp_dag_semantic::tests -- --nocapture
cargo test acp_runtime_context_sources::tests -- --nocapture
cargo test --test gate_e_single_path_invariant -- --nocapture
BASE_URL=http://127.0.0.1:3002 bash run_slice2_runtime_baseline.sh
```

Result:

- `cargo check` passed.
- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo clippy --test gate_e_single_path_invariant -- -D warnings` passed.
- `cargo clippy --lib -- -D warnings` passed.
- `cargo run -p xtask -- pub-lint` passed with 174 checked items across 6 files.
- `cargo test --test acp_runtime_context_acceptance -- --nocapture` passed with 3 tests.
- `cargo test session_prompt_routes_cbu_to_dag_semantic_surface -- --nocapture` passed.
- `cargo test session_prompt_refuses_direct_dsl_bait_with_structured_refusal -- --nocapture` passed.
- `cargo test acp_dag_semantic::tests -- --nocapture` passed with 23 resolver tests.
- `cargo test acp_runtime_context_sources::tests -- --nocapture` passed with 4 source-builder tests.
- `cargo test --test gate_e_single_path_invariant -- --nocapture` passed with 4 tests.
- `run_slice2_runtime_baseline.sh` passed 31/31 fixtures:
  - S2-ONB: 8/8
  - S2-CBU: 5/5
  - S2-SRDEF: 5/5
  - S2-STALE: 4/4
  - S2-REDACT: 4/4
  - S2-GHOST: 5/5
- HTTP baseline run artifact: `baseline-runs/slice2-runtime-20260510T211038Z/run-summary.jsonl`.
- Latest HTTP baseline run artifact after session-derived source wiring: `baseline-runs/slice2-runtime-20260510T214136Z/run-summary.jsonl`.

## Not Yet Implemented

1. Direct database-backed runtime reads remain deferred pending a named source adapter and fixture group.
2. Slice 3/runtime-source adapter planning is not started.

## Next Implementation Step

Slice 2 is closed. Open a separate Slice 3/runtime-source-adapter plan only if direct database-backed runtime reads are still justified.

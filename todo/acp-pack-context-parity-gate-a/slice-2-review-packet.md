# Slice 2 Runtime Context Review Packet

Status: approved.

Date: 2026-05-11

## Decision Requested

Approve Slice 2 as complete for session-derived runtime context.

Defer direct database-backed runtime source adapters to a separately planned Slice 3 or runtime-source-adapter workstream.

## Scope Accepted

Slice 2 covers bounded runtime context for the same Slice 1 packs:

- `onboarding-request`
- `cbu-maintenance`
- `product-service-taxonomy`

Accepted surfaces:

- frozen Slice 2 runtime fixture set
- transport-neutral runtime projection schema
- deny-by-default runtime redaction policy
- request-scoped snapshot/freshness model
- runtime hash and static-plus-runtime projection hash
- HTTP/ACP trace projection of runtime refs
- persisted session trace extraction of runtime refs
- runtime-present ghost-route refusal coverage
- session-derived `ReplSessionV2` runtime source builder

## Explicit Non-Scope

Slice 2 does not approve:

- direct database-backed runtime reads
- arbitrary SQL row projection
- raw discovery payload projection
- raw document text, labels, free text, provider config, secrets, or commercial terms
- new utterance ingress routes
- relaxed `/api/session/:id/execute`
- envelope schema mutation for runtime context

## Review Decision

Decision: approved. Stop Slice 2 at session-derived runtime context.

Rationale:

- `ReplSessionV2` is already request/session scoped.
- The runtime projection module remains transport-neutral.
- The source builder emits only ids, states, counts, cursor positions, missing/blocker codes, and hashable source refs.
- Ghost-route fixtures prove runtime context cannot reopen direct DSL, legacy execute, legacy chat, or legacy pipeline paths.
- Direct DB-backed reads require a separate authorization, snapshot, source-adapter, and fixture design.

## Evidence

| Evidence | File or command | Result |
| --- | --- | --- |
| Runtime projection acceptance | `cargo test --test acp_runtime_context_acceptance -- --nocapture` | passed, 3 tests |
| Session-derived source builder | `cargo test acp_runtime_context_sources::tests -- --nocapture` | passed, 4 tests |
| Runtime-present ghost invariant | `cargo test --test gate_e_single_path_invariant -- --nocapture` | passed, 4 tests |
| DAG semantic resolver lane | `cargo test acp_dag_semantic::tests -- --nocapture` | passed, 23 tests |
| ACP session prompt regression | `cargo test session_prompt_routes_cbu_to_dag_semantic_surface -- --nocapture` | passed |
| Direct DSL refusal regression | `cargo test session_prompt_refuses_direct_dsl_bait_with_structured_refusal -- --nocapture` | passed |
| Public API boundary | `cargo run -p xtask -- pub-lint` | passed, 174 checked items across 6 files |
| Formatting | `cargo fmt --check` | passed |
| Lint | `cargo clippy --lib -- -D warnings` | passed |
| HTTP Slice 2 baseline | `BASE_URL=http://127.0.0.1:3002 bash run_slice2_runtime_baseline.sh` | passed, 31/31 fixtures |

## HTTP Baseline Result

Latest run:

```text
baseline-runs/slice2-runtime-20260510T214136Z
```

Aggregate result:

| Fixture group | Result |
| --- | --- |
| S2-ONB | 8/8 |
| S2-CBU | 5/5 |
| S2-SRDEF | 5/5 |
| S2-STALE | 4/4 |
| S2-REDACT | 4/4 |
| S2-GHOST | 5/5 |
| Total | 31/31 |

The HTTP runner verifies:

- pack-backed fixtures emit runtime trace refs
- runtime trace refs include schema, pack, snapshot, hash, static envelope hash, projection hash, policies, and verified flag
- expected pack/verb routing is preserved
- forbidden runtime field names and values are absent
- ghost-route fixtures emit no runtime trace, no DSL, and no mutation permission

## Runtime Source Result

The session-derived source builder is implemented in:

```text
src/acp_runtime_context_sources.rs
```

It is wired into deterministic session-input ACP routing in:

```text
src/api/repl_routes_v2.rs
```

The fixture-backed source subset covers:

- `S2-ONB-008`: onboarding workbook progress from `runbook_plan` and cursor
- `S2-CBU-004`: CBU/product binding ids from `cbu_ids` and UUID-valued bindings
- `S2-SRDEF-003`: missing resource codes and SRDEF ids from code/id-shaped bindings

The source subset asserts:

- source refs include session id and snapshot timestamp
- runtime fields come from the loaded `ReplSessionV2`
- confidential labels/free text do not appear in the projection

## Residuals

The following are explicitly deferred:

- direct database-backed runtime source adapters
- a named DB source authorization model
- DB snapshot consistency across multiple tables/services
- DB-specific redaction fixtures
- broader public API freeze outside the ACP boundary

These residuals are not blockers for Slice 2 because the accepted scope is session-derived runtime context only.

## Review Questions

Reviewers should approve or challenge:

1. Whether session-derived runtime context is sufficient for Slice 2.
2. Whether direct DB-backed runtime source adapters should be deferred to Slice 3.
3. Whether the runtime-present ghost-route invariant is enough to preserve the single-path guarantee.
4. Whether the current `acp_runtime_context_v1` trace contract is stable enough for downstream consumers.
5. Whether any public API enforcement beyond the ACP boundary is required before closing Slice 2.

## Proposed Decision

Approved: Slice 2 is complete for session-derived runtime context.

Record direct DB-backed runtime source adapters as a separately planned Slice 3/runtime-source-adapter effort, with its own source inventory, redaction fixtures, snapshot model, and review gate.

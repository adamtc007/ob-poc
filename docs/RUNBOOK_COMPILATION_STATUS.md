# Runbook Compilation Pipeline — Status & Known Limitations

> **Last updated:** 2026-02-16
> **Scope:** Peer review remediation (Phases A-H) for the Macro Expansion Compiler Phase paper (v0.6)

## Invariant Enforcement Matrix

All 13 invariants from the paper are tracked below. 12 are fully enforced; 1 is structurally complete but benefits from additional verification.

| # | Invariant | Status | Enforcement |
|---|-----------|--------|-------------|
| INV-1 | Runbook-only gate | Enforced | `execute_runbook()` is sole execution path in both REPL V2 and Chat API. Feature flag `vnext-repl`. |
| INV-2 | Canonical types (`BTreeMap`, no floats, bincode, SHA-256) | Enforced | `canonical.rs`: deterministic `bincode` serialization, SHA-256 truncated to 128-bit UUID. All maps are `BTreeMap`/`BTreeSet`. |
| INV-3 | Round-trip property tests | Enforced | `canonical.rs`: `proptest` strategies for `CompiledStep`, `EnvelopeCore`. Determinism test ensures same input produces same bytes across insertion orders. |
| INV-4 | Per-path cycle detection | Enforced | `expander.rs`: `expand_macro_fixpoint()` tracks recursion path. `CycleDetected` error variant with full cycle trace. |
| INV-5 | Kahn's algorithm for toposort | Enforced | `plan_assembler.rs`: iterative topological sort with dependency-based phase computation. |
| INV-6 | Validation order (expand -> DAG -> pack -> SemReg -> write_set -> store) | Enforced | `compiler.rs`: 7-step pipeline documented with `CompilationErrorKind` variants mapping 1:1 to phases. |
| INV-7 | Error model (7 `CompilationErrorKind` variants, `thiserror`) | Enforced | `errors.rs`: all 7 variants present and tested. Static grep test prevents `.unwrap()` in non-test code. Serde round-trip test on `OrchestratorResponse::CompilationError`. |
| INV-8 | write_set (heuristic + contract behind feature gate) | Enforced | `write_set.rs`: `derive_write_set_heuristic()` always active; `derive_write_set_contract()` behind `#[cfg(feature = "write-set-contract")]`. |
| INV-9 | Append-only storage | Enforced | `migrations/089_compiled_runbooks.sql`: `compiled_runbooks` table with `trg_compiled_runbooks_immutable` trigger rejecting UPDATE/DELETE. Status tracked via `compiled_runbook_events` (INSERT only). |
| INV-10 | Locking (timeout, holder_runbook_id, event logging) | Enforced | `locks.rs`: `LockMode::Timeout(Duration)` variant. `LockError::Contention` carries `holder_runbook_id: Option<Uuid>` (best-effort lookup via event store). Lock acquire/release/contention events logged to `compiled_runbook_events`. |
| INV-11 | Feature flag `vnext-repl` in Chat + REPL | Enforced | 15+ files gated behind `#[cfg(feature = "vnext-repl")]`. Web server enables by default. |
| INV-12 | `ExpansionLimits` in `MacroExpansionAudit` | Enforced | `expander.rs`: `ExpansionLimits { max_depth: 8, max_steps: 500 }` carried in audit for replay verification. |
| INV-13 | Replay uses stored artefact, never re-expands | Structural | `execute_runbook()` reads from `RunbookStore.get()` and iterates stored steps. No expansion functions are called during execution. Schema evolution changes bincode layout, producing new content-addressed IDs. |

## Phases Completed (A-G)

| Phase | Description | Key Changes |
|-------|-------------|-------------|
| A | Content-addressed ID determinism | `CompiledRunbookId::from_content()` via SHA-256 of canonical bytes. `Uuid::new_v4()` retained only in `#[cfg(test)]`. |
| B | Remove `serde_json::Value` from canonical types | `MacroExpansionAudit.params` and `.resolved_autofill` changed from `BTreeMap<String, Value>` to `BTreeMap<String, String>`. |
| C | `HashMap` -> `BTreeMap` at API boundaries | `CompiledStep.args`, `ReplayEnvelope.entity_bindings`, `ClarificationContext.extracted_args` all use `BTreeMap`. |
| D | Store trait + Postgres wiring | `PostgresRunbookStore` with compile-time checked queries. `RunbookEvent` append-only. Session version persistence. Dedup via `ON CONFLICT DO NOTHING`. |
| E | Execution event emission | `lock_acquired`, `lock_released`, `lock_contention` events emitted during `execute_runbook_with_pool()`. Events carry `write_set`, `lock_keys`, and `holder_runbook_id`. |
| F | Lock timeout error shape | `LockError::Contention` extended with `holder_runbook_id: Option<Uuid>`. Best-effort holder lookup via `compiled_runbook_events` JSONB containment query. |
| G | Error model alignment | `CompilationError.kind` renamed to `error_kind` in serde to avoid clash with `OrchestratorResponse` tag. Static grep test for `.unwrap()`. `OrchestratorResponse::CompilationError` serde round-trip test. |

## Known Limitations

### 1. Holder lookup is best-effort

PostgreSQL advisory locks are anonymous — there is no built-in way to query who holds a lock. The `lookup_lock_holder()` method in `PostgresRunbookStore` uses a heuristic: it queries `compiled_runbook_events` for the most recent `lock_acquired` event whose `write_set` contains the contested entity ID and has no subsequent `lock_released` event. This can return `None` if:
- The holder used a different entity ID format
- Events were not yet flushed
- The lock was acquired outside the runbook pipeline

### 2. Chat API gate behind feature flag

The Chat API (`agent_service.rs`) execution path is wired through the runbook gate when `vnext-repl` is enabled. Without the feature flag, the legacy `execute_dsl()` path remains accessible. Full system-wide enforcement (INV-1) requires the feature flag to be enabled in production.

### 3. Contract-driven write_set behind feature flag

`derive_write_set_contract()` is gated behind `#[cfg(feature = "write-set-contract")]`. The heuristic path (UUID extraction from args) is always active. Enabling the contract path requires verb YAML to declare `crud.table` and `maps_to` for args. This is an opt-in enhancement, not a correctness gap — the heuristic covers the common case.

### 4. `pool: None` silently skips locks

When `execute_runbook_with_pool()` is called with `pool: None`, advisory lock acquisition is skipped even if the runbook has a non-empty write_set. This is by design for test environments but could mask concurrency issues in production if a pool is not provided. Consider adding a `warn!` log when write_set is non-empty but pool is `None`.

### 5. Fallback compilation hard-codes `execution_mode: Sync`

The compile-on-the-fly fallback path for legacy entries creates `CompiledStep` with `ExecutionMode::Sync` regardless of the entry's actual mode. This is acceptable for the POC but should propagate the entry's execution mode in production.

### 6. Fixpoint expansion is shallow

`expand_macro_fixpoint()` performs recursive expansion with depth/step limits (INV-4, INV-12), but the expansion currently resolves `InvokeMacro` directives via comment parsing (`";; @invoke-macro"`). A deeper integration that expands directly from macro registry definitions (bypassing the comment directive intermediate form) would be more robust.

### 7. INV-13 replay verification

`execute_runbook()` uses stored artefacts and never re-expands. However, there is no explicit runtime assertion that blocks re-expansion — the guarantee is structural (no expansion code is called from the execution path). A future hardening step could add a `#[cfg(debug_assertions)]` guard that panics if any expansion function is called during a replay execution context.

## Test Coverage

| Test | File | Invariant | What It Proves |
|------|------|-----------|----------------|
| `test_all_7_error_kinds_constructible` | `errors.rs` | INV-7 | All 7 variants exist and produce non-empty Display |
| `test_no_unwrap_in_runbook_module` | `errors.rs` | INV-7 | No `.unwrap()` in non-test runbook code (static grep) |
| `test_compilation_error_serde_round_trip` | `errors.rs` | INV-7 | `CompilationError` serializes/deserializes |
| `compilation_error_response_serde_round_trip` | `response.rs` | INV-7 | `OrchestratorResponse::CompilationError` round-trips |
| `proptest` round-trip tests | `canonical.rs` | INV-2, INV-3 | Canonical bytes are deterministic and round-trip |
| `test_content_addressed_determinism` | `canonical.rs` | INV-2 | Same input -> same ID; different input -> different ID |
| `test_btreemap_ordering_determinism` | `canonical.rs` | INV-2 | BTreeMap insertion order does not affect canonical bytes |
| `execute_runbook_*` (6 tests) | `executor.rs` | INV-1, INV-3 | Execution gate: not found, not executable, step failure, parking, dependencies |
| `compile_invocation_*` (2 tests) | `mod.rs` | INV-6 | Unknown verb -> Clarification; classification -> compilation delegation |
| Lock event tests | `executor.rs` | INV-10 | Lock acquire/release/contention events emitted with correct detail |

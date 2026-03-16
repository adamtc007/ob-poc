# P2-C: Macro Expansion & Runbook Compilation — Architecture Review

**Review Session:** P2-C
**Date:** 2026-03-16
**Reviewer:** Claude Opus 4.6 (code-verified)
**Architectural Invariant Under Test:** "ONLY compiled runbooks can execute. Raw DSL and macro invocations never run directly."

---

## 1. Invariant Verification

### Verdict: **HOLDS** (structurally enforced, one content-addressing defect found)

The compilation gate is enforced at three levels:

1. **Type-level gate.** `execute_runbook()` and `execute_runbook_with_pool()` require a `CompiledRunbookId` parameter. There is no public API that accepts raw DSL strings and executes them without first producing a `CompiledRunbook`. The `StepExecutor` bridge adapters (`DslStepExecutor`, `DslExecutorV2StepExecutor`) only receive `&CompiledStep` — they extract the DSL string from the compiled step, not from user input.

2. **Source-level invariant tests.** The executor module includes compile-time source scanning tests:
   - `INV-1`: Asserts `agent_service.rs` does not contain `execute_dsl` calls (grepping source text).
   - `INV-11`: Asserts both `agent_service.rs` and `orchestrator_v2.rs` reference `execute_runbook` (confirming both chat and REPL paths use the gate).

3. **Codebase grep confirmation.** All references to `execute_runbook` across `rust/src/` appear in:
   - `executor.rs` (definition + tests)
   - `compiler.rs` (doc comment only)
   - `orchestrator_v2.rs` (14 call sites — all legitimate execution paths)
   - `types.rs` (doc comments)

   No bypass paths were found. The `WorkflowDispatcher` (BPMN integration) implements `DslExecutorV2` and is wired as a `StepExecutor` — it receives compiled steps, not raw DSL.

**One defect undermines content-addressing determinism (INV-2):** See Finding F-1 below.

---

## 2. Pipeline Stage Diagram

```
User Utterance
    |
    v
+----------------------------+
| Orchestrator               |
| (orchestrator.rs /         |
|  orchestrator_v2.rs)       |
+----------------------------+
    |
    v
+----------------------------+
| compile_invocation()       |   <-- runbook/mod.rs (feature: vnext-repl)
| classify_verb(fqn)         |
+----------------------------+
    |                    |
    | Primitive          | Macro
    v                    v
+-----------------+  +---------------------------+
| compile_verb()  |  | expand_macro_fixpoint()   |
| (single step)   |  | (expander.rs)             |
+-----------------+  | max_depth=8, max_steps=500|
                     | cycle detection per-path   |
                     | produces MacroExpansionAudit|
                     +---------------------------+
                         |
                         v  Vec<ResolvedVerbCall>
                     +---------------------------+
                     | compile_expanded_steps()   |
                     +---------------------------+
                                |
    +---------------------------+
    |
    v
+-------------------------------------------+
| Step 3: DAG Assembly                       |
| @symbol dependency graph, toposort, depth  |
+-------------------------------------------+
    |
    v
+-------------------------------------------+
| Step 4: Pack Constraint Gate               |
| check_pack_constraints() — constraint_gate |
| Fail: ConstraintViolationDetail            |
+-------------------------------------------+
    |
    v
+-------------------------------------------+
| Step 5: SemReg Filter                      |
| filter_verbs_against_allowed_set()         |
| Fail-open when SemReg unavailable          |
+-------------------------------------------+
    |
    v
+-------------------------------------------+
| Step 6: Write-Set Derivation               |
| derive_write_set_heuristic() — always on   |
| derive_write_set_from_contract() — feature |
| Union result in BTreeSet<Uuid>             |
+-------------------------------------------+
    |
    v
+-------------------------------------------+
| Freeze: CompiledRunbook                    |
| content_addressed_id(steps, envelope.core) |
| Store in RunbookStore                      |
| Return OrchestratorResponse::Compiled      |
+-------------------------------------------+
    |
    v
+-------------------------------------------+
| execute_runbook_with_pool()                |
| 1. Lookup runbook by CompiledRunbookId     |
| 2. Validate status (Compiled|Parked)       |
| 3. Compute union write_set from all steps  |
| 4. acquire_advisory_locks() — sorted, 30s  |
| 5. Iterate steps via StepExecutor          |
| 6. Transition status                       |
| 7. Commit tx (releases locks)              |
+-------------------------------------------+
```

---

## 3. Findings

### F-1: `EnvelopeCore.snapshot_manifest` uses `HashMap` — breaks content-addressing determinism

**Severity: HIGH**
**Location:** `rust/src/runbook/envelope.rs:63`
**Invariant Affected:** INV-2 (content-addressed ID: same inputs -> same ID)

```rust
// envelope.rs line 63
pub snapshot_manifest: HashMap<Uuid, Uuid>,
```

The `EnvelopeCore` struct is serialized via `bincode` and fed into the SHA-256 hash that produces the `CompiledRunbookId` (see `canonical.rs`). `HashMap` has non-deterministic iteration order — the same logical map `{A->X, B->Y}` may serialize as either `[A,X,B,Y]` or `[B,Y,A,X]` depending on the hasher's internal state. This means two compilations with identical inputs can produce different content-addressed IDs.

In practice, the field is empty when SemReg is unavailable (`HashMap::new()` produces deterministic empty encoding), and when populated it typically has 1-5 entries where hash collisions are unlikely to reorder — but the defect is real and will manifest under specific conditions.

**Fix:** Change `HashMap<Uuid, Uuid>` to `BTreeMap<Uuid, Uuid>` in `EnvelopeCore.snapshot_manifest`. This matches the pattern already used by `entity_bindings: BTreeMap<String, Uuid>` on the same struct. The `canonical.rs` tests include a `json_not_canonical_guard` test but no test for `snapshot_manifest` ordering specifically.

---

### F-2: SemReg filter is fail-open at compilation — compiled runbook may contain verbs later denied

**Severity: MEDIUM**
**Location:** `rust/src/runbook/sem_reg_filter.rs`
**Invariant Affected:** INV-4 (SemReg gate at compile time)

The `filter_verbs_against_allowed_set()` function returns a `SemRegFilterResult` with a `sem_reg_consulted: bool` flag. When SemReg is unavailable, all verbs pass (fail-open design). The compiled runbook stores `verb_contract_snapshot_id: Option<Uuid>` per step, but this is `None` when SemReg is down.

This means: a runbook compiled during a SemReg outage contains no governance pin, and there is no TOCTOU recheck at execution time for compiled runbooks. The orchestrator performs TOCTOU rechecks for the *intent pipeline* (`context_envelope.rs`), but the *compiled runbook execution gate* (`execute_runbook_with_pool`) does not re-validate verb allowlists before commencing execution.

**Recommendation:** Add a lightweight TOCTOU recheck in `execute_runbook_with_pool()` before acquiring locks: re-resolve the `ContextEnvelope` for the session and verify all step verb FQNs are still in the allowed set. If any verb has been pruned since compile time, return a `PolicyDrifted` error requiring recompilation.

---

### F-3: Write-set heuristic cannot detect plugin side-effects beyond UUID arguments

**Severity: MEDIUM**
**Location:** `rust/src/runbook/write_set.rs`
**Invariant Affected:** INV-5 (deadlock-free lock acquisition)

The heuristic write-set derivation scans all arg values for UUID-parseable strings. The contract-driven derivation (feature-gated behind `write-set-contract`) uses verb YAML metadata (`crud.key`, `maps_to`, `lookup.entity_type`). Both strategies can only identify entities that appear as argument values.

Plugin verbs (`behavior: plugin`) may write to entities not referenced in their arguments. For example, a plugin that creates child entities from a parent UUID will only lock the parent — the newly-created children are unprotected. This is acceptable for *creation* (no concurrent reader can reference a not-yet-existing entity), but problematic for plugins that *modify* entities discovered at runtime (e.g., `ownership.refresh` which traverses a graph and updates control links).

The `write-set-contract` feature gate provides the correct extension point. When enabled, verb YAML `writes_to` declarations would feed into `derive_write_set_from_contract()`. The infrastructure is ready; the YAML declarations are the missing piece.

**Recommendation:** Audit the top-10 most-used plugin verbs for undeclared write targets. Add `writes_to` declarations to their YAML definitions. Enable the `write-set-contract` feature in the web server build.

---

### F-4: In-memory `RunbookStore` uses `.expect("RwLock poisoned")` — panic on poisoned lock

**Severity: LOW (test-only backend)**
**Location:** `rust/src/runbook/executor.rs` (RunbookStore implementation)

The in-memory `RunbookStore` wraps its state in `RwLock<HashMap<...>>` and accesses it via `.read().expect("RwLock poisoned")` / `.write().expect("RwLock poisoned")`. If any thread panics while holding the write lock, subsequent accesses will panic with "RwLock poisoned".

This is acceptable because:
1. The in-memory store is used only in tests; production uses `PostgresRunbookStore`.
2. The `PostgresRunbookStore` performs hash integrity verification on read (recomputes content-addressed hash and compares against stored `canonical_hash`), adding a defense-in-depth layer not present in the in-memory store.

No fix needed, but the limitation should be documented in the `RunbookStore` struct-level doc comment.

---

### F-5: Macro expansion `path` set uses `HashSet<String>` — non-deterministic but correct

**Severity: INFO (no action needed)**
**Location:** `rust/src/dsl_v2/macros/expander.rs`

The cycle detection in `expand_macro_recursive()` uses `HashSet<String>` to track the current call stack path. While `HashSet` has non-deterministic iteration order, it is used only for *membership testing* (`path.contains(fqn)`) — never iterated or serialized. Cycle detection correctness depends on membership, not order, so this is safe.

---

## 4. Pessimistic Entity Locking — Deadlock, Timeout, and Panic Analysis

### 4.1 Lock Acquisition Protocol

**Location:** `rust/src/runbook/executor.rs:689-740`, `rust/src/database/locks.rs`

```
1. Collect write_set UUIDs from ALL compiled steps (union)
2. Build LockKey values: LockKey::write("entity", uuid.to_string())
3. Keys are already sorted (source is BTreeSet<Uuid>)
4. Begin PostgreSQL transaction
5. SET LOCAL statement_timeout = '30000' (30s, transaction-scoped)
6. For each key: pg_advisory_xact_lock(hash(key))
7. On success: return (tx, LockStats)
8. On timeout (error 57014): rollback, return LockTimeout with write_set
9. On contention: rollback, return LockContention with holder info
```

### 4.2 Deadlock Analysis

**Verdict: Deadlock-free by construction.**

Advisory locks are acquired in sorted UUID order (guaranteed by `BTreeSet` source). Two concurrent runbooks R1 and R2 with overlapping write sets will always attempt to acquire the overlapping lock in the same order. This eliminates the circular-wait condition required for deadlock.

The `lock_key()` function in `database/locks.rs` derives a stable `i64` from `(entity_type, entity_id)` using `DefaultHasher`. While `DefaultHasher` is not guaranteed stable across Rust versions, it is stable within a single process — and advisory locks are per-process anyway (they are session-scoped within PostgreSQL).

### 4.3 Timeout Analysis

**Verdict: Bounded, transaction-scoped, no connection pool leakage.**

The 30-second timeout is implemented via `SET LOCAL statement_timeout = '30000'`. The `LOCAL` qualifier restricts the setting to the current transaction — when the transaction commits or rolls back, the connection returns to its default `statement_timeout` (typically 0, meaning no limit). This prevents leaked timeout settings from affecting subsequent queries on the same pooled connection.

PostgreSQL surfaces the timeout as error code `57014` (query_canceled). The lock acquisition code catches this specific error and maps it to `LockError::Timeout`, which the executor translates to `ExecutionError::LockTimeout`.

### 4.4 Panic Analysis

**Verdict: No panic paths in production lock code.**

The advisory lock code in `database/locks.rs` uses `?` operator throughout — no `.unwrap()` or `.expect()` calls. The `acquire_locks()` function returns `Result<LockResult, LockError>`. The only `.expect()` calls in the runbook module are in the test-only `RunbookStore` (see F-4 above).

The `PostgresRunbookStore` uses `sqlx` async queries with `?` propagation. Transaction rollback is explicit on error paths (not relying on `Drop`), and the `execute_runbook_with_pool()` function has a catch-all rollback in the error path before returning.

### 4.5 Lock Mode Selection

The executor uses `LockMode::Timeout(Duration::from_secs(30))`, not `LockMode::Try` (non-blocking) or `LockMode::Block` (wait indefinitely). This is the correct choice for interactive execution:
- `Try` would fail immediately on any contention, even transient (too aggressive).
- `Block` could wait indefinitely if the holder is a long-running runbook (too permissive).
- `Timeout(30s)` provides a bounded wait with clear error reporting.

---

## 5. Replay and Schema Evolution Guardrails

### 5.1 EnvelopeCore — Deterministic Hash Input

**Location:** `rust/src/runbook/envelope.rs`

The `EnvelopeCore` struct captures all deterministic inputs to compilation:

| Field | Type | Purpose |
|---|---|---|
| `session_cursor` | `u64` | Monotonic position in the session |
| `entity_bindings` | `BTreeMap<String, Uuid>` | Resolved entity references |
| `external_lookup_digests` | `Vec<String>` | SHA-256 of external data consulted |
| `macro_audit_digests` | `Vec<String>` | SHA-256 chain from macro expansion |
| `snapshot_manifest` | `HashMap<Uuid, Uuid>` | SemReg snapshot provenance (**defect: should be BTreeMap**) |

Volatile fields (timestamps, full lookup records) are excluded from `EnvelopeCore` and placed in the outer `ReplayEnvelope`. Only `EnvelopeCore` feeds into the content-addressed hash.

### 5.2 MacroExpansionAudit — Replay Verification (INV-12)

Each macro expansion produces a `MacroExpansionAudit` containing:
- `args_digest`: SHA-256 of the canonical JSON of input arguments
- `output_digest`: SHA-256 of the canonical JSON of expanded steps
- `expansion_limits`: The `ExpansionLimits` active at expansion time (captured in the `MacroExpansionAudit`, stored in the `ReplayEnvelope`)
- Nested audits for recursive macro invocations

On replay, the system can re-run `expand_macro_fixpoint()` with the stored args and compare the new `output_digest` against the stored one. A mismatch indicates that the macro schema has changed since the original compilation — the replay is blocked until an explicit recompilation.

### 5.3 Schema Evolution (INV-13)

The `canonical.rs` module includes a test `test_schema_evolution_with_new_field_inv13` that verifies the content-addressed ID changes when a new field is added to `CompiledStep`. This test uses `bincode::serialize()` to demonstrate that adding a field changes the serialized representation and thus the hash.

**Risk assessment:** Bincode serialization is format-sensitive to field order and count. Adding, removing, or reordering fields in `CompiledStep` or `EnvelopeCore` will change all content-addressed IDs for existing runbooks. This is *intentional* (schema changes should invalidate old runbooks) but means that any struct modification requires either:
1. Migrating existing stored runbooks (re-hash with new schema), or
2. Accepting that old runbook IDs become unreachable (append-only store, old entries are orphaned)

The current `PostgresRunbookStore` uses append-only event storage and derives status from the latest event, so orphaned old-schema entries degrade gracefully (they remain in storage but are never executed).

### 5.4 Replay Risk Matrix

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Macro schema change between compile and replay | Medium (macros evolve with product) | Replay produces different steps | INV-12: audit digest comparison blocks replay |
| SemReg policy drift between compile and execute | Medium (governance evolves) | Runbook may execute verbs now denied | **Gap**: No TOCTOU recheck at execution gate (F-2) |
| Bincode schema evolution (new CompiledStep field) | Low (infrequent struct changes) | Old runbook IDs become unreachable | INV-13: intentional invalidation, append-only store |
| HashMap ordering in snapshot_manifest | Low-Medium (depends on entry count) | Same inputs produce different IDs | **Defect**: F-1 — change to BTreeMap |
| ExpansionLimits change between compile and replay | Low (rarely changed) | Replay may hit new limits | Limits captured in MacroExpansionAudit |
| External lookup data changes | Medium (entity data evolves) | Same entity name resolves to different UUID | `external_lookup_digests` detects drift |

---

## 6. Summary of Severity-Tagged Findings

| ID | Severity | Finding | Status |
|---|---|---|---|
| F-1 | **HIGH** | `EnvelopeCore.snapshot_manifest` uses `HashMap<Uuid, Uuid>` — non-deterministic bincode serialization breaks content-addressed ID invariant (INV-2) | Fix: change to `BTreeMap<Uuid, Uuid>` |
| F-2 | **MEDIUM** | SemReg filter is fail-open; no TOCTOU recheck at execution gate for compiled runbooks | Fix: add lightweight verb allowlist recheck in `execute_runbook_with_pool()` |
| F-3 | **MEDIUM** | Write-set heuristic misses plugin side-effects beyond UUID arguments | Fix: enable `write-set-contract` feature + add `writes_to` YAML declarations |
| F-4 | **LOW** | In-memory RunbookStore panics on poisoned RwLock | Acceptable (test-only backend) — document limitation |
| F-5 | **INFO** | Cycle detection uses HashSet (non-deterministic iteration) but only for membership — safe | No action needed |

---

## 7. Architectural Strengths

1. **Structural enforcement over convention.** The compilation gate is enforced by the type system (`CompiledRunbookId` as sole execution token), not by code review policy. This is the strongest possible guarantee short of formal verification.

2. **Content-addressed deduplication.** Identical compilations produce identical IDs, enabling cache hits and change detection without explicit versioning logic.

3. **Bounded macro expansion.** The fixpoint algorithm with `max_depth=8` and `max_steps=500` plus per-path cycle detection prevents runaway expansion. The `MacroExpansionError` variants (CycleDetected, MaxDepthExceeded, MaxStepsExceeded) provide clear diagnostics.

4. **Append-only event store.** The `PostgresRunbookStore` uses `compiled_runbook_events` with status derived from the latest event. No destructive updates — full audit trail preserved.

5. **Transaction-scoped lock timeout.** `SET LOCAL statement_timeout` ensures the 30s timeout cannot leak to pooled connections, eliminating a common source of production incidents.

6. **Dual write-set strategy.** The heuristic-only path provides conservative coverage without configuration; the contract-driven path (feature-gated) enables precision when verb YAML metadata is available. The union design ensures no write target is missed.

7. **Source-level invariant tests.** INV-1 and INV-11 grep the actual source files for forbidden/required patterns — these tests break the build if someone adds a bypass path, providing continuous regression protection.

---

## Appendix: Files Examined

| File | Lines | Purpose |
|---|---|---|
| `rust/src/runbook/mod.rs` | ~45 | Module root: `compile_invocation()`, `execute_runbook()` |
| `rust/src/runbook/types.rs` | ~319 | `CompiledRunbook`, `CompiledStep`, `CompiledRunbookStatus` |
| `rust/src/runbook/canonical.rs` | ~720 | Content-addressed ID: SHA-256 + bincode + UUID truncation |
| `rust/src/runbook/envelope.rs` | ~185 | `EnvelopeCore`, `ReplayEnvelope`, `MacroExpansionAudit` |
| `rust/src/runbook/executor.rs` | ~1654 | `execute_runbook()`, advisory locking, `RunbookStore` backends |
| `rust/src/runbook/compiler.rs` | ~1200 | 6-step `compile_verb()` pipeline |
| `rust/src/runbook/write_set.rs` | ~343 | Heuristic + contract-driven write-set derivation |
| `rust/src/runbook/sem_reg_filter.rs` | ~80 | SemReg allowed-verb filter (fail-open) |
| `rust/src/runbook/constraint_gate.rs` | ~277 | Pack constraint enforcement |
| `rust/src/runbook/step_executor_bridge.rs` | ~205 | `DslStepExecutor` / `DslExecutorV2StepExecutor` bridges |
| `rust/src/runbook/response.rs` | ~45 | `OrchestratorResponse` enum |
| `rust/src/database/locks.rs` | ~200 | `acquire_locks()`, `LockMode`, `LockKey` |
| `rust/src/bpmn_integration/dispatcher.rs` | ~400 | `WorkflowDispatcher` — DslExecutorV2 impl |
| `rust/src/agent/orchestrator.rs` | ~100 | Unified intent orchestrator entry point |
| `rust/src/dsl_v2/macros/expander.rs` | ~150 | `expand_macro_fixpoint()`, `ExpansionLimits` |

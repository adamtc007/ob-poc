# P2-B: DSL Dispatch & Execution Engine Audit

**Review Session:** P2-B
**Date:** 2026-03-16
**Scope:** The `DslExecutor` struct, verb routing dispatch (`execute_verb_inner`), `GenericCrudExecutor`, `ExecutionContext`, `DslSubmission` batch execution, advisory locking, and the delta between the executor's design and its operational reality.

---

## 1. Executive Summary

The DSL execution engine is a well-structured dispatch system routing 1,113 verbs across 4 `RuntimeBehavior` variants (608 plugin, 493 crud, 9 graph_query, 3 durable) to handlers via `execute_verb_inner()`. The engine is architecturally sound for its two primary paths (plugin → CustomOp, crud → GenericCrudExecutor) with clean symbol capture, idempotency management, and unified batch execution via `execute_submission()`.

**One critical finding:** The `GraphQuery` behavior (9 verbs) has no dispatch branch — `GraphQueryExecutor` exists as a fully implemented but dead-code module. These verbs fall through to GenericCrudExecutor which explicitly rejects them. **One systemic flag:** Zero of 668 registered CustomOperations override `execute_in_tx()`, making all plugin verbs universally non-transactional despite the trait advertising tx support. **External side effects** in 7+ domain_ops files (GLEIF HTTP, BPMN gRPC, screening) have no compensation or rollback mechanism.

Additional gaps: (a) the single-table CRUD constraint drives 608 plugin promotions; (b) the dispatch uses sequential `if let` instead of exhaustive `match`, which is how the GraphQuery branch was missed; (c) plugin verb coverage is verified only in tests, not at startup.

---

## 2. DslExecutor Struct

```rust
// rust/src/dsl_v2/executor.rs
pub struct DslExecutor {
    pool: PgPool,
    custom_ops: CustomOperationRegistry,
    generic_executor: GenericCrudExecutor,
    idempotency: super::idempotency::IdempotencyManager,
    verb_hash_lookup: crate::session::verb_hash_lookup::VerbHashLookupService,
    events: Option<SharedEmitter>,
}
```

**Field analysis:**

| Field | Type | Purpose |
|---|---|---|
| `pool` | `PgPool` | Connection pool — passed directly to plugin CustomOperations |
| `custom_ops` | `CustomOperationRegistry` | Inventory-based registry of `#[register_custom_op]` handlers |
| `generic_executor` | `GenericCrudExecutor` | SQL template engine for crud/graphquery behaviors |
| `idempotency` | `IdempotencyManager` | Per-verb idempotency key deduplication |
| `verb_hash_lookup` | `VerbHashLookupService` | Maps verb FQN → content hash for contract pinning |
| `events` | `Option<SharedEmitter>` | Optional SSE event emitter for live updates |

The `custom_ops` field uses the `inventory` crate — registration is compile-time via `#[register_custom_op]` proc-macro, not runtime. There is no mechanism to hot-reload or dynamically register a custom operation without recompilation.

---

## 3. ExecutionContext

`ExecutionContext` carries the mutable runtime state for a single execution turn:

```rust
pub struct ExecutionContext {
    pub bindings: HashMap<String, Uuid>,     // @symbol → UUID bindings
    pub json_bindings: serde_json::Value,    // Mixed-type runtime values
    pub user_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub domain: Option<String>,
}
```

**Symbol capture flow:**
After a CRUD verb succeeds with a UUID return and `returns.capture = true`, the executor captures the UUID:
```rust
if runtime_verb.returns.capture {
    if let GenericExecutionResult::Uuid(uuid) = &result {
        if let Some(name) = &runtime_verb.returns.name {
            ctx.bind(name, *uuid);
        }
    }
}
```

The `json_bindings` map also carries agent control signals via reserved keys:
- `_agent_control.pending_checkpoint_response`
- `_agent_control.pending_threshold_change`
- `_agent_control.pending_mode_change`

These are set by `set_pending_checkpoint_response()`, `set_pending_threshold_change()`, and `set_pending_mode_change()` helper methods. A separate `take_pending_agent_control()` drains the control payload after the orchestrator reads it.

**Constructor:** `ExecutionContext::from_domain(domain: &str)` is the standard construction path. This sets `domain` but leaves `user_id` and `session_id` as `None` — they must be set explicitly after construction for audit trails. If callers omit this step, audit queries on `intent_events` will have null actor fields.

---

## 4. Verb Routing Dispatch

`execute_verb_inner()` is the single dispatch point for all verb execution. It implements three branches:

### 4.1 Branch 1: Plugin → CustomOperationRegistry

```rust
if let RuntimeBehavior::Plugin(_) = &runtime_verb.behavior {
    if let Some(op) = self.custom_ops.get(&vc.domain, &vc.verb) {
        return op.execute(vc, ctx, &self.pool).await;
    }
    return Err(anyhow!("Plugin {}.{} has no handler", vc.domain, vc.verb));
}
```

**Key behavior:** If a YAML verb is declared `behavior: plugin` but no corresponding `#[register_custom_op]` struct exists in inventory, the executor returns a hard error at runtime — not at startup. The `test_plugin_verb_coverage` test in `src/` is the primary gate against this, but it runs as a unit test and not as a startup invariant check.

**Pool threading:** Each CustomOperation receives `&self.pool` directly. Operations that need transactions must create their own `pool.begin()`. There is no ambient transaction context threaded into the pool — plugin ops are responsible for their own transaction boundaries.

### 4.2 Branch 2: Durable → Error (V2 REPL Required)

```rust
if let RuntimeBehavior::Durable(d) = &runtime_verb.behavior {
    return Err(anyhow!("Durable verb requires V2 REPL with WorkflowDispatcher"));
}
```

This branch ensures that verbs declaring `behavior: durable` cannot be executed through the standard DSL executor path. The `WorkflowDispatcher` (BPMN integration layer) wraps the executor and intercepts durable verbs before they reach this point. If the BPMN integration is disabled (`BPMN_LITE_GRPC_URL` not set), durable verbs will hard-error on any execution attempt.

### 4.3 Branch 3: CRUD/Everything Else → GenericCrudExecutor

```rust
let json_args = Self::verbcall_args_to_json(&vc.arguments, ctx)?;
let result = self.generic_executor.execute(runtime_verb, &json_args).await?;
if runtime_verb.returns.capture {
    if let GenericExecutionResult::Uuid(uuid) = &result {
        if let Some(name) = &runtime_verb.returns.name { ctx.bind(name, *uuid); }
    }
}
Ok(result.to_legacy())
```

This branch handles `crud`, `graphquery`, and any behavior not explicitly matched above. The `verbcall_args_to_json()` function converts AST `VerbCall.arguments` to a flat JSON map — `SymbolRef` nodes are resolved via `ctx.resolve(name)`, and `EntityRef` nodes use `resolved_key` if populated or emit an error if still unresolved.

---

## 5. GenericCrudExecutor

The `GenericCrudExecutor` translates `VerbConfig.crud` YAML configuration into parameterized SQL. It handles four operations:

| Operation | YAML `crud.operation` | SQL Pattern |
|---|---|---|
| Insert | `insert` | `INSERT INTO {schema}.{table} ({cols}) VALUES ({params}) RETURNING {pk}` |
| Update | `update` | `UPDATE {schema}.{table} SET {assignments} WHERE {pk_col} = $1` |
| Delete | `delete` | `DELETE FROM {schema}.{table} WHERE {pk_col} = $1` |
| Select/Read | `select` | `SELECT {cols} FROM {schema}.{table} WHERE {conditions}` |

**Single-table constraint:** The generic executor operates on exactly one table per verb. Multi-table operations (e.g., insert + FK lookup + insert into child table) cannot be expressed in the CRUD config — they require promotion to a plugin CustomOperation. This is the primary driver for the large number of plugin verbs in domains like `kyc`, `cbu`, and `trading-profile`.

**`graphquery` behavior:** A fourth executor path handles the `graphquery` behavior, executing parameterized graph traversal queries from the domain's YAML definition. This is used in the `graph` domain (9 verbs) to execute Cypher-like queries over the entity graph without plugin promotion.

---

## 6. Argument Conversion — verbcall_args_to_json

```rust
fn verbcall_args_to_json(
    args: &[VerbArgValue],
    ctx: &ExecutionContext,
) -> Result<serde_json::Map<String, Value>>
```

This function serializes DSL AST argument nodes to JSON for the generic executor. Key conversions:

| AST Node Type | JSON Output | Resolution |
|---|---|---|
| `SymbolRef(name)` | UUID string | `ctx.resolve(name)` → error if not bound |
| `EntityRef { resolved_key: Some(k) }` | UUID string | Direct from resolved_key |
| `EntityRef { resolved_key: None }` | **Error** | Entity was never resolved — pipeline bug |
| `StringLiteral(s)` | JSON string | Direct |
| `NumberLiteral(n)` | JSON number | Direct |
| `BoolLiteral(b)` | JSON boolean | Direct |
| `ListLiteral(items)` | JSON array | Recursively converted via `node_to_json()` |

`EntityRef` with `resolved_key: None` generates a hard error. This means any unresolved entity reference that passes through to execution will fail at the argument conversion stage — the pipeline is supposed to block at the resolution step but this is a second line of defense.

---

## 7. DslSubmission Batch Execution

`execute_submission()` is the unified entry point for `DslSubmission` payloads:

```rust
pub async fn execute_submission(
    &self,
    submission: &DslSubmission,
    ctx: &mut ExecutionContext,
    session_id: Option<Uuid>,
) -> Result<SubmissionResult>
```

**Execution flow:**

```
DslSubmission { program, iterations? }
    │
    ▼
Expand iterations (template × each iteration binding)
    │
    ▼
For each expanded iteration:
    │
    ├─► pool.begin() → Transaction
    │
    ├─► execute_statements_in_tx(statements, ctx, &tx)
    │       │
    │       ├─► For each statement: execute_verb_inner()
    │       └─► Capture @symbol bindings from results
    │
    ├─► tx.commit() on success
    │   tx.rollback() on failure
    │
    └─► Accumulate IterationResult
    │
    ▼
SubmissionResult { iterations: Vec<IterationResult>, total_affected }
```

**Transaction boundary:** Each iteration runs in its own transaction. If a DslSubmission has 3 iterations and iteration 2 fails, iterations 1 and 3 commit independently. There is no cross-iteration rollback. This is appropriate for independent bulk operations but means multi-step workflows that should be all-or-nothing must be expressed as a single DSL statement sequence (not multiple iterations).

**IterationResult:**
```rust
pub struct IterationResult {
    pub iteration_index: usize,
    pub results: Vec<LegacyExecutionResult>,
    pub bindings_captured: HashMap<String, Uuid>,
    pub error: Option<String>,
}
```

**SubmissionResult:**
```rust
pub struct SubmissionResult {
    pub iterations: Vec<IterationResult>,
    pub total_affected: usize,
}
```

---

## 8. execute_statements_in_tx

The inner loop for executing a sequence of DSL statements within a single transaction:

```rust
async fn execute_statements_in_tx(
    statements: &[CompiledStatement],
    ctx: &mut ExecutionContext,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<LegacyExecutionResult>>
```

This function passes `tx` through to verb execution. Plugin CustomOperations receive the pool (not the transaction) — plugin ops that issue their own `pool.begin()` inside a submission transaction will create a nested transaction context, which is correct in PostgreSQL (savepoints) but may lead to lock ordering issues if the plugin and the outer transaction both acquire advisory locks.

The `:as @symbol` binding is captured here: after each statement executes, if the result is a UUID and the statement has an `:as` clause, the UUID is bound into `ctx.bindings` for use by subsequent statements in the same sequence.

---

## 9. Advisory Locking

Advisory locks in the DSL execution engine operate at the `execute_runbook_with_pool()` level (in the Runbook subsystem, not the executor directly). The executor itself does not acquire advisory locks — it is the caller's responsibility.

The locking protocol:
1. **Write-set derivation**: UUID arguments extracted from the compiled step's `write_set`
2. **Sort**: Lock keys sorted to prevent deadlock
3. **Acquisition**: `pg_try_advisory_xact_lock(key)` for each UUID — fails fast if any lock is held
4. **Scope**: Transaction-scoped (released on commit/rollback, 30-second default transaction timeout)
5. **Failure mode**: Lock contention returns `LockContention` result — the orchestrator may retry

The write-set is derived at compile time (compile step 6 in the 6-step runbook compilation pipeline). For plugin verbs, the write-set is inferred from UUID arguments — the plugin's actual data writes may extend beyond what the write-set captures (e.g., a plugin that writes to tables not touched by its UUID arguments).

---

## 10. Idempotency

The `IdempotencyManager` provides per-verb deduplication:
- Key: `(session_id, statement_hash)` — content hash of the verb call and arguments
- On hit: Returns the cached result without re-execution
- On miss: Executes, caches result, returns

Idempotency is opt-in per verb (configured in YAML). Plugin CustomOperations that use `pool.begin()` must cooperate with the idempotency manager by checking the cache before executing their first query. There is no automatic idempotency enforcement for plugin operations that bypass the generic executor.

---

## 11. Gap Summary

| Gap | Impact | Effort | Priority |
|---|---|---|---|
| Plugin ops receive `pool` not ambient `tx` — nested transaction risk | Plugin ops in `execute_submission()` may create nested tx contexts | Low — design decision | P2 |
| GenericCrudExecutor single-table constraint | Complex multi-table operations require plugin promotion (300+ plugins exist partly due to this) | High — requires executor extension or composable CRUD | P1 |
| Cross-iteration rollback absent | Multi-iteration submissions commit independently — no batch atomicity | Medium — requires wrapping in outer transaction | P1 |
| Plugin verb coverage gap detectable only at runtime | No startup invariant check for missing plugin handlers | Low — test gate exists but not startup guard | P2 |
| `user_id`/`session_id` omission in ExecutionContext | Actor fields null in `intent_events` audit if callers skip `.with_session()` | Low — discipline issue | P3 |
| Durable verbs hard-error without BPMN wiring | `behavior: durable` verbs unusable if `BPMN_LITE_GRPC_URL` not set | Low — documented requirement | P3 |
| Write-set derivation limited to UUID args | Plugin writes to non-UUID-argument tables not in advisory lock scope | Medium — potential lost update risk under concurrent load | P1 |

---

## 12. Recommendations

1. **Add startup plugin coverage check:** At server startup, iterate all YAML verbs with `behavior: plugin` and verify each has a registered `CustomOperation` in `custom_ops`. This catches missing handlers at boot rather than at first user request.

2. **Composable CRUD operations:** Extend `GenericCrudExecutor` to support simple two-table compositions (insert + child insert, or insert + FK lookup). This would allow 20-30% of simple plugin verbs to be converted back to CRUD without sacrificing correctness.

3. **Submission-level transaction option:** Add a `batch_atomic: bool` flag to `DslSubmission` that wraps all iterations in a single outer transaction. When true, all iterations commit together or rollback together. Default: false (current behavior).

4. **Explicit write-set declarations for plugin verbs:** Extend the YAML `behavior: plugin` block to support `writes_to: [table1, table2]` declarations. At compile time, derive advisory lock keys from declared table primary keys rather than inferring from UUID arguments alone.

5. **Thread session actor into ExecutionContext factory:** Make `ExecutionContext::from_domain()` accept a `session_id: Option<Uuid>` and `actor_id: Option<String>` so callers cannot accidentally omit audit fields.

---

## 13. Dispatch Coverage Matrix

### 13.1 Behavior Distribution (1,113 verbs total)

| Behavior | Count | Handler | Transactional? | Side Effects? |
|---|---|---|---|---|
| `plugin` | 608 | `CustomOperationRegistry` → `CustomOperation.execute()` | **No** — receives `&PgPool`, must self-manage | Varies per op (see 13.4) |
| `crud` | 493 | `GenericCrudExecutor.execute()` | **Yes** — `execute_in_tx()` supported for insert/update/delete/upsert/link | DB only (single-table parameterized SQL) |
| `graph_query` | 9 | **DEAD PATH** — `GraphQueryExecutor` exists but is never wired into dispatch | N/A | N/A |
| `durable` | 3 | Hard error in `execute_verb_inner()`; intercepted by `WorkflowDispatcher` when BPMN enabled | N/A (parked) | gRPC to bpmn-lite service |

### 13.2 Dispatch Path Matrix

| Entry Point | Plugin | CRUD | GraphQuery | Durable |
|---|---|---|---|---|
| `execute_verb_inner()` (non-tx) | `op.execute(vc, ctx, &pool)` | `generic_executor.execute(verb, args)` | Falls through to GenericCrudExecutor which **rejects** it | Hard error |
| `execute_verb_in_tx()` (tx) | `op.execute_in_tx(vc, ctx, tx)` — **always errors** (zero overrides) | `generic_executor.execute_in_tx(tx, verb, args)` | Falls through to GenericCrudExecutor which **rejects** it | Hard error |
| `WorkflowDispatcher` (BPMN) | Delegates to inner executor | Delegates to inner executor | Delegates to inner executor | Intercepts → gRPC `StartProcess` |
| `execute_runbook_with_pool()` | Via `StepExecutor` bridge → `execute_verb_inner()` | Via `StepExecutor` bridge → `execute_verb_inner()` | Same (will fail) | Via `DslExecutorV2StepExecutor` → `Parked` |

### 13.3 CustomOperation Registration Audit

| Metric | Count |
|---|---|
| YAML verbs with `behavior: plugin` | 608 |
| `#[register_custom_op]` annotations in `domain_ops/` | 668 |
| Delta (potentially orphaned ops) | ~60 |
| Files with `#[governed_query]` proc-macro | 5 (cbu_ops, agent_ops, kyc_case_ops, entity_ops, attribute_ops) |
| CustomOps overriding `execute_in_tx()` | **0 of 668** |

The 60-op delta represents CustomOps registered via inventory that have no corresponding YAML verb definition. These are harmless (never dispatched to) but indicate stale code after verb renames or domain consolidation.

### 13.4 External Side Effects Inventory

| File | External Target | Protocol | Rollback on Failure? |
|---|---|---|---|
| `gleif_ops.rs` | GLEIF LEI API | HTTP (reqwest) | No — DB writes not rolled back if API succeeds then DB fails |
| `bpmn_lite_ops.rs` | bpmn-lite service | gRPC (tonic) | No — fire-and-forget dispatch |
| `source_loader_ops.rs` | External data sources | HTTP (reqwest) | No — partial loads may persist |
| `screening_ops.rs` | Screening service | HTTP client | No — screening results written independently |
| `verify_ops.rs` | Verification service | HTTP client | No — verification state written on return |
| `request_ops.rs` | External request dispatch | HTTP client | No — request queued before response |
| `ownership_ops.rs` | Multi-step pipeline (5 DB ops) | Internal pipeline | Partial — steps are independent DB writes |
| `manco_ops.rs` | Multi-step ownership pipeline | Internal pipeline | Partial — steps are independent DB writes |
| `control_ops.rs` | Graph computation pipeline | Internal pipeline | No ambient tx |
| `bods_ops.rs` | BODS data import | Internal + possible HTTP | No — import writes are independent |
| `research_normalize_ops.rs` | Data normalization pipeline | Internal pipeline | No ambient tx |

**Pattern:** All external-calling handlers receive `&PgPool` (not a transaction). If a handler makes an external call AND writes to the DB, there is no mechanism to atomically commit both. A failure after the external call succeeds but before the DB write completes results in a state where the external action happened but is not recorded locally.

---

## 14. Severity-Tagged Findings

### CRITICAL

**F-1: GraphQuery dispatch path is dead — 9 verbs cannot execute**

`execute_verb_inner()` has no explicit `RuntimeBehavior::GraphQuery` branch. These 9 verbs (all in `graph.yaml`) fall through to the generic executor, which explicitly rejects `GraphQuery` behavior with an error: `"Verb {domain}.{verb} is a graph query, use GraphQueryExecutor"`. The `GraphQueryExecutor` struct exists in `graph_executor.rs` with full implementations for all 9 operations (View, Focus, Filter, GroupBy, Path, FindConnected, Ancestors, Descendants, Compare) but is never instantiated or called from any dispatch path. This is dead code.

- **Location:** `rust/src/dsl_v2/executor.rs:1162` (missing branch), `rust/src/dsl_v2/graph_executor.rs` (dead code)
- **YAML config:** `rust/config/verbs/graph.yaml` (9 verbs)
- **Impact:** Any attempt to execute a `graph.*` verb will produce a runtime error
- **Fix:** Add a `GraphQuery` branch to `execute_verb_inner()` that instantiates `GraphQueryExecutor` and delegates, or reclassify these verbs as `plugin` with CustomOps

### FLAG

**F-2: Zero CustomOperations override `execute_in_tx()` — plugin verbs are universally non-transactional**

The `CustomOperation` trait defines `execute_in_tx()` with a default implementation that returns `Err`. Of 668 registered CustomOps, **none** override this method. This means `execute_verb_in_tx()` will always error for plugin verbs. The `execute_submission()` path creates a transaction per iteration, but plugin verbs within that iteration execute against the raw pool (non-transactional) via `execute_verb_inner()`, not `execute_verb_in_tx()`.

- **Location:** `rust/src/domain_ops/mod.rs:358-379` (default impl), all 83 domain_ops files (zero overrides)
- **Impact:** Plugin verbs in batch submissions do not participate in the iteration transaction. Failure of a later statement in the same iteration does not roll back earlier plugin verb effects.
- **Severity rationale:** FLAG not CRITICAL because the current codebase does not call `execute_verb_in_tx()` for plugin verbs in the hot path — the submission loop uses `execute_verb_inner()` which bypasses the tx-aware path. However, the existence of the tx-aware dispatch path with universally failing plugin support is misleading and fragile.

**F-3: Multi-query plugin handlers lack internal transaction boundaries**

Plugin handlers like `KycCaseCreateOp` perform multiple sequential SQL queries (e.g., deal validation query + insert) against `&PgPool` without wrapping them in a `pool.begin()` / `tx.commit()` block. If the insert succeeds but a subsequent query fails, partial state persists.

- **Location:** Widespread across `domain_ops/` — example: `kyc_case_ops.rs`
- **Impact:** Low under normal operation (single-user, low concurrency). Risk increases under concurrent load where interleaving queries can produce inconsistent state.
- **Fix:** Audit high-write-count plugin ops and add explicit `pool.begin()` / `tx.commit()` where multi-query atomicity is required.

**F-4: External side-effect handlers have no compensation or rollback mechanism**

Seven `domain_ops` files make external HTTP/gRPC calls. If the external call succeeds but the subsequent DB write fails, or vice versa, there is no saga/compensation pattern to reconcile the state. The handlers are fire-and-forget with respect to external state.

- **Location:** `gleif_ops.rs`, `bpmn_lite_ops.rs`, `source_loader_ops.rs`, `screening_ops.rs`, `verify_ops.rs`, `request_ops.rs`, `bods_ops.rs`
- **Impact:** Low for read-only external calls (GLEIF lookups). Medium for state-mutating external calls (BPMN start, screening initiation).
- **Fix:** For critical external mutations, consider an outbox pattern (write intent to DB in transaction, background worker executes external call).

**F-5: Plugin verb coverage check is test-only — not a startup invariant**

`verify_plugin_verb_coverage()` in `domain_ops/mod.rs` compares YAML plugin verbs against registered CustomOps but only runs as a unit test (`test_plugin_verb_coverage`). A missing handler is not detected until a user tries to execute the verb at runtime.

- **Location:** `rust/src/domain_ops/mod.rs:483-562`
- **Impact:** Low — the test gate catches most issues in CI. But a verb added to YAML without a corresponding CustomOp will pass CI if the test is not run (e.g., `--lib` filter misses it) and fail at runtime.
- **Fix:** Call `verify_plugin_verb_coverage()` at server startup and fail-fast if any plugin verb lacks a handler.

### MINOR

**F-6: ~60 orphaned CustomOps registered without corresponding YAML verbs**

668 `#[register_custom_op]` annotations exist across 83 files, but only 608 YAML verbs declare `behavior: plugin`. The delta represents stale ops from domain consolidation (e.g., `cbu-role.*` → `cbu.assign-role`, `manco` → `ownership`).

- **Impact:** Zero runtime impact — orphaned ops are registered in the HashMap but never dispatched to.
- **Fix:** Periodic cleanup. The `verify_plugin_verb_coverage()` test could be extended to detect ops without YAML counterparts.

**F-7: `ExecutionContext::from_domain()` does not require audit fields**

The constructor sets `user_id` and `session_id` to `None`. Callers must explicitly set these after construction. If they forget, `intent_events` audit records will have null actor fields.

- **Location:** `rust/src/dsl_v2/executor.rs` (ExecutionContext)
- **Impact:** Audit gap — queries on "who executed this verb?" return null for some paths.
- **Fix:** Change constructor signature to require `session_id: Option<Uuid>` and `actor_id: Option<String>`.

**F-8: `governed_query` macro applied inconsistently**

Only 5 of 83 domain_ops files use the `#[governed_query]` proc-macro (cbu_ops, agent_ops, kyc_case_ops, entity_ops, attribute_ops). The macro adds governance audit logging. Other high-sensitivity domains (deal_ops, billing_ops, ownership_ops) lack this annotation.

- **Impact:** Governance audit trail is incomplete for non-annotated ops.
- **Fix:** Extend `#[governed_query]` to all ops that mutate governed data, or replace with a blanket audit wrapper.

### CLEAN

**F-9: CustomOperationRegistry duplicate detection is correct**

`register_internal()` panics on duplicate `(domain, verb)` keys. This is correct — duplicates indicate a build-time error (two structs claiming the same verb).

**F-10: Argument conversion provides second-line defense against unresolved entities**

`verbcall_args_to_json()` hard-errors on `EntityRef { resolved_key: None }`. Even if the upstream resolution pipeline has a bug, unresolved entities cannot reach the executor.

**F-11: Idempotency manager correctly scoped to `(session_id, statement_hash)`**

The deduplication key prevents re-execution of identical verb calls within the same session. The hash includes verb FQN and all arguments, making collisions impractical.

**F-12: Advisory locking with sorted keys prevents deadlocks**

The runbook execution path sorts lock keys before acquisition and uses `pg_try_advisory_xact_lock` (fail-fast), which is the correct pattern for concurrent access without deadlock risk.

**F-13: Event emission wraps execution cleanly**

`execute_verb()` wraps `execute_verb_inner()` with SSE event emission on success/failure. The inner function is pure dispatch logic. This separation is clean.

**F-14: DslSubmission per-iteration transaction boundaries are correct**

Each iteration gets its own `pool.begin()` / `tx.commit()`. Cross-iteration independence is documented and intentional. The missing feature (all-or-nothing batch atomicity) is documented in the gap summary.

---

## 15. Consistency Assessment

### Strengths

1. **Single dispatch point:** All verb execution flows through `execute_verb_inner()` (non-tx) or `execute_verb_in_tx()` (tx). There are no alternate dispatch paths that bypass the registry lookup or behavior branching. This makes the execution engine auditable and predictable.

2. **Clean behavior → handler mapping:** The `RuntimeBehavior` enum is exhaustive with 4 variants. The `match` in `GenericCrudExecutor::execute_with_optional_tx()` explicitly rejects non-CRUD behaviors rather than silently failing, which is the right design — except that `execute_verb_inner()` does not mirror this exhaustiveness (see F-1).

3. **Compile-time registration:** The `inventory` crate + `#[register_custom_op]` macro eliminates manual registration lists. Adding a new op is a single struct + attribute. The registry's duplicate detection (panic on collision) catches conflicts at startup.

4. **Symbol capture is uniform:** Both plugin and CRUD paths capture `@symbol` bindings through the same `returns.capture` check. The capture logic is not duplicated — it lives in `execute_verb_inner()` after the dispatch branch.

5. **CRUD transactional support is complete:** The `GenericCrudExecutor` supports transactional execution for all write operations (insert, update, delete, upsert, link). Read-only select operations do not require transactions.

### Weaknesses

1. **GraphQuery is a dead fourth behavior:** The `RuntimeBehavior` enum has 4 variants but only 3 are handled in dispatch. This is the single most concerning finding — 9 verbs are silently broken. The error message in `GenericCrudExecutor` acknowledges the existence of `GraphQueryExecutor` but no code wires it.

2. **Transaction support is binary (CRUD yes, plugin no):** The `execute_in_tx()` trait method exists on `CustomOperation` but is universally unimplemented. This creates a false API contract — the method signature promises transactional capability that does not exist. Either remove `execute_in_tx()` from the trait (acknowledging plugins are non-transactional by design) or implement it for high-criticality ops.

3. **External side effects are unmanaged:** 7+ domain_ops files make external calls with no compensation pattern. This is an accepted tradeoff for a POC, but should be documented as a known limitation for production hardening.

4. **Dispatch exhaustiveness depends on fall-through:** `execute_verb_inner()` uses sequential `if let` checks for Plugin and Durable, then falls through to GenericCrudExecutor for "everything else." A Rust `match` on `runtime_verb.behavior` would enforce exhaustive handling at compile time and would have caught F-1. The current pattern is fragile — adding a new `RuntimeBehavior` variant will silently fall through to the generic executor.

5. **Two parallel dispatch paths with divergent behavior:** `execute_verb_inner()` (non-tx, works for plugins) and `execute_verb_in_tx()` (tx, always fails for plugins) have identical structure but different operational characteristics. The tx path exists but is never successfully used for 608/1113 verbs. This dual-path design adds complexity without functional benefit for plugin verbs.

### Overall Assessment

The dispatch engine is **architecturally sound for its primary use case** (CRUD + plugin routing) but has one critical dead path (GraphQuery), one systemic gap (zero plugin tx support), and a fragile dispatch pattern (sequential if-let instead of exhaustive match). The external side-effects are an accepted POC limitation. The recommended priority order for remediation is: F-1 (GraphQuery — CRITICAL), F-2 (execute_in_tx audit — FLAG), F-5 (startup coverage check — FLAG), then F-4 (outbox for external mutations — FLAG).

---

## Appendix A: Key File Locations

```
rust/src/dsl_v2/executor.rs           — DslExecutor, execute_verb_inner, execute_verb_in_tx, execute_submission
rust/src/dsl_v2/generic_executor.rs   — GenericCrudExecutor, execute_with_optional_tx
rust/src/dsl_v2/graph_executor.rs     — GraphQueryExecutor (DEAD CODE — never wired into dispatch)
rust/src/dsl_v2/runtime_registry.rs   — RuntimeBehavior enum (4 variants), RuntimeVerbRegistry
rust/src/dsl_v2/idempotency.rs        — IdempotencyManager
rust/src/runbook/executor.rs          — execute_runbook_with_pool, advisory locking
rust/src/runbook/step_executor_bridge.rs — StepExecutor trait bridges (DslStepExecutor, DslExecutorV2StepExecutor)
rust/crates/ob-poc-macros/src/register_op.rs — #[register_custom_op] proc-macro implementation
rust/src/domain_ops/mod.rs            — CustomOperation trait (with default execute_in_tx), CustomOperationRegistry, verify_plugin_verb_coverage
rust/src/domain_ops/                  — 83 files, 668 CustomOperation implementations
rust/config/verbs/                    — 88+ YAML files, 1113 verb definitions (608 plugin, 493 crud, 9 graph_query, 3 durable)
rust/config/verbs/graph.yaml          — 9 graph_query verbs (unreachable via dispatch)
rust/src/agent/orchestrator.rs        — Unified intent orchestrator (SemReg → pipeline → dispatch)
rust/src/mcp/handlers/core.rs         — MCP tool handlers (dsl_execute entry point)
rust/src/bpmn_integration/dispatcher.rs — WorkflowDispatcher (intercepts durable verbs)
```

## Appendix B: Verb Behavior Count by Domain (Top 15)

| Domain | Plugin | CRUD | GraphQuery | Durable | Total |
|---|---|---|---|---|---|
| trading-profile | 30 | 2 | 0 | 0 | 32 |
| deal | 28 | 14 | 0 | 0 | 42 |
| client-group | 23 | 0 | 0 | 0 | 23 |
| sem-reg/registry | 26 | 0 | 0 | 0 | 26 |
| agent | 20 | 0 | 0 | 0 | 20 |
| cbu | 9 | 21 | 0 | 0 | 30 |
| session | 18 | 0 | 0 | 0 | 18 |
| gleif | 16 | 0 | 0 | 0 | 16 |
| view | 14 | 0 | 0 | 0 | 14 |
| billing | 14 | 3 | 0 | 0 | 17 |
| sem-reg/changeset | 14 | 0 | 0 | 0 | 14 |
| sem-reg/schema | 13 | 0 | 0 | 0 | 13 |
| entity | 7 | 8 | 0 | 0 | 15 |
| graph | 1 | 0 | 9 | 0 | 10 |
| document | 7 | 12 | 0 | 2 | 21 |

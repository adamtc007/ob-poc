# ADR 074 — Scope-aware nested DSL primitive (Phase 7 prerequisite)

**Date:** 2026-04-22
**Status:** Proposed (Slice 0.1 of `three-plane-correction-slice-plan-2026-04-22.md`)
**Driver:** Peer review finding F2 — 92 Pattern B write-path sites bypass the ambient
`TransactionScope` via `scope.pool().clone()`. 4 of those additionally spawn a nested
`DslExecutor::new(pool.clone())` from inside a plugin op body, which opens a fresh transaction root
that is invisible to outer rollback.

**Blast radius:** ~92 A-class writes + 4 D-class nested-executor sites across `trading_profile.rs`,
`booking_principal_ops.rs`, `request_ops.rs`, `source_loader_ops.rs`, `gleif_ops.rs`,
`onboarding.rs`, `template_ops.rs`. Without a scope-aware nested-DSL entrypoint, the Phase 7
migration produces *relocated* code, not *correct* code.

---

## 1. The problem

The three-plane v0.3 contract (§8.4) says:

> The Sequencer owns transaction scope — when a transaction begins, when it commits, when it rolls
> back, what the boundary includes. The Runtime owns mechanics — statement execution, pool
> checkout, row accounting, deadlock retries — inside the scope the Sequencer supplies.

Today's Pattern B plugin ops break this in two ways:

**(A) Pool escape for direct SQL.** An op receives `&mut dyn TransactionScope` but calls
`scope.pool().clone()` and runs its INSERT/UPDATE/DELETE through a fresh pool connection. That
connection does **not** participate in the ambient transaction. An outer rollback leaves the write
committed.

**(D) Nested DSL via pool.** Ops that need to execute sub-DSL (template expansion, onboarding
auto-complete, GLEIF recursive enrichment) construct
`DslExecutor::new(scope.pool().clone())`. The nested executor acquires its own pool connection,
which opens a *fresh* transaction root when it commits internally. Outer rollback can't undo what
the inner executor committed.

The `TransactionScope::pool()` method is documented as transitional (`dsl-runtime/src/tx.rs:66`),
but no alternative exists for the nested-DSL case. The whole point of the scope trait is defeated
if ops have no way to run sub-DSL inside it.

---

## 2. Goals

1. **Preserve atomicity.** A failure anywhere in an ambient transaction — outer op, nested op, or
   any SQL inside them — rolls back *everything* together.
2. **Reuse canonical plugin dispatch.** Nested verb execution must go through the same
   `SemOsVerbOpRegistry` (F1 hard-fail infrastructure). Two dispatch paths = two places to forget
   to thread the registry.
3. **No fresh transaction roots.** Nested DSL never opens a new `sqlx::Transaction`.
4. **Bounded recursion.** A runaway recursion caused by a plugin op that invokes itself (directly
   or transitively) must surface as an error, not a stack overflow.
5. **Zero behaviour change for existing call sites.** `execute_plan` / `execute_dsl` / etc. keep
   their current contracts. The new primitive is an **addition**, not a replacement.

Non-goals:

- Generalizing to non-Postgres backends. The `TransactionScope` trait allows that later; this
  primitive is explicitly Postgres-only at the storage boundary.
- Fixing every F2 site in this slice. Slice 0.1 delivers the primitive. Phase 7 slices use it.

---

## 3. Design

### 3.1 Postgres mechanism: SAVEPOINT per nested scope

A nested DSL tree acquires a `SAVEPOINT <name>` at entry and releases it at exit. On error, we
`ROLLBACK TO SAVEPOINT <name>`. This keeps the outer `sqlx::Transaction` intact while preserving
atomicity within the nested unit.

SAVEPOINT nesting is native Postgres — no extra coordination needed. Commit semantics:

- Outer `COMMIT` commits everything, including nested work.
- Outer `ROLLBACK` rolls back everything, including nested work.
- Inner success + outer failure → inner work rolled back (atomic across the scope). ✓ correct.
- Inner failure + outer continues → inner work rolled back via `ROLLBACK TO SAVEPOINT`, outer
  continues. Whether outer should abort on inner failure is the op author's choice; the primitive
  surfaces the inner error as `Result::Err`.

### 3.2 The `NestedExecutionScope` struct

New type in `dsl-runtime::tx`:

```rust
/// A guard that holds a Postgres SAVEPOINT for a nested DSL execution. On
/// drop (without explicit `release()`), the savepoint is rolled back.
/// Created by `TransactionScope::open_nested()`.
pub struct NestedExecutionScope<'scope> {
    parent: &'scope mut dyn TransactionScope,
    savepoint_name: String,
    released: bool,
}

impl<'scope> NestedExecutionScope<'scope> {
    /// Commit the nested unit to the parent scope (i.e. release the
    /// savepoint). Does NOT commit the outer transaction.
    pub async fn release(mut self) -> anyhow::Result<()> { ... }

    /// Explicit rollback of the nested unit. Use when the op detects an
    /// error condition but wants to keep the outer transaction alive.
    pub async fn rollback(mut self) -> anyhow::Result<()> { ... }
}

impl<'scope> Drop for NestedExecutionScope<'scope> {
    /// If neither `release` nor `rollback` was called, roll back the
    /// savepoint to preserve outer atomicity guarantees.
    fn drop(&mut self) { ... }
}
```

A `NestedExecutionScope` **is-a** `TransactionScope` itself (via a blanket impl) — this is what
makes recursion uniform: the nested tree receives `&mut dyn TransactionScope` just like the outer
did.

### 3.3 The `execute_plan_in_scope` entry point

New method on `DslExecutor`:

```rust
impl DslExecutor {
    /// Execute a pre-compiled plan inside an ambient transaction scope. The
    /// scope opens a SAVEPOINT before execution and commits/rolls-back on
    /// result. Plugin dispatch reuses the canonical `SemOsVerbOpRegistry`
    /// installed on the outer executor.
    ///
    /// Recursion is bounded by `ctx.depth_budget` (default 4; override per
    /// execution context). Exceeding the budget returns
    /// `ExecutionError::NestedDepthExceeded`.
    pub async fn execute_plan_in_scope(
        &self,
        plan: &ExecutionPlan,
        ctx: &mut ExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<Vec<ExecutionResult>>;
}
```

Implementation sketch:

1. Check `ctx.depth_budget > 0`; else error.
2. Open a savepoint via `scope.open_nested()`.
3. Decrement depth budget, copy other context state.
4. Walk the plan; for each verb:
   - CRUD → route through `CrudExecutionPort` using the nested scope's executor.
   - Plugin → look up in the outer executor's `sem_os_ops` registry, dispatch with the nested
     scope as the `&mut dyn TransactionScope` argument.
   - Graph / other → existing codepaths, parameterized on the nested scope.
5. On success → nested scope `release()` → outer scope unchanged.
6. On error → nested scope `rollback()` → return error; outer scope free to catch or propagate.

### 3.4 Recursion depth enforcement

`ExecutionContext` gains a `depth_budget: u8` field (default 4). Each call to
`execute_plan_in_scope` decrements it on entry. Hitting 0 yields:

```rust
ExecutionError::NestedDepthExceeded { limit: 4, at_verb: "template.invoke" }
```

Budget of 4 handles the currently-known nested sites:

- `template_invoke_impl` — 1 level deep (invokes sub-templates, which could nest further).
- `onboarding.auto-complete` — 1 level.
- `gleif.enrich` — 1 level (entity.ensure via DSL).

A budget of 4 leaves room for future shallow composition; going much higher invites tail-recursion
bugs that we want to surface loudly rather than silently deepen.

### 3.5 Plugin dispatch inside a nested scope

Post-Phase-5c-migrate slice #80, every plugin op is dispatched via
`dispatch_plugin_via_sem_os_op(op, args, ctx, scope_dyn)` at `executor.rs:1503`. That path **already
takes** `&mut dyn TransactionScope`. The nested primitive simply passes the nested scope through.

No change to `SemOsVerbOp::execute` signature. The trait already threads `scope: &mut dyn
TransactionScope`.

### 3.6 Registry threading

The nested executor uses the **same** `sem_os_ops` registry as the outer executor. This eliminates
the F1-style "forgot to thread the registry" bug by construction — there is no second executor
instance, just a recursive call on the existing one.

---

## 4. Migration recipe (for Phase 7 slice executors)

**Before** (Pattern B write-path leak, `rust/src/domain_ops/template_ops.rs` style):

```rust
async fn execute(&self, args: &Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) {
    let pool = scope.pool().clone();  // ← leak
    let executor = DslExecutor::new(pool.clone());  // ← fresh txn root
    executor.execute_dsl(&inner_dsl, &mut inner_ctx).await?;
    // outer rollback cannot undo what executor committed.
}
```

**After**:

```rust
async fn execute(&self, args: &Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) {
    let outer = ctx.dsl_executor();  // canonical executor threaded by context
    let inner_plan = parse_and_compile(&inner_dsl)?;
    outer.execute_plan_in_scope(&inner_plan, ctx, scope).await?;
    // scope is reused; inner work commits or rolls back with outer atomically.
}
```

**Direct-SQL leaks** (non-nested) use `scope.executor()` (already defined in `tx.rs`):

```rust
// Before:
let pool = scope.pool().clone();
sqlx::query!("INSERT ...").execute(&pool).await?;
// After:
sqlx::query!("INSERT ...").execute(scope.executor()).await?;
```

---

## 5. Open questions

1. **`ctx.dsl_executor()` access.** Today `VerbExecutionContext` doesn't expose a reference to the
   canonical outer `DslExecutor`. The migration recipe above assumes it does. Options:
   - (a) Add `Arc<DslExecutor>` to `VerbExecutionContext` — weight cost = one Arc clone per verb.
   - (b) Pass `&dyn NestedDispatcher` through the context instead — narrower surface, same effect.
   - (c) Add a dedicated nested-dispatch service trait and look it up via
     `ctx.service::<dyn NestedDispatcher>()`. This matches the Phase 5a service-trait pattern.
   **Proposed:** (c). Consistent with existing infrastructure; doesn't widen the context struct.

2. **SAVEPOINT name generation.** Must be unique per scope and valid SQL identifier. Use
   `sp_<uuid_v4_short>` — 8 hex chars, no collisions in practice.

3. **Lock escalation across nested boundaries.** If the outer transaction holds a row lock and the
   nested unit tries to re-acquire it, Postgres is happy (same session). No change needed.

4. **Cancellation / timeout.** Out of scope for Slice 0.1. Future work should wire
   `tokio::select!` + cancellation tokens through the scope chain.

---

## 6. Test strategy for Slice 6.1 (the implementation slice)

Acceptance tests that must be green when Slice 6.1 merges:

1. **Nested success + outer success → both commit.**
2. **Nested success + outer failure → both roll back.** (The F2 correctness bug: today the nested
   pool-based path would commit despite outer rollback.)
3. **Nested failure + outer catches → nested rolls back, outer continues, outer can commit.**
4. **Nested failure + outer propagates → both roll back.**
5. **Depth budget exceeded → `NestedDepthExceeded` error surfaces before any execution.**
6. **Plugin dispatch inside nested scope → reaches the same `SemOsVerbOpRegistry` as outer.**
7. **SAVEPOINT name uniqueness** — property test with 10,000 concurrent nested scopes.
8. **Postgres error during SAVEPOINT release** — scope auto-rolls-back; outer can still commit
   other work.

The test harness for these already exists at
`rust/tests/transaction_rollback_integration.rs`. Extend it with nested cases rather than building
new infrastructure.

---

## 7. Rollback and cost

Slice 6.1 is additive. No existing caller has to change. Phase 7 slices *choose* to use the new
primitive where they find F2 leaks.

Rollback plan: revert Slice 6.1 commit. Phase 7 consumers revert to pool-based nested execution
(regression but recoverable).

Cost estimate: 400-600 LOC including tests.

---

## 8. Relationship to three-plane v0.3 §10.7

Post-slice state:

- Scope ownership: Sequencer (✓ per v0.3, existing).
- Mechanics inside scope: Runtime (✓, this primitive is the Runtime-side primitive that makes
  nested execution scope-respecting).
- Outbox for post-commit effects: unchanged (covered by F13 wiring).

This ADR does NOT attempt to solve §10.7 durability invariants that belong to the follow-on plan
(F5 envelope wiring, F6 single-dispatch invariant). It specifically enables Phase 7 Pattern B
sweep.

---

## 9. Decision

Proceed with Slice 6.1 implementation per the above sketch. Open question Q1 resolved in favour of
(c) service-trait lookup. Q2-Q4 inline.

Phase 7 slices (7.1-7.6) depend on this primitive; they do not start until Slice 6.1 merges.

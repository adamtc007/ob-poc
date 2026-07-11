# EOP-DESIGN-CONTROLPLANE-T9.2-ATOMIC-ADMISSION-001

### Design doc, not implementation. Review before any code lands.
### Basis: EOP-PLAN-CONTROLPLANE-001 Addendum B, T9.2 ("the restructure IS the tranche")
### Status: DRAFT — proposal, 2026-07-11

---

## 0. What T9.2 requires and why it's flagged first, not coded first

T9.2's exit criteria (Addendum B): envelope consumption, pin verification
(`verify_pins`), and the verb's write must all happen **in one transaction
scope** — so nothing can commit a conflicting change between the admission
check and the write it's admitting. Today they don't: `execute_verb_admitting_envelope`
calls `self.admit(...)` (its own pool-acquired, independently-committed
UPDATE) and then `self.execute_verb(...)` (a *separate* transaction,
opened fresh) — a real gap between admission and write that a concurrent
writer could exploit.

Closing that gap turned out to require touching **three structurally
different, all-live-in-production transaction mechanisms**, not one
function. That's why this landed as a design doc first — Addendum B's own
execution note pre-authorized exactly this outcome ("a flagged design
constraint beats a shipped approximation"), and the finding was recorded
in the ownership ledger's "Tranche T9.2" entry before this doc was
written. This doc is the follow-through: here is the actual shape of the
fix, for review before any code changes.

---

## 1. Current state (verified against code, 2026-07-11)

`ObPocVerbExecutor::execute_verb` (`src/sem_os_runtime/verb_executor_adapter.rs`)
routes a dispatched verb to one of three branches, selected at runtime by
the verb's declared behavior:

| Branch | Code | Transaction mechanism | Production-wired? |
|---|---|---|---|
| **1. SemOS-native ops** | `verb_executor_adapter.rs` ~L225-289 | `PgTransactionScope::begin(pool)` → `op.execute(&args, ctx, scope_dyn)` → `scope.commit()`/`rollback()`. Already `TransactionScope`-based. | Yes — the primary path for migrated verbs (Phase 5c-migrate). |
| **2. CRUD fast path** | `verb_executor_adapter.rs` ~L293-309, dispatches to `PgCrudExecutor::execute_crud` (`crates/dsl-runtime/src/crud_executor.rs`) | Holds a bare `pool: PgPool`. Each `execute_select`/`execute_insert`/`execute_update`/`execute_delete`/`execute_upsert` call acquires its own connection from the pool — **no transaction at all today**, each statement autocommits independently. | Yes — wired via `main.rs:1588` (`PgCrudExecutor::new`), `with_crud_port`. |
| **3. Generic path** | `verb_executor_adapter.rs` ~L311-320 → `dsl_v2::executor::DslExecutor::execute_verb` | Opens its own transaction internally (`execute_verb_inner`), **or** accepts a caller-supplied scope via the existing `execute_verb_in_scope(vc, ctx, scope: &mut dyn TransactionScope)` method — branch 3 today calls the *no-scope* variant (`execute_verb`), not `execute_verb_in_scope`. | Yes — the fallback for unmigrated/plugin/graph_query/durable verbs. |

Admission today (`ObPocVerbExecutor::admit`, called from
`execute_verb_admitting_envelope` before any of the three branches runs):
`check_admission` → `try_consume` — both take `&sqlx::PgPool` and run
their own implicit transaction (a single `UPDATE ... RETURNING` — sqlx
autocommits a lone statement), fully independent of whichever of the
three branches executes afterward.

`verify_pins` (`ob-poc-boundary::toctou_recheck`) has **zero production
call sites** — T9.2 is also the first tranche to actually wire it in, not
just close a gap in an existing wiring.

---

## 2. Design principle

**One `PgTransactionScope`, opened once, before branch selection.**
Consumption, pin verification, and the branch's own write all execute
against that single scope's connection. Commit only after the write
succeeds; roll back on failure at *any* step (consume, verify, or write)
— the whole thing is one atomic unit, matching the exit criterion
literally ("in one transaction scope").

```
execute_verb_admitting_envelope(verb_fqn, args, ctx, envelope_handle):
    scope = PgTransactionScope::begin(pool)          # ONE open, before branching
    try:
        admit_in_scope(scope, verb_fqn, envelope_handle)   # was: admit() on its own txn
        verify_pins_in_scope(scope, envelope.snapshot_pins, resolved_entities)  # NEW — zero callers today
        result = dispatch_in_scope(scope, verb_fqn, args, ctx)  # was: execute_verb(), branches on behavior
        scope.commit()
        return result
    except any:
        scope.rollback()
        raise
```

`dispatch_in_scope` is where the three-branch unification happens — see
§3. Everything upstream of it (open scope, admit, verify pins) is new,
uniform code; only the write dispatch itself needs three different
adaptations.

---

## 3. Per-branch changes

### Branch 1 — SemOS-native ops: trivial

Already `TransactionScope`-based. Change: stop opening its *own* scope
(`PgTransactionScope::begin(pool)` inside the branch) and instead use the
scope the caller already opened at the top of
`execute_verb_admitting_envelope`. The `op.execute(&args, ctx, scope_dyn)`
call is unchanged — only who owns `begin`/`commit`/`rollback` moves
outward. Net: delete ~15 lines (the branch's own scope-open/commit/
rollback), the outer wrapper's commit/rollback covers it instead.

### Branch 3 — Generic path: trivial

`self.executor.execute_verb(&vc, &mut exec_ctx)` → `self.executor.execute_verb_in_scope(&vc, &mut exec_ctx, scope_dyn)`. The scope-accepting entry point already exists (`dsl_v2::executor::DslExecutor::execute_verb_in_scope`, `src/dsl_v2/executor.rs:1914`) and its `pub(crate)` visibility already covers this call site — `verb_executor_adapter.rs` (`src/sem_os_runtime/`) is in the same crate (`ob-poc`), confirmed by its existing `use dsl_runtime::...` imports (external-crate style) vs. its bare `crate::dsl_v2::...` reach for this one — no visibility widening needed. No new code in `dsl_v2::executor` itself required — `dsl_v2/executor.rs` stays untouched, consistent with T9.3's precedent of never touching that file's internals.

### Branch 2 — CRUD fast path: the real work

`CrudExecutionPort` (`crates/dsl-runtime/src/port.rs:92`) needs a new
method:

```rust
#[async_trait]
pub trait CrudExecutionPort: Send + Sync {
    async fn execute_crud(&self, contract, args, ctx) -> Result<VerbExecutionOutcome>;  // existing, unchanged

    /// New: same operation, but against a caller-supplied transaction
    /// scope instead of this port's own pool.
    async fn execute_crud_in_scope(
        &self,
        contract: &VerbContractBody,
        args: serde_json::Value,
        ctx: &VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome>;
}
```

`PgCrudExecutor` needs a real implementation of the new method: each of
`execute_select`/`execute_insert`/`execute_update`/`execute_delete`/
`execute_upsert` currently takes `&self` and queries `self.pool` directly
— they need scope-accepting siblings (or a generic executor parameter)
that query `scope.executor()` (`&mut PgConnection`) instead. This is the
largest single piece of new code in T9.2 — five query-builder methods,
each needing a scope-taking variant. Existing `execute_crud` (pool-based)
stays for any caller that still wants pool semantics (none should, once
T9.2 lands, but removing it is out of scope for this tranche — see §7).

---

## 4. Admission primitives: genericize, don't duplicate

`check_admission`/`try_consume` (`agent::control_plane_envelope_store`)
currently take `&sqlx::PgPool`. Widen to `impl sqlx::PgExecutor<'_>`
(implemented by both `&PgPool` and `&mut PgConnection` in sqlx) —
existing pool-based callers (T9.3's `admit_plan`, the bus path, the
runbook path) are unaffected; the new scope-based caller in
`execute_verb_admitting_envelope` passes `scope.executor()`. One
implementation, two callers, no drift risk — same pattern T9.3 already
established with `admit_plan`/`admit_plan_checked`.

---

## 5. Pin verification: don't reuse `RowVersionProvider` as-is

`RowVersionProvider`/`SqlRowVersionProvider` (`ob-poc-boundary::toctou_recheck`)
is `&self`-based, holding `&'a sqlx::PgPool` — safe to share because
`PgPool: Clone + Sync`. A live transaction (`&mut PgConnection`) is a
single-owner, exclusively-borrowed type; it doesn't fit that shape
without interior mutability (a `Mutex`-wrapped connection, or similar) —
not worth the complexity for what's actually a narrow, one-shot need at
this call site.

**Proposal: a new free function, not a trait impl**, mirroring
`verify_pins`'s signature but taking the executor directly per call:

```rust
pub async fn verify_pins_in_scope(
    pins: &SnapshotPins,
    entity_kinds: &[(Uuid, String)],
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<(), anyhow::Error>
```

Internally: the same per-kind `SELECT row_version FROM ... WHERE pk = $1`
`toctou_recheck.rs::SqlRowVersionProvider::row_version` already runs, just
against the passed executor instead of a stored pool reference. (Could
even become a **batched** query reusing T9.1-pre's `EntityFactsSource`
per-kind table mapping — `crates/ob-poc-boundary/src/entity_facts.rs`
already has the 5-kind table/PK mapping this needs; worth checking at
implementation time whether `verify_pins_in_scope` should just call
`entity_facts`'s batched query and compare `row_version` fields, rather
than re-deriving the same table mapping a third time.)

`entity_kinds`/`pins` need to come from the envelope's own resolved
entities and snapshot pins — check `ExecutionEnvelope`/`GatedVerbEnvelope`'s
actual field shape at implementation time; not fully traced in this design
pass.

---

## 6. Ordering, failure handling, rollout

**Ordering within the transaction:** `BEGIN` → consume envelope → verify
pins → dispatch verb write (branch-specific) → `COMMIT`. Any failure at
any step → `ROLLBACK`, propagate the error. No partial commits.

**Rollout safety — this only bites enforced verbs:** `check_admission`'s
`NotEnforced` fast path (empty `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`,
production default) is unaffected in shape — it's still a cheap early
return, just now evaluated against `scope.executor()` instead of a fresh
pool connection (functionally identical, same query, same table).
`verify_pins_in_scope` should likewise no-op (or skip) when the envelope
carries no snapshot pins — matching `verify_pins`'s existing
"entity absent from pin set is silently skipped" posture. Net: **for
every path today (nothing enforced yet), T9.2's changes should be
behaviorally invisible** — same dispatch outcome, same commit/rollback
timing from the caller's perspective, just now sharing one transaction
instead of two. This should be provable, not assumed: see testing below.

**Testing strategy (re-running PIR's own probes, per Addendum B's own
requirement):**
- Live-DB **concurrent-consume probe**: two concurrent callers race to
  consume the same envelope inside the new atomic scope; exactly one
  succeeds, the other observes `AlreadyConsumed` — cited against the
  actual transaction boundary (which `BEGIN`/`COMMIT` the assertion is
  checking against), not just "eventually consistent."
- Live-DB **fault-injection abort probe**: force the write branch to fail
  after admission succeeds (e.g. a constraint violation); assert the
  envelope's consumption is *rolled back too* (re-consumable afterward,
  or provably not marked consumed) — this is the actual atomicity
  property T9.2 exists to prove, and the one most likely to reveal a
  branch that isn't really using the shared scope.
- Re-run for each of the three branches separately — a SemOS-native op, a
  CRUD verb, and a generic/plugin verb — since the unification is
  branch-specific code, not one shared code path.
- Re-run after every commit touching `control_plane_envelope_store.rs`,
  `verb_executor_adapter.rs`, or the CRUD executor — per Addendum B's own
  instruction.

---

## 7. Explicitly out of scope for this tranche

- Removing the pool-based `execute_crud`/`admit`/`check_admission`/
  `verify_pins` variants — keep them for any caller not yet migrated to
  scope-based dispatch (there shouldn't be any once T9.2 lands, but
  deleting is a separate, lower-risk cleanup commit).
- `execute_verb` (the non-admitting entry point) — unaffected; this design
  only touches `execute_verb_admitting_envelope`'s call chain.
- Wiring `verify_pins`/pin capture into T9.1's shadow-only gates
  (`agent::control_plane_shadow`) — that's a *different* consumer of
  `row_version` data (shadow evaluation, non-gating), not this tranche's
  concern. T9.1-pre's `EntityFactsSource` already returns `row_version`
  for exactly this future use, per its own design-pass note.

---

## 8. Open questions for review

1. ~~Is `dsl_v2::executor::DslExecutor::execute_verb_in_scope`'s current
   `pub(crate)` visibility sufficient for `verb_executor_adapter.rs` to
   call it?~~ **Resolved during this design pass:** yes — confirmed
   `verb_executor_adapter.rs` is in the same crate (`ob-poc`), no
   visibility change needed.
2. `CrudExecutionPort::execute_crud_in_scope` — new trait method, default
   implementation possible (delegate to `execute_crud` ignoring the scope,
   degrading to the old pool-based behavior) so existing implementors
   don't break, matching `execute_verb_admitting_envelope`'s own precedent
   of a default degrading to the legacy path? Or should it be a hard
   requirement (no default) so a silently-wrong non-atomic fallback can't
   ship unnoticed? Leaning toward **no default** here specifically,
   because unlike `execute_verb_admitting_envelope`'s envelope-handle
   default (degrading to `execute_verb`, which was already the *entire*
   existing behavior), a silent pool-based fallback inside what's supposed
   to be an atomic scope would defeat T9.2's whole purpose without any
   compiler signal.
3. `verify_pins_in_scope`'s exact input shape (`entity_kinds`/`pins`
   sourced from where on the envelope) needs tracing through
   `ExecutionEnvelope`/`GatedVerbEnvelope` before implementation — flagged
   as unresolved in §5, not guessed at here.

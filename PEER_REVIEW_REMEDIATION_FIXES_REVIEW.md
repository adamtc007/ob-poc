# Peer Review Remediation Fixes — Verification Review

**Artifact reviewed:** `peer-review-remediation-fixes.tar.gz`  
**Files included (6):**
- `rust/src/repl/orchestrator_v2.rs`
- `rust/src/repl/runbook.rs`
- `rust/src/repl/session_v2.rs`
- `rust/src/repl/session_repository.rs`
- `rust/src/runbook/compiler.rs`
- `rust/src/runbook/executor.rs`

**Baseline:** My prior review of the “execution gate remediation” PR identified remaining issues: raw execution bypasses (ScopeGate + HumanGate), locking not held during execution, status corruption on contention, stubby on-the-fly compilation, and non-persisted monotonic runbook versioning.

---

## 1) Headline result

✅ **REPL V2 now enforces “execute only via `CompiledRunbookId`” for the previously identified bypasses** (ScopeGate + HumanGate paths).  
✅ **Advisory locks are now held for the entire execution window** (transaction kept alive until completion).  
✅ **Status corruption on lock contention is fixed** (locks acquired before marking `Executing`).  
✅ **Monotonic version allocator is persisted** via `runbook.next_version_counter` in runbook JSON.  
⚠️ Two small semantic/edge risks remain (see §4).

---

## 2) Fix-by-fix validation

### Fix A — Remove raw DSL execution bypasses (ScopeGate + HumanGate) ✅

**Previous gaps:** `complete_scope_gate()` and human-gate approval resume executed `self.executor.execute(&dsl)` directly.

**Now:** both paths route through `execute_entry_via_gate(...)` with the comment `INV-3: no raw DSL execution`, and **no occurrences** of `.executor.execute(` remain in `orchestrator_v2.rs`.

- `complete_scope_gate(...)` now:
  - adds entry,
  - allocates a runbook version (`session.allocate_runbook_version()`),
  - calls `execute_entry_via_gate(...)`.

- `handle_human_gate_approval(...)` now:
  - resumes entry,
  - allocates a runbook version,
  - calls `execute_entry_via_gate(...)`.

Additionally, `runbook/executor.rs` includes a **regression test** that scans the source of `orchestrator_v2.rs` and fails if any `self.executor.execute(` call appears (non-comment, non-test-only).

**Verdict:** The specific bypasses I flagged are fixed.

---

### Fix B — Locking is “real” (locks held across the execution window) ✅

**Previous gap:** locks were acquired inside a transaction and then committed immediately, releasing locks before step execution.

**Now:** `acquire_advisory_locks(...)` returns the open transaction, and `execute_runbook_with_pool(...)` stores it as `_lock_tx` and keeps it alive during step execution. The transaction is committed at the end; on early return/drop it rolls back, releasing locks.

Also improved:
- Lock acquisition happens **before** status transitions, fixing “stuck Executing” on contention.

**Verdict:** Lock lifetime is now correct for an advisory-lock-based exclusion model.

---

### Fix C — Status ordering prevents corruption on contention ✅

`execute_runbook_with_pool(...)` now:
1) computes write_set
2) attempts lock acquisition
3) only then updates status to `Executing`

So a lock contention error no longer leaves the runbook in `Executing`.

**Verdict:** Fixed.

---

### Fix D — On-the-fly compilation is less stubby ✅ (with one nuance)

Previously the fallback compiler hard-coded:
- `runbook_version: 0`
- empty `write_set`

**Now:** `compile_entry_on_the_fly(...)`:
- requires a caller-provided `version` (documented as coming from `session.allocate_runbook_version()`),
- populates `CompiledStep.write_set` via `derive_write_set(&entry.args)`.

**Verdict:** The fallback is now materially closer to the intended safety model.

---

### Fix E — Monotonic version allocation is persisted ✅

**Previous gap:** load reconstructed version counter from `entries.len()`, causing reuse/collisions.

**Now:**
- Runbook owns `next_version_counter: u64` with `#[serde(default)]`.
- `ReplSessionV2::allocate_runbook_version()` increments `runbook.next_version_counter`.
- Repository load:
  - deserializes the runbook JSON,
  - rebuilds transient indexes,
  - if `next_version_counter==0` and entries exist (old sessions), floors to `entries.len()`,
  - syncs legacy `next_runbook_version` from the persisted counter.

**Verdict:** This closes the persistence hole.

---

## 3) Alignment with your stated invariant

Your claim: **“No DSL executes without a `CompiledRunbookId`. The fallback path compiles on-the-fly → stores → executes through the gate.”**

For **REPL V2**, that claim now matches the code:
- ScopeGate / HumanGate execute via `execute_entry_via_gate`.
- `execute_entry_via_gate`:
  - resolves existing `compiled_runbook_id` **or**
  - compiles on-the-fly, inserts into store, then executes via `execute_runbook_with_pool`.

✅ For the REPL orchestration paths included in this patch: invariant is enforced.

---

## 4) Remaining risks / polish items (small but real) ⚠️

### 4.1 Optional pool means “silent unlock” is still possible
If `pool: None` is passed into `execute_runbook_with_pool(...)`, it will skip DB locks even for non-empty write_set.

- In production REPL, orchestrator passes `self.pool.as_ref()` and should be `Some`.
- But in tests/dev harnesses it could be `None`.

**Suggestion:** Make “no pool” an explicit mode:
- either return an error for mutating steps when pool is None, or
- log loudly and mark `lock_stats` as “unlocked” rather than “locks_acquired = write_set.len()`.

### 4.2 Fallback compiled step hard-codes `execution_mode: Sync`
`compile_entry_on_the_fly(...)` sets:
- `execution_mode: CompiledExecutionMode::Sync`

Even if the original `RunbookEntry.execution_mode` is Durable.

Right now, `execute_entry_via_gate(...)` chooses a durable vs sync StepExecutor based on `is_durable`, which mitigates most behavior. But it’s cleaner (and safer for future logic) to set the compiled step’s execution mode from the entry.

**Suggestion:** map entry.execution_mode → compiled.execution_mode in fallback.

---

## 5) Conclusion

✅ This patch closes the core outstanding gaps from my last review for **REPL V2**:
- no more raw execution bypasses in ScopeGate/HumanGate,
- locks held across execution,
- status ordering fixed,
- version allocator persisted,
- fallback compilation improved.

Only minor follow-up remains around:
- tightening “pool None” behavior,
- aligning fallback compiled execution_mode with entry mode.

If you want “no loose ends” at the repo level (beyond REPL), the next check would be whether **chat/API execution** also routes through the same gate. This patch doesn’t touch those surfaces, so that broader invariant should still be treated as “REPL V2 enforced; chat pending”.

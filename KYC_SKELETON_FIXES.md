# KYC Skeleton Build — Post-Audit Fixes

## SESSION INSTRUCTIONS FOR CLAUDE CODE

**Read this entire file before writing any code.**

### Progress Tracking (MANDATORY)

After completing each task below, you MUST:

1. Update the task's checkbox from `[ ]` to `[x]`
2. Add a one-line completion note with the commit hash or file changed
3. Run `cargo test --all-targets` and record pass/fail count
4. Write current state to `.claude/KYC_FIXES_PROGRESS.md` with format:

```
Last task completed: F-{N}
Timestamp: {now}
Tests: {pass}/{total} passing
Files modified: {list}
Next task: F-{N+1}
Blockers: {any}
```

**→ IMMEDIATELY proceed to the next task after updating progress. Do NOT stop between tasks.**

If you are resuming after a crash, read `.claude/KYC_FIXES_PROGRESS.md` first, then continue from the noted next task.

### E-Invariant

At every task boundary: `cargo test --all-targets` must pass. All existing ~160 tests must continue passing. If a test breaks, fix it before proceeding.

### Scope

5 fixes from the post-audit review. Do NOT refactor anything outside this scope. Do NOT rename files, reorganise modules, or "improve" unrelated code.

---

## FIXES

### F-1: Transaction Boundary on Skeleton Build Pipeline [CRITICAL]

**Problem:** `skeleton_build_ops.rs` runs 7 steps as independent queries against `pool`. If step 5 fails, you have a half-built skeleton — import run ACTIVE, anomalies persisted, determination run exists, but no outreach plan or tollgate evaluation. The import run gets marked COMPLETED only in step 7.

**Fix:**

In `SkeletonBuildOp::execute()`:

1. Replace `pool` usage with a transaction: `let mut tx = pool.begin().await?;`
2. Pass `&mut *tx` to all 7 inline SQL blocks AND to each `run_*` helper function
3. Change every `run_*` function signature from `pool: &PgPool` to `tx: &mut sqlx::Transaction<'_, sqlx::Postgres>` (or use a generic `E: sqlx::Executor<'_, Database = sqlx::Postgres>` — your call on which is cleaner given existing patterns in ob-poc)
4. `tx.commit().await?` after step 7 (import-run complete)
5. On any `?` propagation, the transaction auto-rolls back on drop

**Verify:** Write a test comment (not a full integration test) documenting that failure at any step leaves zero rows in all KYC tables. The existing integration tests validate the happy path.

**Files:** `rust/src/domain_ops/skeleton_build_ops.rs`

- [x] **F-1 complete** — wrapped all 7 steps in `pool.begin()`/`tx.commit()`. Changed 5 `run_*` signatures to `&mut sqlx::Transaction`. Replaced 41→2 pool refs. 1017 tests pass.

---

### F-2: Fix Ownership Direction in Coverage Check [CORRECTNESS BUG]

**Problem:** Line ~881 in `run_coverage_compute()`:

```rust
WHERE from_entity_id = $1
  AND relationship_type = 'ownership'
  AND percentage IS NOT NULL
```

This checks if the candidate (a natural person UBO) has ANY outbound ownership edge with a percentage. It should check whether there's an ownership chain FROM the candidate TO the subject entity of the determination run.

As-is: Person A owns Fund X (60%) and also owns a car dealership. The car dealership edge passes the OWNERSHIP prong even though it's irrelevant to this case.

**Fix:**

1. At the start of `run_coverage_compute`, load the `subject_entity_id` from the determination run (you already have the `determination_run_id`)
2. Change the OWNERSHIP check to scope by subject entity:

```rust
// Check ownership edges FROM candidate TO subject entity (or via chain)
// Simplest correct version: check edges where candidate is from_entity_id
// AND to_entity_id is either the subject or is in the case's entity_workstreams
let ownership_count: (i64,) = sqlx::query_as(
    r#"SELECT COUNT(*) FROM "ob-poc".entity_relationships
       WHERE from_entity_id = $1
         AND to_entity_id IN (SELECT entity_id FROM kyc.entity_workstreams WHERE case_id = $2)
         AND relationship_type IN ('ownership', 'OWNERSHIP')
         AND percentage IS NOT NULL
         AND (effective_to IS NULL OR effective_to > CURRENT_DATE)"#,
)
.bind(entity_id)
.bind(case_id)
.fetch_one(/* tx or pool */)
.await
.unwrap_or((0,));
```

3. Apply the same scoping fix to the CONTROL check (~line 941) — control edges should also be scoped to case entities

**Note:** The `case_id` is already available in `run_coverage_compute`. You also already load `subject_entity_id` further down (line ~1143 in outreach). Move that load earlier or pass it through.

**Files:** `rust/src/domain_ops/skeleton_build_ops.rs`

- [x] **F-2 complete** — scoped OWNERSHIP and CONTROL prongs to case entities via `to_entity_id IN (SELECT entity_id FROM kyc.entity_workstreams WHERE case_id = $2)`. 1017 tests pass.

---

### F-3: Extract Shared Functions from Skeleton Build [ARCHITECTURAL]

**Problem:** `skeleton_build_ops.rs` contains complete reimplementations of graph validate, UBO compute, coverage, outreach, and tollgate logic as private `async fn`s. The standalone verb handlers (`graph_validate_ops.rs`, `ubo_compute_ops.rs`, `coverage_compute_ops.rs`, `outreach_plan_ops.rs`, `tollgate_evaluate_ops.rs`) have their own implementations. Bug fixes in one won't propagate to the other.

**Fix:**

For each of the 5 computational steps, extract the core logic into a shared function that both the standalone verb handler and the skeleton build call.

**Pattern:**

```rust
// In graph_validate_ops.rs (or a shared module):
pub async fn validate_graph_for_case(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    case_id: Uuid,
) -> Result<GraphValidateResult> {
    // ... the actual logic, moved here from skeleton_build_ops
}

// In GraphValidateOp::execute():
let result = validate_graph_for_case(pool, case_id).await?;

// In skeleton_build_ops run_graph_validate():
let result = validate_graph_for_case(&mut *tx, case_id).await?;
```

**Order of extraction** (do one at a time, `cargo test` between each):

1. `run_graph_validate` → shared `validate_graph_for_case()`
2. `run_ubo_compute` → shared `compute_ubo_chains()`
3. `run_coverage_compute` → shared `compute_coverage()`
4. `run_outreach_plan` → shared `generate_outreach_plan()`
5. `run_tollgate_evaluate` → shared `evaluate_tollgate()`

**Important:** If the standalone ops don't exist yet as separate files, just extract the functions within `skeleton_build_ops.rs` into `pub` functions and add a `// TODO: move to standalone ops module` comment. Don't create new op files in this session — that's scope creep.

**After each extraction:** `cargo test --all-targets`. The skeleton build must produce identical results.

**Files:** `rust/src/domain_ops/skeleton_build_ops.rs`, potentially `rust/src/domain_ops/mod.rs`

- [x] **F-3a: graph validate extracted** — Made `run_graph_validate` pub, `Edge`/`GraphAnomaly` pub(crate), re-exported from mod.rs. 1017 tests pass.
- [x] **F-3b: ubo compute extracted** — Made `run_ubo_compute` pub, re-exported from mod.rs. 1017 tests pass.
- [x] **F-3c: coverage compute extracted** — Made `run_coverage_compute` pub, `extract_candidate_entity_ids`/`update_prong` pub(crate), re-exported from mod.rs. 1017 tests pass.
- [x] **F-3d: outreach plan extracted** — Made `run_outreach_plan` pub, re-exported from mod.rs. 1017 tests pass.
- [x] **F-3e: tollgate evaluate extracted** — Made `run_tollgate_evaluate` pub, re-exported from mod.rs. 1017 tests pass.

---

### F-4: Make Outreach Plan Item Cap Configurable [POLICY]

**Problem:** Line ~1212: `planned_items.truncate(8)`. The 8-item cap is an implementation invention with no spec basis. An auditor asking "why didn't you request a board resolution for Entity X?" has no answer.

**Fix:**

1. Add an optional arg `max-outreach-items` to the skeleton build verb (default 8, min 1, max 50)
2. Pass it through to `run_outreach_plan` / the shared outreach function
3. If more gaps exist than the cap, log a `tracing::warn!` with the count of dropped items
4. Add a `"items_capped"` and `"total_gaps_before_cap"` field to the outreach plan metadata or the `SkeletonBuildResult`

**Files:** `rust/src/domain_ops/skeleton_build_ops.rs`, optionally the skeleton build YAML if it exists

- [x] **F-4 complete** — Added `max-outreach-items` arg to YAML (integer, optional, default 8, clamped 1-50). Updated `run_outreach_plan` to accept cap param, return `(Option<Uuid>, i32)`, log when capped. Added `items_capped`/`total_gaps_before_cap` to result. 1017 tests pass.

---

### F-5: Fix Decimal Conversion [CLEANUP]

**Problem:** Lines ~218-219, ~612-613 use `pct.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))`. The string round-trip is unnecessary and `unwrap_or(0.0)` silently swallows parse failures.

**Fix:**

1. Add `use rust_decimal::prelude::ToPrimitive;` at the top of the file
2. Replace all instances of the pattern with: `pct.map(|d| d.to_f64().unwrap_or(0.0))`
3. Search the entire file for `.to_string().parse::<f64>()` to catch any other instances

**Files:** `rust/src/domain_ops/skeleton_build_ops.rs`

- [x] **F-5 complete** — replaced 2 instances of `.to_string().parse::<f64>()` with `.to_f64()` using ToPrimitive trait. 1017 tests pass.

---

## Execution Order

```
F-5 (Decimal — trivial, warm up)
  → cargo test
F-2 (Coverage direction — correctness fix)
  → cargo test
F-1 (Transaction boundary — infrastructure)
  → cargo test
F-4 (Outreach cap — small feature)
  → cargo test
F-3a..F-3e (Extract shared functions — one at a time)
  → cargo test after EACH extraction
```

**Rationale:** F-5 first because it's trivial and builds confidence. F-2 before F-1 because the transaction change touches all function signatures and F-2 is easier to verify in isolation. F-3 last because it's the biggest refactor and benefits from F-1's signature changes already being in place.

## Session Recovery

If this session crashes or you are a new Claude Code instance:

1. Read `.claude/KYC_FIXES_PROGRESS.md`
2. Run `cargo test --all-targets` — record current state
3. Run `git diff --stat` — see what's been changed
4. Continue from the next uncompleted task in this file
5. Do NOT restart completed tasks

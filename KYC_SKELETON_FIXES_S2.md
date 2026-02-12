# KYC Post-Audit Fixes — Session 2 (F-6 through F-8)

## SESSION INSTRUCTIONS FOR CLAUDE CODE

**Read this entire file before writing any code.**

### Prerequisites

Session 1 (KYC_SKELETON_FIXES.md) is complete. The following are already done:

- F-1: Transaction boundary on skeleton build pipeline ✅
- F-2: Ownership direction scoped in coverage check ✅
- F-3a-e: Shared functions extracted from skeleton build ✅
- F-4: Outreach plan item cap configurable ✅
- F-5: Decimal conversion cleanup ✅

**Before starting:** Run `cargo test --all-targets` and confirm all tests pass. If they don't, STOP and report — do not proceed on a broken baseline.

### Progress Tracking (MANDATORY)

After completing each task below, you MUST:

1. Update the task's checkbox from `[ ]` to `[x]`
2. Add a one-line completion note with the commit hash or file changed
3. Run `cargo test --all-targets` and record pass/fail count
4. Write current state to `.claude/KYC_FIXES_S2_PROGRESS.md` with format:

```
Last task completed: F-{N}
Timestamp: {now}
Tests: {pass}/{total} passing
Files modified: {list}
Next task: F-{N+1}
Blockers: {any}
```

**→ IMMEDIATELY proceed to the next task after updating progress. Do NOT stop between tasks.**

If you are resuming after a crash, read `.claude/KYC_FIXES_S2_PROGRESS.md` first, then continue from the noted next task.

### E-Invariant

At every task boundary: `cargo test --all-targets` must pass. All existing tests must continue passing. If a test breaks, fix it before proceeding.

### Scope

3 fixes from the ChatGPT reconciliation review + import-run idempotency tightening. Do NOT refactor anything outside this scope.

---

## FIXES

### F-6: Convert `update-status` from CRUD to Plugin with State Machine Validation [COMPLIANCE]

**Problem:** `kyc-case.yaml` defines `update-status` as `behavior: crud` with a raw UPDATE on `kyc.cases`. No transition validation. An agent or template can move a case from INTAKE directly to APPROVED, skipping DISCOVERY, ASSESSMENT, and REVIEW. This breaks the compliance model — regulators need to see that every case followed the mandated progression.

**Why this matters:** Every other verb in the system assumes the case state machine is enforced. The tollgate evaluations, evidence requirements, and UBO registry progression all depend on the case being in the correct state. A CRUD bypass makes all of those checks advisory rather than mandatory.

**Fix:**

#### F-6a: Create the transition map

Define the allowed transitions as a const in `kyc_case_ops.rs`:

```rust
/// Allowed case status transitions.
/// Key = current status, Value = set of valid next statuses.
const CASE_TRANSITIONS: &[(&str, &[&str])] = &[
    ("INTAKE",      &["DISCOVERY", "WITHDRAWN"]),
    ("DISCOVERY",   &["ASSESSMENT", "BLOCKED", "WITHDRAWN"]),
    ("ASSESSMENT",  &["REVIEW", "BLOCKED", "WITHDRAWN"]),
    ("REVIEW",      &["APPROVED", "REJECTED", "REFER_TO_REGULATOR",
                       "DO_NOT_ONBOARD", "BLOCKED", "WITHDRAWN"]),
    ("BLOCKED",     &["DISCOVERY", "ASSESSMENT", "REVIEW", "WITHDRAWN"]),
    // Terminal states — no outbound transitions (use reopen verb instead)
    ("APPROVED",    &[]),
    ("REJECTED",    &[]),
    ("WITHDRAWN",   &[]),
    ("EXPIRED",     &[]),
    ("REFER_TO_REGULATOR", &[]),
    ("DO_NOT_ONBOARD",     &[]),
];
```

Build a `HashMap` or just do a linear scan — there are only ~11 entries. Validate both:
- Current status has an entry (case exists and is in known state)
- Requested status is in the allowed set

Return `Err(anyhow!("Invalid transition: {} → {}", current, requested))` on violation.

- [x] **F-6a complete** — Added CASE_TRANSITIONS const, is_valid_transition() and is_terminal_status() helpers. 1017 tests pass.

#### F-6b: Create `KycCaseUpdateStatusOp` plugin handler

New struct in `kyc_case_ops.rs`:

```rust
#[register_custom_op]
pub struct KycCaseUpdateStatusOp;
```

Implementation:

1. Extract `case-id` and `status` from verb args
2. Load current case status: `SELECT status FROM kyc.cases WHERE case_id = $1`
3. Validate transition against the map from F-6a
4. If valid: `UPDATE kyc.cases SET status = $2, updated_at = NOW() WHERE case_id = $1`
5. Return `ExecutionResult::Affected(1)` on success
6. Optional: extract `notes` arg and append to case notes on transition

**Important:** The existing `KycCaseCloseOp` already validates that the case is in REVIEW before closing. After F-6b, `update-status` handles all non-terminal transitions, and `close` handles terminal transitions. There should be no overlap — `update-status` should reject terminal statuses (APPROVED, REJECTED, etc.) and direct callers to use `close` instead. Add a clear error message: `"Use kyc-case.close for terminal status '{}'"`.

- [x] **F-6b complete** — Created KycCaseUpdateStatusOp with transition validation and terminal status redirection. 1017 tests pass.

#### F-6c: Update YAML

In `rust/config/verbs/kyc/kyc-case.yaml`, change the `update-status` verb:

```yaml
      update-status:
        description: Update case status with transition validation
        # ... keep existing invocation_phrases, metadata, args ...
        behavior: plugin
        handler: KycCaseUpdateStatusOp
        # REMOVE the crud: block entirely
        returns:
          type: affected
```

Do NOT change any other verb in the YAML file.

- [x] **F-6c complete** — Changed update-status from behavior:crud to behavior:plugin with KycCaseUpdateStatusOp handler. Removed terminal statuses from valid_values. Added notes arg. 1017 tests pass.

#### F-6d: Add unit tests for transition validation

In the `#[cfg(test)] mod tests` block in `kyc_case_ops.rs`, add:

```rust
#[test]
fn test_valid_transitions() {
    // INTAKE → DISCOVERY: allowed
    // INTAKE → ASSESSMENT: rejected
    // DISCOVERY → ASSESSMENT: allowed
    // REVIEW → APPROVED: rejected (must use close verb)
    // APPROVED → anything: rejected (terminal)
    // BLOCKED → DISCOVERY: allowed (unblock path)
}
```

Implement these as assertions against a `fn is_valid_transition(from: &str, to: &str) -> bool` helper that you extract from the handler logic. Test the helper directly — no need for async/DB in these tests.

Also add a test that terminal statuses return an appropriate "use close verb" message:

```rust
#[test]
fn test_terminal_status_redirects_to_close() {
    // Attempting REVIEW → APPROVED via update-status should fail
    // with message containing "close"
}
```

- [x] **F-6d complete** — Added 4 tests: test_valid_transitions, test_terminal_states_have_no_outbound_transitions, test_terminal_status_redirects_to_close, test_update_status_op_metadata. 1021 tests pass (4 new).

#### F-6e: Update integration tests

In `rust/tests/kyc_full_lifecycle.rs`, the `test_full_case_lifecycle` and `test_invalid_state_transitions_rejected` tests currently use raw SQL `UPDATE kyc.cases SET status = ...`. These tests should still pass as-is (they bypass the verb layer), but add a comment:

```rust
// NOTE: This test operates at SQL level, bypassing KycCaseUpdateStatusOp
// transition validation. Verb-level transition tests are in kyc_case_ops::tests.
```

Do NOT rewrite the integration tests to use the verb handler — that's a separate session's work.

- [x] **F-6e complete** — Added SQL-bypass comments to update_case_status helper, test_full_case_lifecycle, and test_invalid_state_transitions_rejected. 1021 tests pass.

---

### F-7: Import Run Begin — Case Linkage on Idempotent Hit [DATA INTEGRITY]

**Problem:** `import_run_ops.rs` lines 113-135: when `ImportRunBeginOp` finds an existing ACTIVE run and returns it, it skips the `case_import_runs` INSERT. If a different case triggers a skeleton build that hits the same scope/source, that case never gets linked. When the run is later superseded, the cascade in `ImportRunSupersedeOp` won't find the unlinked case — silent data loss on the correction path.

**Fix:**

In the `if let Some((run_id,)) = existing` block, add the case linkage before returning:

```rust
if let Some((run_id,)) = existing {
    // Still link this case even though the run already exists
    if let Some(cid) = case_id {
        sqlx::query(
            r#"INSERT INTO kyc.case_import_runs (case_id, run_id, decision_id)
               VALUES ($1, $2, $3)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(cid)
        .bind(run_id)
        .bind(decision_id)
        .execute(pool)
        .await?;
    }

    let result = ImportRunBeginResult { ... };
    return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
}
```

The `ON CONFLICT DO NOTHING` handles the case where this case was already linked. No new edge cases introduced.

**Verify:** Add a unit test comment documenting the invariant: "Any case-id provided to import-run begin MUST appear in case_import_runs regardless of whether the run was newly created or already existed."

**Files:** `rust/src/domain_ops/import_run_ops.rs`

- [x] **F-7 complete** — Added case_import_runs INSERT with ON CONFLICT DO NOTHING in idempotent-hit path. 1017 tests pass.

---

### F-8: Import Run Begin — Accept and Persist `as_of` [AUDIT]

**Problem:** The spec requires import runs to record the as-of date for the graph snapshot. The handler doesn't accept `as_of` and doesn't persist it. Without it, you can't answer "what did the ownership graph look like on the date we ran this import?" — which is exactly what regulators ask.

**Fix:**

#### F-8a: Add `as_of` to the handler

In `ImportRunBeginOp::execute()`:

1. Extract: `let as_of = extract_string_opt(verb_call, "as-of");`
2. Add to the INSERT:

```rust
let run_id: Uuid = sqlx::query_scalar(
    r#"INSERT INTO "ob-poc".graph_import_runs
       (scope_root_entity_id, source, run_kind, source_ref, source_query, as_of)
       VALUES ($1, $2, $3, $4, $5, COALESCE($6::date, CURRENT_DATE))
       RETURNING run_id"#,
)
.bind(scope_root)
.bind(&source)
.bind(&run_kind)
.bind(&source_ref)
.bind(&source_query)
.bind(&as_of)
.fetch_one(pool)
.await?;
```

3. Add `as_of` to the `ImportRunBeginResult` struct
4. Default to CURRENT_DATE in the SQL if not provided — imports always have a date

**Check first:** Verify that `graph_import_runs` already has an `as_of` column. If not, you need a migration. Look at migration 077 or the table definition. If the column doesn't exist:

```sql
ALTER TABLE "ob-poc".graph_import_runs ADD COLUMN IF NOT EXISTS as_of DATE DEFAULT CURRENT_DATE;
```

Add this as a new migration file (078 or whatever the next sequence number is). Do NOT modify existing migrations.

- [x] **F-8a complete** — Added as_of extraction, INSERT column, COALESCE defaulting, result struct field. 1017 tests pass.

#### F-8b: Include `as_of` in idempotency check

Update the existing-run lookup to include `as_of`:

```rust
let existing: Option<(Uuid,)> = sqlx::query_as(
    r#"SELECT run_id FROM "ob-poc".graph_import_runs
       WHERE scope_root_entity_id = $1 AND source = $2
         AND COALESCE(source_ref, '') = COALESCE($3, '')
         AND status = 'ACTIVE' AND run_kind = $4
         AND as_of = COALESCE($5::date, CURRENT_DATE)
       LIMIT 1"#,
)
.bind(scope_root)
.bind(&source)
.bind(&source_ref)
.bind(&run_kind)
.bind(&as_of)
.fetch_optional(pool)
.await?;
```

This prevents collapsing two imports with different as-of dates into the same run. Same-day reimports still get idempotency.

- [x] **F-8b complete** — Added as_of to idempotency SELECT with COALESCE. 1017 tests pass.

#### F-8c: Add `as-of` to the YAML verb definition

If there's an import-run YAML file, add the arg. If import-run verbs are defined only in Rust (no YAML), skip this step and note it.

```yaml
          - name: as-of
            type: date
            required: false
            maps_to: as_of
            description: As-of date for the graph snapshot (defaults to today)
```

- [x] **F-8c complete** — Added as-of arg (type: date, required: false) to import-run.yaml begin verb. 1017 tests pass.

---

## Execution Order

```
F-7 (Import run case linkage — 5 lines, instant)
  → cargo test
F-8a (as_of column + handler arg — may need migration)
  → cargo test
F-8b (as_of in idempotency check)
  → cargo test
F-8c (YAML update if applicable)
  → cargo test
F-6a (Transition map — data only, no behavior change)
  → cargo test
F-6b (Plugin handler — the main work)
  → cargo test
F-6c (YAML swap — behavior: crud → plugin)
  → cargo test
F-6d (Unit tests for transitions)
  → cargo test
F-6e (Integration test comments)
  → cargo test
```

**Rationale:** F-7 is a 5-line fix. F-8 may need a migration so do it early before the bigger F-6 work. F-6 is subdivided into 5 checkpoints because it touches the most sensitive domain logic — if the session blows mid-F-6, you can resume at the right sub-step.

## Session Recovery

If this session crashes or you are a new Claude Code instance:

1. Read `.claude/KYC_FIXES_S2_PROGRESS.md`
2. Run `cargo test --all-targets` — record current state
3. Run `git diff --stat` — see what's been changed
4. Run `git log --oneline -5` — see recent commits
5. Continue from the next uncompleted task in this file
6. Do NOT restart completed tasks

## What's NOT In This Session

These items from the ChatGPT review are either already handled or deferred:

- **Skeleton build duplicates logic** — Fixed in Session 1 (F-3)
- **Per-source import-run boundaries in skeleton build** — Rejected. The spec's per-source runs apply to the research phase, not the derivation pipeline. Skeleton build is a single atomic analysis operation.
- **"Source adapters not called in skeleton build"** — Not a bug. Research phase runs before skeleton build. The reviewer conflated research (graph population) with derivation (graph analysis).
- **DSL-engine integration tests** — Deferred to Session 3. Requires test harness that exercises the full engine pipeline, which is a bigger lift than these targeted fixes.
- **Wiring checklist items (SQLx structs, projections, TS types, embeddings)** — Deferred. These are verification tasks, not code changes, and should run after all code fixes are committed.
- **Update v0.5 architecture doc** — Deferred until all implementation fixes are stable. Updating the spec while code is still changing creates drift in the other direction.

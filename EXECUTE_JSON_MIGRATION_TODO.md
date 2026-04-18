# execute_json Migration — Current Execution Plan for Codex

> **Branch:** `codex-pub-api-surface-cleanup`
> **Goal:** Migrate every `CustomOperation` impl in `rust/src/domain_ops/` from the legacy
> `VerbCall + ExecutionContext` contract to the SemOS `execute_json(args, VerbExecutionContext)` contract.
> **Current repo baseline (verified 2026-04-18):** `625 / 625` ops migrated in `rust/src/domain_ops/`.
> **Remaining:** `0` ops in `rust/src/domain_ops/`.
> **Outcome:** `rust/src/domain_ops/` is code-complete for the SemOS `execute_json(...)` port and
> passes `cargo check --features database -p ob-poc`.

> **Important correction to prior handoff:** `rust/src/domain_ops/entity_ops.rs` is already fully migrated (`6 / 6`).
> Do not start there. The prior note was stale.

---

## Architecture Context

- `CustomOperation` still exposes legacy `execute(...)` and default-shim `execute_json(...)`.
- Migration means:
  - override `execute_json(...)`
  - return `is_migrated() -> true`
  - keep legacy `execute(...)` working
  - share logic through a local `*_impl()` helper when extraction is mechanical
- Trait definition: `rust/src/domain_ops/mod.rs`
- JSON helpers: `rust/src/domain_ops/helpers.rs`
- Current reference implementations:
  - `rust/src/domain_ops/regulatory_ops.rs`
  - `rust/src/domain_ops/economic_exposure_ops.rs`
  - `rust/src/domain_ops/entity_ops.rs`

---

## Proven Migration Pattern

### 1. Keep extraction thin

Legacy `execute(...)` should only extract inputs from `VerbCall` / `ExecutionContext` and then call a shared helper.

### 2. Add a shared `*_impl()` where useful

If the operation reduces to resolved primitives plus SQL or deterministic logic, move the body into a local helper:

```rust
#[cfg(feature = "database")]
async fn my_op_impl(
    entity_id: uuid::Uuid,
    name: &str,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    // existing operation body
}
```

Both `execute(...)` and `execute_json(...)` call the same helper.

### 3. Mirror argument extraction in `execute_json(...)`

Use JSON helpers from `super::helpers`:

```rust
#[cfg(feature = "database")]
async fn execute_json(
    &self,
    args: &serde_json::Value,
    ctx: &mut sem_os_core::execution::VerbExecutionContext,
    pool: &PgPool,
) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
    let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
    let name = json_extract_string(args, "name")?;

    let id = my_op_impl(entity_id, &name, pool).await?;
    Ok(sem_os_core::execution::VerbExecutionOutcome::Uuid(id))
}

fn is_migrated(&self) -> bool {
    true
}
```

### 4. Use 1:1 result mapping

| Legacy `ExecutionResult` | New `VerbExecutionOutcome` |
|--------------------------|----------------------------|
| `Uuid(u)` | `VerbExecutionOutcome::Uuid(u)` |
| `Record(v)` | `VerbExecutionOutcome::Record(v)` |
| `RecordSet(rows)` | `VerbExecutionOutcome::RecordSet(rows)` |
| `Affected(n)` | `VerbExecutionOutcome::Affected(n)` |
| `Void` | `VerbExecutionOutcome::Void` |

### 5. Prefer existing helpers

- `json_extract_string`
- `json_extract_string_opt`
- `json_extract_uuid`
- `json_extract_uuid_opt`
- `json_get_required_uuid`
- `json_extract_bool`
- `json_extract_bool_opt`
- `json_extract_int`
- `json_extract_int_opt`
- `json_extract_string_list`
- `json_extract_string_list_opt`
- `json_extract_cbu_id`

Do not extend `helpers.rs` without review.

---

## Operating Rules

### Repo-level rules

- Run `cargo check` after every file edit. This is required by `AGENTS.md`.
- Use `cargo fmt` only for files you touched.
- Use `cargo clippy -- -D warnings` at batch boundaries.
- Keep diffs surgical. Do not reformat unrelated code.

### Migration rules

- Default to batches of `5-6` ops.
- Prefer finishing a small clean file in one batch.
- Do not mix unrelated dirty files into normal batches.
- Skip any op that would require trait redesign or helper-surface growth.
- Continue to skip the deferred heavy ops already called out by prior work, if encountered.

### Dirty-tree rules

- Treat `capital_ops.rs` and `custody.rs` as reconciliation slices, not general backlog.
- Do not revert or overwrite unrelated in-flight work in the tree.
- If a file is dirty and not part of the current slice, leave it alone.

---

## Execution Slices

### Slice 0 — Baseline Correction

Purpose: align instructions and progress tracking with the current tree before further migration.

- Update this file when counts materially change.
- Verify whether a progress tracker already exists elsewhere.
- If `memory/project_semos_execution_port_progress.md` does not exist, create it before relying on it.
- Do not touch `entity_ops.rs`; it is complete.

### Slice 1 — Single-op clean backlog, batch A

Files:
- `control_compute_ops.rs`
- `coverage_compute_ops.rs`
- `entity_query.rs`
- `graph_validate_ops.rs`
- `onboarding.rs`
- `outreach_plan_ops.rs`

Target: 6 ops

### Slice 2 — Single-op clean backlog, batch B

Files:
- `research_normalize_ops.rs`
- `skeleton_build_ops.rs`
- `tollgate_evaluate_ops.rs`
- remaining op in `sem_os_changeset_ops.rs`
- remaining op in `sem_os_governance_ops.rs`
- remaining op in `sem_os_registry_ops.rs`

Target: 6 ops

### Slice 3 — Tiny clean files

Work in `3-4` batches:

- `template_ops.rs`
- `import_run_ops.rs`
- `matrix_overlay_ops.rs`
- `trading_matrix.rs`
- `trust_ops.rs`
- `ubo_analysis.rs`
- `ubo_compute_ops.rs`

Target: finish each file whole where possible.

### Slice 4 — Medium clean files

Prefer one file per batch unless the file is trivial:

- `partnership_ops.rs`
- `refdata_ops.rs`
- `remediation_ops.rs`
- `research_workflow_ops.rs`
- `tollgate_ops.rs`
- `ubo_graph_ops.rs`
- `bpmn_lite_ops.rs`
- `refdata_loader.rs`
- `ubo_registry_ops.rs`

Notes:
- `screening_ops.rs` and `kyc_case_ops.rs` are dirty in the worktree but already fully migrated. Do not use them for new migration work.

### Slice 5 — Six-to-eight op clean files

- `affinity_ops.rs`
- `bods_ops.rs`
- `investor_role_ops.rs`
- `resource_ops.rs`
- `semantic_ops.rs`
- `verify_ops.rs`
- `batch_control_ops.rs`
- `cbu_role_ops.rs`
- `sem_os_maintenance_ops.rs`
- `access_review_ops.rs`
- `cbu_ops.rs`
- `dilution_ops.rs`
- `ownership_ops.rs`
- `sem_os_schema_ops.rs`
- `shared_atom_ops.rs`
- `state_ops.rs`
- `temporal_ops.rs`

### Slice 6 — Nine-to-thirteen op clean files

Split each file across multiple batches as needed:

- `document_ops.rs`
- `phrase_ops.rs`
- `discovery_ops.rs`
- `evidence_ops.rs`
- `control_ops.rs`
- `manco_ops.rs`
- `request_ops.rs`
- `trading_profile_ca_ops.rs`
- `lifecycle_ops.rs`
- `investor_ops.rs`

### Slice 7 — Large clean files

Run these as file-local campaigns. Do not mix them together in the same commit.

- `source_loader_ops.rs`
- `attribute_ops.rs`
- `service_pipeline_ops.rs`
- `gleif_ops.rs`
- `client_group_ops.rs`
- `deal_ops.rs`
- `booking_principal_ops.rs`
- `trading_profile.rs`

### Slice 8 — Dirty-file reconciliation

Handle only after clean-file momentum is established, or immediately if the human wants the dirty work reconciled first.

- `capital_ops.rs`
- `custody.rs`

Workflow:
- inspect existing uncommitted edits first
- preserve user/other-agent changes
- migrate only the remaining ops
- verify carefully because these files already have partial migration state

### Slice 9 — Final closeout

- recompute migration counts for `rust/src/domain_ops`
- run the DB-backed validation gate
- update the progress tracker
- update the SemOS Execution Port status line in `CLAUDE.md`

---

## Per-Batch Workflow

For each batch:

1. Edit the selected files using the proven pattern.
2. After each file edit, run:
   ```bash
   cd rust && cargo check --features database -p ob-poc
   ```
3. If SQL text changed and `sqlx` offline metadata needs refresh, run:
   ```bash
   cd rust && DATABASE_URL="postgresql:///data_designer" cargo sqlx prepare --workspace
   ```
4. Re-run:
   ```bash
   cd rust && cargo check --features database -p ob-poc
   ```
5. Run lint for the batch:
   ```bash
   cd rust && cargo clippy --features database -p ob-poc --lib -- -D warnings
   ```
6. Sanity-check migration counts for touched files:
   ```bash
   cd rust/src/domain_ops && rg -c '^impl CustomOperation for ' <files>
   cd rust/src/domain_ops && rg -c '^[ ]{4}fn is_migrated\\(&self\\) -> bool \\{$' <files>
   ```
7. Commit the migrated ops plus any required `rust/.sqlx/` changes and tracker updates.

---

## Validation Gate

After every five batches, after changing tiers, or before declaring the stream done:

```bash
cd rust && DATABASE_URL="postgresql:///data_designer" cargo test --features database -p ob-poc --lib
```

Baseline assumption:
- pre-existing known DB timeout failures may remain acceptable if already documented in the progress tracker
- any new failure is a regression until proven otherwise

---

## Progress Tracking

After each batch commit:

- update the running total for `rust/src/domain_ops`
- append a one-line batch entry with files and ops migrated
- note whether `cargo check`, `cargo clippy`, and `cargo test` passed or were skipped

Preferred tracker:
- `memory/project_semos_execution_port_progress.md` if present

If missing:
- create that file before further relying on tracker instructions

Final doc update:
- update the SemOS Execution Port bullet in `CLAUDE.md` once the domain-op migration stream is complete

---

## Stop Conditions

Pause and ask the human if:

- migration would require changing the `CustomOperation` trait surface
- a file needs new helpers in `helpers.rs`
- `VerbExecutionContext` lacks something required for more than one or two ops
- an op is not mechanically portable without redesign
- existing dirty changes in `capital_ops.rs` or `custody.rs` conflict with the migration

Otherwise, keep batching and preserve behavior.

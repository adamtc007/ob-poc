# Phase 2.5 Dispatch Call-Site Audit

**Date:** 2026-04-19  
**Branch:** codex-pub-api-surface-cleanup  
**Scope:** `rust/src` (excluding `rust/target` and `bpmn-lite`)  
**Goal:** Identify every production code path that dispatches a verb/op, enabling migration to `dsl_runtime::VerbExecutionPort::execute_verb` as the single execution capability.

---

## Summary

- **Total dispatch call sites found:** 47
- **ALREADY-PORT:** 4 (comply with SemOS contract)
- **MIGRATE-TO-PORT:** 37 (legacy DslExecutor paths requiring refactor)
- **DELETE:** 2 (test/dev code)
- **BYPASS:** 3 (sync-back thunks, direct op.execute)
- **INTERNAL:** 1 (internal to DslExecutor, can become pub(crate))

**Estimated scope for Phase 2.5 Slice 0:** ~40 call sites to migrate across 5 priority tiers.

---

## Section 1 — Dispatch Call Sites

### Tier 1: Entry Points (Agent / API Routes)

**High priority: these are the API boundaries that drive most prod traffic.**

| File | Line | Code Snippet | Caller Category | Disposition |
|------|------|--------------|-----------------|-------------|
| `rust/src/api/agent_routes.rs` | 2095 | `.execute_plan_atomic_with_locks(&plan, &mut exec_ctx, expansion_report.as_ref())` | agent | MIGRATE-TO-PORT |
| `rust/src/api/agent_routes.rs` | 2103 | `.execute_plan_best_effort(&plan, &mut exec_ctx)` | agent | MIGRATE-TO-PORT |
| `rust/src/api/agent_service.rs` | 1155 | `.execute_dsl(&resolved_dsl, &mut exec_ctx)` | agent | MIGRATE-TO-PORT |
| `rust/src/mcp/handlers/core.rs` | 1015 | `.execute_plan_atomic_with_locks(&plan, &mut ctx, expansion_report.as_ref())` | mcp | MIGRATE-TO-PORT |
| `rust/src/mcp/handlers/core.rs` | 1022 | `.execute_plan_best_effort(&plan, &mut ctx)` | mcp | MIGRATE-TO-PORT |
| `rust/src/mcp/handlers/core.rs` | 1391 | `.execute_submission(&submission, &mut domain_ctx, &limits)` | mcp | MIGRATE-TO-PORT |

**Notes:**
- `agent_routes.rs:2095,2103` are the main Chat API dispatch points; they already propagate pending_* results to SessionContext.
- `agent_service.rs:1155` is behind a `#[cfg(not(feature = "runbook-gate-vnext"))]` gate — legacy fallback when runbook isn't used.
- `mcp/handlers/core.rs` has three major entry points for dsl_execute_submission and intent_pipeline triggers.

---

### Tier 2: Internal Plan Execution (DslExecutor family methods)

**Medium priority: internal to DslExecutor but called from multiple upstream sites. Most should become pub(crate) after phase refactor.**

| File | Line | Code Snippet | Caller Category | Disposition |
|------|------|--------------|-----------------|-------------|
| `rust/src/dsl_v2/executor.rs` | 1860 | `self.execute_verb(&vc, ctx).await?` (in execute_plan) | INTERNAL | INTERNAL |
| `rust/src/dsl_v2/executor.rs` | 2318 | `self.execute_verb(&vc, ctx).await` (in execute_plan_best_effort) | INTERNAL | INTERNAL |
| `rust/src/dsl_v2/executor.rs` | 2564 | `self.execute_verb(&vc, ctx).await?` (in execute_dsl helper: build_from_dict_attrs) | INTERNAL | INTERNAL |
| `rust/src/dsl_v2/executor.rs` | 2591, 2604, 2634, 2668, 2678, 2692, 2702, 2726, 2754, 2775, 2802, 2815, 2840, 2876 | `self.execute_verb(&vc, ctx).await` (14 calls in template/batch helpers) | INTERNAL | INTERNAL |

**Notes:**
- These are recursive calls within `DslExecutor::execute_verb → execute_plan → [per-step] → execute_verb`.
- They are **implementation details** and should become `pub(crate)` once moved to a dedicated internal submodule.
- No caller outside executor.rs needs to invoke execute_verb directly; they use execute_plan/execute_submission/execute_dsl.

---

### Tier 3: SemOS VerbExecutionPort (Already Compliant)

**Low priority: already using the target contract. Validate they remain stable.**

| File | Line | Code Snippet | Caller Category | Disposition |
|------|------|--------------|-----------------|-------------|
| `rust/src/runbook/step_executor_bridge.rs` | 156 | `self.port.execute_verb(&step.verb, args, &mut ctx)` | runbook | ALREADY-PORT |
| `rust/src/sem_os_runtime/verb_executor_adapter.rs` | 136 | `self.executor.execute_verb(&vc, &mut exec_ctx)` | semos | MIGRATE-TO-PORT |

**Notes:**
- `step_executor_bridge.rs:156` is the canonical compliant path: `VerbExecutionPortStepExecutor` bridges runbook steps → VerbExecutionPort contract.
- `verb_executor_adapter.rs:136` is the **adapter itself** calling into DslExecutor's legacy execute_verb. This is an INTERNAL call and will remain, but the return path converts to VerbExecutionOutcome.

---

### Tier 4: Domain Ops (Sync-Back Thunks)

**Medium priority: 14 ops using execute_json_via_legacy to thunk legacy execute() → SemOS execute_json contract.**

| Files | Pattern | Disposition |
|-------|---------|-------------|
| `rust/src/domain_ops/agent_ops.rs` (lines 39–50) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/attribute_ops.rs` (lines 44–55) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/booking_principal_ops.rs` (lines 46–57) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/control_ops.rs` (lines 35–46) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/gleif_ops.rs` (lines 43–54) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/lifecycle_ops.rs` (lines 30–41) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/manco_ops.rs` (lines 40–51) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/request_ops.rs` (lines 34–45) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/sem_os_schema_ops.rs` (lines 35–46) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/session_ops.rs` (lines 56–67) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/source_loader_ops.rs` (lines 47–58) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/trading_profile.rs` (lines 50–61) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/trading_profile_ca_ops.rs` (lines 37–48) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |
| `rust/src/domain_ops/view_ops.rs` (lines 48–59) | `execute_json_via_legacy(self, args, ctx, pool).await` | BYPASS |

**Notes:**
- These 14 ops have **both `execute()` (legacy) and `execute_json()` (SemOS)** methods.
- The `execute_json_via_legacy` helper:
  1. Converts JSON args → `VerbCall`
  2. Converts `VerbExecutionContext` → `ExecutionContext` (legacy)
  3. Calls `op.execute(vc, ctx, pool)` 
  4. Converts result back to `VerbExecutionOutcome`
- They are **marked as migrated (`is_migrated() = true`)** but still use the thunk. **This is not a true native migration** — they don't implement custom execute_json logic.
- **Action:** These 14 ops need genuine execute_json implementations (not thunks) as part of Phase 2.5.

---

### Tier 5: Direct op.execute() Calls (Bypass Paths)

**Low priority: internal, non-production paths that should be deleted or refactored.**

| File | Line | Code Snippet | Context | Disposition |
|------|------|--------------|---------|-------------|
| `rust/src/domain_ops/request_ops.rs` | 33 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in create_request_thunk | BYPASS |
| `rust/src/domain_ops/sem_os_schema_ops.rs` | 34 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in create_schema_thunk | BYPASS |
| `rust/src/domain_ops/session_ops.rs` | 55 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in create_session_thunk | BYPASS |
| `rust/src/domain_ops/manco_ops.rs` | 39 | `op.execute(&verb_call, &mut legacy_ctx, pool)` | thunk using legacy ExecutionContext | BYPASS |
| `rust/src/domain_ops/attribute_ops.rs` | 43 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in attribute_thunk | BYPASS |
| `rust/src/domain_ops/source_loader_ops.rs` | 46 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in load_source_thunk | BYPASS |
| `rust/src/domain_ops/view_ops.rs` | 47 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in create_view_thunk | BYPASS |
| `rust/src/domain_ops/control_ops.rs` | 34 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in control_thunk | BYPASS |
| `rust/src/domain_ops/control_ops.rs` | 1801, 2060 | `show_op.execute(verb_call, ctx, pool)` | nested execute calls in control_show | BYPASS |
| `rust/src/domain_ops/lifecycle_ops.rs` | 29 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in lifecycle_thunk | BYPASS |
| `rust/src/domain_ops/trading_profile.rs` | 49 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in trading_profile_thunk | BYPASS |
| `rust/src/domain_ops/gleif_ops.rs` | 42 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in gleif_thunk | BYPASS |
| `rust/src/domain_ops/booking_principal_ops.rs` | 45 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in booking_principal_thunk | BYPASS |
| `rust/src/domain_ops/trading_profile_ca_ops.rs` | 36 | `op.execute(&vc, &mut exec_ctx, pool)` | sync-back op.execute in trading_profile_ca_thunk | BYPASS |
| `rust/src/bin/bulk_attribute_reconcile.rs` | 52 | `op.execute(&verb_call, &mut ctx, &pool)` | CLI tool (test/dev only) | DELETE |

**Notes:**
- These are all **sync-back thunks** that call `op.execute()` directly instead of routing through the standard dispatch chain.
- They exist because some ops have both execute() (legacy) and execute_json() methods, and certain callers need to invoke the legacy one.
- `bulk_attribute_reconcile.rs` is a CLI tool and should be treated as dev-only code.
- **Action:** Eliminate these thunks by ensuring all callers go through execute_json() or remove the legacy execute() methods entirely.

---

### Tier 6: Raw DSL Execution (execute_dsl)

**Medium priority: high-level orchestrator paths that currently bypass structured runbooks.**

| File | Line | Code Snippet | Caller Category | Disposition |
|------|------|--------------|-----------------|-------------|
| `rust/src/api/agent_service.rs` | 1155 | `executor.execute_dsl(&resolved_dsl, &mut exec_ctx)` | agent | MIGRATE-TO-PORT |
| `rust/src/dsl_v2/sheet_executor.rs` | 571 | `executor.execute_dsl(source, &mut ctx)` | api | MIGRATE-TO-PORT |
| `rust/src/templates/harness.rs` | 473 | `execute_dsl(&expansion.dsl, pool)` (local fn wrapper) | api | MIGRATE-TO-PORT |
| `rust/src/templates/harness.rs` | 527 | `async fn execute_dsl(dsl: &str, pool: &sqlx::PgPool)` | api | MIGRATE-TO-PORT |
| `rust/src/domain_ops/gleif_ops.rs` | 122, 1013, 1035, 1056 | `executor.execute_dsl(&dsl, &mut dsl_ctx)` | domain_ops | MIGRATE-TO-PORT |
| `rust/src/domain_ops/onboarding.rs` | 180 | `executor.execute_dsl(&dsl, ctx)` | domain_ops | MIGRATE-TO-PORT |
| `rust/src/gleif/repository.rs` | 596 | `executor.execute_dsl(&dsl, &mut dsl_ctx)` | api | MIGRATE-TO-PORT |
| `rust/src/research/agent_controller.rs` | 466, 810 | `executor.execute_dsl(dsl, &mut ctx)` | api | MIGRATE-TO-PORT |
| `rust/src/bin/dsl_cli.rs` | 1155, 1556, 2407 | `executor.execute_dsl(&resolved_dsl, &mut exec_ctx)` OR `execute_plan` | test | DELETE |
| `rust/src/bin/batch_test_harness.rs` | 716 | `executor.execute_plan(&plan, &mut ctx)` | test | DELETE |
| `rust/src/bin/dsl_api.rs` | 289 | `executor.execute_plan(&plan, &mut ctx)` | test | DELETE |
| `rust/src/repl/executor_bridge.rs` | 66 | `executor.execute_plan(&plan, &mut ctx)` | repl | MIGRATE-TO-PORT |
| `rust/src/dsl_v2/batch_executor.rs` | 398 | `executor.execute_plan(&plan, &mut child_ctx)` | api | MIGRATE-TO-PORT |

**Notes:**
- `execute_dsl` is the "string → parse → execute" convenience path; it's still calling `execute_plan` internally.
- All `execute_dsl` callers should eventually be gated behind `#[cfg(not(feature = "runbook-gate-vnext"))]` when the runbook refactor completes.
- The CLI and test tools (`dsl_cli.rs`, `batch_test_harness.rs`, `dsl_api.rs`) can be tagged DELETE since they're non-production.

---

### Tier 7: Template and Batch Operations (Internal Composition)

**Low priority: these are nested verb calls within domain ops, not external entry points.**

| File | Line | Code Snippet | Caller Category | Disposition |
|------|------|--------------|-----------------|-------------|
| `rust/src/domain_ops/template_ops.rs` | 109 | `executor.execute_plan(&plan, ctx)` (in template.invoke) | domain_ops | MIGRATE-TO-PORT |
| `rust/src/domain_ops/template_ops.rs` | 463 | `executor.execute_batch(...)` (in template.batch) | domain_ops | MIGRATE-TO-PORT |
| `rust/src/dsl_v2/batch_executor.rs` | 398 | `executor.execute_plan(&plan, &mut child_ctx)` (in batch iteration) | dsl_v2 | INTERNAL |

**Notes:**
- `execute_batch` is a convenience wrapper around `execute_plan` for templated batch iteration.
- These are implementation details within domain ops; they don't need to be refactored immediately as long as the outer op.execute_json() complies with the contract.

---

## Section 2 — DslExecutor Public API

**File:** `rust/src/dsl_v2/executor.rs`

### Public Functions on DslExecutor

| Signature | Line | Visibility | Required? | Notes |
|-----------|------|-----------|-----------|-------|
| `pub fn new(pool: PgPool) -> Self` | 1172 | PUB | YES | Factory; always needed |
| `pub async fn execute_verb(&self, vc: &VerbCall, ctx: &mut ExecutionContext) -> Result<ExecutionResult>` | 1244 | PUB | REFACTOR | This is the main internal dispatch. Should become `pub(crate)` after runbook phase. Currently used by adapter and internal recursion. |
| `pub async fn execute_submission(&self, submission: &DslSubmission, ...) -> Result<ExecutionResult>` | 1566 | PUB | MAYBE | Mid-level API. Used by MCP. Could stay public as a convenience wrapper. |
| `pub async fn execute_plan(&self, plan: &ExecutionPlan, ctx: &mut ExecutionContext) -> Result<Vec<ExecutionResult>>` | 1786 | PUB | REFACTOR | Heavily used by all tiers. Should stay public but only called through VerbExecutionPort after migration. |
| `pub async fn execute_plan_atomic(&self, plan: &ExecutionPlan, ctx: &mut ExecutionContext) -> Result<AtomicExecutionResult>` | 1947 | PUB | REFACTOR | Atomic variant; moderate use. Keep for now. |
| `pub async fn execute_plan_atomic_with_locks(&self, plan: &ExecutionPlan, ctx: &mut ExecutionContext, ...) -> Result<AtomicExecutionResultWithLocks>` | 2086 | PUB | REFACTOR | Locking variant used by agent routes and MCP. Keep public. |
| `pub async fn execute_plan_best_effort(&self, plan: &ExecutionPlan, ctx: &mut ExecutionContext) -> Result<BestEffortExecutionResult>` | 2262 | PUB | REFACTOR | Best-effort variant; heavily used. Keep public. |
| `pub async fn execute_dsl(&self, source: &str, ctx: &mut ExecutionContext) -> Result<HashMap<String, String>>` | 2401 | PUB | REFACTOR | High-level entry point. Gate behind `#[cfg(not(feature = "runbook-gate-vnext"))]` once runbook refactor completes. |
| `pub async fn execute_with_dag(&self, plan: &ExecutionPlan, ...) -> Result<...>` | 2473 | PUB | NO | Currently unused. DELETE. |
| `pub fn verify_registry_coverage(&self) -> std::result::Result<(), Vec<String>>` | 1203 | PUB | MAYBE | Validation utility; can stay public. |
| `pub fn with_events(mut self, events: Option<SharedEmitter>) -> Self` | 1220 | PUB | YES | Builder; keep for observability. |
| `pub fn events(&self) -> Option<&SharedEmitter>` | 1226 | PUB | NO | Accessor; low priority. |
| `pub fn pool(&self) -> &PgPool` | 1232 | PUB | NO | Accessor; low priority. |

### Recommendation

**After Phase 2.5:**
1. `execute_verb` → `pub(crate)` (internal recursion only)
2. `execute_dsl` → gate behind `#[cfg(not(feature = "runbook-gate-vnext"))]` or DELETE
3. Keep `execute_plan*` variants public but **only call through VerbExecutionPort contract** (the adapter will enforce this)
4. DELETE `execute_with_dag` (unused)
5. Keep `verify_registry_coverage`, `with_events`, accessors as they are (low risk)

---

## Section 3 — pending_* Field Readers

**File:** `rust/src/dsl_v2/executor.rs`

ExecutionContext has 15 pending_* fields used to communicate side effects back to the session layer. These must be migrated to `VerbExecutionContext.extensions` after Phase 2.5.

### Readers (not writers) by Category

| Field | Readers | Caller Category | File | Line |
|-------|---------|-----------------|------|------|
| `pending_view_state` | `take_pending_view_state()` | mcp | `mcp/handlers/core.rs` | 1153 |
| `pending_view_state` | `take_pending_view_state()` | agent | `api/agent_routes.rs` | 2345 |
| `pending_view_state` | `take_pending_view_state()` | semos | `sem_os_runtime/verb_executor_adapter.rs` | 595 (is_some check) |
| `pending_viewport_state` | `take_pending_viewport_state()` | mcp | `mcp/handlers/core.rs` | 1156 |
| `pending_viewport_state` | `take_pending_viewport_state()` | agent | `api/agent_routes.rs` | 2360 |
| `pending_viewport_state` | `take_pending_viewport_state()` | semos | `sem_os_runtime/verb_executor_adapter.rs` | 601 (is_some check) |
| `pending_scope_change` | `take_pending_scope_change()` | mcp | `mcp/handlers/core.rs` | 1159 |
| `pending_scope_change` | `take_pending_scope_change()` | agent | `api/agent_routes.rs` | 2375 |
| `pending_scope_change` | `take_pending_scope_change()` | semos | `sem_os_runtime/verb_executor_adapter.rs` | 601 (is_some check) |
| `pending_session` | `take_pending_session()` | agent | `api/agent_service.rs` | 1600 |
| `pending_session` | `take_pending_session()` | agent | `api/agent_routes.rs` | 2389 |
| `pending_session` | `take_pending_session()` | semos | `sem_os_runtime/verb_executor_adapter.rs` | 607 (is_some check) |
| (misc) | `take_pending_agent_control()` | agent | `dsl_v2/executor.rs` | 1081 |

**Other pending_* fields:** pending_session_name, pending_agent_*, pending_checkpoint_response, pending_threshold_change, pending_mode_change, pending_cbu_scope, etc. are set but not read in current production code paths. They are **defined** in ExecutionContext but consumed only within the session layer or agent mode state machine.

### Action Items

1. **Audit all read sites** (7 unique read locations identified above) to map the semantics:
   - Does `pending_view_state` map to a specific `VerbExecutionContext.extensions` key?
   - Can we use a standard envelope like `{"platform_state": {"view_state": {...}}}` or does each field need a unique serialization?

2. **Adapter side-channel mapping** (in `verb_executor_adapter.rs:collect_side_effects`):
   - Currently collects pending_* into `VerbSideEffects.platform_state`.
   - Need to ensure all 15 fields map into a schema-validated extension key.

3. **VerbExecutionContext contract** in `dsl_runtime` crate:
   - Extend `VerbExecutionContext.extensions` to handle platform-specific state.
   - Define a stable schema for `pending_*` fields so SemOS can parse and act on them.

---

## Section 4 — ExecutionResult Typed Variant Readers

**File:** `rust/src/dsl_v2/executor.rs:197–216`

ExecutionResult has four domain-specific variants (beyond Uuid/Record/RecordSet/Affected/Void):
- `EntityQuery(EntityQueryResult)`
- `TemplateInvoked(TemplateInvokeResult)`
- `TemplateBatch(TemplateBatchResult)`
- `BatchControl(BatchControlResult)`

### Readers by Location

| Variant | Readers | File | Line | Caller Category |
|---------|---------|------|------|-----------------|
| `EntityQuery` | `if let ExecutionResult::EntityQuery(r) =>` | `dsl_v2/idempotency.rs` | 295, 478 | INTERNAL |
| `EntityQuery` | `match ... ExecutionResult::EntityQuery(eq) =>` | `bin/dsl_cli.rs` | 1197, 1574 | test |
| `TemplateInvoked` | `if let ExecutionResult::TemplateInvoked(r) =>` | `dsl_v2/idempotency.rs` | 306, 488 | INTERNAL |
| `TemplateInvoked` | `match ... ExecutionResult::TemplateInvoked(ti) =>` | `bin/dsl_cli.rs` | 1198, 1575 | test |
| `TemplateBatch` | `if let ExecutionResult::TemplateBatch(r) =>` | `dsl_v2/idempotency.rs` | 323, 504 | INTERNAL |
| `TemplateBatch` | `match ... ExecutionResult::TemplateBatch(tb) =>` | `bin/dsl_cli.rs` | 1199, 1576 | test |
| `BatchControl` | `if let ExecutionResult::BatchControl(r) =>` | `dsl_v2/idempotency.rs` | 341, 521 | INTERNAL |
| `BatchControl` | `match ... ExecutionResult::BatchControl(_) =>` | `bin/dsl_cli.rs` | 1200, 1577 | test |
| All 4 | Adapter converts to `VerbExecutionOutcome::Record({"_debug": format!("{r:?}")})` | `sem_os_runtime/verb_executor_adapter.rs` | 641–650 | adapter |

### Lossy Conversion Concern

The adapter at `verb_executor_adapter.rs:641–650` **lossy-converts** these typed results into `VerbExecutionOutcome::Record`:

```rust
ExecutionResult::EntityQuery(r) => VerbExecutionOutcome::Record(
    serde_json::json!({"_debug": format!("{r:?}")}),
),
```

**Critical question:** Is this debug-only JSON consumed anywhere in SemOS or the UI, or is it just a fallback for observability?

- If **only for debugging:** Continue the lossy path (acceptable).
- If **consumed by LLM or UI logic:** We need to preserve the full typed result in VerbExecutionOutcome (adds complexity).

### Readers of the Lossy Output

**Currently:** Only `idempotency.rs` and CLI tools read the typed variants directly. The adapter's Record output is not read back in production.

**Recommendation:** The lossy conversion is acceptable for Phase 2.5 if these four variants are internal implementation details that SemOS doesn't need to understand. If future phases require preserving them, extend `VerbExecutionOutcome` with a new variant (e.g., `VerbExecutionOutcome::Internal(serde_json::Value)`) to hold domain-specific results.

---

## Section 5 — Thunk vs Native execute_json

**File:** `rust/src/domain_ops/*.rs`

### Summary

- **Total files with execute_json method:** 87 of 91 domain_ops files
- **Files using execute_json_via_legacy (thunks):** 14
- **Files with native execute_json implementations:** 73

### Thunking Files (14)

These ops have `execute_json` methods that call `execute_json_via_legacy`:

1. agent_ops.rs
2. attribute_ops.rs
3. booking_principal_ops.rs
4. control_ops.rs
5. gleif_ops.rs
6. lifecycle_ops.rs
7. manco_ops.rs
8. request_ops.rs
9. sem_os_schema_ops.rs
10. session_ops.rs
11. source_loader_ops.rs
12. trading_profile.rs
13. trading_profile_ca_ops.rs
14. view_ops.rs

**Pattern:** All 14 files define a local `execute_json_via_legacy` helper that:
1. Converts `VerbExecutionContext` → `ExecutionContext`
2. Converts JSON → `VerbCall`
3. Calls `op.execute(vc, ctx, pool)`
4. Converts result back to `VerbExecutionOutcome`
5. Marks `is_migrated() = true` (misleading — not truly migrated)

### Native Files (73)

The remaining 73 files have genuine `execute_json` implementations that:
- Take `&serde_json::Value` args directly
- Operate on SemOS types (`VerbExecutionContext`)
- Return `VerbExecutionOutcome` directly
- Do not call the legacy `execute()` method

### Edge Cases

**Files with no execute_json:**
- `mod.rs` (trait definition only)
- `bpmn_lite_ops.rs` (separate service boundary)

**Files with both execute and execute_json but NO thunk:**
- These should be inspected to confirm they have genuine native implementations.

---

## Section 6 — Call Chain Analysis

### Happy Path (Current)

```
┌─────────────────────────────────────────────────────────┐
│ Chat API (agent_routes.rs:2095 or agent_service.rs:1155)│
└───────────────────┬─────────────────────────────────────┘
                    │ execute_plan_atomic_with_locks OR execute_dsl
                    ▼
        ┌──────────────────────────┐
        │ DslExecutor::execute_plan│
        └──────────┬───────────────┘
                   │ for each step
                   ▼
    ┌──────────────────────────────┐
    │ DslExecutor::execute_verb    │
    │ (with VerbCall)              │
    └──────────┬───────────────────┘
               │ routes to:
               ├─→ GenericCrudExecutor (YAML-defined verbs)
               └─→ CustomOperation.execute_json (plugins)
                        │
                        ├─→ (14 thunks) execute_json_via_legacy
                        │   → CustomOperation.execute (legacy)
                        │   → convert back
                        │
                        └─→ (73 native) direct execute_json impl
                                → returns VerbExecutionOutcome
                                → converted to ExecutionResult
                                
    ┌──────────────────────────────────────────────────────┐
    │ pending_* side effects extracted from ExecutionContext│
    │ (view_state, session, scope_change, agent_control)   │
    └───────────────┬──────────────────────────────────────┘
                    │
                    ▼
        ┌──────────────────────────┐
        │ Agent Routes / MCP       │
        │ consume pending_* and    │
        │ propagate to SessionCtx  │
        └──────────────────────────┘
```

### Target Path (Phase 2.5)

```
┌──────────────────────────────────────────────────────────┐
│ Chat API / MCP / REPL (via VerbExecutionPort contract)   │
└────────────────────┬─────────────────────────────────────┘
                     │ execute_verb(verb_fqn, args, ctx)
                     ▼
        ┌────────────────────────────────────┐
        │ ObPocVerbExecutor (adapter)         │
        │ (impl VerbExecutionPort)            │
        └────────────┬───────────────────────┘
                     │ routes to:
                     ├─→ CrudExecutionPort (SemOS-native CRUD)
                     │
                     └─→ DslExecutor::execute_verb (plugins + fallback)
                            │
                            ├─→ CustomOperation.execute_json (native)
                            │   → returns VerbExecutionOutcome
                            │
                            └─→ GenericCrudExecutor (legacy CRUD fallback)
                                → returns ExecutionResult
                                → converted to VerbExecutionOutcome
                                
    ┌──────────────────────────────────────────────────────┐
    │ VerbSideEffects extracted via collect_side_effects() │
    │ (new_bindings, platform_state with pending_*)        │
    └───────────────┬──────────────────────────────────────┘
                    │
                    ▼
        ┌──────────────────────────────────┐
        │ VerbExecutionContext.extensions   │
        │ carries platform_state back to    │
        │ caller (REPL / runbook / LLM)     │
        └──────────────────────────────────┘
```

---

## Section 7 — Outstanding Questions

### Q1: Sync-Back Thunks — Should They Be Eliminated?

**Finding:** 14 ops use `execute_json_via_legacy` helper, and 14+ files have direct `op.execute()` calls in thunk functions.

**Decision needed:** 
- **Option A (Recommended):** Implement true native `execute_json()` for all 14 thunking ops during Phase 2.5, delete the thunks, and eliminate direct `op.execute()` calls entirely.
- **Option B:** Keep thunks as a compatibility shim, but isolate them to a `pub(crate)` helper.

**Rationale for A:** Thunks are leaky abstractions; they hide which ops are truly migrated vs. still using legacy paths. Native migrations are cleaner and enable full type safety during SemOS execution.

### Q2: ExecutionResult Domain Variants — Should They Be Preserved?

**Finding:** EntityQuery, TemplateInvoked, TemplateBatch, BatchControl are typed variants that the adapter lossy-converts to `VerbExecutionOutcome::Record` with a `_debug` field.

**Decision needed:**
- **Option A (Current):** Keep lossy conversion; these are internal to ob-poc and not part of the SemOS contract.
- **Option B:** Extend `VerbExecutionOutcome` with a new variant (e.g., `VerbExecutionOutcome::Domain(String, serde_json::Value)`) to preserve these.

**Rationale for A:** If SemOS doesn't need to understand entity query results or batch status separately, the lossy Record output is acceptable for observability. LLMs will work with the Record JSON.

### Q3: DslExecutor::execute_dsl — Should It Be Sunset?

**Finding:** 13 production call sites use `execute_dsl(string)` instead of structured `execute_plan()`. This is behind a `#[cfg(not(feature = "runbook-gate-vnext"))]` gate in agent_service.rs but not in other files.

**Decision needed:**
- Should all `execute_dsl` calls be gated and scheduled for deletion once runbook refactor completes?
- Or should execute_dsl remain as a convenience public API?

**Rationale:** If `runbook-gate-vnext` becomes the default soon, execute_dsl can be deprecated. If it remains optional for compatibility, keep it public.

### Q4: GenericCrudExecutor — Should It Be Deprecated?

**Finding:** The adapter has a `with_crud_port()` option to route CRUD verbs through a SemOS-native CrudExecutionPort instead of GenericCrudExecutor.

**Decision needed:**
- What's the timeline for deprecating GenericCrudExecutor?
- Should Phase 2.5 require all CRUD verbs to have CrudExecutionPort implementations?

**Rationale:** SemOS contract is cleaner; GenericCrudExecutor is legacy. Full deprecation requires CrudExecutionPort coverage for all CRUD verbs (~50+ verbs defined in verbs.yaml).

### Q5: Pending_* Fields — Schema for Extensions?

**Finding:** 15 pending_* fields in ExecutionContext are mapped to `VerbSideEffects.platform_state` but no strict schema is defined.

**Decision needed:**
- Define a canonical envelope for pending_* state in `VerbExecutionContext.extensions`.
- Should it be:
  - A flat map: `{"pending_view_state": {...}, "pending_session": {...}}`
  - A nested envelope: `{"platform_state": {"pending": {...}}}`
  - A versioned schema to support future changes?

**Rationale:** SemOS callers need to parse and act on these fields; without a schema, parsing is fragile.

---

## Section 8 — Migration Roadmap

### Phase 2.5 Slice 0 (This Audit)

**Deliverable:** This document — a ledger of all dispatch call sites.

**Actions:**
1. Validate all 47 call sites are accounted for ✓
2. Categorize by disposition (ALREADY-PORT, MIGRATE-TO-PORT, etc.) ✓
3. Identify high-risk areas (thunks, lossy conversions) ✓
4. Ask outstanding questions (Section 7) → prioritize answers

### Recommended Slice 1 (Thunk Elimination)

**Scope:** Convert the 14 thunking ops to true native execute_json implementations.

**Effort:** ~1–2 weeks (mostly copy-paste and testing)

**Outcome:** 
- All 87 domain_ops have genuine execute_json, no thunks.
- 14+ direct `op.execute()` calls deleted.
- `is_migrated()` becomes a reliable signal.

### Recommended Slice 2 (Entry Point Migration)

**Scope:** Migrate the 6 Tier 1 entry points (agent_routes, agent_service, mcp/handlers) to route through VerbExecutionPort.

**Effort:** ~2–3 weeks (refactoring orchestrator interfaces)

**Outcome:**
- All Chat API, MCP, and REPL execution goes through `VerbExecutionPort::execute_verb`.
- Agent_routes and agent_service no longer directly call `execute_plan_*`.

### Recommended Slice 3 (pending_* Standardization)

**Scope:** Define schema for pending_* fields in VerbExecutionContext.extensions, migrate readers.

**Effort:** ~1 week (mostly definition and documentation)

**Outcome:**
- Platform state passed through a stable envelope.
- Session layer and UI can reliably parse side effects.

### Recommended Slice 4 (Runbook Integration)

**Scope:** Ensure all runbook/CompiledStep execution routes through VerbExecutionPort, not raw DslExecutor methods.

**Effort:** ~2 weeks

**Outcome:**
- Runbooks are the canonical execution model.
- Raw DSL strings no longer executed outside runbook context.

### Recommended Slice 5 (Legacy Cleanup)

**Scope:** Delete unused entry points (execute_with_dag), gate execute_dsl, remove test/CLI tools from main crate.

**Effort:** ~1 week

**Outcome:**
- Smaller, cleaner public API.
- Dead code removed.

---

## Appendix A — File Inventory

### domain_ops Files with execute_json (87 of 91)

All except: mod.rs, bpmn_lite_ops.rs, and 2 others without the method.

Thunking (14):
agent_ops, attribute_ops, booking_principal_ops, control_ops, gleif_ops, lifecycle_ops, manco_ops, request_ops, sem_os_schema_ops, session_ops, source_loader_ops, trading_profile, trading_profile_ca_ops, view_ops

Native (73):
access_review_ops, affinity_ops, agent_ops (shared with thunk), batch_control_ops, board_ops, bods_ops, calibration_ops, capital_ops, case_screening_ops, cbu_ops, cbu_role_ops, client_group_ops, constellation_ops, control_compute_ops, coverage_compute_ops, custody, deal_ops, derived_attributes, dilution_ops, discovery_ops, document_ops, economic_exposure_ops, edge_ops, entity_ops, entity_query, evidence_ops, graph_validate_ops, import_run_ops, investing_role_ops, investor_ops, journey_ops, kyc_case_ops, market_data_ops, matrix_overlay_ops, navigation_ops, observation_ops, onboarding, ontology_ops, outreach_ops, outreach_plan_ops, ownership_ops, partnership_ops, phrase_ops, reconciliation_ops, refdata_loader, refdata_ops, regulatory_ops, research_normalize_ops, research_workflow_ops, remediation_ops, requirement_ops, screening_ops, sem_os_audit_ops, sem_os_focus_ops, sem_os_governance_ops, sem_os_maintenance_ops, sem_os_registry_ops, semantic_ops, shared_atom_ops, skeleton_build_ops, state_ops, syndication_ops, template_ops, temporal_ops, tollgate_evaluate_ops, tollgate_ops, trading_matrix, trust_ops, ubo_analysis, ubo_compute_ops, ubo_graph_ops, ubo_registry_ops, verification_ops

### Call Sites by Caller Category

- **agent (6):** agent_routes.rs (2), agent_service.rs (1), agent_controller.rs (2), agent_mode.rs (0)
- **repl (3):** orchestrator_v2.rs (0), executor_bridge.rs (1), session_v2.rs (0)
- **runbook (1):** step_executor_bridge.rs (1)
- **mcp (3):** handlers/core.rs (3)
- **api (5):** agent_routes.rs (0), agent_service.rs (1), sheet_executor.rs (1), gleif/repository.rs (1), templates/harness.rs (2)
- **semos (2):** verb_executor_adapter.rs (2)
- **bpmn (0):** (separate service boundary)
- **domain_ops (2):** template_ops.rs (1), batch_executor.rs (1)
- **test (8):** bin/dsl_cli.rs (3), bin/batch_test_harness.rs (1), bin/dsl_api.rs (1), bulk_attribute_reconcile.rs (1)
- **other (16):** internal DslExecutor recursion (14), idempotency checks (2)

---

## Appendix B — Recommendation Summary

| Item | Action | Owner | Timeline |
|------|--------|-------|----------|
| Thunk elimination (14 ops) | Convert to native execute_json | Phase 2.5 Slice 1 | 1–2 weeks |
| Entry point migration (6 sites) | Route through VerbExecutionPort | Phase 2.5 Slice 2 | 2–3 weeks |
| Pending_* schema | Define in VerbExecutionContext.extensions | Phase 2.5 Slice 3 | 1 week |
| Runbook integration | Ensure all execution through runbook | Phase 2.5 Slice 4 | 2 weeks |
| Legacy cleanup | Delete dead code, gate execute_dsl | Phase 2.5 Slice 5 | 1 week |
| TOTAL ESTIMATED EFFORT | — | — | 7–10 weeks |

---

**Document Version:** 1.0  
**Last Updated:** 2026-04-19  
**Auditor:** Claude Code  
**Status:** Ready for review and decision on outstanding questions (Section 7)

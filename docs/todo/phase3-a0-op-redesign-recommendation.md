# Phase 3 CR A0 — Op Redesign: Option α Recommendation

**Date:** 2026-05-19
**Status:** Paper exercise — no code changes. Input for A1–A6.

---

## Decision

**Recommend Option α — Remove Op entirely.**

The planning path (`planning::compile` → `ExecutionPlan`) already works without Op and is the primary execution path. Op is a redundant layer; eliminating it is cleaner than thinning it.

---

## Blast radius

Files directly changed under α:

| File | Change |
|------|--------|
| `dsl-core/src/ops.rs` | Delete entirely |
| `dsl-core/src/dag.rs` | Delete `build_execution_plan`; phase grouping moves to YAML `phase_tags` |
| `dsl-core/src/compiler.rs` | Remove `compile_to_ops_ext`, `CompiledProgram`, `VerbHandler`; parser + arg helpers stay |
| `ob-poc-compiler/src/lib.rs` | Delete `ob_poc_verb_handler` — handler produces Ops; no longer needed |
| `dsl-analysis/src/planning_facade.rs` | Switch from `compile_to_ops_ext` to `planning::compile` path; phase metadata from YAML `phase_tags` |
| `executor.rs` | Delete `execute_with_dag`, `execute_op`, `execute_dsl` (Op-dispatch path; zero production callers) |
| `ob-agentic/src/validator.rs` | Switch `compile_to_ops` → `planning::compile` (same destination, different compile path) |

Files NOT changed (Op-path callers that are already on the planning path):
- `execute_plan_atomic_in_scope` — already uses `execute_verb_in_scope` directly ✓
- `execute_plan_atomic` — wraps `execute_plan_atomic_in_scope` ✓
- All tests that use `planning::compile` + `execute_plan` ✓

---

## Why α over β/γ

**F5 (the central finding):** All 13 live Op variants already reconstruct a VerbCall at dispatch time and route through `execute_verb()`. The Op type signature is destroyed at the dispatch boundary. Thinning Op (β) or adding phase tags (γ) preserves a layer that provides no execution-layer value.

**Evidence for α feasibility:**
1. `execute_plan_atomic_in_scope` (the canonical atomic path) works today without any Op involvement — it processes `ExecutionStep { verb_call, injections }` via `execute_verb_in_scope`.
2. `execute_with_dag` (the only production-facing Op entry point in executor.rs) has **zero production callers** — only appears in its own doc comment and in `generic_executor.rs` tests with `dry_run: true`.
3. `planning::compile` (`dsl_v2/execution_plan.rs`) already produces `ExecutionPlan` without Op — this path is what `trading_matrix_materialize_test.rs` and other integration tests use.
4. `ob_poc_verb_handler` (ob-poc-compiler) only exists to produce Op variants. Under α, verb dispatch goes through `SemOsVerbOpRegistry` + `GenericCrudExecutor` — both already registered without needing an Op middleman. The handler becomes a dead artifact.

**The one complication:** `planning_facade::analyse_and_plan` uses `compile_to_ops_ext` to produce Ops, then runs `build_dag_plan` to get phase ordering and cycle detection. Under α, phase ordering comes from verb YAML `phase_tags` (already declared: `phase_tags: [kyc]`, `phase_tags: [trading]`, etc.) and the `planning::compile` path already handles forward injection (dependency ordering). Cycle detection moves to the injection graph in `ExecutionPlan`. This is A5 work.

---

## CR sequence for A-track

| CR | Scope |
|----|-------|
| A1 | Introduce `CompileStep` struct (VerbCall + binding + source_stmt + dep_keys) as the new compilation output alongside Op. No callers migrated yet. |
| A2 | Migrate `planning_facade` to use `planning::compile` path; emit `CompileStep` for diagnostics; phase metadata from YAML phase_tags. Migrate ob-agentic validator. |
| A3 | Delete `execute_with_dag`, `execute_op`, `execute_dsl` from executor.rs (the Op-dispatch path). |
| A4 | Delete `Op` entirely. Delete `build_execution_plan`. Delete `compile_to_ops_ext` and `CompiledProgram`. Delete `ob_poc_verb_handler`. |
| A5 | Wire phase metadata from YAML `phase_tags` into `ExecutionPlan` for any callers that need phase grouping. Verify DAG cycle detection via injection graph. |
| A6 | Regression verification: LSP diagnostics, ob-agentic validator, all existing plan execution tests pass. |

---

## What the verb authoring pattern document enables

See `docs/architecture/dsl-verb-authoring-pattern.md` (produced alongside this CR). The pattern document is the primary deliverable that α makes possible: once Op is gone, the dispatch path is `VerbCall → SemOsVerbOpRegistry → SemOsVerbOp::execute`. The pattern document defines how YAML declarations and Rust SemOsVerbOp implementations are wired together, enforced at startup by the wiring check.

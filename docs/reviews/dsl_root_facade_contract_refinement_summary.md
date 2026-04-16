# DSL Root Facade Contract Refinement Summary

## 1. Root reexport classification

### True root contract
- Core syntax and AST entry points from `dsl-core` remain at [`rust/src/dsl_v2/mod.rs`](../../rust/src/dsl_v2/mod.rs): `parse_program`, `parse_single_verb`, `Argument`, `AstNode`, `Literal`, `Program`, `Span`, `Statement`, `VerbCall`, binding-context types, diagnostics, and compiler/DAG primitives from `dsl-core`.
- These are still contract-worthy at the root because they define the base language surface rather than one specific execution or tooling stage.

### Stage-contract only
- Planning-facing items were moved behind `dsl_v2::planning`: `compile`, `compile_with_planning`, planning result/context types, entity dependency analysis, `quick_validate`, and `analyse_and_plan`.
- Execution-facing items were moved behind `dsl_v2::execution`: `DslExecutor`, `ExecutionContext`, `ExecutionResult`, atomic/best-effort execution result types, runtime registry/config types, and gateway resolver helpers.
- Tooling-facing items were moved behind `dsl_v2::tooling`: validator entry points, validation request/result/diagnostic types, `LspValidator`, semantic-validator entry points, verb lookup helpers, and editor/planning support aliases.

### Deferred transitional compatibility
- Expansion and macro families remain root-visible in [`rust/src/dsl_v2/mod.rs`](../../rust/src/dsl_v2/mod.rs): `expand_templates*`, `BatchPolicy`, lock/policy/report types, macro registry/schema/loading functions, and fixpoint expansion types.
- `enrich_program`, `ConfigLoader`, and several `dsl-core` support modules remain root-visible.
- These were left alone in this pass because they still need a real contract decision rather than a mechanical trim.

## 2. Root surface reductions

Removed root `dsl_v2` reexports for these groups in [`rust/src/dsl_v2/mod.rs`](../../rust/src/dsl_v2/mod.rs):
- `entity_deps::*`
- `execution_plan::*`
- `planning_facade::*`
- `executor::*`
- `gateway_resolver::*`
- `generic_executor::*`
- `runtime_registry::*`
- `lsp_validator::LspValidator`
- `semantic_validator::*`
- `validation::*`
- `verb_registry::*`

The stage seams now source directly from their owning modules instead of depending on root aliases:
- `dsl_v2::planning`
- `dsl_v2::execution`
- `dsl_v2::tooling`

`ob-poc` crate-root reexports were adjusted in [`rust/src/lib.rs`](../../rust/src/lib.rs) so the crate still exposes `DslExecutor`, `ExecutionContext`, `DslV2ExecutionResult`, and `ReturnType`, but those now come from `dsl_v2::execution` rather than from the broad `dsl_v2` root.

## 3. Consumer migrations

### Moved to `syntax`
- `parse_program` imports in `ob-poc-web`, `xtask`, root tests, `repl`, `agent`, `api`, and `mcp` flows.
- Representative files:
  - [`rust/src/api/dsl_viewer_routes.rs`](../../rust/src/api/dsl_viewer_routes.rs)
  - [`rust/src/repl/executor_bridge.rs`](../../rust/src/repl/executor_bridge.rs)
  - [`rust/xtask/src/onboarding_harness.rs`](../../rust/xtask/src/onboarding_harness.rs)
  - [`rust/tests/transaction_rollback_integration.rs`](../../rust/tests/transaction_rollback_integration.rs)

### Moved to `planning`
- `compile`, `quick_validate`, and planning input/output imports in application code, tests, and xtask harnesses.
- Representative files:
  - [`rust/src/runbook/compiler.rs`](../../rust/src/runbook/compiler.rs)
  - [`rust/src/templates/harness.rs`](../../rust/src/templates/harness.rs)
  - [`rust/src/api/agent_routes.rs`](../../rust/src/api/agent_routes.rs)
  - [`rust/tests/db_integration.rs`](../../rust/tests/db_integration.rs)

### Moved to `execution`
- `DslExecutor`, `ExecutionContext`, `ExecutionResult`, runtime registry access, gateway resolver access, and `RuntimeVerbRegistry`.
- Representative files:
  - [`rust/crates/ob-poc-web/src/main.rs`](../../rust/crates/ob-poc-web/src/main.rs)
  - [`rust/src/mcp/handlers/core.rs`](../../rust/src/mcp/handlers/core.rs)
  - [`rust/src/api/agent_types.rs`](../../rust/src/api/agent_types.rs)
  - [`rust/src/research/agent_controller.rs`](../../rust/src/research/agent_controller.rs)
  - [`rust/src/bin/dsl_api.rs`](../../rust/src/bin/dsl_api.rs)

### Moved to `tooling`
- Validator/diagnostic types, `SemanticValidator`, `LspValidator`, verb lookup helpers, and runtime-verb editor support types.
- Representative files:
  - [`rust/crates/dsl-lsp/src/handlers/completion.rs`](../../rust/crates/dsl-lsp/src/handlers/completion.rs)
  - [`rust/crates/dsl-lsp/src/handlers/hover.rs`](../../rust/crates/dsl-lsp/src/handlers/hover.rs)
  - [`rust/crates/dsl-lsp/src/handlers/diagnostics.rs`](../../rust/crates/dsl-lsp/src/handlers/diagnostics.rs)
  - [`rust/tests/semantic_validator_integration.rs`](../../rust/tests/semantic_validator_integration.rs)
  - [`rust/tests/csg_pipeline_integration.rs`](../../rust/tests/csg_pipeline_integration.rs)

### Kept on `runbook` / existing facades
- This pass did not redesign `runbook`, `bpmn_integration`, or macro/expansion contracts.
- Existing public execution/tooling facades there were preserved rather than widened.

## 4. Remaining deferred root items

- Expansion and macro registry/schema/report types at `dsl_v2` root still need a deliberate contract call.
- `enrich_program` remains root-visible and still acts as a cross-stage helper.
- Root syntax/AST/compiler primitives from `dsl-core` remain broad by design for now.
- No attempt was made here to shrink `ob_poc::dsl_v2` down to a tiny facade if doing so would require rethinking the macro/expansion authoring contract.

## 5. Validation

Commands run:
- `cargo fmt`
- `env RUSTC_WRAPPER= cargo check -p dsl-lsp`
- `env RUSTC_WRAPPER= cargo check -p ob-poc --tests`
- `env RUSTC_WRAPPER= cargo check --workspace`
- `env RUSTC_WRAPPER= cargo test -p dsl-lsp --test parser_conformance`

Results:
- All commands passed.
- `parser_conformance` passed: 25 tests, 0 failures.
- A later follow-up removed the unused exports and dead helper seams behind this facade change, and the workspace now passes `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

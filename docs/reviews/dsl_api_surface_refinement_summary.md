# DSL API-Surface Refinement Summary

## Public seams preserved or introduced

- [`rust/src/dsl_v2/mod.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/mod.rs) now exposes stage-oriented seams:
  - `dsl_v2::syntax`
  - `dsl_v2::planning`
  - `dsl_v2::execution`
  - `dsl_v2::tooling`
- [`rust/src/runbook/mod.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/runbook/mod.rs) now acts as the execution-facing facade. Internal module topology is crate-private and the supported surface is carried by root reexports.
- [`rust/src/bpmn_integration/mod.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/bpmn_integration/mod.rs) now exposes a smaller root facade for BPMN runtime integration. Internal store/client/worker modules are crate-private, with only deliberate root reexports left public.

## Consumers migrated away from deep imports

- [`rust/crates/ob-poc-web/src/main.rs`](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-web/src/main.rs)
  - migrated from deep `dsl_v2` and `bpmn_integration` module paths to root/stage seams
  - later follow-up passes moved remaining runtime-registry access onto `dsl_v2::execution`
- [`rust/crates/dsl-lsp/tests/parser_conformance.rs`](/Users/adamtc007/Developer/ob-poc/rust/crates/dsl-lsp/tests/parser_conformance.rs)
  - moved to `dsl_v2::syntax`
- Root/workspace tests migrated off private module trees:
  - [`rust/tests/soft_delete_db.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/soft_delete_db.rs)
  - [`rust/tests/generic_lifecycle_guard_db.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/generic_lifecycle_guard_db.rs)
  - [`rust/tests/entity_deps_integration.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/entity_deps_integration.rs)
  - [`rust/tests/runbook_pipeline_test.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/runbook_pipeline_test.rs)
  - [`rust/tests/runbook_e2e_test.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/runbook_e2e_test.rs)
  - [`rust/tests/bpmn_integration_test.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/bpmn_integration_test.rs)
  - [`rust/tests/bpmn_e2e_harness_test.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/bpmn_e2e_harness_test.rs)
- `xtask` harnesses and the BPMN example no longer import executor/client internals by deep path:
  - [`rust/xtask/src/deal_harness.rs`](/Users/adamtc007/Developer/ob-poc/rust/xtask/src/deal_harness.rs)
  - [`rust/xtask/src/aviva_deal_harness.rs`](/Users/adamtc007/Developer/ob-poc/rust/xtask/src/aviva_deal_harness.rs)
  - [`rust/examples/complete_bpmn_job.rs`](/Users/adamtc007/Developer/ob-poc/rust/examples/complete_bpmn_job.rs)

## Internal modules made less visible

- `dsl_v2` internal plumbing narrowed to `pub(crate)`:
  - `entity_deps`
  - `execution_plan`
  - `executor`
  - `gateway_resolver`
  - `generic_executor`
  - `lsp_validator`
  - `semantic_validator`
- `runbook` internal topology narrowed to `pub(crate)` across canonicalization, compiler, constraint gate, executor, plan compiler/executor, response, classifier, and write-set helpers.
- `bpmn_integration` internal topology narrowed to `pub(crate)` across client/config/store/dispatcher/event bridge/worker modules.
- [`rust/src/plan_builder/mod.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/plan_builder/mod.rs) no longer publicly reexports private runbook modules wholesale; it now reexports only the actual classifier/constraint seam.

## Tests moved or rewritten to stop driving production visibility

- No tests were physically moved in this pass.
- The important boundary improvement is that root tests now target facade/root exports instead of private module paths.
- This means production visibility is no longer being kept broad for:
  - runbook canonical/error/classifier/executor modules
  - BPMN client/store/dispatcher module tree
  - `dsl_v2` executor/planner/entity-deps plumbing

## Deferred structural hotspots

- `dsl_v2` still has a broad public surface beyond the stage seams, but the main hotspot modules called out here were internalized in later passes. The remaining broad areas are mostly root reexports for macro/expansion families, `enrich_program`, and selected `dsl-core` primitives.
- `runbook` root exports are cleaner, but the capability boundary is still partly duplicated by `plan_builder` and REPL/orchestrator integration code.
- `bpmn_integration` now hides its module tree, but its root facade is still fairly large because `ob-poc-web` and workspace tests legitimately assemble BPMN runtime pieces in-process.
- Workspace root tests are still outside the owning modules/crates. They now use the right public seams, but deeper test relocation remains follow-up work.
- Internal dead-code and unused-export fallout from this pass was cleaned up later; the current workspace is now Clippy-clean.

## Validation

- `env RUSTC_WRAPPER= cargo check --workspace`
- `env RUSTC_WRAPPER= cargo check -p ob-poc --tests`
- `env RUSTC_WRAPPER= cargo test -p dsl-lsp --test parser_conformance`
- later follow-up validation also passed with `env RUSTC_WRAPPER= cargo clippy --workspace --all-targets --all-features -- -D warnings`

Results:

- All three commands passed.
- The subsequent Clippy gate also passed after the unused exports and dead internal seams were removed.

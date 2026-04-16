# DSL API-Surface Second-Pass Summary

## 1. Remaining hotspots reviewed

### `runtime_registry`
- Outcome: internalized as a module.
- Decision: the module tree itself is not the contract, but the runtime registry types are still a real execution/tooling contract.
- Current public shape:
  - module is now `pub(crate)`
  - contract-bearing types/functions are exposed through `dsl_v2::execution`
  - `dsl_v2::tooling` also exposes `RuntimeVerbRegistry`, `RuntimeVerb`, and `RuntimeBehavior`

### `planning_facade`
- Outcome: internalized as a module.
- Decision: it is now treated as implementation behind the `planning` and `tooling` seams rather than as its own public namespace.
- Current public shape:
  - module is now `pub(crate)`
  - `analyse_and_plan`, `PlanningInput`, `PlanningOutput`, and the planning synthetic-step type are exposed through existing public seams

### `validation`
- Outcome: internalized as a module.
- Decision: validation remains a real tooling contract, but not as a public module tree.
- Current public shape:
  - module is now `pub(crate)`
  - validation request/result/context/diagnostic types are exposed through `dsl_v2::tooling`

### `expansion`
- Outcome: internalized as a module.
- Decision: expansion helpers used by external tests are contract-worthy as types/functions, but not as a public namespace.
- Current public shape:
  - module is now `pub(crate)`
  - expansion functions and lock/policy types continue via root reexports

### `macros`
- Outcome: internalized as a module.
- Decision: macro schema/registry/expansion types remain part of the compile-facing contract, but the `dsl_v2::macros::*` module path is no longer the supported surface.
- Current public shape:
  - module is now `pub(crate)`
  - root reexports carry the contract-bearing schema/registry/fixpoint items still used externally

## 2. Consumer migrations

- [`rust/crates/ob-poc-web/src/main.rs`](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-web/src/main.rs)
  - moved `RuntimeBehavior` and `runtime_registry()` access to `dsl_v2::execution`

- [`rust/crates/dsl-lsp/src/handlers/diagnostics.rs`](/Users/adamtc007/Developer/ob-poc/rust/crates/dsl-lsp/src/handlers/diagnostics.rs)
  - moved from `planning_facade`, `runtime_registry`, and `validation` module paths to `dsl_v2::tooling`

- [`rust/crates/dsl-lsp/src/handlers/code_actions.rs`](/Users/adamtc007/Developer/ob-poc/rust/crates/dsl-lsp/src/handlers/code_actions.rs)
  - moved planning/validation imports to `dsl_v2::tooling`

- [`rust/crates/dsl-lsp/src/server.rs`](/Users/adamtc007/Developer/ob-poc/rust/crates/dsl-lsp/src/server.rs)
  - moved planning/semantic diagnostic types to `dsl_v2::tooling`

- [`rust/crates/dsl-lsp/src/handlers/completion.rs`](/Users/adamtc007/Developer/ob-poc/rust/crates/dsl-lsp/src/handlers/completion.rs)
  - stopped importing `load_macro_registry` through the old `macros` module path

- Root/workspace tests migrated off hotspot module paths:
  - [`rust/tests/db_integration.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/db_integration.rs)
  - [`rust/tests/expansion_determinism.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/expansion_determinism.rs)
  - [`rust/tests/lock_contention_integration.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/lock_contention_integration.rs)
  - [`rust/tests/intent_hit_rate.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/intent_hit_rate.rs)
  - [`rust/tests/runbook_pipeline_test.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/runbook_pipeline_test.rs)
  - [`rust/tests/runbook_e2e_test.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/runbook_e2e_test.rs)
  - [`rust/tests/csg_pipeline_integration.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/csg_pipeline_integration.rs)
  - [`rust/tests/semantic_validator_integration.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/semantic_validator_integration.rs)
  - [`rust/tests/dataflow_validation.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/dataflow_validation.rs)

- Internal bin cleanup:
  - [`rust/src/bin/dsl_api.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/bin/dsl_api.rs)
  - moved `runtime_registry()` access to the execution seam

## 3. Visibility changes

- [`rust/src/dsl_v2/mod.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/mod.rs)
  - `runtime_registry`: `pub mod` -> `pub(crate) mod`
  - `planning_facade`: `pub mod` -> `pub(crate) mod`
  - `validation`: `pub mod` -> `pub(crate) mod`
  - `expansion`: `pub mod` -> `pub(crate) mod`
  - `macros`: `pub mod` -> `pub(crate) mod`

- Public contract additions to support the narrower shape:
  - execution/tooling reexports for runtime registry types such as `RuntimeBehavior`, `RuntimeVerb`, `RuntimeArg`, and related runtime policy types
  - root reexports for macro schema/registry/fixpoint-expansion items that were previously only reachable under `dsl_v2::macros`
  - tooling exports for validation request/result/context/diagnostic types
  - tooling export for the planning synthetic-step type used by `dsl-lsp`

## 4. Deferred decisions

- `dsl_v2` root is still broad. Hiding these five hotspot modules reduced topology leakage, but the root facade still carries a large number of compile/runtime types.
- Macro contract trimming remains a later design question. This pass kept a broad set of macro schema/registry types public at the root because tests and runbook-facing compile flows still rely on them.
- Runtime registry contract trimming also remains open. This pass deliberately kept `RuntimeBehavior` and related runtime configuration types public because they appear in the registry’s public shape and are used by execution-facing consumers.
- The internal dead-code / unused-import fallout from these now-hidden modules was cleaned up later; the current workspace is Clippy-clean.

## 5. Validation

Commands run:

- `env RUSTC_WRAPPER= cargo check --workspace`
- `env RUSTC_WRAPPER= cargo check -p ob-poc --tests`
- `env RUSTC_WRAPPER= cargo check -p dsl-lsp`
- `env RUSTC_WRAPPER= cargo test -p dsl-lsp --test parser_conformance`

Results:

- All commands passed.
- Subsequent workspace validation also passed with `env RUSTC_WRAPPER= cargo clippy --workspace --all-targets --all-features -- -D warnings`.

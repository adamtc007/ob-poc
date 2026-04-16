# Peer Review: Conservative Visibility-Hygiene Refactor

## Scope

This change set implements the low-risk visibility reductions from the earlier audit, limited to:

- `rust/crates/ob-poc-macros`
- `rust/crates/governed_query_proc`
- `rust/crates/entity-gateway`
- `rust/crates/ob-workflow`
- `rust/crates/sem_os_harness`
- `rust/crates/inspector-projection`

Out of scope items from the audit were intentionally left untouched, especially anything that would require API redesign, facade redesign, or broader architectural decisions.

## What Changed

### 1. Internal proc-macro/helper cleanup

#### `ob-poc-macros`

- Reduced internal helper function visibility from `pub` to `pub(crate)`:
  - `derive_id_type_impl`
  - `register_custom_op_impl`

These are crate-internal proc-macro helpers and did not need wider exposure.

#### `governed_query_proc`

- Reduced clearly internal parse/cache/check/registry items from `pub` to `pub(crate)`:
  - `load_cache`
  - `Violation`
  - `run_checks`
  - `GovernedQueryArgs`
  - internal registry/cache enums, structs, constants, and lookup helpers

This was a direct cleanup of internal helper API surface without changing macro behavior.

### 2. Facade tightening in leaf crates

#### `entity-gateway`

- Made these internal modules private in `src/lib.rs`:
  - `config`
  - `index`
  - `refresh`
  - `search_engine`
  - `search_expr`
  - `server`
- Kept `proto` public.
- Kept the crate-root `pub use` facade intact.
- Updated internal/external consumers to import from the crate root instead of deep module paths:
  - `rust/crates/entity-gateway/src/main.rs`
  - `rust/crates/ob-poc-web/src/main.rs`

Result: the intended crate facade remains the public API, while plumbing is hidden.

#### `ob-workflow`

- Made these modules private in `src/lib.rs`:
  - `blob_store`
  - `cargo_ref`
  - `document`
  - `task_queue`
  - `listener` behind feature flag
- Kept existing root reexports unchanged.

Result: crate facade preserved, internal implementation modules hidden.

#### `sem_os_harness`

- Made these modules private in `src/lib.rs`:
  - `db`
  - `permissions`
  - `projections`
- Tightened `IsolatedDb` fields in `src/db.rs`:
  - `pool` -> `pub(crate)`
  - `dbname` -> `pub(crate)`

Result: helper/support internals are no longer unnecessarily exposed outside the crate.

#### `inspector-projection`

- Reduced `MAX_SCHEMA_VERSION` in `src/validate.rs` from `pub` to `pub(crate)`.
- Tightened generator internals in `src/generator/mod.rs`:
  - `deal` -> private
  - `matrix` -> private
- Removed the public `deal` barrel reexports from `generator/mod.rs`.

## Important Constraint Discovered

`inspector-projection` could not be fully tightened to the level suggested in the audit because `ob-poc` currently imports:

- `inspector_projection::generator::cbu::generate_from_cbu_graph`

Because that deep import is already in active use across crates, this refactor kept the minimum required visibility:

- `generator` remains public
- `generator::cbu` remains public

This was intentionally handled as a compatibility preservation measure rather than introducing a new facade or redesigning the public shape.

## Why This Is Safe

- No business logic was changed.
- No runtime behavior was changed.
- No new abstractions were introduced.
- Existing crate-root facades were preserved where already present.
- Visibility was only tightened where usage proved a narrower boundary was sufficient.
- Where a real cross-crate dependency existed, visibility was left at the minimum required level rather than forcing a redesign.

## Validation

Commands run during and after the refactor:

```bash
env RUSTC_WRAPPER= cargo check
env RUSTC_WRAPPER= cargo check -p entity-gateway
env RUSTC_WRAPPER= cargo check -p ob-workflow
env RUSTC_WRAPPER= cargo check -p sem_os_harness
env RUSTC_WRAPPER= cargo check -p inspector-projection
env RUSTC_WRAPPER= cargo check -p ob-poc-web
env RUSTC_WRAPPER= cargo check --workspace
```

Outcome:

- All checks passed.
- The only workspace fallout was stale deep imports in `ob-poc-web` after tightening `entity-gateway`; those were updated to use the existing crate-root facade.

## Review Focus

Peer review should focus on:

- Whether the narrowed visibility matches actual crate boundaries
- Whether any public item was tightened too aggressively for expected downstream use
- Whether keeping `inspector_projection::generator::cbu` public is the right temporary compatibility boundary
- Whether any follow-up facade work should be scheduled separately rather than folded into this hygiene pass

## Deferred Work

Not addressed here:

- root `ob-poc` visibility sweep
- `dsl-lsp`
- `sem_os_server`
- `ob-agentic` lexicon public-shape mismatches
- `ob-workflow` helper-type public-shape issues
- `taxonomy` public helper-type mismatches
- `dsl_v2` facade/macro exposure issues
- any dead-code cleanup surfaced by tighter module boundaries

Those remain separate structural follow-up work, not part of this conservative pass.

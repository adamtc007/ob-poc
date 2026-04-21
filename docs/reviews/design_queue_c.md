# Design Queue: Items Requiring Architecture Work

**Date:** 2026-04-16
**Audience:** Lead architect
**Purpose:** Items that need design decisions or API facade work before any mechanical execution. None of these are for Codex.

---

## C1: `xtask` — privileged consumer or facade boundary?

**Current state:** `xtask` imports from 8+ deep internal modules (`sem_reg::abac`, `sem_reg::agent::mcp_tools`, `sem_reg::gates`, `sem_reg::onboarding::*`, `calibration::*`, `agent::harness::runner::*`, `repl::decision_log::*`, `session::verb_contract`).

**Options:**
- (a) Declare `xtask` a privileged internal consumer. No action needed — imports are fine, and `xtask` alone is not sufficient reason to keep modules `pub`.
- (b) Build `pub mod tooling_api` facades in `sem_reg` and `calibration`. Higher effort, unclear value since `xtask` is `publish = false`.
- (c) Feature-gate with `#[cfg(feature = "tooling")]`. Adds compile-time complexity.

**Recommendation from investigation:** Option (a). See `opus_second_pub_api_surface_followup.md` §A.5.

---

## C2: `sem_os_postgres::constellation_hydration` — port trait or direct access?

**Current state:** `pub mod` with 1 external consumer (`rust/src/sem_os_runtime/hydration_impl.rs`).

**Options:**
- (a) Add a port trait in `sem_os_core::ports` and have `sem_os_postgres` implement it.
- (b) Keep as-is — single consumer, single implementation, port trait adds abstraction with no pluggability benefit.
- (c) Move the consumer closer to the implementation.

---

## C3: `playbook-core` / `playbook-lower` wildcard re-exports

**Current state:** Both crates use `pub use parser::*`, `pub use lower::*` etc. API surface is opaque from `lib.rs`.

**Action needed:** Audit which items are actually consumed, replace wildcards with explicit allowlists. Matches the feedback rule saved in memory: no `pub use types::*` for broad modules.

---

## ~~C4: `dsl-core::config::types` wildcard re-export~~ — MOVED TO CODEX TIER A (Phases 8-9)

Classification completed. 43 contract types, 11 internal types. Wildcard replacement is now in `codex_tier_a.md` Phases 8 and 9.

---

## C5: `sessions_for_test()` — production function with misleading name

**Current state:** The first review incorrectly flagged this as test-only. It is production code with 9+ call sites in `repl_routes_v2.rs` and 1 in `ob-poc-web/main.rs`. It already has `#[doc(hidden)]`. The name is misleading but the function is load-bearing.

**Action needed:** Rename to `session_store()` or similar. ~40 call sites need updating. This is a rename, not a visibility change.

---

## C6: `dsl_v2` seam completion — move executor variants

**Current state:** `batch_executor`, `graph_executor`, `sheet_executor` are `pub mod` at the `dsl_v2` root but logically belong behind the `execution` seam.

**Action needed:** Move these modules behind the `execution` seam. This is item relocation (changing module structure), not just visibility tightening. Requires updating all consumer import paths.

---

## C7: `test_with_verbs()` — cross-crate test support

**Current state:** `SemOsContextEnvelope::test_with_verbs()` is used in `#[cfg(test)]` blocks across multiple modules within the main crate (`traceability`, `repl`, `agent`). Simple `#[cfg(test)]` on the method won't work because it's on an impl block method, not a standalone function, and workspace integration tests may also need it.

**Options:**
- (a) `test-support` feature flag gating the method
- (b) Move test construction to a builder in a test-utilities module
- (c) Accept it as a permanently public test helper with `#[doc(hidden)]`

---

## C8: `sem_os_core::gates` and `sem_os_core::security` (conditional on Decision B1)

**Current state:** Zero external consumers. Removed from Codex Tier A because their disposition depends on Decision B1 (capability kernel vs domain model).

- If **B1-A (kernel):** These become `pub(crate)` as part of the broader logic-module restriction.
- If **B1-B (domain model):** These stay `pub` but get `#[doc(hidden)]` to stay out of rustdoc.

Either path is mechanical once the decision is made, but the decision has not been made yet.

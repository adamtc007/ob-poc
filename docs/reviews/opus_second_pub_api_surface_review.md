# Opus Second Review: Visibility, Crate Boundaries, and API Surface

**Reviewer:** Claude Opus 4.6 (independent second opinion)
**Date:** 2026-04-16
**Workspace:** `cargo check --workspace` passes cleanly (0 warnings relevant to visibility)
**Scope:** 22 active Rust crates (16 ob-poc + 6 sem_os_*), main crate with 53 public modules

---

## 1. Executive Summary

**Overall verdict:** The cleanup initiative has materially improved the architecture in the DSL and runbook/BPMN areas, where clear stage seams and fortress patterns now exist. The Sem OS *transport and client* boundaries are excellent. However, two large surfaces remain essentially untouched: `sem_os_core` (486 public items, zero `pub(crate)`) and the main `ob-poc` crate root (53 `pub mod` declarations). These two areas account for the majority of remaining incidental reachability.

**Direction:** Correct. The initiative is moving toward capability contracts rather than convenience surfaces. The seam pattern in `dsl_v2` and the fortress pattern in `runbook`/`bpmn_integration` are the right models to propagate.

**Top strengths:**
1. DSL four-seam architecture (`syntax`, `planning`, `execution`, `tooling`) with `pub(crate)` internals
2. Runbook and BPMN fortress patterns — all internal modules `pub(crate)`, curated re-exports
3. `sem_os_server` reduced to 3 public exports (`build_router`, `JwtConfig`, `OutboxDispatcher`)
4. `sem_os_client` is a genuine facade — single trait, two pluggable implementations

**Top concerns:**
1. `sem_os_core` is a flat public namespace — 44 `pub mod` declarations, zero `pub(crate)` anywhere in the crate
2. `dsl_v2` seams coexist with 60+ root-level re-exports, creating dual-path ambiguity
3. Test-only public functions exist without `cfg(test)` or feature gates
4. `xtask` and integration tests still reach into deep internal modules, sustaining `pub` pressure

---

## 2. What Has Improved

### Visibility hygiene

**DSL module (`dsl_v2/mod.rs`):** The most visible improvement. Twelve internal modules are now `pub(crate)`:
- `entity_deps`, `execution_plan`, `executor`, `expansion`, `gateway_resolver`, `generic_executor`, `lsp_validator`, `macros`, `planning_facade`, `runtime_registry`, `semantic_validator`, `validation`

These were previously reachable by any consumer. Now, access is mediated through four named seams. This is a genuine capability boundary, not just a visibility flip.

**Runbook module (`runbook/mod.rs`):** Textbook fortress pattern. All 16 internal modules are `pub(crate)`. The public surface is explicitly curated: `compile_invocation()`, `execute_runbook()`, and a bounded set of domain types. The module-level doc comments explicitly state the two-gate contract and invariants (INV-1a, INV-2, INV-3). This is the cleanest module boundary in the workspace.

**BPMN integration (`bpmn_integration/mod.rs`):** Same fortress pattern. All 14 internal modules `pub(crate)`. Public surface is curated types and entry points. Clean separation of stores, workers, and configuration.

**Feature gating in main crate:** The main `lib.rs` uses `#[cfg(feature = "database")]`, `#[cfg(feature = "server")]`, and `#[cfg(feature = "mcp")]` to conditionally compile 25 of 53 modules. This prevents accidental coupling when features are disabled and is a meaningful structural improvement.

### Sem OS boundaries

**`sem_os_server`:** Reduced to 3 exports. All handlers are private modules. The router function is the only consumer-facing surface. This is a closed deployment unit, not a library — the visibility reflects that correctly.

**`sem_os_client`:** Clean trait facade. The `SemOsClient` trait with 16 methods is the sole boundary. Two implementations (`InProcessClient`, `HttpClient`) are pluggable. The crate-level doc comment explicitly states: *"ob-poc depends on this crate, never on sem_os_postgres or sem_os_server."* This is a deliberate, documented contract.

**`sem_os_obpoc_adapter`:** Clean conversion bridge. Two public entry functions (`build_seed_bundle`, `build_seed_bundle_with_metadata`). Internal `pipeline_seeds` module is correctly private (`mod`, not `pub mod`). No domain logic leakage.

### DSL boundaries

**Stage seams are real, not cosmetic.** The `syntax`, `planning`, `execution`, and `tooling` modules in `dsl_v2` re-export from `pub(crate)` internal modules, creating genuine mediation points. A consumer writing `dsl_v2::planning::compile()` is going through an intentional gate, not a convenience path.

**`dsl-core` is appropriately open.** As a pure, no-database leaf library, its flat `pub mod` structure is justified. All modules are value types (AST, parser, diagnostics) with no mutable state or side effects. The re-exports at the crate root are consolidation, not leakage.

**`dsl-lsp` is appropriately tight.** Four re-exports. The LSP server is a closed consumer of DSL internals, not a public API.

### Test-boundary discipline

**`sem_os_harness`:** Correctly structured. Test-only modules (`db`, `permissions`, `projections`) are gated with `#[cfg(test)]`. The public functions (`run_core_scenario_suite`, etc.) operate against the `SemOsClient` trait, not implementation details. This is a genuine contract-testing harness.

**Runbook internal tests:** The `runbook/mod.rs` contains `#[cfg(test)] mod tests` that uses `super::*` — standard internal testing. These tests do not force any production visibility.

---

## 3. Remaining Public-Surface Leaks

### 3.1 Broad crate/module surfaces

**`sem_os_core` — 486 public items across 59 files, zero `pub(crate)` or `pub(super)` anywhere.**

This is the single largest remaining surface. Every module in `sem_os_core/src/lib.rs` is `pub mod` (44 modules). Every struct, enum, trait, and function within those modules is `pub`. The crate makes no distinction between:
- Port traits (`ports.rs` — 7 async trait interfaces) — genuine external contract
- Proto/DTO types (`proto.rs` — 19 request/response structs) — genuine external contract
- Domain body types (17 `*_def` modules) — genuine external contract (serializable domain models)
- Internal pipeline logic (`context_resolution.rs` — 54 pub items including `VerbFilterContext<'a>`, `rank_views_by_overlap()`) — likely internal
- Internal gate logic (`gates/` — governance and technical gate checks) — likely internal
- Stewardship types (`stewardship/types.rs` — 43 pub items) — mixed

The crate treats everything as equally external. This was not touched by the cleanup.

**Main crate root (`rust/src/lib.rs`) — 53 `pub mod` declarations.**

Every major subsystem is a top-level public module. Many are implementation-internal:
- `calibration` — loopback harness (internal tooling)
- `sage` — intent understanding layer (internal pipeline stage)
- `semtaxonomy`, `semtaxonomy_v2` — internal discovery contracts
- `state_reducer` — internal constellation reducer
- `placeholder` — deferred entity resolution (internal expansion detail)
- `plan_builder` — compilation pipeline decomposition (internal)

These modules are public because `xtask` and integration tests import from them, not because they represent external contracts.

### 3.2 Root re-export sprawl

**`dsl_v2/mod.rs` — dual-path ambiguity.**

The module has two overlapping access patterns:
1. **Seam path:** `dsl_v2::syntax::parse_program`, `dsl_v2::planning::compile`, etc.
2. **Root path:** `dsl_v2::parse_program`, `dsl_v2::Argument`, `dsl_v2::BindingContext`, etc.

Five core types are accessible via both paths simultaneously (`parse_program`, `parse_single_verb`, `BindingContext`, `BindingInfo`, `Program`). Additionally, types from `pub(crate)` modules `expansion` and `macros` are re-exported at the root level (lines 165-182), which undermines the encapsulation those `pub(crate)` declarations intended to provide.

This is a **visibility-echo re-export** pattern: the seams were introduced as a new access path, but the old root-level re-exports were not removed. The result is that consumers can bypass the seams entirely.

**Main crate root re-exports (`lib.rs` lines 206-218).**

```rust
pub use dsl_v2::execution::{DslExecutor, ExecutionContext, ...};
pub use dsl_v2::{parse_program, parse_single_verb, Argument, AstNode, ...};
```

These re-exports at the *crate* root create a third access path: `ob_poc::parse_program`. Combined with `ob_poc::dsl_v2::parse_program` and `ob_poc::dsl_v2::syntax::parse_program`, a consumer has three equivalent import paths for the same item. This dilutes the clarity of the seam architecture.

### 3.3 Items that are `pub` with no external-to-crate consumers

| Item | Location | Evidence |
|------|----------|----------|
| `sqlx_types` module | `sem_os_postgres/src/lib.rs` | Zero imports of `sem_os_postgres::sqlx_types` anywhere in workspace |
| `calibration` module | `ob-poc lib.rs` | Only consumed by `xtask` (dev tooling, not a crate consumer) |
| `sage` module | `ob-poc lib.rs` | Only consumed internally within the main crate |
| `semtaxonomy`, `semtaxonomy_v2` | `ob-poc lib.rs` | Only consumed internally |
| `state_reducer` | `ob-poc lib.rs` | Only consumed internally |
| `placeholder` | `ob-poc lib.rs` | Only consumed internally |
| `plan_builder` | `ob-poc lib.rs` | Only consumed internally |
| `system_info` module | `ob-poc lib.rs` | Trivial; no meaningful consumer |
| `VerbFilterContext<'a>` | `sem_os_core/context_resolution.rs` | Lifetime-bound helper — unlikely external contract |
| `rank_views_by_overlap()`, `compute_view_overlap()` | `sem_os_core/context_resolution.rs` | Internal scoring functions |

### 3.4 Adapter/harness leakage

**`sem_os_postgres::constellation_hydration`** is `pub mod` with exactly one external consumer (`sem_os_runtime/hydration_impl.rs`). This is a concrete Postgres query module exposed publicly because one consumer in the main crate needs it. This should either be a port trait (if the abstraction is worth it) or the consumer should live closer to the implementation.

### 3.5 Transitional compatibility surfaces

**`playbook-core` and `playbook-lower`** use wildcard re-exports (`pub use parser::*`, `pub use lower::*`). These make the public surface opaque — you cannot tell from `lib.rs` what is exported. This is a transitional pattern that predates the cleanup.

---

## 4. Capability-Boundary Assessment

### Sem OS

**What now looks deliberate:**
- `SemOsClient` trait as the sole external boundary — documented, enforced by crate structure
- `sem_os_server` as a closed deployment unit — 3 exports, private handlers
- `sem_os_obpoc_adapter` as a pure conversion bridge — 2 entry functions, private internals
- Port trait architecture in `sem_os_core::ports` — 7 async traits defining storage contracts

**What still looks broad or mixed-role:**
- `sem_os_core` itself. The crate conflates three roles:
  1. **Port definitions** (traits for storage) — genuine external contract
  2. **Domain model types** (body types, seeds, proto) — genuine external contract
  3. **Business logic** (context resolution pipeline, gate evaluation, stewardship rules, ABAC) — internal implementation that happens to be public

  The first two roles justify the crate's existence as a shared dependency. The third role is incidental — context resolution and gate logic are consumed only by `CoreServiceImpl` (in `sem_os_core::service`), not by external crates. Making them public creates a 486-item surface that is ~60% contract and ~40% incidental.

- `sem_os_postgres::sqlx_types` — public module with zero consumers. Pure internal serialization helpers.

### DSL

**What now looks deliberate:**
- Four named seams with `pub(crate)` internals — this is a real stage-oriented contract
- `dsl-core` as a pure leaf library — appropriate flat surface
- `dsl-lsp` as a tight facade — 4 exports
- `runbook` fortress — compile + execute gates, everything else hidden

**What still looks broad or mixed-role:**
- `dsl_v2` root module. The seams exist but don't dominate. The root still re-exports ~60 items directly, including types from `pub(crate)` modules (`expansion::*`, `macros::*`). This creates a situation where the seams are additive (a new way to access things) rather than exclusive (the only way to access things).
- `verb_registry`, `verb_taxonomy`, `operator_types`, `intent`, `intent_tiers` — these are `pub mod` at the `dsl_v2` root, but their role relative to the seams is unclear. Are they additional seam-level concerns, or internal implementation details?
- `batch_executor`, `graph_executor`, `sheet_executor` — three public executor variants at the root. Should these be behind the `execution` seam?

---

## 5. Test-Boundary Assessment

### Where internal tests are correctly internal

- `runbook/mod.rs` `#[cfg(test)] mod tests` — uses `super::*`, validates internal invariants, does not force any production `pub`
- `sem_os_harness` `#[cfg(test)]` modules — gated correctly, test DB isolation
- `dsl_v2` internal modules — individual module tests use `super::*` within `pub(crate)` modules
- `sem_os_obpoc_adapter` tests — use `super::*` within the crate, no external reaching

### Where workspace/harness tests still influence production visibility

**`xtask` as the primary pressure point.**

`xtask` imports from deep internal paths:
- `ob_poc::sem_reg::abac::ActorContext` — deep submodule of `sem_reg`
- `ob_poc::sem_reg::agent::mcp_tools::build_sem_os_service` — 3 levels deep
- `ob_poc::sem_reg::gates::evaluate_publish_gates` — internal gate evaluation
- `ob_poc::sem_reg::onboarding::{entity_infer, manifest, schema_extract, verb_extract, xref}` — 5 internal onboarding submodules
- `ob_poc::sem_reg::store::SnapshotStore` — internal store type
- `ob_poc::calibration::*` — entire calibration module

These imports require the entire `sem_reg` and `calibration` module trees to be `pub`. If `xtask` were restructured to use facade APIs (or if these modules were feature-gated for tooling), many internal modules could become `pub(crate)`.

**Integration tests (`rust/tests/`).**

`tests/sem_reg_integration.rs` imports:
- `ob_poc::sem_reg::attribute_def::AttributeDataType`
- `ob_poc::sem_reg::agent::decisions::AlternativeAction`
- `ob_poc::sem_reg::entity_type_def::EntityTypeDefBody`

These are deep-module imports that treat `sem_reg` submodules as if they were public API. The test was written to validate internals, but the production code stays public to support it.

### Test-only functions without proper gating

Four functions are `pub` in production code with test-specific names but no `#[cfg(test)]` gate:

| Function | Location |
|----------|----------|
| `sessions_for_test()` | `repl/orchestrator_v2.rs:629` |
| `insert_test_entry()` | `repl/verb_config_index.rs:262` |
| `test_emitter()` | `events/mod.rs:153` |
| `test_with_verbs()` | `agent/sem_os_context_envelope.rs:345` |

These are test-support functions baked into the production binary. They should be behind `#[cfg(test)]` or a `test-support` feature flag.

---

## 6. High-Risk or High-Value Remaining Hotspots

Ordered by impact-per-effort:

1. **`dsl_v2` root re-export cleanup.** Remove the 60+ root-level `pub use` statements that duplicate the seam exports. Make the seams the canonical (and only) access path. High value because it completes the stage-seam architecture that is already 80% done.

2. **`sem_os_core` role separation.** Identify which modules are port/contract (keep `pub`) vs. internal logic (make `pub(crate)`). The `context_resolution`, `gates`, `stewardship`, `enforce`, `grounding`, `abac`, `security` modules are strong candidates for `pub(crate)` — they are consumed only by `CoreServiceImpl` and tests.

3. **Test-only function gating.** Gate the 4 identified functions behind `#[cfg(test)]` or a feature flag. Low effort, removes test-support functions from production binary.

4. **`sem_os_postgres::sqlx_types` → `pub(crate)`.** Zero external consumers. Trivial change.

5. **`xtask` facade extraction.** Create a thin `pub mod tooling_api` (or similar) in `sem_reg` and `calibration` that exposes only what `xtask` needs. Then make the internal submodules `pub(crate)`. Higher effort but eliminates the largest source of test-driven `pub` pressure.

6. **Main crate root thinning.** Audit which of the 53 `pub mod` declarations have consumers outside the main crate. Candidates for `pub(crate)`: `sage`, `semtaxonomy`, `semtaxonomy_v2`, `state_reducer`, `placeholder`, `plan_builder`, `system_info`. Requires checking whether `xtask` or tests import them.

7. **`playbook-core`/`playbook-lower` wildcard re-exports.** Replace `pub use *` with explicit re-exports so the public surface is auditable from `lib.rs`.

---

## 7. Over-Correction Risks

### Places where the cleanup may be hiding too much

No evidence of over-correction found. The cleanup has been conservative — it tightened what was clearly internal (`dsl_v2` pipeline stages, `runbook` internals, `bpmn_integration` internals) without touching anything ambiguous. The risk is under-correction, not over-correction.

### Places where a remaining public type/path is probably justified

- **`sem_os_core` domain body types** (all 17 `*_def` modules): These are serializable domain models that flow across crate boundaries via `SeedBundle`, `SemOsClient` responses, and adapter conversions. They should remain public.

- **`sem_os_core::ports`**: The 7 async trait interfaces are the foundation of the hexagonal architecture. They must remain public for `sem_os_postgres` to implement them.

- **`sem_os_core::proto`**: Request/response DTOs consumed by `sem_os_client`, `sem_os_server`, and the harness. Genuine contract types.

- **`dsl_v2::expansion` re-exports at root** (`ExpansionReport`, `TemplatePolicy`, etc.): These are consumed by `runbook/compiler.rs` for lock derivation. The consumption is legitimate, but the re-export path (root instead of seam) is not. These should move to the `execution` or `planning` seam, not be hidden entirely.

- **`sem_os_postgres::constellation_hydration`**: The one consumer (`sem_os_runtime`) is in the main crate. This is a legitimate cross-crate dependency, but it would be cleaner if mediated through a port trait rather than direct Postgres query access.

---

## 8. Recommended Next Steps

### Conservative next slices (do now)

1. **Remove `dsl_v2` root-level re-exports of seam-internal types.** Keep the seam modules as the canonical paths. Redirect the ~10 items re-exported from `expansion::*` and `macros::*` to the appropriate seam. Update consumers (mostly internal — check with `cargo check`).

2. **`sem_os_postgres::sqlx_types` → `pub(crate)`.** Zero consumers, zero risk.

3. **Gate test-only functions.** Add `#[cfg(test)]` to `sessions_for_test`, `insert_test_entry`, `test_emitter`, `test_with_verbs`. If integration tests use them, use a `test-support` feature instead.

4. **Remove `dsl_v2` root-level re-exports of `dsl-core` modules.** Lines 32-67 of `dsl_v2/mod.rs` re-export entire `dsl_core` modules (`pub use dsl_core::ast`, `pub use dsl_core::parser`, etc.) alongside individual type re-exports. The module-level re-exports (`pub use dsl_core::ast`) give consumers `dsl_v2::ast::*` as a third path (in addition to `dsl_core::ast::*` and `dsl_v2::syntax::*`). Remove the module-level re-exports and keep only the type-level ones.

### Defer

- **`sem_os_core` role separation.** This is a larger architectural decision that needs explicit intent about what the "core" contract is (see Questions below). Don't rush it.

- **Main crate root thinning.** Requires understanding which modules `xtask` consumes and whether a `tooling_api` facade is worth building. Higher effort, moderate value.

- **`xtask` facade extraction.** Valuable but requires designing what the tooling API should look like. This is API design work, not mechanical `pub` tightening.

### Needs genuine API/facade design

- **`sem_os_core`:** The crate needs an explicit decision about whether it is a "core library" (everything public, consumers pick what they need) or a "capability kernel" (public ports + types, private logic). Both are valid. The current state is the former by accident rather than by design.

- **`dsl_v2` seam completion:** The seam architecture is 80% done. The remaining 20% is deciding: should root-level access be eliminated entirely, or should a curated root prelude coexist with the seams? The answer depends on whether downstream consumers (ob-poc-web, xtask) prefer flat imports or structured seam paths.

---

## 9. Java-Port Relevance (Brief)

The current boundary cleanup helps a future Java 25 port in two ways:

1. **Trait-based port architecture** in `sem_os_core` maps directly to Java interfaces. The `SnapshotStore`, `ObjectStore`, etc. traits would become `interface SnapshotStore { ... }` with clean implementations. This is already in good shape.

2. **Stage seams in DSL** map to Java module boundaries. `syntax`, `planning`, `execution` would become separate packages with package-private internals and public facade classes. The current `pub(crate)` discipline translates directly.

**Where contract clarity is still insufficient for a safe port:**

- `sem_os_core`'s flat public surface means a Java porter would not know which of the 486 public items are part of the contract vs. incidental. They would likely port everything, creating an unnecessarily large API surface in Java.
- `dsl_v2`'s dual-path ambiguity (root + seam) would force the Java porter to decide which path is canonical — a decision that should be made in Rust first.
- The 4 test-only functions would need to be identified and excluded manually.

---

## 10. Questions for the Lead Architect

1. **What is `sem_os_core`'s intended role?** Is it a "shared domain model" crate (everything public, consumers pick what they need) or a "capability kernel" (public ports + types, private business logic)? The answer determines whether 40% of its surface should become `pub(crate)`. Specifically: should `context_resolution`, `gates`, `stewardship`, `enforce`, `grounding`, and `abac` be externally reachable?

2. **Should the `dsl_v2` seams be exclusive?** The seam modules (`syntax`, `planning`, `execution`, `tooling`) were introduced alongside existing root re-exports. Should the root re-exports be removed to make seams the only access path, or should a curated root prelude coexist for convenience? This affects every downstream consumer's import style.

3. **Is `xtask` a first-class API consumer or a privileged internal tool?** If it's internal, its imports should not drive production visibility — a `#[cfg(feature = "tooling")]` gate or a `tooling_api` facade would let internal modules become `pub(crate)`. If it's a first-class consumer, the modules it imports are part of the contract by definition.

4. **Should `sem_os_postgres::constellation_hydration` be mediated through a port trait?** It's currently a public module with one consumer (`sem_os_runtime`). A port trait would make the hydration queries pluggable (consistent with the rest of the Sem OS architecture), but adds abstraction for a single implementation.

5. **What is the intended lifecycle of `playbook-core` and `playbook-lower`?** Their wildcard re-exports suggest they were written quickly and not yet cleaned up. Are they stable enough to warrant API surface attention, or are they likely to be rewritten?

---

*Review completed 2026-04-16. Workspace compiles cleanly. No code changes made.*

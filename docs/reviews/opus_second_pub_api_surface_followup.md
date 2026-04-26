# Opus Second Review — Follow-Up Investigation

**Reviewer:** Claude Opus 4.6 (second pass)
**Date:** 2026-04-16
**Follows:** `docs/reviews/opus_second_pub_api_surface_review.md`
**Scope:** Verification of first review claims, expanded visibility-toolkit audit, over-correction quantification

---

## A.1 Mechanical Verification of First Review Claims

### Claim: "zero `pub(crate)` in `sem_os_core`"

**Command:** `rg -c 'pub\(crate\)' rust/crates/sem_os_core/src/`
**Result:** No matches. **Claim confirmed.**

### Claim: "53 `pub mod` in main crate root"

**Command:** `rg '^pub mod' rust/src/lib.rs | wc -l`
**Result:** 53. **Claim confirmed.**

Note: 25 of 53 are behind `#[cfg(feature = "database")]`, `#[cfg(feature = "server")]`, or `#[cfg(feature = "mcp")]` guards. Without `--all-features`, only 28 modules compile. The rustdoc output (built without `--all-features`) still rendered all 53 modules because the default feature set includes `database`.

### Claim: "60+ root re-exports in `dsl_v2`"

**Command:** `rg '^pub use|^pub mod' rust/src/dsl_v2/mod.rs | wc -l`
**Result:** 64 (17 `pub use` from dsl-core, 20 `pub mod` local modules, 15 `pub use` from local modules, 8 `pub use` from pub(crate) modules, 4 `pub mod` seam definitions).

**Breakdown:**

| Category | Count | Description |
|----------|-------|-------------|
| `pub use dsl_core::*` (module re-exports) | 7 | `ast`, `parser`, `binding_context`, `config`, `diagnostics`, `dag`, `ops`, `compiler` |
| `pub use dsl_core::{items}` (type re-exports) | 10 | Individual types from dsl-core modules |
| `pub mod` (local public modules) | 20 | `applicability_rules`, `batch_executor`, `cardinality`, `csg_linter`, `display_nouns`, `domain_context`, `enrichment`, `errors`, `execution_result`, `graph_executor`, `idempotency`, `intent`, `intent_tiers`, `operator_types`, `ref_resolver`, `repl_session`, `sheet_executor`, `submission`, `suggestions`, `topo_sort`, `verb_registry`, `verb_taxonomy` |
| `pub use` from local public modules | 15 | Re-exports of specific types from above modules |
| `pub use` from `pub(crate)` modules | 8 | `expansion::*` (17 items), `macros::*` (22 items), `entity_deps::init_entity_deps` |
| `pub mod` seam definitions | 4 | `syntax`, `planning`, `execution`, `tooling` |

**First review said "60+". Actual count: 64.** Claim confirmed, but the composition matters: 8 of those 64 are the re-exports from `pub(crate)` modules that undermine encapsulation. The rest are either legitimate public modules or their type re-exports.

### Claim: §3.3 — 10 items "pub with no external-to-crate consumers"

| Item | First Review Claim | Actual External Consumers | Verdict |
|------|-------------------|--------------------------|---------|
| `sem_os_postgres::sqlx_types` | 0 external | 0 (used only by `store.rs` within the crate) | **Confirmed** |
| `calibration` module | "Only xtask" | xtask only (10+ refs in `xtask/src/calibration.rs`) | **Confirmed** — xtask is a dev tool, not a library consumer |
| `sage` module | "Only internal" | **WRONG** — 4 workspace test files + 2 internal files + 1 test. Total: 9 external consumer files | **Corrected** |
| `semtaxonomy` | "Only internal" | 0 external, 1 internal doc ref | **Confirmed** |
| `semtaxonomy_v2` | "Only internal" | 0 external, 3 internal consumers (`orchestrator`, `harness/assertions`, `intent_pipeline`) | **Confirmed** |
| `state_reducer` | "Only internal" | **WRONG** — 6 workspace test files (`reducer_*.rs`), 1 internal consumer (`domain_ops/state_ops.rs`) | **Corrected** |
| `placeholder` | "Only internal" | 0 external, 1 internal consumer (`domain_ops/entity_ops.rs`) | **Confirmed** |
| `plan_builder` | "Only internal" | 0 external, 1 internal consumer (`runbook/compiler.rs`) | **Confirmed** |
| `system_info` | "No meaningful consumer" | 0 external | **Confirmed** |
| `VerbFilterContext<'a>` | "Unlikely external contract" | 0 external to sem_os_core (used only by `service.rs` within core) | **Confirmed** |
| `rank_views_by_overlap()` | "Internal scoring" | 0 external to sem_os_core (used only by `service.rs`) | **Confirmed** |
| `compute_view_overlap()` | "Internal scoring" | 0 external to sem_os_core (internal only, including tests) | **Confirmed** |

**Corrections to first review:**
- `sage`: Has 4 workspace test consumers (`sage_coverage.rs`, `coder_clash_regressions.rs`, `utterance_api_coverage.rs`, `coder_clash_matrix.rs`) plus internal crate use. NOT "only internal." Making it `pub(crate)` would break these tests.
- `state_reducer`: Has 6 workspace test files. NOT "only internal." Making it `pub(crate)` would break `reducer_integration_tests.rs`, `reducer_eval_tests.rs`, `reducer_parser_tests.rs`, `reducer_validation_tests.rs`, `reducer_state_phase2_tests.rs`, `reducer_persistence_db.rs`.

### Claim: "4 test-only functions without `cfg(test)` gates"

| Function | First Review Claim | Actual Usage | Verdict |
|----------|-------------------|-------------|---------|
| `sessions_for_test()` | test-only | **WRONG** — 9 production call sites in `repl_routes_v2.rs` + 1 in `ob-poc-web/main.rs`. Misnamed but production. | **Critical correction** |
| `insert_test_entry()` | test-only | Used only in `#[cfg(test)]` blocks within the crate. **Confirmed test-only.** | Confirmed |
| `test_emitter()` | test-only | 0 external consumers. **Confirmed test-only.** | Confirmed |
| `test_with_verbs()` | test-only | Used in `#[cfg(test)]` blocks in `traceability/`, `repl/`, `agent/verb_surface.rs`. Internal test use only. | Confirmed but needs feature gate, not `#[cfg(test)]` |

**Critical correction:** `sessions_for_test()` is NOT test-only. It is the production session store accessor used by all REPL route handlers. It has `#[doc(hidden)]` but is a genuine production API. The name is misleading but the function is load-bearing. **Do NOT gate behind `#[cfg(test)]`.**

---

## A.2 Full Visibility-Toolkit Audit

### `#[doc(hidden)]` usage

**Command:** `rg '#\[doc\(hidden\)\]' --type rust`
**Result:** 3 occurrences across the workspace:

| Location | Target | Assessment |
|----------|--------|------------|
| `rust/src/repl/orchestrator_v2.rs:629` | `sessions_for_test()` | Appropriate — production but internal, bad name |
| `rust/crates/ob-poc-macros/src/register_op.rs` | Generated `ctor` initializer function | Appropriate — macro implementation detail |
| `artifacts/peer_review/.../orchestrator_v2.rs` | Stale copy in peer review artifact | Irrelevant |

**Assessment:** `#[doc(hidden)]` is dramatically under-used. Strong candidates for `#[doc(hidden)]`:
- All 4 "test-named" functions (even `sessions_for_test` which is production but internal)
- `system_info` module in main crate root
- `dsl_v2` root re-exports that duplicate seam paths (if seams are declared additive rather than exclusive)
- Internal `sem_os_core` pipeline modules if the architect decides "domain model" role (where modules stay pub but should not appear in rustdoc)

### `pub(super)` usage

**Command:** `rg 'pub\(super\)' --type rust -c`
**Result:** 100 occurrences across 7 files:

| File | Count | Purpose |
|------|-------|---------|
| `rust/src/mcp/handlers/session_tools.rs` | 16 | Handler functions visible to parent `handlers` module only |
| `rust/src/mcp/handlers/batch_tools.rs` | 25 | Same pattern |
| `rust/src/mcp/handlers/learning_tools.rs` | 24 | Same pattern |
| `rust/src/mcp/handlers/cross_workspace_tools.rs` | 8 | Same pattern |
| `rust/src/mcp/handlers/core.rs` | 25 | Same pattern |
| `rust/src/repl/session_v2.rs` | 1 | Single helper visible to parent `repl` module |

**Assessment:** `pub(super)` is used exclusively in the MCP handler subsystem, and correctly so — handler functions need to be visible to the parent `handlers/mod.rs` for routing but should not be visible to the rest of the crate. This is a well-applied pattern. It is not used anywhere else in the codebase. There are opportunities to apply it in other hierarchical module structures (e.g., `sem_reg` submodules, `agent` submodules).

### `pub(in path)` usage

**Command:** `rg 'pub\(in ' --type rust`
**Result:** 0 occurrences. Not used anywhere.

**Assessment:** This is the most granular Rust visibility modifier. Its absence is unsurprising — `pub(super)` and `pub(crate)` cover most use cases. No action needed.

### Rustdoc surface audit

**Command:** `cargo doc --workspace --no-deps` (completed in 30s, 20 warnings)

**Key findings:**
- `ob_poc`: All 53 modules rendered (including feature-gated ones). Rustdoc exposes the full surface.
- `sem_os_core`: All 41 modules rendered. No `#[doc(hidden)]` on any module.
- `dsl_v2`: All 20 public modules + 4 seams rendered. The seams appear alongside the raw modules with no visual distinction.

**Mismatch between intended and documented surface:**
- `sem_os_core::gates` and `sem_os_core::security` have 0 external consumers but render in rustdoc as if they were public API
- `dsl_v2` seam modules (`syntax`, `planning`, `execution`, `tooling`) render at the same level as raw modules (`enrichment`, `cardinality`, etc.) — no way for a consumer to tell which are the intended access points

---

## A.3 Over-Correction Quantification

### Root re-export removal cost (§8.1 of first review)

For each `pub use` in `dsl_v2/mod.rs` that re-exports from a `pub(crate)` module, I counted consumers outside `dsl_v2/` itself:

#### `expansion` module re-exports (17 items re-exported at root)

| Item | Main crate | ob-poc-web | xtask | Tests | Total | Via root path? |
|------|-----------|------------|-------|-------|-------|---------------|
| `expand_templates` | 1 (`agent_routes`) | 0 | 0 | 1 (`expansion_determinism`) | 2 | Yes |
| `expand_templates_simple` | 1 (`agent_routes`) | 0 | 0 | 0 | 1 | Yes |
| `BatchPolicy` | 3 (`agent_routes`, `expansion_audit` x2) | 0 | 0 | 1 | 4 | Yes |
| `ExpansionReport` | 1 (`expansion_audit`) | 0 | 0 | 0 | 1 | Yes |
| `LockAccess` | 0 | 0 | 0 | 1 | 1 | Test only |
| `LockKey` | 0 | 0 | 0 | 1 | 1 | Test only |
| `LockMode` | 0 | 0 | 0 | 1 | 1 | Test only |
| `ExpansionDiagnostic` | 0 | 0 | 0 | 0 | 0 | — |
| `ExpansionError` | 0 | 0 | 0 | 0 | 0 | — |
| `ExpansionOutput` | 0 | 0 | 0 | 0 | 0 | — |
| `LockTarget` | 0 | 0 | 0 | 0 | 0 | — |
| `LockingPolicy` | 0 | 0 | 0 | 0 | 0 | — |
| `PerItemOrigin` | 0 | 0 | 0 | 0 | 0 | — |
| `RuntimePolicy` | 0 | 0 | 0 | 0 | 0 | — |
| `TemplateDigest` | 0 | 0 | 0 | 0 | 0 | — |
| `TemplateInvocationReport` | 0 | 0 | 0 | 0 | 0 | — |
| `TemplatePolicy` | 0 | 0 | 0 | 0 | 0 | — |

**Summary:** Of 17 expansion re-exports, 10 have zero consumers. 4 have main-crate consumers. 3 are test-only. Removing the 10 zero-consumer items is zero-cost. The remaining 7 need migration to the `execution` seam or a dedicated `expansion` seam.

#### `macros` module re-exports (22 items re-exported at root)

| Item | Main crate | ob-poc-web | xtask | Tests | Total | Via root path? |
|------|-----------|------------|-------|-------|-------|---------------|
| `MacroRegistry` | 0 via root, 12 via `dsl_v2::macros::MacroRegistry` | 0 | 0 | 0 | 12 deep | Deep path only |
| `load_macro_registry` | 0 | 0 | 0 | 0 | 0 | — |
| `load_macro_registry_from_dir` | 0 | 0 | 0 | 1 | 1 | Test only |
| All other 19 items | 0 | 0 | 0 | 0 | 0 | — |

**Summary:** Of 22 macro re-exports, 21 have zero external consumers via the root path. `MacroRegistry` is consumed by 12 sites — but ALL go through `dsl_v2::macros::MacroRegistry` (the deep path), not `dsl_v2::MacroRegistry` (the root re-export). The root re-export of `MacroRegistry` itself has 0 consumers. **All 22 root re-exports of macro types can be removed with zero breakage** because the deep `dsl_v2::macros::` path is used instead. However, the deep path itself relies on `macros` being `pub(crate)` plus the root re-export. If the root re-export is removed, consumers using `crate::dsl_v2::macros::MacroRegistry` still work (within the crate) because `macros` is `pub(crate)`.

#### `entity_deps` module re-exports (1 item)

| Item | Consumers | Via root path? |
|------|-----------|---------------|
| `init_entity_deps` | 0 | — |

**Summary:** Zero consumers. Remove with zero cost.

### Overall removal cost assessment

| Category | Items | Zero consumers | Migration needed | Test-only |
|----------|-------|---------------|-----------------|-----------|
| expansion re-exports | 17 | 10 | 4 (main crate) | 3 |
| macros re-exports | 22 | 22 (root path) | 0 | 0 |
| entity_deps re-export | 1 | 1 | 0 | 0 |
| **Total** | **40** | **33** | **4** | **3** |

**The "mechanical" framing is correct.** 33 of 40 `pub(crate)` module re-exports can be removed with zero consumer impact. The 4 main-crate consumers (`expand_templates`, `expand_templates_simple`, `BatchPolicy`, `ExpansionReport`) need their import paths changed from `dsl_v2::X` to `dsl_v2::execution::X` (or a new seam). The 3 test-only items need their test updated. This is genuinely mechanical, not design work.

---

## A.4 `dsl-core` Re-Examination

### Current public surface

`dsl-core/src/lib.rs` exposes:

| Category | Modules | Items at root |
|----------|---------|--------------|
| AST | `pub mod ast` | 5 re-exported types (`AstNode`, `Program`, `Span`, `Statement`, `VerbCall`) + 2 functions |
| Parser | `pub mod parser` | 1 re-exported function (`parse_program`) |
| Binding | `pub mod binding_context` | 1 re-exported type (`BindingContext`) |
| Config | `pub mod config` → `pub mod types` | **`pub use types::*` — 54 types wildcard-exported** |
| Diagnostics | `pub mod diagnostics` | 4 re-exported types (`Diagnostic`, `DiagnosticCode`, `Severity`, `SourceSpan`) |
| DAG | `pub mod dag` | 0 at root (accessed via module) |
| Ops | `pub mod ops` | 0 at root (accessed via module) |
| Compiler | `pub mod compiler` | 0 at root (accessed via module) |
| Viewport | `pub mod viewport_parser` | 4 re-exported types/functions |

**Total public items in crate:** 116 (`rg '^pub (struct|enum|trait|type|fn|const)' --type rust rust/crates/dsl-core/src/ -c` → 116)

### The `pub use types::*` problem

`dsl-core::config::types` contains 54 public types. These are wildcard-exported at two levels:
1. `config/mod.rs`: `pub use types::*` — so `dsl_core::config::VerbConfig` works
2. `lib.rs`: `pub use config::types::*` — so `dsl_core::VerbConfig` works

This means all 54 config types are available at the crate root. Consumers use them via both paths:
- `dsl_core::config::types::VerbsConfig` (37 consumer files across the workspace)
- `dsl_core::VerbConfig` (root path)

### Consumer analysis

37 files outside `dsl-core` import from `dsl_core::`. The dominant pattern: `dsl_core::config::types::{VerbsConfig, VerbConfig, ArgConfig, VerbBehavior}`. The most-used types:

| Type | Consumer count | Role |
|------|---------------|------|
| `VerbsConfig` | ~20 | Top-level config container |
| `VerbConfig` | ~15 | Per-verb definition |
| `ArgConfig` | ~10 | Argument definition |
| `VerbBehavior` | ~8 | Behavior enum |
| `VerbMetadata` | ~5 | Metadata struct |
| `DomainConfig` | ~5 | Domain container |
| `ArgType` | ~4 | Argument type enum |

### Is a prelude appropriate?

**Yes.** A small curated prelude of ~8 items would serve 90%+ of consumers:
- `VerbsConfig`, `DomainConfig`, `VerbConfig`, `ArgConfig`
- `VerbBehavior`, `ArgType`, `VerbMetadata`
- `parse_program`

The remaining 46 config types (lock configs, rule configs, CRD configs, etc.) are specialized and used by fewer than 3 consumers each. They should remain accessible via `dsl_core::config::types::*` but do not belong at the crate root.

### Assessment

The first review's label "appropriately open" was **partially wrong**. `dsl-core` as a leaf library correctly makes all modules `pub`. But the wildcard `pub use types::*` at the crate root dumps 54 specialized types into the root namespace unnecessarily. This is not "curated" — it's "everything at the top." A prelude would improve the contract without restricting access.

**However, this is Tier C work** (API design, not mechanical tightening). Removing the wildcard would break 37 consumer files.

---

## A.5 `xtask` Deep-Import Inventory

### All `xtask` imports grouped by source module

**DSL seam imports (clean):**
- `ob_poc::dsl_v2::execution::{DslExecutor, ExecutionContext, ExecutionResult, RuntimeVerbRegistry}` — 4 files
- `ob_poc::dsl_v2::planning::compile` — 4 files
- `ob_poc::dsl_v2::syntax::parse_program` — 4 files

**Deep internal imports:**

| Source module | Import path | Depth | Files |
|--------------|-------------|-------|-------|
| `agent::learning` | `ob_poc::agent::learning::embedder::CandleEmbedder` | 3 levels | calibration.rs |
| `agent::harness` | `ob_poc::agent::harness::{load_all_suites, load_suite, ScenarioSuite}` | 2 levels | harness.rs |
| `agent::harness::runner` | `ob_poc::agent::harness::runner::{dump_failures, print_suite_report, run_suite, SuiteResult}` | 3 levels | harness.rs |
| `api::agent_service` | `ob_poc::api::agent_service::AgentService` | 2 levels | calibration.rs |
| `calibration` | `ob_poc::calibration::{CalibrationMode, CalibrationScenario, ...}` | 1 level | calibration.rs |
| `domain_ops` | `ob_poc::domain_ops::CustomOperationRegistry` | 1 level | verbs.rs |
| `repl::decision_log` | `ob_poc::repl::decision_log::{...}` | 2 levels | replay_tuner.rs |
| `sem_reg` | `ob_poc::sem_reg::{ObjectType, RegistryService, SnapshotStore}` | 1 level | sem_reg.rs |
| `sem_reg::abac` | `ob_poc::sem_reg::abac::ActorContext` | 2 levels | calibration.rs, sem_reg.rs |
| `sem_reg::agent::mcp_tools` | `ob_poc::sem_reg::agent::mcp_tools::build_sem_os_service` | 3 levels | calibration.rs, sem_reg.rs |
| `sem_reg::gates` | `ob_poc::sem_reg::gates::{evaluate_publish_gates, GateMode, GateSeverity}` | 2 levels | sem_reg.rs |
| `sem_reg::gates_technical` | `ob_poc::sem_reg::gates_technical::check_security_label_presence` | 2 levels | sem_reg.rs |
| `sem_reg::onboarding` | `ob_poc::sem_reg::onboarding::{entity_infer, manifest, schema_extract, verb_extract, xref, seed}` | 2 levels | sem_reg.rs |
| `sem_reg::scanner` | `ob_poc::sem_reg::scanner::{run_onboarding_scan, suggest_security_label}` | 2 levels | sem_reg.rs |
| `sem_reg::types` | `ob_poc::sem_reg::types::{Classification, ObjectType, ...}` | 2 levels | sem_reg.rs, calibration.rs |
| `session` | `ob_poc::session::{UnifiedSession, verb_contract::VerbDiagnostics, verb_sync::VerbSyncService, verb_tiering_linter}` | 1-2 levels | calibration.rs, verbs.rs |

### Facade feasibility assessment

**`sem_reg` tooling facade:** 6 xtask files use `sem_reg`, but the usage is deeply varied — scanning, gate evaluation, onboarding, metrics, tool specs, actor contexts. A single facade would need 15+ functions. Better answer: **declare `xtask` a privileged internal consumer** for `sem_reg`.

**`calibration` tooling facade:** All usage is in `xtask/src/calibration.rs`. The module is already a single consumer. A facade adds nothing.

**`agent::harness` facade:** Two entry points (`load_all_suites` + `run_suite`). Already a reasonable API. The `runner` submodule (`dump_failures`, `print_suite_report`) could be consolidated into the parent.

### Recommendation

`xtask` should be declared a **privileged internal consumer**. The cost of building tooling facades exceeds the visibility benefit, because:
1. `xtask` is `publish = false` and lives in the same workspace
2. Its imports are concentrated in 5 files with specific, non-overlapping concerns
3. A `#[cfg(feature = "tooling")]` gate would add compile-time complexity for zero runtime benefit
4. The deep imports reflect genuine needs (running scans, evaluating gates, building embedders)

The architectural decision is: "xtask imports do not justify making internal modules `pub`." If modules should be `pub(crate)` for other reasons, `xtask` should be migrated at that time. But `xtask` alone is not sufficient reason to keep modules `pub`.

---

## A.6 Updated Findings Summary

### Updated §3: Remaining Leaks (verified numbers)

**Confirmed leaks:**
1. `sem_os_core`: 486 pub items, 41 modules, zero `pub(crate)`, zero `pub(super)`, zero `#[doc(hidden)]`. 2 modules (`gates`, `security`) have zero external consumers.
2. `dsl_v2` root: 64 `pub` declarations. 40 re-exports from `pub(crate)` modules (33 with zero consumers, 4 needing main-crate migration, 3 test-only).
3. `dsl-core`: 54 config types wildcard-exported at crate root via `pub use types::*`.
4. `sem_os_postgres::sqlx_types`: pub with zero external consumers.
5. `sessions_for_test()`: Misnamed production function, not test-only. Has `#[doc(hidden)]` but needs renaming not gating.
6. `test_emitter()`: Zero consumers outside defining file. True test-only.
7. `insert_test_entry()`: Used only in `#[cfg(test)]` blocks. True test-only.
8. `test_with_verbs()`: Used in `#[cfg(test)]` blocks across crate. Needs feature gate not `#[cfg(test)]`.

**Corrected from first review:**
- `sage`: Has 4 workspace test file consumers. NOT "only internal."
- `state_reducer`: Has 6 workspace test file consumers. NOT "only internal."
- `sessions_for_test()`: Is PRODUCTION code with 9+ production call sites. NOT test-only.

### Updated §6: Hotspots (re-ordered by verified cost/value)

1. **Remove 33 zero-consumer root re-exports from `dsl_v2`.** Zero breakage. Pure deletion.
2. **`sem_os_postgres::sqlx_types` → `pub(crate)`.** Zero breakage.
3. **Gate `test_emitter()` and `insert_test_entry()` behind `#[cfg(test)]`.** Zero breakage (only used in test blocks).
4. **Migrate 4 expansion items to execution seam.** 4 call sites in main crate need import path update.
5. **`sem_os_core::gates` and `sem_os_core::security` → `pub(crate)`.** Zero external consumers (if confirmed by full build). Needs architect decision on core's role.
6. **`VerbFilterContext<'a>`, `rank_views_by_overlap()`, `compute_view_overlap()` → `pub(crate)` in `sem_os_core`.** Zero external consumers. Internal to `service.rs` resolution pipeline.

---

*Investigation complete. All claims verified against actual workspace state. 3 corrections to the first review identified and documented.*

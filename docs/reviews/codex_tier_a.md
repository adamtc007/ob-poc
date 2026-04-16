# Codex Tier A: Mechanical pub API Surface Cleanup

**Target executor:** Codex 5.4
**Date:** 2026-04-16 (revised 2026-04-16)
**Prerequisite:** Rust workspace compiles cleanly from `rust/` (`cd rust && env RUSTC_WRAPPER= cargo check --workspace --all-features`).
**Context:** `docs/reviews/opus_second_pub_api_surface_followup.md` contains the verification evidence backing every phase.

This document contains ONLY phases that Codex can execute without interpretive decisions. Every pre-edit grep has been run and confirmed zero consumers. Every change is a visibility tightening with verified zero breakage.

Execution note for the current tree:
- Phases 2 and 3 may already be satisfied before execution. If the target item already matches the stated end-state, treat the phase as a verified no-op and proceed without editing.
- All `cargo` verification commands in this document must be run from `rust/`.

---

## Scope Discipline (MANDATORY)

You MUST NOT:
- Make "while I'm here" edits to unrelated code
- Commit any changes (the architect commits)
- Touch any item not listed in this document
- Change test code unless the phase is specifically about test gating
- Move, rename, or restructure any item (visibility changes only — item location is out of scope)
- Add comments, docstrings, or annotations beyond the specific changes listed

You MUST:
- Follow each phase in exact order
- Run the pre-edit verification before every change
- Run the post-edit verification after every change
- Stop and report if any verification fails
- Leave all changes uncommitted

---

### Phase 1 of 10: `sem_os_postgres::sqlx_types` → `pub(crate)` (Progress: 8%)

**Pre-conditions:** Workspace compiles cleanly.

**Pre-edit verification:**
```bash
rg 'sem_os_postgres::sqlx_types' --type rust -g '!rust/crates/sem_os_postgres/*'
```
Expected: 0 matches.
If any matches found: STOP, report, do not proceed.

**Change:**
- File: `rust/crates/sem_os_postgres/src/lib.rs`
- Line 14: `pub mod sqlx_types;`
- Edit: `pub mod sqlx_types;` → `pub(crate) mod sqlx_types;`

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p sem_os_postgres --all-features
```
Expected: clean compile.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 2.

**E-invariant:** `sqlx_types` is no longer reachable from outside `sem_os_postgres`.

---

### Phase 2 of 10: `test_emitter()` → `#[cfg(test)]` (Progress: 16%)

**Pre-conditions:** Phase 1 passed.

**Pre-edit verification:**
```bash
rg 'test_emitter' --type rust -g '!rust/src/events/*'
```
Expected: 0 matches.
If any matches found: STOP, report, do not proceed.

**Change:**
- File: `rust/src/events/mod.rs`
- Line 153: `pub fn test_emitter(buffer_size: usize) -> (SharedEmitter, EventReceiver) {`
- Edit: Add `#[cfg(test)]` on the line immediately before the function definition. Do NOT change the `pub` keyword or the function signature.

Result should read:
```rust
#[cfg(test)]
pub fn test_emitter(buffer_size: usize) -> (SharedEmitter, EventReceiver) {
```

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p ob-poc --all-features
```
Expected: clean compile.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 3.

**E-invariant:** `test_emitter` is only compiled in test builds.

---

### Phase 3 of 10: `insert_test_entry()` → `#[cfg(test)]` (Progress: 24%)

**Pre-conditions:** Phase 2 passed.

**Pre-edit verification:**
```bash
rg 'insert_test_entry' --type rust -g '!rust/src/repl/verb_config_index.rs'
```
Expected: All matches are inside `#[cfg(test)]` blocks (check manually — they should be in `deterministic_extraction.rs` and `write_set.rs`, both in `#[cfg(test)] mod tests`).
If any match is NOT inside a `#[cfg(test)]` block: STOP, report, do not proceed.

**Change:**
- File: `rust/src/repl/verb_config_index.rs`
- Line 262: `pub fn insert_test_entry(&mut self, entry: VerbIndexEntry) {`
- Edit: Add `#[cfg(test)]` on the line immediately before the function definition. Do NOT change the `pub` keyword.

Result should read:
```rust
    #[cfg(test)]
    pub fn insert_test_entry(&mut self, entry: VerbIndexEntry) {
```

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p ob-poc --all-features
```
Expected: clean compile.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 4.

**E-invariant:** `insert_test_entry` is only compiled in test builds.

---

### Phase 4 of 10: Remove zero-consumer `expansion` re-exports from `dsl_v2` root (Progress: 32%)

**Pre-conditions:** Phase 3 passed.

**Pre-edit verification (for each item):**
```bash
rg 'dsl_v2::ExpansionDiagnostic[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::ExpansionError[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::ExpansionOutput[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::LockTarget[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::LockingPolicy[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::PerItemOrigin[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::RuntimePolicy[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::TemplateDigest[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::TemplateInvocationReport[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
rg 'dsl_v2::TemplatePolicy[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
```
Expected: 0 matches for ALL 10 items.
If any match found: STOP, report which item has consumers, do not proceed.

**Change:**
- File: `rust/src/dsl_v2/mod.rs`
- Lines 164-169: Replace the `expansion` re-export block.

Current (lines 164-169):
```rust
// Re-export expansion module types
pub use expansion::{
    expand_templates, expand_templates_simple, BatchPolicy, ExpansionDiagnostic, ExpansionError,
    ExpansionOutput, ExpansionReport, LockAccess, LockKey, LockMode, LockTarget, LockingPolicy,
    PerItemOrigin, RuntimePolicy, TemplateDigest, TemplateInvocationReport, TemplatePolicy,
};
```

Replace with:
```rust
// Re-export expansion module types (consumed externally)
pub use expansion::{
    expand_templates, expand_templates_simple, BatchPolicy, ExpansionReport, LockAccess, LockKey,
    LockMode,
};
```

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p ob-poc --all-features
```
Expected: clean compile.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 5.

**E-invariant:** 10 zero-consumer expansion types no longer reachable via `dsl_v2::` root path. 7 consumed types remain.

---

### Phase 5 of 10: Remove zero-consumer `macros` re-exports from `dsl_v2` root (Progress: 40%)

**Pre-conditions:** Phase 4 passed.

**Pre-edit verification:**
```bash
# Verify that no consumer uses dsl_v2::MacroRegistry (the root path)
rg 'dsl_v2::MacroRegistry[^a-zA-Z_]' --type rust -g '!rust/src/dsl_v2/*'
# Verify all consumers use dsl_v2::macros::MacroRegistry (the deep path)
rg 'dsl_v2::macros::MacroRegistry' --type rust -g '!rust/src/dsl_v2/*'
```
Expected: 0 matches for root path, 12+ matches for deep path.
If root path has matches: STOP, report, do not proceed.

```bash
# Verify each item has zero consumers via root
for item in expand_macro expand_macro_fixpoint MacroSchema MacroExpansionOutput MacroExpansionStep MacroExpansionError MacroKind MacroTarget MacroArg MacroArgType MacroArgs MacroPrereq MacroRouting MacroUi ArgStyle ExpansionLimits FixpointExpansionOutput SetState VerbCallStep; do
  rg "dsl_v2::${item}[^a-zA-Z_]" --type rust -g '!rust/src/dsl_v2/*'
done
```
Expected: 0 matches for each.
If any match found: STOP, report which item, do not proceed.

**Change:**
- File: `rust/src/dsl_v2/mod.rs`
- Lines 176-182: Replace the `macros` re-export block.

Current (lines 176-182):
```rust
// Re-export macro expansion types
pub use macros::{
    expand_macro, expand_macro_fixpoint, load_macro_registry, load_macro_registry_from_dir,
    ArgStyle, ExpansionLimits, FixpointExpansionOutput, MacroArg, MacroArgType, MacroArgs,
    MacroExpansionError, MacroExpansionOutput, MacroExpansionStep, MacroKind, MacroPrereq,
    MacroRegistry, MacroRouting, MacroSchema, MacroTarget, MacroUi, SetState, VerbCallStep,
};
```

Replace with:
```rust
// Re-export macro expansion types (consumed externally)
pub use macros::{load_macro_registry, load_macro_registry_from_dir, MacroRegistry};
```

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p ob-poc --all-features
```
Expected: clean compile.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 6.

**E-invariant:** 19 zero-consumer macro types no longer reachable via `dsl_v2::` root path. 3 consumed items remain.

---

### Phase 6 of 10: Remove zero-consumer `entity_deps` re-export from `dsl_v2` root (Progress: 48%)

**Pre-conditions:** Phase 5 passed.

**Pre-edit verification:**
```bash
rg 'dsl_v2::init_entity_deps' --type rust -g '!rust/src/dsl_v2/*'
```
Expected: 0 matches.
If any match found: STOP, report, do not proceed.

**Change:**
- File: `rust/src/dsl_v2/mod.rs`
- Lines 139-140:

Current:
```rust
#[cfg(feature = "database")]
pub use entity_deps::init_entity_deps;
```

Delete both lines entirely.

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p ob-poc --all-features
```
Expected: clean compile.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 7.

**E-invariant:** `init_entity_deps` no longer reachable via `dsl_v2::` root path.

---

### Phase 7 of 10: `sem_os_core` internal functions → `pub(crate)` (Progress: 56%)

**Pre-conditions:** Phase 6 passed.

**Pre-edit verification:**
```bash
rg 'VerbFilterContext' --type rust -g '!rust/crates/sem_os_core/*'
rg 'rank_views_by_overlap' --type rust -g '!rust/crates/sem_os_core/*'
rg 'compute_view_overlap' --type rust -g '!rust/crates/sem_os_core/*'
rg 'filter_and_rank_verbs' --type rust -g '!rust/crates/sem_os_core/*'
```
Expected: 0 matches for all four.
If any match found: STOP, report which item, do not proceed.

**Change 7a:**
- File: `rust/crates/sem_os_core/src/context_resolution.rs`
- Line 730: `pub struct VerbFilterContext<'a> {`
- Edit: `pub struct VerbFilterContext<'a> {` → `pub(crate) struct VerbFilterContext<'a> {`

**Change 7b:**
- Same file
- Line 750: `pub fn rank_views_by_overlap(`
- Edit: `pub fn rank_views_by_overlap(` → `pub(crate) fn rank_views_by_overlap(`

**Change 7c:**
- Same file
- Line 773: `pub fn compute_view_overlap(`
- Edit: `pub fn compute_view_overlap(` → `pub(crate) fn compute_view_overlap(`

**Change 7d:**
- Same file
- Line 820: `pub fn filter_and_rank_verbs(`
- Edit: `pub fn filter_and_rank_verbs(` → `pub(crate) fn filter_and_rank_verbs(`

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p sem_os_core --all-features
```
Expected: clean compile.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 8.

**E-invariant:** Four internal resolution-pipeline items no longer reachable outside `sem_os_core`.

---

### Phase 8 of 10: Replace `pub use types::*` in `dsl-core` `config/mod.rs` (Progress: 72%)

**Pre-conditions:** Phase 7 passed.

**Rationale:** `dsl-core::config::types` contains 54 public types. The wildcard `pub use types::*` in `config/mod.rs` re-exports all 54 at the `dsl_core::config::` path. This phase replaces the wildcard with an explicit allowlist of the 43 contract types. The remaining 11 types stay `pub` on their definitions (required because they're embedded in public parent structs) but are no longer re-exported — consumers must use the full `dsl_core::config::types::PolicyConfig` path.

**Pre-edit verification:**
```bash
# Confirm the wildcard exists
rg 'pub use types::\*' rust/crates/dsl-core/src/config/mod.rs
```
Expected: 1 match at line 29.

**Change:**
- File: `rust/crates/dsl-core/src/config/mod.rs`
- Line 29: `pub use types::*;`

Replace with:
```rust
pub use types::{
    // Top-level config containers
    VerbsConfig, CsgRulesConfig, DomainConfig,
    // Verb definition contract
    VerbConfig, VerbOutputConfig, VerbBehavior, VerbMetadata, VerbSentences,
    ConfirmPolicyConfig, ReturnsConfig, ReturnTypeConfig,
    // Verb metadata enums
    VerbTier, SourceOfTruth, VerbScope, VerbStatus, HarmClass, ActionClass,
    // Argument definition contract
    ArgConfig, ArgType, ArgValidation, SlotType, FuzzyCheckConfig,
    LookupConfig, SearchKeyConfig, ResolutionMode,
    // Dataflow contract
    VerbProduces, VerbConsumes, VerbLifecycle,
    // CRUD config
    CrudConfig, CrudOperation,
    // Graph query config
    GraphQueryConfig, GraphQueryOperation,
    // Durable workflow config
    DurableConfig, DurableRuntime,
    // CSG rule types
    ConstraintRule, WarningRule, JurisdictionRule, CompositeRule,
    RuleCondition, RuleRequirement, JurisdictionCondition, AppliesTo, RuleSeverity,
};
```

**Types deliberately excluded from re-export** (accessible via `dsl_core::config::types::` if needed):
- `PolicyConfig`, `BatchPolicyConfig`, `LockingConfig`, `LockModeConfig`, `LockTargetConfig`, `LockAccessConfig` — internal DAG resource dependency types, consumed only by `dsl_v2::runtime_registry`
- `CompositeSearchKey`, `SearchDiscriminator`, `ResolutionTier` — internal s-expression compiler types
- `DynamicVerbConfig`, `DynamicSourceConfig` — legacy dynamic verb generation

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check -p dsl-core --all-features
cd rust && env RUSTC_WRAPPER= cargo check --workspace --all-features
```
Expected: clean compile on both. The 11 excluded types are still `pub` on their struct definitions and accessible via `dsl_core::config::types::*` — only the shortcut re-export at `dsl_core::config::*` is removed.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 9.

**E-invariant:** `dsl_core::config::` re-exports exactly 43 contract types. 11 internal types are no longer at the `config::` shortcut path.

---

### Phase 9 of 10: Replace `pub use config::types::*` in `dsl-core` `lib.rs` (Progress: 88%)

**Pre-conditions:** Phase 8 passed.

**Rationale:** `dsl-core/src/lib.rs` line 35 does `pub use config::types::*`, dumping all 54 config types at the crate root (`dsl_core::VerbConfig`). After Phase 8, this wildcard picks up only the 43 types from the explicit `config/mod.rs` re-export, plus the 11 types still directly in `types` module. Replace with a curated set of the most-used types at the crate root. Less-used types remain accessible via `dsl_core::config::VerbMetadata` etc.

**Pre-edit verification:**
```bash
rg "pub use config::types::\*" rust/crates/dsl-core/src/lib.rs
```
Expected: 1 match at line 35.

**Change:**
- File: `rust/crates/dsl-core/src/lib.rs`
- Line 35: `pub use config::types::*;`

Replace with:
```rust
pub use config::types::{
    VerbsConfig, DomainConfig, VerbConfig, ArgConfig, ArgType, VerbBehavior, VerbMetadata,
    VerbProduces, VerbConsumes, VerbLifecycle, LookupConfig, SearchKeyConfig,
    CrudConfig, CrudOperation, ReturnsConfig, ReturnTypeConfig, VerbOutputConfig,
};
```

This curated set covers the types used by 90%+ of external consumers (37 files). Types like `SlotType`, `FuzzyCheckConfig`, `ConfirmPolicyConfig`, CSG rule types, graph/durable configs remain accessible via `dsl_core::config::SlotType` etc.

**Post-edit verification:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check --workspace --all-features
```
Expected: clean compile. All consumers that import via `dsl_core::config::*` or `dsl_core::config::types::*` or the full module path are unaffected — only the `dsl_core::TypeName` root shortcut is narrowed.
If broken: revert, report, do not proceed.

**Gate:** Do NOT commit. → IMMEDIATELY proceed to Phase 10.

**E-invariant:** `dsl_core::` root re-exports exactly 17 curated config types. All 54 types remain accessible via `dsl_core::config::types::*` for consumers that need the full set.

---

### Phase 10 of 10: Consolidated Verification (Progress: 100%)

**Pre-conditions:** All phases (1-9) passed individually.

**Verification command:**
```bash
cd rust && env RUSTC_WRAPPER= cargo check --workspace --all-features
```
Expected: clean compile.

**Report:**
- Count of items changed per crate:
  - `sem_os_postgres`: 1 (sqlx_types module visibility)
  - `ob-poc` (events): 1 (test_emitter cfg gate)
  - `ob-poc` (repl): 1 (insert_test_entry cfg gate)
  - `ob-poc` (dsl_v2): 3 phases (expansion, macros, entity_deps re-export trimming)
  - `sem_os_core`: 4 items in context_resolution.rs
  - `dsl-core`: 2 files (config/mod.rs wildcard → explicit, lib.rs wildcard → curated)
- Before/after `pub use` counts for modified files (run `rg '^pub use' <file> | wc -l` before and after)
- List any warnings introduced

**Gate:** Do NOT commit. STOP. Report results to architect. Tier A complete.

**E-invariant:** Workspace compiles cleanly. 9 change phases applied, zero functional changes. All changes are uncommitted.

---

## Summary

| Phase | Target | Change | Crate |
|-------|--------|--------|-------|
| 1 | `sqlx_types` | `pub mod` → `pub(crate) mod` | sem_os_postgres |
| 2 | `test_emitter()` | add `#[cfg(test)]` | ob-poc |
| 3 | `insert_test_entry()` | add `#[cfg(test)]` | ob-poc |
| 4 | 10 expansion re-exports | remove from root `pub use` | ob-poc (dsl_v2) |
| 5 | 19 macros re-exports | remove from root `pub use` | ob-poc (dsl_v2) |
| 6 | `init_entity_deps` re-export | delete 2 lines | ob-poc (dsl_v2) |
| 7 | `VerbFilterContext`, `rank_views_by_overlap`, `compute_view_overlap`, `filter_and_rank_verbs` | `pub` → `pub(crate)` | sem_os_core |
| 8 | `config/mod.rs` wildcard | `pub use types::*` → explicit 43-type allowlist | dsl-core |
| 9 | `lib.rs` wildcard | `pub use config::types::*` → curated 17-type root set | dsl-core |
| 10 | — | consolidated verification | workspace |

Total: 9 change phases + 1 verification phase. 36 items tightened across 4 crates. 2 wildcard re-exports replaced with explicit allowlists.

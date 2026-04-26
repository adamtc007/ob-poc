# Architect Decisions: sem_os_core Role & dsl_v2 Seam Strategy

**Date:** 2026-04-16
**Audience:** Lead architect (not Codex)
**Purpose:** Two architectural decisions that gate further cleanup work. Neither is executable by Codex without human design work.
**Context:** `docs/reviews/opus_second_pub_api_surface_followup.md` has the verified evidence.

---

## Decision 1: `sem_os_core` — capability kernel vs shared domain model

### What this decides

Whether `sem_os_core`'s business logic modules remain `pub` (shared domain model: everything is intentionally accessible) or become `pub(crate)` (capability kernel: only ports + body types are the external contract).

This decision also governs `sem_os_core::gates` and `sem_os_core::security`, which have zero external consumers today but may be intentionally public under the domain-model interpretation.

### Affected modules

| Module | External consumer count | Consumers |
|--------|------------------------|-----------|
| `gates` | 0 | — |
| `security` | 0 | — |
| `context_resolution` | 14 files | harness, orchestrator, envelope, calibration, sem_reg, tests, xtask |
| `abac` | 13 files | orchestrator, envelope, calibration, sem_reg, tests, xtask |
| `enforce` | 1 file | `sem_reg/enforce.rs` |
| `grounding` | 2 files | `sage/valid_verb_set.rs`, `sem_os_runtime/constellation_runtime.rs` |
| `stewardship` | 2 files | `api/observatory_routes.rs`, `sem_reg/stewardship/types.rs` |

### Option A: Capability Kernel

Restrict logic modules to `pub(crate)`. Only port traits, body types, proto DTOs, seeds, and principal remain public.

**Consequence:** ~32 files need import path changes. But this is NOT a mechanical migration — it requires designing which types from `context_resolution` and `abac` belong in `proto` (the external boundary) vs. stay internal. For example, `ActorContext` from `abac` is consumed by 13 files and would need re-exporting via `proto`. `ContextResolutionRequest`/`Response` are already in `proto` but many intermediate types (e.g., `DiscoverySurface`, `RankedUniverse`, `GroundedActionSurface`) are consumed directly from `context_resolution`.

**This is design work, not a branch pick.** If you choose Option A, the next step is designing the `proto` facade — listing exactly which types move to `proto`, which stay internal, and which get new contract-oriented wrappers. Only after that design is done can mechanical execution begin.

**Unlocks:** A subsequent Codex plan for mechanical migration (after facade design).

### Option B: Shared Domain Model

All modules stay `pub`. Document the intent with a crate-level doc comment:

```rust
//! sem_os_core — Canonical domain types, port traits, and business logic
//! for the Semantic OS.
//!
//! All modules in this crate are intentionally public as shared domain
//! vocabulary. Consumers may import from any module.
```

For `gates` and `security` (zero consumers): add `#[doc(hidden)]` to keep them out of rustdoc while remaining technically accessible.

**Consequence:** No code changes beyond documentation. The 486-item surface stays as-is. Future consumers may depend on any module.

**Unlocks:** Nothing further needed.

### Recommendation

Neither option is wrong. Option B is lower risk and reflects the crate's current usage pattern. Option A is architecturally cleaner but requires design work that has not been done. I would lean B for now, with `#[doc(hidden)]` on `gates` and `security`, and revisit A if/when a Java port forces the question.

---

## Decision 2: `dsl_v2` seam exclusivity — exclusive vs additive prelude

### What this decides

Whether the 4 seam modules (`syntax`, `planning`, `execution`, `tooling`) are the ONLY access path for DSL types, or whether a curated root prelude coexists alongside them.

Tier A (Codex plan) has already removed the re-exports from `pub(crate)` modules (`expansion`, `macros`, `entity_deps`). This decision is about the remaining root-level re-exports from `pub` modules and `dsl-core`.

### Current state after Tier A

After Tier A execution, the `dsl_v2` root will still have:
- 7 `pub use dsl_core::` module re-exports (e.g., `pub use dsl_core::ast;`)
- 10 `pub use dsl_core::{items}` type re-exports
- 20 `pub mod` local module declarations
- 15 `pub use` from local public modules (e.g., `pub use enrichment::*`)
- 4 seam module definitions (`syntax`, `planning`, `execution`, `tooling`)

### Option A: Exclusive Seams

Remove all root-level re-exports. Consumers must use seam paths (`dsl_v2::syntax::parse_program`) or go directly to `dsl_core::` for core types.

**Consumer migration cost:**

| Current root path | New path | Consumer count |
|-------------------|----------|---------------|
| `dsl_v2::parse_program` | `dsl_v2::syntax::parse_program` | ~5 |
| `dsl_v2::AstNode` | `dsl_v2::syntax::AstNode` | ~3 |
| `dsl_v2::Program` | `dsl_v2::syntax::Program` | ~2 |
| `dsl_v2::Statement` | `dsl_v2::syntax::Statement` | ~3 |
| `dsl_v2::VerbCall` | `dsl_v2::syntax::VerbCall` | ~4 |
| `dsl_v2::BindingContext` | `dsl_v2::syntax::BindingContext` | ~2 |
| `dsl_v2::ConfigLoader` | `dsl_core::config::ConfigLoader` | ~3 |

Also remove from main crate `lib.rs` (lines 212-218):
```rust
pub use dsl_v2::execution::{DslExecutor, ExecutionContext, ...};
pub use dsl_v2::{parse_program, parse_single_verb, Argument, AstNode, ...};
```

**Total:** ~25 import path changes. This IS mechanical after the decision is made — Codex can execute it with a migration table.

**Also needed (Tier C):** Move `batch_executor`, `graph_executor`, `sheet_executor`, `enrichment`, `submission`, `execution_result` behind appropriate seams. This is item relocation, not visibility, so it's design work.

### Option B: Curated Additive Prelude

Keep a minimal root prelude of the 8-10 most-used types. Remove everything else from root.

```rust
// Curated prelude — most common entry points
pub use dsl_core::parser::parse_program;
pub use dsl_core::ast::{AstNode, Program, Statement, VerbCall, Argument, Literal, Span};
pub use dsl_core::binding_context::BindingContext;
pub use dsl_core::config::ConfigLoader;
```

Remove all other root-level `pub use` statements. Items remain accessible via their module paths (`dsl_v2::enrichment::enrich_program` etc.) — only the root shortcut is removed.

**Migration cost:** Lower than Option A. ~15 import path changes for removed convenience re-exports.

### Recommendation

Option B is the pragmatic choice. It preserves the convenience imports that 90% of consumers use while removing the long tail of rarely-used root re-exports. Option A is cleaner but the `lib.rs` root re-export removal has downstream effects on the main crate's own public surface that need careful thought.

After deciding, the mechanical migration (updating import paths) can be given to Codex as a follow-up plan.

---

*Both decisions are independent. You can pick A on one and B on the other.*

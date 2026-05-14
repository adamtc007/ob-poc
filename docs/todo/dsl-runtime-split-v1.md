# dsl-runtime split v1

> **Status:** DRAFT — awaiting Adam review before any execution.
> **Author:** generated from second-review audit (`~/.claude/plans/i-want-a-second-frolicking-wozniak.md`).
> **Posture:** This is the same shape as `capability-crate-restructure-v1.md`, applied to `dsl-runtime` (which v1 did not touch). The audit identified `dsl-runtime` as the workspace's largest dumping ground at 29,667 LOC across 26 top-level modules.

---

## 1. Discipline

Same rule as v1 §1: a slice that moves code without a capability claim drifts. Here the claim is concrete and falsifiable:

- The data plane is **what runs a verb against a database inside a transaction**.
- The analyser plane is **what reads DSL text + verb metadata and produces diagnostics + an executable plan** — without touching a database.

Modules whose primary deps are `sqlx`, `sem_os_core::verb_contract`, the service registry, or a `TransactionScope` are data-plane. Modules whose primary deps are `dsl_core::ast / parser / compiler / dag / diagnostics`, a verb registry, or an entity-ref resolver are analyser-plane.

That cut is observable in the import graph (see §3) and survives the discipline that v1 codified: `unreachable_pub = "deny"` + no `pub use module::*` re-exports.

---

## 1.5 Why not a 3-way split (runtime / registry / analysis)?

The audit floated a 3-way carve. After tracing imports, the 3-way isn't worth the seam cost in v1 of this restructure:

- `crud_executor` does **not** consume the dsl-runtime verb registry. It reaches `sem_os_core::verb_contract::VerbContractBody` directly. So the registry is not shared between data plane and analyser plane.
- The dsl-runtime registry (`verb_registry`, `runtime_registry`, `catalogue_loader`) exists to support **dsl-lsp completion / hover / signature** — i.e. it is analyser-plane infrastructure, full stop.
- Macros likewise — `macros::{schema, loader, conditions, variable, scope}` is the registry side of macros, used by the analyser; the **expansion engine** is the part that touches sessions and lives in `ob-poc`. The analyser-tier subset is what moves.

Conclusion: registry belongs with analysis. If, at v2 review, a third consumer of the registry appears (e.g. a remote MCP planner), splitting registry out is mechanical — every module in this plan tagged "registry" is already a leaf at the analyser tier.

---

## 2. Target capability map

### 2.1 `dsl-runtime` (after) — the execution plane

> **Charter:** Runs verbs inside a `TransactionScope`. Owns `VerbExecutionPort`, `VerbExecutionContext`, `VerbExecutionOutcome`, `CrudExecutionPort`, the metadata-CRUD interpreter, the service registry, and the Pattern A domain ops registered via `extend_registry()`.
>
> **Anti-charter:** No DSL text parsing. No analyser diagnostics. No verb registry beyond the contract metadata it consumes from `sem_os_core::verb_contract`. No LSP/IDE concerns.

Module list (13):

| Module | Why it stays |
|---|---|
| `execution` | `VerbExecutionContext/Outcome/Result/SideEffects` — data-plane primitives |
| `port` | `VerbExecutionPort`, `CrudExecutionPort` traits |
| `crud_executor` | `PgCrudExecutor` — sqlx-backed CRUD interpreter |
| `tx` | `TransactionScope` |
| `services` | `ServiceRegistry`, typed service dispatch |
| `service_traits/` | Service trait definitions consumed by Pattern A ops |
| `state_reducer/` | State reduction during execution (nom-based eval-expression) |
| `domain_ops/` | Pattern A ops (registered via `build_registry()`) |
| `bods/` | BODS-specific execution ops |
| `placeholder/` | Placeholder resolution at execution time |
| `cross_workspace/` | Cross-workspace state coordination at execution |
| `document_bundles/` | Document workflow ops |
| `document_requirements/` | Document workflow ops |

### 2.2 `dsl-analysis` (new) — the analyser plane

> **Charter:** Reads DSL text + verb metadata and produces diagnostics, suggestions, and executable plans. Hosts the verb registry, the macro registry, ref/gateway resolvers, the LSP-facing semantic validator, and the analyse-and-plan orchestrator. Zero `sqlx`. Zero execution-port deps.
>
> **Anti-charter:** Does not execute. Does not hold a `PgPool`. Does not implement `VerbExecutionPort`. Does not own runtime services or Pattern A ops.

Module list (13):

| Module | Why it moves here |
|---|---|
| `validation` | Pure types (920 LOC, zero internal refs): `Diagnostic`, `Severity`, `SourceSpan`, `ValidationContext`, `ValidationResult` |
| `verb_registry` | `UnifiedVerbDef`, `find_unified_verb`, `registry()` — YAML-loaded analyser registry |
| `runtime_registry` | YAML-driven registry powering dsl-lsp completion / hover / signature |
| `catalogue_loader` | Loads the catalogue from YAML for the analyser registry |
| `macros` | Macro registry subset: schema + loader + conditions + variable + scope (NOT the expander, which stays in `ob-poc`) |
| `suggestions` | Frontier-derived "what verb next" — pure-Rust over the registry |
| `ref_resolver` | DSL ref resolver |
| `gateway_resolver` | Resolves refs via `EntityGateway` |
| `lsp_validator` | LSP-facing semantic validator; consumes `ref_resolver` + `gateway_resolver` + `validation` + `verb_registry` |
| `planning_facade` | `analyse_and_plan`: parser → compiler → DAG → diagnostics + plan |
| `stategraph` | State graph used by DAG planning; reaches `dsl_core::config` for verb metadata |
| `verification/` | Plan-verification logic |
| `entity_kind` | Entity-kind classification used during validation (decision-point: confirm at slice time it has no data-plane callers before final move) |

### 2.3 Tightened dependency graph

```
ob-poc (composition)
  ├─→ dsl-runtime (execution)
  └─→ dsl-analysis (analyser)
         ↓
       dsl-core (parser, AST, compiler, DAG primitives)
       ob-templates (template loader)
       ob-poc-types (cross-capability DTOs)

dsl-runtime → sem_os_core (transitional, see v0.3 §7)
              dsl-core (ops, ast for executor input)
              entity-gateway
              sqlx, sem_os_core, governed_query_proc
```

**Critical invariant:** no `dsl-analysis → dsl-runtime` edge. The orchestrator in `ob-poc` produces a plan via `dsl-analysis` and hands it to `dsl-runtime` for execution. The plan type itself stays in `ob-poc-types` (cross-capability DTO, two consumers — v1 §6.5 rule satisfied).

---

## 3. Drift census (current dsl-runtime, 26 modules)

Per `lib.rs` annotations + import-graph sampling (2026-05-14):

**Data-plane (13) — stays:**
`execution`, `port`, `crud_executor`, `tx`, `services`, `service_traits/`, `state_reducer/`, `domain_ops/`, `bods/`, `placeholder/`, `cross_workspace/`, `document_bundles/`, `document_requirements/`

**Analyser-plane (13) — moves to `dsl-analysis`:**
`validation`, `verb_registry`, `runtime_registry`, `catalogue_loader`, `macros`, `suggestions`, `ref_resolver`, `gateway_resolver`, `lsp_validator`, `planning_facade`, `stategraph`, `verification/`, `entity_kind` (decision-point)

**No straddlers identified.** Sampling `planning_facade` (uses `runtime_registry` + `dsl_core` only) and `crud_executor` (uses `sem_os_core::verb_contract` + sqlx, never the registry) confirms the cut is clean.

**Risk: `cross_workspace/` has 18 intra-module references.** This is a single nested-module subtree, not coupling that crosses the planned cut — the references are between `cross_workspace::*` submodules. Confirmed by the per-module count showing all other top-level modules at ≤6 incoming references.

---

## 4. Migration plan

Same slice discipline as v1: compat re-export → `git mv` → tighten `pub(crate)`. Each phase landing requires `cargo build --workspace --all-features` + ob-poc test suite green.

### Phase 1 — Create `dsl-analysis` skeleton
Empty crate with charter / anti-charter doc, `unreachable_pub = "deny"`, `[dependencies]` minimal (dsl-core, ob-templates, ob-poc-types). Added to workspace members. Mirrors v1 Phase 1.

### Phase 2 — Move `validation` (pure types, 920 LOC, zero refs)
Easiest first — confirms the seam works. `git mv src/validation.rs → ../dsl-analysis/src/validation.rs`. Compat re-export `pub use dsl_analysis::validation;` in `dsl-runtime/src/lib.rs` for one phase.

### Phase 3 — Move the registry cluster
`verb_registry` + `runtime_registry` + `catalogue_loader`. These three are the analyser-registry surface that LSP consumes. Move as a paired set; they likely cross-reference.

### Phase 4 — Move `macros` (registry subset)
Schema + loader + conditions + variable + scope. The expander stays in `ob-poc` per the existing `lib.rs` note.

### Phase 5 — Move the resolver cluster
`ref_resolver` + `gateway_resolver`. Paired move — `gateway_resolver` consumes `ref_resolver`.

### Phase 6 — Move `lsp_validator`
Sits on top of the resolver cluster + registry + validation. Confirm all upstream deps are in `dsl-analysis` first.

### Phase 7 — Move `suggestions` + `planning_facade`
The analyser orchestrator. `planning_facade` consumes `runtime_registry`; suggestions is leaf-level.

### Phase 8 — Move `stategraph/` + `verification/`
DAG state graph and plan verification. Both are leaf-level analyser concerns.

### Phase 9 — Move/decide `entity_kind`
Slice-time confirm it has zero data-plane callers, then move. If it has a data-plane caller, leave it in dsl-runtime and add a re-export to dsl-analysis.

### Phase 10 — Tighten dsl-runtime
Remove now-unused deps from `Cargo.toml` (likely candidates: `regex`, `nom`, `serde_yaml` if no remaining consumer; `governed_query_proc` if no live `#[governed_query]` annotation). Tighten `pub` → `pub(crate)` on items that lost their cross-module consumers.

### Phase 11 — Compat-shim removal
Delete the `pub use dsl_analysis::*;` re-exports from `dsl-runtime/src/lib.rs`. Update any remaining downstream consumers (likely `dsl-lsp`, `ob-poc`).

### Phase 12 — Bed-in review (v2 of this plan)
Same discipline as v1 §9: after 2–4 weeks, audit whether anything drifted across the seam.

---

## 5. Decisions to lock before slicing

1. **`entity_kind` placement** — confirm at slice time (Phase 9) by grepping for callers. Default: dsl-analysis.
2. **The plan type's home** — the type produced by `planning_facade` and consumed by `dsl-runtime` execution. Default: `ob-poc-types::plan` (cross-capability DTO, ≥2 consumers per v1 §6.5).
3. **Macro expander stays in `ob-poc`** — already documented; do not move it during this restructure. The expander reaches `UnifiedSession` and `sem_os_obpoc_adapter`, neither of which is analyser-tier.
4. **No-edge invariant** — `dsl-analysis → dsl-runtime` is forbidden. If a slice can't land without that edge, stop and re-design.
5. **Transitional `sem_os_core` dep in `dsl-runtime` stays** — out of scope for this plan. Per `dsl-runtime/Cargo.toml:7-15`, removing it is a separate slice in three-plane v0.3.

---

## 6. What this plan does NOT do

- Does not split `dsl-runtime` further into runtime + registry. See §1.5 — folded into analyser instead.
- Does not touch `sem_os_core` (a separate dumping-ground concern; tracked in the audit as Tier 2 #6).
- Does not refactor `dsl-core`. `dsl-core` is on-charter today (parser/AST/compiler/DAG primitives, 8 consumers, clean).
- Does not address the latent A1 violation in `agent_ops` (tokio::process subprocess spawn) — that's Pattern-B-A1 ledger work, separate from this restructure.
- Does not change runtime behaviour. Every test that passed before must pass after — the cut is purely structural.

---

## 7. Decision record

| Date | Decision | Reason |
|---|---|---|
| 2026-05-14 | Draft authored. Two-way split (`dsl-runtime` / `dsl-analysis`) over three-way (registry separate). | §1.5 — registry has only analyser-plane consumers; a 3-way carve adds a seam without a second use case yet. |
| 2026-05-14 | Plan type → `ob-poc-types`. | v1 §6.5 rule (≥2 consumers across capability crates). |
| 2026-05-14 | Macro expander stays in `ob-poc`. | Reaches `UnifiedSession` + `sem_os_obpoc_adapter`. |

---

## 8. Verification

For each phase:

- `cargo check --workspace --all-features` green.
- `cargo test -p ob-poc --no-run --all-features` green.
- `cargo test --test runbook_e2e_test --test runbook_pipeline_test --test unified_pipeline_tollgates` green (these exercise the analyser→runtime handoff end-to-end).
- After Phase 11, `cargo tree -p dsl-runtime` shows no `dsl-analysis` dep, and `cargo tree -p dsl-analysis` shows no `dsl-runtime` dep.
- `unreachable_pub = "deny"` honoured in both crates throughout.

End-of-plan invariant check (Phase 12):

```bash
# No dsl-runtime → dsl-analysis edge
cd rust && cargo tree -p dsl-runtime | grep -c dsl-analysis  # → 0

# Both crates compile standalone
cargo check -p dsl-runtime --all-features
cargo check -p dsl-analysis --all-features

# No regression in size — split should be near-zero churn in total LOC
# Pre-split:  dsl-runtime ~29,667 LOC
# Post-split: dsl-runtime ~14k + dsl-analysis ~15k (target ranges)
```

---

## 9. Files to read first when starting Phase 1

- `rust/crates/dsl-runtime/Cargo.toml` — current dep declaration
- `rust/crates/dsl-runtime/src/lib.rs` — module list + annotation comments
- `docs/todo/capability-crate-restructure-v1.md` §5 — Phase R/1 ceremony
- `docs/backlog/three-plane-architecture-v0.3.md` §7.1 — three-plane scope contract this plan operates within

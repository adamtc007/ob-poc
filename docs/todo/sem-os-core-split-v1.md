# sem_os_core split v1

> **Status:** DRAFT — awaiting Adam review.
> **Author:** generated from second-review audit + dsl-runtime-split v1 lessons.
> **Posture:** The audit (Tier 2 #6) identified `sem_os_core` as the second-largest dumping ground in the workspace at 26,050 LOC across 45 top-level modules. The dsl-runtime-split v1 migration just landed cleanly (8 commits, no test regressions, no cross-edges) and gives a proven slicing pattern to repeat.

---

## 1. Discipline

Same rule as dsl-runtime-split §1: a slice that moves code without a capability claim drifts. Here the capability claim is **three concerns living together that should not be**:

- **Foundation types and engine primitives** — what every other SemOS crate needs to know to compile. Pure `serde`/`uuid`/`chrono` DTOs + small port traits. No business logic.
- **Ontology vocabulary** — the `*_def` family. Authored YAML shapes for attribute / entity-type / relationship / document-type / state-graph / verb-contract / etc. Definitions of *what exists in the world*.
- **Policy, governance, and projection** — `abac`, `enforce`, `acp_projection`, `affinity`, `diagram`, `observatory`, `stewardship`, `state_simulation`, `authoring`, etc. Definitions of *what may happen* and *what is shown*.

Modules whose primary deps are `serde`/`chrono`/`uuid` and no other SemOS module are foundation. Modules whose name ends in `_def` and whose only crate-internal dep is `types` are ontology. Modules that reach `abac`, `policy_rule`, `enforce`, projection, or observation are policy/governance.

That cut is observable in the import graph and survives the discipline that dsl-runtime-split v1 codified: `unreachable_pub = "deny"` + no `pub use module::*` re-exports + Phase 11 invariant (no cross-edges in either direction).

---

## 1.5 Why three crates, not two

In dsl-runtime-split v1, the audit's three-way carve was folded into two because the registry was consumed only by the analyser plane. Here that argument doesn't hold:

- **`sem_os_postgres`**, **`sem_os_server`**, **`sem_os_client`** all consume the ontology vocabulary (they store/serve/proxy `*_def` bodies).
- **`ob-poc-boundary`** consumes the ontology vocabulary AND the policy projection (ACP projection, classification limits) but not the engine internals.
- **`dsl-analysis`** consumes ontology (`state_graph_def`) but not policy.
- **`dsl-runtime`** consumes engine primitives (`Principal`, `SemOsError`, `verb_contract`) but not policy or ontology beyond verb contracts.

Each tier has at least two independent consumer crates, so each tier is a genuine seam. A 2-way carve would either dump ontology into the engine (defeating the point — that's the current state) or fold engine into policy (which would force every consumer of `Principal` to also pull in `abac` + `observatory` + half the workspace, the opposite of what we want).

Three crates land cleanly. The dep graph stays acyclic and matches consumer needs.

---

## 2. Target capability map

### 2.0 `sem_os_types` (new bottom tier, added 2026-05-14)

> **Charter:** The shared vocabulary every SemOS-touching crate needs to *talk about* the registry. Classifications, evidence grades, security labels, object types, snapshot row shape, attribute visibility, governance tier. Zero workspace deps; depends only on `chrono`, `serde`, `strum`, `uuid`.
>
> **Anti-charter:** No traits beyond derive macros. No business logic. No async. If a future addition needs any other workspace crate, it belongs in `sem_os_core` (engine) or `sem_os_ontology` (vocabulary), not here.
>
> **Why this tier exists** — the original ADR placed `types` in core, but Phase 3 surfaced a real cycle: ontology `*_def` bodies legitimately need types (`AttributeVisibility`, `EvidenceGrade`, `Classification`), forcing `sem_os_ontology → sem_os_core` (Cargo); meanwhile the migration-time compat re-exports in `sem_os_core` already force `sem_os_core → sem_os_ontology`. Cargo rejects the cycle. Extracting `types` to a tiny bottom crate that everything depends on resolves it cleanly: types is *foundation under everything*, not engine-specific. (See §8 decision record.)

Module list (1, ~652 LOC):

| Module | What it provides |
|---|---|
| `lib.rs` (was `sem_os_core::types`) | `GovernanceTier`, `AttributeVisibility`, `Classification`, `EvidenceGrade`, `SecurityLabel`, `ObjectType`, `SnapshotRow`, plus any sibling pure-vocabulary enums in that file. |

### 2.1 `sem-os-core` (after) — engine + foundation

> **Charter:** Foundation types, type-safe IDs, error type, principal/role primitives, port traits for store backends, the execution-context placeholder, seed-data DTOs, and the protobuf-generated wire types. Every other `sem-os-*` crate (and the `ob-poc-*` crates that touch SemOS) depends on this.
>
> **Anti-charter:** No business-logic helpers. No registry mutation. No projection. No policy enforcement. No YAML loading beyond seed data. No async machinery beyond the bare port trait shapes. If a module's job is to *define* something the world contains, it's ontology, not core. If a module's job is to *decide* something, it's policy, not core.

Module list (8 after `types` moves to §2.0):

| Module | Why it stays |
|---|---|
| `error` | `SemOsError` — every crate needs to import this |
| `ids` | type-safe ID newtypes (`AttributeId`, `EntityId`, …) |
| `principal` | `Principal` (actor + role) |
| `service` | service trait surface (2,212 LOC — needs review at slice time to confirm it doesn't reach into policy/projection; if it does, that part splits to policy) |
| `ports` | port traits for store implementations |
| `execution` | execution-context placeholder (per dsl-runtime-split history, this is mostly a stub now) |
| `seeds` | seed-data DTOs |
| `proto` | prost-generated types + `build.rs` |
| `types` (compat shim) | `pub use sem_os_types::*;` — preserves `sem_os_core::types::*` paths for the duration of migration; can drop in Phase 12 if no consumer reaches `sem_os_core::types::*` directly anymore. |

### 2.2 `sem-os-ontology` (new) — the `*_def` vocabulary

> **Charter:** Definition bodies for everything SemOS catalogues — the authored YAML shape of attributes, entity types, document types, relationship types, state graphs, taxonomies, verbs, views, requirement profiles, evidence strategies, observations, etc. Plus the snapshot-body wrapper that registry storage round-trips through.
>
> **Anti-charter:** No ABAC. No projection. No gate evaluation. No registry mutation. Pure data-with-validation. May depend on `sem-os-core`. MUST NOT depend on `sem-os-policy`.

Module list (~21 — `*_def` family + paired body types):

| Module | Notes |
|---|---|
| `attribute_def` | (uses `types::{AttributeVisibility, EvidenceGrade}`) |
| `constellation_family_def` | currently `pub(crate)` — promote to `pub` for cross-crate use |
| `constellation_map_def` | |
| `derivation` | paired with `derivation_spec` |
| `derivation_spec` | |
| `document_type_def` | |
| `entity_type_def` | |
| `evidence` | evidence DTOs paired with `evidence_strategy_def` |
| `evidence_strategy_def` | |
| `macro_def` | currently `pub(crate)` — promote |
| `membership` | membership DTOs |
| `observation_def` | |
| `policy_rule` | the `*_def` for rules — pure type, no enforcement code |
| `proof_obligation_def` | |
| `relationship_type_def` | |
| `requirement_profile_def` | |
| `service_resource_def` | |
| `state_graph_def` | (already a consumer in `dsl-analysis::stategraph`) |
| `state_machine_def` | |
| `taxonomy_def` | |
| `universe_def` | |
| `verb_contract` | (already a consumer in `dsl-runtime::crud_executor`) |
| `view_def` | |

### 2.3 `sem-os-policy` (new) — governance, projection, observation

> **Charter:** Everything that *decides* or *projects*. ABAC primitives, gate logic, policy enforcement, ACP discovery projection, the affinity graph, observatory orientation/projection, stewardship/authoring lifecycle, state simulation, context policy/resolution, security labels, grounding, diagram emission. Depends on `sem-os-core` for primitives and `sem-os-ontology` for the vocabulary it enforces against.
>
> **Anti-charter:** Does not define new ontology shapes. Does not hold a sqlx connection. (Boot-time YAML loading is still permitted under the same relaxed line dsl-analysis adopted — projection is data, not verb execution.)

Module list (~15):

| Module | Notes |
|---|---|
| `abac` | reaches `types::{Classification, EvidenceGrade, SecurityLabel}` — only core types, no ontology |
| `acp_projection` | reaches `domain_pack::ClassificationLimit` — paired with domain_pack |
| `affinity` | (subdir: builder, discovery, query) — affinity graph between SemOS objects |
| `authoring` | (subdir: ports, validate_stage1/2, bundle, metrics, canonical_hash, governance_verbs) — authoring lifecycle |
| `context_policy` | |
| `context_resolution` | 2,158 LOC — slice-time look-twice: confirm no engine-tier deps |
| `diagram` | (subdir: mermaid, enrichment) — mermaid diagram emission |
| `domain_pack` | 869 LOC — domain pack manifests; consumed by acp_projection and state_simulation |
| `enforce` | reaches `abac`, `attribute_def`, `types` — the policy enforcer |
| `gates` | currently `pub(crate)` (subdir: governance, technical, mod) — promote for cross-crate use |
| `grounding` | |
| `observatory` | (subdir: projection, orientation) — observatory projection |
| `security` | currently `pub(crate)` (666 LOC) — promote |
| `state_simulation` | reaches `domain_pack::{DomainPackManifest, DomainTransition}` |
| `stewardship` | (subdir: types, mod) — stewardship lifecycle |

### 2.4 Tightened dependency graph

```
ob-poc (composition)
sem_os_server, sem_os_postgres, sem_os_client, sem_os_obpoc_adapter,
sem_os_harness, sem_os_mcp, ob-poc-boundary, ob-poc-domain (already)
  ├─→ sem_os_policy
  │     ↓
  │   sem_os_ontology
  │     ↓
  ├─→ sem_os_core
  │     ↓
  └─→ sem_os_types
        ↓
      primitives only (serde, chrono, uuid, strum)

dsl-runtime → sem-os-core (transitional, existing — Principal/SemOsError/VerbContractBody)
              + sem-os-ontology (transitional — VerbContractBody is verb_contract)
dsl-analysis → sem-os-core + sem-os-ontology (state_graph_def already, verb_contract on add)
```

**Critical invariants:**
- No `sem-os-types → anything` edge. Types is the absolute bottom.
- No `sem-os-core → sem-os-ontology` edge in the END STATE. Core knows nothing about ontology. (Transitional `sem_os_core → sem_os_ontology` edge exists during migration via compat re-exports; removed in Phase 12.)
- No `sem-os-core → sem-os-policy` edge. Core knows nothing about policy.
- No `sem-os-ontology → sem-os-policy` edge. Ontology defines *what exists*; policy decides *what may happen*.

---

## 3. Drift census (current sem_os_core)

Counts taken 2026-05-14:

- 26,050 LOC across 45 top-level modules (counting top-level files + subdirs).
- 9 modules cleanly engine/foundation per §2.1.
- 21 modules in the `*_def` ontology family per §2.2.
- 15 modules in the policy/projection/observatory family per §2.3.

Three notable confirmations:
- `types.rs` (652 LOC) has **zero crate-internal imports** — pure foundation. Stays in core.
- `*_def` modules consistently import only `crate::types::*` (sampled `attribute_def`, `policy_rule`, `observation_def`, `verb_contract`). Confirms one-way `ontology → core` edge.
- Policy modules reach BOTH `types` AND `*_def` modules (sampled `enforce` uses `abac` + `attribute_def` + `types`). Confirms one-way `policy → ontology → core` chain.

No straddling modules found in the sample. Slice-time verification (Phase 2 of each move) re-runs the grep to catch anything the sample missed.

---

## 4. Migration plan

Same slice discipline as dsl-runtime-split v1: compat re-export → `git mv` → tighten `pub(crate)`. Each phase landing requires `cargo build --workspace --all-features` + ob-poc test suite green.

The blast radius is wider than dsl-runtime-split because there are ~10 external consumer crates (sem_os_postgres, sem_os_server, sem_os_client, sem_os_obpoc_adapter, sem_os_harness, sem_os_mcp, ob-poc-boundary, ob-poc-domain, ob-poc itself, dsl-runtime, dsl-analysis). Phase 11-equivalent cleanup will be the heavy lift.

### Phase 1 — Skeleton crates
Create empty `sem-os-ontology` and `sem-os-policy` with charter doc-comments, `unreachable_pub = "deny"`, minimal deps. Add to workspace members. Mirrors v1 Phase 1 × 2.

### Phase 2.5 — Extract `sem_os_types` (added 2026-05-14)
Move `sem_os_core/src/types.rs` (~652 LOC) into a new `sem_os_types/src/lib.rs`. Replace `sem_os_core/src/types.rs` with a one-line `pub use sem_os_types::*;` compat shim. Add `sem_os_types = { path = "../sem_os_types" }` as a dep of `sem_os_core` and `sem_os_ontology`. Unblocks Phase 3.

### Phase 2 — Pure-type ontology leaves
Move the `*_def` modules with zero or `types`-only crate-internal imports first. Likely batch: `policy_rule`, `observation_def`, `verb_contract`, `view_def`, `taxonomy_def`, `universe_def`, `requirement_profile_def`, `relationship_type_def`, `state_machine_def`. ~9 modules, all leaf-level.

### Phase 3 — Body-with-types ontology
The `*_def` modules that need `types::*`: `attribute_def`, `entity_type_def`, `document_type_def`, `proof_obligation_def`, `evidence_strategy_def`, `service_resource_def`, `state_graph_def`. Plus paired bodies: `evidence`, `derivation`, `derivation_spec`, `membership`. Plus the formerly `pub(crate)` modules `constellation_family_def`, `constellation_map_def`, `macro_def` (promoted to `pub`).

### Phase 4 — Ontology subdirs
Any ontology-tier subdirs (none identified at survey time — confirm at slice).

### Phase 5 — Policy leaves (no inter-policy deps)
`abac`, `policy_rule` already moved; `context_policy`, `grounding`, `security` (promoted from `pub(crate)`).

### Phase 6 — Policy middle layer
`domain_pack`, `context_resolution` (2,158 LOC — slice-time look-twice).

### Phase 7 — Projection cluster
`acp_projection`, `affinity/`, `diagram/`, `observatory/`. Each subdir moves as a unit.

### Phase 8 — Enforcement + simulation
`enforce`, `state_simulation`, `gates/` (promoted).

### Phase 9 — Authoring + stewardship
`authoring/`, `stewardship/`. Large subdirs (~3,000+ LOC combined) — confirm at slice.

### Phase 10 — `service.rs` decision
`service.rs` is 2,212 LOC and listed under §2.1 as engine. Slice-time look-twice: if it imports policy or ontology modules, split out the policy-touching parts. Worst case it stays in core and we revisit.

### Phase 11 — Tighten sem_os_core
Remove now-unused deps. Likely candidates after the moves: `sqlparser` (probably authoring-only), `sha2`/`hex` (authoring `canonical_hash`?), `serde_yaml` (probably observatory/policy-only). `prost`/`prost-build` stays in core (proto types).

### Phase 12 — Drop compat shim
Same as dsl-runtime-split v1 Phase 11. Update every external consumer's imports `sem_os_core::<moved>` → `sem_os_ontology::<moved>` / `sem_os_policy::<moved>`. Add direct deps to each consumer's Cargo.toml. Drop the transitional cross-edges. This is the heavy lift — preview by grepping for `sem_os_core::` consumers in each downstream crate before starting.

### Phase 13 — Bed-in review (v2 of this plan)
Same 2-4 week discipline as dsl-runtime-split's Phase 12.

---

## 5. Decisions to lock before slicing

1. **Promote 4 `pub(crate)` modules to `pub`** — `constellation_family_def`, `gates`, `macro_def`, `security`. They become cross-crate consumers after the split; promotion is necessary, not aspirational.
2. **`service.rs` and `context_resolution.rs` need slice-time investigation** — both are 2k+ LOC and could straddle. If they do, split internally before moving (extract policy parts out of service into a new module, for example).
3. **`prost`-generated types stay in core** — `proto` and the `build.rs` move together if at all; for v1 they stay in `sem-os-core`.
4. **`policy_rule` placed in ontology, not policy** — it's the *_def for rules. The `enforce` module is the policy-tier consumer of `policy_rule`. This matches dsl-runtime-split's "runtime_registry is metadata, not execution" calibration.
5. **No-edge invariants** — `core → ontology` forbidden; `core → policy` forbidden; `ontology → policy` forbidden. Compat re-exports allowed during migration only, removed in Phase 12.

---

## 6. What this plan does NOT do

- Does not change the dsl-runtime transitional dep on `sem_os_core` (separate three-plane v0.3 slice).
- Does not change protobuf generation (proto + build.rs stay in core).
- Does not split `service.rs` (2,212 LOC) preemptively — Phase 10 decides based on real imports.
- Does not touch the standalone `sem_os_server` HTTP layer.
- Does not address `unreachable_pub = "deny"` violations introduced by the split — those get fixed inline as they appear (same as v1).

---

## 7. Risk register (specific to sem_os_core)

| Risk | Mitigation |
|---|---|
| Wider blast radius than dsl-runtime-split — ~10 consumer crates vs ~4 | Plan Phase 12 cleanup as 3 sub-slices (sem_os_* crates first, ob-poc-* second, dsl-* third) |
| `context_resolution.rs` is 2,158 LOC and could straddle policy/ontology/engine | Slice 6.0 read-and-classify before moving; consider internal split first |
| `service.rs` is 2,212 LOC engine candidate but unverified | Phase 10 explicit slice-time look |
| The 4 `pub(crate)` promotions break some "intended private" invariant | Inspect each module before promoting — if it should stay internal-only, the consumer needs to move too |
| Phase 11 dep cleanup may surface deps that look unused but are actually used via `serde` derive macro paths | Run `cargo machete` (or equivalent) before declaring a dep unused |
| Adam's parallel work on `bpmn-lite` continues — same staging discipline as dsl-runtime-split | `git reset HEAD -- bpmn-lite/` before each stage; verify `git diff --cached --stat` |

---

## 8. Decision record

| Date | Decision | Reason |
|---|---|---|
| 2026-05-14 | Three-way split (core / ontology / policy) over two-way. | Each tier has ≥2 independent consumer crates; 2-way would force ontology back into core or engine into policy. |
| 2026-05-14 | `policy_rule` lives in ontology, not policy. | It's a `*_def` body — defines the shape of a rule. The enforcer (`enforce`) is the policy-tier consumer. |
| 2026-05-14 | `prost` proto + build.rs stay in core. | Generated types are foundation; moving them adds build-graph complexity for no charter benefit. |
| 2026-05-14 | `service.rs` placement deferred to Phase 10 slice-time read. | 2,212 LOC could plausibly belong to any tier; cheaper to inspect than to forecast. |
| 2026-05-14 | The 4 `pub(crate)` modules promote to `pub` at split time. | Cross-crate use is the whole point; promotion is part of the slice not a separate concern. |
| 2026-05-14 (mid-migration) | Extracted a 4th tier `sem_os_types` (Phase 2.5). | Phase 3 surfaced a Cargo cycle: ontology `*_def` bodies need `types::*`, forcing `sem_os_ontology → sem_os_core`; existing compat re-exports already force `sem_os_core → sem_os_ontology`. Extracting types to a bottom crate resolves the cycle. Original ADR's "types in core" miscategorised the file — types is foundation under everything, not engine-specific. |

---

## 9. Verification

For each phase:

- `cargo check --workspace --all-features` green.
- `cargo test -p ob-poc --no-run --all-features` compiles (and on Phase 12, full test run with `DATABASE_URL`).
- After Phase 12, `cargo tree -p sem-os-core` shows no `sem-os-ontology` or `sem-os-policy` dep; `cargo tree -p sem-os-ontology` shows no `sem-os-policy` dep.
- `unreachable_pub = "deny"` honoured in all three crates throughout.

End-of-plan invariant check (Phase 13):

```bash
# No core → ontology/policy edges
cd rust
cargo tree -p sem_os_core | grep -cE "sem.os.(ontology|policy)"  # → 0

# No ontology → policy edge
cargo tree -p sem_os_ontology | grep -c "sem.os.policy"  # → 0

# All three crates compile standalone
cargo check -p sem_os_core
cargo check -p sem_os_ontology
cargo check -p sem_os_policy
```

---

## 10. Files to read first when starting Phase 1

- `rust/crates/sem_os_core/Cargo.toml` — current dep declaration
- `rust/crates/sem_os_core/src/lib.rs` — module list + visibility map
- `rust/crates/sem_os_core/src/types.rs` — foundation; sets the shape every other module imports from
- `rust/crates/sem_os_core/src/service.rs` — 2,212 LOC engine candidate, look-twice
- `rust/crates/sem_os_core/src/context_resolution.rs` — 2,158 LOC potential straddler
- `docs/todo/dsl-runtime-split-v1.md` — the executed-and-validated template this plan repeats

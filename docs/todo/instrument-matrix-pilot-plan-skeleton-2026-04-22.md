# Instrument Matrix Pilot — Plan Skeleton (2026-04-22)

> **Status:** scaffold only. This is a framework for the full 7-section pilot plan
> described in `instrument-matrix-pilot-prompt.md`. Each section has stub content
> and pointers so the full execution is mechanical fill-in work.
> **Source prompt:** `/Users/adamtc007/Downloads/instrument-matrix-pilot-prompt.md`
> **Reference spec:** `docs/todo/catalogue-platform-refinement-v1_0.md` (v1.0 — confirmed present)
> **Workspace target:** Instrument Matrix (confirmed by Adam 2026-04-22)
> **Output location when executed:** `docs/todo/instrument-matrix-pilot-plan-2026-04-22.md`

---

## What this skeleton is and is not

**Is:** the structural frame. Section headings match the prompt. Each section has
(a) the scope line from the source prompt, (b) a "what to read" list of concrete
file paths from ob-poc, (c) a "what to produce" list of deliverable sub-artefacts,
and (d) stubs flagged `<<TO BE FILLED IN>>`.

**Is not:** the pilot plan. Executing this skeleton produces the pilot plan.
Reading this skeleton alone does not answer the prompt's 8 pilot questions.

---

## Surface inventory — reconnaissance snapshot

Captured 2026-04-22 to scope the pilot boundary. **This inventory is a skeleton
input, not a deliverable.** The full plan will verify each number and restructure.

| Surface | Location | Count / note |
|---|---|---|
| Workspace enum | `rust/src/repl/types_v2.rs:95` | `WorkspaceKind::InstrumentMatrix` (registry 197-214) |
| Pack | `rust/config/packs/instrument-matrix.yaml` | 210 `allowed_verbs`; workspaces = `[instrument_matrix, cbu]` |
| Verb YAMLs (primary) | `rust/config/verbs/{trading-profile,booking-principal,booking-location,cash-sweep,matrix-overlay}.yaml` + `rust/config/verbs/custody/*.yaml` | ~60 base-verbs across ~10 files (trading-profile 21, cbu 30, custody 6-13/file) |
| Macros | `rust/config/verb_schemas/macros/{instrument,mandate}.yaml` | 7+ asset-family macros + mandate lifecycle |
| Constellations | `rust/config/sem_os_seeds/constellation_maps/{instrument_workspace,instrument_template,trading_streetside}.yaml` | 1 / 6 / 11 slots; `trading_streetside` has a `trading_profile_lifecycle` state machine |
| Router hook | `rust/src/sequencer.rs:1619` + `rust/src/repl/session_v2.rs:1036` | Pattern-match on "instrument" / "matrix" / "trading" |
| Scenarios | `rust/config/scenario_index.yaml` | **0** instrument-matrix scenarios today |
| Custom op impls | `rust/src/domain_ops/trading_profile.rs` + `rust/crates/sem_os_postgres/src/ops/trading_matrix.rs` | 36 + 3 = 39 `SemOsVerbOp` impls |
| Migrations | `migrations/{020_trading_profile_materialization,129_trading_profile_two_stage}.sql` + `rust/migrations/{202412_trading_matrix_storage, 20260105_trading_view_config, 20260106_trading_profile_ast_migration, 20260331_trading_profile_templates}.sql` | 6 migrations; largest is `202412_trading_matrix_storage` (614 LOC) |
| Test fixtures | `rust/tests/fixtures/intent_test_utterances.toml` | 39 instrument/trading utterances |
| Cross-workspace leaks | pack `allowed_verbs` | **0** cbu.* / kyc.* / deal.* FQNs detected in the 210-verb set; `cbu` IS declared as a peer workspace — clarify before Section 1 |

**Boundary ambiguity flagged for Adam**: pack declares `workspaces: [instrument_matrix, cbu]`. The pilot must decide whether verbs routed under the `cbu` peer-workspace listing count as "in scope" for Instrument Matrix pilot or should be re-scoped to a future CBU pilot. This is the first Section 5 open question.

---

## Section 1 — Instrument Matrix codebase reading  `<<HEAVIEST — do first>>`

**Scope (from prompt):** enumerate verbs, DAG structure, cross-workspace leaks,
existing types/traits/plugins, test fixtures, internal docs, and anything that
contradicts v1.0's mental model of the Instrument Matrix region.

**What to read, in order:**

1. `catalogue-platform-refinement-v1_0.md` — **fully**, no skipping. Cite by
   principle (P1–P15) / tranche-phase / DoD-item number henceforth.
2. `rust/config/packs/instrument-matrix.yaml` — the 210-verb pack is the anchor
   list. Reconcile against the per-file counts above.
3. Every verb file listed in the surface table — record per-verb (FQN, file,
   declared domain, behaviour: crud / plugin / template).
4. `rust/src/repl/types_v2.rs` registry entry (197-214) + the journey router
   hook — the runtime's view of what belongs to this workspace.
5. Constellation YAMLs — these give the DAG taxonomy skeleton. Note which slots
   carry state machines and which are placeholder.
6. `rust/src/domain_ops/trading_profile.rs` + `sem_os_postgres/src/ops/trading_matrix.rs`
   — every `SemOsVerbOp` impl. For each: does it expose `state_effect` today? Or
   would v1.0's three-axis schema need a companion declaration file? (First
   Section 4 risk — see below.)
7. The 6 migrations — the persisted schema the pilot declarations must be
   consistent with.
8. `rust/tests/fixtures/intent_test_utterances.toml` — 39 utterances are the
   existing behavioural oracle.

**What to produce:**

- Verb inventory table: `FQN | file | domain | behaviour | cross-workspace?`
- Implicit DAG table: `state | entry | transitions out | terminal?` — seeded
  from constellation YAML + verb preconditions.
- Boundary-leak list: verbs semantically Instrument Matrix but implemented in
  CBU / Deal / KYC crates; verbs declared in instrument-matrix-adjacent files
  but semantically belonging elsewhere.
- Plugin-trait gap analysis: does the existing `SemOsVerbOp` trait expose
  enough semantics for v1.0's three-axis declaration, or is a companion file
  required? Pick one before Section 2 Phase P.1.

`<<TO BE FILLED IN DURING EXECUTION>>`

---

## Section 2 — Pilot phased plan  `<<structural>>`

**Scope (from prompt):** Phases P.1 through P.9 as outlined. Revise the default
structure if Section 1 reveals a better shape.

**Default phase map (from prompt §2, inherit unless Section 1 forces change):**

| Phase | Scope (one-line) | Prior-phase deps | Validates v1.0 principles |
|---|---|---|---|
| P.1 | Schema + validator + catalogue-mode load gate | — | P1, P3, P11, P13 |
| P.2 | Explicit Instrument Matrix DAG taxonomy YAML | P.1 | Tranche-1 Phase-1.3 (scoped) |
| P.3 | Per-verb three-axis declaration; orphan classification A/B/C/D/E | P.1, P.2 | P11 + orphan flow |
| P.4 | Provisional tier review (Adam-as-authority) | P.3 | P-G (pilot convention) |
| P.5 | Runtime triage → Buckets 1/2/3 | P.3 | consistency-check principle |
| P.6 | DB-free catalogue-mode validation + CI gate | P.1, P.2, P.3 | P3 |
| P.7 | `cargo xtask reconcile` subset (`--validate`, `--batch`, `--status`) | P.6 | ops integration |
| P.8 | Lightweight Catalogue workspace prototype (scope-gated — see §5 Q2) | P.7 | P9 hypothesis |
| P.9 | Pilot findings report + effort data + v1.1 candidate changes | all prior | — |

**For each phase, the full plan must include:** `scope | artefacts | dependencies |
exit criteria | v1.0 mapping`. The skeleton stops at the map above.

`<<TO BE FILLED IN DURING EXECUTION>>`

---

## Section 3 — What the pilot does not test  `<<short, listed>>`

Enumerate deferred concerns per prompt §3 (list verbatim bullets from the
source prompt, adapted to ob-poc specifics where relevant):

- Cross-workspace tier consistency (Deal / CBU / KYC out-of-pilot).
- Three-axis schema expressiveness against non-Instrument-Matrix patterns.
- Real P-G governance (Adam-as-authority is the pilot convention).
- Sage / REPL integration against the full reconciled catalogue.
- Forward-discipline activation at estate scale.
- Full Catalogue workspace mechanism if P.8 is descoped.
- Cross-workspace orchestration runbooks.

`<<FILL: pilot-specific deferrals surfaced by Section 1 reading>>`

---

## Section 4 — Pilot-specific risks  `<<code-shaped, not architecture-shaped>>`

**Seed risks from reconnaissance (verify during Section 1):**

- **R-P1.** `SemOsVerbOp` trait doesn't carry `state_effect` / `external_effects` /
  `consequence_tier` today. Section 1 decision required: extend the trait OR
  introduce a companion YAML declaration file per verb. Pick one **before**
  P.1 scope is locked.
- **R-P2.** Pack declares `workspaces: [instrument_matrix, cbu]`. Cross-workspace
  pack bleed is a known concern (`PACK001` lint exists). Pilot must decide the
  CBU-peer verbs' scope. Open question in §5.
- **R-P3.** Existing instrument-matrix DAG is implicit — 3 constellation YAMLs,
  but only `trading_streetside` declares a state machine. P.2 will likely surface
  significant Orphan-C (missing sub-DAG) during P.3.
- **R-P4.** No existing scenarios for instrument matrix (count = 0 in
  `scenario_index.yaml`). P.5 runtime triage has no ScenarioIndex signal to
  leverage — has to work from TOML utterance fixtures only.
- **R-P5.** 39 `SemOsVerbOp` impls concentrated in two files. If Section 1
  companion-declaration decision picks "YAML alongside YAML", 39 new declaration
  files land in `config/verbs/...` or a parallel tree; managing divergence
  between behavioural YAML and declaration YAML becomes a real drift hazard.
- **R-P6.** P.4 Adam-only tier review on ~210 pack verbs is non-trivial effort.
  Explicitly estimate (prompt §4 calls this out).

`<<REFINE during Section 1 reading + add risks surfaced by code contact>>`

---

## Section 5 — Open questions requiring Adam's input

**Status: all four gating questions answered by Adam 2026-04-22.** Decisions
recorded below; they freeze the pilot scope so Sections 1, 6, 7 can be
executed mechanically.

**Q1. CBU-peer verbs in the pack.** **DECIDED: in scope.** Instrument Matrix
and CBU are closely linked; the pilot includes all 210 pack verbs regardless
of FQN prefix. Boundary-leak findings still get flagged in Section 1 reading,
but they don't alter pilot scope.

**Q2. Phase P.8 in-scope or deferred?** **DECIDED: in scope.** Lightweight
Catalogue workspace prototype lands inside the pilot. Observatory / Sage
integration remain post-pilot; P.8 proves the workspace concept (P9
hypothesis) for Instrument Matrix only.

**Q3. Trait extension vs companion declaration.** **DECIDED: companion YAML,
in-place (extend the existing `config/verbs/*.yaml` files).** Rationale:
- CRUD / template / macro verbs have no `SemOsVerbOp` impl, so trait
  extension couldn't cover them uniformly — hybrid model strictly worse than
  either consistent approach.
- In-place colocation (three new axes inside the existing verb YAML entry)
  beats a parallel declaration tree because "missing declaration for verb"
  becomes impossible by construction: same file, same parser, same load path.
- Drift mitigation: debug-build runtime assertion that emitted outcome
  matches declaration (e.g. `state_effect: preserve` asserts no
  `emit_pending_state_advance` fires); lint pass in `cargo x verbs lint`.

**Q4. Pilot verb cap.** **DECIDED: declare all 210.** Prune later. Declaration
is the point of the pilot; catalogue-mode loading (P3) and runbook composition
validation (P12) need the full pack for meaningful signal. Section 9 findings
report the gap between pack-declared and code-implemented (pack 210 vs
plugin 39).

`<<MORE open questions: surfaced by Section 1 code contact — add here during execution>>`

---

## Section 6 — Effort estimate + extrapolation  `<<most valuable forward output>>`

**Skeleton structure — fill per-phase during execution:**

Pilot effort table:

| Phase | Size | Critical path? | Assumption | Uncertainty range |
|---|---|---|---|---|
| P.1 | `<<M/L/XL>>` | — | validator is pure function | — |
| P.2 | `<<S/M>>` | — | DAG taxonomy comes from reading constellations | — |
| P.3 | `<<XL — likely dominant>>` | **yes** | 15-30min/verb × ~210 verbs | **wide** |
| P.4 | `<<L>>` | — | Adam review throughput N verbs/hour | — |
| P.5 | `<<M>>` | — | 39 utterances is enough to surface bucket-3 cases | — |
| P.6 | `<<S>>` | — | no hidden DB deps in instrument-matrix surface | — |
| P.7 | `<<S>>` | — | xtask pattern is well-established | — |
| P.8 | `<<L — only if in-scope>>` | conditional | Q2 answer | — |
| P.9 | `<<M>>` | — | pilot-end artefact assembly | — |

Extrapolation section:

- Per-verb declaration cost from P.3 → predict full-estate cost at ~1,500 verbs.
- Call out **non-linear** terms: orphan rate (Instrument Matrix is
  self-contained; cross-workspace runbook orphans only show up at estate scale);
  P-G review under real authority; cross-workspace tier consistency.
- Call out **over-estimators**: validator (P.1) is one-time, doesn't repeat.
- Call out **under-estimators**: Adam-alone P.4 is optimistic vs committee P-G
  at estate scale.

`<<TO BE FILLED IN DURING EXECUTION>>`

---

## Section 7 — Pilot outputs feeding forward  `<<durable value>>`

Default list from prompt §7 — verify & extend during execution:

- Reconciled Instrument Matrix catalogue → estate-scale Tranche 2 inherits.
- Validator + schema + fixture set → reusable estate-wide.
- Instrument Matrix DAG taxonomy YAML → seeds Tranche-2 Phase-2.A.
- Empirical effort data → calibrates Tranche 2 estimates.
- Architectural findings → feeds v1.1 of the refinement spec.
- (If P.8 in scope) Lightweight Catalogue workspace prototype.
- Bucket-2 follow-up queue → runtime-alignment backlog.

`<<TO BE FILLED IN DURING EXECUTION>>`

---

## Process checkpoints — status

All four gating questions answered 2026-04-22. Frozen scope:

1. ✅ **§5 Q1 — in-scope.** 210-verb pack is the declaration target.
2. ✅ **§5 Q2 — in-scope.** Phase P.8 (lightweight Catalogue workspace
   prototype) is a pilot phase, not deferred.
3. ✅ **§5 Q3 — companion YAML, in-place.** Three axes land inside the
   existing `config/verbs/*.yaml` entries; no trait extension; drift
   mitigated by debug-build runtime assertion + `cargo x verbs lint`.
4. ✅ **§5 Q4 — all 210.** Prune later. Gap between pack-declared (210)
   and code-implemented (39) goes in findings.

Execution is now mechanical fill-in of `<<TO BE FILLED IN>>` sections.
Phase P.1 scope locked on the Q3 decision: extend the serde struct in
`dsl-core/src/config/types.rs` for verb YAMLs, no `SemOsVerbOp` trait
change.

## Execution-time guardrails (from source-prompt process discipline)

- Read v1.0 first, fully. No skipping.
- Section 1 is the heaviest. Don't balance lengths.
- No implementation code. Plan is markdown.
- Don't propose a P-G org model for full refinement.
- Don't tier verbs in the plan (P.4 work, not plan work).
- Reference v1.0 by principle / tranche-phase / DoD item number.

---

**End of skeleton.** To execute: answer the four §5 questions, then proceed
section-by-section to produce `docs/todo/instrument-matrix-pilot-plan-2026-04-22.md`.

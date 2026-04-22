# Instrument Matrix Pilot — Plan (2026-04-22)

> **Status:** plan (not implementation). Produced by reconciling v1.0 of the
> Catalogue Platform Refinement against the current state of the ob-poc
> codebase in the Instrument Matrix region.
> **Source prompt:** `docs/todo/instrument-matrix-pilot-prompt.md`
> **Reference spec:** `docs/todo/catalogue-platform-refinement-v1_0.md` (v1.0)
> **Skeleton predecessor:** `docs/todo/instrument-matrix-pilot-plan-skeleton-2026-04-22.md`
> **Workspace target:** Instrument Matrix (pilot scope confirmed 2026-04-22)
> **P-G convention for pilot:** Adam-as-authority. Pilot tier decisions are
> explicitly **provisional**, flagged for re-review when the mechanism scales
> to full estate under proper organisational P-G (v1.0 §13).

---

## Pre-pilot decisions (locked 2026-04-22)

The four gating questions from §5 of the source prompt were answered before
this plan was written:

1. **CBU-peer verbs in scope.** The pack declares `workspaces: [instrument_matrix, cbu]`; all 210 `allowed_verbs` are pilot scope regardless of FQN prefix.
2. **Phase P.8 in scope.** Lightweight Catalogue workspace prototype lands inside the pilot, not after.
3. **Three-axis declaration goes in-place into `config/verbs/*.yaml`.** No `SemOsVerbOp` trait extension. Drift mitigated by debug-build runtime assertion + `cargo x verbs lint`.
4. **Declare all 210 verbs.** Prune delta between pack-declared (210) and code-implemented (39 plugin + 103 crud) goes in findings.

---

## Section 1 — Instrument Matrix codebase reading

Heaviest section by design. Surface inventoried against three evidence
sources: the pack YAML, the verb / macro declaration YAMLs, and the
`SemOsVerbOp` Rust impls. v1.0 references by principle / phase / DoD item.

### 1.1 Workspace-level anchors

| Anchor | Location | Notes |
|---|---|---|
| `WorkspaceKind::InstrumentMatrix` | `rust/src/repl/types_v2.rs:95` | Registry entry 197–214 (constellation families, subject kinds, default maps). |
| Pack | `rust/config/packs/instrument-matrix.yaml` | 210 `allowed_verbs`; workspaces = `[instrument_matrix, cbu]`. |
| Sequencer routing hook | `rust/src/sequencer.rs:1619` | Pattern match on "instrument" / "matrix" / "trading" in utterance. |
| Session enum conversion | `rust/src/repl/session_v2.rs:1036` | Workspace → session frame. |
| Scenario index | `rust/config/scenario_index.yaml` | **Zero** instrument-matrix scenarios. Pilot P.5 has no ScenarioIndex signal today. |

### 1.2 Verb inventory — by FQN prefix

Aggregated from pack `allowed_verbs` + `config/verbs/` + `config/verb_schemas/macros/`.

| FQN prefix | pack count | declared in | macro? |
|---|---|---|---|
| `trading-profile.*` | 21 | `verbs/trading-profile.yaml` (32 verbs total in file) | no |
| `matrix-overlay.*` | 16 | `verbs/matrix-overlay.yaml` | no |
| `movement.*` | 14 | `verbs/registry/movement.yaml` | no |
| `settlement-chain.*` | 13 | `verbs/custody/settlement-chain.yaml` | no |
| `instrument.*` | 12 | `verb_schemas/macros/instrument.yaml` | **yes** (7 asset-family setup + 5 lifecycle) |
| `trade-gateway.*` | 12 | `verbs/custody/trade-gateway.yaml` | no |
| `tax-config.*` | 11 | `verbs/custody/tax-config.yaml` | no |
| `booking-principal.*` | 9 | `verbs/booking-principal.yaml` | no |
| `cash-sweep.*` | 9 | `verbs/cash-sweep.yaml` | no |
| `corporate-action.*` | 9 | `verbs/custody/corporate-action.yaml` | no |
| `cbu-custody.*` | 8 | `verbs/custody/cbu-custody.yaml` | no |
| `instruction-profile.*` | 7 | `verbs/custody/instruction-profile.yaml` | no |
| `isda.*` | 6 | `verbs/custody/isda.yaml` | no |
| `booking-location.*` | 4 | `verbs/booking-location.yaml` | no |
| `delivery.*` | 3 | `verbs/delivery.yaml` | no |
| `entity-settlement.*` | 3 | `verbs/custody/entity-settlement.yaml` | no |
| `instrument-class.*` | 3 | `verbs/reference/instrument-class.yaml` | no |
| `subcustodian.*` | 3 | `verbs/reference/subcustodian.yaml` | no |
| `security-type.*` | 2 | `verbs/reference/security-type.yaml` | no |
| **Total declared in YAML** | **164 verbs + 12 macros = 176** | across 19 files | |
| **Pack delta** | **210 − 176 = 34 references without YAML declaration** | — | see §1.7 below |

### 1.3 Behaviour taxonomy (of the 164 non-macro verbs)

| Behaviour | Count | Notes |
|---|---|---|
| `crud` | 103 | Dominant; dissolution-candidate surface for v1.0 Tranche 2 Phase 2.D bucket triage later. |
| `plugin` | 50 | 39 have `SemOsVerbOp` impls today (see §1.5). 11 are pack-referenced without impl — Phase P.3 orphan-E candidates. |
| `template` | 0 | Not used in Instrument Matrix surface. |
| `macro` (separate count) | 12 | All in `instrument.*` namespace in `macros/instrument.yaml`. |

### 1.4 Implicit DAG taxonomy

Three constellation maps + families under `config/sem_os_seeds/`:

| Map | Slots | State machines |
|---|---|---|
| `instrument_workspace.yaml` | `workspace_root` (148 verb bindings across 8 capability areas) | none |
| `instrument_template.yaml` | group, trading_profile, settlement_pattern, isda_framework, corporate_action_policy, trade_gateway (6 independent slots) | none |
| `trading_streetside.yaml` | cbu (root), trading_profile, custody, booking_principal, cash_sweep, service_resource, service_intent, booking_location, legal_entity, product, delivery (11 slots) | **`trading_profile_lifecycle`** on `trading_profile` slot |

The **only declared state machine** is:

```
trading_profile_lifecycle:
  states: draft, submitted, approved, active, suspended, archived, rejected
  initial: draft
  transitions:
    draft ↔ submitted ↔ approved ↔ active         (submit / approve / activate)
    active ↔ suspended                             (suspend / reactivate)
    active → archived                              (permanent; archive)
    submitted/rejected → draft                     (revert via create-draft)
```

**Implication for v1.0 P.5 (reconciliation is directional):** Instrument
Matrix has exactly **one** formalised DAG and ~17 implicit slots with no
declared state transitions. Phase P.2 (Instrument Matrix DAG taxonomy) will
not be a documentation exercise — it will be the first time these implicit
sub-DAGs are made explicit. Expect significant Orphan-C (missing sub-DAG)
surfacing during Phase P.3 — see §4 R-P3.

### 1.5 Plugin impl gap analysis

Two files carry every `SemOsVerbOp` for this workspace:

| File | Impl count | Exposes state_effect? | Exposes external_effects? | Exposes consequence_tier? |
|---|---|---|---|---|
| `rust/src/domain_ops/trading_profile.rs` | 36 | ❌ | ❌ | ❌ |
| `rust/crates/sem_os_postgres/src/ops/trading_matrix.rs` | 3 | ❌ | ❌ | ❌ |

The 36-impl file covers the full `trading-profile.*` lifecycle plus component
assembly. The 3-impl file covers cross-matrix queries (`FindImForTrade`,
`FindPricingForInstrument`, `ListOpenSlaBreaches`).

**Confirmed:** the three-axis declaration (P1) has zero code representation
today. This is consistent with v1.0's starting assumption (§4.2, "what
doesn't yet exist").

**Design decision (locked):** the pilot adds the three axes **in-place** to
the existing `config/verbs/*.yaml` entries rather than extending
`SemOsVerbOp`. This means:
- CRUD and macro verbs (which have no Rust impl) are covered uniformly.
- Plugin verbs carry the declaration alongside the behavioural config they
  already have.
- Drift is surfaced by a debug-build runtime assertion: if the declaration
  says `state_effect: preserve` the runtime panics if
  `emit_pending_state_advance` is called during `execute`.
- Lint (`cargo x verbs lint`) enforces the declaration bijection with the
  registry manifest.

### 1.6 Schema + migration footprint

Six migrations bear on the Instrument Matrix region:

| Migration | LOC | Purpose |
|---|---|---|
| `migrations/020_trading_profile_materialization.sql` | 45 | Initial materialization plumbing. |
| `migrations/129_trading_profile_two_stage.sql` | 83 | Group template + CBU instance split (2026-03-31). |
| `rust/migrations/20260105_trading_view_config.sql` | 153 | Trading view projection config. |
| `rust/migrations/20260106_trading_profile_ast_migration.sql` | 446 | AST schema for trading-profile DSL. |
| `rust/migrations/202412_trading_matrix_storage.sql` | 614 | **Largest**. Canonical matrix persistence. |
| `rust/migrations/20260331_trading_profile_templates.sql` | 29 | Two-stage template support. |

**Relevance to pilot:** the existing schema is the ground-truth against
which the declarations must be consistent for P.1's validator — specifically
the `transitions` field of the declaration must reference DAG states that
are consistent with `trading_profile_lifecycle`'s states. No schema change
needed; the catalogue validator is pure-function (P3 — DB-free) and does
not read these tables.

### 1.7 Boundary-leak analysis

**Pack-referenced without YAML declaration: 34 verbs.** These are FQNs in
the pack's `allowed_verbs` list for which no matching `id:` exists in the
`config/verbs/` tree. Three plausible causes, to be distinguished in Phase P.3:

1. **Implicit reference-data verbs** (likely most of them): `instrument-class.*`, `security-type.*`, `subcustodian.*` reference-data CRUD may be registered programmatically rather than YAML-declared. Check the
   registry builder in `sem_os_postgres::ops::build_registry()` + `ob_poc::domain_ops::extend_registry()`.
2. **Stale pack entries**: FQNs that were declared at some point and later
   removed from YAML but not from the pack. Lint target.
3. **Cross-workspace FQNs**: verbs actually declared under a CBU or shared
   file (e.g. `entity.*` shared reference-data) that the pack references
   legitimately.

**Cross-namespace leaks (pack `allowed_verbs` with FQN prefix from another
workspace's domain):** none detected in the surface scan — all 210 FQNs
namespace cleanly to instrument-matrix-native domains. The `cbu` peer-
workspace listing on the pack is a **routing artefact**, not a verb FQN
leak. Pilot scope (Adam decision) includes all 210 regardless.

### 1.8 Fixtures and existing oracles

| Fixture | Location | Coverage |
|---|---|---|
| Intent harness | `rust/tests/fixtures/intent_test_utterances.toml` | 39 instrument/trading utterances — the existing behavioural oracle. |
| Golden corpus | `rust/tests/golden_corpus/book_setup.yaml` + `seed.yaml` | Minimal references (1–2 each); not a real exercise of the Instrument Matrix region. |

**Implication:** P.5 runtime triage has 39 oracle utterances to work
against. v1.0 Tranche 2 Phase 2.D bucket triage expects much richer
signal at estate scale; the pilot's P.5 is therefore intentionally smaller
— a proof that the triage shape works, not a coverage claim. This under-
specificity is an explicit pilot limitation (Section 3).

### 1.9 Findings that contradict v1.0's mental model

v1.0 §4.1 ("what exists") is generally accurate, but three details from
this reading warrant flagging:

1. **v1.0 §4.1 says 625 `CustomOperation` implementations.** The
   `CustomOperation` trait was deleted 2026-04-22 in Phase 5c-migrate
   slice #80 (see CLAUDE.md header: "dsl-runtime-macros deleted"). The
   surface is now `SemOsVerbOp` (~567 + 119 = 686 across two registries).
   Numerically close, but the trait is different. v1.1 candidate
   amendment: refresh §4.1.

2. **v1.0 §3 principle P4 says "declared DAG taxonomy is a governed
   projection artefact"** — present-tense. In ob-poc it isn't. Only one
   state machine (`trading_profile_lifecycle`) is declared; the other
   ~17 slots across three constellations have no declared taxonomy. P.2
   of the pilot produces the first such artefact for the Instrument
   Matrix region. This is consistent with v1.0 §4.2's "what doesn't
   exist" list, but the tension with P4's present-tense phrasing is
   worth noting for v1.1.

3. **v1.0 §4.2 says "declared DAG taxonomies" as an absent artefact
   for all four workspaces.** For Instrument Matrix specifically, the
   `trading_profile_lifecycle` state machine IS declared — so the
   artefact is partially present. P.2's job is narrower than v1.0's
   description suggests: fill in the ~17 gaps, reconcile the one
   existing machine, and formalise the whole as a YAML DAG taxonomy
   per v1.0's schema.

### 1.10 Section 1 summary — what is known and what is not

**Known going into P.1:**
- 210-verb pilot surface, 19 YAML files, 39 plugin impls across 2 files.
- One formal state machine; ~17 implicit sub-DAGs to be made explicit.
- Zero three-axis declaration in code today; zero in YAML today.
- Zero ScenarioIndex entries for the workspace.
- 39 oracle utterances.

**Unknown until P.3 contact:**
- The proportion of 164 verbs that are state-preserving vs state-transition.
- Which of the 50 pack-plugin verbs have context-dependent consequence
  (require escalation rules) vs flat-tier.
- How many of the 34 un-declared pack FQNs resolve as orphans in categories
  A–E (P.3 classification).
- Whether the `instrument.*` macros' composition rules (P12 Components B/C)
  surface genuinely new patterns versus fitting cleanly under the three
  default components.

---

## Section 2 — Pilot phased plan

Nine phases. Each phase has: scope one-liner; artefacts to produce;
dependencies on prior phases; exit criteria; and a v1.0 mapping column
showing which principles / tranche-phases / DoD items it validates.

### Phase P.1 — Schema, validator, catalogue-load gate

**Scope.** Extend the verb YAML schema to carry the three axes (`state_effect`, `external_effects`, `consequence`) with escalation-rule DSL (P1, P10, P11). Implement the validator as a pure function library. Wire catalogue-load validation to run before SemOS's DB pool init (P3). Runbook composition rule (P12) is **scoped-in** for the pilot because P.8 will exercise it against Instrument Matrix macros.

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| Extended verb YAML serde types (three axes) | `rust/crates/dsl-core/src/config/types.rs` |
| Escalation DSL + parser (restricted predicates only — equality, set-membership, thresholds over declared inputs — per v1.0 R6) | `rust/crates/dsl-core/src/config/escalation.rs` (new) |
| Validator library (structural errors + well-formedness errors + conservative policy-sanity warnings per v1.0 §6.2) | `rust/crates/sem_os_core/src/catalogue_validator.rs` (new) |
| Runbook composition engine (Components A / B / C per P12) | `rust/crates/sem_os_core/src/runbook_composition.rs` (new) |
| Startup hook calling validator before DB pool init | `rust/crates/ob-poc-web/src/main.rs` — wire the existing `build_registry() + extend_registry()` call site. |
| Unit fixtures covering: state-preserving + `requires_explicit_authorisation`; state-transition + empty `external_effects` + `requires_explicit_authorisation`; conditional escalation on arg value; conditional escalation on entity attribute | `rust/crates/sem_os_core/tests/fixtures/three_axis_samples/` (new) |

**Dependencies.** None (starts the pilot).

**Exit criteria.**
- Validator fixture suite green with all error classes + conservative warnings exercised (v1.0 DoD items 1, 3, 5).
- Catalogue-load validation firing on `ob-poc-web` startup (v1.0 DoD items 7, 10).
- `cargo x verbs lint` returns non-zero on any verb YAML with a malformed `consequence` block.
- Debug-build runtime assertion committed (drift between declaration's `state_effect` and runtime emission of `emit_pending_state_advance` panics in debug).

**v1.0 mapping.** P1, P3, P10, P11, P12, P13 (structural vs policy); Tranche 1 phases 1.1 + 1.2 + 1.4; Tranche 1 DoD items 1, 3, 5, 7, 10, 12.

---

### Phase P.2 — Instrument Matrix DAG taxonomy YAML

**Scope.** Produce an explicit YAML taxonomy for the Instrument Matrix workspace's DAG(s). Reconcile the existing `trading_profile_lifecycle` state machine. Surface the implicit state space of the 17 other slots across `instrument_template` / `trading_streetside` / `instrument_workspace` and decide — per slot — whether it has a hidden state machine that needs declaring, or is genuinely stateless (a container, a projection, a lookup).

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| `instrument_matrix_dag.yaml` — declared DAG taxonomy per v1.0 §6.2 schema | `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml` (new directory per v1.0 Tranche 1 Phase 1.3) |
| Reconciled `trading_profile_lifecycle` section (states, transitions, terminal states explicit) | same file |
| New state machines for any non-stateless slots discovered | same file |
| Slot-stateless declarations for projection/container slots (explicit "no state" rather than silent absence) | same file |

**Dependencies.** P.1 must land first because the schema for DAG taxonomy is defined there.

**Exit criteria.**
- One YAML file at the new path with no validator errors.
- Every slot in the three constellation maps is accounted for: either carries a declared state machine, or is explicitly marked stateless with a reason.
- `trading_profile_lifecycle` declaration is byte-identical in intent to the current runtime behaviour (no semantic drift introduced).
- Internal consistency check: every state referenced by a verb's `pre:` or `transitions:` field (added in P.3) will resolve against this taxonomy.

**v1.0 mapping.** P4 (declared DAG = governed projection), P5 (reconciliation directional); Tranche 1 Phase 1.3 scoped to one workspace; Tranche 1 DoD item 6 (partial — one workspace of four).

---

### Phase P.3 — Per-verb three-axis declaration + orphan classification

**Scope.** For all 210 pack verbs, add the three axes inline into the
existing `config/verbs/*.yaml` / `config/verb_schemas/macros/*.yaml`
entries. Every verb declares: `state_effect: transition|preserving`;
`external_effects: []` (subset of {observational, emitting, navigating});
`consequence: { baseline: <tier>, escalation: [<rules>] }`. Orphans (verbs
that don't cleanly map to the P.2 taxonomy) are classified A/B/C/D/E per
v1.0 P6 orphan flow. Baseline tiers are **provisional** under Adam-as-
authority (P.4 is the review pass).

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| Three-axis fields added to each verb entry in 19 YAML files | `rust/config/verbs/*.yaml`, `rust/config/verb_schemas/macros/*.yaml` |
| Orphan classification table | `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_orphans.yaml` (new) with one row per orphan verb: `fqn`, `category (A|B|C|D|E)`, `resolution_action`, `provisional_tier` |
| 34-verb delta resolution (the pack-references-without-YAML-declaration set from §1.7) | either (a) formalise the reference-data verbs as YAML, (b) remove stale pack entries, or (c) document legitimate cross-workspace FQNs. Decision per verb. |

**Per-verb effort envelope.** v1.0 assumes 15–30 minutes per verb. Pilot
expects wide variance: simple CRUD verbs under 10 minutes each; plugin
verbs with complex context-dependencies (e.g. `trading-profile.activate`
— escalates based on whether the target CBU has live trades) potentially
30–60 minutes each. Empirical calibration is one of the two most valuable
pilot outputs (§6).

**Dependencies.** P.1 (schema must exist), P.2 (DAG taxonomy must exist
so `pre:` / `transitions:` fields resolve).

**Exit criteria.**
- All 210 pack verbs carry three-axis declarations.
- Validator passes zero structural errors / zero well-formedness errors.
- Orphan table documents disposition of every un-mapped verb.
- Per-verb provisional tier column populated (review-gated in P.4).
- Escalation-rules declared for every context-dependent verb (not every verb — only those where runtime context determines the effective tier). Expected ~10–30% of plugin verbs per v1.0 R10.

**v1.0 mapping.** P1, P6 (orphan flow), P10 (orthogonality), P11 (baseline
+ escalation); Tranche 2 Phase 2.A + 2.B scoped to one workspace; Tranche
2 DoD items 1, 2, 5.

---

### Phase P.4 — Provisional tier review (Adam-as-authority)

**Scope.** Adam, as pilot P-G authority, reviews tier assignments from
P.3. Cluster similar verbs (e.g. all `*.approve` verbs likely cluster
to `requires_confirmation` baseline; `*.archive` to `reviewable`; pure
reads to `benign`). Resolve ambiguous cases with documented rationale.
**Mark every tier decision as provisional pending estate-scale P-G review.**

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| Tier decision record | `docs/todo/instrument-matrix-tier-decisions-2026-04-22.md` (new). Per non-trivial decision: verb FQN, assigned tier, rationale, "provisional-pending-estate-P-G" flag. |
| Tier-cluster table | part of same doc. Rows: cluster name (e.g. "approval verbs"), verbs in cluster, baseline tier, common escalation pattern. |
| Reviewed YAML | commits to `rust/config/verbs/*.yaml` updating baselines from P.3 defaults where review changed them. |

**Dependencies.** P.3 (provisional tiers must exist to review).

**Exit criteria.**
- Every tier decision is documented OR implicit in a cluster rule.
- At least every verb that currently lives in a `*.approve`, `*.archive`,
  `*.activate`, `*.reject`, `*.suspend`, `*.delete`, or any verb touching
  sanctions / settlement-readiness / regulatory classification (per v1.0
  P10's own examples) has an explicit (non-cluster-default) rationale.
- Decision record stamped `provisional — Adam-as-authority pilot
  convention` in its frontmatter.

**v1.0 mapping.** P13 (policy axis, governance-decisioned);
Tranche 2 Phase 2.C scoped to one workspace under pilot P-G convention;
Tranche 2 DoD item 6 (partial — one workspace).

**Explicit non-goal.** The pilot does NOT propose an organisational
structure for full-refinement P-G (§6 of source prompt; v1.0 §13). That
remains a Tranche 1 DoD item for the full refinement.

---

### Phase P.5 — Runtime triage (Buckets 1/2/3)

**Scope.** Run the 39 oracle utterances from
`rust/tests/fixtures/intent_test_utterances.toml` against the reconciled
catalogue. For each: does runtime behaviour at the state-transition
semantic layer match the declaration? Categorise findings into
Buckets 1 (aligned), 2 (divergent but acceptable — queued for follow-up),
3 (divergent and structural — fix in pilot scope).

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| Runtime triage report | `docs/todo/instrument-matrix-runtime-triage-2026-04-22.md` |
| Per-utterance finding | in same doc: utterance, declared tier (composed), runtime tier, bucket, resolution |
| Bucket-3 fixes | per-verb commits to `rust/config/verbs/*.yaml` or (if runtime code needs change) tracked as code follow-up |
| Bucket-2 follow-up queue | appendix to triage report; hands off to runtime-alignment backlog |

**Dependencies.** P.3 (declarations must exist to compare), P.4 (reviewed
tiers).

**Exit criteria.**
- All 39 utterances triaged.
- Bucket 3 empty (all fixed in pilot) or explicitly documented.
- Bucket 2 queue bounded and sized (v1.0 R11 threshold: pilot pauses if
  Bucket 2 > 10 items or > 25% of 39 ≈ 10 items).

**v1.0 mapping.** P7 (runtime alignment semantic, not comprehensive);
Tranche 2 Phase 2.D scoped to one workspace; Tranche 2 DoD item 4.

---

### Phase P.6 — DB-free catalogue-mode validation

**Scope.** Prove that the reconciled Instrument Matrix catalogue validates
and loads without any DB connection. Wire catalogue-load validation
strictly before `ob-poc-web` opens its `sqlx::PgPool`. Add a CI gate that
runs `cargo x catalogue validate` with `DATABASE_URL=` unset and expects
zero failures.

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| CI gate | `.github/workflows/catalogue-validate.yml` (new) or equivalent existing CI entry |
| Startup trace showing validator-then-pool ordering | runtime log in ob-poc-web boot, asserted by an integration test |
| Integration test `catalogue_db_free_smoke.rs` under `rust/tests/` | confirms validator + full registry build against in-memory config only, no `DATABASE_URL` env var consulted |

**Dependencies.** P.1, P.3 (catalogue must exist to validate).

**Exit criteria.**
- `DATABASE_URL=` cargo test passes the new integration test.
- CI job is wired.
- Zero hidden DB dependencies in the validator or the catalogue loader —
  any found are refactored out in pilot scope.

**v1.0 mapping.** P3 (DB-free catalogue-mode); Tranche 1 Phase 1.4
scoped to one workspace; Tranche 1 DoD items 7, 10, 12.

---

### Phase P.7 — `cargo xtask reconcile` minimal client

**Scope.** Three subcommands against the reconciled Instrument Matrix
catalogue:

- `cargo x reconcile --validate` — runs the P.1 validator over the
  catalogue, reports errors and warnings.
- `cargo x reconcile --batch <op>` — scaffold for bulk per-verb
  operations (Tranche 2 at estate scale; pilot validates the shape
  against 210 verbs).
- `cargo x reconcile --status` — prints declaration coverage: X of 210
  verbs fully declared; Y orphan resolutions; Z escalation rules
  declared.

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| `reconcile` subcommand | `rust/xtask/src/reconcile.rs` (new), wired into `rust/xtask/src/main.rs` |
| Unit tests for the three subcommands | `rust/xtask/tests/reconcile_tests.rs` (new) |

**Dependencies.** P.6 (needs the DB-free validator wired).

**Exit criteria.**
- All three subcommands run against the Instrument Matrix catalogue and
  produce the expected output.
- `--status` returns 100% declaration coverage for pack verbs.
- Tests green.

**v1.0 mapping.** Tranche 1 Phase 1.5 scoped to one workspace; Tranche 1
DoD item 8.

---

### Phase P.8 — Lightweight Catalogue workspace prototype

**Scope (pilot-only subset, per Adam 2026-04-22).** Build a *minimal*
Catalogue workspace under `WorkspaceKind::Catalogue` that supports
authorship verbs for the Instrument Matrix region only. **No Observatory
integration. No Sage wiring. No access control** beyond the existing
ABAC plumbing. Proves the mechanism (P9 hypothesis); does not deliver
full Tranche 3 capability.

Authorship verbs in scope:

- `catalogue.propose-verb-declaration` — stage a new three-axis
  declaration for review.
- `catalogue.commit-verb-declaration` — promote staged declaration to
  `config/verbs/*.yaml` after P-G signoff (pilot: Adam-as-authority).
- `catalogue.rollback-verb-declaration` — revert to prior declared state.
- `catalogue.list-proposals` — read staging area.

These four exercise the mechanism end-to-end for Instrument Matrix verbs
without reimplementing the full authorship library Tranche 3 will build.

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| `WorkspaceKind::Catalogue` registry entry | `rust/src/repl/types_v2.rs` — extend enum + registry |
| Pack for catalogue workspace | `rust/config/packs/catalogue.yaml` — just the 4 authorship verbs |
| YAML declarations for the 4 authorship verbs (with their own three-axis declarations; likely `catalogue.commit-*` is `requires_explicit_authorisation`, `catalogue.propose-*` is `reviewable`, `list` is `benign`) | `rust/config/verbs/catalogue.yaml` (new) |
| Plugin impls for the 4 verbs | `rust/src/domain_ops/catalogue_ops.rs` (new), registered via `extend_registry()` |
| Staging store (filesystem-based for pilot, not DB-backed) | staging area under `rust/config/verb_staging/` |
| Integration test that proposes → reviews → commits a declaration change for one Instrument Matrix verb end-to-end | `rust/tests/catalogue_authorship_smoke.rs` (new) |

**Dependencies.** P.7 (xtask + validator in place), P.4 (tier model in
use so authorship verbs can declare their own consequence correctly).

**Exit criteria.**
- End-to-end smoke test green: authorship flow modifies an Instrument
  Matrix verb's declared tier, runs through propose → review → commit,
  and the validator confirms the changed state.
- Access control uses existing ABAC plumbing (no new access model).
- Four authorship verbs carry their own three-axis declarations.
- No Observatory / Sage integration (explicitly out).

**v1.0 mapping.** P8 (governance by architectural enforcement — partial,
pilot-scope proof); P9 (catalogue workspace hypothesis) — pilot validates
the hypothesis for Instrument Matrix only; Tranche 3 Phases 3.A + 3.B
scoped to four verbs; Tranche 3 DoD items 1 + 2 + 3 (minimal — proves
they're implementable, doesn't build all Tranche-3 macros).

---

### Phase P.9 — Pilot findings report + v1.1 candidates

**Scope.** Synthesise everything into a findings document: what worked,
what didn't, what v1.0 over- or under-specified, empirical effort data
per phase and per verb, extrapolation to full-estate effort,
architectural tensions that warrant v1.1 amendments to the spec, and
durable artefacts handed forward.

**Concrete artefacts.**

| Artefact | Location |
|---|---|
| Findings report (this is the pilot deliverable, distinct from this plan) | `docs/todo/instrument-matrix-pilot-findings-2026-04-22.md` (or dated when pilot completes) |
| Effort data table | in findings; per phase, median + range in hours |
| Extrapolation section | in findings; pilot effort × scaling factor → full-estate estimate |
| v1.1 candidate change list | in findings; per candidate: which v1.0 section it amends, why, evidence |
| Durable artefact manifest (pointers to all outputs feeding Tranche 2) | in findings; one-line per artefact |

**Dependencies.** P.1 through P.8 all complete.

**Exit criteria.**
- Report exists.
- Effort data is empirical (real hour counts, not restatements of
  estimates).
- v1.1 candidates are evidence-backed, not speculative.
- All 7 "pilot outputs as inputs" from §7 of the source prompt are
  produced and reachable from the findings doc.

**v1.0 mapping.** Everything — this is the meta-deliverable.

---

### Phase summary table

| Phase | Size | Deps | v1.0 DoD items validated | Gate for next phase? |
|---|---|---|---|---|
| P.1 Schema + validator | L | — | Tranche 1: 1, 3, 5, 7, 10, 12 | yes (P.2, P.3) |
| P.2 DAG taxonomy | M | P.1 | Tranche 1: 6 (partial) | yes (P.3) |
| P.3 Per-verb declaration | **XL** | P.1, P.2 | Tranche 2: 1, 2, 5 | yes (P.4, P.5) |
| P.4 Tier review | L | P.3 | Tranche 2: 6 (partial) | yes (P.5) |
| P.5 Runtime triage | M | P.3, P.4 | Tranche 2: 4 (partial) | yes (P.6) |
| P.6 DB-free validation | S | P.1, P.3 | Tranche 1: 7, 10, 12 | yes (P.7) |
| P.7 xtask client | S | P.6 | Tranche 1: 8 | yes (P.8) |
| P.8 Catalogue workspace proto | L | P.7, P.4 | Tranche 3: 1, 2, 3 (partial) | no |
| P.9 Findings + v1.1 | M | all | all | — |

**Critical path: P.1 → P.2 → P.3 → P.4 → P.5 → P.9.** P.6 / P.7 / P.8
branch off the main line at different points. P.3 is the dominant-effort
phase.

---

## Section 3 — What the pilot does not test

Per source prompt §3 plus evidence from Section 1:

- **Cross-workspace tier consistency.** Deal / CBU / KYC workspaces are
  out of pilot. Tier cluster rules from P.4 may not generalise —
  explicitly re-reviewed at estate scale.
- **Three-axis schema expressiveness against non-Instrument-Matrix patterns.**
  The 210 pilot verbs are dense in trading-profile lifecycle and custody
  configuration. Patterns specific to, e.g., KYC case transitions or
  Deal rate-card negotiation are not stressed.
- **Real-authority P-G governance.** Pilot uses Adam-as-authority; real
  P-G requires organisational partnership (v1.0 R13).
- **Sage and REPL integration against the full reconciled catalogue.**
  The P.4 reviewed tiers sit in YAML; neither Sage autonomy (P14) nor
  REPL tier-aware confirmation is wired in pilot scope.
- **Forward-discipline activation at estate scale.** Pilot doesn't
  remove direct-YAML edit paths; P.8 authorship is additive, not
  exclusive.
- **Full Catalogue workspace mechanism.** P.8 builds four authorship
  verbs. The full Tranche-3 library (authoring macros, Observatory
  integration, Sage agentic authorship) is deferred.
- **Cross-workspace orchestration runbooks.** P12 Components B / C
  (aggregation + cross-scope) aren't stressed because most pilot
  runbooks are Instrument-Matrix-internal. An estate-scale runbook
  spanning KYC → Instrument Matrix → Deal is not rehearsed.
- **Ground truth for the 34 pack-FQN delta (§1.7).** Three causes were
  hypothesised; pilot decides per-verb, but the exhaustive estate-scale
  analysis is Tranche 2 work.

---

## Section 4 — Pilot-specific risks

Code-shaped, ob-poc-specific. Not a restatement of v1.0 R1–R24.

### R-P1. Schema-vs-impl drift from in-place YAML declaration

**Evidence.** 39 plugin verbs in two Rust files; the in-place YAML
declaration moves semantic truth to YAML while behavioural implementation
stays in Rust. The debug-build runtime assertion mitigates but doesn't
eliminate the risk.

**Concrete failure shape.** A developer changes `trading-profile.activate`
to emit a state advance in code without updating the YAML's
`state_effect: preserve` declaration. Debug assertion fires in debug,
release build silently mismatches declaration.

**Mitigation.** (a) Debug assertion committed in P.1. (b) `cargo x verbs
lint` runs in CI and asserts the declaration's `state_effect` value
matches the op's observed behaviour against a fixture suite. (c) Release
builds forbidden from running ops whose lint status is "fail".

### R-P2. Pack–declaration delta (the 34 undeclared FQNs)

**Evidence.** §1.7 — pack `allowed_verbs` has 34 entries that don't
resolve to a YAML `id:` today. Phase P.3 has to decide per-verb whether
these are (a) programmatically registered reference verbs, (b) stale
pack entries, or (c) legitimate cross-workspace FQNs.

**Impact.** P.3 effort estimate assumes 210-verb declaration scope; if
the 34 delta resolves as "stale pack entries to remove", that's pack
cleanup. If it resolves as "programmatically registered reference verbs
to formalise in YAML", that adds 34 net-new YAML entries.

**Mitigation.** P.3 first-task: classify the 34 delta. Surface the
breakdown in the findings. Scope adjustment landed in P.3 scope rather
than discovered in P.9.

### R-P3. Significant Orphan-C surface in P.3

**Evidence.** §1.4 — only one of ~18 constellation slots has a declared
state machine. Every other slot is implicit.

**Impact.** P.3's orphan classification will produce a lot of Orphan-C
(missing sub-DAG) decisions. Each Orphan-C decision is a state-machine
design micro-exercise that v1.0 §Orphan-C assumes is cheap but isn't.

**Mitigation.** P.2 produces the DAG taxonomy YAML before P.3 starts —
the orphan classification in P.3 reduces to "is this verb covered by a
state machine declared in P.2, yes or no". If P.2 surfaces the need for
new state machines during its own reconciliation pass, they're added in
P.2 scope, not deferred to P.3.

### R-P4. No existing ScenarioIndex entries → P.5 has thin oracle

**Evidence.** §1.1 — zero instrument-matrix scenarios in
`config/scenario_index.yaml`. §1.8 — 39 utterance-oracle entries.

**Impact.** Runtime triage in P.5 has less signal than the estate-scale
Tranche 2 Phase 2.D will. Bucket-3 detection may miss divergences that
only surface under scenario composition.

**Mitigation.** Findings (P.9) explicitly documents P.5 coverage ≠
estate-scale coverage. Pilot claims mechanism correctness, not
behavioural coverage.

### R-P5. Two-file concentration of plugin impls

**Evidence.** §1.5 — 36 of 39 impls in `trading_profile.rs` alone.

**Impact.** The in-place YAML declaration decision (Q3 — locked) means
these 39 impls are NOT being refactored. The runtime assertion must
therefore thread through the dispatcher and verify every op's emitted
outcome against its declaration. That threading is touchy:
- The op's declaration lookup happens in the dispatcher (`SemOsVerbOpRegistry` already carries the FQN).
- The outcome observation requires hooking the `VerbExecutionOutcome` return path.
- Only needs to run in `#[cfg(debug_assertions)]`.

**Mitigation.** P.1 scope explicitly includes the dispatcher-level
assertion wiring (not just a library assertion). Code review by someone
familiar with the Phase-5c-migrate sequencer boundary.

### R-P6. P.4 Adam-only tier review throughput

**Evidence.** 210 verbs; even at 2 minutes/verb for pure mechanical
clustering that's 7 hours pure throughput; more realistically 15–30
minutes per non-trivial decision × (say) 60 non-trivial decisions = 15–30
hours. This is the single highest-cost human activity in the pilot for
Adam.

**Impact.** If P.4 stalls, P.5 / P.6 / P.7 / P.8 all block.

**Mitigation.** (a) P.3 pre-clusters aggressively by FQN pattern (all
`*.approve`, `*.archive`, `*.list` etc.) so P.4 review is per-cluster
not per-verb. (b) Decision record is append-only; Adam can pause-and-
resume across sessions. (c) P.8 is explicitly marked "blocked on P.4"
in the gate table so the pilot doesn't start P.8 prematurely.

### R-P7. P.8 authorship verbs touching live YAML

**Evidence.** Proposal → commit flow writes to `rust/config/verbs/*.yaml`.
Live catalogue.

**Impact.** A failed P.8 smoke test can leave the repo in a dirty state
(staged declaration not rolled back). Worse: a partially-committed
declaration that passes validation but diverges from the reviewed P.4
decision.

**Mitigation.** P.8's staging store is a filesystem directory
(`rust/config/verb_staging/`), separate from the authoritative
`config/verbs/`. Commits go through an explicit commit verb that
validates against the P.4 decision record. Integration test runs
against a temp-directory-rooted catalogue, not the live repo.

---

## Section 5 — Open questions requiring Adam's input

§5 Q1–Q4 locked 2026-04-22 (pre-pilot decisions at top of this plan).
Questions surfaced by Section 1 reading that still require Adam input
during pilot execution:

**Q5. The 34 pack-FQN delta disposition.** Cause (a) vs (b) vs (c) per
verb. My recommendation after Section 1 reading: P.3's first task
classifies the delta; Adam reviews the split before bulk declaration
work starts. Likely breakdown: most are (a) reference-data verbs that
should be YAML-declared; a few are (c) legitimate cross-references; (b)
stale entries should be rare.

**Q6. Escalation DSL extension threshold.** v1.0 R5/R6 trade off DSL
restrictiveness against expressiveness. P.3 WILL surface verbs whose
context-dependencies don't fit the starting DSL (equality, set-membership,
thresholds). Does the pilot extend the DSL (P.1 rework) or force-fit with
workarounds and flag as v1.1 candidate? My recommendation: **extend
during pilot** if extension is declarative (e.g. add attribute-exists
predicate); defer as v1.1 candidate if extension requires turing-
completeness or custom predicates. Hard to codify in advance.

**Q7. Runbook composition rule authoring (P12 B + C).** Pilot exercises
composition against Instrument Matrix macros. If the `instrument.*`
macros (7 asset-family + 5 lifecycle) produce composition rules that
diverge from v1.0's three-default-components, does the pilot propose
specific rule extensions in findings (v1.1) or treat the divergence as
Instrument-Matrix-specific and defer? My recommendation: **findings
document the patterns; proposes extensions only if they're likely to
generalise**. Pilot shouldn't bake Instrument-Matrix specifics into
v1.1.

**Q8. P.8 rollback semantics.** `catalogue.rollback-verb-declaration`
needs a history surface. Is a filesystem log (append-only)
sufficient for pilot, or does P.8 scope include a minimal DB-backed
proposal table? My recommendation: **filesystem log for pilot**. DB
staging is Tranche-3 territory.

---

## Section 6 — Effort estimate + extrapolation

### 6.1 Pilot effort table

Estimates in working days. Ranges widen where Section 1 reading was
less direct. Assumes single-engineer focus + Adam review throughput.

| Phase | Expected | Range | Driver of uncertainty |
|---|---|---|---|
| P.1 Schema + validator + load-gate | 8 | 5–12 | Escalation-DSL design iteration |
| P.2 DAG taxonomy | 3 | 2–5 | How many implicit sub-DAGs need new state machines |
| P.3 Per-verb declaration | **15** | **10–25** | Pack-delta classification + context-dependent verb density |
| P.4 Tier review (Adam time) | 4 | 3–7 | Cluster-vs-individual decision count |
| P.5 Runtime triage | 2 | 1–4 | Bucket-3 fixes required |
| P.6 DB-free validation + CI | 1 | 0.5–2 | — |
| P.7 xtask client | 1.5 | 1–3 | — |
| P.8 Catalogue workspace proto | 6 | 4–10 | Staging model + runtime assertion wiring |
| P.9 Findings + v1.1 | 3 | 2–5 | Synthesis depth |
| **Total** | **43.5 days** | **28.5–73 days** | |

Expected ≈ **9 weeks** single-engineer + ~1 week Adam review.
Worst-case ≈ **15 weeks**.

P.3 dominates as predicted. If P.3's range materialises at the high end
(context-dependent verbs at 20%+ density per v1.0 R10), the extrapolation
factor changes (see 6.2).

### 6.2 Extrapolation to ~1,500-verb estate

Per-verb cost extracted from pilot:
- Pilot: 15 working days over 210 verbs = **≈0.57 hours per verb** (P.3 alone).
- Estate-scale: 1,500 verbs × 0.57 hours = **≈855 hours = ~107 working days = ~22 working weeks** for the per-verb declaration alone.

**Linear extrapolation (what scales cleanly):**
- P.3 per-verb declaration effort.
- P.2 DAG taxonomy — not linear because per-workspace effort is
  largely fixed cost; 4 workspaces ≈ 4× P.2 not 7× (Instrument Matrix
  is the 2nd-densest).
- P.5 runtime triage per verb.

**Non-linear (what likely underestimates):**
- **Cross-workspace tier consistency review.** Pilot P.4 reviews 210
  verbs in one cluster family; estate-scale P-G must reconcile tier
  decisions across 4 workspaces with potentially conflicting conventions.
  Likely 2–3× pilot P.4 time per workspace.
- **Orphan-C density elsewhere.** Instrument Matrix has one declared
  state machine; Deal / KYC / CBU likely have richer declared DAG
  already (KYC case FSM is well-established). Instrument Matrix's
  Orphan-C rate is probably a ceiling, not average.
- **Real P-G governance overhead.** Pilot Adam-alone is the fastest
  possible authority model. Committee or per-workspace ownership (v1.0
  §13 candidates) introduces coordination cost.
- **Cross-workspace runbook composition (P12 B/C).** Pilot surface is
  internal-only. Estate-scale will stress Components B + C on cross-
  workspace macros — likely surface genuine rule gaps.

**Non-linear (what likely overestimates):**
- **P.1 schema + validator + load-gate** is one-time infrastructure.
  Estate work reuses, doesn't rebuild. Removes ~5 days of pilot effort
  from the estate multiplier.
- **P.6 DB-free validation infra** is one-time. Estate work reuses.
- **P.7 xtask client** is one-time.
- **P.8 workspace mechanism design** — pilot proves P9; estate implements
  the full Tranche-3 library, but design debate is substantially
  resolved.

### 6.3 Refined estate-scale estimate

Pilot total: 43.5 days.
Subtract reusable infrastructure (P.1 + P.6 + P.7 = 10.5 days): **33 days of per-workspace work** for one workspace at 210 verbs.

Per-workspace cost: **33 days / 210 verbs × N** where N is
workspace verb count.

| Workspace | Verb count (rough) | Estimated work (days) |
|---|---|---|
| Instrument Matrix | 210 | 33 (pilot) |
| Deal | ~300 | ≈47 |
| CBU | ~400 | ≈63 |
| KYC | ~250 | ≈39 |
| Remaining ~340 verbs in other domains | ~340 | ≈53 |
| **Per-workspace subtotal** | ~1,500 | **≈235 days** |
| One-time infrastructure (P.1, P.6, P.7, P.8-full) | — | ~20 days |
| **Non-linear surcharge (cross-workspace P-G, Component B/C)** | — | ~30–60 days |
| **Estate-scale total** | **1,500** | **~285–315 working days ≈ 13–15 weeks** |

v1.0 Tranche 2 ("1,500-verb audit") was undated but described as the
tranche with the most volume. ~13–15 weeks of engineering effort at
estate scale matches the v1.0 framing reasonably well.

The **most valuable forward signal** from this extrapolation: P.3's
per-verb cost is the critical multiplier. If pilot P.3 empirical data
comes in at the high end (25 days), estate-scale Tranche 2 P.3 becomes
**~180 days** of per-verb declaration alone — doubling the total. A
tight pilot P.3 is worth more to v1.0 Tranche-2 planning than any
other pilot deliverable.

### 6.4 Critical path

P.1 → P.2 → P.3 → P.4 → P.5 → P.9. Seven-phase linear chain in the
expected case. P.6 / P.7 / P.8 parallelize off the main line once their
deps land. Actual calendar time with parallelism: **≈7–8 weeks**
expected, **≈12 weeks** worst case.

---

## Section 7 — Pilot outputs feeding forward

Durable artefacts produced by the pilot and their consumers:

| Artefact | Location | Consumer (Tranche/Phase at estate scale) |
|---|---|---|
| **Reconciled Instrument Matrix catalogue** (210 verbs with three-axis declarations) | `rust/config/verbs/*.yaml` updated in-place | Tranche 2 Phase 2.A inherits — Instrument Matrix region doesn't re-do declaration. |
| **Validator + escalation DSL + composition engine** | `rust/crates/sem_os_core/src/catalogue_validator.rs` + `escalation.rs` + `runbook_composition.rs` + `dsl-core` schema types | Tranche 1 Phase 1.2 — estate does not re-implement. |
| **20-verb fixture suite covering schema combinations + escalation + composition** | `rust/crates/sem_os_core/tests/fixtures/three_axis_samples/` | Tranche 1 Phase 1.6 — fixture reuses; may need extension for non-Instrument-Matrix patterns. |
| **Instrument Matrix DAG taxonomy YAML** | `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml` | Tranche 2 Phase 2.A — seeds the taxonomy library. |
| **Orphan classification table** (the 210-verb A/B/C/D/E disposition) | `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_orphans.yaml` | Tranche 2 Phase 2.B — pattern reuses for other workspaces. |
| **Tier decision record** (provisional, Adam-as-authority) | `docs/todo/instrument-matrix-tier-decisions-2026-04-22.md` | Tranche 2 Phase 2.C — re-review under real P-G, pilot clusters inform cross-workspace decisions. |
| **Runtime triage report + Bucket 2 queue** | `docs/todo/instrument-matrix-runtime-triage-2026-04-22.md` | Tranche 2 Phase 2.D — mechanism validated. Bucket 2 hands to runtime-alignment backlog. |
| **`cargo x reconcile` xtask subcommands** | `rust/xtask/src/reconcile.rs` | Tranche 1 Phase 1.5 — estate reuses, adds `--commit`/`--rollback`/`--macro` in Tranche 3. |
| **DB-free catalogue-mode CI gate + startup wiring** | `.github/workflows/catalogue-validate.yml` + `ob-poc-web/main.rs` | Tranche 1 Phase 1.4 — one-shot infrastructure. |
| **Lightweight Catalogue workspace prototype** (4 authorship verbs, FS-staged) | `rust/config/verbs/catalogue.yaml` + `rust/src/domain_ops/catalogue_ops.rs` | Tranche 3 Phase 3.A — P9 hypothesis validated for Instrument Matrix; extends to full authorship library. |
| **Empirical effort data** (pilot P.3 per-verb actuals, P.4 review actuals) | In `docs/todo/instrument-matrix-pilot-findings-2026-04-22.md` | Tranche 2 planning — calibrates the 1,500-verb estimate. |
| **v1.1 candidate-amendment list** | In findings | Feeds v1.1 of the architecture spec before Tranche 1 execution begins. |
| **Architectural findings** (what v1.0 got right, wrong, under-specified) | In findings | v1.1 editorial pass. |

### 7.1 Not-yet-decided outputs

Two durable outputs depend on pilot-empirical evidence and aren't
declarable in advance:

- **Escalation-DSL extensions** — if Q6 resolves "extend during pilot",
  the extended DSL grammar becomes a durable output flowing into v1.1.
- **Composition-rule library extensions** — Q7 may surface patterns
  worth adding to Components B / C defaults; those become v1.1 proposals.

---

## Execution guardrails

From source-prompt §Process discipline, reiterated here as a checklist
for each phase's execution:

- Reference v1.0 by principle / phase / DoD item. Do not restate v1.0
  content.
- Do not propose a P-G authority model for the full refinement. Pilot
  uses Adam-as-authority; full-refinement P-G is organisational, not
  architectural.
- Do not tier verbs in this plan. Tiering is Phase P.4 work, executed
  by Adam during pilot execution.
- No implementation code in this plan. This plan is markdown. Each
  phase's artefact locations are pointers for execution.
- If pilot execution surfaces architectural tension (not just code-
  level adjustment), capture it in P.9 as a v1.1 candidate. If it's
  pure code adjustment, resolve in place.

---

**End of Instrument Matrix Pilot Plan.**

# Instrument Matrix — DAG ↔ DSL Break Remediation Plan (2026-04-23)

> **Purpose:** enumerate, classify, and sequence the concrete DAG↔DSL breaks
> discovered by the Instrument Matrix pilot plan's codebase reading, so the
> break set is explicit before P.2 / P.3 execution rather than implicit in
> phase work.
>
> **Parent doc:** `docs/todo/instrument-matrix-pilot-plan-2026-04-22.md`
> **Reference spec:** v1.1 (`docs/todo/catalogue-platform-refinement-v1_1.md`)
> **Source evidence:** pilot plan §1.4 (implicit DAG taxonomy), §1.5 (plugin
> impl gap), §1.7 (boundary leaks), §4 R-P3 (orphan-C surface).

---

## 1. Break inventory

Five distinct classes of DAG↔DSL break found during the Section 1 codebase
reading. Counts are empirical at 2026-04-22/23 on `main` at the
Instrument Matrix surface.

### Break B-1 — Undeclared sub-DAGs across 17 of 18 constellation slots

**Evidence.** Three constellation maps under
`rust/config/sem_os_seeds/constellation_maps/`:

| Map | Slots | State machines declared |
|---|---|---|
| `instrument_workspace.yaml` | 1 (workspace_root) | 0 |
| `instrument_template.yaml` | 6 | 0 |
| `trading_streetside.yaml` | 11 | 1 (`trading_profile_lifecycle`) |
| **Total** | **18** | **1** |

17 of 18 slots have no formal state machine. Verbs that notionally
transition those slots' states (e.g. `settlement-chain.activate`,
`matrix-overlay.apply`, `corporate-action.*` lifecycle verbs) cannot
have a validatable `transitions:` block per v1.1 P1 because the target
DAG states don't exist as declared artefacts.

**Scale.** At least 17 DAGs to formalise. Some slots may be genuinely
stateless (containers, projections, reference lookups) and need
explicit stateless declaration per P.2's exit criterion 2; the rest
need real state machines.

### Break B-2 — Pack references 34 FQNs with no YAML declaration

**Evidence.** Pilot plan §1.7: `rust/config/packs/instrument-matrix.yaml`
lists 210 `allowed_verbs`; only 176 resolve to an `id:` in
`config/verbs/*.yaml` or `config/verb_schemas/macros/*.yaml`. The 34
unmatched FQNs fall into three plausible causes:

- **(a) Implicit reference-data verbs** — `instrument-class.*`,
  `security-type.*`, `subcustodian.*` may be registered
  programmatically in `sem_os_postgres::ops::build_registry()` or
  `ob_poc::domain_ops::extend_registry()` rather than YAML-declared.
- **(b) Stale pack entries** — FQNs removed from YAML but not cleaned
  out of the pack.
- **(c) Legitimate cross-workspace references** — verbs declared under a
  CBU or shared file that the instrument-matrix pack references.

**Scale.** 34 FQNs. Per-FQN classification required.

### Break B-3 — 100 % of the Instrument Matrix verb surface is un-declared along all three v1.1 P1 axes

**Evidence.** Pilot plan §1.5: 39 `SemOsVerbOp` Rust impls across two
files (`rust/src/domain_ops/trading_profile.rs` — 36; `rust/crates/sem_os_postgres/src/ops/trading_matrix.rs` — 3), **none** of which expose
or carry `state_effect`, `external_effects`, or `consequence_tier`. At
the YAML side, no verb in the Instrument Matrix region carries a
`three_axis:` block today (P.1.a added the schema but declared zero
verbs).

**Scale.** 210 verbs to declare (per §Q4 locked decision — all 210,
prune later).

### Break B-4 — `trading_profile_lifecycle` is declared only in a constellation map, not as a formal DAG taxonomy artefact

**Evidence.** The one existing state machine lives inline in
`rust/config/sem_os_seeds/constellation_maps/trading_streetside.yaml`
on the `trading_profile` slot. It is not a first-class
`rust/config/sem_os_seeds/dag_taxonomies/*.yaml` artefact. The
validator's `transitions.dag` reference (P.1.c `UnknownDagReference`)
cannot be cross-checked until the DAG taxonomy YAML exists.

**Scale.** One state machine to lift.

### Break B-5 — Zero ScenarioIndex entries for the workspace

**Evidence.** Pilot plan §1.1 and §4 R-P4: `rust/config/scenario_index.yaml`
contains zero scenarios targeting the Instrument Matrix workspace.

**Scale.** This is a **testing-signal** gap, not strictly a DAG↔DSL break.
Runtime triage (P.5) has 39 oracle utterances only. Explicitly out of pilot
scope per §3; noted here for completeness so it isn't mistaken for an
architectural break requiring remediation.

---

## 2. Classification matrix

Each break mapped to its remediation ownership, v1.1 reference, and blocker
dependencies:

| Break | Class | Owner phase | v1.1 reference | Blocks |
|---|---|---|---|---|
| B-1 undeclared sub-DAGs | Taxonomy gap | **P.2** | P4, Tranche 1 Phase 1.3 (scoped) | P.3 (verbs can't reference undeclared states) |
| B-2 pack delta | Inventory hygiene | **P.3 first-task** | Tranche 2 Phase 2.A (scoped) | nothing directly; clarifies P.3's declaration scope |
| B-3 undeclared verbs | Declaration gap | **P.3** | P1, Tranche 2 Phase 2.A | P.4 (can't review what isn't declared) |
| B-4 trading_profile_lifecycle lift | Taxonomy reconciliation | **P.2** | P4, P5 directional reconciliation | P.3 transitions-block references |
| B-5 scenarios absent | Testing signal | **out of pilot** (estate scale) | P.7 runtime triage bounded | — |

Of the five breaks, **four are remediated inside existing pilot phases**
(B-1, B-2, B-3, B-4) and **one is out of scope** (B-5). The pilot plan
does not need new phases added — it needs these breaks made explicit as
phase-entry artefacts so P.2 and P.3 don't discover them mid-stream.

---

## 3. Pre-phase audit deliverables (new — fills the implicit→explicit gap)

Three small audit artefacts land **before** P.2 and P.3 start. They're
data artefacts, not code — collectively < 1 day of work.

### A-1 — Slot inventory ledger

**File:** `docs/todo/instrument-matrix-slot-inventory-2026-04-23.md`
(or equivalent).

**Content:** one row per slot across the three constellation maps. 18
rows total:

| Slot | Constellation map | Current state machine | Verbs that notionally mutate this slot | B-1 classification |
|---|---|---|---|---|
| trading_profile | trading_streetside | trading_profile_lifecycle | `trading-profile.approve` etc. | reconcile in P.2 (B-4) |
| settlement_pattern | instrument_template | none | `settlement-chain.*` lifecycle | new state machine in P.2 |
| ... | ... | ... | ... | ... |

Each row's classification column is one of:
- **`declare-stateless`** — slot is a container / projection; no state
  transitions; P.2 records this as an explicit stateless declaration.
- **`new-state-machine`** — P.2 produces a new state machine for this
  slot; states + transitions inferred from the verbs that mutate it.
- **`reconcile-existing`** — lift `trading_profile_lifecycle` (B-4).

**Effort:** ~half a day; mechanical — read constellation map YAMLs,
correlate against the 210-verb pack to find verbs that write to each
slot, then classify.

**Exit criterion for A-1:** every slot has a classification + (where
`new-state-machine`) a bullet list of states suggested by the mutating
verbs' action names.

### A-2 — Pack-delta disposition ledger

**File:** `docs/todo/instrument-matrix-pack-delta-2026-04-23.md`

**Content:** one row per unresolved FQN. 34 rows:

| FQN | Cause (a) registered programmatically | Cause (b) stale | Cause (c) cross-workspace | Disposition |
|---|---|---|---|---|
| `instrument-class.ensure` | ? | ? | ? | ? |
| ... | ... | ... | ... | ... |

For each FQN:
- Grep `sem_os_postgres::ops::build_registry()` + `ob_poc::domain_ops::extend_registry()` for its presence → if found, cause (a).
- Grep `config/verbs/` git history if the FQN appeared recently-then-removed → if found, cause (b).
- Check whether the FQN prefix exists in another workspace's pack → if so, cause (c).
- Disposition: **formalise-yaml** (declare in `config/verbs/`),
  **remove-from-pack** (pack entry is stale), or **legitimate-cross-ref**
  (leave as-is, note in findings).

**Effort:** ~2 hours; mechanical grep-based classification for most;
perhaps 5 edge cases need human judgment.

**Exit criterion for A-2:** every FQN has a disposition.

### A-3 — Unknown-DAG cross-check activation

**Change:** wire `ValidationContext::known_dags` from a live load of the
P.2 DAG taxonomy YAML (when it lands) so the validator's
`UnknownDagReference` error actually fires on typos. Today P.1.c skips
that check when the set is empty — harmless until P.2 exists, but must
be switched on when it does.

**Effort:** ~30 minutes when P.2 lands. Already factored into P.1.g
startup wiring (the startup hook reads the DAG taxonomy alongside the
verb YAMLs and feeds `known_dags` into the validator call).

---

## 4. Break-to-phase sequencing

```
     [A-1 slot inventory]                     [A-2 pack delta]
              │                                      │
              ▼                                      ▼
            P.2 DAG taxonomy YAML          P.3 declare 210 verbs
            (resolves B-1 + B-4)              (resolves B-3)
              │                                      │
              └────────────┬─────────────────────────┘
                           ▼
                    [A-3 wire known_dags]
                           │
                           ▼
                    P.4 tier review ──► P.5 runtime triage
                                              │
                                              ▼
                            (B-5 flagged as estate-scale follow-up
                             in P.9 findings; not remediated in pilot)
```

**Critical ordering** for the break set:
1. **A-1 and A-2 can run in parallel** (they touch different artefacts).
2. **P.2 must finish before P.3 starts** — the DAG taxonomy is the
   reference that P.3's `transitions.dag` fields cross-check against.
3. **A-3 activates at P.2 landing**, not at P.1 — the validator runs
   with an empty `known_dags` set until then, which is fine because no
   verb is yet declaring transitions referencing a DAG.

---

## 5. Risk adjustments to the pilot plan

Three risks from pilot §4 get sharpened by this break enumeration:

**R-P3 (Orphan-C surface) tightened.** The slot inventory A-1 will
classify all 18 slots up-front. Any slot marked `new-state-machine`
becomes P.2 work, NOT a P.3 orphan surprise. This shrinks P.3 orphan
processing significantly — maybe 0 Orphan-C if A-1 is done thoroughly,
versus the "significant surface" forecast in the pilot plan.

**R-P2 (34-verb pack-delta) tightened.** A-2 produces the disposition
up-front rather than deferring to P.3's first task. If 30+ of the 34
turn out to be `formalise-yaml` (cause (a) — programmatically registered
reference data), P.3's declaration scope rises from 210 to ~244.
Effort estimate for P.3 scales accordingly; findings note the delta.

**New risk R-P8 — Slot classification drift.** The slot inventory A-1
is judgement-dense: whether a slot is genuinely stateless vs whether
we haven't discovered its state yet is itself a judgement call. If
A-1's `declare-stateless` calls turn out wrong, P.3 finds verbs that
`transitions:` into a "stateless" slot — which the validator will
reject. *Mitigation:* A-1 exit criterion requires every stateless
slot to have a one-line "why stateless" rationale that references the
verbs that touch it (and demonstrates none of them are transitions).

---

## 6. Effort impact on pilot

| Artefact | Adds to phase | Effort |
|---|---|---|
| A-1 slot inventory | P.2 (pulls slot identification out of P.2 itself) | +0.5 day (no net new effort — P.2 would have done this anyway) |
| A-2 pack delta disposition | P.3 (pulls delta work out of P.3 first task) | +0.25 day (no net new) |
| A-3 known_dags wiring | P.1.g (startup gate slice already scheduled) | +0.1 day |
| **Total** | **+0.85 day** of reshuffling, 0 net new | |

Remediation for B-1/B-2/B-3/B-4 remains embedded in P.2+P.3 as the pilot
plan specifies. The audits only make the break set explicit so phase
execution is informed, not discovering.

---

## 7. Open questions for Adam

**RQ-1.** Should A-1 + A-2 land as commits (i.e. be produced before P.2
/ P.3 start) or only as living docs? Recommendation: **commit as
durable artefacts** — they become P.9 findings inputs and v1.1-candidate
evidence. Adds no schedule cost since the work has to happen somewhere.

**RQ-2.** B-5 (scenarios absent) is explicitly out of pilot, but should
we log the gap as a "Bucket 2 follow-up" entry per pilot P.5 / v1.1
Tranche 2 Phase 2.D bucket model, or as its own new work item? My
recommendation: **Bucket 2 follow-up** — it fits the existing category
("divergent but acceptable — queued for runtime-alignment backlog") and
doesn't need a new category.

---

## 8. Summary

Five breaks identified, four remediated inside existing pilot phases
(P.2 + P.3), one deferred to estate scale (B-5). Three small audit
artefacts (A-1 slot inventory, A-2 pack-delta ledger, A-3 known-DAG
wiring) make the break set explicit before phase execution at zero net
effort cost. No new phases required. R-P3 and R-P2 risks both tighten
— the remediation becomes measurable before P.3 starts, not discovered
during it.

The pilot plan as written is **sound**. This document is the missing
explicit-break inventory that backs it.

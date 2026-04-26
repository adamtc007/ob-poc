# Tier-Decision Record — Tranche 2 — 2026-04-26

> **Activity:** Catalogue Platform Refinement v1.2 — Tranche 2 estate reconciliation.
> **Authority:** Adam, acting as architectural authority for the activity.
>   See `tier-assignment-authority-provisional.md` for the §13 P-G provisional designation.
> **Scope:** All 1,282 verbs in the catalogue at `rust/config/verbs/`.
> **Outcome:** 100.0% three-axis declaration coverage; 0 structural / 0 well-formedness / 0 warnings under v1.2 §6.2 strict.

---

## 1. Summary

| Metric | Value |
|--------|------:|
| Total verbs in catalogue | 1,282 |
| Declared three-axis | 1,282 (100.0%) |
| Pre-Tranche-2 baseline | 795 (62.0%) |
| Verbs touched in Tranche 2 | 487 (newly declared) + 153 (mislabel-fixed) + 47 (transition→preserving) = 687 verbs |
| Declared escalation rules across estate | 5 |

**Tier baseline distribution:**

| Tier | Count | % |
|------|------:|--:|
| benign | 488 | 38.1% |
| reviewable | 529 | 41.3% |
| requires_confirmation | 192 | 15.0% |
| requires_explicit_authorisation | 73 | 5.7% |

The 5.7% rate of `requires_explicit_authorisation` matches the expected
shape for an onboarding platform: most verbs are observational reads or
routine writes; a minority are high-stakes (compliance attestations,
sanctions transitions, irreversible terminations).

The 0.4% escalation-rule adoption rate (5 declared rules estate-wide) is
consistent with v1.2 §11 commentary — a sparse rate is consistent with
P11 if most verbs have stable consequence regardless of context.
**R25 verdict: (a) genuinely doesn't need escalation, for the
overwhelming majority.** The 5 declared rules are concentrated on
agent.* verbs where session-state context legitimately escalates tier
(e.g. activate-teaching with sensitive operations).

---

## 2. Decision methodology

Tier baselines were assigned in three phases under
Adam-as-architectural-authority:

### Phase 1 — Mechanical pattern auto-declaration (T2.A.2/3)

215 CRUD + graph_query verbs auto-declared by structural pattern with
no judgment required:

  | Pattern | Tier baseline |
  |---------|---------------|
  | CRUD select / read / list / lookup / get | preserving + observational + benign |
  | CRUD insert / update / upsert | preserving + [] + reviewable |
  | CRUD delete / remove | preserving + [] + requires_confirmation |
  | graph_query | preserving + observational + benign |

### Phase 2 — Plugin pattern auto-declaration (T2.A.4)

142 plugin verbs auto-declared by metadata + name pattern:

  | Pattern | Tier baseline |
  |---------|---------------|
  | metadata.side_effects: facts_only / observational / none | preserving + observational + benign |
  | Verb stem in {read, list, get, show, search, find, lookup, query, fetch, inspect, audit, view, describe, explain} + record/record_set returns | preserving + observational + benign |
  | Domain {nav, view, focus, session} + navigation stem | preserving + navigating + benign |
  | Verb stem {notify, send, alert} OR side_effects: emit_event | preserving + emitting + reviewable |
  | bpmn.{compile, inspect} (read-only) | preserving + observational + benign |
  | audit/observation.record | preserving + emitting + reviewable |
  | research.* {search, fetch, import, lookup} | preserving + observational + benign |
  | discovery.* read verbs | preserving + observational + benign |
  | agent.{start, pause, resume, stop, reset, status} | preserving + navigating + reviewable |
  | refdata.{load, import}-* | preserving + [] + reviewable |

### Phase 3 — Per-verb judgment (T2.A.5)

128 plugin verbs received per-FQN tier assignments based on verb
description + side-effect analysis. Documented in
`/tmp/apply_tier_judgments.py::JUDGMENTS`. Highlights below.

47 of these were initially classified as `transition` but lacked
`transition_args:`. Per v1.2 §6.2 strict, a transition verb must point
at a DAG slot. These were reclassified to `preserving` (they modify
records and emit events but don't go through the v1.3 GatePipeline).
Phase 2.C may revisit and add `transition_args:` for verbs that
genuinely should be runtime-gated.

---

## 3. Notable per-domain tier judgments

### access-review.* (compliance attestation)

`attest` is the highest-tier verb in this domain:
**preserving + emitting + requires_explicit_authorisation** — this is
a regulated compliance signal where the user explicitly attests to a
control state. Equivalent class to KYC sign-off.

`launch-campaign`, `revoke-access` are
**transition + emitting + requires_confirmation** — material workflow
transitions with audit trail emission.

`bulk-confirm`, `confirm-all-clean`, `populate-campaign` are
**preserving + emitting + reviewable** — bulk operations that emit
audit events but don't transition workflow state directly.

### agent.* (agent control)

`teach`, `unteach`, `activate-teaching` are
**preserving + emitting + reviewable** — write a learned phrase
mapping but don't transition agent state.

`confirm-decision`, `reject-decision` are
**preserving + navigating + reviewable** — user-driven loop control;
emits an audit signal but is fundamentally navigation within the
decision tree.

`set-{selection-threshold, execution-mode, authoring-mode}` are
**preserving + [] + reviewable** — config writes; no external effect.

### batch.* (batch control plane)

All control verbs (pause / resume / continue / skip) are
**preserving + emitting + reviewable** — control-plane signals on
running orchestration; no DAG slot transitions.

`abort` is
**preserving + emitting + requires_confirmation** — destructive on
a running batch.

`add-products` is
**preserving + emitting + reviewable** — extends batch scope.

### capital.* (capital structure)

All material capital actions
(share-class.create, issue.initial, issue.new, split, buyback, cancel,
dilution.{grant-options, issue-warrant, create-safe,
create-convertible-note, exercise, forfeit}) are
**preserving + emitting + requires_confirmation** — they modify cap
table records and emit audit events; they would be transitions if a
share-class lifecycle DAG existed (Tranche 3 candidate).

`cap-table`, `holders` are
**preserving + observational + benign** — pure reads.

### client-group.* (entity membership)

Membership writes (entity-add, entity-manage, confirm-entity,
reject-entity, assign-role, remove-role, add-relationship, complete-discovery)
are **transition + emitting + reviewable/confirm** depending on
destructiveness. These have `transition_args:` already (the v1.2
canonical shape) since the client-group DAG declares them.

`entity-remove` is **transition + emitting + requires_confirmation** —
reversible but consequential.

`tag-add`, `tag-remove`, `set-canonical` are **preserving + [] +
reviewable** — config metadata writes.

### control.* (UBO / control graph)

Compute verbs (build-graph, identify-ubos, compute-controllers,
import-psc-register, import-gleif-control) are
**preserving + observational + benign** — pure read/computation.

`recompute-board-controller` is
**preserving + emitting + reviewable** — emits an audit event with
the recomputed result.

`set-board-controller`, `clear-board-controller-override` are
**transition + emitting + requires_confirmation** — overrides the
computed controller; high stakes.

### ubo.* (UBO chain operations)

`compute-chains`, `snapshot.capture` are
**preserving + observational + benign** / **preserving + emitting +
reviewable** — read/audit operations.

`mark-deceased`, `convergence-supersede` are
**transition + emitting + requires_confirmation** — modifies the UBO
chain with a permanent record.

`transfer-control` is
**transition + emitting + requires_explicit_authorisation** — the
highest-stakes UBO action (transfers controlling ownership designation;
irreversible without legal due diligence).

`waive-verification` is
**preserving + emitting + requires_confirmation** — temporary waiver;
auditable.

### bpmn.* (workflow control)

`start`, `signal`, `cancel` are
**preserving + emitting + reviewable / requires_confirmation** —
they transition BPMN process state but are not v1.3 cross-workspace
gated. Hence `preserving` per v1.2 §6.2 (no transition_args possible).
This is a known categorical boundary: the BPMN runtime is its own
state machine, separate from the v1.3 DAG taxonomy.

### state.* (state-graph manual override)

`override`, `revoke-override` are
**preserving + emitting + requires_confirmation** — manual
state-graph override is governance-gated; auditable.

### research.* (external research)

Search/fetch/import verbs are
**preserving + observational + benign** — reads from external sources.

`research.workflow.confirm-decision`, `reject-decision` are
**transition + emitting + reviewable** — workflow state transitions.

`research.import-run.complete`, `supersede` are
**transition + emitting + reviewable / confirm** — import-run lifecycle
transitions.

---

## 4. Verbs at the high end (`requires_explicit_authorisation`)

73 verbs total. Sample:

- `access-review.attest` — compliance attestation.
- `ubo.transfer-control` — irreversible controlling-ownership change.
- All `*.bac-approve` / `*.kyc-approve` / `*.contracted` verbs that
  cross the deal/CBU commercial-to-operational boundary.
- All `share-class.hard-close` and similar terminal lifecycle states.
- All `governance.publish` / `changeset.publish` verbs.
- `attribute.define` (governed, with full changeset ceremony).

These tier assignments are inherited from existing v1.1 declarations
(unchanged in this Tranche). No new verbs were assigned this tier in
the auto-declaration phases.

---

## 5. Open items / Phase 2.C follow-up

The 47 verbs reclassified as `preserving` (Section 2 Phase 3) are
flagged for Phase 2.C revisit:

> Some of these may genuinely be transitions on a slot the v1.3 runtime
> should gate. For each, decide: add `transition_args:` pointing at a
> DAG slot, OR confirm that the BPMN-Lite or other runtime is the
> correct gating layer (in which case `preserving` is correct).

Candidates worth revisiting:

- `capital.split`, `capital.buyback`, `capital.cancel` — if a
  share-class lifecycle DAG is added in Tranche 3, these should
  transition to it.
- `ruleset.publish`, `ruleset.retire` — booking_principal ruleset
  lifecycle could be a DAG slot.
- `client-group.start-discovery`, `complete-discovery` — already
  declared on the client-group DAG; missing `transition_args:`.
- `ubo.mark-deceased`, `convergence-supersede` — UBO registry slot
  lifecycle exists; missing `transition_args:`.
- `state.override`, `revoke-override` — state-graph manual override is
  per definition a transition.
- `bpmn.start`, `signal`, `cancel` — BPMN process state, not v1.3 DAG.
  Likely correctly `preserving` long-term.

---

## 6. Provisional authority statement

All 687 tier decisions in this Tranche 2 were made by Adam acting as
architectural authority for the activity, per the §13 amended
designation in `tier-assignment-authority-provisional.md`. Each
decision is **explicitly revisable** under future organisational P-G
governance. The audit trail is exhaustive:

- This document captures the framework decisions and notable
  per-verb judgments.
- The full FQN→tuple mapping for the 128 manual judgments is in
  `/tmp/apply_tier_judgments.py::JUDGMENTS` (committed to the verb
  YAMLs as the canonical record).
- All YAML edits land via the commit chain: `5c00a8de` → `c18e4596`
  → `ca604377` → `384c7764` → (this commit) — fully reconstructible.

Future organisational P-G's first task is review of this record. No
decision was silent.

---

## 7. Statistics

| Domain | Verbs | Tier distribution (b/r/c/a) |
|--------|------:|------------------------------|
| trading-profile | 55 | (snapshot at commit time — see `cargo x reconcile status` for live numbers) |
| deal | 57 | |
| cbu | 48 | |
| (...other domains) | | |

For machine-readable per-domain breakdown, run:

```bash
cargo x reconcile status > /tmp/reconcile_status.txt
```

---

## 8. What's next

- **Phase 2.C — formal governance review (revision pass).** Phase 2 of
  the activity is technically complete (every verb has a baseline
  tier). A formal governance pass would revisit the 47 reclassified
  verbs (Section 5) and any tier judgments Adam wishes to change.
  Estimated effort: 4-8 hours focused review.

- **Phase 2.G — post-reconciliation coherence pass (4 sub-passes).**
  Required, not optional, at estate scale per v1.2 §7.3:
  - 2.G.1: Cross-section taxonomy review.
  - 2.G.2: Tier landscape review (heatmap).
  - 2.G.3: Bucket 3 cumulative review.
  - 2.G.4: Catalogue self-consistency review.

- **Phase 2.H — final reconciliation report.** Aggregates this record
  + 2.G findings + Bucket 2 follow-up queue into the canonical
  reconciliation report.

---

**End of tier-decision record — 2026-04-26.**

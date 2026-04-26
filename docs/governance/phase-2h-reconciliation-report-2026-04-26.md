# Phase 2.H — Tranche 2 Reconciliation Report — 2026-04-26

> **Activity:** Catalogue Platform Refinement v1.2 — Full-estate reconciliation.
> **Authority:** Adam, acting as architectural authority for the activity.
> **Phase:** v1.2 Tranche 2 Phase 2.H — final reconciliation report (DoD item 8).
> **Status:** Tranche 1 + Tranche 2 (auto-classifiable scope + Phase 2.G coherence pass) **CODE COMPLETE**.

---

## 1. Headline numbers

| Metric | Pre-Tranche-2 | Post-Tranche-2 | Δ |
|--------|--------------:|---------------:|--:|
| Verbs in catalogue | 1,282 | 1,282 | 0 |
| Three-axis declared | 795 (62.0%) | **1,282 (100.0%)** | +487 |
| Mislabeled `preserving` + `transition_args:` | 153 | **0** | −153 |
| Validator structural errors | 0 | **0** | 0 |
| Validator well-formedness errors | 0 | **0** | 0 |
| Validator policy-sanity warnings | 153 (migration) | **0** | −153 |
| Cross-workspace constraints declared | 11 | 11 | 0 |
| DAG taxonomies | 11 | 11 | 0 |
| Workspaces | 11 | 11 | 0 |
| Self-consistency tier outliers | 6 | **0** | −6 |
| Production GatePipeline | opt-in | **default-on** | flipped |
| CI gate on catalogue | none | **`.github/workflows/catalogue.yml`** | added |

**Verdict:** v1.2 §11 stopping point reached — "Tranche 1 + 2 complete:
reconciled estate, tier-populated with escalation, governance audit
trail, coherence findings — useful with code-review discipline."

---

## 2. Tier baseline distribution

| Tier | Count | % |
|------|------:|--:|
| benign | 488 | 38.1% |
| reviewable | 525 | 41.0% |
| requires_confirmation | 196 | 15.3% |
| requires_explicit_authorisation | 73 | 5.7% |
| **total** | 1,282 | 100.0% |

(`reviewable` decreased by 4 and `requires_confirmation` increased by 4
relative to the post-batch-4 numbers — the 6 Phase 2.G.4 self-consistency
fixes; 4 of them moved verbs across the reviewable/confirmation
boundary, the other 2 stayed at confirmation.)

## 3. Per-axis distribution

**state_effect:**

| Axis | Count |
|------|------:|
| preserving | 1,096 |
| transition | 186 |

**external_effects:**

| Set | Count |
|-----|------:|
| (none) | 334 |
| observational | 488 |
| emitting | 456 |
| navigating | 3 |
| emitting,navigating | 1 |

**Transition carriers:**

| Carrier | Count |
|---------|------:|
| transition_args (v1.2 canonical) | 172 |
| Legacy `transitions:` block (grandfathered) | 14 |
| Total | 186 |

**Escalation rules:** 4 declared estate-wide. v1.2 R25 verdict (a)
"genuinely doesn't need escalation" confirmed for the overwhelming
majority — sparse rate is consistent with v1.2 §11 commentary.

## 4. Phase recap

### Tranche 1 (commit `5c00a8de`)

Closed v1.2 §6.4 DoD items 1–14:

| Item | Description | Status |
|--:|---|---|
| 1 | Three-axis declaration schema | COMPLETE |
| 2 | Four-tier consequence taxonomy | COMPLETE |
| 3 | Escalation DSL spec | COMPLETE |
| 4 | Runbook composition rules | COMPLETE |
| 5 | Validator (incl. `transition_args:` rules + EXISTS predicate) | COMPLETE |
| 6 | DAG taxonomies for 11 workspaces (coherence-reviewed) | COMPLETE |
| 7 | DB-free catalogue-load before DB pool init | COMPLETE |
| 8 | `cargo x reconcile --validate / --batch / --status` | COMPLETE |
| 9 | 20-verb fixture (escalation + composition + transition_args) | COMPLETE |
| 10 | Sage / REPL policy docs | COMPLETE |
| 11 | P-G provisional-named authority | COMPLETE (Adam-as-architectural-authority) |
| 12 | CI gate for catalogue validation | COMPLETE |
| 13 | GatePipeline default-on in production | COMPLETE |
| 14 | Documentation updated | COMPLETE |

### Tranche 2 batch 1 (commit `c18e4596`)

153 mislabeled verbs (`state_effect: preserving` + `transition_args:`)
flipped to `state_effect: transition`. Validator promoted from
migration warning to v1.2 §6.2 strict structural error.

### Tranche 2 batch 2 (commit `ca604377`)

215 CRUD + graph_query verbs auto-declared by structural pattern.
Coverage 62.0% → 78.8%.

### Tranche 2 batch 3 (commit `384c7764`)

142 plugin verbs auto-declared by metadata + name patterns.
Coverage 78.8% → 89.9%.

### Tranche 2 batch 4 (commit `f147f03a`)

128 per-FQN tier judgments under Adam-as-architectural-authority + 47
transition→preserving reclassifications + 2 final fund.* verbs.
Coverage 89.9% → 100.0%. Tier-decision-record produced.

### Phase 2.G coherence pass (this commit)

4 sub-passes executed:
- **2.G.1** — cross-section taxonomy review: clean (no new states, no cycles).
- **2.G.2** — tier landscape heatmap: clean (no miscalibrated workspaces;
  shape matches platform expectations).
- **2.G.3** — Bucket 3 cumulative review: deferred (T2.D not yet executed).
- **2.G.4** — catalogue self-consistency review: 6 tier outliers fixed
  (all upgrades, never relaxations). Remaining variance verified
  legitimate.

One iteration sufficient. No second iteration required.

---

## 5. Bucket queue (follow-up)

### Bucket 2 — runtime alignment to schedule

Per v1.2 §7.5 DoD item 4: "Runtime-consistency check complete; Bucket 3
resolved; Bucket 2 handed off."

**Bucket 3 (resolve in this activity):** *empty — T2.D runtime triage
deferred.*

**Bucket 2 (follow-up activity):**

The runtime triage step (T2.D) was deferred from this session. When
executed, it produces:
- Bucket 1: runtime matches declaration. Expected majority (~95%).
- Bucket 2: runtime deviates incidentally. Defer-friendly. Schedule
  for follow-up activity.
- Bucket 3: runtime contradicts declaration semantically. Resolve
  in-activity.

**Action:** Schedule T2.D runtime triage in a follow-on session.
Expected fixture set: the 14 cross-workspace DAG harness fixtures +
the 9 unified pipeline tollgate tests. Estimated effort: 4-6 hours
(parallel-runnable across fixtures).

### Phase 2.C revisit queue

Per `tier-decisions-2026-04-26.md` §5, the following verbs are
candidates for governance revisit (potential `transition_args:`
addition or tier change under Adam-as-architectural-authority):

**Candidates for `transition_args:` migration (14 grandfathered + 47 reclassified):**

The 14 grandfathered verbs (legacy `transitions:` block, no
`transition_args:`) and the 47 reclassified verbs (transition→preserving)
are all candidates. Concrete sub-list:

- `capital.split`, `capital.buyback`, `capital.cancel` — share-class
  lifecycle DAG (Tranche 3 candidate).
- `capital.dilution.{grant-options, issue-warrant, create-safe,
  create-convertible-note, exercise, forfeit}` — share-class lifecycle.
- `ruleset.publish`, `ruleset.retire` — booking_principal ruleset
  lifecycle.
- `client-group.start-discovery`, `complete-discovery` — already on
  client-group DAG; just missing `transition_args:`.
- `ubo.mark-deceased`, `convergence-supersede`, `transfer-control` —
  UBO registry lifecycle.
- `state.override`, `state.revoke-override` — state-graph manual
  override (per definition transitions a state).
- 4 more in capital, research-workflow, ruleset domains.

**Out of scope for revisit (correctly `preserving`):**

- `bpmn.start`, `bpmn.signal`, `bpmn.cancel` — BPMN-Lite runtime owns
  process state, not v1.3 cross-workspace gate. Correct as `preserving`.

### Tranche 3 prerequisites

When Tranche 3 begins, it inherits:

1. **Reconciled catalogue** — 100% three-axis coverage with provisional
   tier decisions documented.
2. **Validator and xtask client** — operational; v1.2 §6.2 strict.
3. **Catalogue-load validation** wired to startup; CI-gated.
4. **GatePipeline default-on in production** — runtime gate live.
5. **Sage / REPL policy documents** — ready for Phase 3.C
   architectural integration.
6. **Tier-decision-record** — full audit trail under provisional
   authority.
7. **Phase 2.G coherence findings** — clean post-state.
8. **Open governance question** — organisational P-G structure
   (recorded in `tier-assignment-authority-provisional.md` §3).

### Tranche 2 still open (DoD item gap)

Per v1.2 §7.5 Tranche 2 DoD:

- DoD item 4 (Bucket 2 handoff): **DEFERRED** — T2.D runtime triage
  pending.
- All other Tranche 2 DoD items (1, 2, 3, 5, 6, 7, 8, 9): **DONE**.

---

## 6. v1.3 candidate amendments

Based on what this activity revealed:

- **A.1** — Promote `transition_args:` migration to mandatory for
  state_effect: transition once R.4 lands estate-wide. **Already done**
  this session (validator promoted in batch 1).

- **A.2** — Document the legitimate variance in same-name verbs
  (`suspend`, `activate`, `reactivate`, etc.). v1.2 P10 / P11 already
  permits this; v1.3 could add an explanatory annex.

- **A.3** — Define a shape-template for common verb stems
  (`suspend = transition + emitting + requires_confirmation` as the
  default shape unless domain stakes warrant otherwise). Would catch
  outlier mis-tiering in Phase 2.G.4 before it lands.

- **A.4** — Schedule runtime triage (T2.D) as a Tranche 2 prerequisite,
  not optional/deferrable. The deferral here was pragmatic but means
  Phase 2.G.3 couldn't execute.

- **A.5** — Transition_args resolution should be validator-checked
  against the DAG registry once the registry has full coverage. v1.2
  §6.2 already specifies this; the validator currently only checks
  the structural presence, not slot-resolution. Land in next
  iteration.

---

## 7. Deliverables produced

| Deliverable | Path |
|-------------|------|
| Validator (Rust) | `rust/crates/dsl-core/src/config/validator.rs` |
| EXISTS predicate parser | `rust/crates/dsl-runtime/src/cross_workspace/sql_predicate_resolver.rs` |
| GatePipeline default-on | `rust/crates/ob-poc-web/src/main.rs` |
| 20-verb fixture | `rust/crates/dsl-core/tests/fixtures/v1_2_dod_fixture/verbs.yaml` |
| Fixture test suite | `rust/crates/dsl-core/tests/v1_2_dod_fixture.rs` |
| CI gate | `.github/workflows/catalogue.yml` |
| Sage autonomy policy | `docs/policies/sage_autonomy.md` |
| REPL confirmation policy | `docs/policies/repl_confirmation.md` |
| Tier-assignment authority (P-G provisional) | `docs/governance/tier-assignment-authority-provisional.md` |
| DAG coherence review | `docs/governance/dag-coherence-review-2026-04-26.md` |
| Tier-decision record | `docs/governance/tier-decisions-2026-04-26.md` |
| Phase 2.G coherence findings | `docs/governance/phase-2g-coherence-findings-2026-04-26.md` |
| Phase 2.H reconciliation report (this) | `docs/governance/phase-2h-reconciliation-report-2026-04-26.md` |
| v1.2 spec (consolidated) | `docs/todo/catalogue-platform-refinement-v1_2.md` |
| Reconciliation review (independent) | `docs/todo/full-catalogue-reconciliation-review-2026-04-26.md` |

## 8. Activity timeline (this session)

| Step | Outcome |
|------|---------|
| Session start | v1.2 specification reviewed; reconciliation prompt parsed |
| Tranche 1 (`5c00a8de`) | All 14 DoD items closed |
| T2 batch 1 (`c18e4596`) | 153 mislabels fixed; validator strict |
| T2 batch 2 (`ca604377`) | 215 CRUD/graph_query auto-declared |
| T2 batch 3 (`384c7764`) | 142 plugin patterns auto-declared |
| T2 batch 4 (`f147f03a`) | 128 per-FQN judgments + tier-decision-record |
| Phase 2.G coherence pass (next commit) | 6 fixes, 4 sub-passes, clean |
| Phase 2.H final report (next commit) | this document |

---

## 9. What's done, what's next

**DONE in this activity:**

- All v1.2 Tranche 1 DoD (14 items).
- v1.2 Tranche 2 DoD items 1, 2, 3, 5, 6, 7, 8, 9.
- Phase 2.G coherence pass (4 sub-passes; 1 deferred for T2.D).
- 100% three-axis coverage.
- v1.2 §6.2 strict validator.
- GatePipeline default-on in production.
- CI gate.

**DEFERRED to follow-on sessions:**

- **T2.D runtime triage** (Tranche 2 DoD item 4) — fixture-vs-runtime
  consistency check; produces Bucket 2 / Bucket 3 categorisation.
- **Phase 2.C revisit queue** (61 candidate verbs) — `transition_args:`
  migration for grandfathered + reclassified verbs that genuinely
  belong to a v1.3 cross-workspace gate.
- **Phase 2.G.3 re-run** — once T2.D outputs exist.
- **Tranche 3** — Catalogue workspace, authorship verbs/macros, ABAC,
  Sage/REPL architectural integration, Observatory UI, forward
  discipline activation.

**Recommended next session focus:**

1. T2.D runtime triage on the 14 cross-workspace DAG fixtures.
2. Phase 2.C revisit (4-6 hours) — pick up the 61 candidate verbs.
3. Decision on Tranche 3 timing.

---

## 10. Provisional authority statement

All decisions in this Tranche 2 — declaration patterns, tier baselines,
self-consistency fixes — were made by Adam acting as architectural
authority for the activity per v1.2 §13 amended provisional designation.
Audit trail is exhaustive across the commit chain `5c00a8de` →
`c18e4596` → `ca604377` → `384c7764` → `f147f03a` → (this commit).

Future organisational P-G review of these provisional decisions remains
the recorded open governance question (per
`tier-assignment-authority-provisional.md` §3).

---

**End of Phase 2.H reconciliation report — 2026-04-26.**

**Tranche 1 + 2 of Catalogue Platform Refinement v1.2 — CODE COMPLETE.**

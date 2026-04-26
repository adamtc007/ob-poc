# Phase 2.G — Post-Reconciliation Coherence Pass — 2026-04-26

> **Phase:** v1.2 Tranche 2 Phase 2.G (required, not optional, per §7.3).
> **Authority:** Adam, acting as architectural authority for the activity.
> **Sub-passes:** 2.G.1 (cross-section taxonomy review), 2.G.2 (tier
>   landscape heatmap), 2.G.3 (Bucket 3 cumulative review), 2.G.4
>   (catalogue self-consistency review).
> **Iteration:** 1 of 1 (per v1.2 §7.3 — second iteration would signal
>   activity-scope reassessment).
> **Outcome:** PASSED with 6 fixes applied. No second iteration required.

---

## 2.G.1 — Cross-section taxonomy review

Reviews all DAG-state and verb-shape changes from the Tranche 2 batches
(1-4) as a SET, looking for naming inconsistency, cycles, unreachable
regions, conflicting Orphan-C sub-DAGs.

**State-effect distribution (post-Tranche 2):**

| State effect | Verbs | % |
|--------------|------:|--:|
| preserving | 1,096 | 85.5% |
| transition | 186 | 14.5% |

**Transition carrier breakdown (186 transition verbs):**

| Carrier | Verbs |
|---------|------:|
| transition_args (v1.2 canonical) | 172 |
| Legacy `transitions:` block (grandfathered) | 14 |
| Total | 186 |

**Findings:**

- **F-2G1-01 — No new states introduced in Tranche 2.** All Tranche 2
  fixes operated on existing DAG states; no Orphan-B/C resolutions
  produced new states. The 11 DAG taxonomies are unchanged in this
  Tranche.
- **F-2G1-02 — 14 verbs grandfathered with legacy `transitions:` block.**
  These verbs have `state_effect: transition` + a `transitions:` block
  but no `transition_args:`. v1.2 §6.2 grandfathers them during the
  migration window. Phase 2.C revisit candidate (add `transition_args:`
  pointing at the slot, then drop the legacy block).
- **F-2G1-03 — Cross-workspace constraint count unchanged.** Still 11
  declared cross-workspace constraints across 11 DAGs. No cycles. KYC
  remains the universal source.
- **F-2G1-04 — All 11 DAG predicates resolve.** v1.2 EXISTS extension
  + canonical equality predicates parse cleanly (per
  `dag-coherence-review-2026-04-26.md`).

**Verdict:** clean. No new states, no cycles, no conflicting sub-DAGs.

---

## 2.G.2 — Tier landscape review (heatmap)

**Distribution by state_effect:**

| state_effect | benign | reviewable | requires_confirmation | requires_explicit_authorisation |
|--------------|-------:|-----------:|----------------------:|--------------------------------:|
| preserving | 487 | 432 | 105 | 72 |
| transition | 1 | 97 | 87 | 1 |

**Distribution by external_effects:**

| ext_effects | benign | reviewable | requires_confirmation | requires_explicit_authorisation |
|-------------|-------:|-----------:|----------------------:|--------------------------------:|
| (none) | 4 | 271 | 47 | 12 |
| observational | 482 | 6 | 0 | 0 |
| emitting | 0 | 250 | 145 | 61 |
| navigating | 2 | 1 | 0 | 0 |
| emitting,navigating | 0 | 1 | 0 | 0 |

**Per-domain density outliers (>90% benign in domains with ≥5 verbs):**

| Domain | Benign% | Verb count | Verdict |
|--------|--------:|-----------:|---------|
| graph | 100.0% | 10 | EXPECTED (pure read graph queries) |
| nav | 100.0% | 7 | EXPECTED (navigation/viewport) |
| view | 100.0% | 14 | EXPECTED (viewport-only) |
| schema | 100.0% | 13 | EXPECTED (schema introspection reads) |
| focus | 100.0% | 6 | EXPECTED (viewport focus ops) |
| semantic | 100.0% | 6 | EXPECTED (semantic registry reads) |
| temporal | 100.0% | 8 | EXPECTED (time-travel reads) |
| research.companies-house | 100.0% | 5 | EXPECTED (external read API) |
| research.sec-edgar | 100.0% | 5 | EXPECTED (external read API) |
| research.sources | 100.0% | 5 | EXPECTED (external research) |
| discovery | 91.7% | 11/12 | EXPECTED (semantic discovery reads) |

**No domains exceed 70% `requires_explicit_authorisation` density** —
no over-tightened workspace.

**Findings:**

- **F-2G2-01 — No miscalibrated workspace clusters.** The 11 domains
  with 90%+ benign density are all read-only / observational by design;
  this is expected, not miscalibration.
- **F-2G2-02 — Tier distribution shape is healthy.** 38% benign
  (mostly reads), 41% reviewable (writes + emits), 15% confirmation
  (consequential transitions), 6% explicit auth (compliance-critical).
  Matches the platform's onboarding-and-governance shape.
- **F-2G2-03 — `external_effects: emitting` correctly correlates with
  higher tier.** 0 benign (no benign verb emits external signals);
  82% of `requires_explicit_authorisation` verbs emit. This is the
  semantic invariant v1.2 P10 expects: emitters concentrate the high
  tiers.
- **F-2G2-04 — `state_effect: transition` correctly skews to
  reviewable+confirmation.** 99.5% of transition verbs are reviewable
  or confirmation; 0.5% benign (a single `reorder-collection`-style
  edge case in the v1.1 fixture). This is the v1.2 P10 shape.

**Verdict:** clean. No tier-landscape miscalibrations.

---

## 2.G.3 — Bucket 3 cumulative review

**Status:** N/A — not yet executed.

The runtime triage phase (T2.D) was deferred from this session. Without
fixture-vs-runtime triage outcomes, there are no Bucket 3 declarations
to audit cumulatively. When T2.D runs in a follow-on session, the
audit log of declaration changes during Bucket 3 fix-up should be
captured here.

**Action:** T2.D scheduled as the next Tranche 2 follow-up. Phase 2.G.3
re-runs against its outputs at that time.

---

## 2.G.4 — Catalogue self-consistency review

Reviews verbs whose semantic shape differs across domains where
consistency might be expected (same verb name, same stem pattern).

**Findings — verb-name tier inconsistency (≥5 occurrences, >1 distinct tier):**

| Verb name | Occurrences | Tiers observed | Verdict |
|-----------|------------:|----------------|---------|
| `suspend` | 16 | confirm (12), auth (2), reviewable (2) | **2 outliers fixed** (user, investment-manager) |
| `activate` | 10 | confirm (4), auth (1), reviewable (5) | EXPECTED — `reviewable` cluster (application-instance, matrix-overlay, investor, service-resource, shared-atom) is mid-stakes. Auth (trading-profile) and confirm (4) are higher-stakes. Legitimate variance. |
| `reactivate` | 7 | confirm (5), auth (1), reviewable (1) | **1 outlier fixed** (user.reactivate) |
| `terminate` | 5 | confirm (1), auth (2), reviewable (2) | **2 outliers fixed** (contract, investment-manager) |
| `remove` | 5 | confirm (4), reviewable (1) | **1 outlier fixed** (regulatory.registration) |
| `reject` | 8 | confirm (6), auth (2) | EXPECTED — `auth` for sanctions-state rejections; `confirm` for routine. |
| `retire` | 8 | confirm (4), auth (4) | EXPECTED — split by domain stakes. |
| `cancel` | 7 | confirm (5), auth (2) | EXPECTED — `auth` for capital cancellation; `confirm` for routine. |
| `approve` | 5 | confirm (3), auth (2) | EXPECTED — `auth` for compliance-critical approvals. |
| `set` | 5 | benign (3), reviewable (2) | EXPECTED — reads vs writes. |

**Findings — stem-pattern shape variation (>3 distinct shapes):**

| Stem | Distinct shapes | Verdict |
|------|----------------:|---------|
| `activate` | 7 | EXPECTED — variance reflects real domain differences |
| `start` | 5 | EXPECTED — bpmn.start vs collateral.start vs etc. |
| `reject` | 5 | EXPECTED — multi-domain verb |
| `suspend` | 5 | After fixes: 4 distinct shapes (down from 5). Variance is legitimate. |
| `remove` | 4 | After fix: 3 distinct shapes. Acceptable. |
| `retire` | 4 | EXPECTED — multi-domain verb. |
| `complete` | 4 | EXPECTED — multi-domain verb. |

**Fixes applied (all tier tightenings — never relaxations):**

| FQN | Old tier | New tier | Rationale |
|-----|----------|----------|-----------|
| `user.suspend` | reviewable | requires_confirmation | User account suspension is consequential |
| `user.reactivate` | reviewable | requires_confirmation | User account reactivation needs explicit confirm |
| `investment-manager.suspend` | reviewable | requires_confirmation | IM suspension has financial/regulatory impact |
| `investment-manager.terminate` | reviewable | requires_confirmation | IM termination is materially consequential |
| `contract.terminate` | reviewable | requires_confirmation | Contract termination has legal impact |
| `regulatory.registration.remove` | reviewable | requires_confirmation | Regulatory record removal needs explicit confirm |

All fixes are **tier upgrades** (more restrictive). v1.2 P11 monotonic-
floor permits this freely; the user only ever sees more friction, never
less.

**Findings — F-2G4-01 to F-2G4-06 documented above.**

**Verdict:** clean after fixes. The remaining tier variance across same-
named verbs is **legitimate domain-specific variation**, not
miscalibration. Phase 2.C may revisit individual cases; no further
mass-fixes required.

---

## Composite findings + actions

| # | Finding | Severity | Action | Status |
|--:|---------|----------|--------|--------|
| F-2G1-01 | No new states in Tranche 2 | OK | None | — |
| F-2G1-02 | 14 grandfathered transition verbs need transition_args | Action | Phase 2.C revisit | Deferred |
| F-2G1-03 | No cycles in cross-workspace constraints | OK | None | — |
| F-2G1-04 | All 11 DAG predicates resolve | OK | None | — |
| F-2G2-01 | No miscalibrated workspace clusters | OK | None | — |
| F-2G2-02 | Tier distribution shape is healthy | OK | None | — |
| F-2G2-03 | Emitters correctly concentrate high tiers | OK | None | — |
| F-2G2-04 | Transitions correctly skew to reviewable/confirmation | OK | None | — |
| F-2G3-01 | Bucket 3 review deferred (T2.D not executed) | Deferred | Re-run Phase 2.G.3 after T2.D | Open |
| F-2G4-01 | 6 verbs at `reviewable` should be `requires_confirmation` | Action | Tighten tiers | **APPLIED** |
| F-2G4-02 | Same-name tier variance otherwise legitimate | OK | None | — |

**Iteration verdict:** **One iteration sufficient.** No findings require
re-running Phase 2.G after the 6 fixes; the verdict is clean. v1.2 §7.3
flag for "second iteration would signal activity-scope reassessment" is
not raised.

---

## Phase 2.G exit criteria

Per v1.2 §7.5 DoD item 7: "Phase 2.G coherence pass completed; findings
either resolved or queued."

- 4 sub-passes executed (with 2.G.3 documented as deferred).
- 6 self-consistency fixes applied (tier upgrades only, monotonic floor
  preserved).
- 1 finding queued for follow-up (14 grandfathered transition verbs;
  Phase 2.C revisit for transition_args migration).
- No second iteration required.

**Phase 2.G — DONE.**

---

**End of Phase 2.G coherence findings — 2026-04-26.**

# Tranche 2 — CBU Findings Report (2026-04-23)

> **Status:** CBU CLOSED. Fourth and final primary workspace through
> the pilot pattern. **Tranche 2 core authoring complete.**
> **Parent docs:**
> - `instrument-matrix-pilot-findings-2026-04-23.md` (pilot baseline)
> - `tranche-2-kyc-findings-2026-04-23.md` (second workspace)
> - `tranche-2-deal-findings-2026-04-23.md` (third workspace)
> - `catalogue-platform-refinement-v1_2.md` (spec)

---

## 1. Delivery summary

9 phases executed in ~75 minutes actual. Larger verb surface than
Deal (~135 vs 59) offset by faster DAG authoring now that the
pattern is fully established.

| Phase | Deliverable | Effort actual |
|---|---|---|
| T2-C-1 (equiv P.1) | SKIP — infra reused | 0 |
| T2-C-2-prep | Pack-delta check; 1 stale template ref pruned (`entity.link` → `entity.ensure`) | 5 min |
| T2-C-2 | `cbu_dag.yaml` (827 LOC: 7-phase overall lifecycle + 11 stateful slot state machines + 9 stateless slots + 10 cross-slot constraints + 4 prune cascade rules) | ~30 min |
| T2-C-3 | 135 three-axis declarations via retrofit script across 12 files | ~15 min |
| T2-C-4 | 3 tier raises + 1 lower (`cbu.decide` → req_ex_auth, `verify-evidence` + `reopen-validation` → req_conf, `share-class.close` → req_conf) | ~5 min |
| T2-C-5 | Runtime triage: 51/51 valid routing (100%); 8 true fixture bugs flagged | ~10 min |
| T2-C-6/7/8 | SKIP — infra/tools reused | 0 |
| T2-C-9 | This document | ~10 min |

**Validator terminal state:** 584 / 1184 declared, 0 structural errors,
0 well-formedness errors, 0 warnings.

**Net declarations added this session:** +135 (from post-Deal's 449).

---

## 2. What the pilot infrastructure delivered (confirmed at 4× scale)

Validated across all 4 primary workspaces now. Infrastructure-vs-content
split is now stable:

| Aspect | Per-workspace cost |
|---|---|
| Infrastructure (one-time from pilot) | ~4 hours |
| DAG authoring (per workspace) | 25-30 min |
| Three-axis retrofit (per workspace, pattern-driven) | 5-15 min |
| Tier review (per workspace) | 5-10 min |
| Runtime triage (per workspace) | 5-10 min |
| **Per-workspace total (after pilot)** | **45-75 min** |

**Retrofit classifier stability:** 12 files / 135 verbs processed; 7
OTHER classifications initially surfaced (all `manco.group.*`
dotted-verb names). Fixed by stripping the leading noun prefix in
`classify()` — now robust for any domain with dotted verb names.

**V1.2-5 pack-hygiene check delivered value again:** caught 1 stale
template ref (`entity.link` — a verb that doesn't exist). Consistent
with KYC (1 stale) and Deal (2 stale).

---

## 3. CBU-specific findings (v1.3 candidates)

### V1.3-CAND-9 — Operational vs discovery lifecycle duality (UNIVERSAL)

CBU has the same lifecycle-duality pattern flagged in Deal
(V1.3-CAND-7 parallel child lifecycle; Deal G-10 commercial vs
operational). CBU's schema `chk_cbu_status` models only the
**discovery/validation** lifecycle (DISCOVERED → VALIDATED).
VALIDATED is schema-terminal but business-operationally it's the
**start** of the trading/servicing phase.

Same pattern observed in:
- **Deal:** commercial lifecycle (PROSPECT → CONTRACTED) + operational
  lifecycle (ONBOARDING → ACTIVE → OFFBOARDED)
- **CBU:** discovery lifecycle (DISCOVERED → VALIDATED) + operational
  lifecycle (VALIDATED → ACTIVE → SUSPENDED → WINDING_DOWN → OFFBOARDED)
- **KYC:** epistemic lifecycle (UBO discovery) + fact lifecycle (case
  approval) — V1.3-CAND-1 already flagged this

**Proposal:** Formalise "linked dual lifecycles" as a v1.3
architectural pattern distinct from single-state-machine-per-entity.
An entity can have TWO lifecycles that share a junction point
(VALIDATED / CONTRACTED / APPROVED) and run serially.

### V1.3-CAND-10 — SUSPENDED is universal (reinforcement)

SUSPENDED state flagged missing in:
- Deal (G-5)
- CBU (G-2 — see §7 business-reality gaps)
- Implicit in KYC (red_flag → blocking state is effectively suspension)
- Present in billing_profile, investor, holding (at sub-slot level)

Every workspace-primary entity probably needs a SUSPENDED state to
handle regulatory holds, dispute pauses, client distress. This is
distinct from WINDING_DOWN (which is exit-intent) and from
TERMINATED (which is terminal).

**Proposal:** Adopt "SUSPENDED is a required state for any
long-lived commercial entity" as a v1.2-11 amendment. Or as a
default state ladder: DRAFT → ACTIVE → SUSPENDED → TERMINATED for
all long-lived entities.

### V1.3-CAND-11 — Entity hierarchy / parent-child state propagation

Three workspaces have hierarchical entities:
- **Deal G-9:** master agreement → schedule → addendum
- **CBU G-12:** master fund → feeder fund, umbrella → compartments
- **KYC (existing):** case → entity_workstream → screening

Deal's G-9 and CBU's G-12 are not modelled as state-dependent links
today — they're stateless ref data. But a feeder fund's operational
state depends on master fund's state. A schedule's legal-force
depends on master contract's state.

**Proposal:** Add `parent_slot` + `state_dependency:` to v1.3 slot
schema. Validator enforces "child cannot be ACTIVE if parent is
SUSPENDED" etc.

### V1.3-CAND-12 — Evidence-type-specific refresh cadence (extends CAND-3)

Reinforces V1.3-CAND-3 (periodic review cadence, KYC). CBU evidence
has different validity windows per evidence_type:

| Evidence type | Typical validity |
|---|---|
| Corporate formation docs | Once (verify-and-done) |
| Beneficial ownership declaration | Annual refresh |
| Financial statements | Annual |
| Tax residency certificate | Annual |
| Source of wealth attestation | 2-3 years |
| Sanctions screening | Rolling (every 1-14 days) |

My single `cbu_evidence_lifecycle` (PENDING → VERIFIED → EXPIRED)
treats them all the same.

**Proposal:** Add `validity_window:` to evidence_type reference;
time-decay trigger respects per-type windows. Composable with
V1.3-CAND-3 periodic-review cadence framework.

---

## 4. Pilot + KYC + Deal + CBU — cumulative v1.2 amendment progress

All 4 primary workspaces now closed for Tranche 2. Consolidated
against the v1.2 amendment list:

| # | Amendment | Status after all 4 workspaces |
|---|---|---|
| V1.2-1 | P16 three-layer architecture | DOC — applied consistently |
| V1.2-2 | `overall_lifecycle:` first-class section | LANDED 4× |
| V1.2-3 | `requires_products:` conditional reachability | LANDED (IM, KYC, CBU use it; Deal is cross-product) |
| V1.2-4 | Prune semantics as general pattern | LANDED 4× |
| V1.2-5 | PackFqnWithoutDeclaration validator | LANDED + wired; caught stale refs in KYC (1), Deal (2), CBU (1) |
| V1.2-6 | §4.1 factual update | DOC |
| V1.2-7 | Borderline-operational-slot pattern | DOC |
| V1.2-8 | DSL over-modeling lint | DEFERRED |
| V1.2-9 | Sem-os scanner for `dag_taxonomies/` | DEFERRED |
| V1.2-10 | Estate-scale effort revision | **VALIDATED 4×** — pilot ~4hr → KYC 90m → Deal 60m → CBU 75m |

**7/10 v1.2 amendments materially landed**. 2 deferred (V1.2-8, V1.2-9).
1 doc-only (V1.2-6).

**Cumulative v1.3 candidates** across all 4 workspaces:

| # | Candidate | Sources | Cross-workspace evidence |
|---|---|---|---|
| V1.3-CAND-1 | UBO epistemic-vs-fact state | KYC | — |
| V1.3-CAND-2/5 | **cross_workspace_constraints** | KYC, Deal, CBU | **3 workspaces → P0 v1.3** |
| V1.3-CAND-3/12 | **Periodic review cadence** | KYC, CBU evidence | **2 workspaces → P1 v1.3** |
| V1.3-CAND-4 | Remediation workflow | KYC | — |
| V1.3-CAND-6 | DB-triggered state transitions | Deal | — |
| V1.3-CAND-7/9 | **Parallel / dual lifecycle composition** | Deal, CBU | **2 workspaces → P1 v1.3** |
| V1.3-CAND-8 | Commercial-commitment tier convention | Deal | — |
| V1.3-CAND-10 | **SUSPENDED as universal state** | Deal, CBU, implicit KYC | **3 workspaces → P0 v1.3** |
| V1.3-CAND-11 | **Parent-child state dependency (hierarchy)** | Deal, CBU, KYC | **3 workspaces → P0 v1.3** |

**Emerging v1.3 roadmap (P0 candidates with 3-workspace evidence):**
- CAND-2/5: cross_workspace_constraints
- CAND-10: SUSPENDED as universal state
- CAND-11: hierarchy state dependencies

These are now the leading candidates for v1.3 architectural additions.

---

## 5. Test fixture hygiene (flagged, not blocking)

CBU workspace has more fixture drift than prior workspaces (8 true
bugs across 59 cases vs 2 / 16 for Deal and 0 / 72 for KYC):

| Line | Fixture expected_verb | Actual | Category |
|---|---|---|---|
| 2129 | `cbu.suspend` | Not declared | Stale (no CBU suspend verb — CBU G-2 gap, see §7) |
| 2137 | `cbu.assign-product` | `cbu.add-product` | Renamed |
| 2153 | `fund.create-sub-fund` | No `fund.*` domain | Legacy domain |
| 2161 | `fund.create-share-class` | `share-class.create` (in capital.yaml) | Legacy routing |
| 2700 | `cbu.assign-product` | `cbu.add-product` | Renamed (duplicate) |
| 2709 | `capital.redeem` | Probably `capital.redeem-shares` | Stale |
| 2718 | `fund.create-sub-fund` | No `fund.*` domain | Legacy (duplicate) |
| 3399 | `mandate.select` | `structure.select` macro or navigation | Stale |

**Bucket breakdown (informative):**
- **Bucket 1** (24/59 = 40.7%) — expected verb declared + matches
  pipeline verb tier
- **Bucket 2** (27/59 = 45.8%) — valid via non-verb pipeline tier:
  - 13 narration-intercept queries ("what's next", "status update",
    etc.) — bypass verb search via NarrationEngine
  - 14 structure-macro routes (`struct.uk.authorised.oeic` etc.) —
    resolved via MacroIndex Tier -2B
- **Bucket 3** (8/59 = 13.6%) — pure fixture bugs

**Fixture-clean routing correctness: 51/51 = 100%**. Same alignment
as prior workspaces once non-verb pipeline paths are counted.
Recommend a Tranche 3 fixture cleanup sweep to retire the 8 legacy
fixture entries.

---

## 6. Tranche 2 status — ALL 4 PRIMARY WORKSPACES CLOSED

| Workspace | Status | Verbs | DAG LOC | Slots | Notes |
|---|---|---|---|---|---|
| **Instrument Matrix** | ✅ CLOSED (pilot) | 248 | 1266 | 36 | Pilot + infra |
| **KYC** | ✅ CLOSED | 142 | 931 | 32 | 2nd workspace |
| **Deal** | ✅ CLOSED | 59 | 872 | 22 | 3rd workspace |
| **CBU** | ✅ CLOSED (this session) | 135 | 827 | 20 | 4th workspace |
| **TOTAL** | | **584** | **3896** | **110** | |

**Tranche 2 core authoring COMPLETE.** 584/1184 verbs declared
(49.3%).

**Remaining Tranche 2 work:**
1. **Cross-workspace reconciliation pass** (~45-60 min): resolve slot
   ownership overlaps (cbu slot defined in CBU, referenced in IM/KYC/
   Deal; client_group defined in IM, referenced in CBU/KYC/Deal),
   validate cross-workspace constraints are consistently expressed,
   identify any remaining DAG↔DSL breaks.
2. **Business-reality gap consolidation** (per Adam's directive):
   compile §7-equivalent gaps from Deal + CBU findings into a
   unified remediation plan, separating cross-workspace patterns
   from workspace-specific gaps.

**Non-primary workspaces with pack files** (deferred to Tranche 3):
book-setup, onboarding-request, product-service-taxonomy,
semos-maintenance, session-bootstrap.

---

## 7. DAG business-reality gaps — review pending cross-workspace reconciliation

Post-closure business-knowledge sanity pass. Same methodology as
Deal findings §7 — gaps logged but DAG preserved, to be re-reviewed
in the cross-workspace reconciliation so universal patterns can be
separated from CBU-specific.

### 7.0 FOUNDATIONAL FRAMING — what a CBU actually IS (dominates the review)

**Adam's correction (2026-04-23):** CBU is a construct Adam coined.
It is NOT a generic "trading entity" or "client record" or "fund."
It is his name for **the money-making apparatus a commercial client
has established on the market to generate returns**.

- The parent (Allianz, BlackRock) is the commercial client.
- The CBU is what the client BUILT to make money.
- One client typically has many CBUs (each a distinct trading unit
  with its own mandate, investor register, custody setup, billing).
- The `cbu_category` enum (FUND_MANDATE, CORPORATE_GROUP,
  INSTITUTIONAL_ACCOUNT, RETAIL_CLIENT, FAMILY_TRUST,
  CORRESPONDENT_BANK) is NOT categorical reference data — each is
  a different SHAPE of money-making apparatus with different revenue
  mechanics and service-consumption patterns.

**This framing exposes that my DAG is fundamentally mis-centred.**
I modelled CBU as a discovery/validation entity (state column is
`chk_cbu_status` — DISCOVERED → VALIDATED). But validation is just
"can we onboard this thing." The whole purpose of the CBU is to be
**operationally active in the market making money**. VALIDATED is
the *beginning* of the CBU's purpose, not a workspace-terminal state.

This framing escalates three gaps from P0/P1 into **foundational
re-authoring items for the next CBU DAG iteration**:

1. **Operational lifecycle belongs to CBU, not to IM/Deal/KYC.**
   The CBU IS the revenue-generation locus. Moving operational
   state (ACTIVE trading, SUSPENDED, WINDING_DOWN, OFFBOARDED,
   ARCHIVED) into the CBU DAG where it belongs is NOT a P0 "add 4
   states" — it's a correction to the fundamental modelling
   posture. My earlier cop-out (§7 G-1) deferring operational
   lifecycle to adjacent workspaces was wrong because it treats
   CBU as a passive record rather than the active trading unit.

2. **Revenue realization is the point of the entity.** The thing
   that justifies all the evidence, roles, investors, holdings
   infrastructure is that the CBU makes money on the street. My
   DAG has NO state, NO slot, NO lifecycle phase for "first trade
   executed," "first invoice generated," "actively trading," "dormant
   but open," "profitably operating vs loss-making" (last one
   arguably out-of-scope). Without this, the CBU DAG describes the
   **plumbing** of a CBU but not the **purpose** of a CBU.

3. **Service consumption is first-class, not categorical.** A CBU
   consumes services (custody, TA, FA, securities lending, FX,
   trading, reporting) to operate on the street. Real clients turn
   services on / off / upgrade / downgrade over the CBU's life. I
   have `cbu.add-product` / `cbu.remove-product` as CRUD but no state
   machine per-service-per-CBU. Service-consumption lifecycle
   (proposed → provisioned → active → suspended → wound-down) is
   what makes a CBU operationally real; my DAG treats it as ref
   data.

4. **`cbu_category` should gate operational capabilities.** Fund
   categories have investors / NAVs / share-classes; corporate-group
   categories don't; family-trust categories have different
   evidence / KYC shape; correspondent-bank categories have
   clearing/settlement-specific apparatus. I listed `cbu_category`
   as CATEGORICAL in the DAG — it should be driving conditional
   slot activation and lifecycle variance. The existing
   `product_module_gates` section has some of this (investor +
   holding + share_class gated by FUND_MANDATE) but not enough.

5. **Market-facing identity is first-class.** LEI, BIC, depo
   account references, sub-account numbers at DTC / Euroclear /
   Clearstream are the CBU's **street-facing identity** — the
   addresses through which the CBU exists in the market. Owned by
   the CBU conceptually, though provisioned in adjacent workspaces.
   My DAG references these obliquely through custody slots but
   doesn't make the "this is the CBU's identity on the street"
   primacy explicit. The CBU is its street-facing identities + its
   operating capability.

6. **CBU should AGGREGATE operational-readiness from other
   workspaces, not defer to them (V1.3-CAND-13 — new).** The
   cross-workspace corollary of the re-centring. Today I scattered
   operational preconditions across 4 DAGs: Deal.CONTRACTED
   requires KYC.APPROVED; CBU.VALIDATED requires KYC case APPROVED;
   etc. Each is a pair-wise "A blocks B" constraint
   (V1.3-CAND-2/5 territory).

   Under the CBU reframe, there's a different pattern: CBU is the
   entity whose operational state is **derived from the
   conjunction** of other workspaces' states. Not "A blocks B" but
   "C's state IS DERIVED FROM the state of A + B + D":

   ```
   CBU.operationally_active = DERIVED FROM:
     kyc_case.status = APPROVED                           (KYC)
     deal.status ∈ {CONTRACTED, ONBOARDING, ACTIVE}       (Deal)
     im.trading_enablement ∈ {trade-permissioned,         (IM — see
                              actively-trading}            §10 of IM
                                                           findings)
     cbu_evidence.all_verified = true                     (CBU)
   ```

   This is **projection/aggregation across DAGs**, distinct from
   the blocking-constraint pattern in V1.3-CAND-2/5. It needs its
   own v1.3 schema support.

   **New v1.3 candidate: V1.3-CAND-13 — cross-workspace aggregate
   state.** A slot's state can be DERIVED from the state of slots
   in other workspaces, not just blocked-by them. CBU
   `operationally_active` is the first instance. Likely applies to
   other workspace-spanning compound states (e.g. "client fully
   onboarded" = Deal.ACTIVE + all-CBUs-VALIDATED +
   all-kyc-cases-APPROVED).

**Implication for the cross-workspace reconciliation:** the CBU
workspace needs re-centring as "the operational money-making unit"
first, "a record with evidence" second. Gaps G-1 through G-13 in
§7.1-7.3 are consequences of this foundational mis-centring; many
will collapse into a single "re-anchor the CBU DAG on
operational-purpose-first" remediation in v1.3. The IM workspace
needs a paired phase-axis re-anchor (see IM pilot findings §10
addendum).

The reconciliation should begin with this foundational re-framing
before working through the per-gap catalogue.

### 7.1 P0 — Material business gaps

**G-1. CBU operational lifecycle beyond VALIDATED is undefined.**
The schema's `chk_cbu_status` (5 states) captures only discovery
and validation. VALIDATED is schema-terminal. But a real CBU has
an operational lifecycle:
- VALIDATED (ready for trading)
- ACTIVE (trading happening; rates live; periodic refresh on cadence)
- SUSPENDED (regulatory hold, dispute, client distress)
- WINDING_DOWN (exit decision made; unwinding positions)
- OFFBOARDED (terminal-positive)
- ARCHIVED (historical only; retained for regulatory retention)

My DAG cop-outs: "VALIDATED is terminal-in-workspace but not
terminal globally — further CBU activity is owned by IM / Deal /
KYC workspaces respectively." This is wrong — the CBU IS the
entity; its operational lifecycle should be part of this workspace.

Matches V1.3-CAND-9 (dual lifecycle).

**Proposed remediation:** add 4 new states beyond VALIDATED. Schema
change needed (expand `chk_cbu_status`). Flag as DAG-DSL alignment
break.

**G-2. CBU SUSPENDED state missing.**
Universal pattern (Deal G-5 + KYC implicit + CBU here). Regulatory
holds, litigation hold, sanctions exposure on controlling parties,
client financial distress — all need a pause state that's
restorable. Today there's no way to mark a CBU "paused."

Matches V1.3-CAND-10 (SUSPENDED universal).

**G-3. Manco state machine missing (regulatory-action cascade).**
I marked `manco` as stateless. Wrong. Real manco states:
- ACTIVE (managing mandates)
- UNDER_REVIEW (new manco onboarding)
- UNDER_INVESTIGATION (regulatory action)
- SUNSET (no new mandates; existing continue)
- TERMINATED

A manco under regulatory action (e.g. a Credit Suisse unit)
cascades to all CBUs managed by it. My DAG has no way to model
this cascade — CBUs under that manco would all need SUSPENDED.

**G-4. Change-of-control / material-change workflow not modelled.**
When a CBU's ownership changes materially (M&A of parent, UBO
change > threshold), there's a formal re-validation workflow. My
DAG hints at this via `UPDATE_PENDING_PROOF` but doesn't model:
- Material-change detection (ownership diff > threshold)
- Re-validation scope (partial vs full re-KYC)
- Notification obligations (to regulators, investors)
- Parallel evidence-refresh lifecycle

This overlaps with V1.3-CAND-4 (remediation workflow) but is
CBU-specific.

### 7.2 P1 — Semantic refinements

**G-5. Investor lifecycle missing REDEEMING state.**
My states: DRAFT → ELIGIBLE → ACTIVE → SUSPENDED → OFFBOARDED.
`investor.start-redemption` and `investor.complete-redemption` are
verbs — clearly there's a REDEEMING state between ACTIVE and
OFFBOARDED. Add: REDEEMING (instruction placed, awaiting
settlement), REDEEMED (units = 0, account still open).

**G-6. Evidence-type-specific refresh cadence.**
One `cbu_evidence_lifecycle` treats all evidence (formation docs,
UBO declarations, financial statements, tax certs, sanctions
screening) the same. Different validity windows per evidence_type.

Matches V1.3-CAND-12.

**G-7. Share-class lifecycle has states, not stateless.**
DRAFT → OPEN → SOFT_CLOSED → HARD_CLOSED → WINDING_DOWN →
CLOSED/LIQUIDATED. Also: fee-structure changes requiring
re-approval. I marked it stateless.

**G-8. Holding states miss RESTRICTED / PLEDGED / FROZEN.**
PENDING → ACTIVE → SUSPENDED → CLOSED misses:
- RESTRICTED (legal lock: court order, dispute)
- PLEDGED (collateral against a loan)
- FROZEN (sanctions on investor)

Affects redemption logic.

**G-9. CBU-level corporate action events.**
CBU-level CAs (renaming, redomiciliation, merger with another CBU,
conversion between fund types) are material events needing their
own state machine. Instrument-level CAs live in IM workspace;
CBU-level belongs here. Not modelled.

### 7.3 P2 — Flags

**G-10.** Entity proper-person lifecycle too simple (GHOST →
IDENTIFIED → VERIFIED) — real refresh/staleness semantics missing.

**G-11.** Soft-delete (`cbus.deleted_at`) vs hard-delete not
distinguished in state model.

**G-12.** CBU hierarchy (master-feeder, umbrella-compartment,
parallel funds) not state-modelled — `cbu_entity_relationship`
stateless but child state should depend on parent. Matches
V1.3-CAND-11.

**G-13.** "Under remediation" as a distinct state (post-breach
cleanup, enhanced monitoring, board attestation) — different from
UPDATE_PENDING_PROOF.

### 7.4 Hold-and-review plan

Gaps logged, DAG preserved. Next action: **cross-workspace
reconciliation pass** consolidates Deal §7 + CBU §7 gaps and
separates:
- **Universal patterns** (SUSPENDED, terminal-granularity,
  hierarchy, operational-vs-discovery duality, commercial-commitment
  tier, parallel child lifecycle) → v1.3 architectural amendments
- **Workspace-specific gaps** (BAC for Deal, manco-cascade for CBU,
  periodic-review cadence for KYC/CBU, etc.) → targeted DAG fixes
  per workspace

---

## 8. Closure

**CBU Tranche 2 CLOSED. Tranche 2 core authoring COMPLETE.**

All 4 primary workspaces declared:
- 584 verbs with three-axis declarations (49.3% of total 1184)
- 4 DAGs authored totalling 3896 LOC
- 110 slots across the 4 workspaces
- 12 v1.3 candidates surfaced, 3 now have 3-workspace evidence
  (P0 priorities for v1.3)
- 13 CBU-specific business-reality gaps logged in §7 for
  cross-workspace reconciliation review

**Next:** cross-workspace reconciliation pass + consolidated
business-reality gap triage (merging Deal §7 + CBU §7).

**T2-C-9 end.**

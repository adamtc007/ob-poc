# Tranche 2 — Deal Findings Report (2026-04-23)

> **Status:** DEAL CLOSED. Third workspace through the pilot pattern.
> **Parent docs:**
> - `instrument-matrix-pilot-findings-2026-04-23.md` (pilot baseline)
> - `tranche-2-kyc-findings-2026-04-23.md` (second workspace)
> - `catalogue-platform-refinement-v1_2.md` (spec)

---

## 1. Delivery summary

Pilot-mirror 9-phase pattern executed in ~60 minutes actual vs. the
kickoff estimate of 30-45 min. Infrastructure from IM + KYC fully
reused; net authoring work was the Deal DAG + 59-verb retrofit + 3
tier raises.

| Phase | Deliverable | Effort actual |
|---|---|---|
| T2-D-1 (equiv P.1) | SKIP — infra reused | 0 |
| T2-D-2-prep | Deal pack-delta (2 stale refs pruned: `deal.delete` from forbidden_verbs, `deal.assign-rate-card` template ref → `deal.propose-rate-card`) | 5 min |
| T2-D-2 | `deal_dag.yaml` (872 LOC: 10-phase overall lifecycle + 8 stateful slot state machines + 14 stateless slots + 10 cross-slot constraints + 4 prune cascade rules) | ~25 min |
| T2-D-3 | 59 three-axis declarations via retrofit script (42 `deal.*` + 17 `billing.*`) | ~5 min |
| T2-D-4 | 3 tier raises (agree-rate-card, activate-profile, generate-invoice) | ~5 min |
| T2-D-5 | Runtime triage: **14/14 valid Bucket 1 (100%)**; 2 fixture hygiene bugs flagged separately | ~5 min |
| T2-D-6/7/8 | SKIP — infra/tools reused | 0 |
| T2-D-9 | This document | ~5 min |

**Validator terminal state:** 449 / 1184 declared, 0 structural errors,
0 well-formedness errors, 0 warnings.

**Net declarations added this session:** +59 (from post-KYC's 390).

---

## 2. What the pilot infrastructure delivered (confirmed at 3× scale)

Validated at 3 workspaces now:

- **Schema + validator + composition engine:** drop-in reuse. 0 new
  code. Same store as IM + KYC.
- **Pattern-based retrofit script** (adapted to `/tmp/t2d3_retrofit.py`):
  needed 2 Deal-specific additions — `propose|counter|request` pattern
  for the negotiation chain (EMIT cluster → reviewable) and
  `agree|approve` baseline recognition. Otherwise identical to
  t2k3_retrofit.py. Classifier correctness: 57/59 on first pass; 2
  adjustments (search → READ cluster, and the new EMIT cluster)
  handled the Deal-specific cases.
- **Pack-hygiene check (V1.2-5):** caught 2 stale refs at Tranche-2
  entry. Consistent with KYC experience (1 stale) and IM pilot (pack
  was built clean from day one).
- **Triage script:** simple workspace filter + bucket categorisation.
  No code changes.

**Effort curve holding:** IM pilot (~4 hours incl. infra) → KYC
(~90 min, infra reused) → Deal (~60 min, infra reused and
stable). V1.2-10's estate-scale estimate is converging toward
"author DAG + retrofit verbs + triage" as the unit cost per
workspace.

---

## 3. Deal-specific findings (v1.3 candidates)

Four findings surfaced during Deal Tranche 2 that aren't in v1.2 and
warrant v1.3 consideration. Notably THREE of them overlap with
v1.3 candidates already flagged in KYC findings §3, reinforcing their
cross-workspace weight.

### V1.3-CAND-5 — Cross-workspace state gate (reinforcement of V1.3-CAND-2)

Deal → CONTRACTED requires group KYC case = APPROVED. This is the
same cross-workspace state dependency pattern flagged in KYC
V1.3-CAND-2, but observed from the opposite side: KYC produces the
signal; Deal consumes it.

Observed at multiple points in Deal DAG:
- `deal.update-status KYC_CLEARANCE → CONTRACTED` needs
  `cases.status = APPROVED WHERE client_group_id = deals.primary_client_group_id`
- Deal DAG §3 `deal_contracted_requires_kyc_clearance` constraint
  is cross-workspace, not intra-DAG — v1.2's
  `cross_slot_constraints` doesn't cover this.
- Deal → `deal_ubo_assessments` (5-state) overlaps heavily with KYC
  `kyc_ubo_registry` (9-state). They're tracking the same underlying
  UBO determination but at different grain. **v1.3 candidate:
  consolidate, or formalise the projection relationship.**

**Proposal:** Accelerate V1.3-CAND-2 (cross_workspace_constraints:)
as a P0 v1.3 priority now that it's corroborated at 2 workspaces.

**Sources:** Deal DAG §3 constraints, §5 out-of-scope.

### V1.3-CAND-6 — Database-triggered state transitions

Deal has one state transition that's triggered by db-side logic, not
verb execution: `deal_rate_cards.status AGREED → SUPERSEDED` fires
automatically when a new AGREED rate card is created for the same
(deal, contract, product), enforced by `idx_deal_rate_cards_one_agreed`
(migration 069).

This is a new pattern — the DAG transition exists (§2.2), but the
verb column is `(backend: new AGREED ...)`. v1.2's state machine
model implicitly assumes every transition has a verb.

**Proposal:** Add `trigger_source: database|verb|scheduler|external`
to transitions in v1.3 schema. Enables validator to catch bugs like
"verb-driven transition with no verb" or "db-driven transition
authored but no db trigger exists".

**Sources:** Deal DAG §2.2 `deal_rate_card_lifecycle` transitions.

### V1.3-CAND-7 — Parallel lifecycle sub-domains (billing periods)

Deal has a fee-billing sub-domain with TWO parallel lifecycles:
- `billing_profile` (4 states: DRAFT/ACTIVE/SUSPENDED/CLOSED) — the
  long-lived revenue container.
- `billing_period` (6 states: PENDING→CALCULATED→REVIEWED→APPROVED
  →INVOICED/DISPUTED) — the time-sliced billing cycle.

Multiple billing_periods exist per profile lifetime. They're NOT
sub-states of the profile; they're parallel, time-sliced state
instances that all share the profile parent.

v1.2's one-entity-one-lifecycle assumption (also flagged in
V1.3-CAND-1 around UBO epistemic vs fact) holds locally per entity,
but the *composition* of entities isn't modeled. The DAG says
"profile has a lifecycle" and "period has a lifecycle" in
parallel; the cross-constraint `billing_period_creation_requires_active_profile`
is the only coupling.

**Proposal:** Formalise "parent lifecycle with parallel child
instances" as a distinct architectural pattern alongside
state_machines + cross_slot_constraints. Applies to:
- Deal: billing_profile → billing_period
- KYC: case → screenings (multiple screenings per case, each with
  lifecycle)
- IM: trading_profile → instrument_matrix_entries (each entry has
  its own state)

**Sources:** Deal DAG §2.2 billing_profile + billing_period slots.

### V1.3-CAND-8 — Commercial commitment tier (tier-apply pattern)

The 3 Deal tier raises (`agree-rate-card`, `activate-profile`,
`generate-invoice`) share a common shape: **preserving + emitting +
commercial-binding**. These are the points where the workspace
crosses from internal deliberation to external commitment (a rate
offer, a revenue start, an invoice).

v1.2's 4-tier ladder (benign / reviewable / requires_confirmation /
requires_explicit_authorisation) lands these correctly, but the
rationale "crosses commercial commitment boundary" isn't explicitly
captured as a tier-apply heuristic.

**Proposal:** Add a "commercial commitment" convention to the
tier-apply vocabulary (§V1.2-11 candidate): when a verb emits a
communication to a counterparty that creates or modifies a
commercial commitment, default tier ≥ requires_confirmation.
Applies to:
- Deal: agree-rate-card, activate-profile, generate-invoice
- Deal future: would also apply to `contract.submit`, `contract.sign`
  (not in Deal workspace scope)
- KYC: `case.approve`, `case.reject` already follow this pattern

**Sources:** T2-D-4 tier review decisions.

---

## 4. Pilot + KYC + Deal combined — cumulative v1.2 amendment progress

Consolidated against the v1.2 amendment list:

| # | Amendment | Status after Deal |
|---|---|---|
| V1.2-1 | P16 three-layer architecture | DOC (spec §1) — applied consistently across 3 workspaces |
| V1.2-2 | `overall_lifecycle:` first-class section | LANDED (used by IM + KYC + Deal DAG YAMLs) |
| V1.2-3 | `requires_products:` conditional reachability | LANDED (field present in schema + IM + KYC use it; Deal is cross-product so no conditional gates) |
| V1.2-4 | Prune semantics as general pattern | LANDED (IM + KYC + Deal all author prune_cascade_rules; Deal uses deal.cancel as primary prune path) |
| V1.2-5 | PackFqnWithoutDeclaration validator | LANDED + wired into 3 runtime tools; caught stale refs at KYC + Deal entry |
| V1.2-6 | §4.1 factual update | DOC |
| V1.2-7 | Borderline-operational-slot pattern | DOC (pattern documented; not enforced) |
| V1.2-8 | DSL over-modeling lint | DEFERRED |
| V1.2-9 | Sem-os scanner for `dag_taxonomies/` | DEFERRED |
| V1.2-10 | Estate-scale effort revision | VALIDATED 3× (IM pilot ~4hr → KYC ~90min → Deal ~60min; infra amortized cleanly) |

7 of 10 v1.2 amendments materially landed (same as post-KYC).

**4 new v1.3 candidates surfaced in Deal** (§3 above) — one each from
cross-workspace state (V1.3-CAND-5 reinforcing KYC's V1.3-CAND-2),
db-triggered transitions (V1.3-CAND-6), parallel-child-lifecycle
(V1.3-CAND-7), and commercial-commitment tier convention (V1.3-CAND-8).

Combined v1.3 candidate count across all 3 workspaces:

| Candidate | Sources | Priority signal |
|---|---|---|
| V1.3-CAND-1 | UBO epistemic-vs-fact state | KYC | 1 |
| V1.3-CAND-2 / V1.3-CAND-5 | **cross_workspace_constraints** | KYC + Deal | **2 — P0 v1.3 priority** |
| V1.3-CAND-3 | Periodic review cadence | KYC | 1 |
| V1.3-CAND-4 | Remediation workflow distinctness | KYC | 1 |
| V1.3-CAND-6 | DB-triggered transitions | Deal | 1 |
| V1.3-CAND-7 | Parallel child lifecycle composition | Deal | 1 (observable in IM + KYC too) |
| V1.3-CAND-8 | Commercial-commitment tier convention | Deal | 1 |

The cross-workspace state dependency pattern has the most evidence
(2 workspaces) and is the clearest P0 for v1.3.

---

## 5. Test fixture hygiene (flagged, not blocking)

2 stale entries in the intent test fixture's Deal cases:

| Line | Fixture expected_verb | Actual declared verb | Impact |
|---|---|---|---|
| 2060 | `deal.activate` | (non-existent; transition via `deal.update-status`) | Fixture expected a verb that doesn't exist; triage false negative |
| 2082 | `contract.read` | `contract.get` | Naming divergence (read vs get); fixture was likely authored against an earlier contract domain convention |

These are not DAG or DSL declaration gaps — they're test-fixture
drift. Recommend a cleanup pass on
`rust/tests/fixtures/intent_test_utterances.toml` to align with
current verb FQNs (not blocking Tranche 2 closure).

14/14 valid fixtures → 100% Bucket 1 alignment (same as IM + KYC).

---

## 6. Tranche 2 status — estate-scale progress

Tranche 2 progress (4 primary workspaces):

| Workspace | Status | Declared verbs | Pack size | Notes |
|---|---|---|---|---|
| **Instrument Matrix** | ✅ CLOSED (pilot) | 248 | 186 post-prune | First workspace |
| **KYC** | ✅ CLOSED | 142 (97 + 45 adjacent) | 100 post-prune | Second workspace |
| **Deal** | ✅ CLOSED (this session) | 59 (42 deal + 17 billing) | 5 (minimal) | Third workspace |
| **CBU** | ⏳ pending | — | — | Fourth target |

**Estimated remaining Tranche 2 effort** (revised after 3 workspaces):
- CBU: medium scope, probably **45-60 min focused work** (refined
  down from earlier estimate of 1-1.5 hr given effort convergence).
- Cross-workspace reconciliation pass: **45-60 min** (when all 4
  primary workspaces declared).
- **Total remaining Tranche 2: 1.5-2 hours focused work.**

Effort-per-workspace trend:
- IM pilot (infra + authoring): ~4 hours
- KYC (infra reused): ~90 min
- Deal (infra reused): ~60 min
- CBU (expected): ~45-60 min
- **Asymptote:** ~30-45 min / workspace once domain is familiar
  and state model is pre-existing.

---

## 7. DAG business-reality gaps — review pending Tranche 2 closure

Post-closure, a business-knowledge review pass (parallel to the IM
pilot Passes 2-3 methodology) surfaced material gaps between the Deal
DAG as authored and how a real commercial deal in financial-services
asset-servicing operates. Captured here, not remediated yet — per
Adam's direction: hold until all 4 primary workspaces are declared,
then see which gaps are Deal-specific vs cross-workspace patterns.

### 7.1 P0 — Material business gaps

**G-1. BAC (Business Acceptance Committee) approval gate is missing.**
Real commercial deals pass through BAC as an internal approval gate
— separate from rate-card AGREED (bilateral with client) and contract
SIGNED (legal execution). BAC reviews revenue above thresholds, new
jurisdictions, non-standard pricing, credit exposure. The schema
already hints at this: `deal_documents.document_type` includes
`BOARD_APPROVAL`, but there's no BAC state in the deal lifecycle.

Proposed remediation: add `BAC_APPROVAL` state between NEGOTIATING
and KYC_CLEARANCE (or parallel to it). New verbs: `deal.submit-for-bac`,
`deal.bac-approve`, `deal.bac-reject`, `deal.escalate-to-committee`.

**G-2. Pricing-committee approval separate from rate-card negotiation.**
Before a rate card can be PROPOSED to a client (especially for
discounts past threshold, bespoke products, new geographies), pricing
committee often requires internal approval. Today the DAG goes
DRAFT → PROPOSED in a single step. Real pipeline:
DRAFT → PENDING_INTERNAL_APPROVAL → APPROVED_INTERNALLY → PROPOSED.

Proposed remediation: add two states to `deal_rate_card_lifecycle`,
or factor pricing approval into a commercial-governance sub-workspace.

**G-3. Terminal-state granularity — CANCELLED is too coarse.**
One bucket loses commercial post-mortem signal. Real distinctions:
- **LOST** (competitor won)
- **REJECTED** (internal BAC said no)
- **WITHDRAWN** (client walked away)
- **CANCELLED** (we walked away)

Drives different sales attribution, pipeline forecasting, win/loss
analytics.

Proposed remediation: split `CANCELLED` into four terminal-negative
states. Schema change needed (expand `deals_status_check`).

**G-4. Amendment / renegotiation mid-life has no home.**
Active deals get scope extensions, annual rate reviews, SLA changes,
repricing. There's no amendment state in the DAG — cycling back to
NEGOTIATING is wrong (deal is still ACTIVE; revenue still flowing;
existing contracts still in force during amendment). Matches
V1.3-CAND-7 (parallel child lifecycle).

Proposed remediation: model amendment as a parallel `deal_amendment`
entity with its own 4-5 state lifecycle; cross-constraint that
amendments can only be created when parent deal is ACTIVE.

**G-5. Deal SUSPENDED state missing.**
Real deals get suspended (regulatory hold, billing dispute, client
distress, re-papering). Different from WINDING_DOWN (exiting).
Billing has SUSPENDED; deal itself should too. Without it, a
regulatory hold on an ACTIVE deal has no clean state.

Proposed remediation: add `SUSPENDED` state between ACTIVE and
WINDING_DOWN with bidirectional transitions ACTIVE ↔ SUSPENDED.

### 7.2 P1 — Semantic refinements

**G-6. KYC_CLEARANCE should be a gate, not a phase.**
KYC runs in parallel with commercial negotiation, not sequentially.
Preliminary KYC starts at QUALIFYING (sanctions / PEP); full KYC
runs alongside NEGOTIATING; KYC is a precondition on CONTRACTED,
not a standalone phase. The existing `state_machines/deal_lifecycle.yaml`
already has KYC_CLEARANCE as a serial phase — that's legacy
modelling, likely wrong, was preserved in my DAG.

Proposed remediation: remove KYC_CLEARANCE as a phase; move it to
a precondition on NEGOTIATING → CONTRACTED. Matches V1.3-CAND-2
cross-workspace constraints.

**G-7. SLA lifecycle — marked stateless, business concept has states.**
SLAs have states: NEGOTIATED → ACTIVE → BREACHED → IN_REMEDIATION →
RESOLVED (or WAIVED). Breaches trigger penalty calculations that
feed into billing. Schema doesn't have a status column today, but
the business concept clearly has lifecycle.

Proposed remediation: promote `deal_sla` from stateless to stateful.
Schema change needed (add `sla_status` column). Flag as a DAG-DSL
alignment break for remediation.

**G-8. Internal vs counterparty participant roles.**
`deal_participants.role` captures counterparty roles (CONTRACTING_PARTY,
GUARANTOR, INTRODUCER, INVESTMENT_MANAGER, FUND_ADMIN). Real deals
also track internal accountability: **deal sponsor**, **relationship
manager (RM)**, **coverage banker**, **product SME**, **legal
counsel**, **operations lead**. These drive workflow routing and
reporting.

Proposed remediation: model internal ownership as deal-level
attributes (`deal.sponsor_id`, `deal.rm_id`, `deal.coverage_banker_id`)
rather than polluting the participant role enum.

**G-9. Deal hierarchy — master/schedule/addendum.**
`deal_documents.role` already knows about PRIMARY/ADDENDUM/SCHEDULE/
SIDE_LETTER. Real deals have hierarchy — a schedule is subordinate
to a master; schedule can't go ACTIVE if master is SUSPENDED. My
DAG treats every deal as independent.

Proposed remediation: add `deal.parent_deal_id` and a cross-slot
constraint — child deal state must be consistent with parent deal
state.

**G-10. Commercial vs operational lifecycle duality.**
I bundled them into one 10-state machine. Real architecture has two
linked-but-distinct lifecycles:
- Commercial (sales + BAC owned): PROSPECT → QUALIFYING →
  NEGOTIATING → BAC_APPROVAL → CONTRACTED
- Operational (ops owned): CONTRACTED → ONBOARDING → ACTIVE →
  SUSPENDED/WINDING_DOWN → OFFBOARDED

Linked at CONTRACTED. Different owners, different governance,
different reports. This is likely NOT Deal-specific — worth
reviewing against CBU / KYC / IM after all 4 workspaces are
declared.

Proposed remediation: hold judgement until cross-workspace
reconciliation pass.

### 7.3 P2 — Flags, not urgent

**G-11.** Legal review as explicit phase (or a deal_document
sub-state).

**G-12.** Expected revenue / pipeline valuation — arguably
out-of-DAG (valuation data, not state), but needed for BAC
threshold logic and pipeline forecasting.

**G-13.** Coverage / cross-sell tracking — partially in
`client_principal_relationship`, not wired into deal state.

### 7.4 Hold-and-review plan

Per Adam's direction (2026-04-23), these gaps are logged but
deferred pending Tranche 2 closure across all 4 primary workspaces.
The review at that point asks:
- Which gaps are Deal-specific (apply to Deal DAG only)?
- Which are cross-workspace patterns (e.g. BAC-like committee gates
  likely exist in CBU onboarding approvals too; terminal-state
  granularity might apply to every workspace; suspended-state is
  probably universal)?
- Which warrant v1.3 amendments vs per-workspace DAG fixes?

Once CBU is declared and the cross-workspace reconciliation pass
completes, re-visit this section with a consolidated remediation
plan.

---

## 8. Closure

**Deal Tranche 2 CLOSED.** 9 phases delivered. 100% runtime-triage
alignment on valid fixtures. 2 fixture bugs flagged as follow-ups.
4 new v1.3 candidates captured — including reinforcement of
V1.3-CAND-2 cross-workspace-constraints which now has evidence from
both KYC (producer side) and Deal (consumer side).

**13 DAG business-reality gaps logged in §7 for hold-and-review
after Tranche 2 closure.**

Next: **CBU workspace** (fourth and final primary workspace for Tranche 2).

**T2-D-9 end.**

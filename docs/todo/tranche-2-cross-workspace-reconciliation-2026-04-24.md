# Tranche 2 — Cross-Workspace Reconciliation Pass (2026-04-24)

> **Status:** OPEN. Consolidates business-reality gaps + v1.3
> candidates + phase-axis reconsiderations surfaced during Tranche 2.
>
> **Inputs:**
> - `instrument-matrix-pilot-findings-2026-04-23.md` (incl. §10 addendum: phase-axis re-anchor)
> - `tranche-2-kyc-findings-2026-04-23.md` (closed, no business-reality gaps flagged)
> - `tranche-2-deal-findings-2026-04-23.md` (incl. §7 — 13 gaps)
> - `tranche-2-cbu-findings-2026-04-23.md` (incl. §7.0 foundational + §7.1-7.3 — 13 gaps)
>
> **Spec target:** `catalogue-platform-refinement-v1_3` (to be drafted)

---

## 1. Purpose

Four DAGs authored across Tranche 2 (IM + KYC + Deal + CBU). Three
separate business-knowledge sanity passes surfaced gaps that don't
cleanly partition per workspace — many are instances of the same
underlying architectural pattern recurring across workspaces.

This pass:

1. **Consolidates** gaps into a unified catalogue.
2. **Separates** universal architectural patterns (requiring v1.3
   amendments) from workspace-specific business gaps (requiring
   targeted DAG fixes).
3. **Ranks** by cross-workspace evidence strength.
4. **Proposes** a remediation sequence with dependencies.
5. **Flags** decisions that need Adam's input before execution.

---

## 2. The foundational correction (dominates everything else)

**CBU is Adam's coined construct for the money-making apparatus a
commercial client has established on the market.** Not a generic
client-entity; not a fund; not a trading record. Its purpose-in-life
is to be **operationally active in the market making money**.

This correction (logged in CBU findings §7.0) re-frames every
workspace's relationship to the CBU:

| Workspace | Role relative to CBU |
|---|---|
| **CBU** | The money-making apparatus itself — the entity whose operational lifecycle IS the point |
| **IM** | The configuration that makes the CBU trading-capable (part of the apparatus) |
| **KYC** | The gate controlling whether we're allowed to service the CBU |
| **Deal** | The commercial agreement letting us earn revenue for servicing the CBU |

**Consequence for the reconciliation:** many of the gaps flagged
below are symptoms of treating the CBU as a passive record rather
than as the active trading unit. The reconciliation begins here,
not at the gap catalogue.

---

## 3. Universal patterns (v1.3 architectural amendments)

Patterns with **evidence from 2+ workspaces** — these are not
workspace-specific fixes; they're v1.3 architectural additions.

### 3.1 V1.3-CAND-2/5 — cross_workspace_constraints (P0)

**Pattern:** Slot X in workspace A depends on slot Y in workspace B
being in a specific state. Blocking / gating relationship.

**Evidence:**
- KYC produces `cases.status = APPROVED` consumed by IM (mandate
  activation) + Deal (CONTRACTED gate) + CBU (VALIDATED gate)
- Deal produces `deal.status = ACTIVE` consumed by billing +
  (implicitly) CBU operational readiness
- CBU's `VALIDATED` consumed by IM trading_profile activation

**Proposed v1.3 schema:**

```yaml
cross_workspace_constraints:
  - id: deal_contracted_requires_kyc_approved
    source_workspace: kyc
    source_slot: kyc_case
    source_state: APPROVED
    target_workspace: deal
    target_slot: deal
    target_transition: "* -> CONTRACTED"
    severity: error
```

Validator enforces at transition time; no projection.

### 3.2 V1.3-CAND-10 — SUSPENDED as a required universal state (P0)

**Pattern:** Every long-lived commercial entity needs a
pause-and-restore state distinct from WINDING_DOWN (exit intent)
and TERMINATED (terminal).

**Evidence (3 workspaces + sub-slot):**
- Deal G-5: deal-level SUSPENDED missing
- CBU G-2: CBU-level SUSPENDED missing
- KYC implicit: red_flag BLOCKING effectively is suspension
- Already present in sub-slots: `billing_profile.SUSPENDED`,
  `investor.SUSPENDED`, `holding.SUSPENDED`

**Proposed v1.3 convention:** Any slot with
`cardinality: root | mandatory` and `expected_lifetime: long_lived`
SHOULD include a SUSPENDED state with bidirectional transitions to
the preceding operational state. Validator warning if absent.

### 3.3 V1.3-CAND-11 — parent-child state dependency (hierarchy) (P0)

**Pattern:** Child entity's operational state depends on parent's
state. Master funds have feeders; master contracts have schedules;
KYC cases have entity workstreams.

**Evidence (3 workspaces):**
- Deal G-9: master agreement → schedule → addendum hierarchy
- CBU G-12: master fund → feeder fund; umbrella → compartments
- KYC (existing): case → entity_workstream → screening (this one
  is already modelled, providing the pattern reference)

**Proposed v1.3 schema:** Slots gain optional `parent_slot:` and
`state_dependency:` fields. Child cannot be ACTIVE if parent is
SUSPENDED. State cascade from parent to child on transitions.

### 3.4 V1.3-CAND-13 — cross-workspace aggregate state (P0, NEW)

**Pattern:** DIFFERENT from CAND-2/5. This is **projection /
derivation**, not blocking. A slot's state is DERIVED from the
conjunction of other workspaces' states, not blocked by them.

**Evidence (emerging from CBU reframe):**
- `cbu.operationally_active` = `kyc.APPROVED` AND `deal.ACTIVE` AND
  `im.trading_enabled` AND `cbu_evidence.verified`
- Likely also: `client.fully_onboarded` = Deal.ACTIVE AND
  all-CBUs-VALIDATED AND all-KYC-APPROVED

**Proposed v1.3 schema:**

```yaml
derived_cross_workspace_state:
  - id: cbu_operationally_active
    host_workspace: cbu
    host_slot: cbu
    host_state: operationally_active
    derivation:
      all_of:
        - { workspace: kyc, slot: kyc_case, state: APPROVED }
        - { workspace: deal, slot: deal, state: [CONTRACTED, ONBOARDING, ACTIVE] }
        - { workspace: im, slot: trading_profile, state: [trade_permissioned, actively_trading] }
        - { workspace: cbu, slot: cbu_evidence, predicate: "all verified" }
```

Validator caches the derivation; UI exposes the aggregate as a
first-class state.

**This is the highest-leverage pattern of the four P0
candidates** — it enables the CBU DAG re-centring (§4.1) to stay
within workspace boundaries while still owning the operational
aggregate.

### 3.5 V1.3-CAND-7/9 — parallel / dual lifecycle composition (P1)

**Pattern:** An entity has TWO linked-but-distinct lifecycles that
share a junction point and run in sequence, owned by different
roles.

**Evidence (2 workspaces):**
- Deal G-10: commercial lifecycle (sales+BAC owned) + operational
  lifecycle (ops owned); junction at CONTRACTED
- CBU G-1: discovery lifecycle (compliance owned) + operational
  lifecycle (trading+ops owned); junction at VALIDATED

**Proposed v1.3 schema:** Slots can declare multiple linked
lifecycles with explicit junction states. Ownership labels on each
lifecycle drive reporting + permissions.

Also relates to CAND-1 (KYC UBO epistemic-vs-fact state) — same
duality pattern applied to knowledge vs fact.

### 3.6 V1.3-CAND-3/12 — periodic review cadence (P1)

**Pattern:** Regulatory refresh obligations on a cadence
(calendar-driven, risk-tiered).

**Evidence (2 workspaces):**
- KYC: periodic case re-review (annual high-risk, biennial
  low-risk)
- CBU evidence: different validity windows per evidence_type
  (formation docs once-off, UBO declarations annual, sanctions
  screening rolling)

**Proposed v1.3 schema:** `periodic_review_cadence:` and
`validity_window:` on slots / sub-slot types. Integrates with Layer
3 scheduler.

### 3.7 V1.3-CAND-8 — commercial-commitment tier convention (P1)

**Pattern:** Verbs that emit commercial commitment to a
counterparty should default to tier ≥ requires_confirmation.

**Evidence (single workspace, but pattern holds):**
- Deal: agree-rate-card, activate-profile, generate-invoice (all
  raised to requires_confirmation during T2-D-4)
- Applies logically to: contract.submit, contract.sign (out of
  current workspace scope)

**Proposed v1.3 convention:** Documentation-level guidance for
tier-apply script. Tier-raise heuristic: if verb emits external
commercial commitment, default tier raised.

### 3.8 V1.3-CAND-6 — DB-triggered state transitions (P2)

**Pattern:** Some state transitions are fired by database triggers,
not verb execution. Schema assumes every transition has a verb;
this one doesn't.

**Evidence (single workspace):**
- Deal: `deal_rate_cards.status AGREED → SUPERSEDED` triggered by
  `idx_deal_rate_cards_one_agreed` when a new AGREED is inserted

**Proposed v1.3 schema:** Add
`trigger_source: database | verb | scheduler | external` to
transitions. Validator enforces verb-column consistency.

### 3.9 Summary — v1.3 candidate ranking after reconciliation

| Candidate | Evidence | Priority | Nature |
|---|---|---|---|
| V1.3-CAND-2/5 | 3 workspaces | **P0** | Schema: cross_workspace_constraints |
| V1.3-CAND-10 | 3 workspaces | **P0** | Convention: SUSPENDED universal |
| V1.3-CAND-11 | 3 workspaces | **P0** | Schema: parent-child state dependency |
| V1.3-CAND-13 | 1 workspace (new) | **P0** | Schema: cross-workspace aggregate state — highest leverage |
| V1.3-CAND-7/9 | 2 workspaces | P1 | Schema: dual lifecycle composition |
| V1.3-CAND-3/12 | 2 workspaces | P1 | Schema: periodic review cadence |
| V1.3-CAND-8 | 1 workspace | P1 | Convention: commercial-commitment tier |
| V1.3-CAND-1 | 1 workspace | P2 | Schema: epistemic-vs-fact state (subsumable by CAND-7/9) |
| V1.3-CAND-4 | 1 workspace | P2 | Schema: remediation workflow |
| V1.3-CAND-6 | 1 workspace | P2 | Schema: trigger_source enum |

**4 P0 candidates ship as the v1.3 core.** The remaining candidates
are P1/P2 and can ship in v1.3 appendices or v1.4.

---

## 4. Workspace-level re-centring + gaps

### 4.1 CBU — major rework (operational-purpose re-centring)

From CBU findings §7.0. Five foundational concerns:

| # | Concern | Remediation |
|---|---|---|
| 1 | Operational lifecycle belongs to CBU | Add states beyond VALIDATED: ACTIVE, SUSPENDED, WINDING_DOWN, OFFBOARDED, ARCHIVED. Schema expansion required. |
| 2 | Revenue realization missing | Add slot(s) for first-trade-executed, trading-activity tracking, dormancy detection. Or: receive these from IM as projection (depends on IM §10 rework). |
| 3 | Service consumption needs state | Add per-service-per-CBU state machine: proposed → provisioned → active → suspended → wound-down. Currently `cbu.add-product` / `cbu.remove-product` is CRUD. |
| 4 | `cbu_category` gates lifecycle | Expand `product_module_gates` section; category drives conditional slot activation + lifecycle variance. |
| 5 | Market-facing identity first-class | Promote LEI/BIC/depo-account references from obliquely-referenced to first-class slot on CBU. Projected from adjacent workspaces. |

**Prerequisite:** V1.3-CAND-13 (aggregate state) lands first — CBU
can then AGGREGATE kyc+deal+im+evidence rather than duplicate their
state.

**Estimated effort:** ~3 hours (DAG rework) + schema migration for
expanded `chk_cbu_status`.

**CBU G-1 through G-13 (§7.1-7.3) — most collapse into this rework:**

| Gap | Collapses into |
|---|---|
| G-1 Operational lifecycle | Foundational #1 |
| G-2 SUSPENDED | V1.3-CAND-10 universal |
| G-3 Manco state machine (regulatory cascade) | CBU-specific — see §4.5 |
| G-4 Change-of-control workflow | Partial overlap with V1.3-CAND-4 remediation; CBU-specific shape |
| G-5 Investor REDEEMING state | CBU-specific fix — small |
| G-6 Evidence-type-specific cadence | V1.3-CAND-12 |
| G-7 Share-class lifecycle | CBU-specific fix — small |
| G-8 Holding RESTRICTED/PLEDGED/FROZEN | CBU-specific fix — small |
| G-9 CBU-level CA events | CBU-specific fix — medium |
| G-10 Entity proper-person refresh | V1.3-CAND-12 (evidence cadence applied to person-level attestation) |
| G-11 Soft vs hard delete | CBU-specific — small |
| G-12 CBU hierarchy | V1.3-CAND-11 |
| G-13 Under-remediation state | V1.3-CAND-4 |

**Net CBU-specific remediation work after v1.3 lands:** ~5 small/
medium targeted fixes (G-3, G-5, G-7, G-8, G-9, G-11).

### 4.2 IM — medium rework (phase-axis re-anchor)

From IM pilot findings §10 addendum.

- Re-anchor `overall_lifecycle` phases on CBU-trading-enablement
  (dormant → configuring → trade-permissioned → actively-trading →
  restricted → suspended → winding-down → retired).
- Data lifecycle becomes sub-process.
- Phase derivation clauses re-expressed in CBU-observable terms.
- Add `trading_activity` slot (first-trade-at, last-trade-at,
  dormancy detection) — provides the data CBU §4.1 #2 needs.
- Add cross-slot constraint: CBU.actively-trading requires
  trading_profile.ACTIVE + trading_activity.first_trade_at IS NOT
  NULL.

**Estimated effort:** ~1 hour.

**Dependency:** sequenced AFTER CBU re-centring (§4.1) so the new
IM phases have a consumer; trading_activity projects into CBU
aggregate (CAND-13).

### 4.3 Deal — targeted fixes (no re-centring)

Deal correctly centred on our commercial servicing revenue. From
Deal findings §7:

**P0 (Deal-specific after v1.3 lands):**

| Deal gap | Disposition |
|---|---|
| G-1 BAC internal approval gate | **Deal-specific** — add BAC_APPROVAL state between NEGOTIATING and KYC_CLEARANCE; 3 new verbs (deal.submit-for-bac, deal.bac-approve, deal.bac-reject). Schema migration. |
| G-2 Pricing-committee approval | **Deal-specific** — add PENDING_INTERNAL_APPROVAL + APPROVED_INTERNALLY to rate_card lifecycle. |
| G-3 Terminal state granularity | **Deal-specific** — split CANCELLED into LOST / REJECTED / WITHDRAWN / CANCELLED. Schema migration. |
| G-4 Amendment lifecycle | **Deal-specific + leverages CAND-7/9** — parallel `deal_amendment` lifecycle. |
| G-5 SUSPENDED | V1.3-CAND-10 universal |
| G-6 KYC as gate not phase | **Deal-specific** — remove KYC_CLEARANCE phase; model as precondition (uses V1.3-CAND-2/5 or CAND-13). |
| G-7 SLA lifecycle | **Deal-specific** — promote `deal_sla` stateful. Schema migration. |
| G-8 Internal vs counterparty participants | **Deal-specific** — model internal accountability as deal-level attributes. |
| G-9 Deal hierarchy (master/schedule) | V1.3-CAND-11 |
| G-10 Commercial vs operational duality | V1.3-CAND-7/9 |
| G-11 Legal review phase | Flag, not urgent |
| G-12 Expected revenue | Out of DAG scope |
| G-13 Coverage / cross-sell | Partial; flag |

**Estimated effort:** G-1 + G-2 + G-3 + G-7 + G-8 ≈ 3 hours +
schema migration for G-1, G-3, G-7.

### 4.4 KYC — no rework

KYC correctly centred on the validation decision. Post-approval
periodic review (CAND-3) is legitimate re-entry; already flagged.

### 4.5 Cross-workspace ownership clarifications

During authoring the three workspaces often referenced the same
table/entity. The reconciliation clarifies ownership:

| Entity / slot | Canonical owner | Referenced by |
|---|---|---|
| `cbus` | **CBU workspace** | IM (scope: per_cbu), KYC (case.cbu_id), Deal (via commercial_client_entity) |
| `cases` (KYC) | **KYC workspace** | CBU (cross-workspace gate), Deal (cross-workspace gate) |
| `deals` | **Deal workspace** | CBU (via primary_client_group), KYC (case sponsorship) |
| `client_group` | **IM workspace** (group.discovery_status_lifecycle) | CBU + KYC + Deal all reconcile-existing |
| `trading_profile` | **IM workspace** | CBU aggregates its state (post-CAND-13) |
| `manco` | **CBU workspace** (new — G-3) | IM may need manco-cascade hooks |
| `entity_proper_person` / `entity_limited_company_ubo` | **CBU workspace** | KYC workstream lifecycle references these |

---

## 5. Remediation sequence (proposed)

Dependency-ordered. Each phase can be executed independently once
predecessors land.

### Phase R-1: v1.3 spec draft (no code changes)

Draft `catalogue-platform-refinement-v1_3.md` codifying:
- 4 P0 candidates (CAND-2/5, CAND-10, CAND-11, CAND-13) as schema
  + convention additions
- 3 P1 candidates (CAND-7/9, CAND-3/12, CAND-8) as appendices
- Migration guide from v1.2 → v1.3
- Validator + runtime impact

**Effort:** 2-3 hours spec authoring. No DAG / code changes.

### Phase R-2: v1.3 validator + schema support

Extend `dsl-core/config/types.rs` + validator to support:
- `cross_workspace_constraints:` block
- `derived_cross_workspace_state:` block
- `parent_slot:` + `state_dependency:` on slots
- `periodic_review_cadence:` + `validity_window:`
- `trigger_source:` enum on transitions
- SUSPENDED-universal warning lint

**Effort:** 1-2 days engineering. Unblocks all workspace-level
rework.

### Phase R-3: CBU DAG re-centring (major)

Once R-2 lands, re-author `cbu_dag.yaml`:
- Add operational lifecycle states (ACTIVE, SUSPENDED, WINDING_DOWN,
  OFFBOARDED, ARCHIVED)
- Add `derived_cross_workspace_state` for `cbu.operationally_active`
- Re-express service consumption as per-service state machine
- Promote market-facing identity slots

**Effort:** ~3 hours DAG + schema migration for `chk_cbu_status`
expansion.

### Phase R-4: IM phase-axis re-anchor (medium)

Once R-3 lands, re-author `instrument_matrix_dag.yaml`:
- Re-anchor `overall_lifecycle` on CBU-trading-enablement
- Add `trading_activity` slot
- Data lifecycle → sub-process

**Effort:** ~1 hour.

### Phase R-5: Deal targeted fixes

Independent of R-3/R-4. Deal-specific remediations:
- G-1 BAC approval gate (new states + 3 verbs + schema)
- G-2 Pricing internal approval
- G-3 Terminal granularity split (schema)
- G-7 SLA lifecycle (schema)
- G-8 Internal accountability attributes

**Effort:** ~3 hours + schema migrations.

### Phase R-6: CBU targeted gaps

After R-3 lands, remaining CBU-specific fixes:
- G-3 Manco state machine
- G-5 Investor REDEEMING
- G-7 Share-class lifecycle
- G-8 Holding RESTRICTED/PLEDGED/FROZEN
- G-9 CBU-level CA events
- G-11 Soft vs hard delete distinction

**Effort:** ~2 hours (collectively, small/medium each).

### Phase R-7: Test fixture hygiene cleanup

Pure fixture drift cleanup across Deal (2 bugs) + CBU (8 bugs).
Non-blocking; can interleave.

**Effort:** ~30 min.

### Total estimated effort

| Phase | Effort | Dependencies |
|---|---|---|
| R-1 v1.3 spec draft | 2-3 hr | — |
| R-2 validator + schema | 1-2 days | R-1 |
| R-3 CBU re-centring | 3 hr | R-2 |
| R-4 IM phase-axis | 1 hr | R-3 |
| R-5 Deal targeted fixes | 3 hr | R-2 |
| R-6 CBU targeted gaps | 2 hr | R-3 |
| R-7 Fixture cleanup | 30 min | — |
| **Total** | **~12-18 hr** | Serial critical path = R-1 → R-2 → R-3 → R-4 |

---

## 6. Decisions needed from Adam before executing

### D-1. Scope of v1.3 core — P0 candidates only, or include P1?

**Option A:** v1.3 ships only the 4 P0 candidates (CAND-2/5,
CAND-10, CAND-11, CAND-13). Cleaner, tighter, faster to land.
Recommendation.

**Option B:** v1.3 includes all 4 P0 + 3 P1 (CAND-7/9, CAND-3/12,
CAND-8). Richer but larger surface; longer spec authoring.

### D-2. CBU schema expansion — this cycle or deferred?

CBU re-centring (R-3) requires expanding `chk_cbu_status` CHECK
constraint from 5 states to 10 (+SUSPENDED, +ACTIVE,
+WINDING_DOWN, +OFFBOARDED, +ARCHIVED). This is a forward-only
schema migration.

**Option A:** Expand now alongside R-3. Clean.
**Option B:** Defer to a Tranche 3 migration window. R-3 models
the states in DAG but schema lags temporarily.

### D-3. CAND-13 implementation model — derived or materialised?

Aggregate state can be:

**Option A:** Computed on-the-fly by validator/runtime (pure
derivation). Zero storage; small compute cost per query.

**Option B:** Materialised via trigger or scheduled job (stored
in `cbu.operational_state` column). Storage cost; change
propagation cost.

**Option C:** Hybrid — materialised-with-staleness (cached with
timestamp; recompute if stale). Performance middle ground.

### D-4. Who owns the v1.3 spec draft — me or external?

**Option A:** I draft R-1 in the next session. Carries forward
the Tranche 2 context directly.
**Option B:** External / human drafts R-1; I review. Cleaner
separation but loses context.

### D-5. Sequencing — ship v1.3 first, or Tranche 3 workspaces first?

After R-2 (v1.3 support) lands, parallel paths are possible:
- Continue Tranche 3 (remaining non-primary workspaces:
  book-setup, onboarding-request, product-service-taxonomy,
  semos-maintenance, session-bootstrap) using v1.3 conventions from
  day one
- Fix existing 4 primary workspaces (R-3 to R-6) to align with
  v1.3

**Option A:** Ship v1.3 fixes to the 4 primary workspaces first
(R-3 to R-7). Then Tranche 3.
**Option B:** Tranche 3 first using v1.3 conventions; primary-
workspace fixes in a separate pass.
**Option C:** Interleave — R-3 (CBU) + R-5 (Deal) in parallel
with Tranche 3 authoring.

---

## 7. Tranche 2 closure criteria

Tranche 2 is considered **fully closed** when:

- [x] All 4 primary workspace DAGs authored (done 2026-04-23)
- [x] All 4 primary workspaces have three-axis declarations
      (done 2026-04-23; 584 / 1184 verbs)
- [x] All 4 findings docs published including business-reality
      reviews (done 2026-04-23)
- [x] Cross-workspace reconciliation pass document (this document)
- [ ] Adam has reviewed and adjudicated D-1 through D-5
- [ ] v1.3 spec drafting owner assigned (D-4)
- [ ] Execution plan for R-1 through R-7 approved

At the D-1..D-5 decisions point, Tranche 2 formally CLOSES and
execution of the reconciliation remediation begins.

**Reconciliation pass end.**

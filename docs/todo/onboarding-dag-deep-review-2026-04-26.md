# Onboarding DAG Deep Review — 2026-04-26

> **Scope:** business-lens deep review of the 9 onboarding DAGs against the four-layer commercial → operational chain, plus per-slot orphan-state analysis.
>
> **Inputs:** `rust/config/sem_os_seeds/dag_taxonomies/*.yaml` (9 files), all verb YAML in `rust/config/verbs/**`, all macros in `rust/config/verb_schemas/macros/**`.
>
> **Lens:** Adam's coined four-layer chain — Deal (commercial super-DAG) → CBU (resource taxonomy) → Product Services (generic lifecycle) → Lifecycle Resources (BNY-specific).

---

## §1 Executive summary

Three structural findings of decreasing severity drive everything else in this report.

**Finding 1 — Layer-4 (Lifecycle Resources DAG) is missing entirely.** There is no `lifecycle_resources_dag.yaml`. The product → service → resource chain dead-ends at `instrument_matrix_dag.service_resource` (provisioned → activated → suspended → decommissioned), which is doubly-loaded: it is both the product-service binding AND the BNY-application binding. No DAG separates *what generic capability* (custody-settlement) from *which BNY system* (CCC-Settlement-2.7-prod-eu). The runtime gate pipeline cannot enforce "service binds to a live resource" because no lifecycle exists for the resource side.

**Finding 2 — Layer-3 (Product Services) has no lifecycle.** `product_service_taxonomy_dag.yaml` exists but every slot is **stateless** (workspace_root, product, service, service_resource, attribute). Service definitions live as a flat registry — there is no DAG-level lifecycle for "draft service → published service → versioned service → retired service". Compare with semos_maintenance_dag's attribute_def_lifecycle (ungoverned → draft → active → deprecated → retired) which is the right shape for governed service catalogue entries. The CBU-side `service_consumption` slot IS lifecycled (proposed → provisioned → active → suspended → winding_down → retired), so the *consumer* side works, but the *catalogue* side does not.

**Finding 3 — KYC bypasses the GatePipeline entirely; Instrument Matrix has 8 of 12 stateful slots unwired.** Of the 87 `transition_args:` declarations across 13 verb files, **zero** are in KYC verbs (despite kyc_dag declaring 9 stateful slots with ~80 transitions). KYC progression today happens via verbs that the runtime cannot intercept for cross-workspace gate checks or cascade. Instrument Matrix has trading-profile.{restrict, lift-restriction} wired (2 entries) and **nothing else** — settlement_pattern_template, trade_gateway, service_resource, service_intent, delivery, reconciliation, corporate_action_event, collateral_management slots all have ZERO verbs declaring transition_args. CBU is the best-covered workspace and Deal is second-best; everywhere else is sparse.

These three findings compose: even if KYC and IM verbs were fully wired with `transition_args`, the chain would still terminate at a stateless layer-3 catalogue with no layer-4 at all. **Recommendation: prioritise the layer-3/4 modelling (a Tranche 4 candidate) before the wiring backfill — wiring without the missing layers gives partial enforcement of an incomplete model.**

---

## §2 Four-layer commercial chain trace

Adam's framing:

> Deal record (commercial super-DAG) → wraps BAC + KYC + Booking Principal tollgates, products, rate cards, supply contracts.
> Pivot to onboarding → CBUs subscribe to subset of contract products, instrument matrix extends per CBU.
> Products expand → product services (generic lifecycle, vendor-neutral).
> Services map → lifecycle resources (BNY-specific systems, application IDs).

### Layer 1: Deal (commercial super-DAG)

**Status: largely modelled.** `deal_dag.yaml` carries the commercial spine:

- **Primary lifecycle (deal):** PROSPECT → QUALIFYING → NEGOTIATING → BAC_APPROVAL → KYC_CLEARANCE → CONTRACTED + dual_lifecycle (ONBOARDING → ACTIVE → SUSPENDED → WINDING_DOWN → OFFBOARDED).
- **BAC tollgate:** explicit BAC_APPROVAL state with `deal.submit-for-bac` / `deal.bac-approve` / `deal.bac-reject` verbs (all wired with transition_args).
- **KYC tollgate:** cross_workspace_constraint `deal_contracted_requires_kyc_approved` (kyc.kyc_case.APPROVED → deal.KYC_CLEARANCE → CONTRACTED). Mode A blocking. Wired.
- **Pricing internal-approval cycle:** deal_rate_card lifecycle covers DRAFT → PENDING_INTERNAL_APPROVAL → APPROVED_INTERNALLY → PROPOSED → COUNTER_PROPOSED → AGREED. Wired.
- **Onboarding requests (per-CBU expansion):** deal_onboarding_request_lifecycle (PENDING → IN_PROGRESS → BLOCKED → COMPLETED / CANCELLED). Onboarding_request workspace re-uses this.
- **SLA + dual_lifecycle:** deal_sla_lifecycle wired through `deal.start-sla-remediation` etc.

**Booking Principal tollgate: NOT EXPLICITLY MODELLED IN A LIFECYCLE.** `booking_principal` appears as a **stateless** slot in instrument_matrix_dag. There is no booking-principal-clearance gate analogous to the KYC clearance gate. Adam's framing names BAC + KYC + Booking Principals as the three tollgates — only two of three are state-machine-modelled.

**Gap 1A:** Booking-Principal clearance has no lifecycle. Either (a) add a `booking_principal_clearance` slot to deal_dag with its own approve/reject lifecycle, or (b) wire booking_principal validation as a derived_cross_workspace_state feeding deal.CONTRACTED.

### Layer 2: CBU (resource taxonomy)

**Status: well-modelled, fully wired, but missing a Deal subscription link.** `cbu_dag.yaml` is the largest DAG (9 stateful slots) and holds the operational identity:

- Primary cbu_discovery_lifecycle + dual cbu_operational_lifecycle (junction at VALIDATED).
- service_consumption slot (proposed → provisioned → active → suspended → winding_down → retired) — this is what makes a CBU "subscribe to a contracted product".
- Manco regulatory_status, share_class, holding, investor, investor_kyc, cbu_evidence, cbu_corporate_action, cbu_disposition all lifecycled.
- One derived_cross_workspace_state: `cbu_operationally_active` aggregating KYC + Deal + trading_profile + cbu_evidence + service_consumption — this is the canonical Mode B tollgate Adam asked for. Confirmed wired.

**Pivot from Layer 1 to Layer 2 — Deal → CBU subscription:**
- *Indirect via shared KYC clearance.* `cbu_validated_requires_kyc_case_approved` and `deal_contracted_requires_kyc_approved` both gate on the same kyc_case.APPROVED. Both Deal and CBU progress through the same KYC gate, but neither directly references the other.
- **Gap 2A:** The CBU does not declare it requires a Deal in CONTRACTED/ONBOARDING/ACTIVE before activating service_consumption. A CBU could in principle progress to operationally_active without any Deal having been signed for that client — the model permits it. Real-world commercial flow should block this.
- **Recommendation:** add a cross_workspace_constraint `service_consumption_requires_deal_contracted` (deal.deal.CONTRACTED → cbu.service_consumption proposed → provisioned). This makes the Deal → CBU subscription pivot first-class.

### Layer 3: Product Services (generic lifecycle)

**Status: NO LIFECYCLE (stateless).** `product_service_taxonomy_dag.yaml`:

```yaml
slots:
  - workspace_root  (stateless)
  - product         (stateless)
  - service         (stateless)
  - service_resource (stateless)
  - attribute       (stateless)
```

There is no service-definition lifecycle (draft → published → versioned → retired). The product→service expansion is stored in tables but not state-machined.

**Pivot from Layer 2 to Layer 3 — CBU products → Product Services:**
- **Gap 3A:** No DAG link expressed. cbu_dag.service_consumption.proposed → provisioned should reference *which service catalogue entry* is being consumed — but the catalogue has no FSM, so cross_workspace_constraints cannot reference its state. The fact that "service X is published" is not state-machine-checkable.
- **Recommendation:** elevate product_service_taxonomy_dag to state-machined. Pattern after attribute_def_lifecycle:
  - `service` slot: ungoverned → draft → active → deprecated → retired (governance ceremony).
  - `service_version` slot: drafted → reviewed → published → superseded → retired.
  - Then service_consumption.provision can require service.active + service_version.published via cross_workspace_constraints.

### Layer 4: Lifecycle Resources (BNY-specific)

**Status: ABSENT.** No `lifecycle_resources_dag.yaml` exists.

Closest stateful surface: `instrument_matrix_dag.service_resource` (provisioned → activated → suspended → decommissioned). But this slot doubles for both layer-3 service-binding AND layer-4 application binding — there's no separation between "which capability the CBU consumes" vs "which BNY application instance executes it".

**Pivot from Layer 3 to Layer 4 — Services → Lifecycle Resources:**
- **Gap 4A:** Whole layer absent. There is no concept of "this CBU's custody service is bound to BNY system CCC-Settle-2.7 instance prod-eu-1, which is currently in MAINTENANCE_WINDOW".
- **Recommendation:** new DAG `lifecycle_resources_dag.yaml`. Slots:
  - `application` (stateless registry — id, name, vendor, owner_team, environment).
  - `application_instance`: state machine [PROVISIONED, ACTIVE, MAINTENANCE_WINDOW, DEGRADED, OFFLINE, DECOMMISSIONED].
  - `capability_binding`: state machine [DRAFT, PILOT, LIVE, DEPRECATED, RETIRED] — which application_instance fulfils which service for which CBU.
  - cross_workspace_constraint: cbu.service_consumption.active requires capability_binding.LIVE for that (cbu, service) pair.
  - Backend-driven transitions on application_instance via health-check signals (Mode B aggregation feeding service degradation alerts).

### Pivot wiring summary

| Pivot | Mechanism | Status | Remediation |
|-------|-----------|--------|-------------|
| Layer 1 → Layer 2 (Deal → CBU) | Indirect via shared KYC | **Partial** | Add `service_consumption_requires_deal_contracted` |
| Layer 2 → Instrument Matrix | `mandate_requires_validated_cbu` | ✅ Wired | — |
| Layer 2 → Layer 3 (CBU products → Services) | None | **Missing** | Elevate product_service_taxonomy_dag to stateful |
| Layer 3 → Layer 4 (Services → Resources) | None | **Missing** | New DAG |

---

## §3 Per-DAG orphan matrices

Legend (per-state inbound/outbound verb classification):

- `[GATED]` = verb declares `transition_args:` targeting this slot (gate pipeline can intercept)
- `[VERB-ONLY]` = verb exists in YAML but no `transition_args:` declared (bypasses gate pipeline)
- `[MACRO]` = reference is to an operator macro that expands to multiple verbs
- `[BACKEND]` = intentional non-verb transition (Mode B/C backend signal — NOT an orphan)
- `[MISSING]` = no verb or macro exists for this transition
- `[TERMINAL]` = state has no outbound (terminal-by-design)

Orphan flag `🔴` = state has no `[GATED]` or `[VERB-ONLY]` inbound (excluding entry states) OR no `[GATED]` / `[VERB-ONLY]` / `[TERMINAL]` outbound.

### 3.1 cbu_dag (9 stateful slots)

**Slot: cbu (cbu_discovery_lifecycle + cbu_operational_lifecycle dual)**

| State | Inbound | Outbound | Orphan |
|-------|---------|----------|--------|
| DISCOVERED (entry) | (entry) | `cbu.submit-for-validation` [GATED] | — |
| VALIDATION_PENDING | `cbu.submit-for-validation` [GATED]; `cbu.reopen-validation` [GATED] | `cbu.decide` [GATED] (×2 branches) | — |
| VALIDATED (junction) | `cbu.decide` [GATED] | `cbu.request-proof-update` [GATED]; junction → cbu_operational_lifecycle | — |
| UPDATE_PENDING_PROOF | `cbu.request-proof-update` [GATED] | `cbu.submit-for-validation` [GATED] | — |
| VALIDATION_FAILED | `cbu.decide` [GATED] | `cbu.reopen-validation` [GATED] | — |
| dormant (dual entry) | (entry) | `[BACKEND]` operationally_active becomes true | — |
| trade_permissioned | `[BACKEND]` first-trade signal | `[BACKEND]` first trade executed; `cbu.suspend` [GATED]; `cbu.begin-winding-down` [GATED] | — |
| actively_trading | `[BACKEND]` first trade; `cbu.unrestrict` [GATED]; `cbu.reinstate` [GATED] | `cbu.restrict` [GATED]; `cbu.suspend` [GATED]; `cbu.begin-winding-down` [GATED] | — |
| restricted | `cbu.restrict` [GATED] | `cbu.unrestrict` [GATED]; `cbu.suspend` [GATED]; `cbu.begin-winding-down` [GATED] | — |
| suspended | `cbu.suspend` [GATED] | `cbu.reinstate` [GATED]; `cbu.begin-winding-down` [GATED] | — |
| winding_down | `cbu.begin-winding-down` [GATED] | `cbu.complete-offboard` [GATED] | — |
| offboarded | `cbu.complete-offboard` [GATED] | `[BACKEND]` archival scheduler | — |
| archived | `[BACKEND]` archival | (none) | `[TERMINAL]` ✅ |

**Verdict:** ✅ fully wired. CBU is the gold-standard for v1.3 wiring.

**Slot: service_consumption** — ✅ all 6 transitions wired via service-consumption.{provision, activate, suspend, reinstate, begin-winddown, retire} [GATED].

**Slot: cbu_evidence**

| State | Inbound | Outbound | Orphan |
|-------|---------|----------|--------|
| PENDING (entry) | (entry); `cbu.attach-evidence` [VERB-ONLY] | `cbu.verify-evidence` [VERB-ONLY] | — |
| VERIFIED | `cbu.verify-evidence` [VERB-ONLY] | `[BACKEND]` time-decay | — |
| REJECTED | `cbu.verify-evidence` [VERB-ONLY] | (none) | `[TERMINAL]` ✅ |
| EXPIRED | `[BACKEND]` time-decay | `cbu.attach-evidence` [VERB-ONLY] | — |

**Verdict:** ⚠️ functional via verbs but bypasses GatePipeline. cbu.verify-evidence and cbu.attach-evidence should declare `transition_args` targeting cbu_evidence so cross-workspace gates can fire.

**Slot: investor** — investor.{mark-eligible, activate, suspend, reinstate, offboard} all [GATED]. ✅
**Slot: investor_kyc** — investor.{start-kyc, approve-kyc, reject-kyc} [GATED]; `investor.request-documents` [VERB-ONLY] for APPROVED → REFRESH_REQUIRED. ⚠️ minor gap.
**Slot: holding** — holding.{update-status, restrict, lift-restriction, pledge, release-pledge, close} all [GATED]; FROZEN ↔ ACTIVE via `[BACKEND]` sanctions signal. ✅
**Slot: cbu_corporate_action** — cbu-ca.{submit-for-review, approve, reject, withdraw, mark-implemented} all [GATED]. ✅
**Slot: cbu_disposition** — cbu.{flag-for-remediation, clear-remediation, soft-delete, restore, hard-delete} all [GATED]. ✅
**Slot: client_group_entity_review** — all transitions `[BACKEND]`. ✅ (intentional)
**Slot: share_class** — share-class.{launch, soft-close, reopen, hard-close, lift-hard-close, begin-winddown, close} — `share-class.close` [GATED] but launch/soft-close/reopen/hard-close/lift-hard-close/begin-winddown all [VERB-ONLY]. ⚠️ partial.
**Slot: manco** — manco.{approve, reject, flag-regulatory, clear-regulatory, suspend, partial-reinstate, begin-sunset, terminate} all [GATED]. ✅

**Slot: entity_proper_person** — `entity.identify` [VERB-ONLY], `entity.verify` [VERB-ONLY]. ⚠️
**Slot: entity_limited_company_ubo** — all PENDING outbound `[BACKEND]`; `ubo-registry.promote-to-ubo` [VERB-ONLY] for MANUAL_REQUIRED → DISCOVERED. ⚠️

**cbu_dag overall:** 9/13 slots fully GATED; 4 slots partially gated (cbu_evidence, investor_kyc, share_class, entity_*). No orphans.

### 3.2 deal_dag (6 stateful slots)

**Slot: deal (deal_commercial_lifecycle + deal_operational_lifecycle dual)** — all 18+ transitions wired. ✅

**Slot: deal_product** — `deal.update-product-status` [GATED]; `deal.agree-rate-card` [GATED]; `deal.remove-product` [VERB-ONLY]. ⚠️ minor.

**Slot: deal_rate_card** — DRAFT/PROPOSED/COUNTER_PROPOSED/AGREED transitions wired via `deal.{submit-for-pricing-approval, pricing-approve, pricing-reject, propose-rate-card, counter-rate-card, agree-rate-card}` all [GATED]. SUPERSEDED transition `[BACKEND]`. CANCELLED transition implicit. ✅

**Slot: deal_onboarding_request** — every transition uses `deal.update-onboarding-status` which is **NOT [GATED]** — declared but no transition_args. **🔴 Slot orphan-by-pipeline.**

**Slot: deal_document** — every transition uses `deal.update-document-status` [VERB-ONLY]. **🔴 Slot orphan-by-pipeline.**

**Slot: deal_ubo_assessment** — every transition uses `deal.update-ubo-assessment` [VERB-ONLY]. **🔴 Slot orphan-by-pipeline.**

**Slot: billing_profile** — `billing.{activate-profile, suspend-profile, close-profile}` [VERB-ONLY] (billing.yaml has no transition_args). **🔴 Slot orphan-by-pipeline.**

**Slot: billing_period** — `billing.{calculate-period, review-period, approve-period, generate-invoice, dispute-period}` [VERB-ONLY]. **🔴 Slot orphan-by-pipeline.**

**Slot: deal_sla** — `deal.{start-sla-remediation, resolve-sla-breach, waive-sla-breach}` [GATED]; ACTIVE → BREACHED `[BACKEND]`. ✅

**deal_dag overall:** 4 slots fully gated; 5 slots fully [VERB-ONLY] (orphan-by-pipeline). The "single overloaded verb" pattern (`deal.update-status`, `deal.update-onboarding-status`, etc.) means one verb per slot needs `transition_args` with `target_state_arg` — easy backfill.

### 3.3 kyc_dag (9 stateful slots) — **🔴 ENTIRE DAG ORPHAN-BY-PIPELINE**

Every transition uses real verbs (kyc-case.update-status, kyc-case.escalate, kyc-case.close, kyc-case.reopen, screening.run, screening.review-hit, evidence.verify, evidence.reject, etc.) plus macros (case.approve, case.reject, screening-ops.full, etc.). **None of these verbs declare `transition_args`.** The runtime DagRegistry's verb-to-transition index has no entries for any KYC slot.

**Operational impact:**
- Mode A (cross_workspace_constraints) targeting KYC outbound: kyc_dag has 0 cross_workspace_constraints anyway, so no Mode A breakage.
- BUT cbu_dag and deal_dag declare cross_workspace_constraints **inbound from KYC** (`cbu_validated_requires_kyc_case_approved`, `deal_contracted_requires_kyc_approved`). These look up kyc_case.state via SlotStateProvider — the read path works. The gate fires correctly when CBU/Deal verbs fire. ✅
- Mode B (derived_cross_workspace_state): cbu_dag's `cbu_operationally_active` aggregates kyc_case.APPROVED among other facts. Read-side works. ✅
- Mode C (cascade): no cascades target KYC slots. ✅

**Verdict:** KYC is read-only from the GatePipeline's perspective — its state IS consulted by other workspaces' gates, but no KYC verb fires through the gate pipeline itself. This is functional but means:
1. KYC verbs cannot themselves be gated by upstream workspaces (e.g., "kyc-case.escalate requires deal.NEGOTIATING" — currently impossible to express).
2. KYC verbs do not trigger cascades.
3. Dispatch metrics will show KYC verbs bypassing the gate pipeline (which may look like a bug to operators).

**Recommendation:** P1 — add `transition_args:` to ~10 hot KYC verbs (kyc-case.update-status, kyc-case.escalate, kyc-case.close, kyc-case.reopen, evidence.verify, evidence.reject, screening.run, screening.review-hit, ubo-registry.approve, ubo-registry.reject). This unlocks future Mode A constraints inbound to KYC and cascade dispatch from KYC.

**Slot orphan flag:** ALL 9 stateful slots flagged 🔴 by-pipeline (functional via [VERB-ONLY], but invisible to GatePipeline).

### 3.4 instrument_matrix_dag (12 stateful slots)

| Slot | Wiring status |
|------|---------------|
| group | All transitions `[BACKEND]` (research pipeline). ✅ intentional |
| trading_profile_template | `trading-profile.retire-template` [VERB-ONLY]. ⚠️ |
| settlement_pattern_template | settlement-chain.{add-hop, define-location, request-review, enter-parallel-run, go-live, abort-parallel-run, suspend, reactivate, deactivate-chain} **all [VERB-ONLY]**. **🔴** |
| isda_framework | stateless | — |
| corporate_action_policy | stateless | — |
| trade_gateway | trade-gateway.{enable-gateway, activate-gateway, suspend-gateway, reactivate-gateway, retire-gateway} **all [VERB-ONLY]**. **🔴** |
| trading_profile | 12 transitions, only `trading-profile.{restrict, lift-restriction}` [GATED]; the other 10 (submit, approve, reject, create-draft, enter-parallel-run, go-live, abort-parallel-run, suspend, reactivate, supersede, archive) **all [VERB-ONLY]**. ⚠️ |
| trading_activity | All transitions `[BACKEND]`. ✅ intentional |
| service_resource | service-resource.{activate, suspend, reactivate, decommission} **all [VERB-ONLY]**. **🔴** |
| service_intent | service-intent.{suspend, resume, cancel} **all [VERB-ONLY]**. **🔴** |
| delivery | delivery.{start, complete, fail, cancel} **all [VERB-ONLY]**. **🔴** |
| reconciliation | reconciliation.{activate, suspend, reactivate, retire} **all [VERB-ONLY]**. **🔴** |
| corporate_action_event | corporate-action-event.elect [VERB-ONLY]; default-applied `[BACKEND]`. **🔴** (one transition only) |
| collateral_management | collateral-management.{activate, suspend, reactivate, terminate} **all [VERB-ONLY]**. **🔴** |

**instrument_matrix_dag overall:** **8 of 12 stateful slots fully orphan-by-pipeline.** This is the single biggest wiring backlog after KYC.

**Cross-workspace impact:** trading_profile.SUSPENDED feeds CBU's cbu_operationally_active aggregate (Mode B reads). Read-side works. Mode A inbound (mandate_requires_validated_cbu) fires on the trading_profile DRAFT → SUBMITTED transition — but `trading-profile.submit` is [VERB-ONLY], so the gate pipeline cannot intercept it. **🔴 The CBU→IM pivot is wired via cross_workspace_constraint but not gated at runtime because the dispatching verb has no transition_args.**

**Recommendation:** P1 — add transition_args to trading-profile.{submit, approve, reject, enter-parallel-run, go-live, suspend, reactivate} (the 7 lifecycle-progression verbs). This makes the CBU→IM pivot actually gated.

### 3.5 book_setup_dag (1 stateful slot)

**Slot: book** — book.{create, select-structure, mark-ready, abandon} all [GATED] (4 transitions). The other 3 transitions (entities_provisioned → cbus_scaffolded, parties_assigned, mandates_defined) reference `cbu.create`, `cbu.assign-role`, `mandate.create` (all [VERB-ONLY]) and structure macros. ⚠️ partial — book's own state machine is half-driven by other workspaces' verbs which don't declare transition_args targeting book_setup.

**Recommendation:** P3 — book_setup is the journey workspace; orphan flagging is less critical because progression is observed via the cross-workspace_state of the CBUs being scaffolded. Acceptable as-is.

### 3.6 semos_maintenance_dag (5 stateful slots)

| Slot | Status |
|------|--------|
| changeset | `changeset.submit` [VERB-ONLY]; `changeset.enter-review` [VERB-ONLY]; `changeset.approve` [VERB-ONLY]; `changeset.reject` [VERB-ONLY]; `governance.publish` [GATED]. ⚠️ partial |
| attribute_def | `attribute.define` [VERB-ONLY]; `attribute.deprecate` [GATED]; `attribute.retire` [VERB-ONLY]; `attribute.propose-revision` [VERB-ONLY]. ⚠️ partial |
| derivation_spec | `derivation.activate` [VERB-ONLY]; `derivation.recompute-stale` [VERB-ONLY]; backend trigger. ⚠️ |
| service_resource_def | `service-resource.{sync-definitions, check-attribute-gaps, mark-complete}` all [VERB-ONLY]. **🔴** |
| phrase_authoring | `phrase.{check-collision, check-quality, submit-for-review, request-refinement, defer, propose, reactivate}` all [VERB-ONLY]; only `phrase.approve-proposal` and `phrase.reject-proposal` [GATED]. ⚠️ partial |

**Recommendation:** P3 — semos_maintenance gates compose loosely (governance changes are async). Current state acceptable.

### 3.7 onboarding_request_dag (1 stateful slot, reconcile-existing)

**Slot: onboarding_request** — uses deal_onboarding_request_lifecycle from deal_dag. All transitions use `deal.update-onboarding-status` [VERB-ONLY]. **🔴 Orphan-by-pipeline** (same as deal_dag.deal_onboarding_request).

### 3.8 product_service_taxonomy_dag — all stateless, N/A

### 3.9 session_bootstrap_dag — all stateless, N/A

### 3.10 Slot wiring summary

| DAG | Stateful slots | Fully GATED | Partial | Orphan-by-pipeline | Backend-only |
|-----|----------------|-------------|---------|--------------------|--------------|
| cbu | 9 | 6 | 3 | 0 | 1 (client_group_entity_review) |
| deal | 7 | 2 | 0 | 5 | 0 |
| kyc | 9 | 0 | 0 | **9** | 0 |
| instrument_matrix | 12 | 0 | 1 (trading_profile partial) | **8** | 2 (group, trading_activity) |
| book_setup | 1 | 0 | 1 | 0 | 0 |
| semos_maintenance | 5 | 0 | 4 | 1 | 0 |
| onboarding_request | 1 | 0 | 0 | 1 | 0 |
| product_service_taxonomy | 0 | — | — | — | — |
| session_bootstrap | 0 | — | — | — | — |
| **TOTAL** | **44** | **8** | **9** | **24** | **3** |

**24 of 44 stateful slots (54%) bypass the GatePipeline at the dispatching-verb level.**

---

## §4 Verb wiring gap inventory (transition_args backfill list)

Prioritised by structural importance — verbs that fire transitions consulted by Mode A constraints or Mode B aggregates rank highest.

### P1 — Verbs that should be gated (transitions referenced by cross-workspace constraints/aggregates)

These verbs fire transitions that other workspaces gate against. Without `transition_args`, the gate pipeline cannot dispatch these to the registry index:

1. **`trading-profile.submit`** — DRAFT → SUBMITTED is the target of `mandate_requires_validated_cbu` (CBU→IM Mode A constraint). **MUST GATE.**
2. **`trading-profile.approve`, `.reject`, `.go-live`, `.suspend`, `.reactivate`** — feed CBU's `cbu_operationally_active` derived state (Mode B). Should gate to enable post-dispatch cascade refresh.
3. **`evidence.verify`, `evidence.reject`** — feed cbu_evidence VERIFIED state which is part of `cbu_operationally_active` aggregate. Should gate.
4. **`kyc-case.update-status`, `kyc-case.close`** — feed kyc_case state which is consulted by `cbu_validated_requires_kyc_case_approved` and `deal_contracted_requires_kyc_approved`. Read-side works without gating, but gating enables cascade-on-approve.
5. **`deal.update-onboarding-status`** — feeds deal_onboarding_request_lifecycle. Mode B consumed by deal.ONBOARDING → ACTIVE precondition (ALL deal_onboarding_requests COMPLETED). Should gate.
6. **`service-resource.activate`, `.suspend`, `.decommission`** — IM service_resource binds to CBU service_consumption. Should gate.

**P1 backlog: ~12 verbs** to declare transition_args. Estimated effort: 2-4 hours of YAML editing + verb-test re-run.

### P2 — KYC GatePipeline coverage (consistency, not yet operationally required)

Adding transition_args to ~10 KYC progression verbs eliminates the per-workspace inconsistency. Prioritise:

1. `kyc-case.update-status`, `kyc-case.escalate`, `kyc-case.close`, `kyc-case.reopen`
2. `screening.run`, `screening.complete`, `screening.review-hit`
3. `ubo-registry.approve`, `ubo-registry.reject`, `ubo-registry.expire`
4. `red-flag.escalate`, `red-flag.resolve`, `red-flag.waive`

**P2 backlog: ~13 verbs.** Estimated effort: 2 hours.

### P3 — Slot completeness (operationally optional)

Backfill for slots flagged orphan-by-pipeline but not consulted by cross-workspace gates:

- billing.{activate-profile, suspend-profile, close-profile, calculate-period, review-period, approve-period, generate-invoice, dispute-period} — 8 verbs
- deal.{update-document-status, update-ubo-assessment, remove-product} — 3 verbs
- settlement-chain.{add-hop, request-review, go-live, suspend, deactivate-chain} — 5 verbs (representative subset)
- trade-gateway.{enable-gateway, activate-gateway, retire-gateway} — 3 verbs
- service-intent.{suspend, resume, cancel}, delivery.{start, complete}, reconciliation.{activate, retire}, collateral-management.{activate, terminate} — 11 verbs
- semos_maintenance backfills — 8 verbs
- entity.{identify, verify}, ubo-registry.promote-to-ubo, share-class.{launch, soft-close, hard-close, begin-winddown}, investor.request-documents, cbu.{verify-evidence, attach-evidence} — 10 verbs

**P3 backlog: ~48 verbs.** Estimated effort: 1-2 days.

---

## §5 Recommendations

In execution order:

### Recommendation R1 (P0) — Model layer 4 before backfilling layer 2/3 verbs
**Why:** wiring P1+P2+P3 verbs without a layer-4 DAG produces a partially-enforced model. The KYC and IM gaps will appear closed but the chain still breaks at the catalogue.
**What:** new DAG `lifecycle_resources_dag.yaml` with slots `application`, `application_instance`, `capability_binding` (state machines per §2 Layer 4). Schema: new tables `applications`, `application_instances`, `capability_bindings`.
**Sizing:** ~600 LOC of DAG YAML + 1 schema migration + ~25 new verbs (application.register/decommission, application_instance.{provision, activate, enter-maintenance, mark-degraded, restore, decommission}, capability_binding.{draft, pilot, promote-live, deprecate, retire}). Tranche-4-sized work.

### Recommendation R2 (P0) — Elevate product_service_taxonomy_dag to stateful
**Why:** without a service-definition lifecycle, cbu.service_consumption.proposed → provisioned cannot gate on "the service catalogue entry is published". Currently any service_id is acceptable.
**What:** add state machines to `service` and `service_version` slots in product_service_taxonomy_dag (pattern-match attribute_def_lifecycle).
**Sizing:** ~150 LOC DAG YAML + ~12 new verbs + 1 cross_workspace_constraint linking cbu.service_consumption → product_maintenance.service.

### Recommendation R3 (P0) — Add Booking Principal clearance lifecycle
**Why:** Adam's three tollgates are BAC + KYC + Booking Principals. BAC and KYC are state-machined; BP is not.
**What:** add `booking_principal_clearance` slot to deal_dag (or new `booking_principal_dag`) with lifecycle [PENDING → SCREENED → APPROVED / REJECTED → ACTIVE → SUSPENDED → REVOKED]. Add cross_workspace_constraint to deal.KYC_CLEARANCE → CONTRACTED requiring booking_principal_clearance.APPROVED.
**Sizing:** ~80 LOC DAG YAML + ~6 new verbs + 1 schema migration.

### Recommendation R4 (P0) — Add Deal → CBU subscription cross_workspace_constraint
**Why:** Currently a CBU can become operationally_active without any Deal having been signed. Real commercial flow requires a contracted deal.
**What:** add `service_consumption_requires_deal_contracted` cross_workspace_constraint targeting cbu.service_consumption proposed → provisioned.
**Sizing:** ~10 LOC YAML edit. Trivial.

### Recommendation R5 (P1) — Wire the 12 P1 verbs with transition_args
**Why:** Three of these (`trading-profile.submit`, `evidence.verify`, `deal.update-onboarding-status`) are referenced by Mode A/B mechanisms in already-active DAGs. Wiring them closes the runtime-enforcement gap.
**Sizing:** ~12 YAML edits + verb-test re-run. 2-4 hours.

### Recommendation R6 (P2) — Wire KYC verbs (~13)
**Why:** consistency + future cascade dispatch capability.
**Sizing:** ~13 YAML edits + verb-test re-run. 2 hours.

### Recommendation R7 (P3) — Backfill ~48 remaining verbs
**Why:** complete coverage. Operationally optional.
**Sizing:** 1-2 days of mechanical YAML edits.

### Sequencing

```
Tranche 4 (3-4 weeks):
  R1 + R2 + R3 in parallel (architectural)
  → R4 (1-line addition once R1-R3 land)
  → R5 + R6 + R7 (mechanical backfill)
```

Doing R5/R6/R7 BEFORE R1/R2/R3 closes the verb-wiring gap but leaves the architectural gap. Doing them AFTER means a single big-bang wiring slice once the new DAGs land — same total effort, cleaner narrative.

---

## §6 Verification queries

Re-run any of these to confirm gaps:

```bash
# Confirm transition_args declaration count and per-file breakdown
cd /Users/adamtc007/Developer/ob-poc/rust
grep -rc 'transition_args:' config/verbs/ | grep -v ':0$' | sort -t: -k2 -nr

# Confirm a specific KYC verb has no transition_args
grep -A 30 '^      update-status:' config/verbs/kyc/kyc-case.yaml | grep transition_args || echo "confirmed: no transition_args"

# Confirm trading-profile.submit has no transition_args
grep -B1 -A 20 'submit:' config/verbs/trading-profile.yaml | grep transition_args || echo "confirmed: no transition_args on submit"

# List all DAG taxonomies (confirms layer-4 absence)
ls config/sem_os_seeds/dag_taxonomies/

# Confirm product_service_taxonomy has all stateless slots
grep -E "stateless:|state_machine:" config/sem_os_seeds/dag_taxonomies/product_service_taxonomy_dag.yaml

# Run reconcile validator (should be clean — this is a docs-only review)
cargo run -p xtask --quiet -- reconcile validate
```

---

## Appendix A — DAG / verb reference index

| Topic | File |
|-------|------|
| 9 DAG taxonomies | `rust/config/sem_os_seeds/dag_taxonomies/*.yaml` |
| Verb catalogue | `rust/config/verbs/**/*.yaml` |
| Macro catalogue | `rust/config/verb_schemas/macros/*.yaml` |
| DAG loader + types | `rust/crates/dsl-core/src/config/dag.rs` |
| DAG validator | `rust/crates/dsl-core/src/config/dag_validator.rs` |
| DAG runtime registry | `rust/crates/dsl-core/src/config/dag_registry.rs` |
| GatePipeline | `rust/crates/dsl-runtime/src/cross_workspace/gate_checker.rs` |
| DerivedStateEvaluator | `rust/crates/dsl-runtime/src/cross_workspace/derived_state.rs` |
| CascadePlanner | `rust/crates/dsl-runtime/src/cross_workspace/hierarchy_cascade.rs` |
| TransitionArgs metadata | `rust/crates/dsl-core/src/config/types.rs` (TransitionArgs struct) |
| Dispatch hook | `rust/src/runbook/step_executor_bridge.rs` (with_gate_pipeline) |
| v1.3 spec | `docs/todo/catalogue-platform-refinement-v1_3.md` |

---

**End of review.** Next action: Adam to triage the 7 recommendations and confirm Tranche 4 scope (R1+R2+R3 architectural; R4 trivial; R5+R6+R7 mechanical).

# DAG Reachability Audit — Direct-Verb Coverage Across Four Onboarding Flows — 2026-05-02

> **Phase:** v1.5 follow-on — post Phase 5c-migrate, post v1.5b governance remediation.
> **Scope:** Trace utterance → DSL → SemOsVerbOp → DB for four flows (Deal, CBU creation, Product+InstrumentMatrix attachment, Service+Resource discovery), then audit each flow's DAG taxonomy for state-reachability gaps.
> **Authority:** Adam (provisional, per `tier-assignment-authority-provisional.md`).
> **Status:** Findings retained as governance evidence. The remediation campaign has been implemented; transient plan files were removed from completion cleanup.
> **Method:** Direct verbs only. A state X is "reachable" iff at least one verb declares `transition_args.target_state: X`, OR is `behavior: crud` writing X to the carrier's status column with a fixed value, OR is registered in `simple_status_op.rs::STATUS_FLIP_VERBS` with `target_state: X`. Macros, scenarios, plugin side-effects, and trigger-driven transitions are explicitly excluded — they are tracked as separate findings.

---

## 1. Headline

| Flow | DAG | States declared | Direct-reachable | Notes |
|------|-----|-----------------|------------------|-------|
| Deal | `deal_dag.yaml` (deal_commercial_lifecycle + dual_lifecycle.deal_operational_lifecycle) | 19 lifecycle + 8 rate-card + 6 SLA + 8 substate enum values | 19 + 8 + 6 + 2 | **2 P0 broken verbs · 6 unreachable substate values** |
| CBU | `cbu_dag.yaml` (primary + dual + disposition) | 17 | 16 | 1 by-design backend transition; 1 minor orphan branch |
| Product / IM | `instrument_matrix_dag.yaml` (trading_profile + service_consumption + service_intent) | 17 | 14 | 3 plugin-only entry states; template→instance clone is not a verb |
| Service / Resource Discovery | `lifecycle_resources_dag.yaml` (application_instance + capability_binding) + `cbu_lifecycle_instances` substrate | 11 + 6 | 9 + 1 | 2 unreachable terminal states; entire discovery pipeline emits no state transitions |

**Headline:** Deal is the only flow with a production-blocking issue. The other three are correct in the happy path but have observability/declarativeness gaps where state transitions happen as plugin side-effects rather than verb-declared transitions.

---

## 2. Cross-cutting findings

| # | Finding | Severity | Affected |
|---|---------|----------|----------|
| **F-1** | **SimpleStatusOp drift on deal** — `deal.submit-for-bac` and `deal.bac-approve` (registered in `simple_status_op.rs:271-296`) target enum values (`BAC_APPROVAL`, `KYC_CLEARANCE`) that no longer exist after the D-004 IN_CLEARANCE collapse (migration `20260429_carrier_08_deals_in_clearance_substates.sql`). Both verbs will violate `deals_status_check` if invoked. | **P0 BUG** | Deal |
| **F-2** | **Substate columns have no direct verb writers** — `deals.bac_status` and `deals.kyc_clearance_status` gate the IN_CLEARANCE → CONTRACTED transition per `deal_dag.yaml:394`, but only `deal.update-kyc-clearance` writes either column. **Six of the eight substate enum values are unreachable through any direct verb.** | **P0 BUG** | Deal |
| **F-3** | **Plugin-only entry states** — child-lifecycle entry states are often inserted as side-effects of a plugin-behaviour parent verb, with no declarative entry transition. `cbu.add-product` implicitly seeds a `cbu_service_intent` row at `active`; `trading-profile.create-draft` (plugin) is the only path to trading_profile `DRAFT`. | **P1 GAP** | Product/IM, Service Discovery |
| **F-4** | **Two-stage template-to-instance clone is not a verb** — the IM has explicit two-stage instances (`cbu_trading_profiles.cbu_id IS NULL` for templates vs `IS NOT NULL` for cloned instances). The clone happens inside `cbu.add-product` as a side-effect, with no FQN to bind to from a direct utterance. | **P1 GAP** | Product/IM |
| **F-5** | **Discovery pipeline is side-effect-only** — `discovery.run`, `discovery.explain`, `attributes.populate`, `attributes.rollup`, `provisioning.run`, `readiness.compute`, `pipeline.full` (16 verbs in `service_pipeline.rs`) execute as plugin ops with **NO `transition_args`**. They orchestrate side-effects without recording state progression in any DAG. The DAG cannot answer "is discovery in progress / done / failed for CBU X" through state queries. | **P1 GAP** | Service Discovery |
| **F-6** | **Backend-only transitions** — by design, several states are written by triggers/schedulers, not verbs. Acceptable for some (CBU `archived` per scheduler; `application_instance.DEGRADED` per health-check), but `cbu_lifecycle_instances.DECOMMISSIONED` has no `decommission` verb at all. | **P2 mixed** | CBU, Service Discovery |
| **F-7** | **Orphan verb decision branches** — `cbu.decide` (`cbu.yaml:1383-1478`) accepts `decision: REFERRED` but the DAG only encodes APPROVE/REJECT outcomes; the REFERRED path has no DAG state. | **P2** | CBU |

---

## 3. Flow 1 — Deal

**Lifecycles:** `deal_commercial_lifecycle` (PROSPECT → QUALIFYING → NEGOTIATING → IN_CLEARANCE → CONTRACTED + 4 terminal-negative); `dual_lifecycle.deal_operational_lifecycle` (ONBOARDING → ACTIVE → SUSPENDED / WINDING_DOWN → OFFBOARDED). Substate columns (added by D-004): `deals.bac_status` and `deals.kyc_clearance_status`, each with enum `pending|in_review|approved|rejected`.

### 3.1 P0 — broken SimpleStatusOp registrations

`rust/src/domain_ops/simple_status_op.rs:271-296`:

```rust
SimpleStatusConfig {
    fqn: "deal.submit-for-bac",
    target_state: "BAC_APPROVAL",   // NOT in deals_status_check
    state_col: "deal_status",
    ...
},
SimpleStatusConfig {
    fqn: "deal.bac-approve",
    target_state: "KYC_CLEARANCE",  // NOT in deals_status_check
    state_col: "deal_status",
    ...
},
```

Both states were collapsed into `IN_CLEARANCE` by `rust/migrations/20260429_carrier_08_deals_in_clearance_substates.sql` (D-004, 2026-04-29). The `deals_status_check` constraint allows only `IN_CLEARANCE`, not the old separated states. **Calling either verb today will fail at the DB layer.**

### 3.2 P0 — substate gap matrix

DAG precondition for `IN_CLEARANCE → CONTRACTED` (per `deal_dag.yaml`):

```
deals.bac_status = 'approved'
AND deals.kyc_clearance_status = 'approved'
AND parent kyc_case.state = APPROVED
AND every booking_principal_clearance.state in {APPROVED, ACTIVE}
AND deal_contract.state in {DRAFT, EXECUTED}
AND ≥1 deal_rate_card.state = AGREED
```

Substate writer matrix:

| Column | pending | in_review | approved | rejected |
|--------|---------|-----------|----------|----------|
| `bac_status` | implicit on IN_CLEARANCE entry | **none** | **none** | **none** |
| `kyc_clearance_status` | implicit on IN_CLEARANCE entry | **none** | `deal.update-kyc-clearance` | `deal.update-kyc-clearance` |

**Consequence:** with no verb writing `bac_status='approved'`, no deal can advance to CONTRACTED through direct verbs alone.

### 3.3 Fully reachable states

Primary: PROSPECT, QUALIFYING, NEGOTIATING, IN_CLEARANCE, ACTIVE, SUSPENDED, WINDING_DOWN, OFFBOARDED, LOST, REJECTED, WITHDRAWN, CANCELLED. Rate card: DRAFT, PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY, PROPOSED, COUNTER_PROPOSED, AGREED, SUPERSEDED, CANCELLED. SLA: as declared.

### 3.4 Canonical utterance trace

`"create a new deal for Allianz"`

| Layer | Resolution |
|-------|------------|
| Tier | ConstellationVerbIndex matches "create" + "deal" (invocation phrases: "create deal", "new deal for") |
| FQN | `deal.create` |
| Op | `sem_os_postgres::ops::deal::Create` (`rust/crates/sem_os_postgres/src/ops/deal.rs`) |
| SQL | `INSERT INTO "ob-poc".deals (deal_name, primary_client_group_id, ...) VALUES (...) RETURNING deal_id` |
| State | Default `deal_status='PROSPECT'`. Emits PendingStateAdvance `deal:prospect / deal/lifecycle`. |

---

## 4. Flow 2 — CBU

**Lifecycles:** primary discovery (5 states), dual operational (7), disposition (4).

### 4.1 Reachability matrix

| State | Lifecycle | Reachable | Inbound verbs |
|-------|-----------|-----------|---------------|
| DISCOVERED | primary | ✓ | entry |
| VALIDATION_PENDING | primary | ✓ | `cbu.submit-for-validation`, `cbu.reopen-validation` |
| VALIDATED | primary | ✓ | `cbu.decide` (decision=APPROVE) |
| UPDATE_PENDING_PROOF | primary | ✓ | `cbu.request-proof-update` |
| VALIDATION_FAILED | primary | ✓ | `cbu.decide` (decision=REJECT) |
| dormant | dual | ✓ | entry from VALIDATED |
| trade_permissioned | dual | ✓ | backend (operationally_active=true) |
| actively_trading | dual | ✓ | `cbu.reinstate`, `cbu.unrestrict` |
| restricted, suspended, winding_down, offboarded | dual | ✓ | SimpleStatusOp |
| **archived** | dual | **✗ direct** | by-design backend (archival scheduler) |
| active, under_remediation, soft_deleted, hard_deleted | disposition | ✓ | SimpleStatusOp |

### 4.2 Orphan

`cbu.decide` accepts `decision: REFERRED` but the DAG models only APPROVE/REJECT outcomes. REFERRED is currently audit-only (no state mutation). Either add a DAG state, or document REFERRED as audit-only in the verb description.

---

## 5. Flow 3 — Product / Instrument Matrix

**Slots audited (instrument_matrix_dag):** trading_profile (9 states), service_consumption (5), service_intent (3).

### 5.1 Reachability matrix

| State | Slot | Reachable | Notes |
|-------|------|-----------|-------|
| **DRAFT** | trading_profile | ✗ direct | Created by `cbu.add-product` (plugin side-effect) or `trading-profile.create-draft` (plugin, no SimpleStatusOp). No declarative entry verb. |
| SUBMITTED, APPROVED, PARALLEL_RUN, ACTIVE, SUSPENDED, REJECTED, SUPERSEDED, ARCHIVED | trading_profile | ✓ | Full SimpleStatusOp + plugin verb coverage |
| provisioned, active, suspended, winding_down, retired | service_consumption | ✓ | Full SimpleStatusOp coverage |
| **active** | service_intent | ✗ direct | Created in `active` by `cbu.add-product` plugin side-effect. No verb writes `active` explicitly. |
| suspended, cancelled | service_intent | ✓ | `service-intent.{suspend,cancel,resume}` |

### 5.2 Two-stage clone gap (F-4)

The IM declares two stages of `cbu_trading_profiles` instances (template `cbu_id IS NULL`, instance `cbu_id IS NOT NULL`). The clone is a side-effect of `cbu.add-product`, not a first-class verb. A direct utterance like *"clone the custody trading-profile template for Allianz"* has no FQN to bind to.

### 5.3 Canonical utterance trace

`"subscribe Allianz CBU to custody services"` → matches `cbu.add-product` (plugin) → SemOsVerbOp inserts `service_delivery_map` rows AND implicitly clones the trading-profile template into a CBU-scoped instance at `DRAFT`. The clone is invisible to the DAG state machine.

---

## 6. Flow 4 — Service / Resource Discovery

**Slots audited (lifecycle_resources_dag):** application_instance (6 states), capability_binding (5). Substrate: `cbu_lifecycle_instances.status` (6 enum values).

### 6.1 Reachability matrix

| State | Slot/table | Reachable | Notes |
|-------|------------|-----------|-------|
| PROVISIONED, ACTIVE, MAINTENANCE_WINDOW, OFFLINE, DECOMMISSIONED | application_instance | ✓ | `application-instance.{activate,enter-maintenance,exit-maintenance,take-offline,decommission,bring-online}` |
| **DEGRADED** | application_instance | **✗ direct** | DAG specifies `via "(backend: health-check signal)"`. No `application-instance.mark-degraded` verb. |
| DRAFT, PILOT, LIVE, DEPRECATED, RETIRED | capability_binding | ✓ | `capability-binding.{start-pilot,promote-live,deprecate,retire}` + cascade |
| **DECOMMISSIONED** | `cbu_lifecycle_instances` | **✗** | Enum allows it, no verb writes it |
| **PROVISIONING, PROVISIONED** | `cbu_lifecycle_instances` | ✗ direct | Set by triggers/initial insert |
| ACTIVE | `cbu_lifecycle_instances` | ✓ | `service-resource.reactivate` |

### 6.2 Discovery as a non-state-machine (F-5)

The 16 service-pipeline verbs (`service-intent.create`, `discovery.run`, `discovery.explain`, `attributes.{rollup,populate}`, `provisioning.run`, `readiness.compute`, `pipeline.full`, `service-resource.{check-attribute-gaps,sync-definitions}`, etc.) execute as plugin ops with **NO `transition_args`**. They orchestrate side-effects (resource inserts, attribute populates) but record no state progression. The DAG cannot model discovery progress; observability has to come from row counts in side tables.

### 6.3 Canonical utterance trace

`"discover services for Allianz CBU"` → `discovery.run` → `DiscoveryRunOp` → `service_pipeline::dispatch_service_pipeline_verb("discovery", "run", {cbu-id})` → reads `cbu_service_intent`, writes a `discovery_result` Record. **No DAG state mutation.**

---

## 7. Decision points for peer review

The findings split into three categories, each requiring a stance:

| Decision | Options |
|----------|---------|
| **D1.** How to fix the deal substate gating? | (a) Migrate `deal.submit-for-bac` and `deal.bac-approve` to dedicated SemOsVerbOp impls that write `bac_status` instead of `deal_status`. (b) Drop substates and fold back to separated states `BAC_APPROVAL`/`KYC_CLEARANCE` (reverts D-004). |
| **D2.** Should plugin side-effects be modelled as state transitions? | (a) Yes — add explicit verbs (`trading-profile.clone-from-template`, `service-intent.activate`, etc.) so direct utterances can drive them. (b) No — document them as side-effect-only and remove from DAG verb-coverage expectations. |
| **D3.** Should discovery pipeline emit DAG state? | (a) Yes — add a `discovery_state` slot (PENDING → EXECUTING → COMPLETE / FAILED). (b) No — observability stays row-count based, but verbs gain explicit `outputs` so the DAG can detect progress without a state machine. |
| **D4.** What about `cbu.decide REFERRED`? | (a) Add REFERRED as a DAG state with documented outbound transitions. (b) Keep REFERRED audit-only and document it. |
| **D5.** What about `cbu_lifecycle_instances.DECOMMISSIONED`? | (a) Add `service-resource.decommission` verb. (b) Remove DECOMMISSIONED from the enum if cascade-only. |

D1 is non-negotiable (P0 bug). D2-D5 are P1/P2 design decisions for peer review.

---

## 8. Evidence pointers

- DAG taxonomies: `rust/config/sem_os_seeds/dag_taxonomies/{deal_dag,cbu_dag,instrument_matrix_dag,lifecycle_resources_dag}.yaml`
- Verb YAMLs: `rust/config/verbs/{deal,cbu,trading-profile*,service*,discovery,provisioning*,readiness*,pipeline*}.yaml`
- SimpleStatusOp registry: `rust/src/domain_ops/simple_status_op.rs`
- Substate migration: `rust/migrations/20260429_carrier_08_deals_in_clearance_substates.sql`
- Schema constraints: `rust/migrations/master-schema.sql` (`deals_status_check`, `kyc_decisions_status_check`, `entity_workstreams_chk_workstream_status`, `cbu_lifecycle_instances_*`)
- Plugin op impls: `rust/crates/sem_os_postgres/src/ops/{deal,cbu,kyc_case,red_flag,service_pipeline,service_resource,discovery,trading_profile_ca}.rs`
- Sibling reviews to triangulate: `dag-coherence-review-2026-04-26.md`, `dag-business-review-evidence-2026-04-29.md`, `phase-2g-coherence-findings-2026-04-26.md`.

---

## 9. Out of scope

- Macro-driven reachability (the `assemble-cbu` family, `structure.product-suite-*`, screening-ops macros, etc.) — Adam will review macros separately.
- Trigger-driven state transitions (archival scheduler, health checks, cascade rules) — flagged where present, not assessed for correctness.
- Cross-workspace constraint completeness — covered separately in `dag-coherence-review-2026-04-26.md`.
- Composite states (`cbu_operationally_active` and similar derived projections) — these are derived, not transitioned; treated as out-of-scope for direct-verb reachability.

# Service-Resource Data Dictionary Capability — Research & Gap Analysis (v0.1)

> **Status:** Draft for peer review
> **Author:** Claude (research-only pass)
> **Date:** 2026-05-05
> **Purpose:** Characterize the "resource owners declare → CBU consolidates → activate → URL returned" capability that Adam called out as a key STP enabler for onboarding. Identify what exists, what is partially modelled, and what is missing.

---

## 1. The Capability Vision (as stated)

> The strategy is the SemOS has a data dictionary — and every resource should be able to create [a] dictionary with that resource[']s data requirements — then for a CBU onboarding all resources (and there will always be more than one — even for a single product) — can have their dictionar[ies] consolidated into an onboarding data request instance — and that is populated — and, when a resource data dictionary set is populated at the top level — then that resource activation instruction is sent to the resource owner — who then returns the resource url (usually an application account number set).

Restated as a control flow:

```
                       ┌───────────────────────────────────────────────────┐
                       │   SemOS Data Dictionary (AttributeDef registry)   │
                       └───────────────────────────────────────────────────┘
                                              │
                                              │  declared via
                                              ▼
              ┌───────────────────────────────────────────────────────────┐
              │  Resource Owner publishes "this resource needs A,B,C..."  │
              │  (one dictionary per service resource type, scoped)       │
              └───────────────────────────────────────────────────────────┘
                                              │
                       ┌──────────────────────┴──────────────────────┐
                       │   CBU onboarding starts (deal contracted)   │
                       │   Triggers N resources (always > 1)         │
                       └─────────────────────────────────────────────┘
                                              │
                                              ▼
              ┌───────────────────────────────────────────────────────────┐
              │  Onboarding Data Request (consolidated): union of all    │
              │  resource dictionaries, scoped to this CBU instance       │
              │  Tracks: required, populated, source, evidence, %complete │
              └───────────────────────────────────────────────────────────┘
                                              │
                                              │ populated to 100% (per resource set)
                                              ▼
              ┌───────────────────────────────────────────────────────────┐
              │  Activation instruction dispatched to resource owner      │
              │  (out-of-band — owner system / human team)                │
              └───────────────────────────────────────────────────────────┘
                                              │
                                              │ owner returns
                                              ▼
              ┌───────────────────────────────────────────────────────────┐
              │  Resource URL / external account number returned          │
              │  → resource instance flips to ACTIVE                       │
              │  → contributes to CBU operationally_active tollgate       │
              └───────────────────────────────────────────────────────────┘
```

Three clauses are load-bearing:

- **C1.** Each resource owner *authors* its data requirements (a dictionary slice) against the SemOS attribute registry.
- **C2.** Per CBU onboarding, all triggered resources' slices *consolidate* into a single, addressable, populatable instance.
- **C3.** Population → activation instruction → URL return → ACTIVE forms a closed loop with the resource owner.

Each of these has different fidelity in the codebase today. The rest of this doc reports against C1/C2/C3 in turn and lists remediation targets at the end.

---

## 2. What Exists Today

A summary table first, then evidence.

| Layer | Capability | Status | Where |
|------|-----------|--------|-------|
| Dictionary | SemOS AttributeDef + materialization to `attribute_registry` | **Solid** | `sem_os_core/src/attribute_def.rs`, `migrations/20260328_semos_attribute_materialization.sql` |
| Dictionary | Two-tier visibility (External governed / Internal lightweight) | **Solid** | `migrations/20260331_attribute_visibility.sql` |
| Dictionary | Derived attributes + staleness propagation | **Solid** | `migrations/20260327_derived_attribute_persistence.sql` |
| Resource declaration | SRDEF YAML with attribute requirements | **Solid** | `rust/config/srdefs/*.yaml`, `src/service_resources/srdef_loader.rs` |
| Resource declaration | `resource_attribute_requirements` table (DB projection of SRDEF reqs) | **Solid** | `migrations/024_service_intents_srdef.sql` |
| Resource declaration | Source policy, conditional reqs, evidence policy, constraints | **Solid** | same |
| Per-CBU consolidation | `cbu_unified_attr_requirements` (union across SRDEFs, with `required_by_srdefs`) | **Solid** | `migrations/025_cbu_unified_attributes.sql` |
| Per-CBU consolidation | `v_cbu_attr_gaps`, `v_cbu_attr_summary` (% populated) | **Solid** | same |
| Per-CBU consolidation | `cbu_service_readiness` with structured `blocking_reasons` | **Solid** | `migrations/027_service_readiness.sql` |
| Population | `cbu_attr_values` with source/evidence/explain refs | **Solid** | `migrations/025_cbu_unified_attributes.sql` |
| Population | `service-resource.set-attr` verb (per-instance, with state) | **Solid** | `rust/config/verbs/service-resource.yaml:331` |
| Activation gate | `service-resource.activate` validates mandatory attrs | **Solid** | `rust/config/verbs/service-resource.yaml:385` |
| Onboarding request | `deal_onboarding_requests` row (deal × cbu × product) with 5-state FSM | **Solid** | `migrations/20260429_carrier_01_cbu_service_consumption.sql` |
| Onboarding request | OnboardingRequest workspace + DAG | **Solid** | `rust/config/sem_os_seeds/dag_taxonomies/onboarding_request_dag.yaml` |
| Service consumption | `cbu_service_consumption` 6-state FSM, gated by capability_binding LIVE | **Solid** | same migration + `cbu_dag.yaml` |
| Provisioning | `service-resource.provision` accepts `instance-url`, `instance-id` | **Partial** | `service-resource.yaml:206`, auto-generates a URN if not provided |
| Gap analysis | `service-resource.check-attribute-gaps` (SRDEF↔SemOS alignment) | **Solid** | `service_pipeline_service_impl.rs:451` |
| Gap analysis | `service-resource.analyze-lifecycle-gaps` / `check-lifecycle-readiness` / `generate-lifecycle-plan` / `execute-lifecycle-plan` | **Solid** | `service-resource.yaml:1046–1183` |

**Headline:** the substrate (SemOS data dictionary, resource-side declaration, per-CBU consolidation, % populated, gap views, readiness states) is largely in place. The gaps are concentrated in **the C2/C3 closed loop**: there is no first-class *Onboarding Data Request* entity that binds a specific onboarding instance to a consolidated dictionary, and there is no dispatch/return contract with the resource owner.

### 2.1 Concrete shapes (to anchor the gap discussion)

**SemOS AttributeDefBody** (`sem_os_core/src/attribute_def.rs:18`):

```rust
pub struct AttributeDefBody {
    pub fqn: String,                        // e.g. "cbu.jurisdiction_code"
    pub data_type: AttributeDataType,
    pub evidence_grade: EvidenceGrade,
    pub constraints: Option<AttributeConstraints>,
    pub category: Option<String>,
    pub validation_rules: Option<Value>,
    pub applicability: Option<Value>,       // entity_types, jurisdictions, contexts
    pub is_required: Option<bool>,
    pub is_derived: Option<bool>,
    pub derivation_spec_fqn: Option<String>,
    pub visibility: Option<AttributeVisibility>,
    // ...
}
```

**SRDEF YAML** (`rust/config/srdefs/custody.yaml`):

```yaml
srdefs:
  - code: custody_securities
    resource_type: Account
    owner: CUSTODY                          # ← owner today is a free-text label
    provisioning_strategy: request          # create | request | discover
    per_market: true
    attributes:
      - id: market_scope
        requirement: required
        source_policy: [cbu, entity, manual]
        constraints: {type: array, items: string, min_items: 1}
      - id: settlement_currency
        requirement: required
        source_policy: [derived, cbu, manual]
        constraints: {type: string, pattern: "^[A-Z]{3}$"}
      - id: ssi_mode
        requirement: conditional
        condition: ssi_mode == 'standing'
        source_policy: [cbu, manual]
    depends_on: []
```

**CBU unified requirements** (`migrations/025_cbu_unified_attributes.sql:18`):

```sql
CREATE TABLE "ob-poc".cbu_unified_attr_requirements (
    cbu_id          UUID,
    attr_id         UUID,
    requirement_strength TEXT,    -- required | optional | conditional
    merged_constraints   JSONB,
    preferred_source     TEXT,
    required_by_srdefs   JSONB,   -- ["SRDEF::CUSTODY::Account::custody_securities", ...]
    conflict             JSONB,   -- non-null = constraint merge failure
    PRIMARY KEY (cbu_id, attr_id)
);
```

This is the closest thing today to "consolidated onboarding data request." But notice: it is keyed by `cbu_id` only — **not by onboarding_request_id, not by deal_id, not by product/service scope**. It is an evergreen rollup, not an instance.

---

## 3. Gap Analysis Against the Vision

### 3.1 C1 — Resource owners author their dictionary slice

**Status: ~80% there. Authoring path is YAML-on-disk, not in-system.**

What works today:

- SRDEF YAML files live under `rust/config/srdefs/` and are loaded by `srdef_loader.rs` and synced to `service_resource_types` + `resource_attribute_requirements` via `service-resource.sync-definitions`.
- Each SRDEF declaratively states required/optional/conditional attributes with source policy, constraints, and evidence policy.
- `service-resource.check-attribute-gaps` cross-checks every SRDEF attribute against the SemOS active registry and reports `ok | ungoverned | missing`.

Gaps:

- **G1.1 — Authoring is filesystem-only.** Resource owners cannot define/edit their dictionary slice through a verb. SRDEFs land via YAML edit → `sync-definitions` re-import. There is no `service-resource.define`, `service-resource.declare-attribute-requirement`, or governed authoring lifecycle for SRDEFs analogous to `attribute.define`. The `service_resource_def_lifecycle` state machine (`unsynced → synced → gaps_found ↔ complete`) governs the YAML-import side only — there is no propose/validate/publish ceremony.
- **G1.2 — Owner is a free-text label, not a principal.** SRDEFs carry `owner: "CUSTODY"` as a string. There is no link to a SemOS actor, role, ABAC subject, or notification target. Without that link, "send activation instruction to the resource owner" (C3) has no addressee.
- **G1.3 — No formal `ServiceResourceDef` SemOS object type.** Other governed primitives (AttributeDef, DerivationSpec, RequirementProfileDef) live in `sem_os_core` with full body structs and changeset ceremony. SRDEFs do not. They live in YAML and project to the operational store directly. Adam's vision treats the resource owner's dictionary slice as a first-class governed artefact — that requires lifting SRDEF into SemOS proper.
- **G1.4 — No semantic test for "is the SRDEF's attribute slice still valid given current AttributeDefs?"** beyond the `check-attribute-gaps` pass/fail. There is no notion of "this SRDEF is sealed against attribute_registry version N" — a dictionary version pin.

### 3.2 C2 — Per-CBU onboarding consolidates dictionaries into a single instance

**Status: ~50% there. The rollup exists but is not bound to an onboarding instance.**

What works today:

- For a given CBU, `cbu_unified_attr_requirements` rolls up requirements across all SRDEFs whose resources are needed; `v_cbu_attr_gaps` lists missing values; `v_cbu_attr_summary` reports `pct_complete`; `cbu_service_readiness` tracks per-(cbu, product, service) `ready | partial | blocked` with structured blocking reasons.
- `deal_onboarding_requests` exists with `(deal_id, cbu_id, product_id, request_status)` and a 5-state FSM (PENDING → IN_PROGRESS → BLOCKED ↔ IN_PROGRESS → COMPLETED | CANCELLED).
- `cbu_service_consumption` carries the per-(cbu, service_kind) lifecycle and references `onboarding_request_id` for attribution (S-15).

Gaps:

- **G2.1 — No `onboarding_data_request` entity.** There is no first-class entity that says "for this onboarding_request × these N service-resource-types, here is the consolidated set of attribute requirements, and here is the population state." `cbu_unified_attr_requirements` is keyed only on `cbu_id`. Two consequences:
  1. You cannot have two concurrent onboarding requests against the same CBU with different consolidated sets (which is fine if it never happens — but should be a deliberate constraint, not an accident of schema).
  2. The product/service scope isn't carried into the rollup — the unified requirements table mixes attributes required by *any* SRDEF that has been triggered for this CBU at any time. There is no way to ask "what's the onboarding data request for *this* CBU × *this* deal × *this* product?"
- **G2.2 — No per-resource-set sub-bundling within an onboarding instance.** Adam's vision implies each resource's dictionary remains addressable inside the consolidated instance ("when a resource data dictionary set is populated at the top level"). Today, the unified table loses that grouping — `required_by_srdefs` carries provenance per attribute but you can't ask "is the custody SRDEF slice fully populated?" without query gymnastics, and you certainly can't track per-resource-slice state (e.g., custody slice → ready, settlement slice → partial).
- **G2.3 — Triggering: which SRDEFs feed a given onboarding instance is not declarative.** Today the rollup is computed by joining `service_intents` × `product_services` × `service_resource_capabilities` × `resource_attribute_requirements`. That is correct as a derivation — but there is no captured snapshot ("for onboarding_request R, here are the 14 SRDEFs that are in scope, frozen at request creation"), so changes to product↔service bindings during the lifetime of an onboarding request silently retarget the requirements set.
- **G2.4 — No per-onboarding-request `pct_complete`.** `v_cbu_attr_summary.pct_complete` is per-CBU evergreen. The closest per-instance signal is `cbu_service_readiness.status` (ready/partial/blocked) per (cbu, product, service), but it is computed, not the same shape, and not aggregated against the onboarding_request.
- **G2.5 — No "set-level" ready signal.** "When a resource data dictionary set is populated at the top level" implies a per-resource readiness gate that emits an event ("custody slice ready for activation"). Today the activation gate sits inside `service-resource.activate` and only fires on attempt — there is no proactive "this slice is now complete, dispatch the activation instruction" emitter.

### 3.3 C3 — Activation instruction → resource owner → URL return

**Status: ~25% there. Verb surface exists for both legs but the end-to-end contract is missing.**

What works today:

- `service-resource.provision` accepts optional `instance-url` and `instance-id` arguments. If not supplied, it auto-generates `urn:ob-poc:{cbu_id}:{resource_code_lower}:{uuid}`. So the *slot* for a returned URL exists.
- `service-resource.activate` is the gate (mandatory attributes must all be set).
- `cbu_resource_instances` has dimensional grain (market_id, currency, counterparty_entity_id) and instance state (PENDING, PROVISIONING, ACTIVE, SUSPENDED, DECOMMISSIONED).
- The newer **Layer 4** model (`application_instances` + `capability_bindings`) provides per-environment app instances with their own state machine — relevant for "which BNY app provides the resource for this service" but is *separate* from the SRDEF-side resource instance. (This is itself a small gap — see G3.5.)

Gaps:

- **G3.1 — No "dispatch activation instruction" verb / outbox.** There is no `service-resource.dispatch-activation`, `resource-owner.notify`, or outbox effect that says "resource X for CBU Y is ready to be provisioned by the owner — here is the data packet — please return a URL." The phrase "activation instruction is sent to the resource owner" has no counterpart in the verb registry. The Phase 5e outbox infrastructure exists (`outbox::drainer::OutboxDrainerImpl`, with `MaintenanceSpawnConsumer` and `NarrateConsumer`) but no `ResourceOwnerNotifyConsumer`.
- **G3.2 — No "resource owner returns URL" inbound verb.** The shape `service-resource.confirm-activation` or `service-resource.set-external-reference` does not exist. The current pathway is: caller of `provision` supplies the URL up-front, or `set-attr` is used to write the external reference as an attribute value. Neither models a separate inbound event from the owner. There is no idempotency envelope, no correlation key (`provisioning_correlation_id`), no audit of "who returned this URL and when."
- **G3.3 — `cbu_resource_instances` does not carry a per-instance owner lifecycle.** Status moves PENDING → PROVISIONING → ACTIVE — but the transition that should mean "owner has returned the URL" is conflated with "we activated it from inside the system." Consequence: you cannot tell from the table whether the URL was self-generated, externally returned, or inherited.
- **G3.4 — No correlation between "set-level populated" (from G2.5) and dispatch.** Even if G2.5 were solved, there is no rule "when slice S becomes complete, fire `service-resource.dispatch-activation` for all resources of S." This is the spine of the closed loop and is currently missing.
- **G3.5 — Layer-4 ↔ SRDEF gap.** `application_instances`/`capability_bindings` capture "which BNY app instance provides the service" with their own LIVE state. The cross-workspace constraint `service_consumption_active_requires_live_binding` ensures the L4 binding is LIVE before service consumption can go ACTIVE. But the **SRDEF resource instance** (`cbu_resource_instances`) is independent of L4. So an SRDEF resource can be "ACTIVE" without a matching capability_binding being LIVE, and vice versa. The owner-side dispatch (G3.1) plausibly belongs at the L4 layer (because that's where the BNY app actually lives) — that needs deciding.
- **G3.6 — No structured payload contract for the activation instruction.** What gets sent to the owner? The unified attribute values? Full instance metadata? CBU + product context? There is no schema for the "dispatch packet."
- **G3.7 — No SLA / staleness on the dispatch leg.** If the owner takes 5 days to return a URL, what state is the resource in? PROVISIONING is currently implicit and not used. The state machine has no "AWAITING_OWNER_REGISTRATION" state.

### 3.4 Cross-cutting gaps

- **GX.1 — Dictionary-set versioning.** When SRDEF YAML changes mid-onboarding, what is the contract? The existing `service_resource_def_lifecycle` covers SRDEF authoring, not snapshotting against an open onboarding request. (Compounds with G2.3.)
- **GX.2 — No notion of "resource owner principal" in SemOS.** Tied to G1.2. Without principals, dispatch (G3.1) has no addressee and audit has no actor.
- **GX.3 — Verb naming/UI to surface the consolidated data request.** No verb today says "show me the onboarding data request for this onboarding instance." The narration / inspector / observatory each have partial views. A single canonical projection is missing.
- **GX.4 — Discoverability of returned URLs.** External reference / account number is currently expressed as either `instance_url` (varchar on `cbu_resource_instances`) or as an attribute value on `resource_instance_attributes`. The two-place encoding makes "give me all account numbers for this CBU" awkward and undermines the phrase "usually an application account number set."

---

## 4. Remediation Targets

Listed in implementation-priority order — the dependencies flow downward.

### T1. Lift `ServiceResourceDef` into SemOS as a first-class governed object  *(addresses G1.1, G1.3, G1.4, GX.1)*

- Author `ServiceResourceDefBody` in `sem_os_core` alongside `AttributeDefBody`/`DerivationSpecBody`.
- Add `service-resource.define` (governed, full changeset ceremony) and `service-resource.define-internal` (operational tier) verbs paralleling the attribute pattern.
- Materialize SemOS `service_resource_def` snapshots into `service_resource_types` + `resource_attribute_requirements` via a trigger analogous to `materialize_attribute_def_to_registry`.
- Keep YAML loader as a bootstrap path, but make SemOS-authored SRDEFs the source of truth. Pin each onboarding request to a specific SRDEF snapshot (closes GX.1).

### T2. Introduce a "Resource Owner" principal  *(addresses G1.2, GX.2)*

- Extend SRDEF body with an `owner_principal_fqn` (linking to a SemOS-known role/team/external system) — not a free-text string.
- Define `dispatch_channel` per owner: in-system queue / email / webhook / external workflow id.
- Tie the owner principal into the audit chain on activation.

### T3. Introduce `OnboardingDataRequest` as a first-class entity  *(addresses G2.1, G2.2, G2.3, G2.4)*

Schema sketch:

```sql
CREATE TABLE "ob-poc".onboarding_data_requests (
    request_id            UUID PRIMARY KEY,
    onboarding_request_id UUID NOT NULL REFERENCES deal_onboarding_requests(request_id),
    cbu_id                UUID NOT NULL,
    deal_id               UUID NOT NULL,
    product_id            UUID NOT NULL,
    srdef_snapshot_ids    JSONB NOT NULL,          -- frozen at creation (T1)
    status                TEXT NOT NULL DEFAULT 'collecting',
    created_at, updated_at
);

CREATE TABLE "ob-poc".onboarding_data_request_slices (
    request_id            UUID NOT NULL REFERENCES onboarding_data_requests,
    srdef_id              TEXT NOT NULL,           -- one slice per resource type
    slice_status          TEXT NOT NULL,           -- pending | populating | ready | dispatched | activated
    pct_complete          NUMERIC,
    blocking_reasons      JSONB,
    PRIMARY KEY (request_id, srdef_id)
);

CREATE TABLE "ob-poc".onboarding_data_request_attrs (
    request_id            UUID NOT NULL,
    srdef_id              TEXT NOT NULL,
    attr_id               UUID NOT NULL,
    requirement_strength  TEXT,
    merged_constraints    JSONB,
    -- value is read-through to cbu_attr_values / resource_instance_attributes
    PRIMARY KEY (request_id, srdef_id, attr_id),
    FOREIGN KEY (request_id, srdef_id) REFERENCES onboarding_data_request_slices
);
```

- Verb: `onboarding.compile-data-request` — given an `onboarding_request_id`, materialize the slices + attribute set from the (frozen) SRDEF snapshots.
- View: `v_onboarding_data_request` — single source for "% complete per slice / per request" (replaces the per-CBU `v_cbu_attr_summary` for instance-level reporting; the per-CBU view stays as evergreen).
- Connect to existing `cbu_unified_attr_requirements` by recomputing it as a *projection* of all open onboarding data requests for the CBU (instead of being primary).

### T4. Slice-completeness emitter  *(addresses G2.5, G3.4)*

- A trigger or sequencer stage that, on every `cbu_attr_values` / `resource_instance_attributes` write, recomputes the affected slice's `slice_status` and emits `slice.ready` events when it transitions to ready.
- Subscribed by T5.

### T5. Outbox-based "activation instruction" dispatch  *(addresses G3.1, G3.6, G3.7)*

- Define a new outbox effect_kind: `ResourceOwnerActivationInstruction`.
- Implement `ResourceOwnerNotifyConsumer` against the existing `OutboxDrainerImpl` substrate.
- Define the dispatch payload schema: `{ request_id, srdef_id, cbu_id, product, attribute_values, evidence_refs, owner_principal_fqn, correlation_id }`.
- Add verb `service-resource.dispatch-activation` (called by T4 emitter; idempotent on `correlation_id`).
- Add state `AWAITING_OWNER_REGISTRATION` to `cbu_resource_instances` between PROVISIONING and ACTIVE; add SLA timer / staleness signal.

### T6. URL-return inbound verb and reconciliation  *(addresses G3.2, G3.3, GX.4)*

- Verb `service-resource.confirm-activation` (alt: `register-external-reference`) — accepts `correlation_id` (or `(cbu_id, srdef_id, dimensional grain)`) and `external_reference` payload (URL set / account numbers / provider identifiers — see GX.4: settle on a structured shape, not free-text URL).
- Idempotency envelope (mirroring cross_workspace external-call envelope from migration `20260402_*`).
- On success: fills `cbu_resource_instances.instance_url` (or new structured column), records who returned it, transitions AWAITING_OWNER_REGISTRATION → ACTIVE.
- Updates the slice → `activated`; when all slices on a request are activated, transitions `onboarding_data_requests.status` → `completed`.

### T7. Reconcile SRDEF resource instance with Layer-4 capability binding  *(addresses G3.5)*

- Decide: does the activation dispatch live at L4 (BNY application instance) or at the SRDEF resource instance? Two reasonable answers:
  - **L4 anchored:** the dispatch is "tell the BNY app team to register this CBU on their app instance and return the account number on it." SRDEF instance state follows from L4 capability_binding state.
  - **SRDEF anchored:** the dispatch is per resource type abstractly, and L4 is the concrete provider record. A second binding step links the returned URL to a specific app instance.
- Likely the right answer is L4-anchored for `provisioning_strategy = request` resources and SRDEF-anchored for `create` resources. Pick a rule, document it, enforce it via cross-workspace constraint.

### T8. Surface verbs and projections  *(addresses GX.3)*

- Verb `onboarding.show-data-request` returns the consolidated view (all slices + per-attribute population state).
- NarrationEngine: when an onboarding request is in scope, surface "X of Y attributes populated; custody slice ready, settlement waiting on counterparty SSI."
- Inspector projection schema for `OnboardingDataRequest`.
- Observatory: render the slices as children of the OnboardingRequest workspace node, with completion state.

### T9. Set-level versioning & integrity  *(GX.1 follow-on to T1)*

- When SRDEF snapshot changes, surface a "SRDEF changed for an in-flight onboarding request" event; require explicit operator action to rebase or hold (do not silently retarget).

---

## 5. Decisions Required From Adam (peer review)

The questions where the codebase doesn't tell me which way to go:

1. **Q1 (T1).** Is SRDEF being a SemOS-governed object the right move, or should it stay YAML-on-disk and just gain a snapshot pin? The SemOS-first hub invariant (memory: `project_semos_hub_invariant.md`) suggests SemOS-first. Confirm.
2. **Q2 (T2).** What does "resource owner" map to in your operational model? A team / role / external system / OnboardingRequest workspace participant? Is there an existing principal type to reuse?
3. **Q3 (T3).** Should `onboarding_data_requests` be 1:1 with `deal_onboarding_requests`, or 1:N (one request per (deal, cbu, product), or per (deal, cbu) with per-product slices)? The current `deal_onboarding_requests` grain is (deal × cbu × product), which suggests 1:1.
4. **Q4 (T5/T6).** Dispatch channel(s): for the first cut, is in-system outbox + a manual "owner-side operator confirms via verb" enough? Or do we need real webhook/email integration in scope?
5. **Q5 (T6).** What is "the resource URL"? `usually an application account number set` — is it always a single string, or a structured bundle (e.g., `{ account_number, provider, environment, secondary_id }`)? GX.4 hangs on this.
6. **Q6 (T7).** L4-anchored vs SRDEF-anchored dispatch — your call.
7. **Q7 (general).** Is this capability blocked behind v1.3 catalogue platform completion, or does it sit on top of v1.3 as v1.5 / a new spec stream?
8. **Q8 (general).** STP scope: "STP enabler" — is the owner ultimately a system (so T5 must end up being a webhook/API integration eventually), or is the immediate target operator-in-the-loop (manual confirm verb)? Fixes the priority of T5 vs T6.

---

## 6. Suggested Next Step

If T1–T7 land in priority order, the loop closes:

```
SemOS-governed SRDEF (T1)
   → owner principal known (T2)
   → onboarding_data_request compiled at request creation (T3)
   → slice-readiness emitter (T4)
   → activation instruction dispatched via outbox to owner (T5)
   → owner returns URL via inbound verb with idempotency (T6)
   → cross-layer reconciliation between SRDEF and L4 (T7)
   → projections surfaced to UI/narration (T8)
```

T1 + T3 are the architectural cornerstones; T5 + T6 are the new verb pair that materializes the loop; the rest are dependent or cosmetic.

Recommend opening this as `service-resource-data-dictionary-v0_2.md` after Adam's review, with answers to Q1–Q8 baked in, then breaking into tranches.

---

## Appendix A — Files of interest (for reviewers)

| Topic | Path |
|------|------|
| AttributeDef body | `rust/crates/sem_os_core/src/attribute_def.rs:18` |
| DerivationSpec body | `rust/crates/sem_os_core/src/derivation_spec.rs:12` |
| Attribute materialization trigger | `rust/migrations/20260328_semos_attribute_materialization.sql:8` |
| Visibility two-tier | `rust/migrations/20260331_attribute_visibility.sql` |
| Derived persistence | `rust/migrations/20260327_derived_attribute_persistence.sql` |
| SRDEF YAML loader | `rust/src/service_resources/srdef_loader.rs:56` |
| SRDEF example | `rust/config/srdefs/custody.yaml` |
| Service-resource verbs | `rust/config/verbs/service-resource.yaml` |
| `service-resource.check-attribute-gaps` impl | `rust/src/services/service_pipeline_service_impl.rs:451` |
| `service-resource.sync-definitions` impl | `rust/src/services/service_pipeline_service_impl.rs:487` |
| Service resource types schema | `rust/migrations/master-schema.sql` (table `service_resource_types`) |
| Resource attribute requirements | `rust/migrations/024_service_intents_srdef.sql` |
| CBU unified requirements | `rust/migrations/025_cbu_unified_attributes.sql:18` |
| CBU attr gaps view | `rust/migrations/025_cbu_unified_attributes.sql:142` |
| Service readiness | `rust/migrations/027_service_readiness.sql` |
| Service consumption | `rust/migrations/20260429_carrier_01_cbu_service_consumption.sql` |
| Lifecycle resources (L4) | `rust/migrations/20260427_lifecycle_resources_workspace.sql` |
| Service lifecycle | `rust/migrations/20260428_service_lifecycle.sql` |
| Onboarding request DAG | `rust/config/sem_os_seeds/dag_taxonomies/onboarding_request_dag.yaml` |
| CBU DAG (cross-workspace constraints) | `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml` |
| Outbox drainer | `rust/src/cross_workspace/outbox/drainer.rs` (per memory `project_three_plane_phase5e`) |
| Service resource def lifecycle | `rust/config/sem_os_seeds/state_machines/service_resource_def_lifecycle.yaml` |

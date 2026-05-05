# Service-Resource Data Dictionary Capability - Research and Gap Analysis (v0.2)

> Status: Peer-review revised draft
> Author: Codex peer review over v0.1
> Date: 2026-05-05
> Supersedes: `docs/backlog/service-resource-data-dictionary-capability-v0_1.md`
> Purpose: Re-state the service-resource data dictionary capability with repo-verified amendments. The key correction from v0.1 is that the provisioning ledger and request/event substrate already exist; the missing capability is the closed loop at onboarding-instance grain.

## 1. Executive Verdict

The v0.1 headline is directionally correct:

> The project has most of the substrate for service-resource data dictionaries, but not the first-class onboarding-instance loop.

The main architectural gap remains:

> There is no first-class `OnboardingDataRequest` entity that freezes, groups, populates, and tracks the consolidated SRDEF dictionary set for a specific `deal_onboarding_requests.request_id`.

However, v0.1 under-recognised the provisioning side. The repo already has:

- `provisioning_requests` and `provisioning_events`
- owner-system fields and request payload snapshots
- outbound `REQUEST_SENT` and inbound `RESULT` event semantics
- an idempotent SQL result handler, `process_provisioning_result`
- resource instance fields for `resource_url`, `owner_ticket_id`, `last_request_id`, and `last_event_at`
- a provisioning orchestrator that creates instances, creates provisioning requests, records `REQUEST_SENT`, and moves resource instances to `PROVISIONING`

So the C3 gap is not "no dispatch/return contract at all." It is:

> The dispatch/return contract is partially modelled as a provisioning ledger, but it is not yet wired to public outbox delivery, not exposed as a first-class inbound confirmation verb/API, not correlated to an onboarding data request, and not backed by a proper resource-owner principal.

## 2. Revised Capability Scores

| Clause | Vision | v0.1 score | v0.2 score | Reason for amendment |
|---|---|---:|---:|---|
| C1 | Resource owners author dictionary slices | about 80% | about 65-70% | YAML SRDEFs exist, but SemOS SRDEF is declared in DAG/config without a real core object/table/body. Conditional/evidence semantics are weaker than v0.1 implied. |
| C2 | Per-CBU onboarding consolidates dictionaries into a populatable instance | about 50% | about 45-50% | CBU-level rollup exists, but not onboarding-instance grain. Slice grain must include SRDEF parameters, not just `srdef_id`. |
| C3 | Populated slice -> activation instruction -> owner returns URL/account set | about 25% | about 45-55% | Provisioning ledger, events, owner-system fields, and partial result processing already exist. Missing pieces are outbox delivery, inbound verb/API, principal, correlation, and lifecycle semantics. |

## 3. Current Substrate, With Caveats

### 3.1 Strong Existing Substrate

| Layer | Capability | Status | Evidence |
|---|---|---|---|
| SemOS dictionary | `AttributeDefBody`, registry snapshots, materialization to `attribute_registry` | Strong | `rust/crates/sem_os_core/src/attribute_def.rs`, `rust/migrations/20260328_semos_attribute_materialization.sql` |
| SRDEF declaration | YAML SRDEFs with attributes, source policy, constraints, dependencies, dimensional flags | Strong but file-authored | `rust/config/srdefs/*.yaml`, `rust/src/service_resources/srdef_loader.rs` |
| SRDEF projection | `service_resource_types`, `resource_attribute_requirements`, `srdef_id`, `provisioning_strategy` | Strong | `migrations/024_service_intents_srdef.sql` |
| Discovery | CBU service intents -> discovered SRDEFs with parameters | Useful | `rust/src/service_resources/discovery.rs` |
| CBU rollup | `cbu_unified_attr_requirements` and `cbu_attr_values` | Useful but CBU-only | `migrations/025_cbu_unified_attributes.sql` |
| Population | Derived/entity/CBU/manual value population | Partial | `PopulationEngine` in `rust/src/service_resources/discovery.rs` |
| Provisioning ledger | `provisioning_requests`, `provisioning_events`, `v_provisioning_pending`, `process_provisioning_result` | Strong substrate | `migrations/026_provisioning_ledger.sql` |
| Provisioning orchestrator | Creates instance, request, `REQUEST_SENT`, sets instance `PROVISIONING` | Useful | `rust/src/service_resources/provisioning.rs` |
| Resource instances | `cbu_resource_instances` with dimensional grain and owner-return fields | Useful | `rust/migrations/master-schema.sql`, table `cbu_resource_instances` |
| Public outbox | Generic post-commit `public.outbox` and drainer | Strong substrate | `rust/migrations/20260421_public_outbox.sql`, `rust/src/outbox/drainer.rs` |
| Readiness | Per CBU/product/service readiness with blocking reasons | Useful | `migrations/027_service_readiness.sql`, `ReadinessEngine` |
| Deal onboarding | `deal_onboarding_requests` at deal/contract/CBU/product grain | Strong as handoff pivot | `migrations/067_deal_record_fee_billing.sql` |
| Ops attribution | `cbu_service_consumption.onboarding_request_id` FK to deal onboarding request | Useful | `rust/migrations/20260429_carrier_01_cbu_service_consumption.sql` |

### 3.2 Important Caveats

Several v0.1 "Solid" ratings should be read as "schema exists, semantics partial":

- Conditional requirements are stored as `condition_expression`, but provisioning/readiness only check `is_mandatory = TRUE`; conditional expressions are not evaluated.
- `evidence_policy` exists in the database schema, but the YAML loader does not parse an evidence policy from SRDEF files.
- Constraint merging is explicitly incomplete in `AttributeRollupEngine` (`TODO: Proper constraint merging with conflict detection`).
- `cbu_unified_attr_requirements` is keyed only by `(cbu_id, attr_id)`, so it is an evergreen CBU rollup, not an onboarding-instance dictionary.
- The SemOS maintenance DAG refers to `sem_reg.service_resource_defs`, but no such SemOS object/table/core body is implemented.
- `public.outbox` has `ExternalNotify` in the effect-kind enum, but no registered consumer for resource-owner notification.

## 4. Corrected Current Flow

The flow implemented today is closer to this:

```text
service_intents
  -> ResourceDiscoveryEngine
  -> srdef_discovery_reasons (CBU + SRDEF + parameters)
  -> AttributeRollupEngine
  -> cbu_unified_attr_requirements (CBU-only union)
  -> PopulationEngine
  -> cbu_attr_values / derived projections
  -> ProvisioningOrchestrator
  -> cbu_resource_instances (PENDING -> PROVISIONING)
  -> provisioning_requests
  -> provisioning_events.REQUEST_SENT
  -> ReadinessEngine
  -> cbu_service_readiness
```

What is missing is the onboarding-instance spine:

```text
deal_onboarding_requests.request_id
  -> onboarding_data_requests
  -> onboarding_data_request_slices
  -> onboarding_data_request_attrs
  -> slice.ready
  -> provisioning_requests / public.outbox
  -> resource owner
  -> confirm result / URL/account set
  -> cbu_resource_instances ACTIVE
  -> onboarding_data_request completed
```

## 5. Gap Analysis

### G1. SRDEF Is Not Yet a First-Class Governed SemOS Object

There is a declared intent for `service_resource_def` in SemOS maintenance configuration:

- `registry_stewardship.yaml` has a `service_resource_def` slot mapped to `service_resource_types`.
- `semos_maintenance_dag.yaml` declares `service_resource_def_lifecycle` and references `"sem_reg".service_resource_defs`.

But implementation does not match that declared model:

- `ObjectType` does not include `ServiceResourceDef`.
- There is no `ServiceResourceDefBody` beside `AttributeDefBody`, `DerivationSpecBody`, and `RequirementProfileDefBody`.
- No `sem_reg.service_resource_defs` table was found.
- `RegistryService` has typed publish/resolve methods for attribute, derivation, requirement profile, etc., but not service-resource definitions.
- The actual source of truth is YAML under `rust/config/srdefs/`, synced into operational tables.

This is not a small documentation gap. It means SRDEF currently sits between two worlds:

- operationally present through YAML and `service_resource_types`
- architecturally claimed as SemOS-governed, but not actually implemented as such

### G2. Resource Owner Is a Label, Not an Addressable Principal

SRDEFs carry `owner: CUSTODY`, and the provisioning ledger has `owner_system`.

That is enough for reporting and rough routing, but not enough for a closed loop:

- no `owner_principal_fqn`
- no team/system principal model attached to SRDEF
- no dispatch channel
- no permission/audit identity for returned activation results
- no outbox consumer that resolves `owner_system` to a delivery destination

The repo does have generic `Principal` support and application `owner_team` concepts, so this should reuse existing identity patterns rather than invent a detached owner table without a reason.

### G3. Attribute Requirement Semantics Are Incomplete

SRDEF YAML can express required/optional/conditional attributes and constraints. The database can store `source_policy`, `constraints`, `evidence_policy`, and `condition_expression`.

But the effective logic is narrower:

- `is_mandatory` is true only for `requirement == "required"`.
- Conditional requirements are not evaluated during provisioning readiness.
- `evidence_policy` is not parsed from YAML.
- Conflict detection is not complete.
- Source policy is collapsed to a preferred source, and population fallback is limited.

This matters because an onboarding data request must be auditable. It cannot merely list attribute IDs; it must explain why each value is required, conditional, waived, sourced, stale, or evidence-backed.

### G4. No Onboarding-Instance Dictionary Entity

The closest current table is `cbu_unified_attr_requirements`.

That table is intentionally CBU-level:

```sql
PRIMARY KEY (cbu_id, attr_id)
```

It cannot answer these questions cleanly:

- Which exact `deal_onboarding_requests.request_id` caused this requirement?
- Which product/service scope is this request for?
- Which SRDEF snapshots were frozen when the request started?
- Which parameterized resource slices are in scope?
- Is one slice ready while another is still collecting?
- What changed if a product-service binding or SRDEF changed mid-onboarding?

The existing `cbu_service_consumption.onboarding_request_id` is useful attribution, but it does not create a consolidated data dictionary instance.

### G5. Slice Grain Must Include Parameters

v0.1 sketched slices as:

```sql
PRIMARY KEY (request_id, srdef_id)
```

That is too coarse.

SRDEFs can be instantiated per market, currency, and counterparty. The discovery engine already treats the instance key as:

```text
srdef_id + parameters
```

Therefore an onboarding data request slice should be keyed by one of:

- `discovery_id`, if the discovery row is frozen and referenced
- `(request_id, srdef_id, parameters_hash)`
- `(request_id, srdef_snapshot_id, parameters_hash)`

Without this, a CBU onboarding that needs two custody accounts for two markets will collapse separate activation slices into one row.

### G6. Provisioning Ledger Exists, but It Is Not the Closed Loop Yet

The provisioning ledger is materially better than v0.1 implied:

- `provisioning_requests.request_payload` is already a request snapshot.
- `provisioning_events` supports `OUT` and `IN`.
- `REQUEST_SENT`, `ACK`, `RESULT`, `ERROR`, `STATUS`, and `RETRY` are already modelled.
- `process_provisioning_result` updates request status and marks instances `ACTIVE` on success.
- `cbu_resource_instances` has `resource_url`, `owner_ticket_id`, and `last_request_id`.

Remaining gaps:

- `provisioning_requests` has no `onboarding_request_id` or `onboarding_data_request_id`.
- `provisioning_events` are not backed by `public.outbox` delivery for the resource-owner leg.
- `REQUEST_SENT` is recorded internally, but not necessarily delivered externally.
- There is no first-class inbound verb/API that wraps result confirmation in the runtime/audit model.
- `process_provisioning_result` is a SQL function, not integrated as a service-resource verb or typed API route.
- There is no owner principal or dispatch-channel resolution.
- There is no explicit "awaiting owner registration" state distinct from generic `PROVISIONING`.

### G7. Direct `service-resource.provision` and Pipeline `provisioning.run` Need Separation

There are two surfaces:

- `service-resource.provision`: direct resource instance creation.
- `provisioning.run`: CBU pipeline orchestrator that creates provisioning requests/events.

The service-resource data dictionary closed loop should anchor primarily on the pipeline provisioning ledger, not the direct create verb.

The direct verb can remain useful for manual or internal resource creation, but it should not be treated as the main owner-dispatch contract unless it is extended to participate in the ledger.

### G8. Layer 4 Reconciliation Remains Unresolved

Layer 4 has:

- `applications`
- `application_instances`
- `capability_bindings`
- LIVE/ACTIVE gates for downstream service consumption

SRDEF resource instances have:

- `cbu_resource_instances`
- SRDEF identity
- `resource_url` / account number fields
- independent status

The repo can currently represent:

- application capability is LIVE but SRDEF resource instance is not ACTIVE
- SRDEF resource instance is ACTIVE but no matching L4 capability binding is LIVE

The dispatch rule must decide whether activation is anchored:

- at SRDEF slice/resource instance level
- at L4 application instance/capability binding level
- hybrid by `provisioning_strategy`

The likely rule is:

- `provisioning_strategy = request`: dispatch to owner of the application/capability where possible, with SRDEF slice as the data packet.
- `provisioning_strategy = create`: system creates resource instance directly, then records the same ledger outcome.
- `provisioning_strategy = discover`: bind to existing external reference, then validate the result.

### G9. Concrete Schema Defects Found During Peer Review

These are not conceptual gaps; they are implementation defects that should be tracked:

1. `deal_onboarding_requests.request_status` default is `REQUESTED`, but the current CHECK constraint allows only `PENDING`, `IN_PROGRESS`, `BLOCKED`, `COMPLETED`, `CANCELLED`.
2. `ReadinessEngine` checks for resource instance status `FAILED`, but `cbu_resource_instances.status` CHECK does not allow `FAILED`.
3. `provisioning_requests` is described as append-only in comments, but status updates are allowed by design after the immutable trigger is dropped. The wording should be corrected to "request row with mutable status plus append-only events."
4. `service-resource.read` lookup metadata references `resource_type_id` as primary key, while `service_resource_types` uses `resource_id` in the SRDEF loader/schema. This should be verified because stale lookup metadata harms tool reliability.

## 6. Revised Remediation Targets

### T0. Fix Immediate Schema/Metadata Defects

Before adding new architecture, fix the contradictions that will otherwise confuse tests and operators:

- Align `deal_onboarding_requests.request_status` default with its CHECK constraint, preferably default `PENDING`.
- Decide whether resource instances can fail. Either add `FAILED` to `cbu_resource_instances.status`, or remove `FAILED` handling from readiness and represent failure only through `provisioning_requests.status = failed`.
- Update provisioning comments to reflect mutable request status plus append-only events.
- Verify and repair stale `service_resource_types` lookup metadata.

### T1. Reconcile `ServiceResourceDef` as a SemOS-Governed Object

This is not just "lift YAML into SemOS." It is reconciling declared design with actual implementation.

Implement:

- `ObjectType::ServiceResourceDef`
- `ServiceResourceDefBody` in `sem_os_core`
- typed publish/resolve methods in `RegistryService`
- a materialization path from active SemOS SRDEF snapshots into `service_resource_types` and `resource_attribute_requirements`
- a compatibility bootstrap path from YAML into SemOS snapshots
- a version/snapshot ID that can be pinned by onboarding data requests

Keep YAML as seed/bootstrap if useful, but avoid two active sources of truth.

### T2. Complete Attribute Requirement Semantics

Before the onboarding data request becomes contractual, complete the semantics it depends on:

- evaluate `condition_expression`
- parse and persist `evidence_policy` from SRDEF YAML/SemOS body
- implement constraint merge/conflict detection
- carry source policy and evidence requirements into request slices
- represent waived/not-applicable/conditional-not-triggered states explicitly

### T3. Introduce Resource Owner Principal and Dispatch Channel

Add owner identity beyond `owner: CUSTODY`.

Minimum viable shape:

```text
owner_principal_fqn
owner_system
dispatch_channel
dispatch_endpoint_or_queue
owner_team
```

This may be a SemOS principal, application owner team, or an external system identity. The design should avoid duplicating existing `Principal` and `applications.owner_team` concepts.

### T4. Introduce `OnboardingDataRequest`

Create a first-class entity at onboarding handoff grain.

Recommended grain:

```sql
CREATE TABLE "ob-poc".onboarding_data_requests (
    data_request_id        uuid PRIMARY KEY DEFAULT uuidv7(),
    onboarding_request_id  uuid NOT NULL REFERENCES "ob-poc".deal_onboarding_requests(request_id),
    deal_id                uuid NOT NULL,
    contract_id            uuid NOT NULL,
    cbu_id                 uuid NOT NULL,
    product_id             uuid NOT NULL,
    status                 text NOT NULL DEFAULT 'collecting',
    compiled_at            timestamptz NOT NULL DEFAULT now(),
    completed_at           timestamptz,
    created_at             timestamptz NOT NULL DEFAULT now(),
    updated_at             timestamptz NOT NULL DEFAULT now(),
    UNIQUE (onboarding_request_id)
);
```

Status sketch:

```text
collecting -> ready_for_dispatch -> dispatching -> awaiting_owner -> completed
collecting -> blocked
awaiting_owner -> blocked
any non-terminal -> cancelled
```

### T5. Add Parameterized Data Request Slices

The slice must preserve SRDEF grouping and dimensional parameters.

Recommended shape:

```sql
CREATE TABLE "ob-poc".onboarding_data_request_slices (
    slice_id              uuid PRIMARY KEY DEFAULT uuidv7(),
    data_request_id       uuid NOT NULL REFERENCES "ob-poc".onboarding_data_requests(data_request_id),
    discovery_id          uuid REFERENCES "ob-poc".srdef_discovery_reasons(discovery_id),
    srdef_id              text NOT NULL,
    srdef_snapshot_id     uuid,
    resource_type_id      uuid,
    parameters            jsonb NOT NULL DEFAULT '{}',
    parameters_hash       text NOT NULL,
    provisioning_strategy text NOT NULL,
    owner_system          text NOT NULL,
    owner_principal_fqn   text,
    slice_status          text NOT NULL DEFAULT 'collecting',
    pct_complete          numeric,
    blocking_reasons      jsonb NOT NULL DEFAULT '[]',
    created_at            timestamptz NOT NULL DEFAULT now(),
    updated_at            timestamptz NOT NULL DEFAULT now(),
    UNIQUE (data_request_id, srdef_id, parameters_hash)
);
```

Slice status sketch:

```text
collecting -> ready -> dispatched -> awaiting_owner -> activated
collecting -> blocked
awaiting_owner -> failed
any non-terminal -> cancelled
```

### T6. Add Data Request Attribute Rows

Do not make the CBU rollup primary. The onboarding data request should own its compiled requirement rows, while CBU rollup can remain an evergreen projection.

Recommended shape:

```sql
CREATE TABLE "ob-poc".onboarding_data_request_attrs (
    slice_id              uuid NOT NULL REFERENCES "ob-poc".onboarding_data_request_slices(slice_id),
    attr_id               uuid NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    requirement_strength  text NOT NULL,
    condition_expression  text,
    condition_status      text NOT NULL DEFAULT 'unconditional',
    merged_constraints    jsonb NOT NULL DEFAULT '{}',
    source_policy         jsonb NOT NULL DEFAULT '[]',
    evidence_policy       jsonb NOT NULL DEFAULT '{}',
    value_status          text NOT NULL DEFAULT 'missing',
    value_ref             jsonb,
    evidence_refs         jsonb NOT NULL DEFAULT '[]',
    blocking_reason       jsonb,
    created_at            timestamptz NOT NULL DEFAULT now(),
    updated_at            timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (slice_id, attr_id)
);
```

Possible `condition_status`:

```text
unconditional | triggered | not_applicable | unresolved
```

Possible `value_status`:

```text
missing | populated | derived | confirmed | waived | stale | invalid
```

### T7. Compile Onboarding Data Request From Frozen Inputs

Add a verb/API:

```text
onboarding.compile-data-request
```

Input:

```text
onboarding_request_id
```

Responsibilities:

- read `deal_onboarding_requests`
- get its CBU/product/service scope
- use active service intents/discovery or create a request-scoped discovery snapshot
- pin SRDEF snapshot IDs
- materialize slices at parameterized grain
- materialize per-slice attributes
- compute initial completeness/blockers

Open design choice:

- either compile from existing `srdef_discovery_reasons`
- or create `onboarding_data_request_discoveries` as a frozen request-scoped copy

The second is more robust if live CBU discovery can change while onboarding is open.

### T8. Emit Slice Readiness and Dispatch Provisioning Requests

When a slice transitions to `ready`, create or reuse a provisioning request.

Recommended approach:

- Extend `provisioning_requests` with `onboarding_request_id`, `data_request_id`, and `slice_id`.
- Insert `provisioning_events.REQUEST_SENT` as the ledger event.
- Insert `public.outbox` row with an effect kind for owner dispatch, probably reusing or specializing `ExternalNotify`.
- Make dispatch idempotent by `(slice_id, effect_kind)` or the existing request payload idempotency key.

The dispatch payload should include:

```json
{
  "data_request_id": "...",
  "slice_id": "...",
  "provisioning_request_id": "...",
  "onboarding_request_id": "...",
  "cbu_id": "...",
  "product_id": "...",
  "srdef_id": "SRDEF::...",
  "parameters": {},
  "attribute_values": {},
  "evidence_refs": [],
  "owner_system": "CUSTODY",
  "owner_principal_fqn": "...",
  "correlation_id": "..."
}
```

### T9. Add Inbound Result Confirmation Verb/API

Do not leave result handling as only a SQL function.

Add:

```text
service-resource.confirm-provisioning-result
```

or:

```text
provisioning.confirm-result
```

Input:

```text
provisioning_request_id
content_hash or idempotency_key
result payload
acting owner principal/system
```

Responsibilities:

- dedupe using `content_hash` or idempotency key
- append `provisioning_events.RESULT` or `ERROR`
- update `provisioning_requests.status`
- update `cbu_resource_instances.resource_url`, `owner_ticket_id`, `instance_identifier`
- transition instance to `ACTIVE` on success
- transition slice to `activated`
- recompute onboarding data request status

The existing `process_provisioning_result` function can be reused or replaced behind this verb.

### T10. Reconcile SRDEF Instance With L4 Capability Binding

Choose and document the dispatch anchor:

| Strategy | Anchor | Rule |
|---|---|---|
| `request` | L4 where possible | Owner is the application/capability team; SRDEF slice is the request packet. |
| `create` | SRDEF/system | System creates the resource and records ledger outcome. |
| `discover` | SRDEF/system with validation | System binds to existing URL/account/reference and validates readiness. |

Add a cross-layer reference once chosen:

- from `cbu_resource_instances` to `capability_bindings`, or
- from `provisioning_requests` to `capability_bindings`, or
- both, if request is app-bound and resulting resource is CBU-bound.

## 7. Revised Decision List for Adam

1. Should SRDEF become a true SemOS registry object now, or should v1 preserve YAML source-of-truth with snapshot pins as an interim step?
2. What is the authoritative meaning of resource owner: SemOS principal, application owner team, external owner system, or a combination?
3. Should `onboarding_data_requests` be strictly 1:1 with `deal_onboarding_requests`? Current deal onboarding grain suggests yes.
4. Should discovery be frozen by copying `srdef_discovery_reasons`, or by referencing active discovery rows plus SRDEF snapshot IDs?
5. Should the first owner dispatch use in-system outbox/manual operator flow, or immediately target webhook/email integration?
6. Is "resource URL" a single string or a structured bundle? The existing model has `instance_url`, `resource_url`, `instance_identifier`, and owner result payload fields; this should be normalized before UI work.
7. Should failed resource provisioning be represented on `cbu_resource_instances.status`, or only in `provisioning_requests.status` and readiness blockers?
8. Is L4 capability binding mandatory for all request-provisioned SRDEFs, or only for application-backed services?

## 8. v0.2 Recommendation

The immediate spec should be smaller than v0.1's T1-T9 sweep.

Recommended first tranche:

```text
T0  Fix schema/status contradictions
T4  Add onboarding_data_requests
T5  Add parameterized slices
T6  Add request attrs
T7  Add compile-data-request
T8  Link slices to provisioning_requests and public.outbox
T9  Add inbound confirm-result verb/API
```

Do not block this tranche entirely on full SemOS-governed SRDEF if that would delay the closed loop. A pragmatic route is:

1. Pin current YAML-synced SRDEF identity/version in the onboarding data request.
2. Close the onboarding-instance loop.
3. Then migrate SRDEF authoring into SemOS as a second tranche.

That said, the codebase already claims `service_resource_def` in SemOS maintenance configuration. The project should either implement that claim or explicitly mark it as aspirational to avoid future drift.

## 9. Acceptance Criteria for the Capability

This capability should not be considered closed until the following are true:

- Given a `deal_onboarding_requests.request_id`, the system can compile exactly one `onboarding_data_request`.
- The compiled request contains parameterized slices for every required SRDEF instance.
- Each slice lists attributes, conditions, source policy, evidence policy, value status, and blockers.
- Completeness is computable per attribute, per slice, and per onboarding data request.
- A ready slice creates or links to a `provisioning_request`.
- The owner dispatch is represented in `provisioning_events` and `public.outbox`.
- Owner result confirmation is available through a typed verb/API, not only a SQL function.
- Successful confirmation writes the returned external reference/account set and activates the resource instance.
- Readiness and onboarding data request state update after the returned result.
- The flow is idempotent across repeated dispatches and repeated owner callbacks.
- Concurrent onboarding requests for the same CBU do not silently overwrite each other's dictionary scope.

## Appendix A. File References

| Topic | Path |
|---|---|
| AttributeDef body | `rust/crates/sem_os_core/src/attribute_def.rs` |
| ObjectType enum | `rust/crates/sem_os_core/src/types.rs` |
| Registry typed methods | `rust/src/sem_reg/registry.rs` |
| SRDEF YAML examples | `rust/config/srdefs/*.yaml` |
| SRDEF loader and sync | `rust/src/service_resources/srdef_loader.rs` |
| Service-resource direct verbs | `rust/config/verbs/service-resource.yaml` |
| Service pipeline verbs | `rust/config/verbs/service-pipeline.yaml` |
| Service pipeline implementation | `rust/src/services/service_pipeline_service_impl.rs` |
| Discovery and rollup | `rust/src/service_resources/discovery.rs` |
| Provisioning orchestrator/readiness | `rust/src/service_resources/provisioning.rs` |
| Service resource types and SRDEF schema | `migrations/024_service_intents_srdef.sql` |
| CBU unified attributes | `migrations/025_cbu_unified_attributes.sql` |
| Provisioning ledger | `migrations/026_provisioning_ledger.sql` |
| Service readiness | `migrations/027_service_readiness.sql` |
| Deal onboarding requests | `migrations/067_deal_record_fee_billing.sql` |
| Deal onboarding status constraint | `migrations/068_deal_billing_constraints.sql` |
| Public outbox | `rust/migrations/20260421_public_outbox.sql` |
| Outbox drainer | `rust/src/outbox/drainer.rs` |
| L4 application/capability tables | `rust/migrations/20260427_lifecycle_resources_workspace.sql` |
| Service consumption onboarding FK | `rust/migrations/20260429_carrier_01_cbu_service_consumption.sql` |
| SemOS maintenance SRDEF declaration | `rust/config/sem_os_seeds/dag_taxonomies/semos_maintenance_dag.yaml` |
| Registry stewardship SRDEF slot | `rust/config/sem_os_seeds/constellation_maps/registry_stewardship.yaml` |

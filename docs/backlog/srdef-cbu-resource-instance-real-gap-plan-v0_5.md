# SRDEF and CBU Resource Instance Lifecycle - Real-Gap Plan (v0.5)

> Status: Updated research and implementation plan
> Date: 2026-05-05
> Baseline commit: `d66d7501 Implement service-resource data dictionary v0.4`
> Scope: SRDEF + onboarding data request + provisioning request + `cbu_resource_instance` lifecycle visibility for Sage, REPL, and MCP.
> Mode: Planning only. This document does not implement code changes.

## 1. Executive Verdict

The previous audit brief is partly stale. The v0.4 implementation now has the operational closed loop:

```text
deal_onboarding_request
  -> onboarding.compile-data-request
  -> onboarding_data_request / discoveries / slices / attrs
  -> onboarding.dispatch-ready-slices
  -> provisioning_request + REQUEST_PREPARED event
  -> public.outbox ResourceOwnerDispatch
  -> outbox consumer records DISPATCHED
  -> slice awaiting_owner
  -> service-resource.confirm-provisioning-result
  -> cbu_resource_instance ACTIVE | FAILED | CANCELLED
```

The real remaining work is not "build the loop." The loop exists. The real work is to make the loop structurally closed and agent-visible:

- keep SRDEF snapshot lineage correct at slice grain;
- register the provisioning and resource-instance states as first-class SemOS-visible lifecycle nodes;
- add the missing read/lookup surfaces that agents need to navigate without raw SQL;
- separate SRDEF governance lifecycle from SRDEF attribute-coverage lifecycle;
- make cascade declarations auditable instead of implicit in implementation code.

## 2. Stale Findings Removed

These should not drive implementation.

| Stale claim | Current reality | Disposition |
|---|---|---|
| `ServiceResourceDef` object type/body is absent | `ObjectType::ServiceResourceDef` and `ServiceResourceDefBody` exist; sync publishes active `sem_reg.snapshots` rows | Not a gap. There is only a typed registry convenience gap. |
| The resource-owner dispatch/return loop is absent | `dispatch-ready-slices`, `ResourceOwnerDispatch`, `ResourceOwnerStandDown`, `DISPATCHED`, and `confirm-provisioning-result` exist | Not a gap. Continue hardening and visibility only. |
| Tranche 2 is out of scope | Tranche 2 was implemented because Sage/REPL visibility depends on it | Treat the brief's tranche boundary as superseded. |
| `onboarding_handoff.yaml` must exist by that filename | Current repo pattern uses `onboarding_workspace.yaml` with constellation `onboarding.workspace` | Spec naming correction, not a code gap. |
| Direct MCP tools like `tools.onboarding.compile_data_request` must be the only acceptable exposure | Current MCP model exposes generic DSL tools (`dsl_execute`, `verbs_list`, `dsl_signature`, `dsl_complete`, `verb_search`) plus SemReg tools | Do not implement direct per-verb MCP tools unless product direction changes. Validate generic exposure instead. |
| `provisioning_request` lifecycle is only `pending/completed/failed` | Live schema uses `queued/sent/ack/completed/failed/cancelled` | Use actual schema states. |
| `state.application` must be stateful | Lifecycle resources DAG treats application as a stateless registry card; lifecycle is at `application_instance` | No change unless product direction explicitly changes. |

## 3. Current Implementation Snapshot

Implemented and verified in the baseline commit:

- Tranche migrations:
  - `rust/migrations/20260505_service_resource_data_dictionary.sql`
  - `rust/migrations/20260505_service_resource_data_dictionary_tranche2.sql`
  - `rust/migrations/20260505_service_resource_data_dictionary_tranche3.sql`
- SRDEF SemOS snapshot body:
  - `rust/crates/sem_os_core/src/service_resource_def.rs`
  - `ObjectType::ServiceResourceDef`
- SRDEF sync:
  - publishes active `service_resource_def` snapshots;
  - pins `service_resource_types.srdef_snapshot_id`;
  - syncs owner principals;
  - tracks `attribute_gap_count` and `attribute_conflict_count`;
  - sets `lifecycle_status` to `complete` or `gaps_found`.
- Onboarding data request service:
  - compiles frozen data requests;
  - creates frozen discovery/slice/attr rows;
  - evaluates simple conditions, defaults, constraints, and evidence requirements;
  - enforces L4 live binding where configured;
  - dispatches ready slices through `public.outbox`;
  - accepts owner-returned provisioning results idempotently;
  - cascades cancellation to slices, provisioning requests, stand-down outbox, and non-active resource instances.
- Agent-facing DSL registrations:
  - `onboarding.compile-data-request`
  - `onboarding.dispatch-ready-slices`
  - `onboarding.cancel-data-request`
  - `onboarding.cancel-slice`
  - `onboarding.get-data-request`
  - `onboarding.list-data-requests`
  - `onboarding.list-slices`
  - `onboarding.get-slice`
  - `onboarding.get-slice-attrs`
  - `service-resource.confirm-provisioning-result`

Live DB state after sync showed:

```text
active service_resource_def snapshots: 9
service_resource_types with srdef_snapshot_id: 9
resource_owner_principals: 16
SRDEF lifecycle_status: 9 gaps_found, 36 unsynced
```

`gaps_found` is a data stewardship state, not an implementation defect. It means current SRDEF YAML references attributes that are not yet fully present/governed.

## 4. Real Implementation Gaps

### RG1. Slice SRDEF Snapshot Pinning Is Incorrect

Current slice creation writes `onboarding_data_request_slices.srdef_snapshot_id` from the onboarding discovery snapshot id. That pins the slice to `onboarding_data_request_discoveries.discovery_snapshot_id`, not the active `sem_reg.snapshots.snapshot_id` for the SRDEF.

Why this matters:

- `discovery_snapshot_id` answers "why was this SRDEF discovered for this CBU?"
- `srdef_snapshot_id` must answer "which exact SRDEF template version was frozen for this owner request?"
- Agents and auditors cannot reconstruct the frozen dictionary lineage if these are conflated.

Minimum remediation:

- Add `srt.srdef_snapshot_id` to the discovery load query.
- Carry it on `DiscoveryRow`.
- Insert that value into `onboarding_data_request_slices.srdef_snapshot_id`.
- Keep `discovery_snapshot_id` unchanged.
- Add a regression test that compiled slices point to an active `sem_reg.snapshots` row where `object_type = 'service_resource_def'`.

Priority: P0 blocker.

### RG2. `cbu_resource_instance` Is Operationally Mutated but Not SemOS-Visible as This Lifecycle

The table and statuses exist:

```text
PENDING, PROVISIONING, AWAITING_OWNER, ACTIVE, FAILED, CANCELLED, SUSPENDED, DECOMMISSIONED
```

The onboarding loop mutates this state, but there is no dedicated state-machine YAML or onboarding/CBU constellation slot for `state.cbu_resource_instance` in the service-resource data-request lifecycle.

Why this matters:

- Sage can see slice state but cannot cleanly traverse to the delivered resource instance lifecycle.
- The cascade auditor cannot validate transitions into `ACTIVE`, `FAILED`, or `CANCELLED` as declared state effects.
- Closure walking from onboarding cannot prove the loop ends at an active resource instance.

Minimum remediation:

- Add `cbu_resource_instance_lifecycle.yaml` using the actual schema states.
- Add `cbu_resource_instance` slots to `onboarding_workspace.yaml` and `cbu_workspace.yaml`.
- Add cross-visible read verbs in those maps.
- Add DAG node entries in the onboarding or product-service DAG that connect:
  - `onboarding_data_request_slice -> cbu_resource_instance`
  - `onboarding.dispatch-ready-slices` creates or reuses instance
  - `service-resource.confirm-provisioning-result` activates/fails instance
  - cancellation cancels non-active instance

Priority: P1.

### RG3. `provisioning_request` Is Not a First-Class Lifecycle Node

The DB schema and code use `provisioning_requests.status` values:

```text
queued, sent, ack, completed, failed, cancelled
```

The current closed loop relies on this ledger, but SemOS visibility is incomplete.

Why this matters:

- Agents need a stable way to answer "what is waiting on the owner?" without raw SQL.
- Cascade audit must see request status changes from `dispatch-ready-slices`, outbox delivery, confirmation, and cancellation.
- The existing `provisioning.status <request-id>` is not enough for v0.4 because the onboarding slice is the dispatch grain.

Minimum remediation:

- Add `provisioning_request_lifecycle.yaml`.
- Add `provisioning_request` slots to `onboarding_workspace.yaml` and `cbu_workspace.yaml`.
- Add `service-resource.get-provisioning-status <slice-id>` as the v0.4 slice-oriented read.
- Keep existing `provisioning.status <request-id>` as compatibility, but do not treat it as satisfying the slice-grain requirement.

Priority: P1.

### RG4. Reference Nodes Are Used but Not Explicitly Registered

Two frozen/reference concepts are real in implementation:

- `onboarding_data_request_discoveries`
- `srdef_discovery_reasons`

The first is a frozen per-request snapshot. The second is the CBU-level discovery reason/source row from which the frozen snapshot is derived.

Why this matters:

- Agents can inspect slices but cannot walk clean ancestry from slice to discovery reason to SRDEF.
- Closure validation cannot distinguish mutable state nodes from immutable/frozen references.

Minimum remediation:

- Add explicit reference-node declarations to the relevant DAG/constellation metadata:
  - `ref.onboarding_data_request_discovery`
  - `ref.srdef_discovery_reason`
- Add read/list access where needed:
  - list discovery snapshots by `data_request_id`;
  - get discovery snapshot by `discovery_snapshot_id`;
  - get source discovery reason by `discovery_id` if not already covered by existing discovery explain APIs.

Priority: P1.

### RG5. SRDEF Governance Lifecycle Is Conflated With Attribute-Coverage Lifecycle

Current `service_resource_types.lifecycle_status` is used for sync/coverage:

```text
unsynced, synced, gaps_found, complete
```

The lifecycle vision for the SRDEF template itself is different:

```text
draft, active, deprecated, retired
```

These are not interchangeable:

- `active + gaps_found` is a valid stewardship state: the SRDEF exists and is active, but referenced attributes need governance work.
- `deprecated + complete` is also possible: the definition is structurally complete but no longer preferred.

Minimum remediation:

- Preserve current coverage status, but rename its SemOS meaning to `service_resource_def_coverage_lifecycle`.
- Add a separate governance column, for example `governance_status`, with `draft/active/deprecated/retired`.
- Update SRDEF sync so YAML-sourced definitions default to `active` unless marked otherwise.
- Register both lifecycle dimensions:
  - `state.srdef` governance state;
  - SRDEF coverage/readiness projection for attribute gap remediation.

Priority: P2.

### RG6. Typed Registry Convenience for `ServiceResourceDef` Is Missing

SRDEF sync currently writes `sem_reg.snapshots` through `SnapshotStore` directly. That works, but it bypasses the typed `RegistryService` pattern used by other SemOS object bodies.

Why this matters:

- It is not an operational blocker.
- It is a consistency gap for stewardship tools and future typed resolve paths.

Minimum remediation:

- Import `ServiceResourceDefBody` in `rust/src/sem_reg/registry.rs`.
- Add `publish_service_resource_def`, `resolve_service_resource_def`, and `resolve_service_resource_def_by_fqn`.
- Switch `srdef_loader` to the typed method or leave the direct store path with a test proving equivalence.

Priority: P2.

### RG7. Agent Lookup Surface Does Not Include New Operational IDs

Generic MCP DSL exposure exists, but `entity_lookup` does not expose the new lifecycle entities as lookup types. Agents therefore need UUIDs from raw DB inspection or prior context.

Missing lookup targets:

- `onboarding_data_request`
- `onboarding_data_request_slice`
- `provisioning_request`
- `cbu_resource_instance`
- `service_resource_def`
- `resource_owner_principal`

Why this matters:

- Coder can execute DSL only after resolving the right IDs.
- Sage can plan actions but may not be able to ground a slice/request/resource instance reliably.

Minimum remediation:

- Extend MCP/entity lookup taxonomy and resolution handlers for the targets above.
- Ensure `dsl_signature` and `dsl_complete` surface all new verbs.
- Add tests over `verbs_list`, `dsl_signature`, and `dsl_complete` for onboarding and service-resource domains.

Priority: P2.

### RG8. L4 Lifecycle Resources Have a DAG but No Runtime Constellation Map

`lifecycle_resources_dag.yaml` defines `application_instance` and `capability_binding`, and the implementation enforces live binding during data-request compilation. However, there is no corresponding `lifecycle_resources` constellation map/family surfaced to the runtime.

Why this matters:

- L4 enforcement can block a slice, but agents cannot naturally inspect the lifecycle-resource context that caused the block.
- The closure walk from onboarding to L4 binding is incomplete.

Minimum remediation:

- Add a `lifecycle_resources.yaml` constellation map/family or integrate these slots into an existing product/platform workspace deliberately.
- Cross-link from onboarding and CBU workspace maps where `l4_binding_required = true`.
- Provide read/list verbs for application instances and capability bindings if existing CRUD verbs are not enough.

Priority: P3.

### RG9. Cascade Declarations Are Not Machine-Checkable Enough

The implementation performs the required state mutations, and `domain_metadata.yaml` has table read/write footprints. But the audit vision requires explicit cascade declarations that can be compared against actual mutation sets.

Required declared cascades:

- `onboarding.compile-data-request`
  - creates data request/slices/attrs/discoveries;
  - may transition deal onboarding request `PENDING -> IN_PROGRESS`.
- `onboarding.dispatch-ready-slices`
  - creates or reuses `cbu_resource_instance`;
  - creates provisioning request;
  - writes `REQUEST_PREPARED`;
  - writes outbox;
  - moves slice to `dispatched`.
- outbox `ResourceOwnerDispatch`
  - writes `DISPATCHED`;
  - moves provisioning request to `sent`;
  - moves instance to `AWAITING_OWNER`;
  - moves slice to `awaiting_owner`.
- `service-resource.confirm-provisioning-result`
  - writes `RESULT` or `ERROR`;
  - moves provisioning request to `ack/completed/failed`;
  - moves instance to `AWAITING_OWNER/ACTIVE/FAILED`;
  - moves slice to `awaiting_owner/activated/failed`;
  - recomputes data request status.
- cancellations
  - move slices to `cancelled`;
  - move provisioning request to `cancelled`;
  - emit stand-down outbox where applicable;
  - move non-active instance to `CANCELLED`;
  - recompute parent request status.

Minimum remediation:

- Add a structured cascade manifest close to the verb metadata, using the repo's existing metadata conventions where possible.
- Add a test that compares the new manifest to table footprints and state-machine transitions.
- If no cascade auditor exists yet, add a lightweight fixture test that enforces these five verbs/effects.

Priority: P3.

### RG10. Closure Tests Are Missing for the New Lifecycle

Current tests verify compile/clippy/unit behavior, but not the full structural closure expected by the audit.

Minimum remediation:

- Add tests that assert:
  - onboarding workspace exposes compile, dispatch, cancel, read, and confirm verbs;
  - CBU workspace exposes cross-visible onboarding reads/actions;
  - Deal workspace exposes request compile/list/read paths;
  - KYC workspace exposes only `onboarding.get-slice-attrs`;
  - no dangling slot parent refs for the new nodes;
  - the valid verb set includes the expected lifecycle verbs from synthetic onboarding context.

Priority: P3.

## 5. Non-Gaps and Data Stewardship Items

These should not block implementation closure.

| Item | Classification | Reason |
|---|---|---|
| 9 SRDEFs in `gaps_found` | Data stewardship | Implementation is correctly surfacing missing/ungoverned attributes. |
| 36 legacy `service_resource_types` rows in `unsynced` | Data migration/stewardship | These are outside current YAML sync; decide whether to retire, map, or convert. |
| Direct per-verb MCP tools absent | Not a gap by itself | Current MCP architecture is generic DSL tool exposure. The real gap is lookup/completion/signature coverage. |
| `onboarding_handoff.yaml` absent | Naming/spec drift | Actual constellation is `onboarding.workspace` in `onboarding_workspace.yaml`. |
| `REQUEST_SENT` still allowed | Compatibility | `REQUEST_PREPARED` and `DISPATCHED` exist; keeping old event kind is harmless. |

## 6. Dependency-Ordered Implementation Plan

### P0. Correct Frozen SRDEF Lineage

Files likely touched:

- `rust/src/service_resources/onboarding_data_request.rs`
- tests in the same module or a service-resource integration test

Work:

1. Add `srdef_snapshot_id` to `DiscoveryRow`.
2. Select `srt.srdef_snapshot_id` in `load_active_discoveries`.
3. Write that value into `onboarding_data_request_slices.srdef_snapshot_id`.
4. Keep `discovery_snapshot_id` as the FK to `onboarding_data_request_discoveries`.
5. Add a regression test proving slice `srdef_snapshot_id` is a SemOS `service_resource_def` snapshot.

Acceptance:

- Compiled slices have distinct lineage fields:
  - `discovery_snapshot_id -> onboarding_data_request_discoveries`
  - `srdef_snapshot_id -> sem_reg.snapshots`

### P1. Register Operational Lifecycle Nodes

Files likely touched:

- `rust/config/sem_os_seeds/state_machines/cbu_resource_instance_lifecycle.yaml`
- `rust/config/sem_os_seeds/state_machines/provisioning_request_lifecycle.yaml`
- `rust/config/sem_os_seeds/dag_taxonomies/onboarding_request_dag.yaml`
- `rust/config/sem_os_seeds/constellation_maps/onboarding_workspace.yaml`
- `rust/config/sem_os_seeds/constellation_maps/cbu_workspace.yaml`
- `rust/config/sem_os_seeds/domain_metadata.yaml`

Work:

1. Add state machine for `cbu_resource_instance`.
2. Add state machine for `provisioning_request`.
3. Add slots in onboarding and CBU maps.
4. Add parent refs from slice to provisioning request/resource instance.
5. Add reference-node declarations for discovery snapshots and SRDEF discovery reasons.

Acceptance:

- Runtime can hydrate onboarding workspace and see data request, slice, attrs, provisioning request, and resource instance.
- CBU workspace can navigate to related onboarding data requests and resource instances.

### P2. Add Missing Read and Lookup Surfaces

Files likely touched:

- `rust/config/verbs/service-resource.yaml`
- `rust/config/verbs/onboarding.yaml`
- `rust/src/domain_ops/onboarding_data_request.rs`
- `rust/src/api/service_resource_routes.rs`
- MCP lookup/enrichment handlers

Work:

1. Add `service-resource.get-provisioning-status <slice-id>`.
2. Add get/list verbs for provisioning requests if not covered by the slice status read.
3. Add get/list verbs for CBU resource instances if existing service-resource verbs do not provide reliable instance reads.
4. Add entity lookup support for the new operational entities.
5. Add DSL/MCP surface tests for `verbs_list`, `dsl_signature`, `dsl_complete`, and lookup.

Acceptance:

- Agents can discover and invoke/read the lifecycle using DSL/MCP without raw SQL or UUID guessing.

### P3. Split SRDEF Governance From Coverage

Files likely touched:

- new migration for `service_resource_types.governance_status`
- `rust/config/sem_os_seeds/state_machines/service_resource_def_lifecycle.yaml`
- maybe new `service_resource_def_coverage_lifecycle.yaml`
- `rust/src/service_resources/srdef_loader.rs`
- SemOS DAG and registry stewardship map

Work:

1. Add `governance_status` with `draft/active/deprecated/retired`.
2. Preserve existing coverage status under explicit coverage naming.
3. Update sync to set governance status conservatively.
4. Update SemOS DAG to use governance state for `state.srdef`.
5. Keep coverage lifecycle available for gap remediation.

Acceptance:

- An SRDEF can be `active` while also `gaps_found`.
- Agents can distinguish "is this template live?" from "is its attribute dictionary fully governed?"

### P4. Add L4 Runtime Visibility

Files likely touched:

- `rust/config/sem_os_seeds/constellation_maps/lifecycle_resources.yaml`
- `rust/config/sem_os_seeds/constellation_families/lifecycle_resources.yaml`
- existing onboarding/CBU maps
- maybe application/capability read verbs

Work:

1. Add a runtime constellation map for `application_instance` and `capability_binding`, or deliberately host those slots in an existing product/platform map.
2. Add cross-links from onboarding slice L4 blocking state to lifecycle resources.
3. Ensure agents can inspect the live binding that satisfies or blocks a request SRDEF.

Acceptance:

- A slice blocked by missing L4 binding points to an inspectable lifecycle-resource context.

### P5. Add Cascade and Closure Validation

Files likely touched:

- verb metadata files
- domain metadata
- test modules under SemOS runtime/Sage/DSL

Work:

1. Add structured cascade declarations for compile, dispatch, outbox dispatch, confirm, cancel request, and cancel slice.
2. Add tests that compare cascade declarations with state-machine transitions and table footprints.
3. Add closure tests for onboarding, CBU, Deal, and KYC workspace contexts.
4. Add synthetic feasible-frontier tests for the onboarding lifecycle.

Acceptance:

- No orphan new nodes.
- No dangling parent refs.
- Expected verbs are reachable from synthetic onboarding context.
- KYC only exposes `onboarding.get-slice-attrs` for this lifecycle.

## 7. Verification Gate

After implementation, run:

```bash
cd rust
cargo fmt
cargo check
cargo clippy -- -D warnings
cargo test
```

Targeted tests to add or extend:

- SRDEF snapshot pinning regression.
- Onboarding workspace valid verb set.
- CBU/Deal/KYC cross-visible verb assertions.
- `service-resource.get-provisioning-status` by slice id.
- MCP/DSL lookup and signature coverage.
- Cascade manifest consistency.

DB checks after migration/sync:

```sql
SELECT COUNT(*)
FROM "ob-poc".onboarding_data_request_slices s
JOIN sem_reg.snapshots snap ON snap.snapshot_id = s.srdef_snapshot_id
WHERE snap.object_type = 'service_resource_def';

SELECT status, COUNT(*)
FROM "ob-poc".cbu_resource_instances
GROUP BY status;

SELECT status, COUNT(*)
FROM "ob-poc".provisioning_requests
GROUP BY status;
```

## 8. Recommended Next Tranche

Implement in this order:

1. P0 snapshot pinning.
2. P1 lifecycle node registration.
3. P2 read/lookup surfaces.
4. P5 closure tests for the newly registered nodes.
5. P3 governance/coverage split.
6. P4 L4 runtime constellation.

Reasoning:

- P0 fixes a real lineage correctness bug.
- P1 and P2 make the current loop visible to Sage/REPL/MCP.
- P5 prevents regressions and replaces speculative closure claims with executable checks.
- P3 and P4 are important, but they are less likely to break the already-working dispatch/return loop.

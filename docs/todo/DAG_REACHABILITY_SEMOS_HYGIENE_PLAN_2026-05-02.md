# DAG Reachability / SemOS Hygiene Plan - 2026-05-02

> **Status:** Draft for Adam review. Do not execute implementation slices until approved.
> **Purpose:** Make DAG state reachability, SemOS verb wiring, and schema constraints coherent and mechanically enforceable.
> **Architecture:** SemOS remains the canonical state plane. Every authoritative state mutation must be reachable through a registered verb, or explicitly declared as `entry_via: trigger | scheduler | signal | cascade`.

## Evidence

Authoritative evidence has been copied into the repo:

- `docs/governance/dag-reachability-audit-2026-05-02.md`
- `docs/governance/dag-reachability-semos-reconciliation-2026-05-02.md`
- `docs/todo/P0_dag_reachability_remediation.md`

The original files in `~/Downloads` are not required for execution.

## Execution Defaults

- Run Cargo commands from `rust/`, not repo root.
- After every code edit, run `cargo check` before marking a TODO complete.
- Use `cargo fmt` before committing a slice.
- Use `cargo clippy --workspace --all-targets -- -D warnings` at phase boundaries and before final completion.
- Use `cargo test --workspace` for broad validation; narrower tests are acceptable inside a slice only if the slice explicitly says so.
- Do not delete a verb that is still referenced by a DAG transition unless the same slice updates the DAG or adds a replacement path.
- Do not add duplicate migrations when an existing migration already expresses the intended schema. First verify whether the live DB has missed an existing migration.
- Validator work starts in report-only mode. It becomes hard-fail only after the relevant hygiene class is clean.
- One commit per completed slice, except explicitly coupled slices that cannot pass intermediate validation.
- Public Rust functions added by this campaign must have `///` docs with an `Examples` section, per `AGENTS.md`.

## Proposed Schema Authority

`xtask schema-export` currently writes:

- `schema_export.sql`
- `migrations/master-schema.sql`

This plan treats root `migrations/master-schema.sql` as the schema export artifact for the campaign because that is what the current tool writes. `rust/migrations/*.sql` remains the migration source set. `rust/migrations/master-schema.sql` is treated as stale evidence unless Adam separately decides to make `xtask` emit both schema files.

## Phase 0 - Baseline and Current-State Inventory

Goal: freeze the real repo state before any rips or refactors.

### Slice 0.a - Verify evidence and command surface

Steps:

1. Confirm the three evidence files above exist.
2. Confirm `xtask` subcommands:
   - `cargo run -p xtask -- schema-export`
   - `cargo run -p xtask -- verbs lint`
   - `cargo run -p xtask -- reconcile validate`
3. Confirm the current `xtask schema-export` target is root `migrations/master-schema.sql`.

Validation:

- `cd rust && cargo check`

Deliverable:

- Short note in the slice report listing command names and schema target.

### Slice 0.b - Generate current-state inventory

Steps:

1. Inventory every `SimpleStatusConfig` and compare against root `migrations/master-schema.sql`.
2. Inventory every DAG `via:` verb reference and whether the FQN exists in YAML and the runtime registry.
3. Inventory plugin verbs whose YAML says `behavior: plugin` but no `SemOsVerbOp` is registered.
4. Inventory every planned rip target and all DAG/YAML/test references.
5. Write the findings to `docs/governance/dag-hygiene-current-state-2026-05-02.md`.

Validation:

- `cd rust && cargo check`

Acceptance:

- We have a machine-readable or tabular current-state report before touching behavior.

## Phase 1 - Report-Only Validator Foundation

Goal: make later cleanup measurable without breaking the existing developer loop immediately.

### Slice 1.a - SimpleStatus drift report

Implement a report-only check that detects:

- missing carrier table
- missing state column
- invalid target enum/check value
- likely primary-key mismatch where the config `pk_col` is absent

Known expected findings before remediation include:

- `cbu-ca.*`: `state_col` should be `ca_status`; `pk_col` likely should be `event_id`, not `id`.
- `deal.submit-for-bac`, `deal.bac-approve`, `deal.bac-reject`: current SimpleStatus behavior conflicts with substate YAML intent.
- `settlement-chain.*`: table has no `status` column.
- `trading-profile.enter-parallel-run`, `trading-profile.suspend`, `trading-profile.abort-parallel-run`: target values mismatch current CHECK.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo run -p xtask -- reconcile validate` still exits according to current behavior.
- New report command exits zero but prints findings.

### Slice 1.b - DAG verb reference report

Implement a report-only check that verifies every DAG transition `via:` verb has:

- a YAML declaration
- a registered runtime implementation if `behavior: plugin`
- a `SimpleStatusConfig` if implemented through SimpleStatus

Acceptance:

- Rips in later phases are blocked when the target is still referenced by a DAG unless the same slice updates the DAG or replaces the writer.

### Slice 1.c - Writer annotation schema, report-only

Add an optional YAML field for explicit writes:

```yaml
writes:
  - table: deals
    column: bac_status
    value: approved
```

Rules:

- Field is optional in this phase.
- Parser accepts it without requiring all verbs to declare it.
- Validator can use it when present.

Validation:

- Existing YAML parses unchanged.
- `cd rust && cargo check`

## Phase 2 - Mechanical Drift Cleanup

Goal: fix broken known drift without changing architecture.

### Slice 2.a - Fix `cbu-ca.*` SimpleStatus configs

Operation: same-slice repair, not rip.

Steps:

1. Change the five `cbu-ca.*` configs in `rust/src/domain_ops/simple_status_op.rs`:
   - `state_col: "status"` to `state_col: "ca_status"`
   - verify `pk_col`; if table primary key is `event_id`, change `pk_col: "id"` to `pk_col: "event_id"`.
2. Keep YAML declarations in `rust/config/verbs/cbu-ca.yaml`.
3. Add focused integration coverage if no existing test exercises these five verbs.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo test --workspace`
- `cd rust && cargo run -p xtask -- reconcile validate`

Acceptance:

- Five cbu-ca verbs update the actual carrier row and emit normal state advance.

### Slice 2.b - Deal BAC/substate deconflict

Operation: rip-and-replace in one commit.

Steps:

1. Remove `SimpleStatusConfig` entries for:
   - `deal.submit-for-bac`
   - `deal.bac-approve`
   - `deal.bac-reject`
2. Add dedicated plugin ops in `rust/crates/sem_os_postgres/src/ops/deal.rs`:
   - `deal.submit-for-bac`: requires `deal_status = NEGOTIATING`; sets `deal_status = IN_CLEARANCE`, `bac_status = 'in_review'`, `kyc_clearance_status = 'pending'` atomically.
   - `deal.bac-approve`: requires `deal_status = IN_CLEARANCE`; sets `bac_status = 'approved'`; preserves `deal_status`.
   - `deal.bac-reject`: requires `deal_status = IN_CLEARANCE`; sets `bac_status = 'rejected'`; preserves or transitions `deal_status` only according to the existing YAML/business rule confirmed in this slice.
3. Register the ops in `sem_os_postgres::ops::build_registry()`.
4. Update `rust/config/verbs/deal.yaml` so behavior and descriptions match actual substate semantics.
5. Add `writes:` annotations for the three verbs.
6. Do not add a duplicate D-004 migration. First verify whether existing `rust/migrations/20260429_carrier_04_deals_status_in_clearance.sql` and `20260429_carrier_08_deals_in_clearance_substates.sql` already express the target schema.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo test --workspace`
- Integration test: `deal.create -> deal.update-status NEGOTIATING -> deal.submit-for-bac` yields `deal_status = IN_CLEARANCE`, `bac_status = in_review`, `kyc_clearance_status = pending`.
- Integration test: `deal.bac-approve` sets `bac_status = approved` and leaves `deal_status = IN_CLEARANCE`.
- Integration test: old `deal_status = BAC_APPROVAL/KYC_CLEARANCE` write path is rejected by the schema if the current live schema allows the D-004 constraint.

Acceptance:

- No duplicate FQN registrations.
- Deal substate YAML and runtime behavior agree.

### Slice 2.c - Settlement-chain and trading-profile drift decision packet

Operation: no code changes unless Adam approves the chosen path.

The current evidence supports two possible strategies:

- Preserve DAG topology: add/repair schema columns/checks so the existing DAG transitions are executable.
- Rip dead lifecycle: remove SimpleStatus configs and update DAG transitions/states in the same slice so there are no dangling references.

Steps:

1. Produce a short decision packet in `docs/governance/dag-hygiene-settlement-trading-decision-2026-05-02.md`.
2. For each affected FQN, list:
   - current config
   - current schema
   - DAG references
   - YAML declaration
   - recommended action
3. STOP for Adam decision.

Validation:

- `cd rust && cargo check`

Acceptance:

- No rip is executed while DAG references remain unresolved.

## Phase 3 - Deal Direct-Verb Closure

Goal: make IN_CLEARANCE to CONTRACTED reachable through direct verbs.

### Slice 3.a - Add missing substate writer verbs

Steps:

1. Add dedicated ops:
   - `deal.bac-mark-in-review`
   - `deal.kyc-mark-in-review`
2. Confirm whether `deal.update-kyc-clearance` already covers `approved` and `rejected`. If not, repair it in this slice.
3. Add YAML declarations and `writes:` annotations.
4. Register the ops.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo test --workspace`
- Integration test drives:
  `deal.create -> deal.update-status NEGOTIATING -> deal.submit-for-bac -> deal.bac-mark-in-review -> deal.bac-approve -> deal.kyc-mark-in-review -> deal.update-kyc-clearance approved -> deal.update-status CONTRACTED`

Acceptance:

- Contracted deal path is reachable without macros or direct SQL.

### Slice 3.b - Deal precondition closure report green

Steps:

1. Extend report-only validator to use `writes:` annotations for deal substates.
2. Confirm no Class B deal substate closure violation remains.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo run -p xtask -- reconcile validate`
- Report-only hygiene command shows deal substate closure clean.

## Phase 4 - Discovery Pipeline State Coupling

Goal: connect the externalized discovery pipeline to SemOS state.

### Slice 4.a - Discovery state design packet

Default proposal:

- Carrier: `"ob-poc".cbus`
- Column: `cbu_discovery_state`
- States: `PENDING`, `DISCOVERING`, `ROLLUP`, `POPULATE`, `PROVISION`, `READY`, `FAILED`, `BLOCKED`
- DAG: `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml`

Steps:

1. Verify current pipeline verbs and their arg shapes in `service_pipeline.rs` and `service-pipeline.yaml`.
2. Produce the exact transition table for:
   - `service-intent.create`
   - `service-intent.supersede`
   - `discovery.run`
   - `attributes.rollup`
   - `attributes.populate`
   - `attributes.set`
   - `provisioning.run`
   - `readiness.compute`
   - `pipeline.full`
   - `service-resource.sync-definitions`
3. STOP if a pipeline verb lacks enough args to identify the owning CBU.

Validation:

- `cd rust && cargo check`

### Slice 4.b - Add `cbu_discovery_state`

Steps:

1. Add migration for `cbus.cbu_discovery_state` with the approved states.
2. Add the DAG slot and transitions to `cbu_dag.yaml`.
3. Run schema export after applying migration to the local DB.

Validation:

- `cd rust && cargo check`
- Migration applies locally.
- `cd rust && cargo run -p xtask -- schema-export`
- `cd rust && cargo run -p xtask -- reconcile validate`

### Slice 4.c - Emit discovery PendingStateAdvance

Steps:

1. Update service-pipeline ops to emit `PendingStateAdvance` after successful writes.
2. Update YAML `transition_args` and `writes:` where applicable.
3. Keep query-only verbs preserving.
4. Do not rip `service-intent.suspend/resume/cancel`; they already exist as SimpleStatus registrations in the current repo. Repair them only if inventory proves runtime or YAML drift.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo test --workspace`
- New integration test drives the approved discovery cycle and observes `cbu_discovery_state` through DAG/SemOS state, not raw row-count inference.

## Phase 5 - Cascade Hygiene

Goal: eliminate off-carrier direct SQL for SemOS-governed child state.

### Slice 5.a - Add in-transaction child dispatch API

Current blocker:

- `SemOsVerbOpRegistry` supports lookup/register only.
- Parent ops receive `VerbExecutionContext` and a transaction scope, but not a registry-backed child dispatcher.

Proposed implementation:

1. Add a SemOS child-dispatch service trait in `dsl-runtime` that can dispatch a child FQN with JSON args using the caller's `VerbExecutionContext` and transaction scope.
2. Implement the trait in the host using the existing `SemOsVerbOpRegistry`.
3. Wire it into the platform `ServiceRegistry`.
4. Add trace output for parent FQN -> child FQN.

Validation:

- `cd rust && cargo check`
- Unit test: a parent test op dispatches a child test op through the service in the same transaction scope.

Acceptance:

- Cascade refactors can honestly dispatch via registry instead of directly calling child structs or duplicating SQL.

### Slice 5.b - Child verb gap packet

Steps:

1. For each cascade violator, list existing or missing child verbs:
   - `cbu.create`
   - `cbu.assign-ownership`
   - `cbu.assign-control`
   - `cbu.assign-trust-role`
   - `capital.adjust-holding`
   - `cbu.decide`
   - `cbu.add-product`
   - `cbu.delete-cascade`
2. Use current FQNs, not assumed names. Known examples:
   - current client-group removal appears to be `client-group.entity-remove`, not `client-group.remove-entity`.
   - `service-resource.provision` exists.
   - `delivery.start` exists as SimpleStatus.
   - `capital.issue-shares` exists and may satisfy `capital.adjust-holding`.
   - `service-intent.activate`, `trading-profile.clone-from-template`, `entity.deactivate`, `cbu-role.terminate`, and a `cbu_group_members` removal child need confirmation or implementation.
3. STOP for Adam if new child verbs are required.

Validation:

- `cd rust && cargo check`

### Slice 5.c+ - Refactor cascade violators one at a time

Order after child verbs exist:

1. `capital.adjust-holding`
2. `cbu.decide`
3. `cbu.assign-ownership/control/trust-role`
4. `cbu.create`
5. `cbu.add-product`
6. `cbu.delete-cascade`

Per-slice rules:

- Characterize current behavior before refactor.
- Refactor off-carrier writes to child dispatch.
- Preserve event/audit emission order.
- Add or update `transition_args.cascades` only after the YAML schema supports it.

Validation per slice:

- `cd rust && cargo check`
- Focused characterization tests
- `cd rust && cargo test --workspace`

## Phase 6 - Remaining Class B Closure

Goal: close actual verbification-debt tuples, not operational signals or cross-workspace handoffs.

### Slice 6.a - Generate exact 7-tuple debt table

The reconciliation gives category counts and top examples, but not a complete implementable 7-row table.

Steps:

1. Use Phase 1 reports plus DAG predicate parsing to generate:
   - table
   - column
   - required value
   - DAG slot/state/precondition
   - existing writers
   - recommended verb FQN
2. Write `docs/governance/dag-hygiene-class-b-writer-debt-2026-05-02.md`.
3. STOP for Adam review if any row requires business naming or workflow semantics.

Validation:

- `cd rust && cargo check`

### Slice 6.b - Add approved writer verbs

Steps:

1. Implement only the approved rows from 6.a.
2. Add YAML declarations and `writes:` annotations.
3. Add focused tests proving each required tuple can be written through a registered verb.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo test --workspace`
- Hygiene report shows no remaining true verbification-debt rows.

## Phase 7 - `entry_via` Formalization

Goal: turn legitimate non-verb reachability into explicit, validated metadata.

### Slice 7.a - Extend DAG schema

Steps:

1. Add optional `entry_via` to `StateDef`.
2. Supported values:
   - `verb`
   - `cascade { parent: String }`
   - `trigger { name: String }`
   - `scheduler { name: String }`
   - `signal { source: String }`
3. Existing DAG YAML must parse unchanged.

Validation:

- `cd rust && cargo check`
- DAG parser tests

### Slice 7.b - Generate annotation inventory

Steps:

1. Produce a per-state inventory for all four DAGs:
   - `deal_dag.yaml`
   - `cbu_dag.yaml`
   - `instrument_matrix_dag.yaml`
   - `lifecycle_resources_dag.yaml`
2. Pre-fill obvious direct-verb states.
3. Pre-fill trigger/scheduler/signal only where the DAG already has backend annotations.
4. Isolate the 12 NONE/edge cases and 3 mismatches from the reconciliation.
5. Write `docs/governance/dag-entry-via-inventory-2026-05-02.md`.
6. STOP for Adam direction on edge cases.

Validation:

- `cd rust && cargo check`

### Slice 7.c - Annotate DAGs

Steps:

1. Apply approved `entry_via` annotations.
2. Resolve the three known mismatches:
   - deal operational `OFFBOARDED`
   - deal rate-card `CANCELLED`
   - one trading-profile prune/cascade state after Phase 2.c decision

Validation:

- `cd rust && cargo check`
- DAG YAML parse
- `cd rust && cargo run -p xtask -- reconcile validate`

## Phase 8 - Hard-Fail Validator Enforcement

Goal: make hygiene drift fail the standard reconciliation gate.

### Slice 8.a - Hard-fail SimpleStatus drift

Activation condition:

- Phase 2 is clean.

Validation:

- Synthetic failing fixture proves non-zero exit.
- Current repo exits zero.

### Slice 8.b - Hard-fail precondition closure

Activation condition:

- Phase 6 is clean.
- `writes:` annotations exist for plugin writers needed by closure checks.

Validation:

- Synthetic failing fixture proves non-zero exit.
- Current repo exits zero.

### Slice 8.c - Hard-fail cascade pattern

Activation condition:

- Phase 5 is clean.
- YAML schema supports declared cascades.
- Audit/event emission exception is represented explicitly.

Validation:

- Synthetic failing fixture proves non-zero exit.
- Current repo exits zero.

### Slice 8.d - Hard-fail `entry_via` consistency

Activation condition:

- Phase 7 is clean.

Validation:

- Synthetic failing fixture proves non-zero exit.
- Current repo exits zero.

## Final Completion Criteria

- All evidence docs live in repo.
- Root `migrations/master-schema.sql` is refreshed from the approved local DB state.
- No `SimpleStatusConfig` targets a missing column, wrong primary key, or invalid state value.
- Deal IN_CLEARANCE to CONTRACTED is reachable through direct registered verbs.
- Discovery progress is observable and driveable through SemOS/DAG state.
- Cascade violators dispatch SemOS-governed child writes through the registry-backed child dispatcher.
- Every true Class B verbification-debt tuple has a writer.
- Every non-verb state has approved `entry_via`.
- `cd rust && cargo run -p xtask -- reconcile validate` exits zero and includes the four hard hygiene checks.
- `cd rust && cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cd rust && cargo test --workspace` passes, or any residual failures are explicitly documented as pre-existing and unrelated.

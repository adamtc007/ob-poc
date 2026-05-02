# DAG Reachability / SemOS Hygiene - Phase 5 to 8 Closeout Plan - 2026-05-02

> **Status:** Approved and implemented.
> **Scope:** Close remaining Phases 5 through 8 from `docs/todo/DAG_REACHABILITY_SEMOS_HYGIENE_PLAN_2026-05-02.md`.
> **Constraint:** No new architecture. Use existing SemOS/DAG/DSL seams only.

## Non-Negotiables

- No new execution framework.
- No alternate cascade engine.
- No speculative verb names.
- No rip while a DAG transition still references the FQN unless the same slice replaces or removes that transition.
- Report packets come before code where current evidence is incomplete.
- Validator checks start report-only unless the tranche explicitly promotes them to hard-fail after the class is clean.

## Existing Seams To Use

- `dsl_runtime::VerbExecutionContext.services`
- `dsl_runtime::ServiceRegistry` and `ServiceRegistryBuilder`
- `sem_os_postgres::ops::SemOsVerbOpRegistry`
- `dsl_runtime::tx::TransactionScope`
- Existing `writes:` YAML field on `VerbConfig`
- Existing `transition_args` YAML field
- Existing `xtask reconcile validate`

The only allowed extension is adding narrow service traits or config fields where they directly expose an existing concept. Do not introduce a new runtime plane.

## Tranche A - Cascade Closeout Readiness

Goal: make Phase 5 implementable without guessing.

### Slice A1 - Child Verb Gap Packet

Deliverable:

- `docs/governance/dag-hygiene-cascade-child-verb-gap-packet-2026-05-02.md`

Steps:

1. For each cascade violator, inventory direct off-carrier writes from source:
   - `cbu.create`
   - `cbu.assign-ownership`
   - `cbu.assign-control`
   - `cbu.assign-trust-role`
   - `capital.adjust-holding`
   - `cbu.decide`
   - `cbu.add-product`
   - `cbu.delete-cascade`
2. For each off-carrier write, map to an existing registered verb if one exists.
3. Use current FQNs only. Confirm, do not assume:
   - `delivery.start`
   - `service-resource.provision`
   - `kyc-case.update-status`
   - `capital.issue-shares`
   - `client-group.entity-remove`
4. List missing child verbs separately with:
   - intended carrier table
   - column/value or row operation
   - parent verb requiring it
   - recommended FQN
   - whether it is SemOS state, audit/event emission, or operational side effect
5. STOP for Adam approval if any missing child verb requires naming or business semantics.

Validation:

- `cd rust && cargo check`

Acceptance:

- No cascade refactor proceeds with an unknown child verb.

### Slice A2 - Existing-Registry Child Dispatch Service

Goal: allow parent plugin ops to invoke child plugin ops through the current registry and current transaction scope.

Implementation shape:

1. Add a narrow object-safe service trait in `dsl-runtime` service traits, for example `SemOsChildDispatcher`.
2. Trait method accepts:
   - child FQN
   - JSON args
   - mutable `VerbExecutionContext`
   - mutable `TransactionScope`
3. Implement the trait in the host using the existing `SemOsVerbOpRegistry`.
4. Register it through existing `ServiceRegistryBuilder`.
5. Parent ops obtain it with `ctx.service::<dyn SemOsChildDispatcher>()`.

Rules:

- The dispatcher must not open a new transaction.
- The dispatcher must not bypass `SemOsVerbOpRegistry`.
- The dispatcher must preserve `VerbExecutionContext` mutations.
- Trace/log parent FQN -> child FQN in the existing execution/audit style; no new tracing subsystem.

Validation:

- `cd rust && cargo check`
- Unit test with a parent test op dispatching a child test op in the same fake transaction scope.

Acceptance:

- Cascade refactors can call child verbs without direct SQL or direct child struct calls.

## Tranche B - Cascade Refactors

Goal: eliminate off-carrier direct SQL for SemOS-governed child state.

Per-slice process:

1. Add or confirm characterization coverage before refactor.
2. Refactor only the off-carrier state write.
3. Leave event/audit emissions in place unless they are confirmed SemOS-governed carrier state.
4. Use child dispatch service from Slice A2.
5. Add `writes:` to child verbs where missing.
6. Add `transition_args.cascades` only if existing YAML schema supports the needed shape; otherwise defer cascade declaration to Tranche E validator/schema work.

### Slice B1 - `capital.adjust-holding`

Likely child: `capital.issue-shares`, subject to A1 confirmation.

Validation:

- `cd rust && cargo check`
- Focused tests for current holding/share issuance behavior.

### Slice B2 - `cbu.decide`

Likely child: `kyc-case.update-status` for the `cases` update.

Rules:

- Keep `case_evaluation_snapshots` insert in place if classified as event/audit emission.

Validation:

- `cd rust && cargo check`
- Focused `cbu.decide` characterization test.

### Slice B3 - `cbu.assign-ownership/control/trust-role`

Likely need one existing or approved child for `entity_relationships`.

Validation:

- `cd rust && cargo check`
- Focused tests for all three role assignment verbs.

### Slice B4 - `cbu.create`

Refactor only after child verbs are confirmed.

Validation:

- `cd rust && cargo check`
- Focused `cbu.create` characterization test.

### Slice B5 - `cbu.add-product`

Expected children to confirm:

- `delivery.start`
- `service-resource.provision`
- `service-intent.activate` or approved equivalent
- `trading-profile.clone-from-template` or approved equivalent

Stop condition:

- If any child does not exist, STOP and implement the approved child verb first in a separate slice.

Validation:

- `cd rust && cargo check`
- Focused `cbu.add-product` characterization test.

### Slice B6 - `cbu.delete-cascade`

Expected children to confirm:

- client group entity removal
- CBU structure unlink
- entity deactivation
- CBU role termination
- CBU group-member removal

Stop condition:

- If a deletion has no SemOS-safe child verb, STOP for Adam approval before adding it.

Validation:

- `cd rust && cargo check`
- Focused `cbu.delete-cascade` characterization test.
- Audit/event ordering preserved.

## Tranche C - Remaining Class B Writer Closure

Goal: close true verbification-debt tuples.

### Slice C1 - Generate Exact Writer-Debt Packet

Deliverable:

- `docs/governance/dag-hygiene-class-b-writer-debt-2026-05-02.md`

Steps:

1. Use current DAG predicates and existing `writes:` annotations.
2. Produce exact rows:
   - table
   - column
   - required value
   - DAG slot/state/precondition
   - existing writer, if any
   - recommended verb FQN
3. Exclude legitimate trigger/scheduler/signal/cascade cases from this table; those belong to Tranche D.
4. STOP if any row needs business naming.

Validation:

- `cd rust && cargo check`

### Slice C2 - Add Approved Writer Verbs

Steps:

1. Implement only approved C1 rows.
2. Prefer existing `SimpleStatusConfig` for simple fixed status writes.
3. Use dedicated plugin ops only where the write is not a SimpleStatus fit.
4. Add YAML declarations and `writes:`.
5. Add focused tests proving each tuple is writable through a registered verb.

Validation:

- `cd rust && cargo check`
- Focused per-verb tests
- `cd rust && cargo run -p xtask -- reconcile validate`

Acceptance:

- No remaining true Class B writer-debt rows.

## Tranche D - `entry_via` Formalization

Goal: represent legitimate non-verb reachability explicitly.

### Slice D1 - Minimal DAG Schema Extension

Steps:

1. Add optional `entry_via` to the existing DAG state type.
2. Keep existing YAML valid.
3. Support only these shapes:
   - `verb`
   - `cascade: { parent: ... }`
   - `trigger: { name: ... }`
   - `scheduler: { name: ... }`
   - `signal: { source: ... }`

Validation:

- `cd rust && cargo check`
- DAG parser tests.

### Slice D2 - Entry-Via Inventory Packet

Deliverable:

- `docs/governance/dag-entry-via-inventory-2026-05-02.md`

Steps:

1. Inventory states across:
   - `deal_dag.yaml`
   - `cbu_dag.yaml`
   - `instrument_matrix_dag.yaml`
   - `lifecycle_resources_dag.yaml`
2. Pre-fill direct verb states only where a transition `via:` exists.
3. Pre-fill cascade only where Tranche B confirms parent/child dispatch.
4. Pre-fill trigger/scheduler/signal only where existing DAG comments or runtime source prove it.
5. Isolate edge cases for Adam approval.

Validation:

- `cd rust && cargo check`

### Slice D3 - Apply Approved Entry-Via Annotations

Steps:

1. Annotate approved states.
2. Resolve known mismatches:
   - deal operational `OFFBOARDED`
   - deal rate-card `CANCELLED`
   - trading-profile prune/cascade state after the trading-profile schema decision
3. Do not annotate guesses.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo run -p xtask -- reconcile validate`

Acceptance:

- Every non-verb-reachable state is either annotated with approved `entry_via` or remains listed as an explicit unresolved edge case.

## Tranche E - Validator Hardening

Goal: make closed hygiene classes regress-proof.

### Slice E1 - SimpleStatus Drift Hard-Fail

Precondition:

- Mechanical drift class is clean.

Steps:

1. Promote existing SimpleStatus drift report from report-only to hard-fail.
2. Add synthetic regression coverage for:
   - missing table
   - missing state column
   - invalid target value
   - missing PK column

Validation:

- `cd rust && cargo check`
- `cd rust && cargo run -p xtask -- reconcile validate`

### Slice E2 - Precondition Closure Hard-Fail

Precondition:

- Tranche C clean.

Steps:

1. Use existing `writes:` annotations.
2. Fail when a DAG precondition requires a tuple with no registered writer and no approved `entry_via`.
3. Add synthetic regression test.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo run -p xtask -- reconcile validate`

### Slice E3 - Cascade Pattern Hard-Fail

Precondition:

- Tranche B clean.

Steps:

1. Detect plugin ops that directly write off-carrier SemOS-governed state without approved child dispatch.
2. Exempt declared audit/event emissions explicitly.
3. Add synthetic regression test.

Implementation note:

- Prefer simple targeted source scanning over introducing a new AST dependency unless existing parser infrastructure already supports it.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo run -p xtask -- reconcile validate`

### Slice E4 - `entry_via` Consistency Hard-Fail

Precondition:

- Tranche D clean.

Steps:

1. Fail states with no incoming verb path and no approved `entry_via`.
2. Fail `entry_via: cascade` where parent cascade declaration/implementation is absent.
3. Fail trigger/scheduler/signal annotations that do not match known configured names/sources.
4. Add synthetic regression test.

Validation:

- `cd rust && cargo check`
- `cd rust && cargo run -p xtask -- reconcile validate`

## Final Gate

Required before declaring Phases 5 through 8 complete:

- `cd rust && cargo check`
- `cd rust && cargo fmt --check`
- `cd rust && cargo clippy --workspace --all-targets -- -D warnings`
- `cd rust && cargo test --workspace`
- `cd rust && cargo run -p xtask -- reconcile validate`

Final acceptance:

- Cascade child writes go through existing registry dispatch.
- No true Class B writer-debt rows remain.
- Non-verb state reachability is explicitly represented with approved `entry_via`.
- Reconcile hard-fails the four hygiene regressions.
- No new architecture was introduced.

## Implementation Closeout - 2026-05-02

Status: complete.

- Tranche A: child-verb gap packet produced, Adam decisions captured, and registry-backed child dispatch implemented through existing `ServiceRegistry` / `SemOsVerbOpRegistry` seams.
- Tranche B: approved cascade parents now route off-carrier SemOS-governed writes through child verbs; the final `cbu.assign-fund-role` relationship write was moved to `entity-relationship.upsert`.
- Tranche C: writer-debt packet concluded no new writer verbs were required; missing closure was metadata declaration debt and has been applied.
- Tranche D: `entry_via` schema support and approved annotations are in place.
- Tranche E: `reconcile validate` now hard-fails targeted SimpleStatus drift, plugin/DAG runtime drift, deal substate writer gaps, entry-via consistency failures, and known cascade parent direct off-carrier mutations.

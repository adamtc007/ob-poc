# S6 Footprint Remediation Plan

## Objective

Burn down the `811` live verbs that still lack explicit footprint registry entries and remove the ambiguity that is poisoning the SemOS pool.

This phase is not about the new 5D routing dimensions. Those are already in place.

This phase is about the older explicit footprint layer:

- `entity_scope`
- `data_reads`
- `data_writes`
- `preconditions`
- `postconditions`

The target outcome is:

- every live verb has an explicit footprint status
- every business verb has an explicit or mechanically derived footprint
- no-harm Sage/show/research verbs are marked intentionally non-mutating instead of being left as silent gaps

## Important Clarification

The `811` are not true `orphan` rows.

They are mostly:

- live verbs with no explicit `verb_data_footprint` entry yet
- or verbs whose footprint is only implicit in YAML/Rust and not yet materialized into the registry overlay

So the burn-down target is:

`811 missing explicit footprint entries`

not:

`811 orphan rows`

## Core Principle

Do not treat all missing verbs as the same problem.

There are 2 broad classes:

1. `Operational / business verbs`
These must end with explicit or derived reads/writes/pre/post footprint.

2. `No-harm Sage / show / research / navigation verbs`
These usually do not mutate business data and often do not have meaningful DB-side business-table writes.
They must still get an explicit footprint status row, but many should resolve to:

- `reads only`
- `delegated`
- `system_only`
- `none`

They should not remain silent gaps.

## Remediation Strategy

Use a 3-layer approach:

1. classify every missing verb
2. auto-derive everything that is mechanically derivable
3. manually backfill only the genuinely unresolved remainder

This avoids trying to hand-author 811 rows from scratch.

## Required Schema/Model Adjustment

The current overlay is too binary:

- row exists
- row missing

That is not enough.

Add an explicit status model to the footprint authoring layer.

Suggested fields for each verb footprint row:

- `footprint_mode`
  - `explicit`
  - `derived_crud`
  - `derived_lifecycle`
  - `delegated`
  - `read_only`
  - `system_only`
  - `none`
  - `unknown`
- `entity_scope`
- `reads`
- `writes`
- `preconditions`
- `postconditions`
- `evidence_source`
  - `yaml_crud`
  - `yaml_lifecycle`
  - `rust_handler`
  - `state_machine`
  - `manual`

This is the key change that removes ambiguity.

## The 6 Buckets

Every one of the `811` missing verbs should first be assigned to one of these buckets.

### Bucket A: Pure CRUD

Definition:

- verb has direct `crud` mapping in YAML
- reads/writes can be derived from CRUD config and lookup args

Treatment:

- auto-generate footprint
- mark `footprint_mode = derived_crud`

Expected yield:

- high

### Bucket B: Lifecycle/State Verbs

Definition:

- verb is stateful
- lifecycle preconditions or transitions are defined in YAML/state machine

Treatment:

- auto-generate:
  - `preconditions`
  - `postconditions`
  - `node_state_gates`
- attach known reads/writes only where derivable
- mark `footprint_mode = derived_lifecycle`

Expected yield:

- medium

### Bucket C: Plugin Business Verbs

Definition:

- verb uses Rust handler/repository logic
- touches business tables indirectly

Treatment:

- inspect handler + repository + SQLx
- materialize explicit reads/writes/pre/post
- mark `footprint_mode = explicit`

Expected yield:

- medium, but highest-value work

### Bucket D: Delegating/Composite Verbs

Definition:

- verb mostly orchestrates other verbs
- may not touch tables directly

Treatment:

- mark `footprint_mode = delegated`
- record downstream verbs where possible
- only add direct reads/writes if the handler itself does them

Expected yield:

- medium

### Bucket E: No-Harm Read/Show/Research Verbs

Definition:

- Sage-side “show me”, “read”, “inspect”, “research”, “diff”, “describe”
- typically non-mutating

Treatment:

- do not leave blank
- mark as:
  - `read_only`
  - `system_only`
  - `none`
depending on actual behavior

Expected yield:

- high

### Bucket F: Session/UI/System Verbs

Definition:

- session, view, registry, graph, admin, agent-control

Treatment:

- explicit classification
- no business-table writes unless proven otherwise
- mark `system_only` or `none`

Expected yield:

- high

## Burn-Down Order

Do not attack this alphabetically.

Burn it down in this order:

1. classify all `811`
2. auto-fill Buckets A, B, E, F
3. manual backfill Bucket C
4. manual delegated modeling for Bucket D

That sequence should collapse the unresolved set fast.

## Concrete Execution Slices

### S6.1 Classification Pass

Goal:

- assign every missing verb to one of Buckets A-F

Inputs:

- live verb inventory
- YAML verb config
- behavior type
- CRUD presence
- harm class
- lifecycle metadata
- workspace/domain taxonomy

Deliverables:

- `artifacts/footprints/s6_missing_verb_classification.json`
- per-bucket counts

Acceptance:

- all `811` assigned
- zero unclassified verbs

### S6.2 Auto-Derive CRUD Backfill

Goal:

- generate entries for Bucket A

Rules:

- `reads` from lookup tables + target table where needed
- `writes` from CRUD target tables
- `entity_scope` from produced/consumed/lookup entity types
- `preconditions` from required args + lifecycle requirements
- `postconditions` from create/update/delete operation semantics

Deliverables:

- generated footprint patch set
- `artifacts/footprints/s6_crud_backfill_report.json`

Acceptance:

- all Bucket A verbs have explicit rows

### S6.3 Auto-Derive Lifecycle Backfill

Goal:

- generate pre/post footprint for Bucket B

Rules:

- pull `requires_states` from verb lifecycle
- pull transitions from state machines
- synthesize postconditions from transition edges

Deliverables:

- lifecycle-derived patch set
- `artifacts/footprints/s6_lifecycle_backfill_report.json`

Acceptance:

- all Bucket B verbs have explicit lifecycle footprint

### S6.4 No-Harm/System Normalization

Goal:

- close Buckets E and F so they stop counting as missing

Rules:

- no-harm show/read/research verbs:
  - explicit row required
  - usually `writes = []`
- session/view/agent/admin verbs:
  - explicit row required
  - usually `system_only` or `none`

Deliverables:

- `artifacts/footprints/s6_non_mutating_normalization_report.json`

Acceptance:

- no-harm and system verbs no longer appear in missing-footprint counts

### S6.5 Plugin Business Backfill

Goal:

- resolve Bucket C by domain batches

Priority domains:

1. `deal`
2. `cbu`
3. `kyc-case`
4. `screening`
5. `document`
6. `trading-profile`
7. `service-resource`
8. `onboarding`

Method:

- inspect Rust handler
- inspect repository/SQLx calls
- extract direct reads/writes
- extract guard logic as preconditions
- extract state/result semantics as postconditions

Deliverables per batch:

- updated registry rows
- `artifacts/footprints/s6_plugin_batch_<n>.json`

Acceptance:

- each batch closes all targeted verbs

### S6.6 Delegated/Composite Verbs

Goal:

- normalize Bucket D

Method:

- mark direct footprint if any
- record delegated downstream verbs where meaningful
- do not invent fake table writes

Deliverables:

- `artifacts/footprints/s6_delegated_report.json`

Acceptance:

- all composite verbs explicitly marked as delegated or explicit

### S6.7 Final Burn-Down Proof

Goal:

- prove the missing count is reduced to zero or to an explicitly justified residue

Deliverables:

- `artifacts/footprints/s6_final_burndown_report.json`

Acceptance:

- no business verb remains silently missing
- any remaining `none/system_only` verbs are intentionally classified

## Authoring Pattern

Do not keep scaling a single huge hand-maintained block in [domain_metadata.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/sem_os_seeds/domain_metadata.yaml).

Preferred pattern:

1. generate machine-readable candidate rows under `artifacts/footprints/generated/`
2. review by domain
3. merge approved rows into the registry overlay

If the monolith becomes unmanageable, split the footprint overlay by domain later.

## Suggested Mechanical Rules

### Derive `entity_scope`

From:

- `produces`
- `consumes`
- arg lookup entity types
- lifecycle `entity_arg`
- domain fallback only as last resort

### Derive `reads`

From:

- lookup tables
- CRUD target table
- join/base/extension tables
- SQLx query tables in handlers

### Derive `writes`

From:

- insert/update/delete CRUD targets
- SQLx mutation queries in handlers
- explicit event/audit table writes

### Derive `preconditions`

From:

- lifecycle `requires_states`
- precondition checks
- required args with entity lookups
- deal/KYC/business rule guards in Rust handlers

### Derive `postconditions`

From:

- state machine transitions
- emitted status changes
- created/deleted/linked records

## What Counts As Done

`S6` is done when:

- the missing-footprint count is no longer inflated by no-harm/system verbs
- every business verb has an explicit footprint row
- every non-business verb has an explicit non-business classification row
- SemOS can explain both:
  - `why this verb is relevant here`
  - `what this verb touches and requires`

## Practical Success Metric

The real burn-down target is:

- `811 missing` -> `0 silent missing`

That is better than pretending all `811` should become normal business table mutations.

Some should end as:

- `read_only`
- `delegated`
- `system_only`
- `none`

But none should remain implicit.

## Recommended Next Implementation Step

Start with `S6.1` only:

- classify all `811`
- produce bucket counts
- identify the fast-close share from Buckets `A`, `B`, `E`, and `F`

That will show how much of the burn-down can be closed mechanically before touching plugin handlers.

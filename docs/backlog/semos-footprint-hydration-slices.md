# TODO: SemOS Footprint Hydration Implementation Plan In Slices

## Purpose

This document converts the SemOS footprint hydration program into bounded execution slices suitable for Codex `GPT-5.4` in `medium` reasoning mode.

The goal is to make the work executable in safe increments with explicit stop gates, validation commands, and review points.

This plan is a prerequisite for the later session-scoped navigation refactor. The target resolution model remains:

`client_group + workspace + constellation + subject + node_state -> verb set`

That model must not be implemented until the footprint layer is hydrated and verified.

## Execution Rules

These rules apply to every slice:

- do one slice at a time
- do not skip ahead
- do not auto-commit
- stop after validation and report results
- if counts do not reconcile, stop and report instead of guessing
- generate machine-readable artifacts under `artifacts/footprints/`
- treat Rust, YAML contracts, state machines, constellation maps, workflows, and packs as sources of truth
- do not mark a slice complete unless all acceptance criteria for that slice are met

## Global Constraints

- `workspace_affinity` must not silently default to `*` except for explicitly approved global verbs
- invocation phrase collisions must be classified as `fatal` or `scoped_allowed`
- `node_state_gates` must use a controlled vocabulary from a shared registry
- baseline counts must reconcile exactly before cleanup or backfill starts

## Additional Invariants

- footprint hydration must consume both code and config sources; Rust handlers alone are not sufficient
- every generated report must include `slice_id`, `generated_at`, and `repo_revision`
- no slice may claim `100%` coverage if unresolved exceptions are hidden inside `*`
- every `scoped_allowed` collision must name the scoping dimension that makes it safe
- every `node_state_gates` value must be drawn from the approved registry, never invented inline

## Controlled Taxonomies

The final hydration must support these workspace values:

- `ProductMaintenance`
- `Deal`
- `CBU`
- `KYC`
- `InstrumentMatrix`
- `OnBoarding`
- `*`

The `*` value is reserved for:

- `session.*`
- `admin.*`
- `system.*`
- explicitly approved cross-workspace verbs

### Collision Policy

Use this policy in `S2` and later validation:

- `fatal`
  - the same invocation phrase resolves to multiple live verbs with no stable disambiguating dimension
  - the same invocation phrase collides within the same workspace and same subject scope
- `scoped_allowed`
  - phrase overlap is tolerated because the verbs separate cleanly by workspace, subject kind, or constellation family
  - the collision report must record the dimension that makes the overlap safe

### Report Shape

Every generated JSON artifact should include, where applicable:

- `slice_id`
- `generated_at`
- `repo_revision`
- `total_live_verbs`
- `fully_hydrated_verbs`
- `partially_hydrated_verbs`
- `missing_verbs`
- `orphans`
- `exceptions`

## Slice Overview

| Slice | Goal | Human Gate |
|---|---|---|
| S0 | baseline reconciliation | required |
| S1 | schema and validation tooling | optional |
| S2 | orphan, macro, and collision cleanup | required |
| S3 | taxonomy definition | required |
| S4 | workspace affinity stamping | required |
| S5A | CBU workspace family/subject hydration | required |
| S5B | KYC workspace family/subject hydration | required |
| S5C | Deal workspace family/subject hydration | required |
| S5D | InstrumentMatrix workspace family/subject hydration | required |
| S5E | OnBoarding workspace family/subject hydration | required |
| S6A-S6G | original footprint backfill in repeated domain batches | required per batch |
| S7 | node state gates | required |
| S8 | resolution cascade proof | required |

## Slice File Boundaries

These are the default write scopes unless a slice says otherwise:

- `S0`
  - `scripts/`
  - `artifacts/footprints/`
- `S1`
  - `scripts/`
  - footprint schema/config files
  - `artifacts/footprints/`
- `S2`
  - footprint metadata/config
  - macro correction config
  - lexical concept config
  - `artifacts/footprints/`
- `S3`
  - taxonomy YAML files
  - validator support
  - `artifacts/footprints/`
- `S4-S7`
  - footprint metadata/config
  - relevant helper scripts
  - `artifacts/footprints/`
- `S8`
  - test scripts
  - fixture files
  - `artifacts/footprints/`

If a slice needs to go outside these boundaries, it must stop and ask for review first.

---

## Slice S0: Baseline Reconciliation

### Objective

Establish the exact live verb inventory and reconcile all current footprint counts before any schema or hydration work begins.

### Allowed Changes

- reporting scripts
- inventory scripts
- artifact outputs

### Deliverables

- `artifacts/footprints/live_verb_inventory.json`
- `artifacts/footprints/baseline_coverage.json`
- `artifacts/footprints/orphan_footprints.json`
- `artifacts/footprints/collision_report.json`
- `artifacts/footprints/macro_ref_breakage.json`
- `artifacts/footprints/phase_s0_summary.json`

### Tasks

1. enumerate all live effective verbs
2. enumerate all existing footprint entries
3. compute:
   - total live verbs
   - fully hydrated verbs
   - partially hydrated verbs
   - missing footprints
   - orphan footprints
4. enumerate invocation phrase collisions
5. enumerate broken macro-to-verb references
6. reconcile all numbers exactly
7. identify any mismatch between:
   - live registry verbs
   - footprint-bearing verbs
   - lexical concept entries
   - macro-referenced verbs

### Acceptance

- all counts reconcile exactly
- any mismatch between headline counts and computed counts is explicitly surfaced
- the baseline summary names every unresolved discrepancy explicitly

### Validation

- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop after reporting the reconciled baseline. Do not modify schema or registry contents in this slice.

---

## Slice S1: Schema And Validator Tooling

### Objective

Extend the footprint schema and add validation/reporting harnesses without changing footprint content yet.

### Allowed Changes

- footprint schema
- validation scripts
- coverage report scripts
- supporting docs if strictly necessary

### Deliverables

- schema extended with:
  - `workspace_affinity`
  - `constellation_families`
  - `subject_kinds`
  - `node_state_gates`
- `scripts/validate_footprints.sh`
- `scripts/footprint_coverage_report.sh`
- `artifacts/footprints/phase_s1_validation.json`
- `artifacts/footprints/phase_s1_coverage.json`
- `artifacts/footprints/phase_s1_schema_summary.json`

### Tasks

1. extend the footprint structure
2. make `workspace_affinity` required
3. allow empty arrays for:
   - `constellation_families`
   - `subject_kinds`
   - `node_state_gates`
4. implement validator checks for:
   - live verb references
   - missing required fields
   - orphan footprint entries
   - unknown workspaces
   - unknown node state values
   - invalid `*` usage outside the approved allowlist
   - collision report category validity
5. implement coverage reporting with:
   - overall coverage
   - per-domain coverage
   - per-workspace coverage where derivable
   - split of `full`, `partial`, `missing`, and `global_star`

### Acceptance

- validator runs successfully on the current repo
- coverage report emits baseline percentages

### Validation

- run validator
- run coverage report
- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop after the tooling works. Do not start cleanup or tagging in this slice.

---

## Slice S2: Surface Cleanup

### Objective

Clean the existing footprint and lexical surface before bulk hydration.

### Allowed Changes

- footprint entries
- macro correction config
- invocation phrase config
- `verb_concepts.yaml`

### Deliverables

- cleaned registry content
- `artifacts/footprints/phase_s2_cleanup_report.json`
- `artifacts/footprints/phase_s2_collision_classification.json`

### Tasks

1. remove or remap orphan footprint entries
2. apply known macro-to-verb corrections
3. resolve invocation phrase collisions
4. classify collisions as:
   - `fatal`
   - `scoped_allowed`
5. reconcile lexical concept drift against the live verb registry
6. emit a preserved list of all allowed scoped collisions with justification

### Acceptance

- zero orphan footprint entries
- zero broken macro references
- zero fatal invocation phrase collisions
- every retained collision has an explicit scoping reason

### Validation

- run validator
- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop with the cleanup report and list any intentionally retained `scoped_allowed` collisions.

---

## Slice S3: Taxonomy Definition

### Objective

Define the controlled vocabularies before any mass stamping.

### Allowed Changes

- taxonomy YAML files
- validation/reporting support for the new vocabularies

### Deliverables

- `domain_to_workspace_map.yaml`
- `workspace_to_constellation_families.yaml`
- `workspace_to_subject_kinds.yaml`
- `node_state_registry.yaml`
- `artifacts/footprints/phase_s3_taxonomy_review.json`

### Tasks

1. map each domain to one or more workspaces
2. define allowed constellation-family vocabulary by workspace
3. define allowed subject-kind vocabulary by workspace
4. define allowed node-state vocabulary
5. ensure validator can read and enforce these vocabularies
6. define the explicit allowlist for verbs permitted to use `workspace_affinity: "*"`

### Acceptance

- all four taxonomy files parse cleanly
- validator recognizes them
- review summary is ready for signoff
- the `*` allowlist is explicit and reviewable

### Validation

- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop for human review. Do not stamp footprints until these taxonomies are approved.

---

## Slice S4: Workspace Affinity Stamping

### Objective

Assign `workspace_affinity` to every live verb.

### Allowed Changes

- footprint entries
- mapping application scripts

### Deliverables

- updated footprints with `workspace_affinity`
- `artifacts/footprints/phase_s4_workspace_affinity_report.json`
- `artifacts/footprints/phase_s4_star_allowlist_exceptions.json`

### Tasks

1. apply the approved domain-to-workspace map
2. review and correct heuristic exceptions
3. restrict `*` to approved global/cross-workspace verbs only
4. emit an exception report for every `*` usage
5. emit a per-domain summary of single-workspace vs multi-workspace assignment

### Acceptance

- 100% of live verbs have explicit `workspace_affinity`
- all `*` assignments are justified
- no non-allowlisted verb uses `*`

### Validation

- run validator
- run coverage report
- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop and present the exception report for review.

---

## Slice S5A: CBU Workspace Family And Subject Hydration

### Objective

Hydrate `constellation_families` and `subject_kinds` for all `CBU`-affinity verbs.

### Deliverables

- updated footprints for `CBU`
- `artifacts/footprints/phase_s5a_cbu_report.json`

### Tasks

1. list all `CBU`-affinity verbs
2. assign at least one `constellation_family`
3. assign at least one `subject_kind`
4. validate workspace coverage

### Acceptance

- 100% of `CBU` verbs have both fields populated

### Validation

- run validator
- run coverage report
- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop after `CBU` only.

---

## Slice S5B: KYC Workspace Family And Subject Hydration

Same structure as `S5A`, but for `KYC`.

### Deliverables

- `artifacts/footprints/phase_s5b_kyc_report.json`

### Stop Gate

Stop after `KYC` only.

---

## Slice S5C: Deal Workspace Family And Subject Hydration

Same structure as `S5A`, but for `Deal`.

### Deliverables

- `artifacts/footprints/phase_s5c_deal_report.json`

### Stop Gate

Stop after `Deal` only.

---

## Slice S5D: InstrumentMatrix Workspace Family And Subject Hydration

Same structure as `S5A`, but for `InstrumentMatrix`.

### Deliverables

- `artifacts/footprints/phase_s5d_instrument_matrix_report.json`

### Stop Gate

Stop after `InstrumentMatrix` only.

---

## Slice S5E: OnBoarding Workspace Family And Subject Hydration

Same structure as `S5A`, but for `OnBoarding`.

### Deliverables

- `artifacts/footprints/phase_s5e_onboarding_report.json`

### Stop Gate

Stop after `OnBoarding` only.

---

## Slice S6: Original Footprint Backfill

### Objective

Backfill the original missing fields:

- `entity_scope`
- `data_reads`
- `data_writes`
- `preconditions`
- `postconditions`

### Execution Pattern

Run this slice repeatedly in domain batches.

### Batch Size

- 15 to 20 domains per batch

### Batch Identifiers

Use stable identifiers:

- `S6A`
- `S6B`
- `S6C`
- `S6D`
- `S6E`
- `S6F`
- `S6G`

If more than seven batches are needed, stop and revise the slicing plan instead of inventing `S6H+` ad hoc.

### Allowed Changes

- footprint entries
- extraction scripts
- reporting artifacts

### Deliverables Per Batch

- updated footprints for the selected domain batch
- `artifacts/footprints/phase_s6_batch_<n>_report.json`
- `artifacts/footprints/phase_s6_batch_<n>_domains.json`

### Source Inputs

Use all relevant sources:

- Rust handlers
- SQLx queries
- YAML verb contracts
- state machines
- constellation maps
- workflow and pack definitions where relevant

### Tasks

1. choose one domain batch
2. inspect implementation and config sources
3. extract `entity_scope`
4. extract `data_reads`
5. extract `data_writes`
6. extract `preconditions`
7. extract `postconditions`
8. update footprints
9. validate and report
10. record any non-mechanical judgment call in the batch report

### Acceptance

- validator passes
- coverage increases for the selected batch
- no guessed fields are left undocumented
- every ambiguous inference is called out explicitly in the report

### Validation

- run validator
- run coverage report
- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop after one batch. Report:

- domains covered
- verbs updated
- unresolved ambiguities
- remaining uncovered count

---

## Slice S7: Node State Gates

### Objective

Hydrate `node_state_gates` for state-sensitive verbs only.

### Allowed Changes

- footprint entries
- state registry corrections if necessary

### Deliverables

- updated state-sensitive footprints
- `artifacts/footprints/phase_s7_state_gate_report.json`

### Tasks

1. identify state-sensitive verbs using:
   - lifecycle preconditions
   - explicit transition logic
   - state machine semantics
2. assign `node_state_gates`
3. leave non-state-sensitive verbs empty by policy
4. record any verb whose gate depends on unresolved constellation design as `ambiguous`, not guessed

### Acceptance

- every state-sensitive verb has explicit `node_state_gates`
- all gate values come from `node_state_registry.yaml`
- ambiguous verbs are isolated into a review list, not silently completed

### Validation

- run validator
- run coverage report
- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop with an ambiguity list for any verbs whose state gating is not mechanically derivable.

---

## Slice S8: Resolution Cascade Proof

### Objective

Prove that the hydrated footprint layer supports the intended 5D resolution cascade before the session-scoped navigation refactor begins.

### Allowed Changes

- test scripts
- fixture definitions
- reporting artifacts

### Deliverables

- `scripts/test_verb_resolution_cascade.sh`
- `artifacts/footprints/phase_s8_cascade_results.json`
- `artifacts/footprints/phase_s8_unreachable_verbs.json`

### Required Test Cases

1. 5D exact lookup
2. 4D fallback without node state
3. 3D fallback without subject
4. 2D fallback without constellation
5. 1D legacy fallback

### Assertions

- higher-dimensional lookup narrows or equals lower-dimensional lookup
- no live verb is unreachable
- the widest case meets the target latency
- result sets are deterministic for identical inputs

### Acceptance

- all tests pass
- monotonic narrowing holds
- no unreachable verbs remain

### Validation

- run validator
- run cascade test harness
- `env RUSTC_WRAPPER= cargo check`

### Stop Gate

Stop here. Do not begin the session-scoped navigation refactor in the same session.

---

## Human Review Gates

Require explicit review before proceeding after:

- `S0`
- `S3`
- `S4`
- each workspace slice in `S5`
- every batch in `S6`
- `S7`
- `S8`

## Standard Codex Prompt Pattern

Each Codex session prompt should include:

- exact slice identifier
- objective
- files allowed to edit
- required outputs
- validation commands
- explicit stop condition
- explicit forbidden shortcuts

## Required Prompt Footer

Every slice prompt should end with:

- “Do not start the next slice.”
- “If counts do not reconcile, stop and report.”
- “If you need to widen file scope, stop and ask.”
- “Run `env RUSTC_WRAPPER= cargo check` before closing the slice.”

## Example Prompt

“Implement slice `S0` only for SemOS footprint hydration. Use GPT-5.4 in medium reasoning mode. Reconcile the live verb inventory and current footprint coverage exactly. You may edit only reporting or inventory scripts and generate artifacts under `artifacts/footprints/`. Run `env RUSTC_WRAPPER= cargo check` after edits. Stop after `S0` and report reconciled counts, mismatches, and blockers. Do not modify footprint schema, registry content, or hydration metadata in this slice.”

## Recommended Next Step

Start with `S0` and do not authorize any cleanup or backfill work until the baseline numbers are mathematically reconciled.

# Sem OS Single Pipeline Implementation Plan

## Objective

Make Sem OS the sole source of truth for grounded agent action discovery:

`utterance -> semantic intent discovery -> canonical action registry -> constellation binding -> populated entity instance graph -> state-machine evaluation -> valid DSL node verb set -> REPL DSL render`

The acceptance test is:

`constellation + state machine + populated entity instance graph => node-valid DSL options`

This plan does not change the intended architecture. It completes and aligns the implementation to the published architecture and intended Sem OS solution shape.

## Status Update (2026-04-16)

The core single-pipeline direction in this document remains correct, but the surrounding capability boundary is now tighter than when this plan was drafted:

- the standalone Sem OS server contract is intentionally the crate-root embedding surface (`build_router`, `JwtConfig`, `OutboxDispatcher`)
- internal handler/error module trees are no longer treated as public API
- test/harness support is being moved behind `sem_os_harness` and crate-local tests rather than broad production visibility
- dormant `/tools/*` server endpoints were removed because they are not part of the live pipeline contract

The main remaining work around this plan is no longer basic plumbing exposure; it is test-boundary cleanup and adapter-facade cleanup where `ob-poc` still reaches too deeply into Sem OS family crates.

## Principles

- Sem OS owns grounded action truth.
- Sage research mode consumes Sem OS outputs only.
- REPL DSL rendering consumes Sem OS outputs only.
- Parallel action-discovery paths are not kept as fallbacks once Sem OS owns a stage.
- Dead execution paths are removed, not left dormant.

## Canonical Sem OS Contract

`resolve_context` becomes the single agent-facing entrypoint.

It must eventually return:

- `resolved_subject`
- `resolved_constellation`
- `resolved_slot_path`
- `resolved_node_id`
- `resolved_state_machine`
- `current_state`
- `valid_actions`
- `blocked_actions`
- `dsl_candidates`
- `clarifications`
- `evidence`
- `confidence`

The additive grounding types were introduced in:

- [context_resolution.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/sem_os_core/src/context_resolution.rs)

## Execution Order

### 1. Lock The Sem OS Response Contract

Tasks:

- Extend `ContextResolutionResponse` with grounded slot/node/action fields.
- Keep the existing view/verb/attribute evidence fields during migration.
- Make the Sem OS response the only contract Sage and REPL target.

Deliverable:

- One canonical Sem OS action-surface contract.

### 2. Extend Sem OS Seeds To Full Pipeline Coverage

Add registry support for:

- primitive verb contracts
- macro defs
- constellation maps
- reducer state machines
- optional stategraphs if retained as an explanatory layer

Tasks:

- Extend `SeedBundle`.
- Add adapter scanners for macro YAML, constellation YAML, and state machine YAML.
- Bootstrap these into Sem OS snapshots.

Deliverable:

- Sem OS snapshots can represent the full pipeline, not only primitive verbs and schema artifacts.

### 3. Attach Missing Slot State Models

Priority fix:

- bind `case` slots to `kyc_case_lifecycle`

Tasks:

- audit all constellation slots
- classify each slot as:
  - reducer-backed
  - stateless/read-only
  - dependency-only transitional
  - incomplete
- eliminate accidental state-model gaps for actionable slots

Deliverable:

- every actionable slot has an explicit state model or explicit stateless declaration

### 4. Port Grounded Runtime Into Sem OS Core

Current grounded logic lives in legacy `sem_reg`.

Source modules to migrate:

- `rust/src/sem_reg/reducer/*`
- `rust/src/sem_reg/constellation/*`
- selected discovery/stategraph logic where it contributes to the grounded action surface

Tasks:

- move reducer evaluation into `sem_os_core`
- move constellation binding/action-surface logic into `sem_os_core`
- expose the migrated path only through `resolve_context`

Deliverable:

- `resolve_context` computes grounded slot/node state and valid actions instead of returning `MigrationPending`

### 5. Replace Heuristic Preconditions

Tasks:

- remove placeholder `preconditions_met: true`
- evaluate preconditions against:
  - resolved slot state
  - dependency states
  - overlay conditions
  - optional graph node conditions

Deliverable:

- `valid_actions` are truly executable
- `blocked_actions` carry concrete reasons

### 6. Fold Discovery And Graph Walk Into One Resolver

Tasks:

- make `discovery.valid-transitions` a view over canonical Sem OS resolution
- make `discovery.graph-walk` either:
  - part of the same resolver, or
  - an explanatory overlay only

Rule:

- there must be only one source of valid next action truth

Deliverable:

- no competing runtime path computes “what can I do next?”

### 7. Cut Sage Research Mode Over

Tasks:

- replace any Sage-side action discovery from raw YAML
- replace any old monolith-only grounded action suggestions
- make Sage consume only Sem OS `resolve_context`

Deliverable:

- Sage cannot hallucinate actions from stale or parallel registries

### 8. Cut REPL DSL Rendering Over

Tasks:

- make REPL accept only Sem OS-resolved actions
- render DSL from canonical action records and bindings
- stop REPL-side macro or verb inference

Deliverable:

- all emitted DSL has Sem OS grounding lineage

### 9. Remove Hardcoded Constellation Assumptions

Tasks:

- remove hardcoded constellation names from reducer evaluation
- make all reducer evaluation driven by resolved constellation context

Deliverable:

- grounded action surfaces work for all supported constellations, not one implicit default

### 10. Remove Declarative Noise

Tasks:

- either implement or remove inert declarations such as `bulk_macros` entries that have no executable effect
- fail validation on unsupported declarations instead of carrying them silently

Deliverable:

- config only expresses executable truth

### 11. Delete Parallel Runtime Entry Points

Tasks:

- remove old grounded action-computation entrypoints after Sem OS parity
- unregister legacy agent-facing operations that bypass Sem OS
- remove duplicate inventories that ignore macros/constellations/state models

Deliverable:

- only Sem OS can compute a valid action surface

### 12. Add Regression Gates

Tasks:

- validate that every actionable slot has a state model
- validate that all macro expansion targets resolve to canonical actions
- validate that all state-machine transition verbs resolve to canonical actions
- validate that generated inventories are built from Sem OS snapshots, not raw config

Deliverable:

- drift becomes a build or test failure

## Dead Code And Execution Paths To Remove

These must be removed after the Sem OS replacements land. Leaving them in place risks future re-use and hallucinated action discovery.

### Remove Entirely

- Sage-side raw verb or macro discovery used for actionable suggestions
- REPL-side action inference
- heuristic valid-transition computations not backed by Sem OS grounded state
- old grounded action-surface entrypoints in legacy `sem_reg` once ported
- hardcoded default-constellation shortcuts
- duplicate inventory/report generators that do not include macros, constellation maps, and state machines
- inert config concepts that never affect execution

### Disable Early, Delete After Cutover

- legacy discovery ops that independently compute valid actions
- agent-facing routes wired to old grounded action pipelines
- commented or alternative runtime entrypoints that can be accidentally reactivated

## Migration Checkpoints

### Checkpoint A

- Sem OS contract extended
- no consumer cutover yet

### Checkpoint B

- Sem OS registry stores macros, constellation maps, and state machines
- old runtime still computes grounded actions

### Checkpoint C

- Sem OS computes grounded action surfaces
- Sage switched to Sem OS

### Checkpoint D

- REPL switched to Sem OS
- old runtime paths disabled

### Checkpoint E

- old runtime paths deleted
- regression gates active

## Definition Of Done

The migration is complete only when all are true:

- `resolve_context` is the sole grounded action-discovery entrypoint
- Sem OS snapshots store the full pipeline definition surface
- Sage consumes only Sem OS grounded actions
- REPL consumes only Sem OS grounded actions
- valid next actions come from constellation/state-machine/entity-graph evaluation
- no legacy or heuristic runtime path can compute actionable options independently
- dead code and parallel paths have been removed

## Acceptance Tests

### Core Grounding Test

Given:

- a constellation
- a bound slot or node
- a populated entity instance graph
- a state machine

Sem OS must return:

- the effective state
- the valid primitive and macro actions
- blocked actions with reasons
- deterministic DSL candidates

### Agent Safety Test

Given an utterance in Sage research mode:

- Sage must not invent actions from raw YAML or parallel registries
- all suggested actions must be present in Sem OS grounded action output

### REPL Safety Test

Given a rendered DSL statement:

- there must be a Sem OS action-surface record from which it was produced

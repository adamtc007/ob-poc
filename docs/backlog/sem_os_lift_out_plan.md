# SemOS Lift-Out Plan

## Purpose

This document proposes a review-first plan to lift migrated `CustomOperation` implementations out of `ob-poc` and into SemOS-owned crates.

It is intentionally a planning document only. It does not authorize code movement by itself.

## Problem Statement

The `execute_json(args, VerbExecutionContext)` migration is now complete in `rust/src/domain_ops`, but the implementations still live inside the `ob-poc` crate.

That leaves an important architectural coupling in place:

- SemOS execution contracts exist
- SemOS runtime can invoke the operations through a unified interface
- but `ob-poc` still owns the actual operation implementations

Until that ownership is inverted, SemOS remains operationally dependent on `ob-poc` domain code.

## Current State

- `rust/src/domain_ops` contains `625` `CustomOperation` impls
- each now has an explicit `execute_json(...)`
- the same ops still expose legacy `execute(...)`
- op registration still flows through `ob-poc`
- many ops still import `ob-poc` repositories, services, result types, and runtime helpers directly

This means the codebase is contract-aligned but not yet decomposed.

## Desired End State

The target architecture is:

- `sem_os_core`
  - execution traits
  - execution outcome types
  - domain-neutral contracts and shared models
- `sem_os_postgres`
  - generic persistence helpers
  - generic CRUD/materialization execution infrastructure
- SemOS plugin crates
  - actual operation implementations, grouped by bounded domain
  - registration surfaces for those operations
  - any domain-local helpers that are not app-specific
- `ob-poc`
  - app composition root
  - web/API/runtime wiring
  - app-specific repository and service implementations
  - adapters that satisfy SemOS-facing traits where required

In the target state:

- SemOS-owned operations are not implemented in `ob-poc`
- `ob-poc` depends on plugin crates for operation behavior
- plugin crates do not depend on `ob-poc` internals
- the legacy `execute(...)` path can eventually be removed

## Non-Goals

This plan does not propose:

- redesigning the execution trait surface again
- changing operation behavior while moving ownership
- broad SQL or schema redesign
- collapsing all ops into one giant SemOS crate
- forcing obviously app-specific glue out of `ob-poc`

## Core Architectural Position

The coupling problem is real.

The `execute_json(...)` migration solved invocation unification, but not crate-level ownership. SemOS still reaches real business behavior through `ob-poc` code. That is better than the legacy split contract, but it is not a proper separation of concerns.

The lift-out should therefore optimize for:

- ownership separation
- explicit dependency inversion
- bounded extraction by domain
- preservation of runtime behavior

## Extraction Principles

1. Move ownership by domain, not by scattered files.
2. Extract interfaces before extracting implementations.
3. Keep app-specific adapters in `ob-poc`.
4. Do not move code that is obviously composition glue.
5. Prefer several small plugin crates over one large catch-all crate.
6. Preserve behavior first; simplify after extraction.

## Domain Classification Model

Before moving code, every `domain_ops` file should be classified into one of three buckets.

### Bucket A: SemOS-Owned

These ops belong in SemOS plugin crates because they are fundamentally domain/runtime capabilities rather than app glue.

Typical examples:

- schema and discovery/introspection ops
- navigation/view/session ops if they can be expressed against stable interfaces
- document/evidence/request style workflow surfaces
- shared atom/state/remediation style infrastructure-facing ops

### Bucket B: SemOS Plugin With App Adapter

These ops should move out, but only after introducing traits for dependencies that currently live in `ob-poc`.

Typical examples:

- booking/trading ops using app repositories
- agent ops using local control surfaces
- ownership or screening ops that call `ob-poc` services directly

### Bucket C: App Glue

These should stay in `ob-poc` unless a later architectural reason appears.

Typical examples:

- ops that are thin wrappers over app-only workflows
- ops tightly coupled to UI/session conventions that are not yet generalized
- ops whose semantics are not meaningfully reusable outside `ob-poc`

## Proposed Crate Topology

Initial proposal:

- `sem_os_plugin_schema`
- `sem_os_plugin_view`
- `sem_os_plugin_session`
- `sem_os_plugin_documents`
- `sem_os_plugin_requests`
- `sem_os_plugin_evidence`
- `sem_os_plugin_state`
- `sem_os_plugin_trading`
- `sem_os_plugin_booking_principal`
- `sem_os_plugin_agent`

This is a planning set, not a final list. Some may merge. Some may remain in `ob-poc`.

## Dependency Inversion Strategy

The main blocker to lift-out is direct import dependence on `ob-poc`.

That should be resolved as follows:

### What moves first

- operation-local result structs and helper functions that are domain-local
- generic execution adapters that clearly belong to SemOS/plugin infrastructure
- domain models that are reused across multiple extracted ops and are not app-specific

### What stays behind initially

- repositories backed by app-specific schema assumptions
- service implementations tied to app runtime
- web/runtime composition

### How to bridge

For any extracted op that currently depends on `ob-poc` code:

1. define a trait at the plugin boundary
2. depend on that trait inside the plugin crate
3. implement the trait inside `ob-poc`
4. inject the implementation at registration/composition time

This keeps plugin crates reusable while avoiding a hard reverse dependency on `ob-poc`.

## Registration Strategy

Current registration is still centered in `ob-poc/domain_ops`.

Target registration model:

- each plugin crate exposes a registrar or operation bundle
- `ob-poc` imports those registrars during startup
- `ob-poc` assembles the runtime surface explicitly

That turns `ob-poc` into the composition root rather than the owner of all operation code.

## Recommended Execution Order

### Phase 0: Inventory And Ownership Map

Deliverables:

- file-by-file ownership classification for all `domain_ops` files
- candidate plugin crate map
- explicit list of app-only files that should remain in `ob-poc`

No code moves in this phase.

### Phase 1: Low-Coupling Extraction

Start with files that are easiest to externalize and least dependent on app-only services.

Recommended first candidates:

- `sem_os_schema_ops.rs`
- `view_ops.rs`
- `session_ops.rs`
- `source_loader_ops.rs`

Goal:

- prove the extraction pattern
- establish registration shape
- establish trait/adaptor conventions

### Phase 2: Infrastructure And Shared Runtime Domains

Recommended next candidates:

- `state_ops.rs`
- `shared_atom_ops.rs`
- `resource_ops.rs`
- `batch_control_ops.rs`
- `sem_os_maintenance_ops.rs`

Goal:

- move SemOS-adjacent operational domains into plugin crates

### Phase 3: Workflow Domains

Recommended candidates:

- `document_ops.rs`
- `evidence_ops.rs`
- `request_ops.rs`
- `lifecycle_ops.rs`

Goal:

- extract reusable workflow surfaces that can sit behind stable interfaces

### Phase 4: Heavy Adapter Domains

Recommended candidates:

- `booking_principal_ops.rs`
- `trading_profile.rs`
- `trading_profile_ca_ops.rs`
- `agent_ops.rs`

Goal:

- introduce explicit adapter traits
- eliminate direct plugin dependence on app internals

### Phase 5: Final App Boundary Review

At this point, review what remains in `ob-poc/domain_ops`.

Expected output:

- a smaller set of app-owned operations
- a clear rationale for each file that remains
- a deletion plan for extracted file stubs and compatibility shims

## Suggested First Extraction Slice

For peer review, the most defensible first slice is:

- `sem_os_schema_ops.rs`
- `view_ops.rs`
- `session_ops.rs`

Why:

- all now have explicit `execute_json(...)`
- they are structurally similar
- they are good representatives of read-heavy and state-heavy behavior
- they will reveal whether the proposed plugin registration and adapter model is workable

## Risks

### Risk 1: False Reuse

Some ops may look generic but encode `ob-poc` assumptions.

Mitigation:

- classify first
- extract only after dependency review

### Risk 2: Plugin Crate Explosion

Too many tiny crates may become operationally noisy.

Mitigation:

- start with a provisional crate map
- merge adjacent domains when dependency boundaries are weak

### Risk 3: Hidden `ob-poc` Coupling

Some ops may compile-clean in isolation but still depend conceptually on app runtime behavior.

Mitigation:

- define adapter traits explicitly
- require dependency diagrams for each extracted domain

### Risk 4: Registration Drift

During transition, some ops may be registered from both old and new locations.

Mitigation:

- for each extracted domain, switch registration in one step
- delete old registration immediately after the new path is live

## Decision Gates

Before implementation starts, the following should be agreed:

1. Which domains are truly SemOS-owned.
2. Which domains remain app-owned for now.
3. What plugin crate topology is acceptable.
4. What dependency inversion pattern is mandatory.
5. What the first extraction slice will be.

## Review Questions

1. Is the Bucket A / B / C classification model the right ownership test?
2. Are the proposed first extraction targets the right ones?
3. Should session/view/navigation stay together or be separated?
4. Should trading and booking become separate plugin crates or a combined operational crate?
5. Which current `domain_ops` files should explicitly remain in `ob-poc`?

## Definition Of Done

The lift-out is complete only when:

- extracted operation implementations no longer live in `ob-poc`
- registration happens through plugin crates, not `ob-poc/domain_ops`
- plugin crates do not depend on `ob-poc` internals
- app-specific behavior is consumed through explicit traits or left in `ob-poc`
- the compatibility bridge is reduced or removed
- `ob-poc` becomes a consumer/composer of SemOS plugins rather than their code owner

## Recommended Next Step

Peer review this document first.

If approved, the next artifact should be a concrete ownership matrix:

- file
- bucket
- target crate
- blocker dependencies
- extraction order

That should be created before any lift-out code changes begin.

# ADR: Request State vs BPMN Runtime State

**Status:** Proposed  
**Date:** 2026-04-14  
**Decision Type:** Architecture Decision Record

## Context

The current REPL to BPMN integration spans two different concerns:

- `ob-poc` issues a durable orchestration request from a REPL/runbook entry.
- `bpmn-lite` executes that request as a runtime process instance with internal job queue, waits, retries, timers, and incidents.

These are not the same state machine.

The recent session-state and BPMN round-trip work exposed two related risks:

- state ownership is not yet expressed clearly enough between requester and runtime
- shared Postgres-backed tests can be polluted by retained BPMN bridge rows and missing harness isolation

The main architectural question is:

> Should the caller/requester state and BPMN runtime state be represented as one model, or as two coordinated models?

This ADR answers that question and defines the implementation boundary.

## Decision

We will use **two coordinated state models**:

- `ob-poc` owns **request state**
- `bpmn-lite` owns **runtime state**

The sync between them is a **projection boundary**, not shared mutation and not a mirrored status enum.

## Rationale

The requester cares about coarse business progress:

- requested
- queued for dispatch
- running
- returned
- killed
- failed

The BPMN engine cares about execution detail:

- instance started
- service task queued
- service task claimed
- service task completed
- waiting on message
- waiting on timer
- boundary timer fired
- incident created
- terminated
- completed

If these are collapsed into one model:

- ob-poc becomes coupled to engine internals
- queue churn can be mistaken for business state change
- retries become hard to reason about
- request determinism degrades

The correct split is:

- `ob-poc` tracks the lifecycle of the **request**
- `bpmn-lite` tracks the lifecycle of the **execution**

## State Ownership

### ob-poc request state

`ob-poc` is the canonical owner of request identity and request lifecycle.

Canonical request identity:

- `request_key`
- derived from `correlation_key`, or equivalently from `(runbook_id, entry_id)`

Requester-facing lifecycle:

- `requested`
- `dispatch_pending`
- `in_progress`
- `returned`
- `killed`
- `failed`

`ob-poc` does **not** attempt to mirror every BPMN internal transition.

### BPMN runtime state

`bpmn-lite` is the canonical owner of process execution state.

Runtime-owned state includes:

- `ProcessInstance.state`
- fibers
- waits
- timers
- job queue
- incidents
- payload history
- event log

The BPMN job queue is **not** a business request queue. It is an internal runtime queue for work inside an already-created process instance.

## Idempotency Model

Two separate idempotency boundaries are required.

### 1. Request-level idempotency

This prevents duplicate process instances for the same logical request.

Key:

- `request_key`
- typically `correlation_key`
- logically equivalent to `(runbook_id, entry_id)`

Invariant:

> One logical request may have at most one active BPMN instance.

Implications:

- duplicate dispatch retries must not create duplicate active instances
- `bpmn_pending_dispatches` is a retry queue for the same request, not a source of new requests

### 2. Job-level idempotency

This prevents duplicate domain side effects from at-least-once job delivery.

Key:

- `job_key`

Invariant:

> A redelivered job must not reapply business effects twice.

Implications:

- BPMN job queue duplicates are acceptable if job execution is idempotent
- job redelivery must never be interpreted as a new request

## Lifecycle Projection Rules

The requester state is projected from BPMN lifecycle edges only.

### Transition table

| Event | ob-poc request state |
|---|---|
| durable REPL/runbook entry created for BPMN route | `requested` |
| BPMN unavailable, dispatch queued in `bpmn_pending_dispatches` | `dispatch_pending` |
| `StartProcess` accepted and instance created | `in_progress` |
| BPMN `Completed` | `returned` |
| BPMN `Cancelled` or `Terminated` | `killed` |
| BPMN `IncidentCreated` or permanent dispatch failure | `failed` |

Projection boundary components:

- `WorkflowDispatcher`
- `PendingDispatchWorker`
- `EventBridge`
- `SignalRelay`

These are responsible for turning runtime lifecycle edges into requester-state transitions.

## Persistence Model

We will introduce a dedicated requester-side state record rather than overloading correlation rows as the sole source of truth.

Suggested ob-poc request-state fields:

- `request_key`
- `session_id`
- `runbook_id`
- `entry_id`
- `correlation_key`
- `process_key`
- `process_instance_id`
- `status`
- `requested_at`
- `started_at`
- `completed_at`
- `failed_at`
- `killed_at`
- `last_error`

This record is the coarse request-lifecycle projection.

The existing BPMN bridge tables remain integration and audit tables:

- `bpmn_correlations`
- `bpmn_parked_tokens`
- `bpmn_pending_dispatches`
- `bpmn_job_frames`

They are not the canonical requester-state table.

## Cleanup and Retention

The current bridge tables are largely status-retained rather than lifecycle-pruned.

That is acceptable for audit history, but only if active and retained rows are clearly separated.

### Required retention policy

Active rows:

- remain queryable and operational

Terminal rows:

- retained for audit for a bounded TTL, or archived elsewhere

Reaper behavior:

- remove or archive expired terminal rows
- never delete active rows

### Why this matters

Without a retention policy:

- shared test databases get polluted by history
- session deletion leaves unrelated retained bridge rows behind
- operational queries must defensively filter retained rows forever

## Testing Implications

The ignored BPMN E2E harness currently uses a shared Postgres database rather than ephemeral per-test databases.

That means test isolation must be explicit:

- tests must run against a test database
- mutable bridge tables must be cleaned before each run
- concurrent harness runs must be serialized or namespaced

This is a harness requirement, not proof that all retained history is a production bug.

However, the tests also revealed a real production concern:

- BPMN bridge tables currently have no TTL/reaper policy
- and do not have clear ownership-based cleanup on session deletion

Both concerns should be addressed.

## Consequences

### Positive

- clear ownership boundary between requester and runtime
- better deterministic behavior in REPL/runbook execution
- duplicate jobs become manageable without duplicate requests
- BPMN can remain granular without leaking engine detail into ob-poc
- active-state queries become clearer

### Negative

- one additional requester-state table or equivalent repository
- explicit projection logic required at lifecycle boundaries
- retention and cleanup must be designed, not assumed

### Neutral

- BPMN still persists richer internal state than ob-poc
- ob-poc still needs correlation and audit records for bridge observability

## Implementation Plan

### Phase 1: Add requester-state model

- create a dedicated ob-poc request-state table and repository
- use `correlation_key` as canonical request key
- persist requester lifecycle independently from BPMN internal state

### Phase 2: Enforce request-level idempotency

- ensure duplicate durable requests cannot create duplicate active BPMN instances
- apply the guard at dispatch/start boundary

### Phase 3: Keep BPMN runtime granular

- preserve current `bpmn-lite` instance/job/wait/timer/incident model
- document that job queue state is runtime-only, not requester state

### Phase 4: Wire lifecycle projection

- dispatcher writes `requested` or `dispatch_pending`
- successful BPMN start writes `in_progress`
- terminal BPMN events project back to `returned`, `killed`, or `failed`

### Phase 5: Add retention and cleanup policy

- implement TTL/reaper for terminal bridge rows
- add ownership cleanup where safe
- keep active rows untouched

### Phase 6: Harden E2E test isolation

- require test DB by default
- clear mutable bridge rows before harness runs
- serialize shared-DB BPMN E2E tests

## Acceptance Criteria

This ADR is considered implemented when all of the following are true:

1. A single durable REPL/runbook request creates at most one active BPMN instance.
2. Re-dispatch of the same request cannot create a second active instance.
3. BPMN job redelivery does not duplicate side effects.
4. ob-poc request state changes only on defined lifecycle projection edges.
5. BPMN runtime queue state does not directly drive requester state transitions.
6. Terminal bridge rows do not pollute active queries.
7. Shared-DB BPMN E2E tests do not require ad hoc manual cleanup.

## Non-Goals

This ADR does not propose:

- mirroring full BPMN runtime state into ob-poc
- collapsing requester and runtime into one shared status enum
- removing BPMN bridge audit tables
- replacing BPMN job-level idempotency with requester-only idempotency

## Approval Request

Approval is requested for the following architectural position:

> Treat requester state and BPMN runtime state as two different, coordinated state models with separate ownership, separate idempotency boundaries, and explicit lifecycle projection between them.

# Session State Management — Revised Implementation Plan

This document replaces the prior rewrite and reflects the intended scope more
precisely:

1. fix the critical persistence bug in the canonical agent session state
2. add a session stack to BPMN-lite
3. use the same canonical session-stack struct on both the agent side and the
   BPMN-lite side

This is now the working implementation plan for review before code changes.

## Implementation Audit

Status as of 2026-04-14 against the codebase:

### Phase 1

- Task 1.1 `DONE`
  `ReplSessionV2` durable state now includes the critical persisted fields in
  active use, including `bindings`, `cbu_ids`, and `name`.
- Task 1.2 `DONE`
  `SessionRepositoryV2` now round-trips those fields symmetrically through
  `extended_state`.
- Task 1.3 `SUPERSEDED BY EQUIVALENT IMPLEMENTATION`
  The code does not use the proposed `SessionSlot` wrapper, but it does track
  repository versions in `persistence_versions` and updates them after save and
  restore. The acceptance criterion is met even though the exact struct shape is
  different.
- Task 1.4 `SUPERSEDED BY EQUIVALENT IMPLEMENTATION`
  The code does not expose the proposed `get_or_restore_session(...)` helper,
  but `get_session(...)` already restores persisted sessions, rehydrates them,
  and reinserts them into orchestrator memory with the returned version.
- Task 1.5 `PARTIAL`
  Targeted coverage exists for persistence/versioning behavior, but full
  save-load-save coverage is not yet complete in one focused test slice.
- Additional hardening `DONE`
  Initial lazy TTL eviction on access is now implemented in the orchestrator.
  This is intentionally only first-step hardening and not a full bounded-memory
  solution.

### Phase 2

- Task 2.1 `DONE`
  The shared subset is now represented by the canonical `SessionStackState`
  model and its supporting types.
- Task 2.2 `DONE`
  The canonical stack types live in `rust/crates/ob-poc-types`.
- Task 2.3 `DONE`
  `ReplSessionV2` embeds `session_stack: SessionStackState`.
- Task 2.4 `DONE`
  Existing persisted sessions restore through serde-compatible session loading.
- Task 2.5 `DONE`
  The server exposes `GET /api/observatory/session/:id/session-stack-graph`.
- Task 2.6 `DONE`
  The observatory UI exposes the `session_stack` tab and renders the projection.

### Phase 3

- Task 3.1 `DONE`
  BPMN-lite `ProcessInstance` carries `session_stack: SessionStackState`.
- Task 3.2 `DONE`
  BPMN-lite Postgres and memory stores persist the `session_stack`.
- Task 3.3 `DONE`
  BPMN-routed REPL dispatch copies `SessionStackState` by value into BPMN-lite
  only when the verb is routed to BPMN runtime.
- Task 3.4 `DONE`
  `JobActivation` exposes the BPMN-owned `session_stack` directly to the worker.
- Task 3.5 `NOT STARTED`
  There is not yet an implemented BPMN-side mutation/synchronization policy for
  updating `ProcessInstance.session_stack` after deliberate BPMN-side context
  changes.

### Phase 4

- Task 4.1 `DONE`
  End-to-end choreography coverage now exists in the BPMN harness. The
  `e2e_03_full_happy_path_choreography` test proves the process advances from
  dispatch through both service tasks, signals, and terminal completion.
- Task 4.2 `DONE`
  Explicit non-aliasing tests prove copy-by-value ownership on the BPMN-lite
  side for `ProcessInstance.session_stack`, `JobActivation.session_stack`, and
  start-boundary persistence. The ob-poc side persists an independent session
  copy and does not alias BPMN-lite storage.
- Task 4.3 `DONE`
  Runtime verification now covers the return path into ob-poc orchestration.
  The `e2e_08_signal_relay_bounces_to_orchestrator` test proves BPMN terminal
  completion is relayed back to the parked REPL entry and resumes it correctly.

### Delta Review Adjustments

- Dispatcher global latch warning `SUPERSEDED / NOT APPLICABLE`
  The current code already passes session stack per request and does not use the
  old `Mutex<Option<...>>` design.
- Stable bridge DTO requirement `DONE`
  `SessionStackState` is documented as a compact copy-by-value bridge DTO, not a
  mirror of `ReplSessionV2`.
- `UnifiedSession` boundary clarification `DONE`
  The legacy boundary is now explicit in code comments.

## 0. Non-Negotiable End State

Verification status:

- `cargo check` passes for the Rust workspace and BPMN-lite workspace.
- `cargo test --features "database,vnext-repl" --test bpmn_e2e_harness_test e2e_03_full_happy_path_choreography -- --ignored --nocapture` passes.
- `cargo test --features "database,vnext-repl" --test bpmn_e2e_harness_test e2e_08_signal_relay_bounces_to_orchestrator -- --ignored --nocapture` passes.

This implementation is complete only when all of the following are true:

1. an agent/MCP-backed session created and mutated through the unified session
   input surface (`POST /api/session/:id/input`) preserves its canonical session
   state across save/load/restore
2. the UI can open that session and render a DAG visualization of the canonical
   session-stack struct
3. executing a REPL runbook step whose verb routes into BPMN runtime copies that
   same canonical session-stack value into a BPMN-lite process instance
4. BPMN-lite persists its own independent copy on the process instance session
   stack
5. the BPMN worker can observe that BPMN-owned session stack without reaching
   back into ob-poc session persistence

There are no shared-record semantics anywhere in this flow. The boundary is:
same canonical value type, copied by value, stored independently.

The copy trigger is strict:
- session creation does not copy the session stack to BPMN-lite
- UI navigation does not copy the session stack to BPMN-lite
- non-BPMN verbs do not copy the session stack to BPMN-lite
- only REPL runbook execution of a verb that uses BPMN runtime performs the copy

## 1. Direct Answer On Scope

This plan is intended to cover both of the following:

1. the critical session persistence bug in the canonical agent session state
2. the introduction of a BPMN-lite session stack that uses the same canonical
   session-stack struct model as the agent session

Important clarification:

- the shared canonical type should be the session-stack/context model, not the
  entire `ReplSessionV2`
- `ReplSessionV2` currently contains many agent-only fields
  (`messages`, `runbook`, proposal state, pending lookup state, transient pack
  state, and other REPL concerns)
- BPMN-lite should not embed the full `ReplSessionV2`
- instead, the stack/context part of session state must be extracted into a
  shared canonical struct that both `ReplSessionV2` and BPMN-lite use

That is the only implementation shape that satisfies your requirement without
coupling BPMN-lite to the full REPL runtime.

## 2. Critical Findings That Drive The Plan

The current codebase has these relevant issues:

1. `ReplSessionV2` is the canonical agent session state, but some of its durable
   runtime fields are loaded from `extended_state` and not fully written back.
   This is the critical persistence bug.
2. `SessionRepositoryV2::save_session()` expects optimistic-concurrency version
   input, but orchestrator callers currently save with hard-coded `0`.
3. `ReplOrchestratorV2` has no proper restore path from persisted session state
   back into the in-memory map.
4. BPMN-lite has no session stack today. `ProcessInstance` owns payload, flags,
   counters, joins, and lifecycle state, but no workspace/session-stack state.
5. The existing session stack types live under REPL code
   (`WorkspaceFrame`, `WorkspaceKind`, related constraint/view state), which
   means BPMN-lite cannot reuse them cleanly in their current location.

## 3. Architecture Decision

### AD-1. `ReplSessionV2` remains the canonical interactive session

Do not move the full REPL session into BPMN-lite.

`ReplSessionV2` continues to own:
- messages
- runbook
- pending execution state
- agent/repl interaction state
- transient pack/runtime state

### AD-2. Extract a canonical shared session-stack model

Create a shared session-stack/context model that contains only the part of
session state that must exist on both:

- current active workspace
- workspace stack
- scope
- constraint cascade
- subject/workspace context
- trace sequence for continuity

This shared model becomes the single source of truth for stack/context shape.

`ReplSessionV2` will embed it.
`ProcessInstance` will embed it.

Important boundary rule:
- this is pass-by-value, not shared-record state
- ob-poc and BPMN-lite use the same canonical struct definition
- they do not point to the same persisted row or snapshot record
- crossing the boundary creates a copy of the struct value
- each system persists and mutates its own copy independently

### AD-3. Shared session-stack types must move to a shared crate

Because BPMN-lite and `ob-poc` are separate workspaces, the shared type cannot
remain under `rust/src/repl`.

The cleanest location is either:

1. `rust/crates/ob-poc-types`
2. a new small shared crate if `ob-poc-types` becomes too UI/API-oriented

For this implementation, prefer `ob-poc-types` unless review identifies a hard
reason to introduce a new crate.

### AD-4. BPMN-lite owns a persisted session stack, not just a payload envelope

Do not stop at `SessionContextSnapshot` inside the payload only.

The engine must persist session-stack state as part of the BPMN instance model,
so that:
- job activations see the current process session stack
- signals/resume paths preserve it
- crashes and restarts do not lose it

The payload envelope can still exist for dispatch compatibility, but it is not
the long-term source of truth once the BPMN instance is created.

Initialization trigger:
- BPMN-lite session-stack state is initialized only when REPL runbook execution
  invokes a BPMN-routed verb
- sessions that never invoke BPMN-routed verbs never create BPMN-lite
  `session_stack` state

### AD-5. Standalone BPMN-lite means shared type contract, separate persistence

BPMN-lite must remain standalone.

That means:

- it may use the same canonical Rust struct definition or serde schema as
  ob-poc
- it must not depend on ob-poc's persisted session record as its storage source
- it must not hold a live pointer/reference to ob-poc session state
- its persisted `session_stack` on `ProcessInstance` is a copy created at the
  integration boundary

Operationally:

- ob-poc session state is persisted in ob-poc storage
- BPMN-lite process session-stack state is persisted in BPMN-lite storage
- synchronization happens only at explicit integration points
  (dispatch, signal, resume, completion), never by shared-record aliasing

## 4. Canonical Shared Model

The implementation must extract a shared model of this form:

```rust
pub struct SessionStackState {
    pub session_id: Uuid,
    pub scope: Option<SessionScope>,
    pub active_workspace: Option<WorkspaceKind>,
    pub workspace_stack: Vec<WorkspaceFrame>,
    pub trace_sequence: u64,
}
```

With these shared supporting types:

- `SessionScope`
- `WorkspaceKind`
- `SubjectKind`
- `WorkspaceFrame`
- `ConstraintCascadeState`
- `ViewportSnapshot`

The shape requirement is fixed:

- agent and BPMN-lite must use the same canonical stack/context structs
- no duplicate `WorkspaceFrame`-like or `SessionContextSnapshot`-like models
  once the extraction is complete

## 5. Revised Implementation Plan

### Phase 1 — Fix The Critical Agent Session Persistence Bug

Goal:
After Phase 1, the canonical agent session state no longer loses durable state
across checkpoint/save/load cycles.

This is the first and mandatory step before any BPMN-lite stack work.

#### Task 1.1 — Make the agent session persistence bug explicit and complete

Files:
- `rust/src/repl/session_v2.rs`
- `rust/src/repl/session_repository.rs`

Treat this as a session-state bug in `ReplSessionV2`, not just a repository bug.

Current failure mode:
- `load_session()` reads durable state from `extended_state`
- `save_session()` does not write the full corresponding state back
- the session silently loses state on round trip

Minimum durable fields explicitly identified in review:
- `bindings`
- `cbu_ids`
- `name`

Acceptance criteria:
- `ReplSessionV2` save/load preserves these fields exactly
- no silent loss occurs across repeated checkpoint cycles

#### Task 1.2 — Make `extended_state` symmetric with the durable session state

Files:
- `rust/src/repl/session_repository.rs`

Update `save_session()` so every field currently deserialized from
`extended_state` is also serialized into `extended_state`.

At minimum, this must include:
- `bindings`
- `cbu_ids`
- `name`

Acceptance criteria:
- `extended_state` save/load is symmetric for all currently durable fields

#### Task 1.3 — Fix optimistic concurrency in the orchestrator

Files:
- `rust/src/repl/orchestrator_v2.rs`

Replace raw in-memory sessions with a typed slot:

```rust
struct SessionSlot {
    session: ReplSessionV2,
    persistence_version: i64,
}
```

Required behavior:
- new session starts at version `0`
- successful save updates in-memory version
- restored session keeps the version returned by the repository
- no save call passes hard-coded `0` after the first write

Acceptance criteria:
- repeated saves do not fail with stale-version conflicts

#### Task 1.4 — Add restore path from repository into the orchestrator

Files:
- `rust/src/repl/orchestrator_v2.rs`
- `rust/src/api/repl_routes_v2.rs`

Add this typed restore helper:

```rust
pub async fn get_or_restore_session(
    &self,
    session_id: Uuid,
) -> anyhow::Result<Option<ReplSessionV2>>
```

Behavior:
- check in-memory slot map first
- if missing and repository configured, load from DB
- rebuild transient state needed for restored sessions
- store restored session back into the slot map with correct version

Acceptance criteria:
- persisted session can be restored into active orchestrator memory

#### Task 1.5 — Add persistence tests for the agent session fix

Files:
- `rust/src/repl/session_repository.rs`
- `rust/src/repl/orchestrator_v2.rs`

Add tests for:

1. `save -> load` preserves `bindings`, `cbu_ids`, `name`
2. `save -> mutate -> save` uses repository version correctly
3. restore path loads a persisted session and stores the returned version

Verification:
- `cargo check`
- targeted repository/orchestrator tests

Exit criteria:
- the critical agent session persistence bug is fixed
- save/load symmetry is restored
- restore path exists and works

### Phase 2 — Extract The Shared Canonical Session-Stack Struct And Use It In ob-poc

Goal:
After Phase 2, the stack/context model is no longer owned only by REPL code.
There is one shared canonical session-stack struct that both agent and
BPMN-lite can use.

#### Task 2.1 — Identify the exact shared subset

Current `ReplSessionV2` contains both:
- stack/context state that BPMN-lite must share
- agent-only session state that BPMN-lite must not own

Extract the shared subset into a canonical struct. At minimum this includes:
- `active_workspace`
- `workspace_stack`
- session scope
- constraint cascade per frame
- subject/workspace identifiers
- trace sequence for continuity

Explicitly exclude these agent-only state areas:
- `messages`
- `runbook`
- staged pack runtime
- proposal/pending lookup state
- REPL response history

Deliverable:
- exact shared field inventory implemented in the shared crate
- explicit note in code/docs that this shared model is a value type copied
  across the boundary, not a shared record

#### Task 2.2 — Move shared types into a shared crate

- `rust/crates/ob-poc-types`

Move or re-home the shared stack/context types so they are not defined under
`rust/src/repl`.

Likely moved/shared types:
- `SessionScope`
- `WorkspaceKind`
- `SubjectKind`
- `WorkspaceFrame`
- `ViewportSnapshot`
- extracted constraint cascade struct
- new canonical `SessionStackState`

Acceptance criteria:
- BPMN-lite can depend on the shared types without depending on REPL modules
- there is one canonical definition for the stack/context model

#### Task 2.3 — Make `ReplSessionV2` embed the shared canonical struct

Files:
- `rust/src/repl/session_v2.rs`
- affected call sites

Reshape `ReplSessionV2` so its stack/context fields are grouped under the new
shared struct rather than duplicated inline.

- `ReplSessionV2` owns a `SessionStackState` field

Acceptance criteria:
- agent session stack/context now uses the shared canonical model
- no duplicate local-only `WorkspaceFrame` shape remains in REPL code

#### Task 2.4 — Preserve serialization compatibility during extraction

Files:
- shared types crate
- `rust/src/repl/session_v2.rs`
- `rust/src/repl/session_repository.rs`

The extraction must not break restore of existing persisted sessions.

Use serde compatibility techniques as needed:
- field aliases
- wrapper conversion
- compatibility constructors

Acceptance criteria:
- old persisted sessions still restore after the extraction

#### Task 2.5 — Add server projection for session-stack DAG visualization

Files:
- `rust/crates/ob-poc-types/src/graph_scene.rs`
- `rust/src/api/observatory_routes.rs`
- `rust/src/repl/session_stack_projection.rs`

Chosen implementation:
- keep the existing constellation graph scene endpoint as-is
- add a dedicated session-stack graph projection endpoint backed by the
  canonical shared `SessionStackState`

Required endpoint:

```text
GET /api/observatory/session/:id/session-stack-graph
```

Response type:
- `GraphSceneModel`

Projection rules:
- root node represents the session stack state
- each workspace frame becomes a node
- edges represent stack order and parent/child workspace transitions
- node metadata includes workspace kind, subject identity, stale state,
  constraint identifiers, and viewport focus where available
- the projection reads only the canonical shared session-stack struct, not
  parallel ad hoc fields

Acceptance criteria:
- the backend can render the canonical session-stack struct as a DAG projection
- no duplicate UI-only session-stack shape is introduced

#### Task 2.6 — Add observatory UI surface for the session-stack DAG

Files:
- `ob-poc-ui-react/src/api/observatory.ts`
- `ob-poc-ui-react/src/lib/query.ts`
- `ob-poc-ui-react/src/features/observatory/ObservatoryPage.tsx`
- UI components under `ob-poc-ui-react/src/features/observatory/components/`
  as needed

Chosen implementation:
- add a third observatory tab: `session_stack`
- fetch `GET /api/observatory/session/:id/session-stack-graph`
- render it using the existing canvas/rendering path that consumes
  `GraphSceneModel`

User-visible result:
- the user can run the agent/MCP session in the UI
- open the observatory for that same session
- switch to the session-stack tab
- see the DAG visualization of the canonical session-stack struct

Acceptance criteria:
- session-stack graph loads for a real agent session
- UI rendering is driven by the server projection, not client-side
  reinterpretation
- no bespoke client-only session-stack data model is introduced

Verification:
- `cargo check`
- targeted restore/persistence tests
- observatory route tests for the new graph endpoint

Exit criteria:
- one canonical session-stack struct exists
- `ReplSessionV2` uses it
- BPMN-lite can import it
- the UI can visualize the canonical session-stack DAG for a live session

### Phase 3 — Add A Session Stack To BPMN-lite Using The Shared Struct

Goal:
After Phase 3, BPMN-lite process instances carry persisted session-stack state
using the same canonical session-stack struct as the agent side, but only for
REPL runbook steps that execute BPMN-routed verbs.

#### Task 3.1 — Add `session_stack` to `ProcessInstance`

Files:
- `bpmn-lite/bpmn-lite-core/src/types.rs`
- `bpmn-lite/bpmn-lite-core/src/engine.rs`
- `bpmn-lite/bpmn-lite-core/src/store.rs`
- `bpmn-lite/bpmn-lite-core/src/store_memory.rs`
- `bpmn-lite/bpmn-lite-core/src/store_postgres.rs`

Add a field similar to:

```rust
pub session_stack: SessionStackState,
```

to `ProcessInstance`.

Required behavior:
- instance creation initializes it from dispatch input
- persisted instance round-trips it in memory store and Postgres store

Boundary note:
- this `session_stack` is BPMN-lite-owned persisted state
- it is initialized from an ob-poc value copy
- it is not a reference back to the ob-poc session row

Acceptance criteria:
- BPMN-lite owns durable session-stack state on the process instance

#### Task 3.2 — Add BPMN-lite schema support for persisted session stack

Files:
- new migration under `bpmn-lite/bpmn-lite-core/migrations/`
- `store_postgres.rs`

Add a new column on `process_instances`, for example:

```sql
session_stack JSONB NOT NULL DEFAULT '{}'
```

and wire it through:
- save
- load
- atomic start
- atomic complete/update paths

Acceptance criteria:
- Postgres-backed BPMN-lite instances persist the session stack
- restart/reload preserves stack state

#### Task 3.3 — Initialize BPMN-lite session stack from the agent session

Files:
- `rust/src/bpmn_integration/dispatcher.rs`
- `rust/src/bpmn_integration/client.rs`

At dispatch time:
- when REPL runbook execution reaches a BPMN-routed verb, build the canonical
  shared `SessionStackState` from the current agent session
- send it into BPMN-lite when starting that process
- persist it onto the new `ProcessInstance`

The dispatch transport uses a payload envelope for compatibility, but the end state is:
- BPMN-lite instance owns the canonical session stack after start

Copy semantics:
- BPMN-routed REPL runbook execution copies the canonical shared struct by
  value from the ob-poc session into BPMN-lite process state
- after start, the two records are independent unless an explicit sync point is
  implemented

Acceptance criteria:
- a newly started process instance contains the same stack/context shape as the
  originating agent session
- no BPMN process instance receives a session-stack copy unless it was started
  by a BPMN-routed REPL runbook verb

#### Task 3.4 — Expose BPMN-lite session stack directly on job activation

Files:
- `bpmn-lite/bpmn-lite-core/src/types.rs`
- `bpmn-lite/bpmn-lite-core/src/engine.rs`
- `rust/src/bpmn_integration/client.rs`
- `rust/src/bpmn_integration/worker.rs`

Chosen implementation:
- `JobActivation` carries `session_stack: SessionStackState`
- BPMN-lite fills it from `ProcessInstance.session_stack` at activation time
- the worker reads it directly from the activation payload
- worker logic never needs to fetch the ob-poc session record to recover stack
  state

Acceptance criteria:
- worker execution can access the BPMN-owned session stack using the shared
  canonical struct

#### Task 3.5 — Update BPMN instance session stack when BPMN-side context changes

Files:
- BPMN worker/relay/integration paths

Once BPMN-lite owns a session stack, process steps that intentionally change
workspace/session-stack context must update that state on the instance.

This is the key difference from a pure snapshot-at-start design.

Required behavior:
- when a BPMN-routed action changes stack/workspace context, the BPMN instance's
  `session_stack` is updated
- resumed jobs/signals see the latest process session stack, not only the
  original dispatch snapshot

Acceptance criteria:
- BPMN session-stack state is live process state, not just a copied artifact

Verification:
- `cargo check`
- BPMN-lite store tests
- BPMN integration tests covering instance save/load and worker visibility
- tests proving that job activation carries the copied canonical session-stack
  value

Exit criteria:
- BPMN-lite has a real persisted session stack
- it uses the same canonical session-stack struct as the agent side
- the worker consumes BPMN-owned stack state directly from activation payloads

### Phase 4 — End-To-End UI + BPMN Copy Flow

Goal:
After Phase 4, the full user flow works:
- run an agent/MCP-backed session in the UI
- view the canonical session-stack DAG in observatory
- execute a REPL runbook step whose verb routes into BPMN runtime
- verify BPMN instance owns an independent copied session stack using the same
  canonical struct
#### Task 4.1 — Add end-to-end integration coverage for the canonical stack flow

Add tests covering:

1. unified session input creates/mutates a session whose canonical
   `SessionStackState` is persisted and restorable
2. observatory session-stack graph endpoint reflects that canonical stack state
3. BPMN-routed REPL runbook execution copies the same canonical value into
   `ProcessInstance.session_stack`
4. BPMN instance reload preserves that copied value
5. job activation exposes that BPMN-owned `session_stack` to the worker
6. BPMN-side context updates persist to the BPMN instance copy without mutating
   the original ob-poc session record

#### Task 4.2 — Add explicit non-aliasing assertions

Add test assertions that prove:

- ob-poc session persistence and BPMN-lite process persistence use separate
  records
- updating BPMN instance `session_stack` does not mutate ob-poc persisted
  session state
- restoring ob-poc session state does not mutate BPMN instance state unless an
  explicit sync point is executed

#### Task 4.3 — Verification gate

Verification:
- `cargo check`
- targeted REPL persistence tests
- targeted observatory route/UI tests where available
- BPMN-lite store tests
- BPMN integration tests

Required manual verification:

1. create or open a session in the UI
2. drive it through unified session input / agent flow
3. open observatory and inspect the session-stack DAG tab
4. execute a REPL runbook step from that same session whose verb routes into
   BPMN runtime
5. confirm the BPMN instance now contains a copied `session_stack`
6. confirm BPMN-side state remains independent from the ob-poc session record

Exit criteria:
- agent and BPMN-lite use the same canonical session-stack model
- the UI visualizes that canonical model for the live session
- BPMN instance owns a separate copied value of that same model
- no parallel long-term stack models remain

## 6. Out Of Scope

These are not included unless you explicitly want them added:

- moving the full `ReplSessionV2` type into BPMN-lite
- merging agent-only session concerns including messages, runbook state, and proposals into
  BPMN-lite engine state
- broader `UnifiedSession` consolidation outside the shared stack extraction

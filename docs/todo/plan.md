# Session-Scoped Constellation Refactor Plan

## Purpose

This plan defines the review target for refactoring constellation identity, session state, route identity, and UI selection so they all align to one canonical model:

`Session -> Client Group -> Workspace -> Constellation -> Subject -> Verb DSL`

The immediate goal is architectural alignment, not just removal of CBU-first routing.

## Primary Workspaces

The system should treat these as the main top-level workspaces after client-group scope is established:

- `ProductMaintenance`
- `Deal`
- `CBU`
- `KYC`
- `Instrument Matrix`
- `OnBoarding`

Each workspace may expose one or more constellations, and each constellation may optionally focus a subject such as a deal, CBU, KYC case, onboarding handoff, or matrix instance.

## Canonical Session State Model

The agent/session state model should be:

1. `empty`
2. `client_group_scoped`
3. `workspace_selected`
4. `constellation_loaded`
5. `subject_focused`
6. `verb_set_constrained`

This sequence is the architectural source of truth for:

- agent session navigation
- server route identity
- UI selection state
- constellation hydration
- node-level DSL verb surfaces

## Core Principle

Separate `navigation identity` from `business identity`.

Navigation identity:

- `session_id`
- `client_group_id`
- `workspace`
- `constellation_family`
- `constellation_map`

Business identity:

- `deal_id`
- `cbu_id`
- `case_id`
- `handoff_id`
- `matrix_id`
- `product_id`

The refactor is complete only when navigation identity is no longer implicitly derived from `cbu_id`.

## Current Misalignment

The repo still has a partial `CBU -> case -> constellation` assumption in these areas:

- backend route identity
- frontend constellation API calls
- UI selector state
- some session projections and feedback structs

This is incompatible with the intended SAGE model where:

1. the user first scopes a `client group`
2. then enters a `workspace`
3. then loads a `constellation`
4. then focuses a `subject`

## Target Architecture

### 1. Session Scope

The agent starts with no scope and must insist on `client_group`.

Output of this step:

- active `client_group_id`
- active `session_id`

### 2. Workspace Selection

After client-group scope is known, the user or agent selects one of:

- `ProductMaintenance`
- `Deal`
- `CBU`
- `KYC`
- `Instrument Matrix`
- `OnBoarding`

Output of this step:

- active workspace
- valid constellation family set for that workspace

### 3. Constellation Selection

A workspace owns one or more constellations.

Examples:

- `ProductMaintenance` workspace -> product / service / resource taxonomy constellations
- `Deal` workspace -> commercial / handoff constellations
- `CBU` workspace -> operating / maintenance constellations
- `KYC` workspace -> group ownership / clearance / delta-review constellations
- `Instrument Matrix` workspace -> trading permission / lifecycle constellations
- `OnBoarding` workspace -> handoff / activation constellations

Output of this step:

- active constellation family
- active constellation map

### 4. Subject Focus

Only after the workspace and constellation are known should the session focus a business subject.

Examples:

- a deal
- an existing CBU
- a linked KYC case
- an onboarding handoff
- an instrument matrix resource

This subject is optional for some workspace views and mandatory for others.

### 5. Verb Surface

The allowed verb set must be computed from:

`client_group + workspace + constellation + optional subject + effective node state`

Not from raw entity type alone.

## Refactor Goals

### Goal 1: Replace Entity-First Route Identity

Introduce a shared `ConstellationContextRef` or equivalent payload used by backend and frontend.

Minimum fields:

- `session_id`
- `client_group_id`
- `workspace`
- `constellation_family`
- `constellation_map`
- `subject_kind`
- `subject_id`
- `view_kind`

### Goal 2: Make Workspace First-Class

Define a shared workspace enum across:

- backend routes
- session state
- chat feedback structs
- frontend selection state

### Goal 3: Make Constellations Workspace-Owned

Constellations should no longer be loaded as:

- `constellation for cbu`

They should be loaded as:

- `constellation for workspace within client-group scope`

with optional subject focus.

### Goal 4: Unify Agent and UI Feedback

The same context envelope should drive:

- constellation hydration
- summary strips
- DAG/progress feedback
- node-level verb surfaces
- chat/session UI hints

## Implementation Phases

### Phase 1: Define Shared Context Contracts

Add shared types for:

- `WorkspaceKind`
- `SubjectKind`
- `ConstellationContextRef`
- `ResolvedConstellationContext`

Use them in new server-side route handlers first without breaking old routes.

### Phase 2: Add Context Resolver Layer

Create a resolver that takes session/workspace context and returns:

- valid constellations
- valid subject kinds
- resolved default subject if applicable
- hydration inputs
- scoped verb surface context

The resolver should be the only place that translates navigation context into business subject context.

### Phase 3: Introduce New Session-Scoped Routes

Add new route family, for example:

- `GET /api/constellation/context`
- `POST /api/constellation/resolve`
- `GET /api/constellation/hydrate`
- `GET /api/constellation/summary`

These routes must accept session/workspace context, not only `cbu_id`.

### Phase 4: Migrate Frontend API Layer

Refactor the frontend constellation client to call:

- `getConstellation(context)`
- `getSummary(context)`
- `listSelectableSubjects(context)`

instead of:

- `getConstellation(cbuId, caseId)`

### Phase 5: Refactor UI Navigation

The UI flow must become:

1. choose client group
2. choose workspace
3. choose constellation
4. choose subject if needed
5. inspect slots and verbs

The current CBU/case selector should become a generic subject selector scoped by workspace and constellation.

### Phase 6: Align Agent Session Feedback

Agent session payloads should include:

- `session_scope`
- `workspace`
- `constellation_context`
- `subject_ref`
- `hydrated_constellation`
- `workspace_state_view`

This lets the agent and UI talk about the same active frame.

### Phase 7: Compatibility Layer

Keep old routes such as CBU-based constellation endpoints as compatibility wrappers during migration.

These wrappers should:

- resolve legacy `cbu_id + case_id`
- build the new context envelope
- call the new resolver/hydration pipeline

### Phase 8: Deprecation

After the UI and agent both use the new contract:

- remove direct CBU-first assumptions
- deprecate old route shapes
- remove stale client helpers

## Workspace Expectations

### Deal

Focus:

- commercial scope
- contracts
- products
- negotiated rate cards
- onboarding handoff

Likely subjects:

- `deal`
- `contract`
- `handoff`

### CBU

Focus:

- operating structure
- maintenance
- resources
- linked roles and servicing shape

Likely subjects:

- `cbu`

### KYC

Focus:

- group ownership
- UBO
- control
- group clearance
- delta KYC review

Likely subjects:

- `client_group`
- `case`
- optional linked `cbu`

### Instrument Matrix

Focus:

- permissions
- lifecycle requirements
- resource activation rules

Likely subjects:

- `cbu`
- `matrix`
- `product`

### OnBoarding

Focus:

- handoff from deal
- target existing CBU
- activation path
- downstream provisioning

Likely subjects:

- `handoff`
- `deal`
- `cbu`

## Files To Review Closely

Backend:

- `/Users/adamtc007/Developer/ob-poc/rust/src/api/constellation_routes.rs`
- `/Users/adamtc007/Developer/ob-poc/rust/src/sage/session_context.rs`
- `/Users/adamtc007/Developer/ob-poc/rust/src/agent/onboarding_state_view.rs`
- `/Users/adamtc007/Developer/ob-poc/rust/src/api/deal_types.rs`
- `/Users/adamtc007/Developer/ob-poc/rust/src/api/deal_routes.rs`
- `/Users/adamtc007/Developer/ob-poc/rust/src/sem_os_runtime/constellation_runtime.rs`

Frontend:

- `/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/api/constellation.ts`
- `/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/features/chat/components/ConstellationPanel.tsx`
- `/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/features/chat/components/OnboardingStateCard.tsx`
- `/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/types/chat.ts`

Session / REPL / agent alignment:

- `/Users/adamtc007/Developer/ob-poc/rust/src/repl/session_v2.rs`
- `/Users/adamtc007/Developer/ob-poc/rust/src/session/verb_sync.rs`

## Review Questions

1. Are the six workspaces the correct stable top-level session partitions?
2. Which constellation families belong to each workspace?
3. Which workspaces require a focused subject and which can operate at group scope?
4. Should `constellation_map` always be explicit, or can workspace defaults resolve it?
5. Is `OnBoarding` a distinct workspace, or should it be modeled as a handoff mode between `Deal` and `CBU`?
6. Which legacy CBU-first routes must remain during migration?
7. Which server/UI structs should be canonical for session feedback?

## Acceptance Criteria

The architecture is aligned when all of the following are true:

- no top-level constellation route requires `cbu_id`
- no UI constellation view loads before `client_group` and `workspace` are known
- no verb surface is computed without `workspace` and `constellation`
- all subject focus happens after workspace selection
- the agent session state and UI selection state use the same context model
- legacy CBU routes exist only as adapters, not as the architecture

## Non-Goals For This Review

- redesigning the Sem OS slot model itself
- redesigning all business constellations
- rewriting KYC storage schema in this phase
- removing backwards compatibility before the new contract is live

## Recommended Review Outcome

Peer review should either:

- approve this session-first architecture as the target refactor shape

or

- return a revised canonical sequence and workspace partitioning before implementation begins

Implementation should not proceed beyond compatibility scaffolding until that architectural review is settled.

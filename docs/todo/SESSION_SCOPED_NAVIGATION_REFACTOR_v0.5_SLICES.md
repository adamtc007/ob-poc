# Session-Scoped Navigation Refactor v0.5 — Implementation Plan In Slices

## Purpose

This document converts [SESSION_SCOPED_NAVIGATION_REFACTOR_v0.5.md](/Users/adamtc007/Downloads/SESSION_SCOPED_NAVIGATION_REFACTOR_v0.5.md) into an execution plan grounded in the current `ob-poc` codebase and database shape.

It is intended for pre-execution review.

The target model remains:

`Session -> Client Group -> Workspace -> Constellation -> Subject (optional) -> Verb Surface`

## Current Baseline

The repo is not at zero. These parts are already landed:

- SemOS footprint hydration prerequisite is complete enough to start this refactor.
- V2 REPL already has:
  - `ScopeGate`
  - `WorkspaceSelection`
  - workspace-aware pack routing
  - `active_workspace` on the session
- `ProductMaintenance` is now a first-class workspace in SemOS taxonomy and pack routing.
- The `service-resource` runtime verbs have explicit footprint coverage, and the plugin audit reports `unresolved_binding_count = 0`.

## Workspace Reconciliation Note

The v0.5 paper names four primary operational workspaces in the session-state table:

- `Deal`
- `CBU`
- `KYC`
- `Instrument Matrix`

The current repo already has `ProductMaintenance` as a live first-class workspace with:

- SemOS workspace taxonomy membership
- a design-time product/service/resource constellation
- a runtime `product-service-taxonomy` pack

For execution planning, treat `ProductMaintenance` as an explicit fifth workspace unless design review says otherwise. `N0` must resolve whether the paper should be amended permanently, but the implementation slices should not pretend the current repo is still four-workspace.

The major gaps against v0.5 are:

- no stack-machine session model yet
- no shared `WorkspaceFrame` / `ConstellationContextRef` contracts
- constellation hydration routes are still CBU-first
- no navigation resolver / hydration resolver split
- no utterance decomposition or conversation-mode layer for Sage
- no stack-aware session feedback envelope returned uniformly across Sage
- frontend UI selection flow is not yet migrated to `client_group -> workspace -> constellation -> subject`

## Paper Cross-Reference

| Paper Section | Status | Notes |
|---|---|---|
| The Stack Machine | not started | REPL has workspace selection, but no frame stack, PUSH/POP/COMMIT, or stale recovery |
| Sage and REPL — Two Modes, One Pipeline | partial | REPL exists; Sage/REPL boundary is not yet modeled via stack-aware contracts |
| Utterance Semantics Layer | not started | no `UtteranceFrame`, `ConversationMode`, `ScopeCue`, `TemporalCue` |
| Single Utterance Pipeline | not started | current pipeline is not stack-mode driven |
| Sage Response Policies | not started | no fixed foreign-write confirmation or stale recovery narration |
| Session State Model — Partial Order | partial | states 1-3 exist in REPL, but 4-6 are not generalized via shared contracts |
| Resolver Architecture — Two Resolvers | not started | current hydration is direct route/runtime call, not resolver-based |
| Shared Contracts | not started | currently only ad hoc REPL/session/constellation structs |
| Routes — Seven Total | not started | current constellation routes remain CBU-keyed |
| Implementation Phases | partial | pieces of Phase 1/4/6 exist informally, but not in the v0.5 shape |

## Execution Rules

- implement one slice at a time
- do not skip slices
- stop after each slice for review
- keep backward compatibility until the compatibility slice says otherwise
- prefer additive routes and structs before removals
- keep the current REPL usable while the stack model is introduced
- every slice ends with `cargo check`

## Primary Code Surfaces

- REPL/session:
  - [rust/src/repl/types_v2.rs](/Users/adamtc007/Developer/ob-poc/rust/src/repl/types_v2.rs)
  - [rust/src/repl/session_v2.rs](/Users/adamtc007/Developer/ob-poc/rust/src/repl/session_v2.rs)
  - [rust/src/repl/orchestrator_v2.rs](/Users/adamtc007/Developer/ob-poc/rust/src/repl/orchestrator_v2.rs)
  - [rust/src/api/repl_routes_v2.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/repl_routes_v2.rs)
- Current constellation hydration:
  - [rust/src/api/constellation_routes.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/constellation_routes.rs)
  - `rust/src/sem_os_runtime/constellation_runtime.rs`
- Existing session feedback/projection surfaces:
  - [rust/src/sage/session_context.rs](/Users/adamtc007/Developer/ob-poc/rust/src/sage/session_context.rs)
  - [rust/src/agent/onboarding_state_view.rs](/Users/adamtc007/Developer/ob-poc/rust/src/agent/onboarding_state_view.rs)
  - [rust/src/agent/composite_state.rs](/Users/adamtc007/Developer/ob-poc/rust/src/agent/composite_state.rs)
- Frontend API surfaces:
  - [ob-poc-ui-react/src/api/replV2.ts](/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/api/replV2.ts)
  - [ob-poc-ui-react/src/api/constellation.ts](/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/api/constellation.ts)

## Slice Overview

| Slice | Goal | Paper Xref | Human Gate |
|---|---|---|---|
| N0 | baseline and gap confirmation | Purpose, Prerequisite, Routes | required |
| N1 | shared contracts | Shared Contracts, Session State Model | required |
| N2 | stack machine session layer | The Stack Machine, Partial Order | required |
| N3 | navigation resolver | Resolver Architecture, Workspace Registry | required |
| N4 | hydration resolver + 5D cascade binding | Resolver Architecture, Verb Surface Resolution Cascade | required |
| N5 | new session-scoped routes | Routes — Seven Total | required |
| N6 | REPL integration on stack contracts | Sage and REPL, Phase 4 | required |
| N7A | deterministic utterance semantics | Utterance Decomposition, Conversation Mode, Deixis | required |
| N7B | probabilistic fallback and tuning | Utterance Semantics Layer, Regression Classes | required |
| N8 | Sage response policies | Sage Response Policies | required |
| N9 | frontend API migration | Phase 6 | required |
| N10 | UI navigation refactor | Phase 7, Client Situation Surface | required |
| N11 | compatibility and deprecation | Phase 8-9 | required |

---

## Slice N0: Baseline And Gap Confirmation

### Objective

Freeze the starting state after the footprint work and the initial workspace REPL work.

### Tasks

1. inventory current session/repl workspace support
2. inventory current constellation routes and payload identity
3. inventory current frontend API assumptions
4. write a short baseline artifact summarizing:
   - what is already implemented
   - what remains for v0.5
5. explicitly reconcile workspace count:
   - whether `ProductMaintenance` remains first-class
   - whether the paper should be amended from four to five primary workspaces

### Acceptance

- the starting line is explicit and reconciled
- no implementation work yet

---

## Slice N1: Shared Contracts

### Objective

Introduce the v0.5 shared contracts without changing route behavior yet.

### Paper Xref

- `WorkspaceFrame`
- `ConstellationContextRef`
- `ResolvedConstellationContext`
- `WorkspaceStateView`
- `SessionFeedback`
- `UtteranceFrame`
- `ConversationMode`
- `ScopeCue`
- `TemporalCue`

### Tasks

1. define Rust backend types
2. define frontend transport types where needed
3. keep current payloads working, but start embedding the new context types

### v0.5 Binding

This slice must bind to the actual shared contracts from the paper, not just the type names.

Implement or stub with exact field-level intent for:

- `WorkspaceFrame`
- `ConstellationContextRef`
- `ResolvedConstellationContext`
- `WorkspaceStateView`
- `SessionFeedback`
- `UtteranceFrame`
- `ConversationMode`
- `ScopeCue`
- `TemporalCue`

The execution target is the paper’s `Shared Contracts` section, especially:

- one frame per workspace stack entry
- one request envelope for resolver input
- one resolved TOS object
- one hydrated workspace view
- one session feedback payload returned with every Sage response
- one pre-ECIR utterance semantic frame

### Acceptance

- contracts compile
- no runtime behavior change yet

---

## Slice N2: Stack Machine Session Layer

### Objective

Upgrade the session model from single active workspace to stack of workspace frames.

### Paper Xref

- The Stack Machine
- Stack Constraints
- Session State Model — Partial Order

### Tasks

1. add frame stack storage to the V2 session
2. implement `PUSH`, `HYDRATE`, `POP`, `COMMIT`
3. enforce max depth `3`
4. mark lower frames frozen
5. add stale marker metadata on restored frames

### v0.5 Binding

This slice must implement the stack invariants as tests, not leave them implicit:

- max depth = `3`
- only TOS is hydrated
- frames below TOS are frozen
- `PUSH` is atomic push + resolve + hydrate
- `POP` restores exact previous frame modulo stale marker
- `COMMIT` collapses to depth `1`
- resolvers stay stack-oblivious and receive no stack metadata

### Acceptance

- REPL session can hold multiple frames
- only TOS is active/hydrated
- POP/COMMIT semantics are explicit in code

---

## Slice N3: Navigation Resolver

### Objective

Add the first pure resolver: navigation context to top-of-stack frame.

### Paper Xref

- Resolver Architecture — Navigation Resolver
- Workspace Registry

### Tasks

1. create a navigation resolver service/module
2. resolve:
   - client group
   - workspace
   - legal constellations for that workspace
   - optional default subject kinds
3. keep it stack-oblivious; it resolves only the context given

### v0.5 Binding

This slice must also preserve the paper’s situation-surface on-ramp:

- single-context resolution now
- batch-signature compatibility from day one:
  - `Vec<ConstellationContextRef>`

Even if only single-item resolution is wired first, the resolver contract should not block later batch usage.

### Acceptance

- no route or UI should hand-build workspace/constellation context anymore

---

## Slice N4: Hydration Resolver And Resolution Cascade

### Objective

Add the second pure resolver: hydrate TOS into working surface and bind the existing 5D SemOS verb cascade.

### Paper Xref

- Resolver Architecture — Hydration Resolver
- Verb Surface Resolution Cascade

### Tasks

1. create `ResolvedConstellationContext -> WorkspaceStateView` hydration
2. connect workspace/constellation/subject/node-state to existing SemOS footprint resolution
3. ensure the resolver returns:
   - hydrated slots
   - subject focus
   - verb surface
   - stale indicators

### v0.5 Binding

This slice must implement the explicit fallback chain from the paper:

- 5D exact
- 4D fallback without node state
- 3D fallback without subject
- 2D fallback without constellation
- 1D legacy fallback

Required property:

- monotonic narrowing across the cascade

### Acceptance

- hydration is no longer directly route-specific
- 5D verb surface resolution hangs off the resolver, not ad hoc route logic

---

## Slice N5: New Session-Scoped Routes

### Objective

Add the v0.5 route layer without breaking the old routes yet.

### Paper Xref

- Routes — Seven Total

### Tasks

1. add stack-aware session routes
2. add stack-oblivious constellation routes operating on TOS context envelopes
3. keep current `/api/cbu/:cbu_id/...` constellation routes as compatibility shims

### v0.5 Binding

This slice should implement the paper’s seven-route split explicitly.

Constellation routes:

- `GET /api/constellation/context`
- `POST /api/constellation/resolve`
- `GET /api/constellation/hydrate`
- `GET /api/constellation/summary`

Session stack routes:

- `POST /api/session/:id/stack/push`
- `POST /api/session/:id/stack/pop`
- `POST /api/session/:id/stack/commit`

### Acceptance

- new routes can load constellation state without `cbu_id` as the root identity

---

## Slice N6: REPL Integration

### Objective

Move the V2 REPL from workspace-aware to stack-aware.

### Paper Xref

- Sage and REPL — Two Modes, One Pipeline
- Implementation Phase 4

### Tasks

1. use the new stack contracts in REPL state transitions
2. make workspace navigation `PUSH -> HYDRATE -> COMMIT`
3. make foreign writes explicit stack transitions
4. ensure REPL executes only against TOS

### v0.5 Binding

This slice must preserve the strict Sage/REPL boundary:

- REPL executes only at session state `6`
- REPL executes only against TOS
- Sage resumes after every execution and renders feedback from the resulting TOS state

### Acceptance

- REPL no longer relies on one flat `active_workspace`
- stack transitions are explicit and testable

---

## Slice N7A: Deterministic Utterance Semantics

### Objective

Implement the deterministic pre-ECIR semantic frame, cue tables, and mode-to-stack mapping.

### Paper Xref

- Utterance Decomposition — Before ECIR
- Conversation Mode Classification
- Pronoun and Deixis Resolution Policy
- Single Utterance Pipeline

### Tasks

1. add `UtteranceFrame` decomposition before entity/verb resolution
2. classify `ConversationMode`
3. implement pronoun/deixis policy against TOS/render history
4. map modes to stack actions:
   - inspect
   - navigate
   - compare
   - prepare
   - mutate
   - confirm
   - return

### v0.5 Binding

This slice covers the deterministic half only:

- lexical cue extraction for:
  - `action_phrase`
  - `target_workspace_hint`
  - `subject_hint`
  - `scope_cue`
  - `temporal_cue`
- explicit mode resolution for clear cases
- deterministic deixis rules for:
  - `here/this/current`
  - `that/the [entity]`
  - `there/that workspace`
  - `go back` after `COMMIT` vs ordinary `POP`

### Acceptance

- Sage can choose stack behavior from the utterance frame instead of implicit heuristics

### Human Gate

Review cue tables and deterministic mode mapping before adding fallback classification.

---

## Slice N7B: Probabilistic Fallback And Tuning

### Objective

Layer ambiguous/fallback utterance classification on top of `N7A`.

### Paper Xref

- Utterance Semantics Layer
- Utterance Regression Classes

### Tasks

1. implement fallback classification when deterministic rules do not settle the frame
2. add regression classes for:
   - inspect vs navigate
   - prepare vs mutate
   - compare
   - ambiguous pronouns on read
   - ambiguous pronouns on write
3. define or load a tuning/test corpus

### v0.5 Binding

Codex can implement:

- fallback hook points
- regression harness
- fixture loading

Human review is required for:

- ambiguous utterance labels
- acceptance thresholds
- tuning decisions

### Acceptance

- deterministic path remains authoritative where it applies
- ambiguous write-path resolution still fails closed to clarification

---

## Slice N8: Sage Response Policies

### Objective

Make Sage responses obey the v0.5 confirmation, stale recovery, and effect narration policy.

### Paper Xref

- Write Confirmation Requirements
- Foreign Write Confirmation Structure
- Stale Recovery Policy
- Effect Narration

### Tasks

1. implement strict write-confirmation gate
2. implement fixed foreign-write confirmation rendering
3. implement stale recovery narration + rehydrate prompt
4. implement post-REPL effect narration

### v0.5 Binding

This slice must inline the paper’s rules:

Write confirmation skip is allowed only if all seven are true:

1. single candidate verb
2. single subject
3. same workspace as TOS
4. no handoff ambiguity
5. no stale warning on TOS
6. low-risk / reversible / idempotent effect class
7. utterance is imperative `mutate`, not exploratory `prepare`

Foreign write confirmation must render, in fixed order:

- current workspace
- target workspace
- proposed action
- if confirmed
- commit consequence
- return option

Effect narration must render, in fixed order:

1. what changed
2. what state is now true
3. what next

Stale recovery must:

- narrate staleness explicitly
- say what may have changed
- offer re-hydration
- block pronoun-based write preparation on stale frames

### Acceptance

- Sage replies become policy-driven, not ad hoc

---

## Slice N9: Frontend API Migration

### Objective

Move the frontend clients to the new session/context routes.

### Paper Xref

- Implementation Phase 6

### Tasks

1. migrate `replV2` transport types to shared stack/context contracts
2. migrate constellation API from CBU-keyed calls to context envelopes
3. keep compatibility adapters until UI migration is complete

### Acceptance

- frontend no longer assumes `cbu_id + case_id` as the universal selector

---

## Slice N10: UI Navigation Refactor

### Objective

Make the UI match the stack/session model.

### Paper Xref

- Implementation Phase 7
- Client Situation Surface — On-Ramp

### Tasks

1. render:
   - client group scope
   - workspace selector
   - constellation selector
   - subject selector
2. show TOS session feedback
3. show stale frame warnings and return behavior

### Acceptance

- UI selection order matches the architecture target

---

## Slice N11: Compatibility And Deprecation

### Objective

Retire the old CBU-first constellation identity once the new stack/context path is live.

### Paper Xref

- Implementation Phase 8
- Implementation Phase 9

### Tasks

1. keep compatibility shims during rollout
2. migrate all internal callers
3. remove deprecated route assumptions
4. update docs and runtime guidance

### Acceptance

- no core runtime path requires CBU-first constellation identity

## Recommended Execution Order

The safe order is:

`N0 -> N1 -> N2 -> N3 -> N4 -> N5 -> N6 -> N7A -> N7B -> N8 -> N9 -> N10 -> N11`

Do not start `N7A` before `N1-N6` are stable. Do not start `N7B` before `N7A` has been reviewed.

## Immediate Recommendation

Start with `N0` and `N1` only.

Those two slices will:

- freeze the actual baseline
- define the shared contracts
- create the reviewable boundary before any route or session rewiring begins

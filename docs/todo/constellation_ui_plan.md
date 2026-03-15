# Constellation UI Plan

**Date:** 2026-03-15  
**Scope:** Session-integrated UI for rendering the server-side constellation graph returned by the new constellation API harness.  
**Goal:** Keep the graph inside the same agent session window so it becomes part of the user-agent feedback loop, not a separate tool.

---

## Objective

Build a session-native constellation visualization that:

- renders the hydrated server-side constellation payload;
- stays visible within the same agent session surface as chat;
- supports user-to-agent and agent-to-user feedback;
- explains slot state, blockers, warnings, and next available actions;
- refreshes as the session mutates structure state.

This should behave as a conversational state pane, not as a separate navigation destination.

---

## Product Position

The constellation graph is not just diagnostics. It is shared conversational state.

The intended interaction loop is:

1. User asks for status or next steps.
2. Agent reasons over the same slot graph the user can see.
3. UI renders structure, blockers, and progress inline with the chat session.
4. User clicks a slot or asks a follow-up from what they see.
5. Session context updates and the agent continues from that shared state.

This implies:

- chat remains the primary interaction surface;
- constellation is a docked session panel or split-pane view;
- slot selection becomes part of session context;
- agent messages should be able to reference visible slot ids.

---

## UX Direction

## Layout

Recommended desktop layout:

- Left or center: agent conversation timeline.
- Right: persistent constellation panel.
- Top of constellation panel: summary strip.
- Main constellation body: graph/tree view.
- Bottom or slide-over: selected slot inspector.

Recommended mobile layout:

- chat remains primary;
- constellation opens as a tab, drawer, or bottom sheet within the same session;
- selected slot inspector replaces or overlays the graph pane.

## Panel Sections

1. Summary strip
- overall progress
- completion percentage
- blocking slot count
- ownership chain stats

2. Graph/tree renderer
- root CBU
- child slots
- recursive ownership chain node
- visual state encoding by `effective_state`

3. Slot inspector
- slot name/path
- computed state
- effective state
- warnings
- available verbs
- blocked verbs with reasons
- overlays/evidence on demand

4. Session linkage
- “ask agent about this slot”
- “why is this blocked?”
- “what should I do next?”

---

## UI Contract

The frontend should treat the server as source of truth.

Primary read routes:

- `GET /api/cbu/:cbu_id/constellation?map_name=...`
- `GET /api/cbu/:cbu_id/constellation/summary?map_name=...`
- `GET /api/constellation/by-name?name=...&map_name=...`
- `GET /api/constellation/search-cbus?name=...`

Routing note:

- these REST routes now exist and should be the default UI read path;
- existing DSL/plugin verbs (`constellation.hydrate`, `constellation.summary`) remain valid server-side integration points;
- UI reads should prefer REST for simplicity and typed JSON handling;
- mutations should continue to flow through the session/Sage/Coder pipeline rather than new direct constellation write APIs.

Primary payload:

- `HydratedConstellation`
- nested `HydratedSlot`
- `ConstellationSummary`

The UI should not recompute reducer state locally.

The UI may derive:

- expanded/collapsed state
- selected slot
- filtered views
- emphasis/highlighting

The UI should not derive:

- progress/state transitions
- blocker semantics
- summary counts

## Case Context

`case_id` is optional in the API, but it materially changes the constellation output.

A single CBU can have multiple cases, and different cases produce different:

- workstreams
- screenings/evidence overlays
- reducer states
- blockers and next actions

Recommended UI rule:

- if no `case_id` is active, render the structural constellation with case-independent data only;
- if a `case_id` is active, render the case-scoped constellation;
- always show the active case context in the constellation header;
- when multiple cases exist, expose a case selector instead of silently guessing.

This means the session should bind both:

- active `cbu_id`
- active `case_id` when applicable

---

## Proposed Frontend State Model

Session-scoped view state:

- `selectedCbuId: string | null`
- `selectedCaseId: string | null`
- `selectedMapName: string`
- `constellation: HydratedConstellation | null`
- `summary: ConstellationSummary | null`
- `selectedSlotPath: string | null`
- `expandedSlotPaths: Set<string>`
- `isLoading: boolean`
- `error: string | null`
- `lastLoadedAt: string | null`

Derived state:

- `selectedSlot`
- `blockingSlots`
- `nextActionSlots`
- `ownershipChainSlot`

Session integration state:

- `focusedSlotPath`
- `slotPathMentionMap`
- `pendingRefreshReason`

---

## Component Plan

## 1. `SessionConstellationPane`

Responsibilities:

- own data loading for the active CBU/case;
- render loading/error/empty states;
- coordinate graph + summary + inspector.

Inputs:

- `sessionId`
- `cbuId`
- `caseId`
- `mapName`

Outputs:

- slot selection events
- refresh requests
- agent context events

## 2. `ConstellationSummaryBar`

Responsibilities:

- render high-level counts;
- show overall progress and blocking emphasis.

Fields to render:

- `overall_progress`
- `completion_pct`
- `blocking_slots`
- `slots_empty_mandatory`
- `slots_in_progress`
- `ownership_chain`

## 3. `ConstellationTree`

Responsibilities:

- render `HydratedSlot` hierarchy;
- support expand/collapse;
- support selection;
- visually encode state severity and completeness.

Visual treatment:

- `empty`: muted / hollow
- `placeholder`: dashed / provisional
- `filled`: neutral-progress
- `workstream_open` / `in-progress`: amber
- `verified` / `approved`: green
- blocking slots: red badge/outline

## 4. `ConstellationSlotNode`

Responsibilities:

- display one slot row;
- show state chip, progress, warning count, blocked count;
- optionally show compact verb affordances.

Minimum fields:

- `name`
- `effective_state`
- `progress`
- `blocking`
- warning count
- blocked verb count

## 5. `ConstellationSlotInspector`

Responsibilities:

- show selected slot details;
- show structured explanations;
- provide “ask agent” affordances.

Sections:

- identity: name/path/type/cardinality
- state: computed/effective/progress
- warnings
- available verbs
- blocked verbs with reasons
- overlays/raw data

## 6. `ConstellationEmptyState`

Responsibilities:

- explain when no CBU is selected;
- offer search/resolve by name;
- suggest loading an example CBU.

## 7. `ConstellationSearchResolver`

Responsibilities:

- support debug/demo flows using `search-cbus` / `by-name`;
- resolve a user-entered name like “Allianz”.

This can be debug-only initially.

## 8. `ConstellationCaseSelector`

Responsibilities:

- display the active case context;
- allow switching when multiple cases exist;
- trigger reload on case changes.

This can be minimal in the first pass:

- case badge in the header
- dropdown when more than one case is available

---

## Session Integration Plan

## Agent -> Graph

Agent responses should be able to reference slot ids or paths:

- `management_company`
- `case`
- `case.tollgate`
- `ownership_chain`

UI behavior:

- detect slot-path mentions in agent responses;
- render them as clickable chips/links;
- clicking focuses and opens the slot in the constellation panel.

## Graph -> Agent

The inspector should expose session actions like:

- “Ask why this is blocked”
- “Ask what to do next”
- “Explain this slot”
- “Show evidence for this slot”

These should prefill or dispatch structured prompts into the existing session input flow rather than bypassing the agent.

Example prompts:

- `Why is depositary blocked?`
- `What action should I take for management_company?`
- `Explain the ownership_chain state.`

## Refresh Model

Refresh triggers:

- session startup with bound CBU
- successful DSL action affecting structure/roles/case/state
- manual refresh
- explicit agent recommendation to refresh

Refresh policy:

- debounce repeated refreshes after a mutation burst;
- keep stale data visible while reloading;
- show last refresh timestamp.

Phase 1 refresh rule:

- refresh after any successful Coder-mode mutation likely to affect constellation-visible state.

High-signal domains:

- `state`
- `cbu`
- `entity`
- `case`
- `kyc-workstream`
- `screening`
- `evidence`
- `red-flag`
- `ubo`
- `tollgate`

Longer term this can become footprint-aware, but Phase 1 should prefer correctness over minimal refreshes.

---

## Rendering Strategy

Phase 1 should use a tree-first renderer, not a force-directed graph.

Reason:

- the payload is hierarchical already;
- it is easier to make readable in a session panel;
- slot/state inspection matters more than physics/layout novelty;
- recursive ownership chain can still be represented as a special expandable branch.

Recommended progression:

1. Tree renderer for all slots.
2. Special visual treatment for `ownership_chain`.
3. Optional future graph canvas for ownership exploration.

This avoids over-investing in a graph library before the interaction model settles.

Backend dependency note:

- the current backend `ownership_chain` payload is summary-oriented for UI purposes;
- it does not yet provide the full node/edge graph structure needed for a rich interactive graph canvas;
- therefore early UI work should treat `ownership_chain` as a structured summary/inspector section, not as a force-directed graph requirement.

---

## Data Mapping Rules

Map each `HydratedSlot` to a node model:

- `id = path`
- `label = name`
- `state = effective_state`
- `progress`
- `blocking`
- `warnings`
- `available_verbs`
- `blocked_verbs`
- `children`

Special handling:

- root `cbu` should always be pinned/open
- `entity_graph` should display node/edge counts from:
  - `graph_node_count`
  - `graph_edge_count`

Visual encoding rule:

- use `effective_state` for color/badge/state semantics;
- use `progress` for bars and percentage display;
- use `blocking` for red emphasis and urgency.

Inspector overlay rendering:

- group by `source_name`
- show compact key/value rows
- collapse by default

Slot path rule:

- use `path` as the stable UI identifier;
- use it for selection keys, deep links, and message-to-slot references.

Type contract rule:

- create explicit TypeScript interfaces mirroring the Rust JSON payloads:
  - `HydratedConstellation`
  - `HydratedSlot`
  - `ConstellationSummary`
- do not leave the wire format implicit at the component boundary.

---

## Suggested Build Phases

## Phase 1 — Session Pane Skeleton

- add `SessionConstellationPane`
- fetch `summary` and full `constellation` via the REST harness routes
- render loading/error/empty states
- mount in the existing session layout
- show active case context if present

Definition of done:

- selected CBU/session can show constellation data in the same window as chat

## Phase 2 — Tree Renderer

- render rooted slot hierarchy
- add state chips and progress bars
- add selection and expansion

Definition of done:

- user can inspect slot state and understand blockers visually

## Phase 3 — Slot Inspector + Agent Linkage

- render warnings, blocked verbs, available verbs, overlays
- add “ask agent about this slot”
- make slot references in agent messages clickable

Definition of done:

- graph and conversation are visibly linked both directions

## Phase 4 — Refresh + Mutation Awareness

- refresh after relevant successful actions
- preserve selection/focus
- highlight changed slots on refresh
- keep case-bound context stable across refreshes

Definition of done:

- graph feels live during agent-driven work

## Phase 5 — Ownership Chain Enrichment

- render `ownership_chain` cleanly with the current summary-level backend payload
- emphasize graph counts and warnings
- optionally add subgraph/modal rendering only after backend graph payload enrichment exists

Definition of done:

- recursive ownership path is understandable without leaving session, within the limits of the current backend payload

---

## Technical Risks

1. Session layout crowding
- The graph pane can overwhelm the conversation if given too much weight.

2. Overlays too noisy
- Raw overlay data can flood the inspector unless grouped/collapsed.

3. Slot-path/agent-reference drift
- Agent responses need a stable convention for slot references.

4. Refresh churn
- Repeated agent actions can cause a noisy refresh loop without debounce/highlight strategy.

5. Recursive graph complexity
- `ownership_chain` may eventually need a richer renderer than the tree panel.

---

## Implementation Notes

- Keep the constellation visualization read-mostly.
- Do not let the pane silently mutate state outside the agent/session pipeline.
- Prefer session-coordinated actions over direct button-heavy CRUD.
- Make the graph legible before making it fancy.
- Treat the tree as the canonical Phase 1 UI, with graph-canvas work deferred.

---

## Recommended Next Step

Start with a tree-based `SessionConstellationPane` mounted beside the agent conversation, backed by:

- `GET /api/cbu/:cbu_id/constellation`
- `GET /api/cbu/:cbu_id/constellation/summary`

Then add slot selection + inspector + agent prompt actions before investing in more advanced graph rendering.

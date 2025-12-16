# Proposal: Entity Resolution UI for Agent-Assisted DSL

**Author:** Claude (AI Agent)  
**Date:** 2024-12-16  
**Status:** Draft - Awaiting Peer Review  
**Related:** `docs/ENTITY_RESOLUTION_UI.md` (detailed design)

---

## Executive Summary

This proposal introduces an **interactive entity resolution UI** integrated into the agent chat experience. When DSL is generated (from templates or natural language), unresolved entity references are presented to the user in a resolution panel, allowing human-in-the-loop disambiguation before execution.

**Key value proposition:** Transform entity resolution from a blocking error into an interactive, guided experience that leverages both AI intelligence and human judgment.

---

## Problem Statement

### Current Pain Points

1. **Silent Failures**: Entity resolution happens invisibly. When it fails, the user gets an error with no visibility into what was attempted.

2. **All-or-Nothing**: A single unresolved entity blocks the entire DSL execution. No partial progress is possible.

3. **No Disambiguation**: When multiple entities match (e.g., "Allianz Global Investors" matches 16 entities), the system either picks arbitrarily or fails. The user has no say.

4. **Repeated Friction**: Users must edit DSL manually to fix entity names, then re-submit. This breaks flow and requires DSL syntax knowledge.

5. **Lost Context**: Error messages don't explain *why* resolution failed or *what* alternatives exist.

### Why This Matters

The DSL is a **hybrid of code and data references**. Unlike pure programming languages where all identifiers are defined in code, our DSL references live database entities. This fundamental difference requires a different UX paradigm - one that acknowledges the data dependency.

---

## Proposed Solution

### High-Level Approach

Introduce a **Resolution Panel** that appears inline in the chat when DSL contains unresolved entity references. The panel:

1. Shows all unresolved references grouped by entity type
2. Provides search UI tailored to each entity type's schema
3. Displays ranked matches with discriminating attributes
4. Allows user to select, refine search, or create new entities
5. Updates AST in real-time as resolutions are confirmed

### User Experience Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ USER: "Add Allianz Global Investors as IM for Apex Fund"       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ AGENT: I've generated DSL for this. There are 2 entities       │
│ that need your confirmation:                                    │
│                                                                 │
│ ┌─────────────── RESOLUTION PANEL ────────────────────────────┐│
│ │                                                              ││
│ │ 1. "Allianz Global Investors" (company)                     ││
│ │    ┌─────────────────────────────────┐                      ││
│ │    │ 🔍 Allianz Global               │ [Search]             ││
│ │    └─────────────────────────────────┘                      ││
│ │    ● Allianz Global Investors GmbH (DE)        95% ✓        ││
│ │    ○ Allianz Global Investors Luxembourg S.A.  92%          ││
│ │    ○ Allianz Global Investors UK Limited       88%          ││
│ │                                                              ││
│ │ 2. "Apex Fund" (cbu)                                        ││
│ │    ✓ Auto-resolved: Apex Capital Fund (LU)                  ││
│ │                                                              ││
│ │ ─────────────────────────────────────────────────────────── ││
│ │ [Confirm & Execute]                      Resolved: 2/2      ││
│ └──────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

---

## Technical Design

### Architecture Overview

```
┌──────────────┐     ┌──────────────┐     ┌──────────────────┐
│   Agent      │────▶│  Session     │────▶│  Resolution      │
│   Chat       │     │  State       │     │  Session         │
└──────────────┘     └──────────────┘     └──────────────────┘
                                                   │
                     ┌─────────────────────────────┤
                     ▼                             ▼
              ┌──────────────┐            ┌──────────────────┐
              │  EntityGateway│            │  Resolution      │
              │  (gRPC)      │◀───────────│  Panel (UI)      │
              └──────────────┘            └──────────────────┘
```

### Key Components

| Component | Responsibility |
|-----------|---------------|
| `ResolutionSession` | State container for pending resolutions |
| `UnresolvedRef` | Single entity needing resolution with context |
| `SearchSchema` | Parsed from verb YAML, drives search UI |
| `ResolutionPanel` | egui component for interactive resolution |
| `/api/session/:id/resolution/*` | REST endpoints for resolution workflow |

### API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/resolution/start` | POST | Extract unresolved from session DSL |
| `/resolution/search` | POST | Search with refined query/discriminators |
| `/resolution/commit` | POST | Apply resolutions to AST |
| `/resolution/cancel` | POST | Abandon resolution session |

### Data Model

```rust
pub struct ResolutionSession {
    pub id: Uuid,
    pub session_id: Uuid,
    pub unresolved: Vec<UnresolvedRef>,
    pub auto_resolved: Vec<ResolvedRef>,
    pub pending_resolutions: HashMap<String, ResolvedRef>,
    pub state: ResolutionState,
}

pub struct UnresolvedRef {
    pub ref_id: String,
    pub entity_type: String,
    pub search_value: String,
    pub search_schema: SearchSchema,  // From verb YAML
    pub context: RefContext,          // Which verb/arg
    pub initial_matches: Vec<EntityMatch>,
}
```

---

## Design Decisions & Rationale

### Decision 1: Inline Panel vs Modal

**Choice:** Inline panel in chat flow  
**Rationale:** 
- Maintains conversation context
- User can see agent's explanation alongside resolution UI
- Feels like a natural part of the dialogue, not an interruption
- Resolution is *part of* the interaction, not separate from it

**Alternative considered:** Full-screen modal  
**Why rejected:** Breaks flow, hides context, feels like an error state

### Decision 2: Batch vs Sequential Resolution

**Choice:** Show all unresolved at once (batch)  
**Rationale:**
- User sees full scope of work upfront
- Can prioritize or skip as needed
- Reduces round-trips
- Enables "auto-resolve all high-confidence" action

**Alternative considered:** One-at-a-time wizard  
**Why rejected:** Tedious for bulk operations, no overview

### Decision 3: Search Schema from Verb YAML

**Choice:** Derive search UI from existing verb YAML `lookup.search_key`  
**Rationale:**
- Single source of truth (verb YAML already defines entity lookup)
- S-expression schema already supports discriminators with selectivity
- No new configuration needed - leverage existing infrastructure
- Consistent with LSP autocomplete behavior

**Alternative considered:** Separate UI configuration per entity type  
**Why rejected:** Duplication, maintenance burden, potential inconsistency

### Decision 4: Pre-fetch Initial Matches

**Choice:** Include initial matches in `/resolution/start` response  
**Rationale:**
- Reduces latency for first render
- Many entities will have obvious matches
- Enables auto-resolution detection server-side

**Alternative considered:** Lazy load on panel open  
**Why rejected:** Adds perceptible delay, worse UX

### Decision 5: Resolution Stored in AST

**Choice:** Commit resolutions by updating `EntityRef.resolved_key` in AST  
**Rationale:**
- AST is source of truth for session
- Resolved DSL can be serialized with UUIDs for audit
- Execution path already handles resolved EntityRefs
- Consistent with existing semantic validator flow

**Alternative considered:** Separate resolution map  
**Why rejected:** Adds indirection, risks inconsistency

---

## Open Questions for Review

### Q1: Auto-Resolution Threshold

Should we auto-resolve matches above a certain confidence threshold (e.g., 95%)?

**Options:**
- A) Always require user confirmation (safest, most tedious)
- B) Auto-resolve exact matches only (reference data like roles)
- C) Auto-resolve above 95% confidence (faster, small risk)
- D) User preference setting (flexible, adds complexity)

**Current design:** Option B (auto-resolve exact matches)

### Q2: Create-in-Place Flow

When no matches are found, should we offer inline entity creation?

**Options:**
- A) Yes - mini form in resolution panel (convenient, scope creep)
- B) No - just offer "Create New" button that opens separate flow
- C) Suggest DSL for entity creation (meta, but consistent)

**Current design:** Deferred to Phase 4

### Q3: Resolution History/Memory

Should we remember user's resolution choices for future sessions?

**Options:**
- A) No memory - each session is independent
- B) Session-level memory - within one session, reuse choices
- C) User-level memory - persist across sessions (privacy concern?)
- D) Suggestion-based - "Last time you resolved X as Y"

**Current design:** Not addressed - needs discussion

### Q4: Agent Involvement in Resolution

Should the agent help during resolution (e.g., "Based on context, I think you mean...")?

**Options:**
- A) Agent is passive - just triggers resolution, user handles it
- B) Agent provides hints - adds context to each unresolved ref
- C) Agent actively helps - can refine search, suggest matches
- D) Agent can resolve on user's behalf with confirmation

**Current design:** Option B (agent provides context hints)

### Q5: Bulk Operations Scaling

For bulk operations (e.g., 100 funds), will the UI become unwieldy?

**Options:**
- A) Paginate the resolution panel
- B) Group by entity type with expand/collapse
- C) "Auto-resolve all" with exception review
- D) Two-pass: auto-resolve, then show only failures

**Current design:** Not addressed for v1 - start with <20 entities

---

## Implementation Phases

### Phase 1: Backend Foundation (Est: 2-3 days)
- [ ] Add `ResolutionSession` to session state
- [ ] Implement unresolved ref extraction from AST
- [ ] Build `/resolution/start` endpoint
- [ ] Build `/resolution/search` endpoint (proxy to EntityGateway)
- [ ] Build `/resolution/commit` endpoint

### Phase 2: Basic UI (Est: 2-3 days)
- [ ] Create `ResolutionPanel` egui component
- [ ] Wire up to resolution API
- [ ] Basic search input and match list
- [ ] Selection and commit flow

### Phase 3: Chat Integration (Est: 1-2 days)
- [ ] Add `ResolutionRequired` message type
- [ ] Integrate panel into chat flow
- [ ] Handle panel open/close state
- [ ] Agent context hints

### Phase 4: Polish (Est: 2-3 days)
- [ ] Discriminator refinement UI
- [ ] Confidence scoring display
- [ ] Auto-resolution for high-confidence
- [ ] Create-in-place mini form
- [ ] Keyboard navigation

**Total estimated effort:** 7-11 days

---

## Risks & Mitigations

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| EntityGateway latency impacts UX | Medium | Low | Pre-fetch, debounce, show loading state |
| UI complexity overwhelms users | High | Medium | Progressive disclosure, good defaults |
| Resolution state desync with AST | High | Low | Single source of truth (AST), transactions |
| Scope creep into entity creation | Medium | High | Firm Phase 4 boundary, separate proposal |
| Performance with bulk operations | Medium | Medium | Defer bulk optimization, start small |

---

## Success Metrics

1. **Resolution Success Rate**: % of sessions with unresolved refs that complete resolution (target: >90%)
2. **Time to Resolution**: Average time from panel open to commit (target: <30s for <5 entities)
3. **Auto-Resolution Rate**: % of refs auto-resolved without user action (target: >50% for reference data)
4. **User Satisfaction**: Qualitative feedback on resolution experience

---

## Appendix: Alternatives Considered

### Alternative A: Smarter Agent Generation

**Idea:** Make the agent better at generating correct entity names upfront.

**Why insufficient:**
- Agent can't know all entity names in database
- Disambiguation still required for similar names
- Doesn't help with template expansion
- User might *want* to pick from options

### Alternative B: Fuzzy Execution

**Idea:** Execute with best-match entities, let user review after.

**Why rejected:**
- Violates data integrity - wrong entities could be linked
- Undo is complex (would need transaction rollback)
- User loses control over what's created

### Alternative C: Pre-Validation Mode

**Idea:** Require all entities to exist before DSL generation.

**Why rejected:**
- Chicken-and-egg: how do you create the first entities?
- Breaks natural language flow
- Too restrictive for exploratory use

---

## Request for Review

Please review this proposal and provide feedback on:

1. **Design decisions** - Do the rationales make sense? Any blind spots?
2. **Open questions** - What are your preferences on Q1-Q5?
3. **Phasing** - Is the phase breakdown appropriate?
4. **Risks** - Any risks not identified?
5. **Alternatives** - Any other approaches we should consider?

---

*This proposal was generated by Claude as part of the ob-poc agent development. It represents a synthesis of the existing codebase architecture, user experience considerations, and technical constraints.*

# Entity-First Utterance Parsing Refactor Spec

> **Status:** Proposed — awaiting approval before coding
> **Author:** Claude Opus 4.6 + Adam
> **Date:** 2026-03-23
> **Scope:** Medium (3 PRs, ~300 LOC, 2-3 days)

## Problem Statement

The ECIR noun scanner runs BEFORE entity resolution. Entity names in
utterances ("HSBC Holdings plc", "Goldman Sachs Group", "Allianz Dynamic
Commodities") pollute the noun scan because the scanner doesn't know
they're entity names — it tries to match "Holdings", "Group", "Dynamic"
against domain noun aliases.

Meanwhile, the entity linker (which DOES know all entity names from an
in-memory snapshot of 23K+ patterns) runs AFTER verb resolution. The
information that could prevent the collision arrives too late.

## Principle

**Entity resolution is a conversation turn, not a background step.**

The user always sees what entity the system thinks they mean. If
ambiguous, the user picks before the pipeline continues. If clear,
the UUID flows silently through every subsequent stage.

**If we pay for entity resolution, we keep the result.** The entity
UUID is a first-class value from utterance parsing through to execution.
No second lookup. No `<Entity>` placeholder resolution later.

## Proposed Pipeline

```
Utterance: "open a KYC case for HSBC"
    │
    ▼
Stage 0: Entity Mention Extraction (fast, in-memory)
    │ MentionExtractor.extract() against EntitySnapshot
    │ Returns: Vec<MentionSpan> with candidates + scores
    │
    ├─ CLEAR HIT (1 candidate, score > 0.90)
    │   Entity: "HSBC Holdings plc" (uuid-123, kind=group)
    │   Spans: [(25, 29)]
    │   → Continue to Stage 1 silently
    │
    ├─ AMBIGUOUS (2+ candidates, close scores)
    │   Candidates:
    │     1. HSBC Holdings plc (group, score 0.92)
    │     2. HSBC Bank plc (company, score 0.88)
    │     3. HSBC Custody Services (cbu, score 0.85)
    │   → PAUSE: Return ClarifyEntity decision to user
    │   → User picks → UUID resolved → Resume from Stage 1
    │
    └─ NO MATCH (no candidates above threshold)
        Spans: [] (no masking)
        → Continue to Stage 1 without entity context
        → If verb requires entity, prompt later
    │
    ▼
Stage 1: ECIR Noun Scan (with entity masking)
    │ NounIndex.extract_with_exclusions(utterance, entity_spans)
    │ Entity name spans pre-marked as covered → not scanned for nouns
    │ Extracts: "KYC case" noun + "open" action
    │
    ▼
Stage 2: Verb Resolution (entity-constrained)
    │ ECIR resolve (noun + action → verb candidates)
    │ OR embedding search (with entity_kind from Stage 0)
    │ Verb surface filtered by entity kind (company → kyc verbs)
    │
    ▼
Stage 3: SemOS Grounding
    │ Subject = resolved entity UUID
    │ Constellation grounding: uuid-123 → kyc.onboarding slot
    │ GroundedActionSurface: current_state + valid_actions
    │
    ▼
Stage 4: DSL Assembly
    │ Verb + entity UUID → DSL with pre-resolved entity
    │ (kyc-case.create :entity-id "uuid-123")
    │ No <HSBC> placeholder. No resolution modal. Already resolved.
    │
    ▼
Stage 5: Confirm + Execute
    │ Entity UUID in runbook → direct DB operation
    │ No re-resolution at execution time
```

## Entity Resolution Outcomes

| Outcome | User Experience | Pipeline Behavior |
|---------|----------------|-------------------|
| **Clear** (1 candidate, high score) | Silent — user doesn't see resolution | UUID flows through, spans mask ECIR |
| **Ambiguous** (2+ candidates) | "Which HSBC?" with numbered options | Pipeline pauses → user picks → resume |
| **Ambiguous entity KIND** | "Is 'HSBC' a group, company, or CBU?" | Kind determines constellation + verb surface |
| **No match** | No interruption OR "I don't recognise X" | Continue without entity context |
| **New entity** | "Create a new entity for HSBC?" | Routes to entity.create flow |

## Interactive Resolution UX

The entity clarification uses the existing `DecisionPacket` system:

```json
{
  "kind": "clarify_entity",
  "prompt": "Which HSBC did you mean?",
  "choices": [
    {
      "id": "uuid-123",
      "label": "HSBC Holdings plc",
      "description": "Group — parent holding company",
      "entity_kind": "group",
      "constellation": "group.ownership"
    },
    {
      "id": "uuid-456",
      "label": "HSBC Bank plc",
      "description": "Company — UK banking subsidiary",
      "entity_kind": "company",
      "constellation": "kyc.onboarding"
    }
  ]
}
```

User clicks option → numeric selection (like verb disambiguation) →
pipeline resumes with resolved UUID + kind.

## What Moves Where

### Current Order
1. Verb search (ECIR + embeddings)
2. Entity linking (after verb selected)
3. DSL generation (with `<Entity>` placeholders)
4. Entity resolution modal (user resolves placeholders)
5. Execution

### New Order
1. **Entity mention extraction** (fast, in-memory)
2. **Entity clarification** (if ambiguous — user picks)
3. Verb search (ECIR masked + entity-kind constrained)
4. DSL generation (with pre-resolved UUIDs)
5. Execution (no second resolution)

## Files to Change

| File | Change | PR |
|------|--------|-----|
| `mcp/noun_index.rs` | Add `extract_with_exclusions()` method | PR 1 |
| `entity_linking/resolver.rs` | Add `extract_mention_spans()` to trait | PR 1 |
| `lookup/service.rs` | Reorder: mentions → mask → verb search | PR 2 |
| `mcp/verb_search.rs` | Accept entity exclusion spans parameter | PR 2 |
| `agent/orchestrator.rs` | Wire entity resolution BEFORE verb search | PR 3 |
| `api/agent_service.rs` | Handle ClarifyEntity before verb disambiguation | PR 3 |

## Entity UUID Persistence

Once resolved, the entity UUID is stored in:
- `session.context.resolved_entities: HashMap<String, ResolvedEntity>`
- Each `ResolvedEntity` carries: `uuid`, `canonical_name`, `entity_kind`, `constellation_slot`

Subsequent utterances in the same session can reference the entity
by name without re-resolution: "now screen them" → "them" = last
resolved entity UUID.

## What This Enables

1. **No double lookup** — entity resolved once, UUID reused everywhere
2. **No resolution modal** — entity resolved before DSL generation
3. **Entity-kind verb constraining** — "HSBC" is a group → only group verbs
4. **Constellation grounding** — UUID → specific slot → state machine → valid actions
5. **ECIR accuracy** — entity names masked → no noun scan pollution
6. **Pronoun resolution** — "screen them" → last resolved entity

## Risk Assessment

| Risk | Level | Mitigation |
|------|-------|------------|
| Entity snapshot not loaded | Low | Returns empty spans → ECIR runs normally |
| Over-masking (entity name contains noun) | Low | Longest-match-first: "Goldman Sachs Group" masks all 3 words, "group" doesn't leak |
| Ambiguous resolution adds a conversation turn | Medium | Only fires when genuinely ambiguous; clear hits are silent |
| New entities not in snapshot | None | No match → no masking → prompt for creation if verb needs entity |
| Performance | None | MentionExtractor p95 < 5ms (in-memory) |

## Implementation Order

**PR 1** (Small, no behavioral change):
- `NounIndex.extract_with_exclusions()` with unit tests
- `EntityLinkingService.extract_mention_spans()` trait method
- `ResolvedEntity` struct in session types

**PR 2** (Medium, internal reorder):
- `LookupService.analyze()` reordered: mentions → mask → search
- `HybridVerbSearcher.search()` accepts optional exclusion spans
- All search() callers updated with `None` default

**PR 3** (Medium, user-facing):
- Orchestrator runs entity extraction at Stage 0
- ClarifyEntity decision returned before verb disambiguation
- Entity UUID stored in session, reused on subsequent turns
- Integration test: "open a KYC case for HSBC Holdings plc"

## Success Metrics

| Metric | Before | Target |
|--------|--------|--------|
| ECIR false positives from entity names | 7+ | 0 |
| Entity resolution lookups per utterance | 2 (linker + DSL compiler) | 1 |
| Resolution modal frequency | Every entity reference | Only when ambiguous |
| Embedding tier accuracy | 49.5% | +5% from better ECIR masking |

## Not In Scope

- Changing the entity snapshot format
- Adding new entity types to the snapshot
- Entity relationship resolution ("HSBC's custodian")
- Cross-session entity memory (already exists in session context)

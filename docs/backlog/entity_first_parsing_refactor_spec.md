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

## Proposed Pipeline: Co-Resolution (Entity + Verb Mutually Constrain)

The key insight: the entity name and the verb context are **correlated
clues**. "Open a KYC case for HSBC" — "KYC case" hints that HSBC is
probably a CBU (you open cases for CBUs, not groups). "Trace the
ownership chain for HSBC" — "ownership chain" hints group.

The parser must use BOTH signals simultaneously, not sequentially.

```
Utterance: "open a KYC case for HSBC"
    │
    ▼
Stage 0: Parallel Extraction (both run, neither commits)
    │
    ├─ Entity Mention Extraction (fast, in-memory)
    │   MentionExtractor.extract() against EntitySnapshot
    │   "HSBC" → 3 candidates:
    │     HSBC Holdings plc (group, 0.92)
    │     HSBC Bank plc (company, 0.88)
    │     HSBC Custody Services (cbu, 0.85)
    │
    └─ ECIR Noun Scan (masked by entity spans)
        NounIndex.extract_with_exclusions(utterance, entity_spans)
        "KYC case" → noun=kyc-case, action=open → kyc-case.create
        Verb subject_kinds = [cbu]  ← THIS IS THE HINT
    │
    ▼
Stage 1: Co-Resolution (entity + verb constrain each other)
    │
    │ Verb context says: subject_kind = [cbu]
    │ Entity candidates: group(0.92), company(0.88), cbu(0.85)
    │
    │ Re-rank by verb hint:
    │   HSBC Custody Services (cbu) → boosted by verb match (+0.15)
    │   HSBC Holdings plc (group) → penalised (kind mismatch)
    │   HSBC Bank plc (company) → penalised (kind mismatch)
    │
    │ After re-ranking:
    │   HSBC Custody Services (cbu, 1.00) ← now dominant
    │
    ├─ CLEAR (dominant candidate after re-ranking)
    │   → UUID resolved silently, continue pipeline
    │
    ├─ STILL AMBIGUOUS (re-ranking didn't resolve)
    │   → Show candidates to user WITH verb context:
    │     "Which HSBC for this KYC case?"
    │     1. HSBC Custody Services (CBU) ← recommended
    │     2. HSBC Holdings plc (group)
    │     3. HSBC Bank plc (company)
    │   → User picks → resume
    │
    └─ NO ENTITY MATCH
        → Continue without entity context
    │
    ▼
Stage 2: Verb Resolution (entity-kind locked)
    │ Entity kind = cbu (from co-resolution)
    │ Verb candidates filtered: only cbu-applicable verbs
    │ kyc-case.create confirmed (subject_kinds includes cbu)
    │
    ▼
Stage 3: SemOS Grounding
    │ Subject = uuid (HSBC Custody Services)
    │ Constellation: kyc.onboarding
    │ State: entity_workstream state for this CBU
    │ Valid actions from GroundedActionSurface
    │
    ▼
Stage 4: DSL Assembly
    │ (kyc-case.create :cbu-id "uuid-789")
    │ Entity UUID pre-resolved. No placeholder.
    │
    ▼
Stage 5: Confirm + Execute
    │ UUID in runbook → direct DB operation
```

### The Co-Resolution Algorithm

```
fn co_resolve(entity_candidates, verb_candidates) → (entity, verb):

    1. If no entity candidates → use verb alone (existing behavior)
    2. If no verb candidates → use entity alone (kind → default verbs)
    3. If both present:
       a. For each entity candidate, score verb compatibility:
          - verb.subject_kinds contains entity.kind → +0.15 boost
          - verb.subject_kinds empty (global) → no change
          - verb.subject_kinds doesn't contain entity.kind → -0.10 penalty
       b. For each verb candidate, score entity compatibility:
          - entity.kind matches verb.subject_kinds → +0.10 boost
       c. Re-rank both lists
       d. If entity now has clear winner → resolve silently
       e. If still ambiguous → show options with verb context hint
```

### Contextual Clues from the Utterance

The parser extracts hints beyond just the first word:

| Utterance Fragment | Hint | Entity Kind Boost |
|---|---|---|
| "KYC case for X" | KYC case → subject_kinds=[cbu] | Boost cbu entities |
| "ownership chain for X" | ownership → subject_kinds=[group,entity] | Boost group entities |
| "screening for X" | screening → subject_kinds=[entity] | Boost person/company entities |
| "assign X as depositary" | depositary role → kind=company | Boost company entities |
| "trading profile for X" | trading → subject_kinds=[cbu] | Boost cbu entities |
| "board of X" | board → subject_kinds=[company] | Boost company entities |
| "shares in X" | capital → subject_kinds=[fund] | Boost fund entities |

These hints come from the ECIR noun scan (Stage 0) — the domain noun
tells us what kind of entity is expected. The verb's `subject_kinds`
declaration provides the ground truth for re-ranking.
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

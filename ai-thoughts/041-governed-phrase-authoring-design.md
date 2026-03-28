# ADR 041: Governed Phrase Authoring — SemOS Learning Feedback Loop

> **Status:** Design Brief — Ready for Architecture Review
> **Date:** 2026-03-28
> **Context:** The phrase bank (24,025 patterns) is the primary driver of utterance hit rate (62% first-attempt). Phrase maintenance is currently a developer workflow (YAML edit → compile → embed). It should be a SemOS-governed, agent-assisted capability with a closed feedback loop.

---

## Problem

Three disconnected systems handle phrase-related data:

1. **YAML invocation_phrases** — developer-authored, compiled to `dsl_verbs.yaml_intent_patterns`
2. **Learning loop** — `invocation_phrases` table + `learning_candidates` (agent.teach/unteach) — direct DB writes, no governance
3. **SemOS verb contracts** — `sem_reg.snapshots` with `object_type = 'verb_contract'` — governed, immutable

Phrases added via the learning loop bypass SemOS governance entirely. They can introduce collisions, drift from domain vocabulary, or shadow better-fit verbs — all the problems we just spent a session cleaning up.

## Desired State

**All phrase mutations flow through SemOS.** The learning loop feeds INTO SemOS governance, not around it. The feedback loop:

```
utterance miss → agent observes → proposes phrase (grounded in context)
    → changeset → collision check → review → publish
    → materialized to operational phrase bank → better match next time
```

## Key Design Requirements

### 1. Phrase proposals must be grounded, not random

A phrase proposal needs context:
- **Workspace** — which workspace was the user in?
- **Pack** — which pack was active?
- **Constellation state** — what entity type, what lifecycle state?
- **Domain** — which verb domain is the target?
- **Miss details** — what the user typed, what verb was expected, what was matched instead

This context determines whether a phrase is valid. "close it out" is valid for `kyc-case.close` when in KYC workspace with a case in review state. It's noise in a Deal workspace.

### 2. Collision detection at proposal time

Before accepting a phrase, the system must check:
- Does this exact phrase already exist on another verb?
- Does this phrase semantically overlap with existing phrases on competing verbs?
- Would adding this phrase create a new shadowing problem?

The collision audit we ran (74 found, all resolved) should be automated and run on every phrase proposal.

### 3. SemOS governance for phrase lifecycle

Phrases should follow the same governance pipeline as other SemOS objects:
- **Draft** — proposed by agent or human, not yet active
- **Review** — collision check passed, awaiting stewardship approval
- **Active** — published, materialized to operational phrase bank
- **Deprecated** — replaced or found to cause collisions

This maps to a new SemOS object type or an extension of `verb_contract` snapshots.

### 4. Agent (Sage) as phrase proposer

The agent should:
- Detect utterance misses in real time (pipeline returns NoMatch or wrong verb)
- Generate candidate phrases grounded in the session context
- Score candidates against existing phrases for collision risk
- Submit proposals through the SemOS changeset pipeline
- Learn from reviewer feedback (approved/rejected) to improve future proposals

### 5. Materialization to operational phrase bank

When a phrase is published in SemOS:
- Trigger materializes it to `dsl_verbs.yaml_intent_patterns` or `invocation_phrases` table
- Embeddings are auto-generated (populate_embeddings or inline Candle embed)
- The verb search index picks it up on next query (or hot-reload)

## Architecture Sketch

```
User utterance → REPL pipeline → miss detected
    ↓
Agent (Sage) observes miss context:
  - workspace, pack, constellation state
  - utterance text, expected verb (if known), matched verb (if any)
  - entity type, lifecycle state
    ↓
Sage generates phrase candidates:
  - grounded in domain vocabulary
  - collision-checked against phrase bank
  - scored by semantic similarity to target verb's existing phrases
    ↓
Phrase proposal → SemOS changeset:
  - object_type: 'phrase_mapping' (new) or extension of 'verb_contract'
  - definition: { verb_fqn, phrase, context: { workspace, pack, entity_kind, state } }
  - collision_report: { checked_against: N phrases, conflicts: [] }
    ↓
Governance pipeline:
  - Auto-validate: collision check, domain alignment
  - Stewardship review: human approves/rejects with rationale
  - Publish: materializes to operational phrase bank
    ↓
Phrase active → next utterance match improves
```

## Verb Coverage for This Capability

### Existing (already wired):
- `agent.teach` — direct phrase learning (needs governance wrapping)
- `agent.unteach` — phrase removal
- `changeset.compose` / `changeset.add-item` — changeset authoring
- `governance.publish` — publish pipeline

### New verbs needed:
- `phrase.propose` — agent-generated phrase proposal with context
- `phrase.check-collisions` — collision audit for a candidate phrase
- `phrase.list-candidates` — show pending phrase proposals
- `phrase.batch-propose` — bulk phrase proposals from miss analysis

### Constellation integration:
- Add `phrase_authoring` slot to `registry.stewardship` constellation map
- State machine: `proposed → collision_checked → reviewed → published | rejected`
- Verb gates: propose (always), check-collisions (after propose), publish (after review)

## What NOT to do

- Don't let phrases bypass SemOS — no direct DB writes to invocation_phrases
- Don't accept phrases without context — workspace/domain/state must be captured
- Don't auto-publish without collision check — every phrase must be validated
- Don't treat all misses as phrase candidates — some misses are genuinely out of scope

## Dependencies

- SemOS Maintenance workspace (done)
- Phrase bank collision audit (done — 74 resolved to 0)
- Agent miss detection in REPL pipeline (partially exists in learning infrastructure)
- New SemOS object type or verb_contract extension for phrase mappings

## Estimated Scope

| Component | Effort | Priority |
|-----------|--------|----------|
| SemOS phrase object type | Medium | P1 |
| Collision detection service | Small | P1 |
| Phrase materialization trigger | Small | P1 |
| Agent miss observation hook | Medium | P2 |
| Sage phrase proposal generation | Medium | P2 |
| Governance pipeline integration | Small | P2 (reuses existing) |
| Hot-reload for published phrases | Medium | P3 |
| Batch miss analysis tooling | Small | P3 |

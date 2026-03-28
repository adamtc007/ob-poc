# Governed Phrase Authoring — Vision, Scope & Capability

> **Version:** 1.0
> **Date:** 2026-03-28
> **Status:** Peer Review Draft — LLM Sandbox Model
> **ADR:** `ai-thoughts/041-governed-phrase-authoring-design.md`
> **Audience:** Engineering, Architecture, Product

---

## 1. Executive Summary

The utterance-to-REPL pipeline resolves natural language to deterministic DSL verbs through a 7-tier resolution chain. The primary driver of accuracy is the **phrase bank** — 24,025 invocation patterns mapped to 1,433 verbs, delivering 62% first-attempt and 85% two-attempt hit rates.

Phrase maintenance is currently a developer workflow (YAML edit → compile → embed). This document proposes **governed phrase authoring** — a SemOS-native capability where an AI agent observes session utterance misses, proposes context-grounded phrase additions, validates against the existing phrase bank for collisions, and submits proposals through the SemOS governance pipeline for human review.

**The core proposition:** AI handles breadth and depth (24,000 phrases, 7 workspaces, 17 state machines, thousands of sessions). Humans handle domain judgement (approve, reject, refine). SemOS governs the lifecycle.

---

## 2. Problem Statement

### 2.1 Current State

| Component | Location | Governance |
|-----------|----------|------------|
| Invocation phrases | `config/verbs/**/*.yaml` | None — developer YAML edit |
| Learned phrases | `invocation_phrases` table | None — direct DB write via `agent.teach` |
| SemOS verb contracts | `sem_reg.snapshots` | Full — changeset → review → publish |

Three disconnected systems. Phrases added via the learning loop bypass SemOS governance. They can introduce collisions (74 found and resolved manually in the 2026-03-28 cleanup), drift from domain vocabulary, or shadow better-fit verbs.

### 2.2 What's Missing

1. **No feedback loop** — utterance misses are logged but not analysed or acted upon
2. **No collision detection** — phrase additions aren't checked against 24,000 existing patterns
3. **No context grounding** — phrases are added without reference to workspace, constellation state, or entity lifecycle
4. **No governance** — the teach/learn path writes directly to DB, bypassing SemOS
5. **No AI assistance** — humans must manually identify which phrases to add, a task that requires holding 1,433 verbs and 24,000 patterns in working memory

---

## 3. Vision

### 3.1 Architectural Invariant

**All paths lead to SemOS.** Phrase mutations are no exception. Every phrase addition, modification, or deprecation flows through SemOS governance — changeset → collision check → review → publish → materialization.

### 3.2 The Feedback Loop

```
SESSION RUNTIME                          SEMOS GOVERNANCE
─────────────────                        ────────────────────
User utterance
    ↓
REPL pipeline (7-tier resolution)
    ↓
Match / Miss / Wrong Match
    ↓
Session trace (append-only log)
    ↓
    ════════════════════════════════════════════════════
    ↓
AI OBSERVATION PHASE
    ↓
Agent trawls session traces
Identifies miss patterns
Groups by frequency, context
    ↓
AI ANALYSIS PHASE
    ↓
Collision detection (24k phrases)
State machine validation
Constellation grounding
    ↓
AI PROPOSAL PHASE
    ↓
Structured proposal with evidence
    ↓
                                         Changeset created
                                             ↓
                                         Collision gate (automated)
                                             ↓
                                         HIL review (approve/reject/refine)
                                             ↓
                                         Publish
                                             ↓
                                         Materialization trigger
    ↓                                        ↓
Phrase active in bank ←──────────────── Operational phrase bank updated
    ↓
Next session: better match
```

### 3.3 AI Proposes, Human Decides

The agent handles what humans cannot:
- Cross-session pattern recognition (thousands of traces)
- Collision detection against 24,000+ phrases
- Context grounding across 7 workspaces × 17 state machines × 1,433 verbs
- Semantic similarity scoring against competing verbs

The human handles what AI should not decide alone:
- Domain vocabulary correctness ("we say 'redemption', not 'buyback'")
- Regulatory sensitivity ("this phrase implies a commitment we can't make")
- Organisational convention ("our firm calls this a 'mandate', not a 'deal'")

---

## 4. Capability Model

### 4.1 Session Trace Observation

**Input:** `session_traces` table (append-only mutation log)

Each trace entry carries:
- `session_id`, `sequence`, `timestamp`
- `operation` (utterance, verb_match, execution, error)
- `payload` (utterance text, matched verb, confidence, outcome)
- `stack_snapshot` (workspace, pack, constellation_family, entity_state)

The agent queries traces for:
- **NoMatch outcomes** — user typed something, nothing matched
- **Rejections** — user rejected the proposed verb
- **Rephrasings** — user tried again with different words (consecutive utterances to same verb)
- **Low-confidence matches** — verb matched but below strong threshold (< 0.70)
- **Clarification cycles** — 2+ turns to resolve one intent

### 4.2 Pattern Analysis

**Grouping strategies:**

| Strategy | Signal | Example |
|----------|--------|---------|
| **Frequency** | Same utterance across N sessions | "pull the cap table" missed 12 times last week |
| **Semantic clustering** | Similar utterances (embedding distance < 0.15) | "close it out", "close this case", "shut the case" |
| **State-specific** | Miss only occurs in specific constellation state | "approve" misses only in KYC review state |
| **Workspace-specific** | Miss only occurs in one workspace | "fee schedule" misses only in Deal workspace |
| **Rephrase chain** | User rephrased → what they typed next matched | "process redemption" → rephrased to "redeem shares" → matched |

**Collision analysis:**

For each candidate phrase:
1. Exact match check against all 24,000+ patterns
2. Semantic similarity check (cosine > 0.90 with patterns on different verbs = collision risk)
3. Domain prefix check (phrase contains a domain keyword mapped to a different verb)
4. Historical collision check (was this phrase previously removed for causing shadowing?)

### 4.3 Proposal Generation

**Proposal structure:**

```json
{
  "phrase": "close it out",
  "target_verb": "kyc-case.close",
  "confidence": 0.92,
  "context": {
    "workspace": "kyc",
    "pack": "kyc-case",
    "constellation_state": "review",
    "entity_type": "case",
    "stack_depth": 1
  },
  "evidence": {
    "miss_count": 7,
    "unique_sessions": 5,
    "time_range": "2026-03-20 to 2026-03-28",
    "rephrase_to": "close the kyc case",
    "rejection_count": 0
  },
  "collision_report": {
    "checked_against": 24025,
    "exact_conflicts": [],
    "semantic_near_misses": [
      {"phrase": "close kyc case", "verb": "kyc-case.close", "sim": 0.92, "safe": true}
    ],
    "shadowing_risk": "none"
  },
  "proposal_rationale": "7 users typed 'close it out' in KYC workspace at review state. All rephrased to 'close the kyc case' which matched. Adding this phrase would save a clarification turn."
}
```

### 4.4 Governance Pipeline

**SemOS object:** New object type `phrase_mapping` or extension of `verb_contract`

**Lifecycle states:**

```
proposed → collision_checked → reviewed → published | rejected
                                  ↑
                                  └── refined (human modifies phrase)
```

**Automated gates:**
- Collision check passes (zero exact conflicts, zero high-risk semantic overlaps)
- Target verb exists in SemOS as active verb_contract
- Target verb is valid in the proposed workspace/constellation context
- Phrase doesn't contain PII, profanity, or regulatory-sensitive language

**Human gates:**
- Domain vocabulary correctness
- Organisational convention alignment
- Regulatory sensitivity review (for compliance-adjacent verbs)

### 4.5 Materialization

When a phrase is published:

1. **Trigger on `sem_reg.snapshots` INSERT** (same pattern as attribute materialization)
2. Extracts phrase + verb FQN from definition
3. UPSERTs to `dsl_verbs.yaml_intent_patterns` (operational phrase bank)
4. Computes embedding via Candle BGE-small-en-v1.5
5. INSERTs to `verb_pattern_embeddings` with new vector
6. Updates `verb_centroids` for the affected verb

**Hot reload:** The REPL's exact phrase match (Tier 0) queries the DB on every turn. Published phrases are immediately available without server restart.

---

## 5. DSL Verb Surface

### 5.1 Existing Verbs (to be governance-wrapped)

| Verb | Current Behaviour | Governed Behaviour |
|------|------------------|-------------------|
| `agent.teach` | Direct DB write to `invocation_phrases` | Submit to SemOS changeset instead |
| `agent.unteach` | Direct DB delete | Deprecate in SemOS instead |

### 5.2 New Verbs

| Verb | Type | Purpose |
|------|------|---------|
| `phrase.observe-misses` | plugin | Trawl session traces for miss patterns |
| `phrase.analyse-candidates` | plugin | Run collision detection + context grounding on miss patterns |
| `phrase.propose` | plugin | Generate structured proposal with evidence |
| `phrase.batch-propose` | plugin | Bulk proposals from aggregated miss analysis |
| `phrase.review-proposals` | plugin | List pending proposals with context + collision reports |
| `phrase.approve` | plugin | Accept proposal → changeset → publish |
| `phrase.reject` | plugin | Reject with reason → agent learns |
| `phrase.check-collisions` | plugin | Standalone collision audit for a candidate phrase |
| `phrase.coverage-report` | plugin | Per-workspace phrase coverage and gap analysis |

### 5.3 Constellation Integration

Add `phrase_authoring` slot to `registry.stewardship` constellation map:

```yaml
phrase_authoring:
  type: entity
  entity_kinds: [phrase]
  cardinality: optional
  state_machine: phrase_authoring_lifecycle
  verbs:
    observe: { verb: phrase.observe-misses, when: [empty, filled] }
    analyse: { verb: phrase.analyse-candidates, when: filled }
    propose: { verb: phrase.propose, when: filled }
    review: { verb: phrase.review-proposals, when: filled }
    approve: { verb: phrase.approve, when: filled }
    reject: { verb: phrase.reject, when: filled }
    coverage: { verb: phrase.coverage-report, when: [empty, filled] }
```

**State machine:** `phrase_authoring_lifecycle`

```
observed → analysed → proposed → collision_checked → reviewed → published | rejected
```

---

## 6. Integration Points

### 6.1 Session Trace (Input)

- **Table:** `session_traces`
- **Key fields:** `session_id`, `sequence`, `operation`, `payload`, `stack_snapshot`
- **Query pattern:** Aggregate misses by utterance text, group by workspace/state

### 6.2 Phrase Bank (Target)

- **Operational:** `dsl_verbs.yaml_intent_patterns` (text array column)
- **Embeddings:** `verb_pattern_embeddings` (pgvector 384-dim)
- **Centroids:** `verb_centroids` (per-verb centroid for shortlisting)

### 6.3 SemOS Registry (Governance)

- **Snapshots:** `sem_reg.snapshots` with `object_type = 'phrase_mapping'`
- **Changesets:** Existing changeset pipeline
- **Materialization:** AFTER INSERT trigger (same pattern as attribute materialization)

### 6.4 Learning Infrastructure (Migration)

- **Current:** `invocation_phrases` table + `learning_candidates` table
- **Migration:** Wrap `agent.teach` to submit through SemOS governance instead of direct write
- **Backward compat:** Existing learned phrases remain operational; new ones go through governance

### 6.5 REPL Pipeline (Consumer)

- **Tier 0:** Exact phrase match queries `dsl_verbs.yaml_intent_patterns` — picks up published phrases immediately
- **Tier 3:** Semantic search queries `verb_pattern_embeddings` — picks up new embeddings after materialization
- **No server restart required** — both paths query DB on each turn

---

## 7. AI Model Requirements

### 7.1 Pattern Recognition (Observation Phase)

- Session trace aggregation (SQL + application logic)
- Utterance clustering (BGE embeddings + cosine similarity for grouping)
- Rephrase chain detection (consecutive utterances by same user to same final verb)
- No external LLM required — deterministic analysis on structured data

### 7.2 Collision Detection (Analysis Phase)

- Exact string match (trivial)
- Semantic similarity scoring (Candle BGE, same model as phrase bank)
- Historical collision lookup (was this phrase previously removed?)
- No external LLM required — uses existing embedding infrastructure

### 7.3 Proposal Generation (Propose Phase)

- Structured output (JSON proposal with evidence)
- Rationale generation (natural language explanation of why this phrase should be added)
- **This is the one phase that benefits from LLM** — generating the `proposal_rationale` field and suggesting alternative phrasings
- Can use the existing Sage agent integration (Claude API)

### 7.4 Sandbox Consideration

The observation and analysis phases can run entirely in a **sandbox** — no external API calls, no LLM inference. They process structured data (session traces, phrase bank) with deterministic algorithms (SQL aggregation, cosine similarity, string matching).

The proposal phase can optionally use LLM for rationale generation, but the core proposal (phrase + verb + context + evidence) is deterministic. The LLM adds explanatory text, not decision logic.

This means the capability can be developed and tested without LLM dependency, with LLM-assisted rationale as an optional enhancement.

---

## 8. Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| First-attempt hit rate | 62.4% | 75%+ | `intent_hit_rate` test suite |
| Two-attempt hit rate | 84.9% | 92%+ | `intent_hit_rate` test suite |
| Phrase collisions | 0 (after manual cleanup) | 0 (automated) | Collision audit in CI |
| Miss-to-proposal latency | N/A (manual) | < 24 hours | Automated observation pipeline |
| Proposal acceptance rate | N/A | > 70% | Governance metrics |
| Time to first match after miss | N/A (days/weeks) | < 1 hour after publish | Session trace analysis |

---

## 9. Phased Delivery

### Phase 1: Observation Infrastructure (P1)

- `phrase.observe-misses` verb — trawl session traces for miss patterns
- `phrase.coverage-report` verb — per-workspace gap analysis
- SQL views over `session_traces` for miss aggregation
- No governance integration yet — diagnostic only

### Phase 2: Collision Detection Service (P1)

- `phrase.check-collisions` verb — standalone collision check
- Automated collision audit in CI (`scripts/lint_phrase_collisions.sh`)
- Integration with existing phrase bank (24,025 patterns)

### Phase 3: SemOS Phrase Object Type (P2)

- New `phrase_mapping` object type in SemOS
- Materialization trigger (snapshot → phrase bank)
- Wrap `agent.teach` to submit through SemOS governance
- State machine: `phrase_authoring_lifecycle`

### Phase 4: AI Proposal Pipeline (P2)

- `phrase.propose` and `phrase.batch-propose` verbs
- Pattern analysis (frequency, clustering, rephrase chains)
- Context grounding (workspace, constellation state, entity type)
- Evidence assembly (miss count, sessions, time range)

### Phase 5: HIL Review Surface (P3)

- `phrase.review-proposals`, `phrase.approve`, `phrase.reject` verbs
- SemOS Maintenance workspace integration (phrase_authoring slot)
- Rejection feedback loop (agent learns from rejections)

### Phase 6: LLM-Assisted Rationale (P3, Optional)

- Sage agent generates `proposal_rationale` natural language
- Alternative phrasing suggestions
- Domain vocabulary alignment scoring

---

## 10. Non-Goals

- **Real-time phrase learning** — proposals go through governance, not instant activation
- **Unsupervised phrase addition** — AI proposes, human decides. Always.
- **Multi-language support** — English only for v1.0
- **Phrase deletion automation** — deprecation requires human review
- **Custom embedder per workspace** — single BGE model for all workspaces

---

## 11. Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| SemOS Maintenance workspace | Done | ScopeGate fork, constellation, pack |
| Session trace infrastructure | Done | `session_traces` table, TraceEntry, TraceOp |
| Phrase bank (24,025 patterns) | Done | Zero collisions after 2026-03-28 cleanup |
| Collision audit tooling | Done | Python script, can be converted to verb |
| Materialization trigger pattern | Done | `trg_materialize_attribute_def` — reuse pattern |
| Candle BGE embedder | Done | Same model for proposal embedding |
| SemOS governance pipeline | Done | Changeset → review → publish |
| Sage agent integration | Done | Claude API for rationale generation |

---

## 12. Open Questions

1. **Object type:** New `phrase_mapping` vs extension of `verb_contract`? A separate type is cleaner but adds schema. An extension keeps the object count down but muddies the verb_contract semantics.

2. **Observation frequency:** Real-time (per-session) vs batch (hourly/daily)? Batch is simpler and avoids noise from single-session anomalies.

3. **Auto-publish threshold:** Should high-confidence proposals (10+ misses, zero collisions, same verb every time) skip human review? Probably not for v1.0 — trust must be earned.

4. **Phrase expiry:** Should phrases have a TTL? If a phrase hasn't matched in 90 days, should it be flagged for review? Prevents phrase bank bloat.

5. **Cross-workspace phrases:** A phrase valid in KYC workspace might shadow a different verb in Deal workspace. Should proposals be workspace-scoped or global? Context-gated proposals (workspace + state) are safer.

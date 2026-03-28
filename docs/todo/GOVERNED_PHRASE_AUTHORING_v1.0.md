# Governed Phrase Authoring — Vision, Scope & Capability

> **Version:** 1.2
> **Date:** 2026-03-28
> **Status:** Implementation-Ready Draft
> **ADR:** `ai-thoughts/041-governed-phrase-authoring-design.md`
> **Audience:** Engineering, Architecture, Product
>
> **Changelog (v1.2):** Single canonical materialization target committed — `phrase_bank` is the sole operational store, `dsl_verbs.yaml_intent_patterns` eliminated as materialization target (§4.5, §6.2). Phase sequencing fixed — `phrase_bank` schema creation moved to Phase 2, legacy migration follows in Phase 2.5 (§9). `constellation_state` removed from v1.0 schema and lookup — workspace-only scoping for v1; state scoping deferred to v1.1 with clear prerequisites (§4.5, §12 Decision 6). Uniqueness constraint tightened to `(phrase, workspace)` — at most one active winner per phrase per workspace (§4.5.1). Explicit phrase precedence order defined (§4.5.2). Wrong-match elevated to first-class observation signal with priority weighting in confidence score (§4.1, §4.3.1). Risk-weighted confidence — verb sensitivity class introduces approval tiers (§4.3.2). Quality gate token-count rule made context-aware (§4.2.1). Review surface design moved to Phase 3 (§9). Expiry telemetry explicitly separated from hot-path reads (§12 Decision 4).
>
> **Changelog (v1.1):** Context-scoped phrase materialization resolved; FSM backward arcs and `deferred` state added; batch observation with watermark committed; `agent.teach` migration path made concrete; confidence score semantics defined; phrase quality gates added; throughput model added to success metrics; minor fixes: PII gate scoped to v1.0 capability, terminology overload resolved, `phrase.batch-propose` semantics specified.

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
6. **No phrase quality gates** — no automated check for overly generic, overly specific, or domain-inappropriate phrases
7. **No wrong-match detection** — phrases that consistently resolve to the wrong verb are not identified or prioritised for correction

---

## 3. Vision

### 3.1 Architectural Invariant

**All paths lead to SemOS.** Phrase mutations are no exception. Every phrase addition, modification, or deprecation flows through SemOS governance — changeset → collision check → quality gate → review → publish → materialization.

**Corollary:** No phrase persists outside SemOS governance. Existing `invocation_phrases` entries written via `agent.teach` will be migrated into governed `phrase_mapping` objects (see §9, Phase 2.5). The direct-write path will be deprecated once migration completes.

**Single operational store (v1.2 invariant):** `phrase_bank` is the sole materialization target for all governed phrases. The legacy `dsl_verbs.yaml_intent_patterns` text array is consumed read-only as a YAML-sourced fallback during migration. It is never written to by the governance pipeline. Once YAML phrases are migrated into `phrase_bank` (Phase 6, post-v1.0), `yaml_intent_patterns` becomes vestigial.

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
AI OBSERVATION PHASE (batch, daily, watermarked)
    ↓
Agent trawls session traces
  from last_observed_sequence watermark
Identifies miss patterns AND wrong-match patterns
Groups by frequency, context, harm severity
    ↓
AI ANALYSIS PHASE
    ↓
Quality gate (specificity, ambiguity)
Collision detection (24k phrases)
  — cross-workspace collision check
State machine validation
Constellation grounding
Risk classification (verb sensitivity)
    ↓
AI PROPOSAL PHASE
    ↓
Structured proposal with evidence
  + risk tier assignment
    ↓
                                         Changeset created
                                             ↓
                                         Collision gate (automated)
                                             ↓
                                         Quality gate (automated)
                                             ↓
                                         HIL review (approve/reject/refine)
                                           — risk-tiered review queue
                                             ↓
                                         Publish
                                             ↓
                                         Materialization trigger
    ↓                                        ↓
Phrase active in phrase_bank ←───────── phrase_bank updated
    ↓                                    (workspace-qualified entry)
Next session: better match
```

### 3.3 AI Proposes, Human Decides

The agent handles what humans cannot:
- Cross-session pattern recognition (thousands of traces)
- Wrong-match detection and harm prioritisation
- Collision detection against 24,000+ phrases across all workspaces
- Context grounding across 7 workspaces × 17 state machines × 1,433 verbs
- Semantic similarity scoring against competing verbs
- Phrase quality screening (specificity, ambiguity, domain fit)
- Risk classification based on target verb sensitivity

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

The agent queries traces for six signal types, classified into two priority tiers:

**Priority 1 — Wrong-match signals (harmful; phrase resolves to wrong verb):**
- **Wrong-match → rejection** — verb matched, user explicitly rejected, then rephrased to a different verb
- **Wrong-match → undo** — verb executed, user immediately undid the action (indicates wrong verb was invoked)
- **Wrong-match → error cascade** — verb executed, triggered a downstream error (state machine violation, constraint failure)

**Priority 2 — Miss signals (inconvenient; no verb resolved):**
- **NoMatch outcomes** — user typed something, nothing matched
- **Rephrasings** — user tried again with different words (consecutive utterances to same verb)
- **Low-confidence matches** — verb matched but below strong threshold (< 0.70)
- **Clarification cycles** — 2+ turns to resolve one intent

**Why wrong-matches are Priority 1:** A phrase that consistently maps to the wrong verb is more damaging than one that simply misses. A miss costs the user a clarification turn. A wrong-match can trigger an incorrect state transition, corrupt data, or force an undo — in compliance-sensitive workspaces, this may be irreversible. Wrong-match signals are therefore weighted higher in confidence scoring (§4.3.1) and surfaced first in the review queue (§4.3.2).

#### 4.1.1 Observation Frequency — Batch with Watermark

**Decision:** Batch observation, daily cadence.

The `session_traces` table is append-only. Per-session observation would require O(n) aggregation scans growing linearly with platform usage. A daily batch job avoids this:

- Agent maintains a persistent `last_observed_sequence` watermark in `phrase_observation_state`
- Each batch run queries `WHERE sequence > last_observed_sequence`
- On completion, watermark advances to the max sequence processed
- Single-session anomalies are filtered by requiring `miss_count >= 3` across `unique_sessions >= 2` before a pattern qualifies for analysis
- Wrong-match patterns have a lower qualifying threshold: `wrong_match_count >= 2` across `unique_sessions >= 1` (even a single-session wrong-match on a sensitive verb warrants investigation)

**Schema:**

```sql
CREATE TABLE phrase_observation_state (
    id            SERIAL PRIMARY KEY,
    last_observed_sequence BIGINT NOT NULL DEFAULT 0,
    last_run_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    patterns_found INT NOT NULL DEFAULT 0,
    wrong_match_patterns_found INT NOT NULL DEFAULT 0,
    next_run_at   TIMESTAMPTZ  -- optional: schedule hint
);
```

### 4.2 Pattern Analysis

**Grouping strategies:**

| Strategy | Signal | Example |
|----------|--------|---------|
| **Frequency** | Same utterance across N sessions | "pull the cap table" missed 12 times last week |
| **Semantic clustering** | Similar utterances (embedding distance < 0.15) | "close it out", "close this case", "shut the case" |
| **State-specific** | Miss only occurs in specific constellation state | "approve" misses only in KYC review state |
| **Workspace-specific** | Miss only occurs in one workspace | "fee schedule" misses only in Deal workspace |
| **Rephrase chain** | User rephrased → what they typed next matched | "process redemption" → rephrased to "redeem shares" → matched |
| **Wrong-match chain** | Phrase matched verb A, user rejected/undid, then matched verb B | "close it" → matched `deal.close` → rejected → rephrased to "close the case" → matched `kyc-case.close` |

**Collision analysis:**

For each candidate phrase:
1. Exact match check against all 24,000+ patterns
2. Semantic similarity check (cosine > 0.90 with patterns on different verbs = collision risk)
3. Domain prefix check (phrase contains a domain keyword mapped to a different verb)
4. Historical collision check (was this phrase previously removed for causing shadowing?)
5. **Cross-workspace collision check** — candidate phrase tested against all workspaces, not just the source workspace. If the same phrase maps to different verbs in different workspaces, this is flagged as a context-scoped candidate requiring workspace qualification (see §4.5)

#### 4.2.1 Phrase Quality Gates

Beyond collision detection, candidate phrases pass through automated quality screening before reaching human review. These gates catch low-value or hazardous phrases early:

| Gate | Rule | Rationale |
|------|------|-----------|
| **Minimum token count** | Reject phrases with fewer than 3 tokens **unless** the phrase is workspace-scoped AND has zero cross-verb collisions (cosine < 0.70 to all other verbs in that workspace) | Overly generic short phrases ("do it", "run") are ambiguous, but workspace-scoped domain terms ("redeem", "freeze", "cap table") are valid when collision-free in their workspace |
| **Cross-verb ambiguity** | Reject if cosine similarity > 0.85 to phrases on 2+ other verbs | A phrase that's semantically close to multiple verbs will cause more confusion than it resolves |
| **Single-user idiolect** | Flag (not reject) if all misses originate from a single user | May be idiosyncratic phrasing that doesn't generalise; human reviewer decides |
| **Stop-word ratio** | Reject if > 60% of tokens are stop words | "do the thing with the" carries no discriminating semantic content |
| **Historical rejection** | Flag if a semantically similar phrase (cosine > 0.90) was previously rejected | Avoids re-proposing variants of phrases the domain expert already declined |

Quality gate failures are logged with reason codes in the proposal for transparency. Flagged (not rejected) proposals proceed to human review with a warning annotation.

### 4.3 Proposal Generation

**Proposal structure:**

```json
{
  "phrase": "close it out",
  "target_verb": "kyc-case.close",
  "confidence": 0.82,
  "risk_tier": "elevated",
  "signal_type": "wrong_match",
  "context": {
    "workspace": "kyc",
    "pack": "kyc-case",
    "observed_constellation_state": "review",
    "entity_type": "case",
    "stack_depth": 1,
    "scope_qualifier": "workspace"
  },
  "evidence": {
    "miss_count": 3,
    "wrong_match_count": 4,
    "wrong_match_verb": "deal.close",
    "unique_sessions": 5,
    "unique_users": 4,
    "time_range": "2026-03-20 to 2026-03-28",
    "rephrase_to": "close the kyc case",
    "rejection_count": 4,
    "undo_count": 0,
    "error_cascade_count": 0
  },
  "collision_report": {
    "checked_against": 24025,
    "exact_conflicts": [],
    "semantic_near_misses": [
      {"phrase": "close kyc case", "verb": "kyc-case.close", "sim": 0.92, "safe": true}
    ],
    "cross_workspace_conflicts": [
      {"phrase": "close it out", "verb": "deal.close", "workspace": "deal", "sim": 1.0, "resolution": "workspace-scoped"}
    ],
    "shadowing_risk": "none (workspace-scoped)"
  },
  "quality_report": {
    "token_count": 3,
    "short_phrase_exception": true,
    "short_phrase_reason": "workspace-scoped, zero cross-verb collisions in kyc workspace",
    "cross_verb_ambiguity": 0.61,
    "single_user": false,
    "stop_word_ratio": 0.33,
    "previously_rejected_similar": false,
    "gates_passed": true,
    "flags": []
  },
  "proposal_rationale": "4 users typed 'close it out' in KYC workspace and were incorrectly matched to deal.close, requiring rejection and rephrase. Adding this phrase as workspace-scoped to KYC would eliminate a wrong-match that currently resolves to a verb in a different workspace."
}
```

#### 4.3.1 Confidence Score Semantics

The `confidence` field is a composite score computed as a weighted sum of five normalised signals:

| Signal | Weight | Range | Computation |
|--------|--------|-------|-------------|
| **Frequency signal** | 0.25 | 0–1 | `min(miss_count / 20, 1.0)` — saturates at 20 misses |
| **Session breadth** | 0.20 | 0–1 | `min(unique_sessions / 10, 1.0)` — saturates at 10 sessions |
| **Collision safety** | 0.20 | 0–1 | `1.0 - max_semantic_similarity_to_other_verbs` — higher when no near-misses |
| **Rephrase confirmation** | 0.15 | 0 or 1 | `1.0` if a rephrase chain confirms target verb, `0.0` otherwise |
| **Wrong-match severity** | 0.20 | 0–1 | `min(wrong_match_count / 5, 1.0)` — saturates at 5 wrong-matches. `0.0` if no wrong-matches observed |

**Formula:** `confidence = (0.25 × frequency) + (0.20 × breadth) + (0.20 × collision_safety) + (0.15 × rephrase_confirmation) + (0.20 × wrong_match_severity)`

**Interpretation for reviewers:**

| Range | Meaning |
|-------|---------|
| 0.85–1.00 | Strong candidate — high frequency, broad session coverage, clean collision profile, rephrase-confirmed, or wrong-match-driven |
| 0.60–0.84 | Moderate candidate — review evidence carefully, may lack breadth or have near-miss collisions |
| < 0.60 | Weak candidate — likely low frequency or collision concerns; inspect before approving |

**Note:** A proposal can score highly on confidence alone via wrong-match severity even with moderate miss count. This is intentional — a low-frequency phrase that consistently fires the wrong verb is more urgent than a high-frequency phrase that simply misses.

#### 4.3.2 Risk Tiers and Approval Classes

Not all verbs carry equal business risk. A phrase attached to `kyc-case.approve` (which advances a compliance workflow) has higher consequences if wrong than a phrase attached to `ui.navigate` (which changes a screen). The confidence score measures evidence quality; the risk tier measures consequence severity.

**Verb sensitivity classification:**

| Sensitivity | Criteria | Examples |
|-------------|----------|---------|
| **Critical** | Verb triggers irreversible state transitions, compliance-reportable actions, or financial commitments | `kyc-case.approve`, `kyc-case.reject`, `deal.execute`, `ubo.certify`, `cbu.terminate` |
| **Elevated** | Verb modifies entity state in ways that require undo or create downstream dependencies | `kyc-case.close`, `entity.archive`, `doc.accept`, `cbu.link-structure` |
| **Standard** | Verb performs navigation, display, query, or easily reversible operations | `ui.navigate`, `entity.view`, `report.generate`, `phrase.coverage-report` |

**Sensitivity is a property of the target verb, not the phrase.** It is looked up from the verb's SemOS contract metadata (`verb_contract.sensitivity_class`). If unset, defaults to `elevated` (fail-safe — unknown verbs get stricter review).

**Approval class routing:**

| Risk Tier | Confidence Threshold for Review Queue | Review Requirement |
|-----------|--------------------------------------|--------------------|
| **Critical** | Any confidence (all proposals reviewed) | Senior domain reviewer + mandatory written rationale for approval |
| **Elevated** | confidence ≥ 0.50 | Standard human review |
| **Standard** | confidence ≥ 0.60 | Standard human review; batch approval permitted |

Proposals below the confidence threshold for their risk tier are auto-deferred with reason `insufficient_evidence`. They re-enter observation and may qualify in a future batch.

**Review queue ordering:** Within each risk tier, proposals are sorted by: (1) wrong-match proposals first, (2) confidence descending. This ensures wrong-match corrections on critical verbs are reviewed before high-frequency misses on standard verbs.

### 4.4 Governance Pipeline

**SemOS object:** New object type `phrase_mapping` (separate from `verb_contract` — see §12, Decision 1)

**Lifecycle states:**

```
                              ┌──────────────────────────────┐
                              │                              │
                              ▼                              │
proposed → collision_checked ─┬→ quality_checked → reviewed ─┬→ published
                              │                      │  ▲    │
                              │                      │  │    └→ rejected
                              │                      │  │
                              │                      ▼  │
                              │                   refined ──→ collision_checked
                              │                              (backward arc)
                              │
                              └→ deferred
                                 (human parks for later;
                                  re-enters at reviewed
                                  on manual promotion)
```

**State transitions:**

| From | To | Trigger |
|------|----|---------|
| `proposed` | `collision_checked` | Automated collision gate completes |
| `collision_checked` | `quality_checked` | Automated quality gate completes |
| `collision_checked` | `deferred` | Human defers (e.g., waiting for upstream verb redesign) |
| `quality_checked` | `reviewed` | Automated gates pass; enters human review queue (risk-tiered) |
| `reviewed` | `published` | Human approves (with mandatory rationale for critical-tier) |
| `reviewed` | `rejected` | Human rejects (with reason code; fed back to agent) |
| `reviewed` | `refined` | Human modifies phrase text or context |
| `refined` | `collision_checked` | Refinement triggers re-run of collision + quality gates |
| `deferred` | `reviewed` | Human manually promotes deferred proposal |

**Key design decisions:**
- `refined` always re-enters at `collision_checked`, not `reviewed`. A human edit may introduce new collisions or quality failures. The backward arc is mandatory.
- `deferred` is a parking state, not a terminal. Proposals can sit here indefinitely and be promoted when context changes (e.g., a new verb is added that resolves the ambiguity).
- `rejected` is terminal. If the same pattern resurfaces, the agent creates a new proposal (which the historical-rejection quality gate will flag).

**Automated gates:**
- Collision check passes (zero exact conflicts, zero high-risk semantic overlaps)
- Cross-workspace collision check passes or phrase is workspace-scoped
- Target verb exists in SemOS as active verb_contract
- Target verb is valid in the proposed workspace/constellation context
- Quality gates pass (see §4.2.1)
- Profanity filter (regex-based, v1.0 scope — see below)

**PII / regulatory-sensitive language detection (v1.0 scope note):** Profanity filtering is implemented via regex in v1.0. PII detection (entity names, account numbers in phrase text) and regulatory sensitivity screening are non-trivial NLP problems deferred to v1.1. For v1.0, these are covered by human review.

**Human gates:**
- Domain vocabulary correctness
- Organisational convention alignment
- Regulatory sensitivity review (for compliance-adjacent verbs)
- Written approval rationale (mandatory for critical-tier verbs)

### 4.5 Materialization — Workspace-Scoped Phrases

**Design decisions:**
1. **`phrase_bank` is the sole materialization target.** The governance pipeline never writes to `dsl_verbs.yaml_intent_patterns`. That column remains a read-only YAML-sourced fallback consumed by the REPL as a lower-precedence Tier 0 source until YAML phrases are migrated (Phase 6).
2. **Workspace-only scoping for v1.0.** `constellation_state` is not part of the v1.0 schema, uniqueness constraint, or lookup. State is recorded in proposals as observational context (`observed_constellation_state`) but does not affect phrase resolution. See §12, Decision 6 for rationale and v1.1 prerequisites.

When a phrase is published:

1. **Trigger on `sem_reg.snapshots` INSERT** (same pattern as attribute materialization)
2. Extracts phrase + verb FQN + workspace from definition
3. **UPSERTs to `phrase_bank`** — the sole operational store. If an active row exists for this `(phrase, workspace)`, it is deactivated (`active = FALSE`) and the new row's `supersedes_id` points to it.
4. Computes embedding via Candle BGE-small-en-v1.5
5. INSERTs to `verb_pattern_embeddings` with new vector and workspace qualifier
6. Updates `verb_centroids` for the affected verb

#### 4.5.1 Phrase Bank Schema

```sql
CREATE TABLE phrase_bank (
    id                  SERIAL PRIMARY KEY,
    phrase              TEXT NOT NULL,
    verb_fqn            TEXT NOT NULL REFERENCES dsl_verbs(fqn),
    workspace           TEXT,          -- NULL = global (matches any workspace)
    source              TEXT NOT NULL DEFAULT 'governed',  -- 'governed' | 'yaml' | 'legacy'
    risk_tier           TEXT NOT NULL DEFAULT 'elevated',  -- 'critical' | 'elevated' | 'standard'
    sem_reg_snapshot_id UUID,          -- link back to SemOS object (NULL for yaml/legacy sources)
    supersedes_id       INT REFERENCES phrase_bank(id),   -- NULL unless this row supersedes an older entry
    active              BOOLEAN NOT NULL DEFAULT TRUE,     -- FALSE when superseded or deprecated
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- At most one active phrase per (phrase, workspace) pair.
    -- This prevents ambiguous Tier 0 resolution.
    -- Uses partial unique index (PostgreSQL).
    CONSTRAINT uq_phrase_bank_active EXCLUDE USING btree (
        phrase WITH =, (COALESCE(workspace, '__global__')) WITH =
    ) WHERE (active = TRUE)
);

CREATE INDEX idx_phrase_bank_lookup
    ON phrase_bank (phrase, workspace)
    WHERE (active = TRUE);
```

**Uniqueness invariant (v1.2):** The partial exclusion constraint guarantees that for any given phrase text in any given workspace (including `NULL` for global), there is at most one active row. This eliminates nondeterministic Tier 0 behaviour. If a human-refined proposal supersedes an existing phrase, the old row is set `active = FALSE` and `supersedes_id` is set on the new row. The `COALESCE` handles NULL workspace comparison (two `NULL` workspaces are treated as equal for uniqueness purposes).

#### 4.5.2 Phrase Precedence Order

When the REPL resolves a phrase at Tier 0, the lookup follows a strict precedence order:

```sql
-- Tier 0: phrase_bank (workspace-qualified, governed)
SELECT verb_fqn
FROM phrase_bank
WHERE phrase = $1
  AND active = TRUE
  AND (workspace IS NULL OR workspace = $2)
ORDER BY
    CASE WHEN workspace IS NOT NULL THEN 0 ELSE 1 END,  -- workspace-specific first
    CASE source
        WHEN 'governed' THEN 0     -- governed (reviewed, published) highest
        WHEN 'legacy'   THEN 1     -- legacy (migrated from agent.teach) next
        WHEN 'yaml'     THEN 2     -- yaml (migrated from config files) lowest
    END
LIMIT 1;

-- Tier 0 fallback: dsl_verbs.yaml_intent_patterns (global, unqualified)
-- Only reached if phrase_bank returns no rows.
-- Eliminated after Phase 6 (YAML migration).
SELECT fqn FROM dsl_verbs WHERE $1 = ANY(yaml_intent_patterns);
```

**Precedence summary (highest to lowest):**

| Rank | Scope | Source | Description |
|------|-------|--------|-------------|
| 1 | Workspace-specific | `governed` | Published through SemOS governance with workspace qualifier |
| 2 | Workspace-specific | `legacy` | Migrated from `agent.teach`, workspace-scoped during migration |
| 3 | Workspace-specific | `yaml` | Migrated from YAML config, workspace-scoped during migration |
| 4 | Global | `governed` | Published through SemOS governance, no workspace qualifier |
| 5 | Global | `legacy` | Migrated from `agent.teach`, no workspace qualifier |
| 6 | Global | `yaml` | Migrated from YAML config, no workspace qualifier |
| 7 | Global (fallback) | YAML column | `dsl_verbs.yaml_intent_patterns` — legacy fallback, pre-migration |

**Supersession rule:** When a governed phrase is published that matches an existing active phrase (same `phrase` + `workspace`), the existing row is deactivated (`active = FALSE`) and the new row's `supersedes_id` points to it. The exclusion constraint enforces single-winner. Audit trail is preserved — deactivated rows remain in the table.

**Tier 3 (semantic search) qualification:** The `verb_pattern_embeddings` table gains a `workspace` column. Semantic search filters by workspace context before ranking by cosine similarity.

**Hot reload:** The REPL's exact phrase match (Tier 0) queries the DB on every turn. Published phrases are immediately available without server restart. **The Tier 0 read path never writes** — see §12, Decision 4 for expiry telemetry separation.

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
| `phrase.observe-misses` | plugin | Trawl session traces for miss and wrong-match patterns from watermark. Returns pattern summary with priority classification |
| `phrase.analyse-candidates` | plugin | Run collision detection + quality gates + context grounding + risk classification on patterns |
| `phrase.propose` | plugin | Generate structured proposal with evidence for a single candidate phrase |
| `phrase.batch-propose` | plugin | Bulk proposals from aggregated analysis. Runs de-duplication across the batch (no two proposals for the same phrase), priority-orders by risk tier then confidence score descending, and caps batch size at 50 proposals per run to keep human review tractable |
| `phrase.review-proposals` | plugin | List pending proposals with context + collision reports + quality reports, grouped by risk tier |
| `phrase.approve` | plugin | Accept proposal → changeset → publish. Requires written rationale for critical-tier |
| `phrase.reject` | plugin | Reject with reason code → agent learns (updates historical rejection index) |
| `phrase.defer` | plugin | Park proposal in `deferred` state with optional reason |
| `phrase.check-collisions` | plugin | Standalone collision audit for a candidate phrase (cross-workspace) |
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
    observe: { verb: phrase.observe-misses, slot_state: [empty, filled] }
    analyse: { verb: phrase.analyse-candidates, slot_state: filled }
    propose: { verb: phrase.propose, slot_state: filled }
    review: { verb: phrase.review-proposals, slot_state: filled }
    approve: { verb: phrase.approve, slot_state: filled }
    reject: { verb: phrase.reject, slot_state: filled }
    defer: { verb: phrase.defer, slot_state: filled }
    coverage: { verb: phrase.coverage-report, slot_state: [empty, filled] }
```

**Terminology note (v1.1):** The `slot_state` key refers to the constellation slot cardinality state (`empty` / `filled`), indicating whether the phrase_authoring slot has bound entities. This is distinct from the `phrase_authoring_lifecycle` FSM states (`proposed`, `collision_checked`, etc.) which govern individual phrase proposals.

**State machine:** `phrase_authoring_lifecycle`

```
proposed → collision_checked → quality_checked → reviewed → published | rejected
                ↑                                   │
                │                                   ↓
                └──────────────────────────── refined
                                                 │
              deferred ←────────────────── collision_checked
                │                          (also reachable from
                └─→ reviewed               collision_checked directly)
                    (manual promotion)
```

---

## 6. Integration Points

### 6.1 Session Trace (Input)

- **Table:** `session_traces`
- **Key fields:** `session_id`, `sequence`, `operation`, `payload`, `stack_snapshot`
- **Query pattern:** Aggregate misses and wrong-matches by utterance text, group by workspace/state, classify by signal priority tier
- **Watermark:** `phrase_observation_state.last_observed_sequence` — batch job processes only new entries

### 6.2 Phrase Bank (Target) — Single Canonical Store

- **Operational store:** `phrase_bank` table — the sole materialization target. Workspace-qualified entries with `source`, `risk_tier`, `active`, `supersedes_id`, and `sem_reg_snapshot_id` columns (see §4.5.1)
- **Legacy fallback (read-only):** `dsl_verbs.yaml_intent_patterns` (text array column) — consumed by Tier 0 only if `phrase_bank` returns no match. Never written to by the governance pipeline. Scheduled for migration in Phase 6.
- **Embeddings:** `verb_pattern_embeddings` (pgvector 384-dim) — extended with `workspace` column for workspace-qualified semantic search
- **Centroids:** `verb_centroids` (per-verb centroid for shortlisting)

**Tier 0 resolution order:** `phrase_bank` (workspace-qualified, precedence-ordered per §4.5.2) is queried first. If no match, fall through to `dsl_verbs.yaml_intent_patterns` (global, unqualified). This is a transitional arrangement — once YAML phrases are migrated into `phrase_bank` (Phase 6), the fallback is eliminated.

### 6.3 SemOS Registry (Governance)

- **Snapshots:** `sem_reg.snapshots` with `object_type = 'phrase_mapping'`
- **Changesets:** Existing changeset pipeline
- **Materialization:** AFTER INSERT trigger on `sem_reg.snapshots` WHERE `object_type = 'phrase_mapping'` — writes to `phrase_bank` only

### 6.4 Learning Infrastructure (Migration)

- **Current:** `invocation_phrases` table + `learning_candidates` table
- **Migration:** Wrap `agent.teach` to submit through SemOS governance instead of direct write
- **One-time migration (Phase 2.5):** Bulk-import existing `invocation_phrases` entries into `sem_reg.snapshots` as `phrase_mapping` objects with status `published`, then materialize into `phrase_bank` with `source = 'legacy'`. After validation, deprecate the direct-write path. See §9, Phase 2.5 for detail.
- **Post-migration:** The `invocation_phrases` table is retained read-only for audit trail. All new writes go through SemOS governance.

### 6.5 REPL Pipeline (Consumer)

- **Tier 0:** Exact phrase match queries `phrase_bank` first (workspace-qualified, precedence-ordered per §4.5.2), then falls through to `dsl_verbs.yaml_intent_patterns`
- **Tier 3:** Semantic search queries `verb_pattern_embeddings` (with workspace filter) — picks up new embeddings after materialization
- **No server restart required** — both paths query DB on each turn
- **Read-only hot path:** Tier 0 and Tier 3 are read-only operations. No writes occur on the phrase resolution path. See §12, Decision 4 for expiry telemetry separation.

---

## 7. AI Model Requirements

### 7.1 Pattern Recognition (Observation Phase)

- Session trace aggregation (SQL + application logic)
- Watermark-based incremental processing (`WHERE sequence > last_observed_sequence`)
- Utterance clustering (BGE embeddings + cosine similarity for grouping)
- Rephrase chain detection (consecutive utterances by same user to same final verb)
- **Wrong-match chain detection** (verb matched → rejected/undone → different verb matched)
- Priority classification (wrong-match signals as Priority 1, miss signals as Priority 2)
- No external LLM required — deterministic analysis on structured data

### 7.2 Collision Detection & Quality Screening (Analysis Phase)

- Exact string match (trivial)
- Semantic similarity scoring (Candle BGE, same model as phrase bank)
- Cross-workspace collision check (all workspaces, not just source)
- Historical collision lookup (was this phrase previously removed?)
- Historical rejection lookup (was a similar phrase previously rejected?)
- Quality gate evaluation (context-aware token count, cross-verb ambiguity, stop-word ratio, single-user check)
- Verb sensitivity classification lookup (`verb_contract.sensitivity_class`)
- No external LLM required — uses existing embedding infrastructure + deterministic rules

### 7.3 Proposal Generation (Propose Phase)

- Structured output (JSON proposal with evidence, collision report, quality report, risk tier)
- Confidence score computation (see §4.3.1 for five-signal scoring function)
- Risk tier assignment from verb sensitivity classification (see §4.3.2)
- Rationale generation (natural language explanation of why this phrase should be added)
- **This is the one phase that benefits from LLM** — generating the `proposal_rationale` field and suggesting alternative phrasings
- Can use the existing Sage agent integration (Claude API)

### 7.4 Sandbox Consideration

The observation and analysis phases can run entirely in a **sandbox** — no external API calls, no LLM inference. They process structured data (session traces, phrase bank) with deterministic algorithms (SQL aggregation, cosine similarity, string matching, quality heuristics, risk classification lookups).

The proposal phase can optionally use LLM for rationale generation, but the core proposal (phrase + verb + context + evidence + confidence score + risk tier) is deterministic. The LLM adds explanatory text, not decision logic.

This means the capability can be developed and tested without LLM dependency, with LLM-assisted rationale as an optional enhancement.

---

## 8. Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| First-attempt hit rate | 62.4% | 75%+ | `intent_hit_rate` test suite |
| Two-attempt hit rate | 84.9% | 92%+ | `intent_hit_rate` test suite |
| Phrase collisions | 0 (after manual cleanup) | 0 (automated) | Collision audit in CI |
| Wrong-match rate | Unmeasured | < 2% of resolved utterances | Session trace analysis (wrong-match signals) |
| Miss-to-proposal latency | N/A (manual) | < 24 hours | Automated observation pipeline |
| Proposal acceptance rate | N/A | > 70% | Governance metrics |
| Time to first match after miss | N/A (days/weeks) | < 1 hour after publish | Session trace analysis |

### 8.1 Throughput Model

Moving from 62% to 75% first-attempt hit rate requires closing ~34% of the current miss surface. Back-of-envelope estimate:

- Current miss volume: ~38% of utterances = ~38 misses per 100 utterances
- Target miss volume: ~25% = ~25 misses per 100 utterances
- Delta: ~13 percentage points = ~13 additional matches per 100 utterances

**Phrase yield assumptions:**
- Each high-quality phrase covers ~2.5 miss patterns on average (accounting for semantic clustering — "close it out", "close this case", "shut the case" all resolve with 1–2 phrases)
- Estimated net-new governed phrases needed: **~2,000–3,000** (covering ~5,000–7,500 miss patterns)

**Pipeline throughput at target operating tempo:**

| Parameter | Conservative | Optimistic |
|-----------|-------------|------------|
| Daily miss patterns qualifying for analysis | 15 | 40 |
| Analysis → proposal conversion rate | 60% | 80% |
| Daily proposals generated | 9 | 32 |
| Human review acceptance rate | 70% | 85% |
| Daily phrases published | 6 | 27 |
| Days to reach 2,500 phrases | ~420 (~14 months) | ~93 (~3 months) |

**Implication:** At conservative throughput, the 75% target is a 12–18 month objective. Batch-propose with weekly review sessions (50 proposals per batch) is more realistic than daily trickle review. The review surface is central to feasibility — see Phase 3 (review design) in §9.

---

## 9. Phased Delivery

### Phase 1: Observation Infrastructure (P1)

- `phrase.observe-misses` verb — trawl session traces for miss and wrong-match patterns from watermark
- `phrase.coverage-report` verb — per-workspace gap analysis
- `phrase_observation_state` table with watermark tracking
- Wrong-match chain detection logic
- Priority classification (Priority 1: wrong-match, Priority 2: miss)
- SQL views over `session_traces` for miss and wrong-match aggregation
- No governance integration yet — diagnostic only

### Phase 2: Phrase Bank Schema, Collision Detection & Quality Service (P1)

- **`phrase_bank` table creation** — schema per §4.5.1, including `active`, `supersedes_id`, partial exclusion constraint
- `phrase.check-collisions` verb — standalone collision check (cross-workspace)
- Quality gate implementation (context-aware token count, cross-verb ambiguity, stop-word ratio, single-user flag, historical rejection)
- Automated collision audit in CI (`scripts/lint_phrase_collisions.sh`)
- Integration with existing phrase bank (24,025 patterns)
- Verb sensitivity classification metadata on `verb_contract` (`sensitivity_class` field)

### Phase 2.5: Legacy Phrase Migration (P1)

*Depends on: Phase 2 (`phrase_bank` table exists)*

- One-time bulk import of existing `invocation_phrases` entries into `sem_reg.snapshots` as `phrase_mapping` objects with status `published`
- Materialize into `phrase_bank` table with `source = 'legacy'`, `workspace = NULL` (global), `active = TRUE`
- Validation pass: confirm Tier 0 resolution produces identical results before and after migration (run `intent_hit_rate` test suite, compare exact-match results)
- Deprecate direct-write path in `agent.teach` (wrap to submit through SemOS governance)
- Retain `invocation_phrases` table read-only for audit trail

### Phase 3: SemOS Phrase Object Type, Review Surface & Governance Pipeline (P2)

- New `phrase_mapping` object type in SemOS
- Materialization trigger (snapshot → `phrase_bank` only)
- State machine: `phrase_authoring_lifecycle` (with backward arcs, `deferred` state)
- **Review surface design and implementation** — risk-tiered review queue, batch review UX (50 proposals per session), wrong-match proposals surfaced first, mandatory rationale for critical-tier approvals. Review ergonomics are central to throughput (see §8.1) and are built alongside the governance object type, not deferred.
- `phrase.review-proposals`, `phrase.approve`, `phrase.reject`, `phrase.defer` verbs
- SemOS Maintenance workspace integration (phrase_authoring slot)
- Rejection feedback loop (agent learns from rejections; updates historical rejection index)
- Refinement flow with mandatory collision + quality re-check (backward arc to `collision_checked`)

### Phase 4: AI Proposal Pipeline (P2)

- `phrase.propose` and `phrase.batch-propose` verbs
- Confidence score computation (§4.3.1 five-signal scoring function)
- Risk tier assignment from verb sensitivity classification (§4.3.2)
- Pattern analysis (frequency, clustering, rephrase chains, wrong-match chains)
- Context grounding (workspace, constellation state, entity type)
- Cross-workspace collision detection in proposal generation
- Evidence assembly (miss count, wrong-match count, sessions, users, time range)
- Quality report assembly

### Phase 5: LLM-Assisted Rationale (P3, Optional)

- Sage agent generates `proposal_rationale` natural language
- Alternative phrasing suggestions
- Domain vocabulary alignment scoring

### Phase 6: YAML Phrase Migration (Post-v1.0)

- Bulk-import `dsl_verbs.yaml_intent_patterns` entries into `phrase_bank` with `source = 'yaml'`
- Workspace inference during migration (use verb domain prefix to assign workspace where unambiguous; flag ambiguous phrases for human review)
- Validation pass: confirm Tier 0 resolution produces identical results
- Eliminate `yaml_intent_patterns` fallback from Tier 0 lookup
- `phrase_bank` becomes the sole Tier 0 source

---

## 10. Non-Goals

- **Real-time phrase learning** — proposals go through governance, not instant activation
- **Unsupervised phrase addition** — AI proposes, human decides. Always.
- **Multi-language support** — English only for v1.0
- **Phrase deletion automation** — deprecation requires human review
- **Custom embedder per workspace** — single BGE model for all workspaces
- **PII / regulatory-sensitivity auto-detection** — deferred to v1.1; v1.0 relies on profanity regex + human review
- **Auto-publish threshold** — high-confidence proposals do not skip human review in v1.0. Trust must be earned. Revisit after 6 months of governed operation with measured false-positive rate.
- **Constellation-state-scoped phrase resolution** — v1.0 is workspace-scoped only. State is recorded observationally but does not affect resolution. See §12, Decision 6.

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
| Verb sensitivity classification | **New** | `sensitivity_class` metadata on `verb_contract` — required before Phase 2 |

---

## 12. Resolved Design Decisions (formerly Open Questions)

### Decision 1: Object type — New `phrase_mapping` (Resolved v1.1)

**Decision:** Separate `phrase_mapping` object type.

A `phrase_mapping` has a fundamentally different lifecycle, different governance gates (collision + quality), and different materialization target (phrase bank, not verb contract). Extending `verb_contract` would muddy its semantics — a verb contract governs what a verb *does*; a phrase mapping governs how users *refer to* a verb. These are orthogonal concerns. The additional schema cost (one new object type in `sem_reg`) is trivial relative to the semantic clarity gained.

### Decision 2: Observation frequency — Batch, daily (Resolved v1.1)

**Decision:** Daily batch with persistent watermark. See §4.1.1.

Batch avoids O(n) per-session aggregation on an append-only table. The watermark pattern (`last_observed_sequence`) ensures no traces are missed and no traces are double-processed. The minimum threshold (3+ misses across 2+ sessions) filters single-session noise. Wrong-match patterns use a lower threshold (2+ across 1+ sessions) due to higher severity.

### Decision 3: Auto-publish threshold — No, not in v1.0 (Resolved v1.1)

**Decision:** All proposals require human review in v1.0. See §10.

Revisit after 6 months of governed operation. If the proposal acceptance rate exceeds 90% with zero post-publish collisions over a sustained period, auto-publish for confidence > 0.90 proposals on standard-tier verbs can be considered in v1.1. Critical-tier verbs will never be auto-published.

### Decision 4: Phrase expiry — Deferred to v1.1 (Telemetry model specified v1.2)

**Open for v1.1.** Phrases that haven't matched in 90 days should be flagged for review to prevent phrase bank bloat.

**v1.2 clarification — hot-path separation:** Tier 0 phrase resolution is a read-only operation. Match telemetry (`last_matched_at`, `match_count`) will **not** be captured inline with Tier 0. Instead, expiry telemetry will use one of the following asynchronous strategies (to be selected during v1.1 implementation):

| Strategy | Mechanism | Trade-off |
|----------|-----------|-----------|
| **Batch scan** | Daily job replays session traces, identifies which phrases were matched, bulk-updates `phrase_bank.last_matched_at` | Simplest; reuses existing watermark infrastructure; ~24h staleness |
| **Write-behind buffer** | Tier 0 appends matched `phrase_bank.id` to an in-memory ring buffer; a background thread flushes to DB every N seconds | Sub-minute staleness; requires application-level buffer management |
| **Trace-derived** | Extend session trace payload to include `matched_phrase_id`; expiry job queries traces directly | Zero hot-path changes; telemetry piggybacked on existing append-only infrastructure |

The key invariant: **the Tier 0 read path never writes.** Phrase expiry is a governance concern, not a resolution concern. It runs asynchronously or is derived from existing trace data.

### Decision 5: Cross-workspace phrases — Workspace-scoped (Resolved v1.1, refined v1.2)

**Decision:** Phrases are workspace-qualified with an optional `workspace` column. See §4.5.

A phrase valid in KYC workspace that shadows a different verb in Deal workspace is stored with `workspace = 'kyc'`. Global phrases (valid everywhere) have `workspace = NULL`. The Tier 0 lookup uses precedence ordering (§4.5.2) to prefer workspace-specific matches over global ones. Cross-workspace collision detection in the analysis phase (§4.2) identifies when workspace scoping is required and sets the `scope_qualifier` field in the proposal.

### Decision 6: Constellation-state scoping — Deferred to v1.1 (New v1.2)

**Decision:** v1.0 uses workspace-only scoping. `constellation_state` is not part of the `phrase_bank` schema, uniqueness constraint, or Tier 0 lookup.

**Rationale:** State-scoped phrase resolution requires:
1. The Tier 0 lookup to receive constellation state as a parameter (currently not passed)
2. The uniqueness constraint to include state (changing from `(phrase, workspace)` to `(phrase, workspace, constellation_state)`)
3. Collision detection to enumerate state-specific conflicts
4. A fallback rule when the session has no active constellation (which state-scoped phrase applies?)

These are non-trivial runtime changes. Workspace scoping alone resolves the primary cross-workspace collision problem (§4.5). State scoping provides finer granularity but introduces ambiguity in sessions where constellation state is not yet established.

**v1.1 prerequisites:** Before implementing state scoping, the REPL must pass `constellation_state` to Tier 0, and a fallback rule must be defined for sessions without active constellation state. The `observed_constellation_state` field recorded in v1.0 proposals provides training data to assess whether state scoping would materially improve hit rates — if proposals show that the same phrase in the same workspace always occurs in the same state, state scoping adds no value. If they show the same phrase mapping to different verbs in different states within the same workspace, state scoping is justified.

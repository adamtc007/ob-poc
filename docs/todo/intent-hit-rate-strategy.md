# Intent Hit Rate Strategy: ≤2 Prompts to Execution

## Problem Statement

Current pipeline achieves low first-attempt verb selection rates. Users frequently
hit NeedsClarification or NoMatch outcomes, requiring 3+ round-trips before execution.
Target: **≤2 prompts** from utterance to verb execution for the top-80 verbs.

## Current Pipeline Diagnosis

```
utterance
  → Tier 0: Macros        (exact label match — works well, but limited vocabulary)
  → Tier 0.5: Lexicon     (token overlap — fast, deterministic, but brittle)
  → Tier 1-2: Learned     (exact phrase from training data — cold start problem)
  → Tier 3: User Semantic  (pgvector — requires per-user history)
  → Tier 6: Global Semantic (BGE-small-en-v1.5, 384-dim embeddings)
  → Tier 7: Phonetic       (dmetaphone — typo correction only)
  → Ambiguity gate: threshold=0.65, margin=0.05
  → LLM: argument extraction only (NOT verb selection)
```

### Five Root Causes

**1. Coverage Gap — 47.6% invisible (311/653 verbs)**
No invocation_phrases → no embeddings → invisible to Tier 6. The entire capital
domain (21 verbs), SLA (17), team (15), lifecycle (16), pricing (14), attribute (11),
identifier (11) are completely dark. Plus 14 of 19 CBU verbs.

**2. Semantic Space Collisions**
Custody banking vocabulary is dense. "Check the client" could plausibly match:
screening.sanctions, screening.pep, entity.verify, kyc.assess-risk, control.analyze,
document.verify, ownership.reconcile. BGE embeddings can't distinguish intent from
surface similarity. The 0.05 ambiguity margin means most real utterances trigger
NeedsClarification.

**3. No LLM in Verb Selection**
The LLM is ONLY used for argument extraction AFTER verb selection. The verb selection
itself is purely embedding-based — no reasoning about user intent, domain context,
or workflow state. This is the critical missing tier.

**4. Narrow Acceptance Band**
threshold=0.65 + margin=0.05 creates a tight window:
- score >= 0.65 AND (top - runner_up) > 0.05 → Matched (auto-proceed)
- score >= 0.65 AND margin <= 0.05 → Ambiguous (show menu)
- score >= 0.55 AND score < 0.65 → Suggest (show menu)
- score < 0.55 → NoMatch (fail)

For a 653-verb registry, embedding similarity between related verbs (e.g.,
control.list-controllers vs ownership.who-controls) is often within 0.05. The
system is structurally biased toward ambiguity.

**5. No Contextual Boosting**
If user is in KYC workflow (stage_focus=semos-kyc), "run a check" should strongly
prefer screening.* verbs. But verb search uses uniform scores — no domain boost,
no entity-type boost, no workflow-state boost.

---

## Strategy: Four Phases

### Phase 1: Fill the Data (Week 1)

**Impact: +25% hit rate. Risk: Zero.**

Merge the existing draft files and generate phrases for remaining verbs.

1. **Merge `_invocation_phrases_draft.yaml`** (945 lines) and
   **`_invocation_phrases_extension.yaml`** (1327 lines) into domain YAML files.

2. **Generate phrases for remaining 271 verbs** using Claude with this prompt template:

```
Given verb: {domain}.{verb_name}
Description: {description}
Required args: {args}
Domain context: Custody banking KYC/AML onboarding system

Generate 8-12 invocation phrases that a compliance analyst or onboarding
specialist would naturally say. Include:
- Direct imperative ("create a share class")
- Question form ("what share classes exist?")
- Contextual ("I need to set up the ISIN for this fund")
- Abbreviated ("new share class")

Do NOT include the verb name itself as a phrase.
```

3. **Run `populate_embeddings`** to regenerate vectors for all 653 verbs.

4. **Phrase quality audit**: Current phrases are too short and too technical.
   "create CBU" is not what a user says. They say "I need to onboard a new client"
   or "set up a fund structure for Allianz". Enrich existing 342-verb phrases to
   include natural business language, not just DSL vocabulary.


### Phase 2: LLM-Powered Intent Classification (Week 2-3)

**Impact: +30% hit rate. Risk: Medium (latency, cost).**

Add a new tier between Lexicon (0.5) and Semantic (6) that uses an LLM to classify
intent when embedding search is ambiguous or low-confidence.

#### Architecture

```
utterance
  → Tier 0: Macros (unchanged)
  → Tier 0.5: Lexicon (unchanged)
  → Tier 1-2: Learned (unchanged)
  
  ──── DECISION GATE ────
  If top_score >= 0.85 → skip to LLM arg extraction (high confidence)
  If top_score < 0.85 → invoke NEW Tier 4: LLM Intent Classifier
  ────────────────────────
  
  → Tier 4 [NEW]: LLM Intent Classifier
      Input:  utterance + domain_hint + entity_context + top-5 candidates
      Output: { selected_verb, confidence, reasoning }
      
  → Tier 6: Global Semantic (fallback if LLM fails)
  → Tier 7: Phonetic (unchanged)
```

#### LLM Classifier Prompt Design

The key insight: **don't send 653 verbs to the LLM**. Send a two-level taxonomy:

**Level 1 — Domain classification** (fast, structured output):
```
Given this user request in a custody banking onboarding system:
"{utterance}"

The user is currently working on: {workflow_context}
Active entity type: {entity_kind}

Which domain is this about?
1. cbu — Client Business Unit lifecycle (create, update, delete, assign roles)
2. entity — Legal entity management (create companies, persons, trusts)
3. kyc/screening — KYC checks, sanctions, PEP, adverse media
4. document — Document collection, verification, extraction
5. ubo — Beneficial ownership chains, control structures
6. fund — Fund structures (umbrellas, subfunds, share classes, feeders)
7. deal — Deal lifecycle, rate cards, contracts, SLAs
8. ownership — Ownership computation, reconciliation, rights
9. control — Corporate control chains, board controllers
10. view/session — Navigation, filtering, loading data
11. gleif — LEI lookups, GLEIF hierarchy imports
12. other

Reply with ONLY the number.
```

**Level 2 — Verb selection** (within the winning domain, ~5-50 verbs):
```
The user said: "{utterance}"
Domain: {selected_domain}

Which verb best matches their intent?
{verb_list_with_descriptions}

Reply JSON: {"verb": "domain.verb-name", "confidence": 0.0-1.0}
```

This two-stage approach keeps each LLM call small and fast (~50-100 tokens output).
Total added latency: ~200-400ms for both calls (using claude-haiku or equivalent).

#### Optimization: Single-Call Variant

For production, combine both levels into one call with the top-5 embedding
candidates plus the domain's full verb list:

```
User said: "{utterance}"
Context: Working on {workflow}, entity type {entity_kind}

Embedding search found these candidates:
1. cbu.assign-role (score: 0.72) — Assign entity to CBU role
2. cbu.role.assign (score: 0.69) — Assign role via v2 API
3. team.assign-member (score: 0.64) — Add member to team
4. entity.update (score: 0.61) — Update entity attributes
5. ubo.add-control (score: 0.58) — Add control relationship

Additional verbs in the top domain (cbu):
- cbu.create, cbu.update, cbu.list, cbu.delete, ...

Which verb is the user asking for? Consider:
- Their workflow context (KYC → prefer screening/document verbs)
- Entity type constraints (person → prefer entity/ubo verbs)
- Semantic intent, not just keyword similarity

Reply JSON: {"verb": "...", "confidence": 0.0-1.0, "reason": "..."}
```

#### Implementation in verb_search.rs

```rust
// New tier in HybridVerbSearcher::search()

// DECISION GATE: if embedding top score is ambiguous, use LLM classifier
let top_score = results.first().map(|r| r.score).unwrap_or(0.0);
if top_score < 0.85 && top_score >= 0.45 {
    // Embedding found something but not confident enough
    let llm_result = self.llm_intent_classifier
        .classify(query, &results, domain_filter, workflow_context)
        .await?;
    
    if llm_result.confidence >= 0.80 {
        // LLM is confident — use its selection
        results.insert(0, VerbSearchResult {
            verb: llm_result.verb,
            score: llm_result.confidence,
            source: VerbSearchSource::LlmClassifier,
            matched_phrase: format!("LLM: {}", llm_result.reason),
            description: self.get_verb_description(&llm_result.verb).await,
        });
    }
}
```

#### Cost Control

- Only invoke LLM when embedding score is in the ambiguous range (0.45-0.85)
- Use claude-haiku-4.5 for classification (~$0.001 per call)
- Cache LLM classifications for identical utterances (TTL: 1 hour)
- Track LLM agreement rate with embedding top-1 to measure added value
- Circuit breaker: if LLM latency > 2s, fall back to embedding-only


### Phase 3: Contextual Scoring (Week 3-4)

**Impact: +15% hit rate. Risk: Low.**

Add three scoring boosts to the post-search ranking:

#### 3a. Workflow Domain Boost

```rust
fn apply_workflow_boost(results: &mut [VerbSearchResult], stage_focus: Option<&str>) {
    let boost_domains: HashSet<&str> = match stage_focus {
        Some("semos-kyc") => ["kyc", "screening", "document", "ubo", "entity"].into(),
        Some("semos-onboarding") => ["cbu", "entity", "fund", "deal", "contract"].into(),
        Some("semos-data-management") => ["attribute", "identifier", "gleif", "graph"].into(),
        Some("semos-stewardship") => ["lifecycle", "control", "ownership", "temporal"].into(),
        _ => return,
    };
    
    for result in results.iter_mut() {
        let domain = result.verb.split('.').next().unwrap_or("");
        if boost_domains.contains(domain) {
            result.score = (result.score + 0.08).min(1.0); // +0.08 boost
        }
    }
}
```

#### 3b. Entity-Type Boost

If session has a dominant entity type (e.g., "limited-company"), boost verbs that
operate on that type:

```rust
fn apply_entity_type_boost(results: &mut [VerbSearchResult], entity_kind: Option<&str>) {
    // Example: if working with a fund entity, boost fund.* and ownership.* verbs
    let boost_verbs = match entity_kind {
        Some("fund") | Some("umbrella") | Some("subfund") => 
            vec!["fund.", "ownership.", "capital."],
        Some("limited-company") | Some("partnership") =>
            vec!["entity.", "control.", "ubo.", "gleif."],
        Some("proper-person") =>
            vec!["ubo.", "screening.", "entity."],
        _ => return,
    };
    
    for result in results.iter_mut() {
        if boost_verbs.iter().any(|prefix| result.verb.starts_with(prefix)) {
            result.score = (result.score + 0.05).min(1.0);
        }
    }
}
```

#### 3c. Session Momentum Boost

Track the last 3 verbs executed in session. Boost verbs in the same domain or
related domains (following typical workflow sequences):

```rust
// Common workflow sequences (verb A → likely next verb B)
const WORKFLOW_SEQUENCES: &[(&str, &[&str])] = &[
    ("cbu.create", &["entity.create-limited-company", "cbu.assign-role", "cbu.add-product"]),
    ("entity.create-limited-company", &["cbu.assign-role", "gleif.search", "screening.sanctions"]),
    ("screening.sanctions", &["screening.pep", "screening.adverse-media", "document.solicit"]),
    ("ubo.add-ownership", &["ubo.add-control", "ubo.calculate", "ownership.compute"]),
    ("deal.create", &["deal.add-participant", "deal.add-product", "deal.create-rate-card"]),
];
```


### Phase 4: Query Rewriting (Week 4-5)

**Impact: +10% hit rate. Risk: Medium.**

Before embedding search, optionally rewrite the query to canonical form. This
handles the gap between natural language and technical vocabulary.

**Examples:**
| User Says | Rewritten Query |
|-----------|-----------------|
| "check if they're on any lists" | "run sanctions screening" |
| "who owns this company" | "list beneficial owners" |
| "set up the fund structure" | "create umbrella fund" |
| "what documents are missing" | "list missing documents for entity" |
| "link the LEI" | "GLEIF search and import" |

**Implementation**: Lightweight LLM call with domain vocabulary as context.
Only triggered when embedding search returns no results (score < 0.55).

```rust
// In process_as_natural_language, after NoMatch from verb search:
if candidates.is_empty() || candidates[0].score < 0.55 {
    if let Some(rewritten) = self.rewrite_query(instruction).await? {
        // Retry verb search with rewritten query
        candidates = self.verb_searcher.search(&rewritten, ...).await?;
    }
}
```

---

## Threshold Tuning

Current thresholds are conservative. With Phase 1-3 improvements, we can tune:

| Parameter | Current | Proposed | Rationale |
|-----------|---------|----------|-----------|
| semantic_threshold | 0.65 | 0.60 | With LLM classifier backing, lower threshold captures more |
| ambiguity_margin | 0.05 | 0.08 | Wider margin → fewer false ambiguities |
| fallback_threshold | 0.55 | 0.45 | With query rewriting, can retrieve more candidates |
| High-confidence skip | N/A | 0.85 | Skip LLM classifier when embedding is very confident |

---

## Measurement

### Key Metrics

1. **First-attempt hit rate**: % of utterances that reach `PipelineOutcome::Ready`
   on first try. Target: 70% (currently estimated ~35%).

2. **Two-attempt hit rate**: % reaching Ready within 2 turns (including
   disambiguation selection). Target: 90%.

3. **Verb selection accuracy**: When pipeline selects a verb, is it the right one?
   Measure via learning signal data (user corrections). Target: 95%.

4. **Latency P95**: End-to-end from utterance to DSL generation.
   Target: <1.5s (currently ~800ms without LLM classifier).

### A/B Testing

Deploy LLM classifier behind a feature flag. Compare:
- Control: embedding-only pipeline
- Treatment: embedding + LLM classifier

Measure first-attempt hit rate and user satisfaction (corrections per session).

---

## Implementation Priority

| Phase | Effort | Impact | Dependencies |
|-------|--------|--------|-------------|
| 1: Fill Data | 2 days | +25% | None |
| 2: LLM Classifier | 5 days | +30% | LLM service endpoint |
| 3: Contextual Scoring | 3 days | +15% | Phase 1 |
| 4: Query Rewriting | 3 days | +10% | Phase 2 |

Phase 1 is pure upside with zero risk — do it first. Phase 2 is the big unlock.
Phases 3-4 are refinements that compound on top.

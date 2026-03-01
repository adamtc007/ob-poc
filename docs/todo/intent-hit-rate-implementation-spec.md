# Intent Hit Rate — Implementation Spec

**Reference:** `docs/intent-hit-rate-strategy.md`
**Test Harness:** `rust/tests/intent_hit_rate.rs` + `rust/tests/fixtures/intent_test_utterances.toml`
**Owner:** Claude Code execution target

---

## Pre-flight Checks

Before starting ANY phase, verify these invariants hold:

```bash
# E-0: Existing tests pass
cargo test --lib -- verb_surface 2>&1 | tail -3
cargo test --test verb_search_integration -- --nocapture 2>&1 | tail -10

# E-1: Server compiles
cargo check --features database 2>&1 | tail -5

# E-2: YAML parses (all 37 verb files)
cargo test --lib -- test_load_verbs_config 2>&1 | tail -3
```

If any E-invariant fails, STOP and fix before proceeding.

---

## Continuation Protocol

Each phase ends with a GATE. Claude Code MUST:
1. Run the gate command
2. Paste the output
3. If gate PASSES → IMMEDIATELY proceed to next phase
4. If gate FAILS → fix and re-run gate (do NOT skip ahead)

Progress tracker (update after each gate):
```
Phase 1A: [ ] Merge draft phrases
Phase 1B: [ ] Generate missing phrases (271 verbs)
Phase 1C: [ ] Enrich existing phrases
Phase 1D: [ ] Sync + populate embeddings
Phase 2A: [ ] LlmIntentClassifier struct
Phase 2B: [ ] Wire into HybridVerbSearcher
Phase 2C: [ ] Wire into IntentPipeline
Phase 3A: [ ] Workflow domain boost
Phase 3B: [ ] Entity-type boost
Phase 3C: [ ] Session momentum boost
Phase 4A: [ ] Query rewriter
Phase 4B: [ ] Wire into pipeline NoMatch path
```

---

## Phase 1: Fill the Data

**Goal:** Raise invocation_phrase coverage from 52.4% to 100%.
**Impact:** +25% hit rate. **Risk: Zero** (data-only, no code changes).

### Phase 1A — Merge Draft Phrase Files

The two draft files contain phrases for verbs that already have some coverage.
Merge them into the domain YAML files.

**Source files:**
- `rust/config/verbs/_invocation_phrases_draft.yaml` (945 lines)
- `rust/config/verbs/_invocation_phrases_extension.yaml` (1327 lines)

**Target files:** `rust/config/verbs/{domain}.yaml` (37 files)

**Procedure:**
1. Parse each draft file — extract `{domain}.{verb}` → `invocation_phrases[]`
2. For each verb, APPEND new phrases to existing `invocation_phrases` in the
   domain YAML file (do NOT replace — merge, dedup)
3. After merge, rename draft files to `_invocation_phrases_draft.yaml.merged`
   and `_invocation_phrases_extension.yaml.merged`

**Constraints:**
- Phrases must be 3-10 words each
- No duplicate phrases within a verb
- Do not add the verb FQN itself as a phrase (e.g., "cbu.create" is NOT a phrase)
- Preserve existing phrase order (new phrases appended after existing)

**GATE 1A:**
```bash
# Verify YAML still parses
cargo test --lib -- test_load_verbs_config 2>&1 | tail -3
# Count: should show 342+ verbs with phrases (was 342)
python3 -c "
import yaml, glob
count = 0
for f in sorted(glob.glob('rust/config/verbs/*.yaml')):
    if '_' in f.split('/')[-1][0]: continue
    data = yaml.safe_load(open(f))
    if not data: continue
    for d in data.get('domains', {}).values():
        for v in d.get('verbs', {}).values():
            if v and v.get('invocation_phrases'): count += 1
print(f'Verbs with phrases: {count} (was 342, target 342+)')
"
```
→ IMMEDIATELY proceed to Phase 1B

---

### Phase 1B — Generate Missing Phrases (271 verbs)

**Target:** Every verb with `invocation_phrases: []` or missing `invocation_phrases`.

**Domains needing FULL phrase generation (0 verbs have phrases):**
- `capital` (21 verbs) — share class, issuance, splits, buybacks
- `sla` (17 verbs) — SLA definitions, metrics, breaches
- `lifecycle` (16 verbs) — entity lifecycle states, transitions
- `team` (15 verbs) — team management, assignments
- `pricing-config` (14 verbs) — fee schedules, rate cards
- `attribute` (11 verbs) — attribute registry, mappings
- `identifier` (11 verbs) — ISIN, SEDOL, LEI identifiers
- `service-resource` (10 verbs) — custody/TA/FA services
- `cash-sweep` (9 verbs) — cash management
- `matrix-overlay` (9 verbs) — trading matrix overlays
- `temporal` (8 verbs) — temporal versioning
- `batch` (7 verbs) — batch operations
- `investment-manager` (7 verbs) — IM relationships
- `user` (7 verbs) — user preferences
- `semantic` (6 verbs) — semantic registry

**Domains needing PARTIAL phrase generation:**
- `cbu` — 14 of 19 verbs missing (add-product, assign-role, decide, delete, etc.)
- `entity` — 12 of 21 missing
- `gleif` — 10 of 16 missing
- `ownership` — 10 of 22 missing
- `cbu.role` — 8 of 10 missing
- `graph` — 7 of 10 missing
- `client-group` — 5 of 23 missing
- `session` — 5 of 18 missing
- `control` — 5 of 16 missing
- `bods` — 6 of 9 missing

**Phrase generation rules:**
1. 8-12 phrases per verb
2. Each phrase 3-8 words
3. Mix of:
   - Direct imperative: "create a share class"
   - Question form: "what share classes exist?"
   - Contextual: "I need to set up the ISIN"
   - Abbreviated: "new share class"
4. Use REAL custody banking vocabulary:
   - "NAV" not "net asset value calculation"
   - "TA" not "transfer agency service"
   - "CDD pack" not "customer due diligence document collection"
   - "PSC register" not "persons with significant control register"
   - "OFAC" not "Office of Foreign Assets Control"
5. Do NOT include verb name or FQN as a phrase
6. Phrases should reflect what an onboarding analyst ACTUALLY SAYS

**Implementation approach:**
- Process one domain YAML file at a time
- For each verb missing phrases, read its `description` and `args` to understand purpose
- Generate phrases inline in the YAML under each verb's `invocation_phrases:` key
- Validate YAML after each file

**GATE 1B:**
```bash
cargo test --lib -- test_load_verbs_config 2>&1 | tail -3
python3 -c "
import yaml, glob
total = 0; with_p = 0
for f in sorted(glob.glob('rust/config/verbs/*.yaml')):
    if '_' in f.split('/')[-1][0]: continue
    data = yaml.safe_load(open(f))
    if not data: continue
    for d in data.get('domains', {}).values():
        for v in d.get('verbs', {}).values():
            total += 1
            if v and v.get('invocation_phrases'): with_p += 1
print(f'Coverage: {with_p}/{total} ({with_p*100//total}%) — target: 100%')
assert with_p == total, f'FAIL: {total - with_p} verbs still missing phrases'
print('GATE 1B: PASS')
"
```
→ IMMEDIATELY proceed to Phase 1C

---

### Phase 1C — Enrich Existing Low-Quality Phrases

Many existing phrases are too short and too technical. Enrich the TOP 50 verbs
(by expected usage frequency) with additional natural-language phrases.

**Top 50 verbs to enrich** (must have ≥10 phrases after enrichment):
```
cbu.create, cbu.list, cbu.update, cbu.assign-role, cbu.role.assign,
entity.create-limited-company, entity.create-proper-person, entity.list, entity.update,
screening.sanctions, screening.pep, screening.adverse-media,
ubo.list-ubos, ubo.add-ownership, ubo.add-control, ubo.calculate, ubo.trace-chains,
fund.create-umbrella, fund.create-subfund, fund.create-share-class,
fund.list-subfunds, fund.list-share-classes,
document.missing-for-entity, document.solicit, document.verify, document.upload-version,
deal.create, deal.get, deal.list, deal.add-participant, deal.create-rate-card,
deal.update-status,
gleif.search, gleif.import-tree, gleif.import-to-client-group,
ownership.compute, ownership.who-controls, ownership.trace-chain,
control.list-controllers, control.list-controlled,
view.universe, view.book, view.cbu, view.zoom-in, view.zoom-out,
session.load-cluster, session.set-client, session.load-deal,
client-group.discover-entities, client-group.list-relationships,
agent.teach
```

**Enrichment pattern — add these phrase TYPES if missing:**
- Business language: "onboard a new client" for cbu.create
- Question form: "who are the beneficial owners?" for ubo.list-ubos
- Abbreviated jargon: "run the AML checks" for screening.sanctions
- Result-oriented: "show me the ownership structure" for ubo.list-owners
- Problem statement: "what documents are still outstanding?" for document.missing-for-entity

**GATE 1C:**
```bash
cargo test --lib -- test_load_verbs_config 2>&1 | tail -3
python3 -c "
import yaml, glob
top50 = ['cbu.create','cbu.list','cbu.update','screening.sanctions','screening.pep',
  'entity.create-limited-company','entity.list','ubo.list-ubos','ubo.add-ownership',
  'fund.create-umbrella','document.missing-for-entity','document.solicit',
  'deal.create','deal.list','gleif.search','ownership.compute','view.universe',
  'view.book','session.load-cluster','agent.teach']
found = {}
for f in sorted(glob.glob('rust/config/verbs/*.yaml')):
    if '_' in f.split('/')[-1][0]: continue
    data = yaml.safe_load(open(f))
    if not data: continue
    for dn, dd in data.get('domains', {}).items():
        for vn, vd in dd.get('verbs', {}).items():
            fqn = f'{dn}.{vn}'
            if fqn in top50:
                found[fqn] = len(vd.get('invocation_phrases', []))
low = {k:v for k,v in found.items() if v < 10}
if low:
    for k,v in sorted(low.items()):
        print(f'  NEEDS MORE: {k} has {v} phrases (need ≥10)')
    print(f'GATE 1C: FAIL — {len(low)} verbs below 10 phrases')
else:
    print('GATE 1C: PASS — all sampled verbs have ≥10 phrases')
"
```
→ IMMEDIATELY proceed to Phase 1D

---

### Phase 1D — Sync Phrases to DB and Populate Embeddings

**Step 1: Sync YAML → database**

The server does this on startup via `VerbSyncService::sync_all()` which calls
`sync_invocation_phrases()` (file: `rust/src/session/verb_sync.rs:231`).

For manual sync without full server restart:
```bash
# Option A: restart server (handles sync automatically)
# Option B: direct SQL sync (if server is running)
cargo test --test verb_search_integration -- test_verb_sync --nocapture
```

**Step 2: Populate embeddings**

File: `rust/crates/ob-semantic-matcher/src/bin/populate_embeddings.rs`

```bash
cd rust && cargo run --release --bin populate_embeddings
```

This reads from `v_verb_intent_patterns` view (migration 038) which UNIONs
`yaml_intent_patterns` + `intent_patterns` (learned), computes BGE-small-en-v1.5
embeddings (384-dim), and writes to `verb_pattern_embeddings` table.

**Step 3: Verify embedding coverage**

```sql
-- Should show 0 verbs without embeddings
SELECT v.full_name
FROM "ob-poc".dsl_verbs v
LEFT JOIN "ob-poc".verb_pattern_embeddings e ON v.full_name = e.verb
WHERE e.verb IS NULL
  AND v.full_name NOT LIKE 'test.%';
```

**GATE 1D:**
```bash
# Run the intent hit rate test
DATABASE_URL=$DATABASE_URL INTENT_VERBOSE=1 \
  cargo test --test intent_hit_rate -- --nocapture 2>&1 | tail -30

# Baseline metrics — record these numbers
# Expected after Phase 1: first-attempt ≥45%, two-attempt ≥65%
```
→ IMMEDIATELY proceed to Phase 2A (progress: 30%)

---

## Phase 2: LLM Intent Classifier

**Goal:** Add LLM-powered verb selection for ambiguous embedding matches.
**Impact:** +30% hit rate. **Risk: Medium** (new code, LLM latency).

### Phase 2A — Create LlmIntentClassifier

**New file:** `rust/src/mcp/llm_intent_classifier.rs`

**Struct:**
```rust
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use ob_agentic::llm_client::LlmClient;
use super::verb_search::VerbSearchResult;

/// LLM-powered intent classifier for ambiguous verb selection.
///
/// Invoked when embedding search returns candidates in the ambiguous range
/// (0.45-0.85). Uses a two-pass strategy:
/// 1. Domain classification (12 choices)
/// 2. Verb selection within domain (5-50 choices)
///
/// Or single-pass with top-5 candidates + domain verb list.
pub struct LlmIntentClassifier {
    llm_client: Arc<dyn LlmClient>,
    /// Cache: utterance hash → (verb_fqn, confidence, timestamp)
    cache: tokio::sync::RwLock<HashMap<u64, CachedClassification>>,
    /// Cache TTL in seconds
    cache_ttl_secs: u64,
}

pub struct ClassificationResult {
    pub verb: String,
    pub confidence: f32,
    pub reason: String,
}

struct CachedClassification {
    verb: String,
    confidence: f32,
    reason: String,
    cached_at: std::time::Instant,
}

/// Context passed from orchestrator to improve classification accuracy
#[derive(Debug, Clone, Default)]
pub struct ClassificationContext {
    /// Current workflow (e.g., "semos-kyc", "semos-onboarding")
    pub stage_focus: Option<String>,
    /// Dominant entity kind from entity linking (e.g., "limited-company", "fund")
    pub entity_kind: Option<String>,
    /// Domain hint from UI or previous verb
    pub domain_hint: Option<String>,
    /// Last 3 executed verbs (for momentum)
    pub recent_verbs: Vec<String>,
}
```

**Key method — `classify()`:**

```rust
impl LlmIntentClassifier {
    pub fn new(llm_client: Arc<dyn LlmClient>) -> Self { ... }

    /// Classify user intent using LLM reasoning.
    ///
    /// Only called when embedding search is ambiguous (top score 0.45-0.85).
    /// Returns None if LLM confidence < 0.70 (fall through to embedding result).
    pub async fn classify(
        &self,
        utterance: &str,
        embedding_candidates: &[VerbSearchResult],
        context: &ClassificationContext,
    ) -> Result<Option<ClassificationResult>> {
        // 1. Check cache
        // 2. Build prompt with top-5 candidates + domain context
        // 3. Call LLM via chat_json()
        // 4. Parse response, validate verb exists in registry
        // 5. Cache result
        // 6. Return if confidence >= 0.70
    }
}
```

**Prompt design (single-pass variant):**

The prompt MUST include:
1. The user's utterance
2. The top-5 embedding candidates with scores and descriptions
3. Workflow context (if available)
4. Entity type context (if available)
5. A COMPLETE list of verbs in the top candidate's domain (for recall)

The prompt MUST instruct:
- Return JSON: `{"verb": "domain.verb", "confidence": 0.0-1.0, "reason": "..."}`
- Consider workflow context when disambiguating
- "confidence" should reflect certainty, not similarity
- If no verb fits well, return confidence < 0.5

**Implementation constraints:**
- Use `llm_client.chat_json()` for structured output
- Cache with 1-hour TTL, keyed on `hash(utterance + stage_focus + entity_kind)`
- Timeout: 3 seconds. If LLM doesn't respond, return None (fall through)
- Do NOT call LLM if `embedding_candidates` is empty

**New enum variant needed:**
```rust
// In verb_search.rs, add to VerbSearchSource:
/// LLM intent classifier (reasoning over embedding candidates)
LlmClassifier,
```

**Files to create/modify:**
| File | Action |
|------|--------|
| `rust/src/mcp/llm_intent_classifier.rs` | CREATE — full implementation |
| `rust/src/mcp/mod.rs` | MODIFY — add `pub mod llm_intent_classifier;` |
| `rust/src/mcp/verb_search.rs:87` | MODIFY — add `LlmClassifier` to `VerbSearchSource` |

**GATE 2A:**
```bash
cargo check --features database 2>&1 | tail -5
cargo test --lib -- llm_intent_classifier 2>&1 | tail -5
# Unit tests for: prompt building, cache hit/miss, timeout handling, JSON parsing
```
→ IMMEDIATELY proceed to Phase 2B (progress: 45%)

---

### Phase 2B — Wire LlmIntentClassifier into HybridVerbSearcher

**File:** `rust/src/mcp/verb_search.rs`

**Changes:**

1. Add field to `HybridVerbSearcher`:
```rust
// After line ~260 (in struct fields)
/// LLM intent classifier for ambiguous embedding matches
llm_classifier: Option<Arc<LlmIntentClassifier>>,
```

2. Add builder method:
```rust
pub fn with_llm_classifier(mut self, classifier: Arc<LlmIntentClassifier>) -> Self {
    self.llm_classifier = Some(classifier);
    self
}
```

3. Add classification context parameter to `search()`:
```rust
// Change signature (line 370):
pub async fn search(
    &self,
    query: &str,
    user_id: Option<Uuid>,
    domain_filter: Option<&str>,
    limit: usize,
    allowed_verbs: Option<&HashSet<String>>,
    classification_ctx: Option<&ClassificationContext>,  // NEW
) -> Result<Vec<VerbSearchResult>> {
```

4. Insert LLM classifier AFTER Tier 6 (global semantic), BEFORE Tier 7 (phonetic):

```rust
// After the global semantic search block (around line 610), before phonetic:

// Tier 6.5: LLM Intent Classifier (when embeddings are ambiguous)
let top_score = results.first().map(|r| r.score).unwrap_or(0.0);
if let Some(ref classifier) = self.llm_classifier {
    // Only invoke LLM when embedding is ambiguous: has candidates but not confident
    if top_score >= 0.45 && top_score < 0.85 && results.len() >= 2 {
        let ctx = classification_ctx
            .cloned()
            .unwrap_or_default();
        match classifier.classify(query, &results, &ctx).await {
            Ok(Some(classification)) if classification.confidence >= 0.70 => {
                // LLM is confident — insert at top of results
                let description = self.get_verb_description(&classification.verb).await;
                // Only use LLM result if verb passes allowed_verbs filter
                let passes_filter = allowed_verbs
                    .map(|av| av.contains(&classification.verb))
                    .unwrap_or(true);
                if passes_filter && !seen_verbs.contains(&classification.verb) {
                    tracing::info!(
                        verb = %classification.verb,
                        confidence = classification.confidence,
                        reason = %classification.reason,
                        "VerbSearch: LLM classifier override"
                    );
                    seen_verbs.insert(classification.verb.clone());
                    results.insert(0, VerbSearchResult {
                        verb: classification.verb,
                        score: classification.confidence,
                        source: VerbSearchSource::LlmClassifier,
                        matched_phrase: format!("LLM: {}", classification.reason),
                        description,
                    });
                }
            }
            Ok(_) => {
                tracing::debug!("VerbSearch: LLM classifier returned low confidence or None");
            }
            Err(e) => {
                tracing::warn!(error = %e, "VerbSearch: LLM classifier failed, continuing with embedding results");
            }
        }
    }
}
```

5. **Update ALL call sites** of `search()` to pass the new parameter.

Call sites to update (add `None` for now — Phase 2C will thread context):

| File | Line | Current | Change |
|------|------|---------|--------|
| `rust/src/mcp/intent_pipeline.rs` | ~505 | `.search(instruction, None, domain_filter, 5, self.allowed_verbs.as_ref())` | Add `, None` |
| `rust/src/mcp/handlers/core.rs` | (search calls) | Various | Add `, None` |
| `rust/tests/verb_search_integration.rs` | (all test search calls) | Various | Add `, None` |
| `rust/tests/intent_hit_rate.rs` | (~line 155) | `.search(&case.utterance, ...)` | Add `, None` |

**GATE 2B:**
```bash
cargo check --features database 2>&1 | tail -5
# ALL existing tests must still pass:
cargo test --lib -- verb_surface 2>&1 | tail -3
cargo test --test verb_search_integration -- --nocapture 2>&1 | tail -10
```
→ IMMEDIATELY proceed to Phase 2C (progress: 55%)

---

### Phase 2C — Thread ClassificationContext from Orchestrator

**File:** `rust/src/agent/orchestrator.rs`

In `handle_utterance()` (line 179), after entity linking (Step 1) and before
pipeline invocation (Stage A), build the classification context:

```rust
// After line ~260 (after dominant_entity_kind is computed):
let classification_ctx = crate::mcp::llm_intent_classifier::ClassificationContext {
    stage_focus: ctx.stage_focus.clone(),
    entity_kind: dominant_entity_kind.clone(),
    domain_hint: None, // Could come from session.domain_hint
    recent_verbs: vec![], // Phase 3C will populate this
};
```

Then pass it into `process_with_scope()`. This requires:

1. Add `classification_ctx: Option<ClassificationContext>` field to `IntentPipeline`
2. Add `with_classification_context()` builder method
3. Thread it through `process_with_scope()` → `process_as_natural_language()` → `verb_searcher.search()`

**File changes:**

| File | Change |
|------|--------|
| `rust/src/mcp/intent_pipeline.rs:190` | Add `classification_ctx: Option<ClassificationContext>` field |
| `rust/src/mcp/intent_pipeline.rs` | Add `pub fn with_classification_context(mut self, ctx: ClassificationContext) -> Self` |
| `rust/src/mcp/intent_pipeline.rs:505` | Pass `self.classification_ctx.as_ref()` to `search()` |
| `rust/src/agent/orchestrator.rs:~275` | Build context and pass via `pipeline.with_classification_context(ctx)` |

**Also wire LlmIntentClassifier into AgentState:**

| File | Change |
|------|--------|
| `rust/src/api/agent_state.rs` | Add `pub llm_classifier: Option<Arc<LlmIntentClassifier>>` |
| `rust/src/api/agent_state.rs` (init) | Create classifier from `create_llm_client()` if available |
| `rust/src/api/agent_service.rs` (build_orchestrator_context) | Pass classifier to verb_searcher |

**GATE 2C:**
```bash
cargo check --features database 2>&1 | tail -5
cargo test --lib -- verb_surface 2>&1 | tail -3
# Run intent hit rate — expect improvement over Phase 1D baseline
DATABASE_URL=$DATABASE_URL INTENT_VERBOSE=1 \
  cargo test --test intent_hit_rate -- --nocapture 2>&1 | tail -30
# Expected: first-attempt ≥55%, two-attempt ≥75%
```
→ IMMEDIATELY proceed to Phase 3A (progress: 65%)

---

## Phase 3: Contextual Scoring

**Goal:** Boost verb scores based on workflow, entity type, and session history.
**Impact:** +15% hit rate. **Risk: Low.**

### Phase 3A — Workflow Domain Boost

**File:** `rust/src/mcp/verb_search.rs`

Add a post-processing function called AFTER dedup but BEFORE `allowed_verbs` filtering:

```rust
/// Boost verbs in domains matching the current workflow stage_focus.
///
/// Applied after candidate retrieval but before SemReg filtering.
/// Boost is additive (+0.08) and capped at 1.0.
fn apply_workflow_boost(results: &mut [VerbSearchResult], stage_focus: Option<&str>) {
    let boost_domains: &[&str] = match stage_focus {
        Some("semos-kyc") => &["kyc", "screening", "document", "ubo", "entity", "requirement"],
        Some("semos-onboarding") => &["cbu", "entity", "fund", "deal", "contract", "booking-principal"],
        Some("semos-data-management") => &["attribute", "identifier", "gleif", "graph", "semantic"],
        Some("semos-stewardship") => &["lifecycle", "control", "ownership", "temporal", "monitoring"],
        _ => return,
    };

    for result in results.iter_mut() {
        let domain = result.verb.split('.').next().unwrap_or("");
        if boost_domains.contains(&domain) {
            let old = result.score;
            result.score = (result.score + 0.08).min(1.0);
            tracing::trace!(
                verb = %result.verb,
                old_score = old,
                new_score = result.score,
                stage = stage_focus.unwrap_or("none"),
                "Workflow domain boost applied"
            );
        }
    }
}
```

**Integration point:** Inside `search()`, call after `normalize_candidates()` (around line ~640):

```rust
// After: let mut results = normalize_candidates(results, limit);
// Before: if let Some(allowed) = allowed_verbs {

// Apply contextual boosts
if let Some(ctx) = classification_ctx {
    apply_workflow_boost(&mut results, ctx.stage_focus.as_deref());
}

// Re-sort after boost (scores may have changed)
results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
```

**GATE 3A:**
```bash
cargo test --lib -- verb_surface 2>&1 | tail -3
cargo check --features database 2>&1 | tail -5
```
→ IMMEDIATELY proceed to Phase 3B (progress: 75%)

---

### Phase 3B — Entity-Type Boost

**File:** `rust/src/mcp/verb_search.rs`

```rust
/// Boost verbs that operate on the dominant entity type in session.
///
/// When entity linking resolves a dominant entity (e.g., user is talking about
/// a fund), boost verbs that naturally operate on that entity type.
fn apply_entity_type_boost(results: &mut [VerbSearchResult], entity_kind: Option<&str>) {
    let boost_prefixes: &[&str] = match entity_kind {
        Some("fund") | Some("umbrella") | Some("subfund") | Some("share-class") =>
            &["fund.", "ownership.", "capital.", "cash-sweep."],
        Some("limited-company") | Some("partnership-limited") =>
            &["entity.", "control.", "ubo.", "gleif.", "bods."],
        Some("proper-person") =>
            &["ubo.", "screening.", "entity."],
        Some("trust-discretionary") =>
            &["ubo.", "entity.", "delegation."],
        Some("cbu") =>
            &["cbu.", "cbu.role.", "document.", "deal."],
        Some("deal") =>
            &["deal.", "contract.", "billing.", "sla."],
        Some("client-group") =>
            &["client-group.", "gleif.", "bods."],
        _ => return,
    };

    for result in results.iter_mut() {
        if boost_prefixes.iter().any(|p| result.verb.starts_with(p)) {
            result.score = (result.score + 0.05).min(1.0);
        }
    }
}
```

**Integration:** Call immediately after `apply_workflow_boost()`:
```rust
if let Some(ctx) = classification_ctx {
    apply_workflow_boost(&mut results, ctx.stage_focus.as_deref());
    apply_entity_type_boost(&mut results, ctx.entity_kind.as_deref());
}
```

**GATE 3B:**
```bash
cargo check --features database 2>&1 | tail -5
```
→ IMMEDIATELY proceed to Phase 3C (progress: 80%)

---

### Phase 3C — Session Momentum Boost

**File:** `rust/src/mcp/verb_search.rs`

```rust
/// Boost verbs that follow natural workflow sequences from recent verbs.
///
/// Uses a static lookup of common verb-to-verb transitions in custody banking.
fn apply_momentum_boost(results: &mut [VerbSearchResult], recent_verbs: &[String]) {
    if recent_verbs.is_empty() {
        return;
    }

    // Common workflow progressions
    let sequences: &[(&str, &[&str])] = &[
        ("cbu.create", &["entity.create-limited-company", "cbu.assign-role", "cbu.add-product", "cbu.role.assign"]),
        ("entity.create-limited-company", &["cbu.assign-role", "cbu.role.assign", "gleif.search", "screening.sanctions"]),
        ("entity.create-proper-person", &["ubo.add-ownership", "ubo.add-control", "screening.sanctions", "screening.pep"]),
        ("screening.sanctions", &["screening.pep", "screening.adverse-media", "document.solicit"]),
        ("screening.pep", &["screening.adverse-media", "document.solicit"]),
        ("ubo.add-ownership", &["ubo.add-control", "ubo.calculate", "ownership.compute"]),
        ("ubo.calculate", &["ubo.list-ubos", "ubo.trace-chains", "ownership.who-controls"]),
        ("deal.create", &["deal.add-participant", "deal.add-product", "deal.create-rate-card"]),
        ("deal.add-participant", &["deal.add-product", "deal.create-rate-card"]),
        ("fund.create-umbrella", &["fund.create-subfund", "fund.create-share-class"]),
        ("fund.create-subfund", &["fund.create-share-class", "fund.add-to-umbrella"]),
        ("gleif.search", &["gleif.import-tree", "gleif.enrich", "gleif.import-to-client-group"]),
        ("document.solicit", &["document.upload-version", "document.verify"]),
        ("document.upload-version", &["document.verify", "document.extract"]),
        ("client-group.start-discovery", &["client-group.discover-entities", "client-group.confirm-entity"]),
    ];

    let last = recent_verbs.last().map(|s| s.as_str()).unwrap_or("");
    if let Some((_, next_verbs)) = sequences.iter().find(|(prev, _)| *prev == last) {
        for result in results.iter_mut() {
            if next_verbs.contains(&result.verb.as_str()) {
                result.score = (result.score + 0.06).min(1.0);
            }
        }
    }
}
```

**Integration:** Add after entity_type_boost:
```rust
if let Some(ctx) = classification_ctx {
    apply_workflow_boost(&mut results, ctx.stage_focus.as_deref());
    apply_entity_type_boost(&mut results, ctx.entity_kind.as_deref());
    apply_momentum_boost(&mut results, &ctx.recent_verbs);
}
```

**Threading `recent_verbs` from session:**

In `orchestrator.rs` where `classification_ctx` is built, populate `recent_verbs`
from the session's last 3 executed verbs. The session trace has this data in
`session.messages` or could be tracked explicitly.

For now, add a `recent_verbs: Vec<String>` field to `UnifiedSessionContext` or
`SessionContext`, populated after each successful execution in `agent_service.rs`.

**GATE 3C:**
```bash
cargo check --features database 2>&1 | tail -5
cargo test --lib -- verb_surface 2>&1 | tail -3
# Full hit rate test
DATABASE_URL=$DATABASE_URL INTENT_VERBOSE=1 \
  cargo test --test intent_hit_rate -- --nocapture 2>&1 | tail -30
# Expected: first-attempt ≥65%, two-attempt ≥85%
```
→ IMMEDIATELY proceed to Phase 4A (progress: 85%)

---

## Phase 4: Query Rewriting

**Goal:** Handle NoMatch by rewriting natural language to canonical form.
**Impact:** +10% hit rate. **Risk: Medium.**

### Phase 4A — Query Rewriter

**New file:** `rust/src/mcp/query_rewriter.rs`

```rust
/// Rewrite natural language queries to canonical verb vocabulary.
///
/// Only invoked on NoMatch (embedding score < fallback_threshold).
/// Uses LLM to translate business language to system vocabulary.
pub struct QueryRewriter {
    llm_client: Arc<dyn LlmClient>,
}

impl QueryRewriter {
    pub async fn rewrite(&self, utterance: &str) -> Result<Option<String>> {
        // Prompt: "Rewrite this custody banking request using these verb categories: ..."
        // Return None if LLM can't map it
    }
}
```

**Prompt must include** the domain taxonomy (12 domains) and example mappings:
```
"check if they're on any lists" → "run sanctions screening"
"who owns this company" → "list beneficial owners"  
"what documents are missing" → "missing documents for entity"
"set up the fund structure" → "create umbrella fund"
```

### Phase 4B — Wire into Pipeline

**File:** `rust/src/mcp/intent_pipeline.rs`

In `process_as_natural_language()`, after the NoMatch return (around line 540):

```rust
// BEFORE returning NoMatch, try query rewriting
if candidates.is_empty() || candidates[0].score < self.verb_searcher.fallback_threshold() {
    if let Some(ref rewriter) = self.query_rewriter {
        if let Ok(Some(rewritten)) = rewriter.rewrite(instruction).await {
            tracing::info!(
                original = instruction,
                rewritten = %rewritten,
                "Query rewriter: retrying with canonical form"
            );
            // Retry search with rewritten query
            let retry_candidates = self.verb_searcher
                .search(&rewritten, None, domain_filter, 5, self.allowed_verbs.as_ref(), None)
                .await?;
            if !retry_candidates.is_empty() && retry_candidates[0].score >= threshold {
                // Rewrite succeeded — continue with rewritten candidates
                candidates = retry_candidates;
                // Fall through to ambiguity check below
            }
        }
    }
}
```

**GATE 4B (FINAL):**
```bash
cargo check --features database 2>&1 | tail -5
cargo test --lib -- verb_surface 2>&1 | tail -3
cargo test --test verb_search_integration -- --nocapture 2>&1 | tail -10

# FINAL HIT RATE MEASUREMENT
DATABASE_URL=$DATABASE_URL INTENT_VERBOSE=1 \
  cargo test --test intent_hit_rate -- --nocapture 2>&1 | tee /tmp/hit_rate_final.txt

# TARGET: first-attempt ≥70%, two-attempt ≥90%
grep "First-attempt\|Two-attempt" /tmp/hit_rate_final.txt
```

---

## E-Invariants (must hold after EVERY phase)

| ID | Assertion | Command |
|----|-----------|---------|
| E-0 | All existing tests pass | `cargo test --lib -- verb_surface` |
| E-1 | Server compiles | `cargo check --features database` |
| E-2 | YAML parses | `cargo test --lib -- test_load_verbs_config` |
| E-3 | Verb search integration passes | `cargo test --test verb_search_integration` |
| E-4 | No regressions in semantic threshold | Baseline hit rate does not decrease |

## File Change Summary

| Phase | Files Created | Files Modified |
|-------|--------------|----------------|
| 1A-1D | 0 | 37 YAML files in `rust/config/verbs/` |
| 2A | `rust/src/mcp/llm_intent_classifier.rs` | `rust/src/mcp/mod.rs`, `verb_search.rs` |
| 2B | 0 | `verb_search.rs`, `intent_pipeline.rs`, test files |
| 2C | 0 | `orchestrator.rs`, `agent_state.rs`, `agent_service.rs`, `intent_pipeline.rs` |
| 3A-3C | 0 | `verb_search.rs`, `orchestrator.rs` |
| 4A | `rust/src/mcp/query_rewriter.rs` | `rust/src/mcp/mod.rs` |
| 4B | 0 | `intent_pipeline.rs` |

# TODO — Unified Verb Discovery: Multi-Channel + Voting + Explainability

## Problem Summary

Current verb discovery has three issues:

1. **Channels disconnected** — `user_id=None` disables learned branches; `learned_data=None` kills global warmup map
2. **Early-exit drops evidence** — macro match returns immediately, no multi-channel voting possible  
3. **Feedback is blind** — captures `similarity=1.0` with empty alternatives, can't calibrate

**Root cause files:**
- `rust/src/mcp/intent_pipeline.rs` → `verb_searcher.search(instruction, None, ...)`
- `rust/src/api/agent_service.rs` → `HybridVerbSearcher::new(verb_service, None)`
- `rust/src/mcp/verb_search.rs` → macro early-return + `seen_verbs` dedupe
- `rust/src/api/agent_routes.rs` → `capture_match(..., &[])`

---

## Target Architecture
```
┌─────────────────────────────────────────────────────────────────┐
│  Input: "create a cbu for acme"                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  HybridVerbSearcher (via factory)                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐               │
│  │ Macro Exact │ │ User Learned│ │Global Learned│ ...          │
│  └──────┬──────┘ └──────┬──────┘ └──────┬───────┘              │
│         │               │               │                       │
│         └───────────────┴───────────────┘                       │
│                         │                                       │
│                         ▼                                       │
│              HashMap<verb, Accumulator>                         │
│              → merge evidence per verb                          │
│              → fused_score = max(raw scores)  [Phase 1]         │
│              → fused_score = weighted         [Phase 2, later]  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  IntentPipeline: select winner                                  │
│  • Existing threshold logic unchanged (0.55/0.65 calibrated)    │
│  • Return winner + candidates + evidence                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Feedback capture: winner + alternatives + real scores          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 0: Warmup + Macro Registry in AgentState

**Problem:** Web agent (`AgentState`) never calls warmup, so `learned_data` is always `None` in chat even after we wire it.

**File:** `rust/src/api/agent_routes.rs`

In `AgentState::with_semantic(...)`, add warmup:
```rust
use crate::agent::learning::warmup::LearningWarmup;

// After pool/embedder setup, before returning AgentState:
let warmup = LearningWarmup::new(pool.clone());
let (learned_data, stats) = warmup.warmup().await.expect("Learning warmup failed");
tracing::info!(?stats, "Learning warmup loaded");
```

**File:** `rust/src/api/agent_service.rs`

Add field to `AgentService`:
```rust
pub struct AgentService {
    // ... existing fields ...
    learned_data: Option<SharedLearnedData>,
    macro_registry: Arc<OperatorMacroRegistry>,
}
```

Update constructor to accept these and store them.

**File:** `rust/src/api/agent_routes.rs`

Load macro registry once at startup (not per-request):
```rust
use crate::macros::OperatorMacroRegistry;

// In AgentState::with_semantic or similar init:
let macro_registry = Arc::new(
    OperatorMacroRegistry::load_from_directory("config/verb_schemas/macros")
        .expect("Failed to load macro registry")
);
```

Pass `learned_data` and `macro_registry` into `AgentService`.

---

## Phase 1: Wire Types (ob-poc-types)

**File:** `rust/crates/ob-poc-types/src/lib.rs`

Add these types near `ChatResponse` (~line 703):
```rust
// ============================================================================
// VERB MATCH SOURCE (shared enum for evidence attribution)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VerbMatchSource {
    UserLearnedExact,
    UserLearnedSemantic,
    LearnedExact,
    LearnedSemantic,
    Semantic,
    DirectDsl,
    GlobalLearned,
    PatternEmbedding,
    Phonetic,
    Macro,
    #[serde(other)]
    Unknown,
}

// ============================================================================
// CHAT DEBUG / EXPLAINABILITY (optional, gated by OB_CHAT_DEBUG=1)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDebugInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_match: Option<VerbMatchDebug>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbMatchDebug {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<VerbCandidateDebug>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<VerbCandidateDebug>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<VerbSelectionPolicyDebug>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidateDebug {
    pub verb: String,
    pub score: f32,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_source: Option<VerbMatchSource>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_phrase: Option<String>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<VerbEvidenceDebug>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbEvidenceDebug {
    pub source: VerbMatchSource,
    pub score: f32,
    pub matched_phrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSelectionPolicyDebug {
    pub algorithm: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambiguity_margin: Option<f32>,
}
```

Add field to `ChatResponse`:
```rust
pub struct ChatResponse {
    // ... existing fields ...
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<ChatDebugInfo>,
}
```

---

## Phase 2: Evidence Structs in verb_search

**File:** `rust/src/mcp/verb_search.rs`

Add internal evidence type:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerbEvidence {
    pub source: VerbSearchSource,
    pub score: f32,
    pub matched_phrase: String,
}
```

Extend `VerbSearchResult`:
```rust
pub struct VerbSearchResult {
    pub verb: String,
    pub description: Option<String>,  // NOTE: keep as Option<String>
    pub source: VerbSearchSource,
    pub score: f32,
    pub matched_phrase: String,
    
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<VerbEvidence>,  // NEW
}
```

---

## Phase 3: Verb Search Factory

**New file:** `rust/src/mcp/verb_search_factory.rs`
```rust
use std::sync::Arc;
use sqlx::PgPool;

use crate::agent::learning::embedder::SharedEmbedder;
use crate::agent::learning::warmup::SharedLearnedData;
use crate::database::VerbService;
use crate::macros::OperatorMacroRegistry;
use crate::mcp::verb_search::HybridVerbSearcher;

pub struct VerbSearcherFactory;

impl VerbSearcherFactory {
    pub fn build(
        pool: &PgPool,
        embedder: SharedEmbedder,
        learned_data: Option<SharedLearnedData>,
        macro_registry: Arc<OperatorMacroRegistry>,
    ) -> HybridVerbSearcher {
        let verb_service = Arc::new(VerbService::new(pool.clone()));

        HybridVerbSearcher::new(verb_service, learned_data)
            .with_embedder(embedder)
            .with_macro_registry(macro_registry)
    }
}
```

**File:** `rust/src/mcp/mod.rs`

Add:
```rust
pub mod verb_search_factory;
```

---

## Phase 4: Refactor HybridVerbSearcher.search() — Evidence Accumulation

**File:** `rust/src/mcp/verb_search.rs`

**CRITICAL:** Do NOT change score computation yet. Use `max(raw_scores)` to preserve calibrated thresholds.

Add search mode enum:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    Fast,      // early-return on macro exact (current behavior)
    #[default]
    Ensemble,  // all channels, accumulate evidence
}
```

Add accumulator (internal, not exported):
```rust
struct VerbAccumulator {
    verb: String,
    description: Option<String>,
    evidence: Vec<VerbEvidence>,
}

impl VerbAccumulator {
    fn new(verb: String, description: Option<String>) -> Self {
        Self { verb, description, evidence: vec![] }
    }
    
    fn push(&mut self, source: VerbSearchSource, score: f32, phrase: String) {
        self.evidence.push(VerbEvidence { 
            source, 
            score, 
            matched_phrase: phrase 
        });
    }
    
    fn update_description(&mut self, desc: Option<String>) {
        if self.description.is_none() && desc.is_some() {
            self.description = desc;
        }
    }
    
    /// IMPORTANT: Use max(raw) to preserve threshold calibration.
    /// Weighted fusion deferred to Phase 2.
    fn fused_score(&self) -> f32 {
        self.evidence.iter().map(|e| e.score).fold(0.0_f32, f32::max)
    }
    
    fn best_source(&self) -> VerbSearchSource {
        self.evidence
            .iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
            .map(|e| e.source.clone())
            .unwrap_or(VerbSearchSource::Semantic)
    }
    
    fn best_phrase(&self) -> String {
        self.evidence
            .iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
            .map(|e| e.matched_phrase.clone())
            .unwrap_or_default()
    }
    
    fn into_result(self) -> VerbSearchResult {
        VerbSearchResult {
            verb: self.verb,
            description: self.description,
            source: self.best_source(),
            score: self.fused_score(),
            matched_phrase: self.best_phrase(),
            evidence: self.evidence,
        }
    }
}
```

Refactor `search()` to use accumulator pattern:
```rust
pub async fn search(
    &self,
    input: &str,
    user_id: Option<Uuid>,
    domain_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<VerbSearchResult>> {
    let mode = self.mode.unwrap_or_default();
    let mut accumulators: HashMap<String, VerbAccumulator> = HashMap::new();
    
    let normalized = normalize_input(input);

    // ─────────────────────────────────────────────────────────────
    // 1. Macro channel
    // ─────────────────────────────────────────────────────────────
    if let Some(registry) = &self.macro_registry {
        for candidate in registry.search(&normalized) {
            let acc = accumulators
                .entry(candidate.verb.clone())
                .or_insert_with(|| VerbAccumulator::new(
                    candidate.verb.clone(), 
                    candidate.description.clone()
                ));
            acc.push(VerbSearchSource::Macro, candidate.score, candidate.matched_phrase.clone());
            acc.update_description(candidate.description.clone());
        }
        
        // Fast mode: early-return if macro exact (preserves current behavior)
        if mode == SearchMode::Fast {
            if let Some(acc) = accumulators.values().find(|a| a.fused_score() >= 0.99) {
                return Ok(vec![acc.clone().into_result()]);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────
    // 2. Global learned exact (sync map lookup, not async search)
    // ─────────────────────────────────────────────────────────────
    if let Some(learned) = &self.learned_data {
        let guard = learned.read().expect("learned_data lock poisoned");
        if let Some(verb) = guard.resolve_phrase(&normalized) {
            let acc = accumulators
                .entry(verb.to_string())
                .or_insert_with(|| VerbAccumulator::new(verb.to_string(), None));
            acc.push(VerbSearchSource::LearnedExact, 1.0, normalized.clone());
        }
    }

    // ─────────────────────────────────────────────────────────────
    // 3. User learned exact (if user_id provided)
    // ─────────────────────────────────────────────────────────────
    if let (Some(learned), Some(uid)) = (&self.learned_data, user_id) {
        // Check if there's a user-specific learned exact method
        // If not, skip this channel for now
        // TODO: Implement user-specific exact lookup if needed
    }

    // ─────────────────────────────────────────────────────────────
    // 4. Semantic search (pgvector / pattern embeddings)
    // ─────────────────────────────────────────────────────────────
    if let Some(embedder) = &self.embedder {
        let semantic_results = self.verb_service
            .search_semantic(&normalized, embedder, domain_filter, limit)
            .await?;
            
        for candidate in semantic_results {
            let acc = accumulators
                .entry(candidate.verb.clone())
                .or_insert_with(|| VerbAccumulator::new(
                    candidate.verb.clone(), 
                    candidate.description.clone()
                ));
            acc.push(VerbSearchSource::PatternEmbedding, candidate.score, candidate.matched_phrase.clone());
            acc.update_description(candidate.description.clone());
        }
    }

    // ─────────────────────────────────────────────────────────────
    // 5. Phonetic fallback
    // ─────────────────────────────────────────────────────────────
    let phonetic_results = self.verb_service
        .search_phonetic(&normalized, domain_filter, limit)
        .await?;
        
    for candidate in phonetic_results {
        let acc = accumulators
            .entry(candidate.verb.clone())
            .or_insert_with(|| VerbAccumulator::new(
                candidate.verb.clone(), 
                candidate.description.clone()
            ));
        acc.push(VerbSearchSource::Phonetic, candidate.score, candidate.matched_phrase.clone());
        acc.update_description(candidate.description.clone());
    }

    // ─────────────────────────────────────────────────────────────
    // Finalize: sort by fused score, truncate
    // ─────────────────────────────────────────────────────────────
    let mut results: Vec<VerbSearchResult> = accumulators
        .into_values()
        .map(|a| a.into_result())
        .collect();
    
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    
    Ok(results)
}
```

---

## Phase 5: Wire user_id in IntentPipeline

**File:** `rust/src/mcp/intent_pipeline.rs`

Add helper (note: `user_id` is `Uuid`, not `Option<Uuid>`):
```rust
fn effective_user_id(&self) -> Uuid {
    self.session
        .as_ref()
        .and_then(|s| s.read().ok().map(|g| g.user_id))
        .unwrap_or(Uuid::nil())
}
```

Update `process_as_natural_language()`:
```rust
// Before:
let candidates = self.verb_searcher.search(instruction, None, domain_filter, 5).await?;

// After:
let candidates = self.verb_searcher
    .search(instruction, Some(self.effective_user_id()), domain_filter, 5)
    .await?;
```

---

## Phase 6: Wire Factory into AgentService

**File:** `rust/src/api/agent_service.rs`

In `get_intent_pipeline()` or wherever `HybridVerbSearcher` is constructed:
```rust
use crate::mcp::verb_search_factory::VerbSearcherFactory;

// Before:
let verb_searcher = HybridVerbSearcher::new(verb_service, None);

// After:
let verb_searcher = VerbSearcherFactory::build(
    &self.pool,
    self.embedder.clone(),
    self.learned_data.clone(),  // now Some(...) from Phase 0
    self.macro_registry.clone(),
);
```

Add `debug` field to `AgentChatResponse`:
```rust
pub struct AgentChatResponse {
    // ... existing fields ...
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<ob_poc_types::ChatDebugInfo>,
}
```

Update ALL struct literal constructors (`staged_response()`, `ok_response()`, `fail()`, direct literals) to include `debug: None,`.

---

## Phase 7: Wire Factory into MCP Handlers

**File:** `rust/src/mcp/handlers/core.rs`

Use factory instead of manual construction:
```rust
use crate::mcp::verb_search_factory::VerbSearcherFactory;

let verb_searcher = VerbSearcherFactory::build(
    &pool,
    embedder.clone(),
    Some(learned_data.clone()),
    macro_registry.clone(),
);
```

---

## Phase 8: Route Mapping — Pass Debug Through

**File:** `rust/src/api/agent_routes.rs`

Add `debug: None,` to ALL `ChatResponse { ... }` literals (early returns for `/help`, `/verbs`, etc.).

In final return, pass through debug:
```rust
Ok(Json(ChatResponse {
    message: response.message,
    dsl: dsl_state,
    session_state: api_session_state_to_enum(&response.session_state),
    commands: response.commands,
    disambiguation_request: response
        .disambiguation
        .as_ref()
        .map(to_api_disambiguation_request),
    verb_disambiguation: response.verb_disambiguation,
    intent_tier: response.intent_tier,
    unresolved_refs: response
        .unresolved_refs
        .as_ref()
        .map(|refs| api_unresolved_refs_to_api(refs)),
    current_ref_index: response.current_ref_index,
    dsl_hash: response.dsl_hash,
    debug: response.debug,  // NEW
}))
```

---

## Phase 9: Feedback Capture with Real Alternatives

**File:** `rust/src/api/agent_routes.rs`

Update `capture_match()` call site. Alternatives must be `ob_semantic_matcher::MatchResult`:
```rust
use ob_semantic_matcher::{MatchResult, MatchMethod};

// Where capture_match is called:
let winner_verb = /* the selected verb */;
let winner_score = /* from PipelineResult */;

let alternatives: Vec<MatchResult> = candidates
    .iter()
    .filter(|c| c.verb != winner_verb)
    .take(5)
    .map(|c| MatchResult {
        verb_name: c.verb.clone(),
        pattern_phrase: c.matched_phrase.clone(),
        similarity: c.score,
        match_method: MatchMethod::Semantic,  // or map from c.source
        category: "chat".to_string(),
        is_agent_bound: true,
    })
    .collect();

capture_match(&pool, &winner, winner_score, &user_input, user_id, &alternatives).await;
```

---

## Phase 10: Populate Debug Payload (Gated)

**File:** `rust/src/api/agent_service.rs`

Add env-gated debug population using `PipelineResult.verb_candidates`:
```rust
use std::env;

lazy_static::lazy_static! {
    static ref CHAT_DEBUG_ENABLED: bool = env::var("OB_CHAT_DEBUG")
        .map(|v| v == "1")
        .unwrap_or(false);
}

fn build_debug_info(
    selected: Option<&VerbSearchResult>,
    candidates: &[VerbSearchResult],
) -> Option<ob_poc_types::ChatDebugInfo> {
    if !*CHAT_DEBUG_ENABLED {
        return None;
    }
    
    Some(ob_poc_types::ChatDebugInfo {
        verb_match: Some(ob_poc_types::VerbMatchDebug {
            selected: selected.map(verb_result_to_debug),
            candidates: candidates.iter().map(verb_result_to_debug).collect(),
            policy: Some(ob_poc_types::VerbSelectionPolicyDebug {
                algorithm: "max_raw_score".to_string(),
                accept_threshold: Some(0.65),
                ambiguity_margin: Some(0.05),
            }),
        }),
    })
}

fn verb_result_to_debug(r: &VerbSearchResult) -> ob_poc_types::VerbCandidateDebug {
    ob_poc_types::VerbCandidateDebug {
        verb: r.verb.clone(),
        score: r.score,
        primary_source: Some(source_to_api(&r.source)),
        matched_phrase: Some(r.matched_phrase.clone()),
        description: r.description.clone(),
        evidence: r.evidence.iter().map(|e| ob_poc_types::VerbEvidenceDebug {
            source: source_to_api(&e.source),
            score: e.score,
            matched_phrase: e.matched_phrase.clone(),
        }).collect(),
    }
}

fn source_to_api(s: &VerbSearchSource) -> ob_poc_types::VerbMatchSource {
    match s {
        VerbSearchSource::Macro => ob_poc_types::VerbMatchSource::Macro,
        VerbSearchSource::UserLearnedExact => ob_poc_types::VerbMatchSource::UserLearnedExact,
        VerbSearchSource::LearnedExact => ob_poc_types::VerbMatchSource::LearnedExact,
        VerbSearchSource::UserLearnedSemantic => ob_poc_types::VerbMatchSource::UserLearnedSemantic,
        VerbSearchSource::LearnedSemantic => ob_poc_types::VerbMatchSource::LearnedSemantic,
        VerbSearchSource::Semantic => ob_poc_types::VerbMatchSource::Semantic,
        VerbSearchSource::PatternEmbedding => ob_poc_types::VerbMatchSource::PatternEmbedding,
        VerbSearchSource::Phonetic => ob_poc_types::VerbMatchSource::Phonetic,
        _ => ob_poc_types::VerbMatchSource::Unknown,
    }
}
```

In `process_chat()` or similar, where you have `PipelineResult r`:
```rust
let debug = build_debug_info(
    r.verb_candidates.first(),
    &r.verb_candidates,
);

AgentChatResponse {
    // ... existing fields ...
    debug,
}
```

---

## Future Phase: Weighted Score Fusion (Deferred)

**DO NOT IMPLEMENT YET** — requires threshold recalibration.

When ready, replace `fused_score()` with:
```rust
const WEIGHT_MACRO: f32 = 1.00;
const WEIGHT_USER_LEARNED_EXACT: f32 = 0.98;
const WEIGHT_LEARNED_EXACT: f32 = 0.95;
const WEIGHT_USER_LEARNED_SEMANTIC: f32 = 0.90;
const WEIGHT_LEARNED_SEMANTIC: f32 = 0.85;
const WEIGHT_PATTERN_EMBEDDING: f32 = 0.75;
const WEIGHT_PHONETIC: f32 = 0.60;

fn fused_score_weighted(&self) -> f32 {
    self.evidence.iter()
        .map(|e| weight_for_source(&e.source) * e.score)
        .fold(0.0_f32, f32::max)
}
```

Then recalibrate `ACCEPT_THRESHOLD` and `AMBIGUITY_MARGIN` via testing.

---

## Tests to Add
```rust
#[tokio::test]
async fn verb_search_merges_evidence_for_same_verb() {
    // Same verb from Macro + Semantic → evidence.len() == 2
    // fused_score = max(macro_score, semantic_score)
}

#[tokio::test]
async fn verb_search_does_not_early_return_in_ensemble_mode() {
    // Set mode = Ensemble
    // Macro match exists but semantic channels still queried
    // Result has evidence from multiple sources
}

#[tokio::test]
async fn intent_pipeline_passes_user_id() {
    // Verify effective_user_id() returns Uuid::nil() when no session
    // Verify user_id flows through to search()
}

#[tokio::test]
async fn agent_routes_feedback_capture_includes_alternatives() {
    // Verify capture_match() receives non-empty alternatives
    // Verify similarity != 1.0 (uses real score)
}

#[tokio::test]
async fn debug_payload_gated_by_env() {
    // OB_CHAT_DEBUG=0 → debug is None
    // OB_CHAT_DEBUG=1 → debug is Some with candidates
}
```

---

## File Checklist

| File | Changes |
|------|---------|
| `rust/src/api/agent_routes.rs` | Phase 0: warmup + macro registry; Phase 8: pass debug; Phase 9: real alternatives |
| `rust/src/api/agent_service.rs` | Phase 0: add fields; Phase 6: use factory; Phase 10: populate debug |
| `rust/crates/ob-poc-types/src/lib.rs` | Phase 1: debug types + `ChatResponse.debug` |
| `rust/src/mcp/verb_search.rs` | Phase 2: `VerbEvidence`; Phase 4: accumulator pattern |
| `rust/src/mcp/verb_search_factory.rs` | **NEW** Phase 3 |
| `rust/src/mcp/mod.rs` | Phase 3: export factory |
| `rust/src/mcp/intent_pipeline.rs` | Phase 5: `effective_user_id()`, pass user_id |
| `rust/src/mcp/handlers/core.rs` | Phase 7: use factory |

---

## Definition of Done

- [ ] `AgentState` warms up `SharedLearnedData` at startup
- [ ] `AgentState` loads `OperatorMacroRegistry` once at startup
- [ ] Agent chat and MCP use identical `HybridVerbSearcher` via factory
- [ ] `search()` passes `user_id`, enabling user-learned channels
- [ ] Macro match does NOT early-return in Ensemble mode
- [ ] `VerbSearchResult.evidence` populated with all channel votes
- [ ] `fused_score = max(raw)` preserves calibrated thresholds
- [ ] `ChatResponse.debug` populated when `OB_CHAT_DEBUG=1`
- [ ] `capture_match()` receives real `similarity` and `alternatives: Vec<MatchResult>`
- [ ] All `ChatResponse` / `AgentChatResponse` literals include `debug` field
- [ ] Tests pass

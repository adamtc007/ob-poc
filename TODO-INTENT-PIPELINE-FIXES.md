# TODO: Intent Pipeline & Scope Resolution Fixes

> **For:** Claude Code implementation  
> **Created:** 2026-01-26  
> **Reviewed:** Opus peer review incorporated  
> **Status:** Ready for implementation  
> **Priority:** HIGH - blocks production quality agent UX

---

## Problem Summary

The intent pipeline has four categories of issues preventing production-quality UX:

| Category | Problem | User Impact |
|----------|---------|-------------|
| **1. Scope Resolution** | Stage 0 too eager, typed search missing | "Allianz" resolves to wrong thing |
| **2. Candle Hints** | No domain/context hints to verb search | Poor verb ranking for domain-specific queries |
| **3. Context Recovery** | Sentence fragments lose compositional meaning | "Allianz Lux funds" split into 3 unrelated tokens |
| **4. Ambiguous Input** | No graceful degradation for nonsense/vague prompts | Cryptic errors or hallucinated verbs |

---

## 1. SCOPE RESOLUTION FIXES

### 1.1 Stage 0 Scope-Gate Too Eager

**File:** `rust/src/mcp/scope_resolution.rs`

**Problem:** Current code treats any ≤3-word input without verb indicators as "might be a client name". This causes "Allianz CBU", "Allianz custody", "Lux funds" to be consumed by Stage 0.

**Current code (lines 141-146):**
```rust
let words: Vec<&str> = lower.split_whitespace().collect();
if words.len() <= 3 && !Self::has_verb_indicator(&lower) {
    return true; // Might be just a client name - TOO PERMISSIVE
}
```

**Fix — tighten to explicit scope verbs OR high-confidence single token:**

Stage 0 should ONLY attempt scope-set if:
1. Input matches an **explicit scope prefix** ("work on", "switch to", "set client", "context:", "load"), OR
2. Input is **exactly one token** AND would match with **high confidence** (≥0.85)

Remove the "≤3 words" heuristic entirely.

```rust
/// Explicit prefixes that indicate scope-setting intent
const SCOPE_PREFIXES: &[&str] = &[
    "work on ",
    "working on ",
    "switch to ",
    "set client to ",
    "set client ",
    "context: ",
    "context:",
    "load ",
    "client is ",
    "for client ",
];

/// Tokens that indicate the input is a TARGET reference, not scope-setting.
/// If ANY of these appear, Stage 0 should NOT consume the input.
const TARGET_INDICATORS: &[&str] = &[
    // CBU/structure
    "cbu", "cbus",
    // Fund structure
    "fund", "funds", "spv", "spvs", "sicav", "sicavs", "manco", "mancos",
    "subfund", "subfunds", "umbrella",
    // Product/service
    "portfolio", "portfolios", "product", "products", "account", "accounts",
    "custody", "book", "mandate", "mandates",
    // KYC/compliance
    "kyc", "ubo", "ubos", "pep", "peps", "sanctions",
    // Entity structure
    "entity", "entities", "person", "persons", "company", "companies",
    "director", "directors", "shareholder", "shareholders",
    // Ownership
    "holding", "holdings", "ownership", "stake", "stakes",
];

/// Minimum confidence for single-token scope acceptance (high bar)
const MIN_SINGLE_TOKEN_CONFIDENCE: f64 = 0.85;

pub fn is_scope_phrase(input: &str) -> bool {
    let lower = input.to_lowercase();

    // GUARD: If contains target indicators, NOT a scope phrase
    if Self::has_target_indicator(&lower) {
        return false;
    }

    // Rule 1: Explicit scope prefix → yes
    for prefix in SCOPE_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    // Rule 2: Single token only (will be validated by confidence in resolve())
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.len() == 1 {
        return true;  // But resolve() will require high confidence
    }

    // Everything else → NOT a scope phrase, let Candle handle it
    false
}

fn has_target_indicator(input: &str) -> bool {
    let lower = input.to_lowercase();
    TARGET_INDICATORS.iter().any(|t| {
        lower.split_whitespace().any(|word| word == *t || word.ends_with(*t))
    })
}
```

**Update `resolve()` to handle single-token with higher threshold:**

```rust
// In resolve(), after fetching matches:
let top = &matches[0];
let is_single_token = input.trim().split_whitespace().count() == 1;

let has_clear_winner = top.exact_match  // Exact match always wins
    || (!is_single_token && matches.len() == 1 && top.confidence >= 0.7)
    || (is_single_token && top.confidence >= MIN_SINGLE_TOKEN_CONFIDENCE)  // Higher bar for bare names
    || (matches.len() > 1 && (top.confidence - matches[1].confidence) > self.ambiguity_gap);
```

**Acceptance tests:**
```
"work on allianz"       → ScopeResolved (explicit prefix)
"switch to blackrock"   → ScopeResolved (explicit prefix)
"allianz"               → ScopeResolved only if confidence >= 0.85
"Allianz CBU"           → NotScopePhrase (has "cbu" indicator)
"Allianz custody"       → NotScopePhrase (has "custody" indicator)
"Lux CBUs"              → NotScopePhrase
"Allianz funds"         → NotScopePhrase
"show allianz products" → NotScopePhrase (has verb + target)
"allianz ireland"       → NotScopePhrase (2 tokens, no prefix)
```

---

### 1.2 Scoped Disambiguation Ignores Expected Type (THE CORE FIX)

**Files:** 
- `rust/src/mcp/scope_resolution.rs`
- `rust/src/api/agent_service.rs`

**Problem:** `search_entities_by_client_group()` ignores the `entity_type` parameter and always searches entity tags. When a verb expects a CBU slot (`:cbu-id <Allianz>`), it returns entities instead of CBUs.

**Fix — Type-dispatched search with typed candidates:**

**Step 1: Add `ScopedMatch` struct with display metadata for UI:**

```rust
/// Unified scoped match result — works for CBU, entity, client_group, etc.
/// Includes display metadata for disambiguation UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedMatch {
    pub id: Uuid,
    pub name: String,
    pub matched_tag: String,
    pub confidence: f64,
    pub match_type: String,        // "exact", "fuzzy", "trigram", "code"
    
    // Typed candidate metadata for UI
    pub result_type: String,       // "cbu", "entity", "client_group"
    pub display_kind: String,      // "CBU", "Legal Entity", "Person", "Client Group"
    pub source: String,            // "tag", "alias", "direct", "code"
}
```

**Step 2: Add `search_in_scope()` dispatcher:**

```rust
/// Search within scope, dispatching by expected entity type.
/// This is THE key fix: slot type controls what "Allianz" can mean.
#[cfg(feature = "database")]
pub async fn search_in_scope(
    pool: &PgPool,
    scope: &ScopeContext,
    expected_type: &str,  // "cbu" | "entity" | "person" | ...
    query: &str,
    limit: usize,
) -> Result<Vec<ScopedMatch>> {
    let Some(group_id) = scope.client_group_id else {
        return Ok(vec![]);
    };

    match expected_type {
        "cbu" => search_cbus_in_scope(pool, group_id, query, limit, scope.persona.as_deref()).await,
        "entity" | "person" | "company" => {
            let matches = search_entities_in_scope(pool, scope, query, limit).await?;
            Ok(matches.into_iter().map(|m| ScopedMatch {
                id: m.entity_id,
                name: m.entity_name.clone(),
                matched_tag: m.matched_tag,
                confidence: m.confidence,
                match_type: m.match_type,
                result_type: expected_type.to_string(),
                display_kind: match expected_type {
                    "person" => "Person".to_string(),
                    "company" => "Company".to_string(),
                    _ => "Legal Entity".to_string(),
                },
                source: "tag".to_string(),
            }).collect())
        }
        _ => {
            // Fallback to entity search
            let matches = search_entities_in_scope(pool, scope, query, limit).await?;
            Ok(matches.into_iter().map(|m| ScopedMatch {
                id: m.entity_id,
                name: m.entity_name.clone(),
                matched_tag: m.matched_tag,
                confidence: m.confidence,
                match_type: m.match_type,
                result_type: expected_type.to_string(),
                display_kind: expected_type.to_string(),
                source: "tag".to_string(),
            }).collect())
        }
    }
}
```

**Step 3: Add `search_cbus_in_scope()` with trigram + code matching:**

```rust
/// Search CBUs within a client group scope.
/// Prefers: exact match → code match → trigram similarity → ILIKE fallback
#[cfg(feature = "database")]
async fn search_cbus_in_scope(
    pool: &PgPool,
    group_id: Uuid,
    query: &str,
    limit: usize,
    _persona: Option<&str>,
) -> Result<Vec<ScopedMatch>> {
    // CBU → client_group join path:
    // cbus.commercial_client_entity_id → client_group_entity.entity_id
    
    let query_lower = query.to_lowercase();
    
    let rows = sqlx::query!(
        r#"
        WITH cbu_matches AS (
            SELECT
                c.cbu_id as id,
                c.name,
                -- Match type priority: exact > code > trigram > ilike
                CASE 
                    WHEN LOWER(c.name) = $2 THEN 'exact'
                    WHEN c.cbu_id::text ILIKE $2 || '%' THEN 'code'
                    WHEN similarity(c.name, $2) > 0.3 THEN 'trigram'
                    WHEN c.name ILIKE '%' || $2 || '%' THEN 'ilike'
                    ELSE 'none'
                END as match_type,
                -- Confidence scoring
                CASE 
                    WHEN LOWER(c.name) = $2 THEN 1.0
                    WHEN c.cbu_id::text ILIKE $2 || '%' THEN 0.95
                    ELSE COALESCE(similarity(c.name, $2), 0.0)
                END as confidence
            FROM "ob-poc".cbus c
            JOIN "ob-poc".client_group_entity cge 
                ON cge.entity_id = c.commercial_client_entity_id
            WHERE cge.group_id = $1
              AND cge.membership_type != 'historical'
              AND (
                  LOWER(c.name) = $2                           -- exact
                  OR c.cbu_id::text ILIKE $2 || '%'           -- code prefix
                  OR similarity(c.name, $2) > 0.3             -- trigram
                  OR c.name ILIKE '%' || $2 || '%'            -- fallback ILIKE
              )
        )
        SELECT id, name, match_type as "match_type!", confidence as "confidence!: f64"
        FROM cbu_matches
        WHERE match_type != 'none'
        ORDER BY
            CASE match_type 
                WHEN 'exact' THEN 0 
                WHEN 'code' THEN 1 
                WHEN 'trigram' THEN 2 
                ELSE 3 
            END,
            confidence DESC
        LIMIT $3
        "#,
        group_id,
        query_lower,
        limit as i32
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ScopedMatch {
            id: r.id,
            name: r.name.clone(),
            matched_tag: r.name,
            confidence: r.confidence,
            match_type: r.match_type,
            result_type: "cbu".to_string(),
            display_kind: "CBU".to_string(),
            source: if r.match_type == "code" { "code" } else { "direct" }.to_string(),
        })
        .collect())
}
```

**Step 4: Update `agent_service.rs` to use typed search:**

```rust
async fn search_entities_by_client_group(
    &self,
    entity_type: &str,
    query: &str,
    limit: usize,
    client_group_id: Uuid,
) -> Result<Vec<EntityMatchOption>, String> {
    use crate::mcp::scope_resolution::{search_in_scope, ScopeContext};

    let scope = ScopeContext::new().with_client_group(client_group_id, String::new());

    // TYPE-DISPATCHED SEARCH
    let matches = search_in_scope(&self.pool, &scope, entity_type, query, limit)
        .await
        .map_err(|e| format!("Scoped search failed: {}", e))?;

    Ok(matches
        .into_iter()
        .map(|m| EntityMatchOption {
            entity_id: m.id,
            name: m.name,
            entity_type: m.result_type,      // Preserve actual type
            jurisdiction: None,
            context: Some(format!("{} ({})", m.display_kind, m.match_type)),
            score: Some(m.confidence as f32),
        })
        .collect())
}
```

**Schema dependency:** Verify `cbus.commercial_client_entity_id` is populated. If not, need alternative join or linking table.

---

### 1.3 Verb Search Doesn't Receive Scope Context

**Files:**
- `rust/src/mcp/verb_search.rs`
- `rust/src/mcp/intent_pipeline.rs`

**Problem:** Scope context not passed to verb search — can't boost persona-relevant verbs or deprioritize scope-setting verbs when already scoped.

**Fix — Add hints including expected slot types:**

```rust
/// Search hints for verb ranking
pub struct SearchHints {
    pub persona: Option<String>,           // kyc, trading, ops, onboarding
    pub recent_domains: Vec<String>,       // recency bias
    pub scope: Option<ScopeContext>,       // current client scope
    pub domain_filter: Option<String>,     // explicit filter
    pub slot_hints: Vec<String>,           // inferred from prompt tokens
}

impl SearchHints {
    /// Infer slot hints from prompt tokens
    pub fn from_prompt(prompt: &str, scope: Option<&ScopeContext>) -> Self {
        let lower = prompt.to_lowercase();
        let mut slot_hints = Vec::new();
        
        // Infer slot types from prompt tokens
        if lower.contains("cbu") || lower.contains("cbus") {
            slot_hints.push("cbu".to_string());
        }
        if lower.contains("kyc") || lower.contains("ubo") {
            slot_hints.push("kyc".to_string());
        }
        if lower.contains("product") || lower.contains("custody") {
            slot_hints.push("product".to_string());
        }
        if lower.contains("entity") || lower.contains("person") || lower.contains("company") {
            slot_hints.push("entity".to_string());
        }
        
        Self {
            persona: scope.and_then(|s| s.persona.clone()),
            recent_domains: vec![],
            scope: scope.cloned(),
            domain_filter: None,
            slot_hints,
        }
    }
}
```

**Update `search()` signature and apply hints:**

```rust
pub async fn search(
    &self,
    query: &str,
    user_id: Option<Uuid>,
    domain_filter: Option<&str>,
    limit: usize,
    hints: Option<&SearchHints>,  // NEW
) -> Result<Vec<VerbSearchResult>> {
    // ... existing search logic ...
    
    // Apply hint boosts after initial ranking
    if let Some(h) = hints {
        for result in &mut results {
            // Boost verbs matching slot hints
            for hint in &h.slot_hints {
                if result.verb.contains(hint) || self.verb_accepts_type(&result.verb, hint) {
                    result.score += 0.05;
                }
            }
            
            // Boost verbs matching persona
            if let Some(persona) = &h.persona {
                if result.verb.starts_with(persona) {
                    result.score += 0.05;
                }
            }
            
            // Deprioritize scope-setting verbs when already scoped
            if h.scope.as_ref().map(|s| s.has_scope()).unwrap_or(false) {
                if result.verb.starts_with("session.load") || result.verb.starts_with("session.set") {
                    result.score -= 0.05;
                }
            }
        }
        
        // Re-sort after boosts
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    }
    
    // ... rest of existing logic ...
}
```

**Update `intent_pipeline.rs`:**

```rust
let hints = SearchHints::from_prompt(instruction, existing_scope.as_ref());
let candidates = self
    .verb_searcher
    .search(instruction, None, domain_hint, 5, Some(&hints))
    .await?;
```

---

### 1.4 ScopeCandidates Picker Not Implemented

**Files:**
- `rust/src/api/agent_service.rs`
- `rust/src/api/session.rs`

**Problem:** When `PipelineOutcome::ScopeCandidates` is returned, `disambiguation: None` — no picker UI.

**Fix:**

**Step 1: Add to session.rs:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisambiguationItem {
    EntityMatch { /* existing */ },
    ClientGroupMatch {
        candidates: Vec<ClientGroupCandidate>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGroupCandidate {
    pub group_id: Uuid,
    pub group_name: String,
    pub matched_alias: String,
    pub confidence: f64,
    pub entity_count: Option<i64>,
}
```

**Step 2: Build picker in agent_service.rs:**

```rust
if let PipelineOutcome::ScopeCandidates = &result.outcome {
    let candidates = if let Some(ScopeResolutionOutcome::Candidates(c)) = &result.scope_resolution {
        c.iter().map(|sc| ClientGroupCandidate {
            group_id: sc.group_id,
            group_name: sc.group_name.clone(),
            matched_alias: sc.matched_alias.clone(),
            confidence: sc.confidence,
            entity_count: None,
        }).collect()
    } else {
        vec![]
    };

    let disambig = DisambiguationRequest {
        request_id: Uuid::new_v4(),
        items: vec![DisambiguationItem::ClientGroupMatch { candidates }],
        prompt: "Which client did you mean?".to_string(),
        original_intents: None,
    };

    return Ok(AgentChatResponse {
        message: "Multiple clients match. Please select:".to_string(),
        disambiguation: Some(disambig),
        // ...
    });
}
```

---

## 2. CANDLE HINT IMPROVEMENTS

### 2.1 Slot-Type Inference from Prompt

**Covered in 1.3 above** — `SearchHints::from_prompt()` infers slot types from prompt tokens and boosts matching verbs.

### 2.2 Query Expansion for Synonyms (Optional — Low Priority)

**File:** `rust/src/mcp/verb_search.rs`

Small synonym map for common variations. Keep it lightweight:

```rust
const QUERY_SYNONYMS: &[(&str, &[&str])] = &[
    ("show", &["display", "list", "view", "see"]),
    ("create", &["add", "new", "make"]),
    ("owner", &["ownership", "shareholder", "holder"]),
    ("ubo", &["beneficial owner", "ultimate owner"]),
];
```

**Priority:** Low — nice-to-have after core fixes.

---

## 3. COMPOSITE CONTEXT RECOVERY

### 3.1 Sentence-Level Context Preservation

**File:** `rust/src/mcp/intent_pipeline.rs`

**Problem:** "Allianz Luxembourg funds" gets split into 3 unrelated tokens.

**Fix — Simple deterministic chunking (no NLP):**

Use existing lists + simple heuristics:

```rust
pub struct CompositeContext {
    pub original: String,
    pub noun_phrases: Vec<NounPhrase>,
    pub verb_context: Option<VerbSearchResult>,
}

pub struct NounPhrase {
    pub text: String,
    pub span: (usize, usize),
    pub likely_type: Option<String>,
}

/// Known geography tokens for compound detection
const GEOGRAPHY_TOKENS: &[&str] = &[
    "lux", "luxembourg", "ireland", "irish", "dublin", "cayman", 
    "jersey", "guernsey", "uk", "us", "eu", "emea", "apac",
];

impl CompositeContext {
    pub fn from_input(input: &str) -> Self {
        let mut noun_phrases = Vec::new();
        let words: Vec<&str> = input.split_whitespace().collect();
        
        let mut i = 0;
        while i < words.len() {
            // Heuristic 1: Consecutive capitalized words = compound name
            if words[i].chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                let start = i;
                while i < words.len() && 
                      (words[i].chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                       || GEOGRAPHY_TOKENS.contains(&words[i].to_lowercase().as_str())) {
                    i += 1;
                }
                if i > start {
                    let text = words[start..i].join(" ");
                    noun_phrases.push(NounPhrase {
                        text,
                        span: (start, i),
                        likely_type: Self::infer_type(&words[start..i]),
                    });
                    continue;
                }
            }
            
            // Heuristic 2: Word + target indicator = typed reference
            if i + 1 < words.len() {
                let next_lower = words[i + 1].to_lowercase();
                if TARGET_INDICATORS.contains(&next_lower.as_str()) {
                    let text = format!("{} {}", words[i], words[i + 1]);
                    noun_phrases.push(NounPhrase {
                        text,
                        span: (i, i + 2),
                        likely_type: Some(next_lower),
                    });
                    i += 2;
                    continue;
                }
            }
            
            i += 1;
        }
        
        Self {
            original: input.to_string(),
            noun_phrases,
            verb_context: None,
        }
    }
    
    fn infer_type(words: &[&str]) -> Option<String> {
        for word in words {
            let lower = word.to_lowercase();
            if TARGET_INDICATORS.contains(&lower.as_str()) {
                return Some(lower);
            }
        }
        None
    }
}
```

---

### 3.2 Conversation Memory (Optional — Future)

**File:** `rust/src/api/session.rs`

Pronoun resolution ("show its funds" → "its" = last mentioned client). Cap at 5-10 turns.

**Priority:** Future enhancement after core fixes.

---

## 4. AMBIGUOUS INPUT HANDLING

### 4.1 Classify Input Quality

**File:** `rust/src/mcp/intent_pipeline.rs`

```rust
pub enum InputQuality {
    Clear,
    Ambiguous { candidates: Vec<VerbSearchResult> },
    TooVague { best_guess: Option<String> },
    Nonsense,
}

pub fn classify_input(candidates: &[VerbSearchResult], threshold: f32) -> InputQuality {
    match candidates.first() {
        None => InputQuality::Nonsense,
        Some(top) if top.score < 0.30 => InputQuality::Nonsense,
        Some(top) if top.score < threshold => {
            InputQuality::TooVague { best_guess: Some(top.verb.clone()) }
        }
        Some(top) => {
            if let Some(runner_up) = candidates.get(1) {
                if runner_up.score >= threshold && (top.score - runner_up.score) < 0.05 {
                    return InputQuality::Ambiguous { 
                        candidates: candidates[..2].to_vec() 
                    };
                }
            }
            InputQuality::Clear
        }
    }
}
```

---

### 4.2 Graceful Degradation Response Templates

**File:** `rust/src/api/agent_service.rs`

```rust
pub enum AgentResponse {
    Success { message: String, result: Value },
    NeedsClarification { 
        original: String,
        options: Vec<ClarificationOption>,
        hint: String,
    },
    TooVague {
        original: String,
        suggestions: Vec<String>,
        example: String,
    },
    NotUnderstood {
        original: String,
        help_text: String,
    },
}

fn build_response(quality: InputQuality, scope: Option<&ScopeContext>) -> AgentResponse {
    match quality {
        InputQuality::Clear => unreachable!(), // handled elsewhere
        InputQuality::Ambiguous { candidates } => AgentResponse::NeedsClarification {
            original: "".to_string(),
            options: candidates.iter().map(|c| ClarificationOption {
                verb: c.verb.clone(),
                description: c.description.clone(),
            }).collect(),
            hint: format!("Did you mean '{}' or '{}'?", 
                candidates[0].verb, candidates[1].verb),
        },
        InputQuality::TooVague { best_guess } => {
            let example = if scope.map(|s| s.has_scope()).unwrap_or(false) {
                "Try 'show CBUs' or 'list products'"
            } else {
                "Try 'work on [client name]' to set context first"
            };
            AgentResponse::TooVague {
                original: "".to_string(),
                suggestions: vec!["show", "create", "add", "list"].iter().map(|s| s.to_string()).collect(),
                example: example.to_string(),
            }
        }
        InputQuality::Nonsense => AgentResponse::NotUnderstood {
            original: "".to_string(),
            help_text: "I couldn't understand that. Try a command like 'show Allianz CBUs' or 'add custody product'.".to_string(),
        },
    }
}
```

---

### 4.3 Confidence Tiers for UI

```rust
pub enum ConfidenceTier {
    High,      // >= 0.85 - auto-execute
    Medium,    // 0.70-0.85 - proceed with "did you mean?"
    Low,       // 0.55-0.70 - require confirmation
    VeryLow,   // < 0.55 - require clarification
}

impl From<f32> for ConfidenceTier {
    fn from(score: f32) -> Self {
        match score {
            s if s >= 0.85 => ConfidenceTier::High,
            s if s >= 0.70 => ConfidenceTier::Medium,
            s if s >= 0.55 => ConfidenceTier::Low,
            _ => ConfidenceTier::VeryLow,
        }
    }
}
```

---

## Implementation Order (Recommended)

**Fastest functional win first:**

| Phase | Tasks | Time | Impact |
|-------|-------|------|--------|
| **1** | 1.2 (Typed search dispatch) | 2 hrs | THE core "Allianz means different things" fix |
| **2** | 1.1 (Stage 0 scope guard) | 1 hr | Stops scope stealing |
| **3** | 1.4 (ScopeCandidates picker) | 1 hr | UX completeness |
| **4** | 4.1 + 4.2 (Input quality + responses) | 2 hrs | Graceful degradation |
| **5** | 1.3 (Verb search hints) | 1 hr | Better ranking |
| **6** | 3.1 (Composite context) | 1 hr | Better disambiguation |
| **7** | 2.2 + 3.2 (Synonyms, memory) | Optional | Future enhancement |

---

## Files to Modify

| File | Sections |
|------|----------|
| `rust/src/mcp/scope_resolution.rs` | 1.1, 1.2 |
| `rust/src/mcp/intent_pipeline.rs` | 1.3, 3.1, 4.1, 4.3 |
| `rust/src/mcp/verb_search.rs` | 1.3, 2.2 |
| `rust/src/api/agent_service.rs` | 1.2, 1.4, 4.2 |
| `rust/src/api/session.rs` | 1.4, 3.2 |

---

## Test Scenarios

```bash
# 1.1: Stage 0 guard (tightened)
"work on allianz"       → ScopeResolved (explicit prefix)
"switch to blackrock"   → ScopeResolved (explicit prefix)
"allianz"               → ScopeResolved only if confidence >= 0.85
"Allianz CBU"           → NotScopePhrase
"Allianz custody"       → NotScopePhrase
"allianz ireland"       → NotScopePhrase (2 tokens, no prefix)

# 1.2: Typed search  
(cbu.add-product :cbu-id <Allianz>) → searches CBUs, returns CBU matches
(entity.assign-role :entity-id <John>) → searches entities, returns entity matches

# 4.1: Input quality
"asdfgh"                → Nonsense → friendly error
"do it"                 → TooVague → suggestions
"show"                  → Ambiguous → "show what?"

# 3.1: Composite context
"Allianz Luxembourg funds" → preserved as compound
```

---

## Schema Questions (verify before implementing 1.2)

1. Is `cbus.commercial_client_entity_id` reliably populated?
2. If not: add `cbu_client_group` linking table or `client_label` column?
3. Does `cbus` table have a `code` or `short_code` column for code matching?

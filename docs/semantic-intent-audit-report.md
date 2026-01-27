# Semantic Intent Engine Audit Report

**Date:** 2026-01-27  
**Purpose:** Deep review of Candle BGE intent matching capability for LLM review (Claude Opus, ChatGPT)  
**Status:** Agent chat unusable due to sparse learning and weak entity/verb disambiguation

---

## Executive Summary

The semantic intent pipeline has the **infrastructure** but lacks **content density** and **disambiguation logic**. Key findings:

1. **7,717 patterns embedded** across 1,001 verbs - sounds good, but...
2. **31 of 46 YAML files have ZERO `invocation_phrases`** - including critical domains (view, investor, ownership, fund)
3. **33 words collide** between entity types and verb patterns ("fund", "trust", "company", "partnership")
4. **No entity type embedding layer** - system can't distinguish "create a fund" (verb) from "load the Acme Fund" (entity reference)
5. **BGE model works on sentence embeddings** - but we're matching single words/short phrases, losing semantic signal

---

## Part 1: Current Architecture

### 1.1 The BGE Embedding Model

**Model:** BAAI/bge-small-en-v1.5 (384 dimensions)  
**Framework:** HuggingFace Candle (pure Rust, no Python)  
**Mode:** Asymmetric retrieval (query vs target embeddings differ)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  BGE ASYMMETRIC RETRIEVAL                                                   │
│                                                                              │
│  QUERY (user input):                                                        │
│    "Represent this sentence for searching relevant passages: add custody"   │
│    → embed_query() → 384-dim vector                                         │
│                                                                              │
│  TARGET (stored patterns):                                                  │
│    "add custody product"  (no prefix)                                       │
│    → embed_target() → 384-dim vector                                        │
│                                                                              │
│  SIMILARITY: cosine(query_vec, target_vec)                                  │
│                                                                              │
│  KEY INSIGHT: BGE is trained for SENTENCE retrieval, not word matching     │
│  Short patterns like "drill" or "fund" produce weak semantic signal        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Key Implementation Files

| File | Purpose | Lines |
|------|---------|-------|
| `rust/crates/ob-semantic-matcher/src/embedder.rs` | CandleEmbedder - BGE model wrapper | ~300 |
| `rust/src/mcp/verb_search.rs` | HybridVerbSearcher - 6-tier search priority | ~700 |
| `rust/src/mcp/intent_pipeline.rs` | IntentPipeline - verb discovery → arg extraction | ~600 |
| `rust/src/database/verb_service.rs` | VerbService - DB access for patterns | ~400 |
| `rust/crates/ob-semantic-matcher/src/bin/populate_embeddings.rs` | Batch embedding population | ~200 |

### 1.3 Search Priority (6-Tier)

```rust
// From verb_search.rs
// 1. User learned exact match (score 1.0)
// 2. Global learned exact match (score 1.0)  
// 3. User semantic (pgvector similarity)
// 4. Global semantic (pgvector similarity)
// 5. Blocklist filter
// 6. Fallback with lower threshold
```

### 1.4 Current Thresholds

```rust
semantic_threshold: 0.65,  // Decision gate - accept match above this
fallback_threshold: 0.55,  // Retrieval cutoff - fetch candidates above this
AMBIGUITY_MARGIN: 0.05,    // If top-2 within 5%, mark ambiguous
```

---

## Part 2: The Coverage Problem

### 2.1 Pattern Density by Domain

**CRITICAL (1.0 patterns/verb - unusable):**
| Domain | Verbs | Patterns | User-Facing? |
|--------|-------|----------|--------------|
| view | 25 | 25 | ✅ Navigation |
| investor | 21 | 21 | ✅ Core workflow |
| ownership | 18 | 18 | ✅ UBO discovery |
| trading-profile | 66 | 122 | ✅ Trading setup |
| settlement-chain | 15 | 15 | ✅ Settlement |
| trust | 8 | 8 | ✅ Entity creation |
| partnership | 7 | 7 | ✅ Entity creation |

**These domains have only the verb name as a pattern.** No synonyms, no natural language variations.

### 2.2 YAML Files Without invocation_phrases

```
config/verbs/view.yaml          # 0 phrases - NAVIGATION IS BROKEN
config/verbs/investor.yaml      # 0 phrases
config/verbs/ownership.yaml     # 0 phrases
config/verbs/fund.yaml          # 0 phrases  
config/verbs/ubo.yaml           # 0 phrases
config/verbs/graph.yaml         # 0 phrases
config/verbs/screening.yaml     # 0 phrases
config/verbs/lifecycle.yaml     # 0 phrases
... (31 files total with 0 phrases)
```

### 2.3 What Good Coverage Looks Like

From `fund.yaml` verb `create-umbrella` (74 patterns):
```yaml
invocation_phrases:
  - "create umbrella"
  - "new sicav"
  - "create sicav"
  - "create fund umbrella"
  - "umbrella fund"
  - "establish fund"
  - "create vcic"
  - "create oeic"
  - "new umbrella structure"
  - "register fund"
  - "create icav"
  - "create fcp"
  - "fonds commun de placement"
  # ... 60+ more variations
```

This verb matches because users can say it **many different ways**.

---

## Part 3: The Entity/Verb Disambiguation Problem

### 3.1 Collision Words

These words appear in BOTH verb patterns AND entity type names:

| Word | Entity Types | Problem |
|------|--------------|---------|
| fund | fund_standalone, fund_umbrella, fund_subfund, fund_feeder, fund_master, fund_share_class | "create a fund" vs "load the Acme Fund" |
| trust | TRUST_DISCRETIONARY, TRUST_CHARITABLE, TRUST_FIXED_INTEREST, TRUST_UNIT | "create trust" vs "show Trust Corp" |
| company | LIMITED_COMPANY_*, management_company | "create company" vs "load Acme Company" |
| partnership | PARTNERSHIP_GENERAL, PARTNERSHIP_LIMITED, PARTNERSHIP_LLP | verb vs entity |
| owner | PROPER_PERSON_BENEFICIAL_OWNER | "add owner" vs "show the owner" |

### 3.2 Current Entity Resolution Flow

```
User: "load the Allianz Fund"
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│  STEP 1: Verb Discovery (HybridVerbSearcher)                    │
│                                                                  │
│  Query: "load the Allianz Fund"                                 │
│  Embed as query → search verb_pattern_embeddings                │
│                                                                  │
│  Results:                                                        │
│    session.load-cbu      0.72  "load fund"                      │
│    session.load-cluster  0.68  "load allianz"                   │
│    fund.create           0.61  "fund"  ← WRONG but plausible   │
│                                                                  │
│  Problem: "Allianz Fund" is an ENTITY, not a verb argument     │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│  STEP 2: Argument Extraction (LLM)                              │
│                                                                  │
│  LLM sees: verb=session.load-cbu, instruction="load the..."    │
│  Extracts: { "cbu-name": "Allianz Fund" }                       │
│                                                                  │
│  ✅ This part works IF verb discovery was correct               │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│  STEP 3: Entity Resolution (EntityGateway)                      │
│                                                                  │
│  Search entities WHERE name ILIKE '%Allianz Fund%'              │
│  Returns candidates, user picks if ambiguous                    │
│                                                                  │
│  ✅ This part works IF we get here                              │
└─────────────────────────────────────────────────────────────────┘
```

**The failure mode:** Verb discovery sees "fund" and thinks user wants a fund.* verb instead of loading an entity.

### 3.3 What's Missing: Entity Type Embedding Layer

```
PROPOSED: Two-stage intent classification
═══════════════════════════════════════════════════════════════════

User: "load the Allianz Fund"
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│  STAGE 0: Entity Detection (NEW)                                │
│                                                                  │
│  Scan for entity references using:                              │
│    1. Named entity recognition patterns                         │
│    2. Entity type embeddings (fund, trust, company, person)     │
│    3. Known entity name matches (fuzzy)                         │
│                                                                  │
│  Detected: "Allianz Fund" → likely entity reference             │
│  Mask token: "load the <ENTITY:fund>"                           │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│  STAGE 1: Verb Discovery (existing)                             │
│                                                                  │
│  Query: "load the <ENTITY>"  ← entity masked out               │
│  Now correctly matches: session.load-cbu                        │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 4: BGE Sentence Matching Deep Dive

### 4.1 How BGE Works

BGE (BAAI General Embedding) is a **contrastive learning** model trained on:
- Query-passage pairs from search engines
- Question-answer pairs
- Paraphrase pairs

It embeds **sentences** into a 384-dimensional space where:
- Similar meanings cluster together
- Query→Target asymmetry is baked in (instruction prefix)

### 4.2 The Short Pattern Problem

BGE excels at: `"What is the capital of France?"` → `"Paris is the capital city of France."`

BGE struggles with: `"drill"` → `"drill down into entity"`

**Why?** Short patterns lack semantic context. The word "drill" in isolation could mean:
- Power tool
- Military exercise  
- Oil drilling
- UI navigation (our intent)

### 4.3 Current Pattern Examples (Problematic)

```sql
-- From verb_pattern_embeddings
verb_name      | pattern_phrase
---------------+----------------
view.drill     | drill           ← Too short, ambiguous
view.surface   | surface         ← Too short
view.universe  | universe        ← Too short
investor.create| create          ← Collides with 50+ other verbs
```

### 4.4 Recommended Pattern Structure

**Minimum viable pattern:** 3-5 words with context

```yaml
# BAD - too short, ambiguous
invocation_phrases:
  - "drill"
  - "surface"

# GOOD - sentence-like, contextual
invocation_phrases:
  - "drill down into this"
  - "drill into the entity"
  - "go deeper into details"
  - "show me more detail"
  - "expand this node"
  - "zoom into entity"
```

### 4.5 Multi-Word Pattern Matching Strategy

```
┌─────────────────────────────────────────────────────────────────┐
│  PATTERN MATCHING DECISION TREE                                  │
│                                                                  │
│  Input: "show me the allianz lux funds"                         │
│                                                                  │
│  1. EXACT MATCH CHECK                                           │
│     └─ Hash lookup in learned phrases                           │
│     └─ O(1), score=1.0 if found                                 │
│                                                                  │
│  2. N-GRAM EXTRACTION (if no exact)                             │
│     └─ Generate: ["show me the", "me the allianz",              │
│                   "the allianz lux", "allianz lux funds",       │
│                   "show me", "me the", "the allianz", ...]      │
│                                                                  │
│  3. SEMANTIC SEARCH (batch)                                     │
│     └─ Embed each n-gram                                        │
│     └─ Search verb_pattern_embeddings                           │
│     └─ Aggregate scores by verb                                 │
│                                                                  │
│  4. ENTITY DETECTION (parallel)                                 │
│     └─ "allianz" → client group match                          │
│     └─ "lux" → jurisdiction (LU)                                │
│     └─ "funds" → entity type filter                             │
│                                                                  │
│  5. COMBINE SIGNALS                                             │
│     └─ Verb: session.load-cluster (from n-grams)               │
│     └─ Entity: Allianz client group                             │
│     └─ Filter: jurisdiction=LU, type=fund                       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 5: Current Implementation Code

### 5.1 CandleEmbedder (embedder.rs)

```rust
// rust/crates/ob-semantic-matcher/src/embedder.rs

pub struct CandleEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl CandleEmbedder {
    /// Embed user query (with instruction prefix for retrieval)
    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        // BGE instruction prefix for queries
        let prefixed = format!(
            "Represent this sentence for searching relevant passages: {}",
            text
        );
        self.embed_internal(&prefixed).await
    }

    /// Embed target/pattern (no prefix - stored in DB)
    pub async fn embed_target(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_internal(text).await
    }

    async fn embed_internal(&self, text: &str) -> Result<Vec<f32>> {
        // Tokenize
        let encoding = self.tokenizer.encode(text, true)?;
        let input_ids = Tensor::new(encoding.get_ids(), &self.device)?;
        let attention_mask = Tensor::new(encoding.get_attention_mask(), &self.device)?;
        
        // Forward pass through BERT
        let output = self.model.forward(&input_ids, &attention_mask)?;
        
        // CLS token pooling (first token)
        let cls_embedding = output.get(0)?.get(0)?;
        
        // Normalize to unit vector
        let norm = cls_embedding.sqr()?.sum_all()?.sqrt()?;
        let normalized = cls_embedding.broadcast_div(&norm)?;
        
        Ok(normalized.to_vec1()?)
    }
}
```

### 5.2 HybridVerbSearcher (verb_search.rs)

```rust
// rust/src/mcp/verb_search.rs

pub struct HybridVerbSearcher {
    verb_service: Option<VerbService>,
    embedder: Option<Arc<CandleEmbedder>>,
    learned_data: Option<SharedLearnedData>,
    semantic_threshold: f32,   // 0.65
    fallback_threshold: f32,   // 0.55
}

impl HybridVerbSearcher {
    /// Main search entry point
    pub async fn search(&self, query: &str, user_id: Option<Uuid>) -> VerbSearchOutcome {
        // Compute embedding ONCE (Issue G fix)
        let query_embedding = self.embedder.as_ref()
            .unwrap()
            .embed_query(query)
            .await
            .ok();

        // 1. User learned exact
        if let Some(result) = self.search_user_learned_exact(query, user_id).await {
            return VerbSearchOutcome::Matched(result);
        }

        // 2. Global learned exact
        if let Some(result) = self.search_global_learned_exact(query).await {
            return VerbSearchOutcome::Matched(result);
        }

        // 3-6. Semantic search with normalization
        if let Some(ref embedding) = query_embedding {
            let candidates = self.search_global_semantic_with_embedding(embedding, 10).await?;
            
            // Normalize and check for ambiguity
            let normalized = normalize_candidates(candidates);
            
            if let Some(top) = normalized.first() {
                if top.score >= self.semantic_threshold {
                    // Check ambiguity (Issue J fix)
                    if let Some(runner_up) = normalized.get(1) {
                        let margin = top.score - runner_up.score;
                        if margin < AMBIGUITY_MARGIN {
                            return VerbSearchOutcome::Ambiguous {
                                top: top.clone(),
                                runner_up: runner_up.clone(),
                                margin,
                            };
                        }
                    }
                    return VerbSearchOutcome::Matched(top.clone());
                }
            }
        }

        VerbSearchOutcome::NoMatch
    }
}

/// Normalize candidates: dedupe by verb, keep highest score
pub fn normalize_candidates(mut candidates: Vec<VerbSearchResult>) -> Vec<VerbSearchResult> {
    use std::collections::HashMap;
    
    let mut best_by_verb: HashMap<String, VerbSearchResult> = HashMap::new();
    
    for candidate in candidates {
        best_by_verb
            .entry(candidate.verb.clone())
            .and_modify(|existing| {
                if candidate.score > existing.score {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    
    let mut result: Vec<_> = best_by_verb.into_values().collect();
    result.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    result
}
```

### 5.3 IntentPipeline (intent_pipeline.rs)

```rust
// rust/src/mcp/intent_pipeline.rs

pub struct IntentPipeline {
    verb_searcher: HybridVerbSearcher,
    llm_client: Arc<dyn LlmClient>,
}

impl IntentPipeline {
    /// Process natural language → DSL
    pub async fn process(&self, instruction: &str, session: &AgentSession) -> IntentResult {
        // Step 1: Verb discovery
        let verb_outcome = self.verb_searcher.search(instruction, session.user_id).await;
        
        let verb = match verb_outcome {
            VerbSearchOutcome::Matched(result) => result.verb,
            VerbSearchOutcome::Ambiguous { top, runner_up, .. } => {
                return IntentResult::NeedsClarification {
                    candidates: vec![top, runner_up],
                    original_instruction: instruction.to_string(),
                };
            }
            VerbSearchOutcome::NoMatch => {
                return IntentResult::NoMatch {
                    instruction: instruction.to_string(),
                    threshold: self.verb_searcher.semantic_threshold(),
                };
            }
        };

        // Step 2: Get verb definition
        let verb_def = self.get_verb_definition(&verb).await?;
        
        // Step 3: LLM argument extraction
        let args = self.extract_arguments(&verb_def, instruction).await?;
        
        // Step 4: Build DSL
        let dsl = self.build_dsl(&verb, &args)?;
        
        IntentResult::Success { verb, dsl, args }
    }

    /// LLM prompt for argument extraction
    async fn extract_arguments(&self, verb_def: &VerbDefinition, instruction: &str) -> Result<JsonValue> {
        let prompt = format!(r#"
Extract argument values from the instruction for the DSL verb.

VERB: {verb}
DESCRIPTION: {description}

ARGUMENTS:
{args_schema}

INSTRUCTION: "{instruction}"

RULES:
1. Extract values mentioned in the instruction
2. For "entity name" parameters:
   - Extract ONLY the proper noun/entity name
   - Do NOT include descriptive words like "cbu", "universe", "fund"
3. For jurisdiction/country parameters, normalize to ISO 3166-1 alpha-2:
   - UK, Britain → GB
   - USA, America → US
   - Germany → DE
4. If required parameter not found, set to null

Return JSON object with argument names as keys.
"#,
            verb = verb_def.full_name(),
            description = verb_def.description,
            args_schema = self.format_args_schema(verb_def),
            instruction = instruction,
        );
        
        self.llm_client.complete(&prompt).await
    }
}
```

### 5.4 VerbService (verb_service.rs)

```rust
// rust/src/database/verb_service.rs

pub struct VerbService {
    pool: PgPool,
}

impl VerbService {
    /// Semantic search in verb_pattern_embeddings
    pub async fn search_verb_patterns_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<SemanticMatch>> {
        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = sqlx::query_as::<_, VerbPatternSemanticRow>(
            r#"
            SELECT pattern_phrase, verb_name, 
                   1 - (embedding <=> $1::vector) as similarity, 
                   category
            FROM "ob-poc".verb_pattern_embeddings
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $3
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(&embedding_vec)
        .bind(limit as i32)
        .bind(min_similarity)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| SemanticMatch {
            phrase: r.pattern_phrase,
            verb: r.verb_name,
            similarity: r.similarity,
        }).collect())
    }
}
```

### 5.5 populate_embeddings Binary

```rust
// rust/crates/ob-semantic-matcher/src/bin/populate_embeddings.rs

#[tokio::main]
async fn main() -> Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    let embedder = CandleEmbedder::new()?;
    
    // Get patterns needing embeddings
    let patterns: Vec<PatternRow> = sqlx::query_as(
        r#"
        SELECT id, pattern_phrase 
        FROM "ob-poc".verb_pattern_embeddings 
        WHERE embedding IS NULL
        "#
    )
    .fetch_all(&pool)
    .await?;
    
    println!("Embedding {} patterns...", patterns.len());
    
    for pattern in patterns {
        // embed_target (no instruction prefix) for stored patterns
        let embedding = embedder.embed_target(&pattern.pattern_phrase).await?;
        
        sqlx::query(
            r#"
            UPDATE "ob-poc".verb_pattern_embeddings 
            SET embedding = $1::vector 
            WHERE id = $2
            "#
        )
        .bind(Vector::from(embedding))
        .bind(pattern.id)
        .execute(&pool)
        .await?;
    }
    
    Ok(())
}
```

---

## Part 6: Database Schema

### 6.1 verb_pattern_embeddings

```sql
CREATE TABLE "ob-poc".verb_pattern_embeddings (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    verb_name          VARCHAR(100) NOT NULL,        -- e.g., "session.load-cluster"
    pattern_phrase     TEXT NOT NULL,                -- e.g., "load the allianz book"
    pattern_normalized TEXT NOT NULL,                -- lowercase, trimmed
    phonetic_codes     TEXT[] NOT NULL DEFAULT '{}', -- soundex/metaphone
    embedding          VECTOR(384) NOT NULL,         -- BGE embedding
    category           VARCHAR(50) DEFAULT 'navigation',
    is_agent_bound     BOOLEAN DEFAULT false,
    priority           INTEGER DEFAULT 50,
    match_method       TEXT DEFAULT 'semantic',
    created_at         TIMESTAMPTZ DEFAULT NOW(),
    updated_at         TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(verb_name, pattern_normalized)
);

-- IVFFlat index for fast similarity search
CREATE INDEX idx_verb_pattern_embedding_ivfflat 
ON "ob-poc".verb_pattern_embeddings 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 10);
```

### 6.2 entity_types (with embeddings)

```sql
-- Entity types already have embedding column but it's unused
CREATE TABLE "ob-poc".entity_types (
    entity_type_id   UUID PRIMARY KEY,
    name             VARCHAR(255) NOT NULL UNIQUE,
    type_code        VARCHAR(100) UNIQUE,
    entity_category  VARCHAR(20),           -- PERSON, SHELL
    embedding        VECTOR(768),           -- Currently unused!
    embedding_model  VARCHAR(100),
    -- ...
);
```

### 6.3 dsl_verbs (source of truth)

```sql
CREATE TABLE "ob-poc".dsl_verbs (
    verb_id              UUID PRIMARY KEY,
    domain               VARCHAR(100) NOT NULL,
    verb_name            VARCHAR(100) NOT NULL,
    description          TEXT,
    yaml_intent_patterns TEXT[],              -- From YAML invocation_phrases
    intent_patterns      TEXT[],              -- Learned patterns (preserved)
    -- ...
    
    UNIQUE(domain, verb_name)
);
```

---

## Part 7: Recommendations

### 7.1 Immediate: Add invocation_phrases to Critical Domains

**Priority 1 - Navigation (view.yaml):**
```yaml
drill:
  invocation_phrases:
    - "drill down"
    - "drill into"
    - "go deeper"
    - "show more detail"
    - "expand this"
    - "zoom into"
    - "dive into"
    - "explore further"

surface:
  invocation_phrases:
    - "go back"
    - "surface up"
    - "zoom out"
    - "back to parent"
    - "show less detail"
    - "collapse"
    - "return to"
```

**Priority 2 - Investor lifecycle (investor.yaml)**
**Priority 3 - Ownership/UBO (ownership.yaml, ubo.yaml)**
**Priority 4 - Trading profile (trading-profile.yaml)**

### 7.2 Medium-term: Entity Type Disambiguation Layer

1. **Populate entity_types.embedding** with BGE vectors
2. **Pre-scan input** for entity type signals before verb search
3. **Mask detected entities** so verb search sees structure, not noise

### 7.3 Long-term: Multi-Word N-gram Strategy

1. Extract overlapping n-grams from input
2. Batch embed all n-grams
3. Search patterns for each, aggregate by verb
4. Combine with entity detection for final ranking

---

## Part 8: Questions for LLM Review

1. **Is BGE the right model for short-phrase matching?** Should we use a different model optimized for short text (e.g., all-MiniLM-L6-v2 for phrases < 5 words)?

2. **N-gram extraction strategy:** What's the optimal n-gram range for intent matching? Should we weight longer matches higher?

3. **Entity detection:** Should entity detection happen BEFORE or IN PARALLEL with verb search? What's the best architecture?

4. **Threshold tuning:** With better pattern coverage, should thresholds be adjusted? Current: semantic=0.65, fallback=0.55

5. **Confidence aggregation:** When multiple n-grams match the same verb, how should scores be combined? Max? Mean? Weighted by n-gram length?

---

## Appendix A: Current Statistics

```
Total verbs:                1,012
Verbs with embeddings:      1,001  (99%)
Total patterns:             7,717
Average patterns/verb:      7.7

CRITICAL domains (1.0 patterns/verb):
  - view:            25 verbs, 25 patterns
  - investor:        21 verbs, 21 patterns
  - ownership:       18 verbs, 18 patterns
  - settlement-chain: 15 verbs, 15 patterns
  - (28 more domains)

WELL-COVERED domains (10+ patterns/verb):
  - cbu-custody:     21 verbs, 465 patterns (22.1/verb)
  - ubo:             26 verbs, 336 patterns (12.9/verb)
  - request:          9 verbs, 245 patterns (27.2/verb)
  - screening:        3 verbs, 110 patterns (36.7/verb)

Entity/verb collision words: 33
  - fund, trust, company, partnership, owner, person, ...
```

## Appendix B: File Locations

| Category | Path |
|----------|------|
| Embedder | `rust/crates/ob-semantic-matcher/src/embedder.rs` |
| Verb Search | `rust/src/mcp/verb_search.rs` |
| Intent Pipeline | `rust/src/mcp/intent_pipeline.rs` |
| Verb Service | `rust/src/database/verb_service.rs` |
| Populate Script | `rust/crates/ob-semantic-matcher/src/bin/populate_embeddings.rs` |
| Verb YAML | `rust/config/verbs/*.yaml` |
| Entity Types | `ob-poc.entity_types` table |
| Pattern Embeddings | `ob-poc.verb_pattern_embeddings` table |
| Learned Phrases | `agent.invocation_phrases` table |


# TODO: Candle Migration & Pipeline Consolidation

**Priority**: CRITICAL  
**Estimated Effort**: 3-4 days  
**Created**: 2025-01-18  
**Status**: NOT STARTED  
**Goal**: Single unified DSL pipeline with local Candle embeddings

---

## Executive Summary

This migration consolidates all DSL generation paths into a single pipeline:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          UNIFIED DSL PIPELINE                                │
│                                                                              │
│   Agent Prompt ──► verb_search ──► dsl_generate ──► dsl_execute             │
│                       │                │                                     │
│                       ▼                ▼                                     │
│               Candle Embedder    LLM (JSON only)                            │
│               (384-dim, local)   Deterministic Assembly                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Removes**:
- OpenAI embeddings API dependency
- Legacy `IntentExtractor` class
- Legacy `FeedbackLoop.generate_valid_dsl()`
- Any path that bypasses `verb_search`

**Replaces with**:
- Local Candle embeddings (5-15ms vs 100-300ms)
- 384-dim vectors (vs 1536)
- Single entry point: MCP `verb_search` → `dsl_generate`

---

## Phase 0: Preparation & Audit

### 0.1 Document Current State

**Files to audit**:
```
rust/src/agent/learning/embedder.rs     ← OpenAIEmbedder (REMOVE)
rust/src/bin/dsl_mcp.rs                 ← OPENAI_API_KEY check (REMOVE)
rust/src/mcp/verb_search.rs             ← Uses 1536-dim embeddings (CHANGE)
rust/src/mcp/intent_pipeline.rs         ← Keep, update embedder
rust/src/mcp/handlers/core.rs           ← Wire Candle embedder
rust/src/dsl_v2/intent_extractor.rs     ← LEGACY (REMOVE)
rust/src/agentic/orchestrator.rs        ← Uses legacy IntentExtractor (REMOVE)
rust/src/agentic/feedback.rs            ← FeedbackLoop (REMOVE)
rust/crates/ob-semantic-matcher/        ← Candle embedder (USE THIS)
rust/crates/ob-poc-web/src/routes/voice.rs ← Already uses Candle (KEEP)
```

### 0.2 Identify All Entry Points

**Current DSL generation paths** (some must be eliminated):

| Path | Entry Point | Status |
|------|-------------|--------|
| MCP `verb_search` → `dsl_generate` | `handlers/core.rs:1086` | ✅ KEEP (primary) |
| Voice `SemanticMatcher` | `voice.rs:match_voice` | ✅ KEEP (uses Candle) |
| Legacy `IntentExtractor` | `dsl_v2/intent_extractor.rs` | ❌ REMOVE |
| Legacy `FeedbackLoop` | `agentic/feedback.rs` | ❌ REMOVE |
| Legacy `Orchestrator` | `agentic/orchestrator.rs` | ❌ REMOVE |

### 0.3 Backup/Branch Strategy

```bash
git checkout -b candle-migration
git tag pre-candle-migration
```

---

## Phase 1: Add Candle Embedder to Learning Module

### 1.1 Create CandleEmbedder Wrapper

**File**: `rust/src/agent/learning/embedder.rs`

**Action**: Add new `CandleEmbedder` struct that wraps `ob_semantic_matcher::Embedder`

```rust
use ob_semantic_matcher::Embedder as CandleInner;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Local embedder using Candle + all-MiniLM-L6-v2
/// 
/// 384-dimensional embeddings computed locally in 5-15ms.
/// No API key required. Model cached in ~/.cache/huggingface/
pub struct CandleEmbedder {
    inner: Arc<Mutex<CandleInner>>,
}

impl CandleEmbedder {
    pub fn new() -> Result<Self> {
        let inner = CandleInner::new()
            .map_err(|e| anyhow!("Failed to load Candle model: {}", e))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

#[async_trait]
impl Embedder for CandleEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let text = text.to_string();
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = inner.blocking_lock();
            guard.embed(&text)
        })
        .await?
        .map_err(|e| anyhow!("{}", e))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = inner.blocking_lock();
            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            guard.embed_batch(&refs)
        })
        .await?
        .map_err(|e| anyhow!("{}", e))
    }

    fn model_name(&self) -> &str {
        "all-MiniLM-L6-v2"
    }

    fn dimension(&self) -> usize {
        384
    }
}
```

### 1.2 Update Cargo Dependencies

**File**: `rust/Cargo.toml`

```toml
[dependencies]
ob-semantic-matcher = { path = "crates/ob-semantic-matcher" }
```

### 1.3 Update Exports

**File**: `rust/src/agent/learning/mod.rs`

```rust
pub use embedder::{
    CachedEmbedder, CandleEmbedder, Embedder, Embedding, NullEmbedder, SharedEmbedder,
};
// REMOVE: OpenAIEmbedder from exports
```

---

## Phase 2: Schema Migration (1536 → 384)

### 2.1 Create Migration File

**File**: `migrations/034_candle_embeddings.sql`

```sql
-- Migration: 034_candle_embeddings.sql
-- Migrate from OpenAI 1536-dim to Candle 384-dim embeddings
-- This is a DESTRUCTIVE migration - embeddings must be regenerated

-- Step 1: Drop old indexes
DROP INDEX IF EXISTS agent.idx_invocation_phrases_embedding;
DROP INDEX IF EXISTS agent.idx_entity_aliases_embedding;
DROP INDEX IF EXISTS agent.idx_blocklist_embedding;
DROP INDEX IF EXISTS agent.idx_user_phrases_embedding;

-- Step 2: Change column types (requires clearing data)
-- Option A: Recreate columns (loses existing embeddings)
ALTER TABLE agent.invocation_phrases 
    DROP COLUMN IF EXISTS embedding,
    ADD COLUMN embedding vector(384);

ALTER TABLE agent.entity_aliases
    DROP COLUMN IF EXISTS embedding,
    ADD COLUMN embedding vector(384);

ALTER TABLE agent.phrase_blocklist
    DROP COLUMN IF EXISTS embedding,
    ADD COLUMN embedding vector(384);

ALTER TABLE agent.user_learned_phrases
    DROP COLUMN IF EXISTS embedding,
    ADD COLUMN embedding vector(384);

-- Step 3: Update embedding_model default
ALTER TABLE agent.invocation_phrases 
    ALTER COLUMN embedding_model SET DEFAULT 'all-MiniLM-L6-v2';

ALTER TABLE agent.entity_aliases
    ALTER COLUMN embedding_model SET DEFAULT 'all-MiniLM-L6-v2';

ALTER TABLE agent.phrase_blocklist
    ALTER COLUMN embedding_model SET DEFAULT 'all-MiniLM-L6-v2';

ALTER TABLE agent.user_learned_phrases
    ALTER COLUMN embedding_model SET DEFAULT 'all-MiniLM-L6-v2';

-- Step 4: Recreate IVFFlat indexes for 384-dim
CREATE INDEX idx_invocation_phrases_embedding
ON agent.invocation_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

CREATE INDEX idx_entity_aliases_embedding
ON agent.entity_aliases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

CREATE INDEX idx_blocklist_embedding
ON agent.phrase_blocklist
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);

CREATE INDEX idx_user_phrases_embedding
ON agent.user_learned_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Step 5: Update semantic search functions
CREATE OR REPLACE FUNCTION agent.search_similar_phrases(
    query_embedding vector(384),
    similarity_threshold REAL DEFAULT 0.7,
    max_results INT DEFAULT 5
) RETURNS TABLE(
    phrase TEXT,
    verb TEXT,
    similarity REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ip.phrase,
        ip.verb,
        (1 - (ip.embedding <=> query_embedding))::REAL as similarity
    FROM agent.invocation_phrases ip
    WHERE ip.embedding IS NOT NULL
      AND (1 - (ip.embedding <=> query_embedding)) > similarity_threshold
    ORDER BY ip.embedding <=> query_embedding
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION agent.search_user_phrases(
    p_user_id UUID,
    query_embedding vector(384),
    similarity_threshold REAL DEFAULT 0.7,
    max_results INT DEFAULT 5
) RETURNS TABLE(
    phrase TEXT,
    verb TEXT,
    confidence REAL,
    similarity REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        up.phrase,
        up.verb,
        up.confidence,
        (1 - (up.embedding <=> query_embedding))::REAL as similarity
    FROM agent.user_learned_phrases up
    WHERE up.user_id = p_user_id
      AND up.embedding IS NOT NULL
      AND (1 - (up.embedding <=> query_embedding)) > similarity_threshold
    ORDER BY up.embedding <=> query_embedding
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql;

-- Step 6: Update semantic_verb_patterns table if exists
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables 
               WHERE table_schema = 'ob-poc' 
               AND table_name = 'semantic_verb_patterns') THEN
        DROP INDEX IF EXISTS "ob-poc".idx_semantic_verb_patterns_embedding;
        
        ALTER TABLE "ob-poc".semantic_verb_patterns
            DROP COLUMN IF EXISTS embedding,
            ADD COLUMN embedding vector(384);
        
        CREATE INDEX idx_semantic_verb_patterns_embedding
        ON "ob-poc".semantic_verb_patterns
        USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
    END IF;
END $$;

-- Step 7: Update verb_pattern_embeddings (voice pipeline)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables 
               WHERE table_schema = 'ob-poc' 
               AND table_name = 'verb_pattern_embeddings') THEN
        -- This table already uses 384-dim from ob-semantic-matcher
        -- Verify or update if needed
        NULL;
    END IF;
END $$;

COMMENT ON TABLE agent.invocation_phrases IS 
    'Learned phrase→verb mappings. Embeddings: 384-dim all-MiniLM-L6-v2 (Candle)';
```

### 2.2 Create Backfill Binary

**File**: `rust/src/bin/backfill_candle_embeddings.rs`

```rust
//! Backfill Candle embeddings after migration
//!
//! Usage: DATABASE_URL=... cargo run --bin backfill_candle_embeddings

use anyhow::Result;
use ob_poc::agent::learning::embedder::CandleEmbedder;
use sqlx::postgres::PgPoolOptions;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;

    eprintln!("Loading Candle embedder...");
    let start = Instant::now();
    let embedder = CandleEmbedder::new()?;
    eprintln!("Embedder loaded in {:?}", start.elapsed());

    // Backfill invocation_phrases
    backfill_table(&pool, &embedder, "agent.invocation_phrases", "phrase").await?;
    
    // Backfill entity_aliases
    backfill_table(&pool, &embedder, "agent.entity_aliases", "alias").await?;
    
    // Backfill user_learned_phrases
    backfill_table(&pool, &embedder, "agent.user_learned_phrases", "phrase").await?;
    
    // Backfill phrase_blocklist
    backfill_table(&pool, &embedder, "agent.phrase_blocklist", "phrase").await?;

    eprintln!("Backfill complete!");
    Ok(())
}

async fn backfill_table(
    pool: &sqlx::PgPool,
    embedder: &CandleEmbedder,
    table: &str,
    text_column: &str,
) -> Result<()> {
    eprintln!("Backfilling {}...", table);
    
    let query = format!(
        "SELECT id, {} FROM {} WHERE embedding IS NULL",
        text_column, table
    );
    
    let rows: Vec<(i64, String)> = sqlx::query_as(&query)
        .fetch_all(pool)
        .await?;
    
    let total = rows.len();
    if total == 0 {
        eprintln!("  No rows to backfill");
        return Ok(());
    }
    
    let update_query = format!(
        "UPDATE {} SET embedding = $2, embedding_model = 'all-MiniLM-L6-v2' WHERE id = $1",
        table
    );
    
    for (i, (id, text)) in rows.into_iter().enumerate() {
        let embedding = embedder.embed(&text).await?;
        sqlx::query(&update_query)
            .bind(id)
            .bind(&embedding)
            .execute(pool)
            .await?;
        
        if (i + 1) % 100 == 0 || i + 1 == total {
            eprintln!("  {}/{}", i + 1, total);
        }
    }
    
    eprintln!("  Done: {} rows", total);
    Ok(())
}
```

---

## Phase 3: Update MCP Server

### 3.1 Replace OpenAI with Candle

**File**: `rust/src/bin/dsl_mcp.rs`

**Before**:
```rust
use ob_poc::agent::learning::embedder::{CachedEmbedder, OpenAIEmbedder};

// Check for OpenAI API key to enable semantic search
let server = if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
    eprintln!("[dsl_mcp] OPENAI_API_KEY found, enabling semantic search");
    let embedder = Arc::new(CachedEmbedder::new(Arc::new(OpenAIEmbedder::new(api_key))));
    McpServer::with_learned_data_and_embedder(pool, learned_data, embedder)
} else {
    eprintln!("[dsl_mcp] No OPENAI_API_KEY, semantic search disabled");
    McpServer::with_learned_data(pool, learned_data)
};
```

**After**:
```rust
use ob_poc::agent::learning::embedder::{CachedEmbedder, CandleEmbedder};

// Always use local Candle embedder (no API key required)
eprintln!("[dsl_mcp] Loading Candle embedder (all-MiniLM-L6-v2)...");
let start = std::time::Instant::now();
let candle = CandleEmbedder::new()?;
eprintln!("[dsl_mcp] Embedder loaded in {:?}", start.elapsed());

let embedder = Arc::new(CachedEmbedder::new(Arc::new(candle)));
let server = McpServer::with_learned_data_and_embedder(pool, learned_data, embedder);
```

### 3.2 Update Verb Search Queries

**File**: `rust/src/mcp/verb_search.rs`

Update all SQL queries from `vector(1536)` references to work with 384-dim.
The queries themselves don't specify dimension, but ensure the embedder produces 384-dim.

Add dimension validation:
```rust
impl HybridVerbSearcher {
    /// Validate embedding dimension matches expected 384
    fn validate_embedding(embedding: &[f32]) -> Result<()> {
        const EXPECTED_DIM: usize = 384;
        if embedding.len() != EXPECTED_DIM {
            return Err(anyhow!(
                "Invalid embedding dimension: expected {}, got {}. Are you using Candle embedder?",
                EXPECTED_DIM,
                embedding.len()
            ));
        }
        Ok(())
    }
}
```

### 3.3 Add Embedder to Handlers

**File**: `rust/src/mcp/handlers/core.rs`

Ensure `HybridVerbSearcher` gets the embedder:

```rust
async fn get_verb_searcher(&self) -> Result<HybridVerbSearcher> {
    let mut guard = self.verb_searcher.lock().await;
    if guard.is_none() {
        let verbs_dir = std::env::var("VERBS_DIR").unwrap_or_else(|_| "config/verbs".to_string());
        
        let searcher = if let Some(learned) = &self.learned_data {
            let mut searcher = HybridVerbSearcher::full(
                &verbs_dir,
                self.pool.clone(),
                Some(learned.clone()),
            )
            .await?;
            
            // Add embedder for semantic search
            if let Some(embedder) = &self.embedder {
                searcher = searcher.with_embedder(embedder.clone());
            }
            
            searcher
        } else {
            HybridVerbSearcher::phrase_only(&verbs_dir)?
        };
        
        *guard = Some(searcher);
    }
    Ok(guard.as_ref().unwrap().clone())
}
```

---

## Phase 4: Remove Legacy Code

### 4.1 Remove OpenAIEmbedder

**File**: `rust/src/agent/learning/embedder.rs`

**DELETE** the entire `OpenAIEmbedder` struct and impl:

```rust
// DELETE THIS ENTIRE BLOCK (~80 lines)
/// OpenAI embeddings client
pub struct OpenAIEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimension: usize,
}

impl OpenAIEmbedder { ... }

#[async_trait]
impl Embedder for OpenAIEmbedder { ... }
```

### 4.2 Remove Legacy IntentExtractor

**File**: `rust/src/dsl_v2/intent_extractor.rs`

**DELETE** entire file.

**File**: `rust/src/dsl_v2/mod.rs`

**Remove**:
```rust
pub mod intent_extractor;
pub use intent_extractor::IntentExtractor;
```

### 4.3 Remove Legacy FeedbackLoop DSL Generation

**File**: `rust/src/agentic/feedback.rs`

**Remove** `generate_valid_dsl` method or entire file if only used for DSL generation.

### 4.4 Remove Legacy Orchestrator DSL Path

**File**: `rust/src/agentic/orchestrator.rs`

**Remove** or refactor:
- `intent_extractor: IntentExtractor` field
- `feedback_loop: FeedbackLoop` field  
- `process()` method that uses `intent_extractor.extract()`
- Any method that calls `feedback_loop.generate_valid_dsl()`

**Keep** only if orchestrator is used for execution coordination (not DSL generation).

### 4.5 Update agentic/mod.rs

**File**: `rust/src/agentic/mod.rs`

**Remove** exports of deleted modules:
```rust
// REMOVE these if modules are deleted
pub use generator::IntentExtractor;
pub use feedback::FeedbackLoop;
```

### 4.6 Remove OPENAI_API_KEY References

**Search and remove** all `OPENAI_API_KEY` references:
```bash
grep -rn "OPENAI_API_KEY" rust/src --include="*.rs"
```

Expected removals:
- `rust/src/bin/dsl_mcp.rs` - environment check
- `rust/src/agent/learning/embedder.rs` - from_env() method
- Documentation comments

---

## Phase 5: Consolidate Voice Pipeline

### 5.1 Verify Voice Uses Same Embedder

**File**: `rust/crates/ob-poc-web/src/routes/voice.rs`

The voice pipeline already uses `ob_semantic_matcher::SemanticMatcher` which uses Candle internally. 

**Verify** the embedder dimension matches:
- `ob_semantic_matcher::Embedder` → 384-dim ✅
- New `agent::learning::CandleEmbedder` → 384-dim ✅

No changes needed if both use `all-MiniLM-L6-v2`.

### 5.2 Consider Shared Embedder Instance

**Optional optimization**: Share single Candle model instance between MCP and Voice pipelines.

```rust
// Shared embedder singleton
lazy_static! {
    static ref SHARED_CANDLE: Arc<CandleEmbedder> = 
        Arc::new(CandleEmbedder::new().expect("Failed to load Candle"));
}
```

---

## Phase 6: Wire Single Pipeline

### 6.1 Document Canonical Path

After migration, the ONLY valid DSL generation path is:

```
Agent/User Input
       │
       ▼
┌──────────────────┐
│   verb_search    │  ← MCP tool
│  (semantic + phrase)│
└────────┬─────────┘
         │ Top verb + signature
         ▼
┌──────────────────┐
│  dsl_generate    │  ← MCP tool
│  (LLM extracts JSON)│
│  (Deterministic assembly)│
└────────┬─────────┘
         │ Valid DSL
         ▼
┌──────────────────┐
│   dsl_execute    │  ← MCP tool
│  (Runtime execution)│
└──────────────────┘
```

### 6.2 Add Pipeline Enforcement

**File**: `rust/src/mcp/intent_pipeline.rs`

Add validation that DSL was generated through the pipeline:

```rust
/// Marker trait for DSL generated through canonical pipeline
#[derive(Debug, Clone)]
pub struct ValidatedDsl {
    pub source: String,
    pub verb: String,
    pub generated_via: DslGenerationMethod,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub enum DslGenerationMethod {
    /// Generated via verb_search → dsl_generate pipeline
    IntentPipeline,
    /// Voice command via SemanticMatcher
    VoicePipeline,
}
```

### 6.3 Remove Bypass Entry Points

Ensure no code paths allow DSL execution without going through:
1. `verb_search` (verb discovery)
2. `dsl_generate` (arg extraction + assembly)
3. `dsl_validate` (syntax check)
4. `dsl_execute` (execution)

**Audit** these files for direct DSL construction:
- `rust/src/dsl_v2/custom_ops/` - OK if generating DSL programmatically for macros
- `rust/src/research/` - OK if for research queries
- `rust/src/api/` - Should use MCP tools, not direct generation

---

## Phase 7: Testing & Validation

### 7.1 Unit Tests

**File**: `rust/src/agent/learning/embedder.rs` (tests module)

```rust
#[tokio::test]
async fn test_candle_embedder_dimension() {
    let embedder = CandleEmbedder::new().unwrap();
    let embedding = embedder.embed("test phrase").await.unwrap();
    assert_eq!(embedding.len(), 384);
}

#[tokio::test]
async fn test_candle_embedder_similarity() {
    let embedder = CandleEmbedder::new().unwrap();
    let emb1 = embedder.embed("create a fund").await.unwrap();
    let emb2 = embedder.embed("spin up a new fund").await.unwrap();
    let emb3 = embedder.embed("delete everything").await.unwrap();
    
    let sim_12: f32 = emb1.iter().zip(&emb2).map(|(a, b)| a * b).sum();
    let sim_13: f32 = emb1.iter().zip(&emb3).map(|(a, b)| a * b).sum();
    
    assert!(sim_12 > sim_13, "Similar phrases should have higher similarity");
    assert!(sim_12 > 0.7, "Similar phrases should be > 0.7");
}

#[tokio::test]
async fn test_candle_embedder_batch() {
    let embedder = CandleEmbedder::new().unwrap();
    let texts = vec!["one", "two", "three"];
    let embeddings = embedder.embed_batch(&texts).await.unwrap();
    assert_eq!(embeddings.len(), 3);
    assert!(embeddings.iter().all(|e| e.len() == 384));
}
```

### 7.2 Integration Tests

**File**: `rust/tests/integration/candle_migration.rs`

```rust
#[tokio::test]
async fn test_verb_search_with_candle() {
    let pool = test_pool().await;
    let embedder = Arc::new(CandleEmbedder::new().unwrap());
    
    let searcher = HybridVerbSearcher::full("config/verbs", pool, None)
        .await
        .unwrap()
        .with_embedder(embedder);
    
    let results = searcher.search("create a cbu", None, None, 5).await.unwrap();
    assert!(!results.is_empty());
    assert!(results[0].verb.contains("cbu"));
}

#[tokio::test]
async fn test_full_pipeline_candle() {
    // Test: prompt → verb_search → dsl_generate → validate
    let pool = test_pool().await;
    let embedder = Arc::new(CandleEmbedder::new().unwrap());
    
    // ... full integration test
}
```

### 7.3 Performance Tests

```rust
#[tokio::test]
async fn test_candle_latency() {
    let embedder = CandleEmbedder::new().unwrap();
    
    let start = std::time::Instant::now();
    for _ in 0..100 {
        embedder.embed("test semantic search performance").await.unwrap();
    }
    let elapsed = start.elapsed();
    
    let avg_ms = elapsed.as_millis() / 100;
    assert!(avg_ms < 20, "Average embed should be < 20ms, got {}ms", avg_ms);
}
```

---

## Phase 8: Documentation Update

### 8.1 Update CLAUDE.md

Remove all references to `OPENAI_API_KEY` and update:
- Search priority diagram (now includes Candle)
- Tool usage patterns
- Debugging checklist

### 8.2 Update README

Remove `OPENAI_API_KEY` from environment setup.

### 8.3 Create Migration Guide

Document for anyone running existing deployment:
1. Run migration `034_candle_embeddings.sql`
2. Run `backfill_candle_embeddings` binary
3. Restart `dsl_mcp` (no API key needed)

---

## Files Summary

### Files to CREATE

| File | Purpose |
|------|---------|
| `migrations/034_candle_embeddings.sql` | Schema: 1536→384 |
| `rust/src/bin/backfill_candle_embeddings.rs` | Backfill script |
| `rust/tests/integration/candle_migration.rs` | Integration tests |

### Files to MODIFY

| File | Changes |
|------|---------|
| `rust/src/agent/learning/embedder.rs` | Add CandleEmbedder, REMOVE OpenAIEmbedder |
| `rust/src/agent/learning/mod.rs` | Update exports |
| `rust/src/bin/dsl_mcp.rs` | Use Candle, remove OPENAI check |
| `rust/src/mcp/verb_search.rs` | Add dimension validation |
| `rust/src/mcp/handlers/core.rs` | Wire embedder to searcher |
| `rust/Cargo.toml` | Add ob-semantic-matcher dependency |

### Files to DELETE

| File | Reason |
|------|--------|
| `rust/src/dsl_v2/intent_extractor.rs` | Legacy DSL generation |
| Parts of `rust/src/agentic/orchestrator.rs` | Legacy pipeline |
| Parts of `rust/src/agentic/feedback.rs` | Legacy DSL generation |

---

## Execution Checklist

```
[ ] Phase 0: Create branch, audit current state
[ ] Phase 1: Add CandleEmbedder to learning module
[ ] Phase 2: Run schema migration
[ ] Phase 3: Update dsl_mcp to use Candle
[ ] Phase 4: Remove legacy OpenAI code
[ ] Phase 5: Verify voice pipeline compatibility
[ ] Phase 6: Wire single pipeline, remove bypasses
[ ] Phase 7: Run all tests, verify latency
[ ] Phase 8: Update documentation
[ ] Final: Merge to main, tag release
```

---

## Success Criteria

1. **No OpenAI dependency**: `grep -r "OPENAI" rust/src` returns nothing
2. **Single pipeline**: All DSL goes through `verb_search` → `dsl_generate`
3. **384-dim embeddings**: All tables use `vector(384)`
4. **Latency**: `verb_search` with semantic < 50ms (was 200-400ms)
5. **Tests pass**: All unit and integration tests green
6. **Voice works**: Voice pipeline still functional
7. **No side doors**: Audit confirms no bypass paths

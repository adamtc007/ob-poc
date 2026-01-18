# TODO: Migrate from OpenAI to Candle Embeddings

**Priority**: HIGH (performance + cost)  
**Estimated Effort**: 1-2 days  
**Created**: 2025-01-18  
**Status**: NOT STARTED  
**Depends On**: None (can be done independently)

## Overview

Replace OpenAI API embeddings (1536-dim, 100-300ms latency, $cost) with local Candle embeddings (384-dim, 5-15ms latency, free).

### Impact

| Metric | Before (OpenAI) | After (Candle) |
|--------|-----------------|----------------|
| Embed latency | 100-300ms | 5-15ms |
| Cost per embed | $0.00002 | $0 |
| Offline capable | No | Yes |
| Startup dependency | API key | Model file (~22MB) |

---

## Phase 1: Add Candle Embedder to Learning Module

### 1.1 Create Wrapper

**File**: `rust/src/agent/learning/embedder.rs`

Add alongside existing `OpenAIEmbedder`:

```rust
use ob_semantic_matcher::Embedder as CandleInner;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Local embedder using Candle + all-MiniLM-L6-v2
pub struct CandleEmbedder {
    // Mutex because CandleInner is not Send+Sync (holds model state)
    inner: Arc<Mutex<CandleInner>>,
}

impl CandleEmbedder {
    /// Create new Candle embedder
    /// 
    /// First call downloads model (~22MB) from HuggingFace.
    /// Subsequent calls use cached model in ~/.cache/huggingface/
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
        let inner = self.inner.lock().await;
        // Candle is CPU-bound, run in blocking context
        let text = text.to_string();
        let embedding = tokio::task::spawn_blocking({
            let inner = inner.clone();
            move || inner.embed(&text)
        })
        .await??;
        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let inner = self.inner.lock().await;
        let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let embeddings = tokio::task::spawn_blocking({
            let inner = inner.clone();
            move || {
                let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                inner.embed_batch(&refs)
            }
        })
        .await??;
        Ok(embeddings)
    }

    fn model_name(&self) -> &str {
        "all-MiniLM-L6-v2"
    }

    fn dimension(&self) -> usize {
        384
    }
}
```

### 1.2 Add Dependency

**File**: `rust/Cargo.toml`

```toml
[dependencies]
ob-semantic-matcher = { path = "crates/ob-semantic-matcher" }
```

---

## Phase 2: Schema Migration (1536 → 384)

### 2.1 Add New Columns

**File**: `migrations/XXX_candle_embeddings.sql`

```sql
-- Add 384-dim embedding columns (keep 1536 for migration period)

ALTER TABLE agent.invocation_phrases
ADD COLUMN IF NOT EXISTS embedding_384 vector(384);

ALTER TABLE agent.entity_aliases
ADD COLUMN IF NOT EXISTS embedding_384 vector(384);

ALTER TABLE agent.user_learned_phrases
ADD COLUMN IF NOT EXISTS embedding_384 vector(384);

ALTER TABLE agent.phrase_blocklist
ADD COLUMN IF NOT EXISTS embedding_384 vector(384);

-- Semantic verb patterns (if exists)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables 
               WHERE table_schema = 'ob-poc' 
               AND table_name = 'semantic_verb_patterns') THEN
        ALTER TABLE "ob-poc".semantic_verb_patterns
        ADD COLUMN IF NOT EXISTS embedding_384 vector(384);
    END IF;
END $$;

-- Create IVFFlat indexes for 384-dim vectors
CREATE INDEX IF NOT EXISTS idx_invocation_phrases_emb384
ON agent.invocation_phrases
USING ivfflat (embedding_384 vector_cosine_ops) WITH (lists = 100);

CREATE INDEX IF NOT EXISTS idx_user_phrases_emb384
ON agent.user_learned_phrases
USING ivfflat (embedding_384 vector_cosine_ops) WITH (lists = 100);

CREATE INDEX IF NOT EXISTS idx_blocklist_emb384
ON agent.phrase_blocklist
USING ivfflat (embedding_384 vector_cosine_ops) WITH (lists = 50);
```

### 2.2 Backfill Script

**File**: `rust/src/bin/migrate_embeddings_candle.rs`

```rust
//! Migrate embeddings from OpenAI 1536-dim to Candle 384-dim

use anyhow::Result;
use ob_poc::agent::learning::embedder::CandleEmbedder;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;

    eprintln!("Loading Candle embedder...");
    let embedder = CandleEmbedder::new()?;
    eprintln!("Embedder loaded");

    // Migrate invocation_phrases
    eprintln!("Migrating agent.invocation_phrases...");
    let phrases: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, phrase FROM agent.invocation_phrases WHERE embedding_384 IS NULL"
    )
    .fetch_all(&pool)
    .await?;

    let total = phrases.len();
    for (i, (id, phrase)) in phrases.into_iter().enumerate() {
        let embedding = embedder.embed(&phrase).await?;
        sqlx::query(
            "UPDATE agent.invocation_phrases SET embedding_384 = $2 WHERE id = $1"
        )
        .bind(id)
        .bind(&embedding)
        .execute(&pool)
        .await?;

        if (i + 1) % 100 == 0 || i + 1 == total {
            eprintln!("  {}/{}", i + 1, total);
        }
    }
    eprintln!("Done: {} invocation_phrases", total);

    // Migrate user_learned_phrases
    eprintln!("Migrating agent.user_learned_phrases...");
    let user_phrases: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT id, phrase FROM agent.user_learned_phrases WHERE embedding_384 IS NULL"
    )
    .fetch_all(&pool)
    .await?;

    for (id, phrase) in user_phrases {
        let embedding = embedder.embed(&phrase).await?;
        sqlx::query(
            "UPDATE agent.user_learned_phrases SET embedding_384 = $2 WHERE id = $1"
        )
        .bind(id)
        .bind(&embedding)
        .execute(&pool)
        .await?;
    }
    eprintln!("Done: user_learned_phrases");

    // Migrate phrase_blocklist
    eprintln!("Migrating agent.phrase_blocklist...");
    let blocklist: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT id, phrase FROM agent.phrase_blocklist WHERE embedding_384 IS NULL"
    )
    .fetch_all(&pool)
    .await?;

    for (id, phrase) in blocklist {
        let embedding = embedder.embed(&phrase).await?;
        sqlx::query(
            "UPDATE agent.phrase_blocklist SET embedding_384 = $2 WHERE id = $1"
        )
        .bind(id)
        .bind(&embedding)
        .execute(&pool)
        .await?;
    }
    eprintln!("Done: phrase_blocklist");

    eprintln!("Migration complete!");
    Ok(())
}
```

---

## Phase 3: Update Verb Search

### 3.1 Use 384-dim Column

**File**: `rust/src/mcp/verb_search.rs`

Update all pgvector queries to use `embedding_384`:

```rust
// Before
let row = sqlx::query_as::<_, (String, String, f64)>(r#"
    SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
    FROM agent.invocation_phrases
    WHERE embedding IS NOT NULL
    ORDER BY embedding <=> $1::vector
    LIMIT 1
"#)

// After
let row = sqlx::query_as::<_, (String, String, f64)>(r#"
    SELECT phrase, verb, 1 - (embedding_384 <=> $1::vector) as similarity
    FROM agent.invocation_phrases
    WHERE embedding_384 IS NOT NULL
    ORDER BY embedding_384 <=> $1::vector
    LIMIT 1
"#)
```

Apply same change to:
- `search_user_learned_semantic()`
- `search_learned_semantic()`
- `search_global_semantic()`
- `check_blocklist()`

### 3.2 Update Embedding Dimension Check

```rust
impl HybridVerbSearcher {
    fn validate_embedding(&self, embedding: &[f32]) -> Result<()> {
        if embedding.len() != 384 {
            return Err(anyhow!(
                "Invalid embedding dimension: expected 384, got {}",
                embedding.len()
            ));
        }
        Ok(())
    }
}
```

---

## Phase 4: Update MCP Server

### 4.1 Use Candle by Default

**File**: `rust/src/bin/dsl_mcp.rs`

```rust
use ob_poc::agent::learning::embedder::{CachedEmbedder, CandleEmbedder};

#[tokio::main]
async fn main() -> Result<()> {
    // ... pool and warmup ...

    // Load Candle embedder (local, fast, free)
    eprintln!("[dsl_mcp] Loading Candle embedder...");
    let start = std::time::Instant::now();
    let candle = CandleEmbedder::new()?;
    eprintln!("[dsl_mcp] Candle model loaded in {:?}", start.elapsed());
    
    let embedder = Arc::new(CachedEmbedder::new(Arc::new(candle)));

    let server = McpServer::with_learned_data_and_embedder(pool, learned_data, embedder);
    server.run().await
}
```

### 4.2 Remove OpenAI Dependency (Optional)

**File**: `rust/src/bin/dsl_mcp.rs`

Remove the `OPENAI_API_KEY` check:
```rust
// DELETE THIS BLOCK
if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
    // ...
}
```

---

## Phase 5: Cleanup (After Validation)

### 5.1 Drop Old Columns

**File**: `migrations/XXX_drop_1536_embeddings.sql`

```sql
-- Only run after confirming 384-dim is working

-- Drop old 1536-dim columns
ALTER TABLE agent.invocation_phrases DROP COLUMN IF EXISTS embedding;
ALTER TABLE agent.entity_aliases DROP COLUMN IF EXISTS embedding;
ALTER TABLE agent.user_learned_phrases DROP COLUMN IF EXISTS embedding;
ALTER TABLE agent.phrase_blocklist DROP COLUMN IF EXISTS embedding;

-- Rename 384 to embedding
ALTER TABLE agent.invocation_phrases RENAME COLUMN embedding_384 TO embedding;
ALTER TABLE agent.entity_aliases RENAME COLUMN embedding_384 TO embedding;
ALTER TABLE agent.user_learned_phrases RENAME COLUMN embedding_384 TO embedding;
ALTER TABLE agent.phrase_blocklist RENAME COLUMN embedding_384 TO embedding;

-- Rebuild indexes
DROP INDEX IF EXISTS idx_invocation_phrases_emb384;
CREATE INDEX idx_invocation_phrases_embedding
ON agent.invocation_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Same for other tables...
```

### 5.2 Remove OpenAI Embedder Code

**File**: `rust/src/agent/learning/embedder.rs`

Delete `OpenAIEmbedder` struct and implementation.

---

## Testing Checklist

### Functional Tests
- [ ] `CandleEmbedder::new()` loads model successfully
- [ ] `embed()` returns 384-dim vector
- [ ] `embed_batch()` returns correct number of vectors
- [ ] Embeddings are L2 normalized (norm ≈ 1.0)

### Similarity Tests
- [ ] Similar phrases have similarity > 0.7
- [ ] Dissimilar phrases have similarity < 0.4
- [ ] Same phrase has similarity = 1.0

### Integration Tests
- [ ] `verb_search` finds verbs via semantic matching
- [ ] `intent_feedback` stores 384-dim embeddings
- [ ] `learning_import` embeds imported phrases
- [ ] Blocklist semantic matching works

### Performance Tests
- [ ] Single embed < 20ms
- [ ] Batch embed (10 texts) < 50ms
- [ ] Full verb_search (with semantic) < 50ms

---

## Rollback Plan

If issues arise:
1. Set environment variable to use OpenAI: `USE_OPENAI_EMBEDDINGS=1`
2. Update `dsl_mcp.rs` to check this flag
3. Keep both `embedding` (1536) and `embedding_384` columns
4. Queries check which column is populated

```rust
// Fallback logic
let embedder: Arc<dyn Embedder> = if std::env::var("USE_OPENAI_EMBEDDINGS").is_ok() {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    Arc::new(OpenAIEmbedder::new(api_key))
} else {
    Arc::new(CandleEmbedder::new()?)
};
```

---

## Files Changed

| File | Change |
|------|--------|
| `rust/src/agent/learning/embedder.rs` | Add `CandleEmbedder` |
| `rust/src/bin/dsl_mcp.rs` | Use Candle by default |
| `rust/src/mcp/verb_search.rs` | Use `embedding_384` column |
| `rust/Cargo.toml` | Add `ob-semantic-matcher` dependency |
| `migrations/XXX_candle_embeddings.sql` | Add 384-dim columns |
| `rust/src/bin/migrate_embeddings_candle.rs` | NEW - migration script |

---

## Success Criteria

1. **Latency**: verb_search with semantic < 50ms (vs 200-400ms with OpenAI)
2. **Quality**: Same or better verb matching accuracy
3. **Reliability**: No API failures, offline capable
4. **Cost**: $0 embedding cost
5. **Startup**: Model loads in < 5s on first run, < 1s cached

# 037: Candle Semantic Pipeline Remediation Plan

**Created:** 2026-01-19  
**Status:** PLAN  
**Priority:** HIGH - Core agent functionality blocked

---

## Executive Summary

The Candle semantic pipeline has all components built but is not wired end-to-end. Semantic verb discovery is non-functional because:

1. No warmup syncs YAML `invocation_phrases` → pgvector with embeddings
2. REST API doesn't create/use the embedder
3. Missing database tables
4. Dead fallback code (YAML substring matching) adds complexity without value

**Goal:** Single-path semantic verb discovery that works out of the box.

---

## Current State (Broken)

```
User: "set up an ISDA"
    │
    ▼
HybridVerbSearcher.search()
    │
    ├─► Tier 1-5: DB queries fail (tables empty/missing)
    │
    ├─► Tier 6: YAML substring match
    │       └─► "set up an ISDA" does NOT contain "establish isda" → NO MATCH
    │
    └─► Tier 7: Cold start semantic → table empty → NO MATCH
    │
    ▼
Result: No verb found ❌
```

---

## Target State (Fixed)

```
User: "set up an ISDA"
    │
    ▼
CandleEmbedder.embed("set up an ISDA") → [0.12, -0.34, ...] (384-dim)
    │
    ▼
pgvector cosine similarity on agent.invocation_phrases
    │
    ├─► "establish isda" → isda.create (0.91 similarity)
    ├─► "create isda agreement" → isda.create (0.89)
    └─► "set up master agreement" → isda.create (0.85)
    │
    ▼
Result: isda.create ✅
```

---

## Remediation Tasks

### Phase 1: Database Schema (Day 1 Morning)

#### 1.1 Create Missing Tables

**File:** `migrations/037_candle_pipeline_complete.sql`

```sql
-- User-specific learned phrases
CREATE TABLE IF NOT EXISTS agent.user_learned_phrases (
    id BIGSERIAL PRIMARY KEY,
    user_id UUID NOT NULL,
    phrase TEXT NOT NULL,
    verb TEXT NOT NULL,
    confidence NUMERIC(3,2) DEFAULT 1.0,
    embedding vector(384),
    embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2',
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(user_id, phrase)
);

CREATE INDEX idx_user_learned_phrases_user ON agent.user_learned_phrases(user_id);
CREATE INDEX idx_user_learned_phrases_embedding 
    ON agent.user_learned_phrases USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Phrase blocklist (negative examples)
CREATE TABLE IF NOT EXISTS agent.phrase_blocklist (
    id BIGSERIAL PRIMARY KEY,
    phrase TEXT NOT NULL,
    blocked_verb TEXT NOT NULL,
    user_id UUID,  -- NULL = global blocklist
    reason TEXT,
    embedding vector(384),
    embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2',
    expires_at TIMESTAMPTZ,  -- NULL = permanent
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(phrase, blocked_verb, user_id)
);

CREATE INDEX idx_phrase_blocklist_embedding 
    ON agent.phrase_blocklist USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);
```

#### 1.2 Verify invocation_phrases Has Embedding Column

Migration 034 should have added it, but verify:

```sql
-- Check column exists
SELECT column_name, data_type 
FROM information_schema.columns 
WHERE table_schema = 'agent' AND table_name = 'invocation_phrases';

-- If missing:
ALTER TABLE agent.invocation_phrases ADD COLUMN IF NOT EXISTS embedding vector(384);
ALTER TABLE agent.invocation_phrases ADD COLUMN IF NOT EXISTS embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2';
```

---

### Phase 2: YAML → pgvector Sync (Day 1 Afternoon)

#### 2.1 Create Sync Binary

**File:** `rust/src/bin/sync_verb_embeddings.rs`

```rust
//! Sync YAML invocation_phrases to pgvector with Candle embeddings
//!
//! Usage:
//!   cargo run --bin sync_verb_embeddings --features database
//!
//! This should run:
//! - On server startup (warmup)
//! - After verb YAML changes (CI/CD)
//! - Manually when needed

use anyhow::Result;
use ob_poc::agent::learning::embedder::CandleEmbedder;
use ob_poc::dsl_v2::config::loader::ConfigLoader;
use ob_agentic::lexicon::verb_phrases::VerbPhraseIndex;
use sqlx::PgPool;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    let embedder = Arc::new(CandleEmbedder::new()?);
    
    // Load all invocation_phrases from YAML
    let loader = ConfigLoader::from_env();
    let verbs_dir = loader.config_dir().join("verbs");
    let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir.to_str().unwrap())?;
    
    let phrases: Vec<(String, String)> = phrase_index
        .all_phrases()
        .into_iter()
        .collect();
    
    println!("Syncing {} phrases to pgvector...", phrases.len());
    
    // Batch embed (more efficient)
    let texts: Vec<&str> = phrases.iter().map(|(p, _)| p.as_str()).collect();
    let embeddings = embedder.embed_batch(&texts).await?;
    
    // Upsert to DB
    for ((phrase, verb), embedding) in phrases.iter().zip(embeddings.iter()) {
        sqlx::query(r#"
            INSERT INTO agent.invocation_phrases (phrase, verb, embedding, source)
            VALUES ($1, $2, $3::vector, 'yaml_sync')
            ON CONFLICT (phrase, verb) DO UPDATE 
            SET embedding = $3::vector, updated_at = now()
        "#)
        .bind(phrase)
        .bind(verb)
        .bind(embedding)
        .execute(&pool)
        .await?;
    }
    
    println!("Done. {} phrases synced.", phrases.len());
    Ok(())
}
```

#### 2.2 Add `all_phrases()` to VerbPhraseIndex

**File:** `rust/crates/ob-agentic/src/lexicon/verb_phrases.rs`

```rust
impl VerbPhraseIndex {
    /// Get all (phrase, verb) pairs for bulk operations
    pub fn all_phrases(&self) -> Vec<(String, String)> {
        self.phrase_to_verb
            .iter()
            .flat_map(|(phrase, matches)| {
                matches.iter().map(move |m| (phrase.clone(), m.fq_name.clone()))
            })
            .collect()
    }
}
```

#### 2.3 Integrate into Server Startup

**File:** `rust/src/bin/dsl_api.rs` (add warmup)

```rust
// After pool creation, before server start:
if std::env::var("SKIP_EMBEDDING_SYNC").is_err() {
    eprintln!("[dsl_api] Syncing verb embeddings...");
    sync_verb_embeddings(&pool, &embedder).await?;
}
```

---

### Phase 3: Wire Embedder to REST API (Day 2 Morning)

#### 3.1 Create Embedder in dsl_api.rs

**File:** `rust/src/bin/dsl_api.rs`

```rust
// Add import
use ob_poc::agent::learning::embedder::{CandleEmbedder, CachedEmbedder};

// In main(), after pool creation:
let candle = CandleEmbedder::new()?;
let embedder = Arc::new(CachedEmbedder::new(Arc::new(candle)));

// Pass to router/state
let app_state = AppState {
    pool: pool.clone(),
    embedder: Some(embedder.clone()),
    // ...
};
```

#### 3.2 Wire to AgentService

**File:** `rust/src/api/agent_service.rs`

Ensure `AgentService` receives and uses the embedder for verb search.

---

### Phase 4: Remove Dead Fallback Code (Day 2 Afternoon)

#### 4.1 Simplify HybridVerbSearcher

**File:** `rust/src/mcp/verb_search.rs`

Remove Tier 6 (YAML substring matching). The search becomes:

```rust
pub async fn search(&self, query: &str, ...) -> Result<Vec<VerbSearchResult>> {
    // Tier 1: User learned exact
    // Tier 2: Global learned exact  
    // Tier 3: User semantic (pgvector)
    // Tier 4: Global semantic (pgvector) ← PRIMARY PATH
    // Tier 5: Blocklist filter
    // REMOVED: Tier 6 YAML substring
    // Tier 7: Cold start semantic (if tier 4 empty)
}
```

**Rationale:** 
- If "set up an ISDA" doesn't match "establish isda" semantically (0.8+ cosine), it won't match via substring either
- YAML phrases are now IN pgvector with embeddings - that's the lookup path
- Substring matching was a workaround for missing semantic search

#### 4.2 Remove VerbPhraseIndex from Search Path

Keep `VerbPhraseIndex` for:
- Sync to pgvector (source of truth)
- Listing available verbs
- CLI tooling

Remove from:
- `HybridVerbSearcher` runtime search path

#### 4.3 Delete or Deprecate

| File/Code | Action |
|-----------|--------|
| `VerbPhraseIndex.find_matches()` | Keep for tooling, remove from search |
| `LearnedData` HashMap | Keep for exact match (fast path) |
| `phrase_to_verb` in-memory map | Remove from search hot path |

---

### Phase 5: Testing & Validation (Day 2-3)

#### 5.1 Update Test Harness

**File:** `rust/src/bin/agent_chat_harness.rs`

- Remove expectations that rely on substring matching
- Test semantic similarity: "set up an ISDA" → `isda.create`
- Test that natural variations work

#### 5.2 Add Integration Test

```rust
#[tokio::test]
async fn test_semantic_verb_discovery() {
    let pool = test_pool().await;
    let embedder = Arc::new(CandleEmbedder::new().unwrap());
    
    // Sync test phrases
    sync_test_phrases(&pool, &embedder).await;
    
    let searcher = HybridVerbSearcher::full(...)
        .with_embedder(embedder)
        .with_semantic_threshold(0.75);
    
    // These should ALL match isda.create via semantic similarity
    let test_cases = vec![
        "set up an ISDA",
        "establish ISDA agreement", 
        "create master agreement",
        "ISDA with Goldman",
        "need an ISDA",
    ];
    
    for input in test_cases {
        let results = searcher.search(input, None, None, 1).await.unwrap();
        assert!(!results.is_empty(), "No match for: {}", input);
        assert_eq!(results[0].verb, "isda.create", "Wrong verb for: {}", input);
    }
}
```

#### 5.3 Measure Latency

Target: < 20ms for verb discovery (embed + pgvector lookup)

```rust
let start = Instant::now();
let results = searcher.search("set up an ISDA", None, None, 5).await?;
let elapsed = start.elapsed();
assert!(elapsed < Duration::from_millis(20));
```

---

## Migration Checklist

```
[ ] 1.1 Create migration 037 with missing tables
[ ] 1.2 Apply migration to dev database
[ ] 2.1 Create sync_verb_embeddings binary
[ ] 2.2 Add all_phrases() to VerbPhraseIndex
[ ] 2.3 Run sync, verify data in pgvector
[ ] 3.1 Add CandleEmbedder to dsl_api.rs
[ ] 3.2 Wire embedder to AgentService
[ ] 4.1 Remove Tier 6 from HybridVerbSearcher
[ ] 4.2 Clean up unused code paths
[ ] 5.1 Update test harness
[ ] 5.2 Add integration tests
[ ] 5.3 Verify latency targets
[ ] Documentation updated in CLAUDE.md
```

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Sync takes too long on startup | Batch embed, cache model, skip if recent |
| pgvector index not efficient | IVFFlat with proper list count, HNSW if needed |
| Edge case phrases not matched | Lower threshold (0.7), add to YAML |
| Model download on first run | Document, pre-cache in Docker image |

---

## Success Criteria

1. **Functional:** "set up an ISDA" → `isda.create` without substring match
2. **Performance:** < 20ms verb discovery latency
3. **Simplicity:** Single code path for semantic search
4. **Maintainability:** YAML is source of truth, auto-synced to pgvector

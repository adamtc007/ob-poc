# Performance Analysis: Verb Search Pipeline

## Current Architecture

### Search Priority Layers

| Layer | Source | Latency | Requires |
|-------|--------|---------|----------|
| 1. User learned exact | DB | 2-5ms | Pool |
| 2. Global learned exact | In-memory | <1ms | Warmup |
| 3. User learned semantic | DB + API | 100-300ms | Pool + Embedder |
| 4. Global learned semantic | DB + API | 100-300ms | Pool + Embedder |
| 5. Blocklist check | DB + API | 100-300ms | Pool + Embedder |
| 6. YAML phrase match | In-memory | <1ms | VerbPhraseIndex |
| 7. Global semantic | DB + API | 100-300ms | Pool + Embedder |

### What Changed

**Old System (phrase-only)**:
```
User input → VerbPhraseIndex (in-memory HashMap) → result
Latency: ~1-5ms
```

**New System (with semantic)**:
```
User input → Exact checks → Semantic checks (OpenAI API) → Phrase checks → result
Latency: 100-500ms (with API key) or 5-20ms (without)
```

## Performance Concerns

### 1. OpenAI API Latency

Each `embedder.embed(query)` call is an HTTP request to OpenAI:
- Latency: 100-300ms per call
- Cost: ~$0.00002 per embedding (text-embedding-3-small)

**Mitigation**: `CachedEmbedder` caches embeddings by text string.

### 2. Multiple API Calls Per Search

Looking at `verb_search.rs`:
```rust
// Line 196: User learned semantic
embedder.embed(query).await?

// Line 210: Global learned semantic  
embedder.embed(query).await?  // CACHED - same query

// Line 221-248: Blocklist checks
embedder.embed(query).await?  // CACHED - same query

// Line 276: Global semantic
embedder.embed(query).await?  // CACHED - same query
```

Because of caching, same query only embeds ONCE. 

### 3. Multiple DB Round-Trips

Each semantic tier is a separate DB query:
```sql
-- User learned semantic
SELECT ... FROM agent.user_learned_phrases ... ORDER BY embedding <=> $1

-- Global learned semantic
SELECT ... FROM agent.invocation_phrases ... ORDER BY embedding <=> $1

-- Blocklist check (called per candidate!)
SELECT EXISTS ... FROM agent.phrase_blocklist ... embedding <=> $1

-- Global semantic
SELECT ... FROM "ob-poc".semantic_verb_patterns ... ORDER BY embedding <=> $1
```

That's **4+ DB queries** with pgvector similarity search.

### 4. Blocklist Check Is Called Per Candidate

```rust
// Line 245-248
if self.has_semantic_capability()
    && self.check_blocklist(query, user_id, &m.fq_name).await?
{
    // Called for EACH YAML phrase match candidate
}
```

If 5 YAML phrases match, blocklist is checked 5 times (each is a DB query).

## Latency Breakdown

### With Semantic Enabled (OPENAI_API_KEY set)

| Stage | First Request | Cached Request |
|-------|---------------|----------------|
| Exact match checks | 5ms | 5ms |
| OpenAI embedding | 150-300ms | 0ms (cached) |
| Semantic DB queries | 20-40ms | 20-40ms |
| Blocklist checks | 15-30ms | 15-30ms |
| YAML phrase match | 1ms | 1ms |
| **Total** | **200-400ms** | **40-80ms** |

### Without Semantic (no API key)

| Stage | Latency |
|-------|---------|
| Global learned exact | <1ms |
| YAML phrase match | 1-2ms |
| **Total** | **2-5ms** |

## Recommendations

### Option 1: Keep Semantic Optional (Current)

Current code correctly disables semantic without `OPENAI_API_KEY`:
```rust
if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
    // Full semantic search
} else {
    // Phrase-only mode - fast path
}
```

**Pros**: Zero latency impact if not needed
**Cons**: Misses paraphrase matches

### Option 2: Use Local Embeddings

Replace OpenAI with local all-MiniLM-L6-v2 (Candle):
- Latency: 5-15ms (vs 150-300ms)
- Dimension: 384 (vs 1536) - requires schema change
- No API cost

```rust
// Alternative to OpenAI
let embedder = Arc::new(CachedEmbedder::new(
    Arc::new(LocalEmbedder::new()?)  // Candle-based
));
```

**Pros**: ~10x faster than OpenAI
**Cons**: Different embedding space (can't mix with existing 1536-dim vectors)

### Option 3: Batch Semantic Queries

Instead of 4 separate DB queries, combine into one:
```sql
WITH query_embedding AS (
    SELECT $1::vector AS emb
)
SELECT 'user_learned' AS source, phrase, verb, confidence, 
       1 - (embedding <=> (SELECT emb FROM query_embedding)) AS similarity
FROM agent.user_learned_phrases
WHERE user_id = $2 AND embedding IS NOT NULL

UNION ALL

SELECT 'global_learned' AS source, phrase, verb, 1.0, 
       1 - (embedding <=> (SELECT emb FROM query_embedding)) AS similarity
FROM agent.invocation_phrases
WHERE embedding IS NOT NULL

UNION ALL

SELECT 'global_pattern' AS source, pattern_phrase, verb_name, 1.0,
       1 - (embedding <=> (SELECT emb FROM query_embedding)) AS similarity
FROM "ob-poc".semantic_verb_patterns
WHERE embedding IS NOT NULL

ORDER BY similarity DESC
LIMIT 10
```

**Pros**: Single DB round-trip
**Cons**: More complex query, harder to maintain priorities

### Option 4: Request-Scoped Embedding Cache

Ensure embedding is computed exactly ONCE per search:
```rust
pub async fn search(&self, query: &str, ...) -> Result<...> {
    // Compute embedding once at start
    let query_embedding = if self.has_semantic_capability() {
        Some(self.embedder.as_ref().unwrap().embed(query).await?)
    } else {
        None
    };
    
    // Pass pre-computed embedding to all methods
    self.search_user_learned_semantic_with_embedding(user_id, &query_embedding).await?
    self.check_blocklist_with_embedding(&query_embedding, verb).await?
    // etc.
}
```

**Pros**: Eliminates redundant embed calls
**Cons**: Requires refactoring all methods

### Option 5: Lazy Semantic Fallback

Only use semantic search when phrase matching fails:
```rust
pub async fn search(&self, query: &str, ...) -> Result<...> {
    // Fast path first
    if let Some(result) = self.search_exact_and_phrase(query).await? {
        return Ok(vec![result]);
    }
    
    // Slow path only if fast path empty
    if self.has_semantic_capability() {
        return self.search_semantic(query).await;
    }
    
    Ok(Vec::new())
}
```

**Pros**: 95%+ requests stay on fast path
**Cons**: Misses semantic boost for exact matches

## Recommended Implementation

**Short term**: Option 5 (Lazy Semantic Fallback)
- Keeps phrase-only path fast (<5ms)
- Semantic only for truly novel phrases
- Minimal code change

**Medium term**: Option 4 + Option 3 (Request-scoped cache + Batched queries)
- Compute embedding once per request
- Single DB round-trip for all semantic tiers
- Target: <100ms even with semantic

**Long term**: Option 2 (Local Embeddings)
- Eliminate OpenAI dependency
- ~10ms embedding latency
- Requires migration of existing embeddings

## Metrics to Track

Add timing instrumentation:
```rust
let start = Instant::now();
let result = self.search(query, ...).await?;
tracing::info!(
    query = query,
    duration_ms = start.elapsed().as_millis(),
    source = ?result.first().map(|r| &r.source),
    "verb_search completed"
);
```

Target latencies:
- P50: <50ms
- P95: <200ms
- P99: <500ms

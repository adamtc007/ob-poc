# Agent Semantic Pipeline

> **Status:** ✅ Production
> **Last Updated:** 2026-01-18
> **Key Insight:** LLM removed from semantic intent loop via Candle local embeddings

---

## Executive Summary

The agent pipeline uses **local ML inference (Candle)** for verb discovery, eliminating
LLM from the semantic matching loop. This provides 10-20x faster intent matching with
zero API costs and offline capability.

| Before | After | Impact |
|--------|-------|--------|
| LLM understands intent + selects verb | Candle embeds + pgvector matches | **10-20x faster** |
| 200-500ms per query | 5-15ms per query | **Network eliminated** |
| $0.00002 per embedding | $0 | **100% cost reduction** |
| 2 LLM calls per interaction | 1 LLM call (args only) | **50% LLM reduction** |

---

## Architecture: Separation of Concerns

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         UNIFIED DSL PIPELINE                                 │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  VERB DISCOVERY (Pure Rust - NO LLM)                                    ││
│  │                                                                          ││
│  │  User text: "set up an ISDA with Goldman"                               ││
│  │       ↓                                                                  ││
│  │  verb_search (HybridVerbSearcher)                                       ││
│  │       │                                                                  ││
│  │       ├── Tier 1-2: Learned exact (HashMap)           < 1ms             ││
│  │       ├── Tier 3-4: Learned semantic (Candle+pgvector) 5-15ms           ││
│  │       ├── Tier 5:   Blocklist filter                   5-10ms           ││
│  │       ├── Tier 6:   YAML phrases (VerbPhraseIndex)    < 1ms             ││
│  │       └── Tier 7:   Cold semantic (Candle+pgvector)    10-20ms          ││
│  │       ↓                                                                  ││
│  │  Result: isda.create (score: 0.95)                     TOTAL: 6-20ms    ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                      ↓                                       │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  ARG EXTRACTION (LLM Required)                                          ││
│  │                                                                          ││
│  │  dsl_generate with verb signature                                       ││
│  │       ↓                                                                  ││
│  │  LLM chat_with_tool (structured output)               200-500ms         ││
│  │       ↓                                                                  ││
│  │  JSON: { counterparty: "Goldman", governing_law: "NY" }                 ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                      ↓                                       │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  DSL ASSEMBLY (Deterministic Rust)                                      ││
│  │                                                                          ││
│  │  build_dsl_program(verb, args)                        1-5ms             ││
│  │       ↓                                                                  ││
│  │  (isda.create :counterparty "Goldman" :governing-law "NY")              ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                                                              │
│  TOTAL: 210-530ms (LLM is 80-95% of latency)                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7-Tier Verb Search Priority

The `HybridVerbSearcher` combines multiple discovery strategies:

| Tier | Source | Latency | Score | Description |
|------|--------|---------|-------|-------------|
| 1 | User learned exact | <1ms | 1.0 | User-specific phrase→verb mappings |
| 2 | Global learned exact | <1ms | 1.0 | System-wide learned mappings |
| 3 | User learned semantic | 5-15ms | 0.8-0.99 | User phrases via embedding similarity |
| 4 | Global learned semantic | 5-15ms | 0.8-0.99 | Global phrases via embedding similarity |
| 5 | Blocklist filter | 5-10ms | — | Reject blocked verb+phrase combinations |
| 6 | YAML invocation_phrases | <1ms | 0.7-1.0 | Exact/substring match from verb YAML |
| 7 | Cold semantic | 10-20ms | 0.5-0.95 | Fallback: embed query + pgvector search |

### Key Insight: Learning Amplification

> "One learned phrase catches 5-10 paraphrases via embedding similarity."

When a user teaches the system "set up ISDA" → `isda.create`, the embedding similarity
also catches:
- "establish ISDA"
- "create ISDA agreement"
- "ISDA setup"
- "new ISDA"

This is because all these phrases have similar embeddings (cosine similarity > 0.8).

---

## Latency Breakdown

### Happy Path (Learned Phrase Hit)

| Stage | Component | Latency | Cumulative |
|-------|-----------|---------|------------|
| 1 | User input captured | 0ms | 0ms |
| 2 | Learned exact lookup | <1ms | 1ms |
| 3 | Verb found (tier 1-2) | — | 1ms |
| 4 | LLM arg extraction | 200-500ms | 201-501ms |
| 5 | DSL builder | 1-5ms | 202-506ms |
| 6 | Validation | 10-50ms | 212-556ms |
| **Total** | | | **~220-560ms** |

### Cold Path (Semantic Search Required)

| Stage | Component | Latency | Cumulative |
|-------|-----------|---------|------------|
| 1 | User input captured | 0ms | 0ms |
| 2 | Learned exact miss | <1ms | 1ms |
| 3 | Candle embedding | 5-15ms | 6-16ms |
| 4 | pgvector search | <1ms | 7-17ms |
| 5 | Verb found (tier 7) | — | 7-17ms |
| 6 | LLM arg extraction | 200-500ms | 207-517ms |
| 7 | DSL builder | 1-5ms | 208-522ms |
| 8 | Validation | 10-50ms | 218-572ms |
| **Total** | | | **~230-580ms** |

### Bottleneck Analysis

```
┌────────────────────────────────────────────────────────────────┐
│  TIME BUDGET ALLOCATION                                        │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  Verb Discovery (Candle)     ████░░░░░░░░░░░░░░░░  3-5%       │
│  LLM Arg Extraction          ████████████████████  80-95%     │
│  DSL Build + Validation      ██░░░░░░░░░░░░░░░░░░  2-10%      │
│                                                                │
│  LLM IS THE BOTTLENECK. Everything else is negligible.        │
└────────────────────────────────────────────────────────────────┘
```

---

## Candle Embedder Details

### Model Configuration

| Property | Value |
|----------|-------|
| Model | `sentence-transformers/all-MiniLM-L6-v2` |
| Framework | HuggingFace Candle (pure Rust) |
| Dimensions | 384 |
| Inference | CPU (no GPU required) |
| Model size | ~22MB |
| Cache location | `~/.cache/huggingface/` |

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Cold start (first embed) | 1-3s (model load) |
| Warm embed | 5-15ms |
| Memory footprint | ~100MB |
| Thread safety | `Arc<Mutex<Embedder>>` |
| Batch embedding | Supported |

### Code Path

```rust
// rust/src/agent/learning/embedder.rs

pub struct CandleEmbedder {
    inner: Arc<Mutex<ob_semantic_matcher::Embedder>>,
}

impl CandleEmbedder {
    pub fn new() -> Result<Self> {
        let inner = ob_semantic_matcher::Embedder::new()?;
        Ok(Self { inner: Arc::new(Mutex::new(inner)) })
    }
}

#[async_trait]
impl Embedder for CandleEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let inner = self.inner.clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = inner.blocking_lock();
            guard.embed(&text)
        }).await?
    }
    
    fn dimension(&self) -> usize { 384 }
    fn model_name(&self) -> &str { "all-MiniLM-L6-v2" }
}
```

---

## pgvector Integration

### Schema

```sql
-- agent.invocation_phrases (learned phrase→verb mappings)
CREATE TABLE agent.invocation_phrases (
    id UUID PRIMARY KEY,
    phrase TEXT NOT NULL,
    verb TEXT NOT NULL,
    embedding vector(384),           -- Candle embedding
    embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2',
    occurrence_count INTEGER DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- IVFFlat index for fast similarity search
CREATE INDEX idx_invocation_phrases_embedding 
ON agent.invocation_phrases 
USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 100);
```

### Similarity Search

```sql
-- Find similar phrases (cosine distance < 0.2 = similarity > 0.8)
SELECT phrase, verb, 1 - (embedding <=> $1) as similarity
FROM agent.invocation_phrases
WHERE embedding <=> $1 < 0.2
ORDER BY embedding <=> $1
LIMIT 5;
```

---

## Learning Loop

### Correction Capture

When user edits agent-generated DSL before executing:

```
Agent generates: (entity.create :name "Barclays")
User edits to:   (entity.ensure-limited-company :name "Barclays PLC")
User executes → Correction detected → Learning candidate created
```

### Learning Types

| Type | Risk | Auto-Apply | Threshold |
|------|------|------------|-----------|
| Entity alias | Low | ✅ Immediate | N/A |
| Invocation phrase | Medium | After 3x | 3 occurrences |
| Lexicon token | Medium | Manual review | N/A |
| Prompt change | High | ❌ Never | N/A |

### Warmup at Startup

```rust
// rust/src/agent/learning/warmup.rs

pub async fn warmup(&self) -> Result<(LearnedData, WarmupStats)> {
    // 1. Load entity aliases
    let aliases = self.load_entity_aliases().await?;
    
    // 2. Load invocation phrases
    let phrases = self.load_invocation_phrases().await?;
    
    // 3. Apply pending threshold learnings
    let applied = self.apply_threshold_learnings().await?;
    
    // Typical duration: 100-200ms
    Ok((LearnedData { aliases, phrases }, stats))
}
```

---

## Comparison: Before vs After Candle

### Before (OpenAI Embeddings)

```
User: "set up an ISDA"
    ↓
HTTP POST to OpenAI API           100-300ms (network + queue)
    ↓
OpenAI embeds text (1536-dim)     (included in above)
    ↓
pgvector similarity search        <1ms
    ↓
Verb found: isda.create           TOTAL: 100-300ms
    ↓
LLM extracts args                 200-500ms
    ↓
TOTAL: 300-800ms
```

**Problems:**
- Network dependency (no offline)
- API costs ($0.00002/embed)
- 1536 dimensions (storage overhead)
- External dependency risk

### After (Candle Local)

```
User: "set up an ISDA"
    ↓
Candle embed (local CPU)          5-15ms
    ↓
pgvector similarity search        <1ms
    ↓
Verb found: isda.create           TOTAL: 6-16ms
    ↓
LLM extracts args                 200-500ms
    ↓
TOTAL: 210-520ms
```

**Benefits:**
- No network required (offline capable)
- Zero API cost
- 384 dimensions (60% less storage)
- No external dependency
- 10-20x faster verb discovery

---

## Key Files

| File | Purpose |
|------|---------|
| `rust/src/agent/learning/embedder.rs` | CandleEmbedder wrapper |
| `rust/src/mcp/verb_search.rs` | HybridVerbSearcher (7-tier) |
| `rust/src/mcp/handlers/core.rs` | MCP tool handlers |
| `rust/src/agent/learning/warmup.rs` | Startup learning load |
| `rust/crates/ob-semantic-matcher/src/lib.rs` | Candle model wrapper |
| `migrations/034_candle_embeddings.sql` | 1536→384 dimension migration |

---

## MCP Tools

| Tool | Description | Uses Candle? |
|------|-------------|--------------|
| `verb_search` | Find verbs matching natural language | ✅ Yes |
| `dsl_generate` | Convert instruction to DSL | ✅ For verb discovery |
| `intent_feedback` | Record user corrections | ✅ For embedding storage |
| `intent_block` | Block verb for phrase | ✅ For similarity matching |
| `learning_import` | Bulk import phrases | ✅ For batch embedding |

---

## Offline Capability

With Candle, the system can operate partially offline:

| Feature | Online | Offline |
|---------|--------|---------|
| Verb discovery | ✅ Full | ✅ Full (Candle local) |
| Arg extraction | ✅ LLM | ❌ Requires LLM |
| Entity resolution | ✅ EntityGateway | ✅ Local Tantivy |
| DSL execution | ✅ PostgreSQL | ✅ PostgreSQL |

**Offline mode:** Verb discovery + entity resolution work. Only LLM arg extraction
requires network. Future: cache common patterns for fully offline drafting.

---

## Enterprise Considerations

### Security

- **Data stays local**: Embeddings computed on-device, never sent externally
- **No API keys**: Eliminates credential management for embeddings
- **Air-gap capable**: Works in isolated networks

### Cost

- **Zero marginal cost**: No per-embedding charges
- **Fixed compute**: CPU inference included in server resources
- **Storage efficient**: 384-dim vs 1536-dim = 75% reduction

### Reliability

- **No external dependency**: HuggingFace model cached locally
- **Deterministic**: Same input → same embedding
- **Auditable**: Rust code, no black-box API

### Performance at Scale

| Metric | Value |
|--------|-------|
| Embeddings/second | ~100 (single thread) |
| Concurrent users | Limited by LLM, not Candle |
| Memory per user | Negligible (shared model) |

# Candle Embedder: Local ML for ob-poc

## What is Candle?

**Candle** is HuggingFace's pure-Rust ML framework — think PyTorch but compiled to native code with zero Python dependencies.

### Key Properties

| Property | Value |
|----------|-------|
| Language | Pure Rust |
| Runtime | No Python, no GIL |
| Memory | Memory-mapped weights |
| Model | all-MiniLM-L6-v2 |
| Dimensions | 384 |
| Model Size | ~22MB |
| Latency | 5-15ms (CPU) |
| Dependencies | candle-core, tokenizers |

### Why It's Perfect for This Problem

1. **No Network Latency** — Model runs locally, no API calls
2. **No Cost** — Free, unlimited embeddings
3. **Offline Capable** — Works without internet after model download
4. **Rust Native** — Same language as ob-poc, zero FFI overhead
5. **Deterministic** — Same input always produces same output
6. **Compact** — 384 dimensions is enough for phrase matching

---

## You Already Have It

The crate `ob-semantic-matcher` already implements everything:

```
rust/crates/ob-semantic-matcher/
├── src/
│   ├── embedder.rs      ← Candle embedder (all-MiniLM-L6-v2)
│   ├── matcher.rs       ← SemanticMatcher with pgvector
│   ├── phonetic.rs      ← Double Metaphone fallback
│   ├── feedback/        ← Learning loop infrastructure
│   └── types.rs         ← MatchResult, VerbPattern
├── Cargo.toml           ← candle-core 0.8, pgvector 0.4
└── bin/
    └── populate_embeddings.rs
```

### Existing Embedder Code

```rust
// rust/crates/ob-semantic-matcher/src/embedder.rs

pub struct Embedder {
    model: BertModel,      // Candle BERT
    tokenizer: Tokenizer,  // HuggingFace tokenizers
    device: Device,        // CPU (GPU optional)
    normalize: bool,       // L2 normalize for cosine sim
}

impl Embedder {
    pub fn new() -> Result<Self> {
        Self::with_model("sentence-transformers/all-MiniLM-L6-v2")
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Returns 384-dimensional vector
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Batch processing - more efficient
    }
}
```

---

## Performance Comparison

| | OpenAI API | Candle Local |
|---|------------|--------------|
| **Latency** | 100-300ms | 5-15ms |
| **Throughput** | ~3-5/sec (rate limited) | ~100/sec |
| **Cost** | $0.00002/embed | $0 |
| **Offline** | No | Yes |
| **Dimensions** | 1536 | 384 |
| **Quality** | Excellent | Good (sufficient for phrases) |

### Latency Breakdown

```
OpenAI API:
  DNS lookup:     5-10ms
  TLS handshake:  20-50ms
  Network RTT:    30-100ms
  Server compute: 20-50ms
  Response:       10-30ms
  ─────────────────────────
  Total:          100-300ms

Candle Local:
  Tokenize:       0.5-1ms
  Forward pass:   3-10ms
  Mean pooling:   0.5-1ms
  L2 normalize:   0.1ms
  ─────────────────────────
  Total:          5-15ms
```

---

## Model: all-MiniLM-L6-v2

The model used is [sentence-transformers/all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2).

### Why This Model?

| Criteria | all-MiniLM-L6-v2 |
|----------|------------------|
| **Task** | Semantic similarity |
| **Input** | Sentences/phrases (not documents) |
| **Speed** | 5x faster than BERT-base |
| **Size** | 22MB (vs 440MB BERT-base) |
| **Quality** | 95% of BERT-base on STS benchmark |

### Embedding Quality for Phrases

```rust
// Actual test from embedder.rs
let emb1 = embedder.embed("who owns this company").unwrap();
let emb2 = embedder.embed("show me the ownership structure").unwrap();
let emb3 = embedder.embed("zoom in on the graph").unwrap();

// Cosine similarity (embeddings are L2 normalized)
let sim_12: f32 = emb1.iter().zip(&emb2).map(|(a, b)| a * b).sum();
let sim_13: f32 = emb1.iter().zip(&emb3).map(|(a, b)| a * b).sum();

// Result: sim_12 (0.72) > sim_13 (0.31)
// "who owns" correctly clusters with "ownership" not "zoom"
```

---

## Integration Plan

### Step 1: Schema Migration (384 dimensions)

Current schema uses 1536 dimensions (OpenAI). Need to change to 384.

```sql
-- Option A: New column (safe migration)
ALTER TABLE agent.invocation_phrases
ADD COLUMN embedding_384 vector(384);

-- Option B: Replace column (requires re-embed all)
ALTER TABLE agent.invocation_phrases
ALTER COLUMN embedding TYPE vector(384);

-- Same for:
-- agent.user_learned_phrases
-- agent.phrase_blocklist
-- "ob-poc".semantic_verb_patterns
```

### Step 2: Wire Candle Embedder into Learning

```rust
// rust/src/agent/learning/embedder.rs

// Add Candle embedder option
pub struct CandleEmbedder {
    inner: ob_semantic_matcher::Embedder,
}

impl CandleEmbedder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: ob_semantic_matcher::Embedder::new()?,
        })
    }
}

#[async_trait]
impl Embedder for CandleEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        // Candle is sync, wrap in spawn_blocking for async
        let text = text.to_string();
        let inner = self.inner.clone(); // Embedder needs to be Clone
        tokio::task::spawn_blocking(move || inner.embed(&text))
            .await?
    }

    fn model_name(&self) -> &str {
        "all-MiniLM-L6-v2"
    }

    fn dimension(&self) -> usize {
        384
    }
}
```

### Step 3: Update MCP Server Startup

```rust
// rust/src/bin/dsl_mcp.rs

use ob_poc::agent::learning::embedder::CandleEmbedder;

#[tokio::main]
async fn main() -> Result<()> {
    // ... pool setup ...

    // Use Candle instead of OpenAI
    eprintln!("[dsl_mcp] Loading Candle embedder (all-MiniLM-L6-v2)...");
    let embedder = Arc::new(CachedEmbedder::new(
        Arc::new(CandleEmbedder::new()?)
    ));
    eprintln!("[dsl_mcp] Embedder loaded");

    let server = McpServer::with_learned_data_and_embedder(
        pool, 
        learned_data, 
        embedder
    );

    server.run().await
}
```

### Step 4: Re-embed Existing Data

```rust
// One-time migration script
async fn migrate_embeddings(pool: &PgPool, embedder: &CandleEmbedder) -> Result<()> {
    let phrases: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, phrase FROM agent.invocation_phrases"
    )
    .fetch_all(pool)
    .await?;

    for (id, phrase) in phrases {
        let embedding = embedder.embed(&phrase).await?;
        sqlx::query(
            "UPDATE agent.invocation_phrases SET embedding_384 = $2 WHERE id = $1"
        )
        .bind(id)
        .bind(&embedding)
        .execute(pool)
        .await?;
    }

    Ok(())
}
```

---

## Architecture After Migration

```
┌─────────────────────────────────────────────────────────────┐
│                     User Input                               │
│                  "spin up a fund"                            │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              HybridVerbSearcher                              │
├─────────────────────────────────────────────────────────────┤
│  1. Exact match (in-memory)              │ <1ms             │
│  2. YAML phrase match (in-memory)        │ <1ms             │
│  ─────────────────────────────────────────────────────────  │
│  3. Candle embed (local)                 │ 5-15ms           │
│  4. pgvector similarity (384-dim)        │ 5-10ms           │
├─────────────────────────────────────────────────────────────┤
│  Total (with semantic): 15-30ms                             │
│  Total (phrase hit):    <5ms                                │
└─────────────────────────────────────────────────────────────┘
```

---

## Considerations

### Dimension Change: 1536 → 384

- **Cannot mix** — 1536 and 384 vectors incompatible
- **Must re-embed all** — One-time migration
- **Index rebuild** — IVFFlat index needs recreation

### Model Download

First startup downloads ~22MB from HuggingFace:
```
~/.cache/huggingface/hub/
└── models--sentence-transformers--all-MiniLM-L6-v2/
    ├── config.json
    ├── tokenizer.json
    └── model.safetensors
```

Subsequent startups use cached files.

### CPU vs GPU

Current implementation uses CPU:
```rust
let device = Device::Cpu;
```

GPU acceleration possible with `candle-core` features:
- CUDA: `features = ["cuda"]`
- Metal (Mac): `features = ["metal"]`

For phrase embedding, CPU is sufficient (5-15ms is fast enough).

### Async Consideration

Candle operations are synchronous. For async context:
```rust
// Wrap in spawn_blocking to avoid blocking the runtime
tokio::task::spawn_blocking(move || embedder.embed(&text)).await?
```

---

## Quick Start

### Test Existing Embedder

```bash
cd rust/crates/ob-semantic-matcher
cargo test test_embed_single -- --ignored
```

### Benchmark Latency

```rust
let embedder = Embedder::new()?;
let start = Instant::now();
for _ in 0..100 {
    embedder.embed("spin up a fund for acme corp")?;
}
println!("100 embeds: {:?}", start.elapsed()); // ~500-1500ms total
```

---

## Recommendation

**Use Candle.** The existing `ob-semantic-matcher` crate already has everything you need.

Migration path:
1. Add 384-dim column to tables
2. Wire `CandleEmbedder` into `dsl_mcp`
3. Run backfill script to embed existing phrases
4. Update `verb_search.rs` to use 384-dim column
5. Drop OpenAI dependency

Expected result:
- Semantic search latency: 100-300ms → 15-30ms
- No API costs
- Offline capable
- Simpler deployment (no `OPENAI_API_KEY` required)

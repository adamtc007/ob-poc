# Sage Embeddings Integration Note

**Status**: Tranche 0 sub-phase 0.2 — audit complete
**Date**: 2026-05-21
**Purpose**: Integration reference for Tranche 1 pack matcher

---

## Where the infrastructure lives

`rust/crates/ob-semantic-matcher/` — a standalone library crate inside the
ob-poc workspace.  It has **no dependency on any `dsl-*` crate**, so it is
safe to add as a dependency of a future `dsl-sage-embeddings` adapter crate
or directly from any Sage-related crate without introducing cycles.

Key source files:
- `src/embedder.rs` — the `Embedder` struct (BGE-small-en-v1.5, Candle)
- `src/matcher.rs` — `SemanticMatcher` (embedder + pgvector similarity + phonetic fallback)
- `src/types.rs` — `VerbPattern`, `MatchResult`, `MatcherConfig`

---

## Current interface

### Model

| Property | Value |
|----------|-------|
| Model | `BAAI/bge-small-en-v1.5` |
| Embedding dimension | 384 |
| Pooling | CLS token (not mean pooling) |
| Normalisation | L2 (unit vectors; dot product == cosine similarity) |
| Revision pinned | `5c38ec7c...` (SHA) for reproducibility |
| Cache | HuggingFace hub cache (~/.cache/huggingface), ~130 MB first download |
| Device | CPU (GPU/Metal optional feature flag) |

### Core `Embedder` API

```rust
// Embed a user utterance (adds retrieval instruction prefix)
pub fn embed_query(&self, text: &str) -> Result<Vec<f32>>;

// Embed a stored pattern/target (no prefix)
pub fn embed_target(&self, text: &str) -> Result<Vec<f32>>;

// Batch variants for efficiency
pub fn embed_batch_queries(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
pub fn embed_batch_targets(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

pub fn embedding_dim(&self) -> usize;  // always 384
pub fn model_name(&self) -> &str;      // "BAAI/bge-small-en-v1.5"
```

### Query vs target distinction

BGE is an **asymmetric retrieval model**.  User utterances are queries
(pass through `embed_query` — instruction prefix applied); stored pack
utterance-bindings are targets (pass through `embed_target` — no prefix).
Mixing the two degrades quality.  The `SemanticMatcher` in the production ob-poc
pipeline already enforces this distinction.

### `SemanticMatcher` (production use)

`SemanticMatcher` wraps `Embedder` with pgvector similarity search against a
Postgres database.  It requires `PgPool` — appropriate for the main ob-poc
server.  For the Sage pack matcher (Tranche 1), we do **not** need pgvector
because the pack catalogue is small (12–50 packs) and can be held fully in
memory.

---

## Integration point for Tranche 1

The Tranche 1 pack matcher needs:
1. `Embedder::embed_query()` for the user utterance.
2. `Embedder::embed_batch_targets()` for all pack utterance-bindings.
3. In-memory cosine similarity (dot product of unit vectors — already L2 normalised).

No pgvector required for this scale.  Implement the pack-matching layer inside
a new `dsl-sage` crate (or a sub-module within `ob-poc`) and take `Embedder`
as a dependency.

Minimal integration sketch:

```rust
use ob_semantic_matcher::Embedder;

pub struct PackMatcher {
    embedder: Embedder,
    // (pack_name, binding_idx) -> Vec<f32>
    index: Vec<(String, Vec<f32>)>,
}

impl PackMatcher {
    pub fn new() -> Result<Self> {
        let embedder = Embedder::new()?;
        Ok(Self { embedder, index: Vec::new() })
    }

    pub fn add_pack_bindings(&mut self, pack_name: &str, bindings: &[&str]) -> Result<()> {
        let vecs = self.embedder.embed_batch_targets(bindings)?;
        for vec in vecs {
            self.index.push((pack_name.to_string(), vec));
        }
        Ok(())
    }

    pub fn match_utterance(&self, utterance: &str) -> Result<Vec<(String, f32)>> {
        let q = self.embedder.embed_query(utterance)?;
        // Dot product (unit vectors → cosine similarity)
        let mut scores: std::collections::HashMap<String, f32> = Default::default();
        for (name, binding_vec) in &self.index {
            let sim: f32 = q.iter().zip(binding_vec).map(|(a, b)| a * b).sum();
            let best = scores.entry(name.clone()).or_insert(0.0_f32);
            *best = best.max(sim);
        }
        let mut ranked: Vec<(String, f32)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(ranked)
    }
}
```

---

## Circular dependency check

`ob-semantic-matcher` depends on: `candle-*`, `tokenizers`, `sqlx`, `pgvector`,
`tokio`, `anyhow`, `tracing`.  It has **no `dsl-*` dependency** and no
`ob-poc` (application crate) dependency.

Adding `ob-semantic-matcher` as a dependency of a Sage crate (`dsl-sage` or
similar) introduces no cycle.  If we add it directly to `ob-poc-web`, that is
also fine — it already transitively pulls in `sqlx`.

---

## Gap: thin adapter crate

The Tranche 0 design proposed a potential `dsl-sage-embeddings` adapter crate.
After audit: **this adapter is not necessary** for Tranche 1.  The `Embedder`
struct is the integration point and it has a clean, well-typed public API.  A
thin wrapper would add a crate boundary without adding value at this scale.

Recommended approach for Tranche 1: add `ob-semantic-matcher` directly as a
dependency of whichever crate implements `PackMatcher`, and call
`Embedder::new()` at startup (the 130 MB model download happens only once;
subsequent calls use the HF cache).

---

## Latency budget note

Tranche 1 target: < 2 seconds end-to-end for a single utterance.  The
`Embedder` forward pass (CPU, BGE-small) takes ~15–50 ms per query in practice.
The bottleneck in Tranche 1 will be the LLM ranking call, not the embedding
retrieval.

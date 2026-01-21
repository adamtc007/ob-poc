# Embedding Population Performance

**Status:** ACCEPTABLE - Delta loading makes this a rare operation

## Current Performance (--release, Apple Silicon M-series)

| Metric | Value |
|--------|-------|
| Embedding rate | ~28 patterns/sec |
| Full load (7500 patterns) | ~4.5 minutes |
| Incremental (50 new) | ~2 seconds |
| DB insert (bulk UNNEST) | ~0.2 seconds for 7500 |

## What's Been Done

1. **Bulk INSERT with UNNEST** - DB inserts are now 0.2s (was 60s)
2. **Delta loading** - Only embeds NEW patterns, skips existing
3. **Native CPU flags** - `RUSTFLAGS="-C target-cpu=native"`

## Bottleneck: Candle CPU Inference

The BERT model inference in Candle is inherently slow on CPU (~35ms per pattern). This is expected for transformer models without GPU acceleration.

## Future Options (if needed)

| Option | Expected Improvement | Effort |
|--------|---------------------|--------|
| Metal GPU | 3-5x faster | Medium - add `metal` feature |
| Smaller model | 2-3x faster | Low - but may reduce quality |
| External API (OpenAI) | 10x faster | Low - but adds cost/network |

## Why It's Acceptable

- **Rare operation**: Only run after YAML verb changes
- **Delta loading**: Incremental updates are fast (~2s)
- **Background capable**: Can run while doing other work
- **One-time cost**: Embeddings are cached in DB

## Commands

```bash
# Standard run (uses delta loading)
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings

# Force re-embed all
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings -- --force
```

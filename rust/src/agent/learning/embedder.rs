//! Embedding service for semantic learning
//!
//! Provides text embeddings for semantic phrase matching via pgvector.
//! Uses local Candle embeddings (all-MiniLM-L6-v2) - no API key required.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Embedding vector type (384 dimensions for all-MiniLM-L6-v2)
pub type Embedding = Vec<f32>;

/// Standard embedding dimension (all-MiniLM-L6-v2)
pub const EMBEDDING_DIMENSION: usize = 384;

/// Trait for text embedding services
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Generate embedding for text
    async fn embed(&self, text: &str) -> Result<Embedding>;

    /// Batch embed multiple texts (more efficient)
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>>;

    /// Model identifier for storage
    fn model_name(&self) -> &str;

    /// Embedding dimension
    fn dimension(&self) -> usize;
}

/// Shared embedder type for use across handlers
pub type SharedEmbedder = Arc<dyn Embedder>;

/// Local embedder using Candle + all-MiniLM-L6-v2
///
/// 384-dimensional embeddings computed locally in 5-15ms.
/// No API key required. Model cached in ~/.cache/huggingface/
pub struct CandleEmbedder {
    inner: Arc<Mutex<ob_semantic_matcher::Embedder>>,
}

impl CandleEmbedder {
    /// Create a new Candle embedder
    ///
    /// Downloads the model (~22MB) on first use.
    /// Subsequent calls use the cached model from ~/.cache/huggingface/
    pub fn new() -> Result<Self> {
        let inner = ob_semantic_matcher::Embedder::new()
            .map_err(|e| anyhow!("Failed to load Candle model: {}", e))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Blocking embed for synchronous code paths (like ESPER navigation).
    ///
    /// Intended for rare trie misses where we need to fall back to semantic search.
    /// Takes ~5-15ms per embed, acceptable for occasional fallback.
    ///
    /// # Panics
    /// Must not be called from an async context (use `embed` instead).
    pub fn embed_blocking(&self, text: &str) -> Result<Embedding> {
        let guard = self.inner.blocking_lock();
        guard
            .embed(text)
            .map_err(|e| anyhow!("Candle embed failed: {}", e))
    }

    /// Blocking batch embed for synchronous code paths.
    ///
    /// More efficient than calling embed_blocking multiple times.
    pub fn embed_batch_blocking(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let guard = self.inner.blocking_lock();
        guard
            .embed_batch(texts)
            .map_err(|e| anyhow!("Candle batch embed failed: {}", e))
    }
}

#[async_trait]
impl Embedder for CandleEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let text = text.to_string();
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = inner.blocking_lock();
            guard
                .embed(&text)
                .map_err(|e| anyhow!("Candle embed failed: {}", e))
        })
        .await?
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = inner.blocking_lock();
            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            guard
                .embed_batch(&refs)
                .map_err(|e| anyhow!("Candle batch embed failed: {}", e))
        })
        .await?
    }

    fn model_name(&self) -> &str {
        "all-MiniLM-L6-v2"
    }

    fn dimension(&self) -> usize {
        EMBEDDING_DIMENSION
    }
}

/// Cached embedder wrapper for efficiency
pub struct CachedEmbedder {
    inner: Arc<dyn Embedder>,
    cache: RwLock<HashMap<String, Embedding>>,
    max_cache_size: usize,
}

impl CachedEmbedder {
    /// Create cached wrapper around an embedder
    pub fn new(inner: Arc<dyn Embedder>) -> Self {
        Self {
            inner,
            cache: RwLock::new(HashMap::new()),
            max_cache_size: 10000,
        }
    }

    /// Create with custom cache size
    pub fn with_max_cache(inner: Arc<dyn Embedder>, max_size: usize) -> Self {
        Self {
            inner,
            cache: RwLock::new(HashMap::new()),
            max_cache_size: max_size,
        }
    }
}

#[async_trait]
impl Embedder for CachedEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(emb) = cache.get(text) {
                return Ok(emb.clone());
            }
        }

        // Generate embedding
        let embedding = self.inner.embed(text).await?;

        // Cache result (with size limit)
        {
            let mut cache = self.cache.write().await;
            if cache.len() < self.max_cache_size {
                cache.insert(text.to_string(), embedding.clone());
            }
        }

        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        // Check which texts are cached
        let mut results = vec![None; texts.len()];
        let mut uncached_indices = Vec::new();
        let mut uncached_texts = Vec::new();

        {
            let cache = self.cache.read().await;
            for (i, text) in texts.iter().enumerate() {
                if let Some(emb) = cache.get(*text) {
                    results[i] = Some(emb.clone());
                } else {
                    uncached_indices.push(i);
                    uncached_texts.push(*text);
                }
            }
        }

        // Fetch uncached embeddings
        if !uncached_texts.is_empty() {
            let new_embeddings = self.inner.embed_batch(&uncached_texts).await?;

            // Store in cache and results
            let mut cache = self.cache.write().await;
            for (idx, embedding) in uncached_indices.into_iter().zip(new_embeddings) {
                if cache.len() < self.max_cache_size {
                    cache.insert(texts[idx].to_string(), embedding.clone());
                }
                results[idx] = Some(embedding);
            }
        }

        // Convert to final result
        results
            .into_iter()
            .enumerate()
            .map(|(i, opt)| opt.ok_or_else(|| anyhow!("Missing embedding for index {}", i)))
            .collect()
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }

    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}

/// Null embedder for testing (returns zero vectors)
#[derive(Default)]
pub struct NullEmbedder {
    dimension: usize,
}

impl NullEmbedder {
    pub fn new() -> Self {
        Self {
            dimension: EMBEDDING_DIMENSION,
        }
    }

    pub fn with_dimension(dimension: usize) -> Self {
        Self { dimension }
    }
}

#[async_trait]
impl Embedder for NullEmbedder {
    async fn embed(&self, _text: &str) -> Result<Embedding> {
        Ok(vec![0.0; self.dimension])
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        Ok(texts.iter().map(|_| vec![0.0; self.dimension]).collect())
    }

    fn model_name(&self) -> &str {
        "null"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_null_embedder() {
        let embedder = NullEmbedder::new();
        let embedding = embedder.embed("test").await.unwrap();
        assert_eq!(embedding.len(), EMBEDDING_DIMENSION);
        assert!(embedding.iter().all(|&x| x == 0.0));
    }

    #[tokio::test]
    async fn test_cached_embedder() {
        let inner = Arc::new(NullEmbedder::new());
        let cached = CachedEmbedder::new(inner);

        // First call - not cached
        let emb1 = cached.embed("test").await.unwrap();

        // Second call - should be cached
        let emb2 = cached.embed("test").await.unwrap();

        assert_eq!(emb1, emb2);
    }

    #[tokio::test]
    async fn test_batch_embed() {
        let embedder = NullEmbedder::new();
        let texts = vec!["one", "two", "three"];
        let embeddings = embedder.embed_batch(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 3);
        assert!(embeddings.iter().all(|e| e.len() == EMBEDDING_DIMENSION));
    }

    #[tokio::test]
    async fn test_embedding_dimension_constant() {
        assert_eq!(EMBEDDING_DIMENSION, 384);
    }
}

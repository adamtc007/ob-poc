//! Embedding service for semantic learning
//!
//! Provides text embeddings for semantic phrase matching via pgvector.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Embedding vector type (matches pgvector dimension)
pub type Embedding = Vec<f32>;

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

/// OpenAI embeddings client
pub struct OpenAIEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimension: usize,
}

impl OpenAIEmbedder {
    /// Create embedder with default model (text-embedding-3-small)
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: "text-embedding-3-small".to_string(),
            dimension: 1536,
        }
    }

    /// Create embedder with specific model
    pub fn with_model(api_key: String, model: String, dimension: usize) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            dimension,
        }
    }

    /// Create from environment variable
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow!("OPENAI_API_KEY environment variable not set"))?;
        Ok(Self::new(api_key))
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": text
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<EmbeddingResponse>()
            .await?;

        response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| anyhow!("No embedding in response"))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // OpenAI supports batch embedding
        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": texts
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<EmbeddingResponse>()
            .await?;

        // Sort by index to maintain order
        let mut embeddings: Vec<_> = response.data.into_iter().collect();
        embeddings.sort_by_key(|d| d.index);

        Ok(embeddings.into_iter().map(|d| d.embedding).collect())
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
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
        Self { dimension: 1536 }
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
        assert_eq!(embedding.len(), 1536);
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
    }
}

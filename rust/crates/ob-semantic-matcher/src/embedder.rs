//! Application facade over the host-neutral semantic embedder.
//!
//! This module deliberately retains ob-poc's local fine-tuned bundle search.
//! Model parsing and inference live in `semantic-embedder`; PostgreSQL,
//! pgvector, feedback, centroids, and population remain in this application
//! crate.

use anyhow::Result;
use semantic_embedder::{
    CandleEmbedder, DEFAULT_EMBEDDING_DIMENSION, DEFAULT_MODEL_REPOSITORY, DEFAULT_MODEL_REVISION,
};
use tracing::info;

/// Embedding dimension of the current ob-poc compatibility model.
pub const EMBEDDING_DIM: usize = DEFAULT_EMBEDDING_DIMENSION;

/// ob-poc compatibility facade for BGE query and target embeddings.
pub struct Embedder {
    inner: CandleEmbedder,
}

impl Embedder {
    /// Load the first configured local fine-tuned bundle, then fall back to
    /// the exact upstream model revision.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embedder = ob_semantic_matcher::Embedder::new()?;
    /// assert_eq!(embedder.embedding_dim(), 384);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new() -> Result<Self> {
        let paths = [
            "assets/bge-small-en-v1.5-finetuned",
            "../assets/bge-small-en-v1.5-finetuned",
            "rust/assets/bge-small-en-v1.5-finetuned",
        ];
        for path in paths {
            if std::path::Path::new(path).is_dir() {
                info!(path, "found local fine-tuned semantic model");
                return Self::with_model(path);
            }
        }
        Self::with_model(DEFAULT_MODEL_REPOSITORY)
    }

    /// Load a local model directory or the named Hugging Face repository at
    /// the compatibility revision.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embedder = ob_semantic_matcher::Embedder::with_model(
    ///     "BAAI/bge-small-en-v1.5",
    /// )?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_model(model_name: &str) -> Result<Self> {
        Self::with_model_and_revision(model_name, DEFAULT_MODEL_REVISION)
    }

    /// Load a local model directory or an exact remote model revision.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embedder = ob_semantic_matcher::Embedder::with_model_and_revision(
    ///     "BAAI/bge-small-en-v1.5",
    ///     semantic_embedder::DEFAULT_MODEL_REVISION,
    /// )?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_model_and_revision(model_name: &str, revision: &str) -> Result<Self> {
        Ok(Self {
            inner: CandleEmbedder::with_model_and_revision(model_name, revision)?,
        })
    }

    /// Embed a user query with the retrieval instruction.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embedder = ob_semantic_matcher::Embedder::new()?;
    /// let vector = embedder.embed_query("find a process")?;
    /// assert_eq!(vector.len(), embedder.embedding_dim());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.inner.embed_query(text)?)
    }

    /// Embed a retrieval target without the query instruction.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embedder = ob_semantic_matcher::Embedder::new()?;
    /// let vector = embedder.embed_target("create process")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn embed_target(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.inner.embed_target(text)?)
    }

    /// Batch-embed targets while preserving input order.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embedder = ob_semantic_matcher::Embedder::new()?;
    /// let vectors = embedder.embed_batch_targets(&["one", "two"])?;
    /// assert_eq!(vectors.len(), 2);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn embed_batch_targets(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(self.inner.embed_batch_targets(texts)?)
    }

    /// Batch-embed queries while preserving input order.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embedder = ob_semantic_matcher::Embedder::new()?;
    /// let vectors = embedder.embed_batch_queries(&["one", "two"])?;
    /// assert_eq!(vectors.len(), 2);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn embed_batch_queries(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(self.inner.embed_batch_queries(texts)?)
    }

    /// Legacy target embedding retained for application compatibility.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[allow(deprecated)]
    /// let vector = ob_semantic_matcher::Embedder::new()?.embed("target")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    #[deprecated(note = "use embed_target for explicit query/target semantics")]
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_target(text)
    }

    /// Legacy target batch embedding retained for application compatibility.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[allow(deprecated)]
    /// let vectors = ob_semantic_matcher::Embedder::new()?.embed_batch(&["target"])?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    #[deprecated(note = "use embed_batch_targets for explicit query/target semantics")]
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch_targets(texts)
    }

    /// Return the fixed model output dimension.
    #[must_use]
    pub const fn embedding_dim(&self) -> usize {
        EMBEDDING_DIM
    }

    /// Return the compatibility model repository identity.
    #[must_use]
    pub const fn model_name(&self) -> &str {
        DEFAULT_MODEL_REPOSITORY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_constants_match_the_shared_model_contract() {
        assert_eq!(
            EMBEDDING_DIM,
            semantic_embedder::DEFAULT_EMBEDDING_DIMENSION
        );
        assert_eq!(DEFAULT_MODEL_REPOSITORY, "BAAI/bge-small-en-v1.5");
    }
}

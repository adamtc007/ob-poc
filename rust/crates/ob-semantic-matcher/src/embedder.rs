//! Sentence embedding using Candle and BGE-small-en-v1.5
//!
//! This module loads the BAAI/bge-small-en-v1.5 model and computes
//! 384-dimensional embeddings for text inputs.
//!
//! BGE is a **retrieval-optimized** model (query→target) vs MiniLM's
//! similarity model (paraphrase detection). This aligns with our
//! intent→verb lookup use case.
//!
//! Key differences from MiniLM:
//! - CLS token pooling (not mean pooling)
//! - Query instruction prefix for asymmetric retrieval
//! - Higher confidence scores (threshold adjustment required)

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;
use tracing::{debug, info};

/// BGE retrieval instruction prefix - apply to QUERIES ONLY, never targets
///
/// This tells the model we're doing retrieval search, which activates
/// instruction-following behavior that bridges informal queries to formal targets.
const QUERY_PREFIX: &str = "Represent this sentence for searching relevant passages: ";

/// Model repository on HuggingFace Hub
const MODEL_REPO: &str = "BAAI/bge-small-en-v1.5";

/// Embedding dimension (same as MiniLM - no pgvector schema changes needed)
pub const EMBEDDING_DIM: usize = 384;

/// Sentence embedder using BGE-small-en-v1.5
///
/// This model produces 384-dimensional embeddings optimized for retrieval tasks.
/// It uses CLS token pooling (not mean pooling like MiniLM) and supports
/// instruction prefixes for asymmetric query/target embedding.
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl Embedder {
    /// Create a new embedder, downloading the model if needed
    ///
    /// The model is cached in the HuggingFace cache directory (~/.cache/huggingface).
    /// First download is ~130MB.
    pub fn new() -> Result<Self> {
        Self::with_model(MODEL_REPO)
    }

    /// Create an embedder with a specific model name
    pub fn with_model(model_name: &str) -> Result<Self> {
        info!("Loading embedding model: {}", model_name);

        let device = Device::Cpu; // Use CPU for portability; GPU can be added later

        // Download model files from HuggingFace Hub
        let api = Api::new().context("Failed to create HuggingFace API client")?;
        let repo = api.repo(Repo::new(model_name.to_string(), RepoType::Model));

        let config_path = repo
            .get("config.json")
            .context("Failed to download config.json")?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .context("Failed to download tokenizer.json")?;
        let weights_path = repo
            .get("model.safetensors")
            .context("Failed to download model.safetensors")?;

        debug!("Model files downloaded to cache");

        // Load config
        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(&config_path).context("Failed to read config.json")?,
        )
        .context("Failed to parse config.json")?;

        debug!("Model config: hidden_size={}", config.hidden_size);

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        debug!("Tokenizer loaded");

        // Load model weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)
                .context("Failed to load model weights")?
        };

        let model = BertModel::load(vb, &config).context("Failed to build BERT model")?;

        info!("Embedding model loaded successfully (BGE-small-en-v1.5)");

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Internal: run forward pass and extract CLS embedding
    ///
    /// BGE uses CLS token pooling (position 0), NOT mean pooling.
    /// Output is L2 normalized for cosine similarity.
    fn forward(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.forward_batch(&[text])?;
        Ok(embeddings.into_iter().next().unwrap())
    }

    /// Internal: batch forward pass with CLS extraction
    fn forward_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Tokenize all texts
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        // Find max length for padding
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);

        // Prepare input tensors
        let mut all_input_ids = Vec::new();
        let mut all_attention_mask = Vec::new();
        let mut all_token_type_ids = Vec::new();

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let attention = encoding.get_attention_mask();
            let type_ids = encoding.get_type_ids();

            // Pad to max length
            let mut padded_ids = ids.to_vec();
            let mut padded_attention = attention.to_vec();
            let mut padded_type_ids = type_ids.to_vec();

            padded_ids.resize(max_len, 0);
            padded_attention.resize(max_len, 0);
            padded_type_ids.resize(max_len, 0);

            all_input_ids.extend(padded_ids);
            all_attention_mask.extend(padded_attention);
            all_token_type_ids.extend(padded_type_ids);
        }

        let batch_size = texts.len();

        // Create tensors
        let input_ids = Tensor::from_vec(all_input_ids, (batch_size, max_len), &self.device)?;
        let attention_mask =
            Tensor::from_vec(all_attention_mask, (batch_size, max_len), &self.device)?;
        let token_type_ids =
            Tensor::from_vec(all_token_type_ids, (batch_size, max_len), &self.device)?;

        // Convert to appropriate types
        let input_ids = input_ids.to_dtype(DType::U32)?;
        let token_type_ids = token_type_ids.to_dtype(DType::U32)?;

        // Forward pass
        let output = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))?;

        // CLS token extraction: take position 0 from each sequence
        // output shape: (batch_size, seq_len, hidden_size)
        // We want: (batch_size, hidden_size) by taking [:, 0, :]
        let cls_embeddings = output.narrow(1, 0, 1)?.squeeze(1)?;

        // L2 normalize for cosine similarity (pgvector expects unit vectors)
        let normalized = Self::l2_normalize(&cls_embeddings)?;

        // Convert to Vec<Vec<f32>>
        let embeddings_2d = normalized.to_vec2::<f32>()?;

        Ok(embeddings_2d)
    }

    /// L2 normalize embeddings for cosine similarity
    fn l2_normalize(tensor: &Tensor) -> Result<Tensor> {
        let norm = tensor
            .sqr()?
            .sum_keepdim(1)?
            .sqrt()?
            .clamp(1e-12, f64::MAX)?;
        let normalized = tensor.broadcast_div(&norm)?;
        Ok(normalized)
    }

    // ========== PUBLIC API ==========

    /// Embed a user query (WITH retrieval instruction prefix)
    ///
    /// Use this for search queries from user input. The instruction prefix
    /// tells BGE to optimize for retrieval, bridging informal queries to
    /// formal verb patterns.
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        let prefixed = format!("{}{}", QUERY_PREFIX, text);
        self.forward(&prefixed)
    }

    /// Embed a verb pattern/target (NO prefix)
    ///
    /// Use this for verb patterns stored in the database. No prefix needed
    /// because these are the targets being searched, not queries.
    pub fn embed_target(&self, text: &str) -> Result<Vec<f32>> {
        self.forward(text)
    }

    /// Batch embed targets (for populate_embeddings)
    ///
    /// Efficient batch embedding of verb patterns. No prefix applied.
    pub fn embed_batch_targets(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.forward_batch(texts)
    }

    /// Batch embed queries
    ///
    /// Efficient batch embedding of user queries. Instruction prefix applied.
    pub fn embed_batch_queries(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let prefixed: Vec<String> = texts
            .iter()
            .map(|t| format!("{}{}", QUERY_PREFIX, t))
            .collect();
        let refs: Vec<&str> = prefixed.iter().map(|s| s.as_str()).collect();
        self.forward_batch(&refs)
    }

    // ========== LEGACY API (for backward compatibility) ==========

    /// Legacy: embed without query/target distinction
    ///
    /// Defaults to target embedding (no prefix). Use embed_query or embed_target
    /// for explicit control.
    #[deprecated(
        note = "Use embed_query() or embed_target() for explicit query/target distinction"
    )]
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_target(text)
    }

    /// Legacy: batch embed without query/target distinction
    ///
    /// Defaults to target embedding (no prefix). Use embed_batch_queries or
    /// embed_batch_targets for explicit control.
    #[deprecated(
        note = "Use embed_batch_queries() or embed_batch_targets() for explicit query/target distinction"
    )]
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch_targets(texts)
    }

    /// Get the embedding dimension (384 for BGE-small-en-v1.5)
    pub fn embedding_dim(&self) -> usize {
        EMBEDDING_DIM
    }

    /// Get the model name
    pub fn model_name(&self) -> &str {
        MODEL_REPO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires model download
    fn test_embed_single() {
        let embedder = Embedder::new().expect("Failed to load embedder");
        let embedding = embedder
            .embed_target("follow the white rabbit")
            .expect("Failed to embed");

        assert_eq!(embedding.len(), EMBEDDING_DIM);

        // Check that it's normalized (L2 norm ≈ 1.0)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_query_vs_target() {
        let embedder = Embedder::new().expect("Failed to load embedder");

        // Query embedding should be different from target (due to prefix)
        let query_emb = embedder.embed_query("load the cbu").unwrap();
        let target_emb = embedder.embed_target("load the cbu").unwrap();

        // They should be different
        let diff: f32 = query_emb
            .iter()
            .zip(&target_emb)
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.1, "Query and target embeddings should differ");
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_batch() {
        let embedder = Embedder::new().expect("Failed to load embedder");
        let texts = vec!["follow the rabbit", "zoom in", "show ownership"];

        let embeddings = embedder
            .embed_batch_targets(&texts)
            .expect("Failed to embed batch");

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), EMBEDDING_DIM);
        }
    }

    #[test]
    #[ignore] // Requires model download
    fn test_retrieval_similarity() {
        let embedder = Embedder::new().expect("Failed to load embedder");

        // Query: informal user input
        let query = embedder.embed_query("who owns this company").unwrap();

        // Targets: formal verb patterns
        let target_good = embedder
            .embed_target("discover ownership structure")
            .unwrap();
        let target_bad = embedder.embed_target("zoom in on graph").unwrap();

        // Cosine similarity (embeddings are normalized)
        let sim_good: f32 = query.iter().zip(&target_good).map(|(a, b)| a * b).sum();
        let sim_bad: f32 = query.iter().zip(&target_bad).map(|(a, b)| a * b).sum();

        // Query should be more similar to ownership target
        assert!(
            sim_good > sim_bad,
            "Expected sim_good ({}) > sim_bad ({})",
            sim_good,
            sim_bad
        );
    }
}

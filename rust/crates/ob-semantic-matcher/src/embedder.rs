//! Sentence embedding using Candle and all-MiniLM-L6-v2
//!
//! This module loads the HuggingFace sentence-transformers model and computes
//! 384-dimensional embeddings for text inputs.

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;
use tracing::{debug, info};

/// Sentence embedder using all-MiniLM-L6-v2
///
/// This model produces 384-dimensional embeddings optimized for semantic similarity.
/// It's small (~22MB) and fast, making it suitable for real-time voice matching.
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    normalize: bool,
}

impl Embedder {
    /// Create a new embedder, downloading the model if needed
    ///
    /// The model is cached in the HuggingFace cache directory (~/.cache/huggingface).
    pub fn new() -> Result<Self> {
        Self::with_model("sentence-transformers/all-MiniLM-L6-v2")
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

        info!("Embedding model loaded successfully");

        Ok(Self {
            model,
            tokenizer,
            device,
            normalize: true, // L2 normalize for cosine similarity
        })
    }

    /// Compute embedding for a single text input
    ///
    /// Returns a 384-dimensional vector suitable for cosine similarity search.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text])?;
        Ok(embeddings.into_iter().next().unwrap())
    }

    /// Compute embeddings for a batch of texts
    ///
    /// More efficient than calling `embed` multiple times.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
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

        // Mean pooling over sequence dimension (with attention mask)
        let attention_mask_expanded = attention_mask
            .unsqueeze(2)?
            .expand(output.shape())?
            .to_dtype(output.dtype())?;

        let sum_embeddings = (output.clone() * &attention_mask_expanded)?.sum(1)?;
        let sum_mask = attention_mask_expanded.sum(1)?.clamp(1e-9, f64::MAX)?;
        let mean_pooled = sum_embeddings.broadcast_div(&sum_mask)?;

        // Optionally L2 normalize
        let final_embeddings = if self.normalize {
            Self::l2_normalize(&mean_pooled)?
        } else {
            mean_pooled
        };

        // Convert to Vec<Vec<f32>>
        let embeddings_2d = final_embeddings.to_vec2::<f32>()?;

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

    /// Get the embedding dimension (384 for all-MiniLM-L6-v2)
    pub fn embedding_dim(&self) -> usize {
        384
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
            .embed("follow the white rabbit")
            .expect("Failed to embed");

        assert_eq!(embedding.len(), 384);

        // Check that it's normalized (L2 norm ≈ 1.0)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_batch() {
        let embedder = Embedder::new().expect("Failed to load embedder");
        let texts = vec!["follow the rabbit", "zoom in", "show ownership"];

        let embeddings = embedder.embed_batch(&texts).expect("Failed to embed batch");

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 384);
        }
    }

    #[test]
    #[ignore] // Requires model download
    fn test_semantic_similarity() {
        let embedder = Embedder::new().expect("Failed to load embedder");

        let emb1 = embedder.embed("who owns this company").unwrap();
        let emb2 = embedder.embed("show me the ownership structure").unwrap();
        let emb3 = embedder.embed("zoom in on the graph").unwrap();

        // Cosine similarity (embeddings are normalized)
        let sim_12: f32 = emb1.iter().zip(&emb2).map(|(a, b)| a * b).sum();
        let sim_13: f32 = emb1.iter().zip(&emb3).map(|(a, b)| a * b).sum();

        // "who owns" should be more similar to "ownership" than to "zoom in"
        assert!(
            sim_12 > sim_13,
            "Expected sim_12 ({}) > sim_13 ({})",
            sim_12,
            sim_13
        );
    }
}

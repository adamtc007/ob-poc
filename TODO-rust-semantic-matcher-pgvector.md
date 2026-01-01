# TODO: Rust Semantic Matcher with pgvector

## Overview

Pure Rust implementation of semantic intent matching using Candle (HuggingFace Rust) for embeddings and pgvector for storage/search. Single codebase, no Python runtime dependency.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    STARTUP (once)                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 1. Load MiniLM model via Candle                     │   │
│  │ 2. Check pgvector for existing pattern embeddings   │   │
│  │ 3. If missing/stale: embed all patterns, store      │   │
│  │ 4. Load phonetic index into memory                  │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    RUNTIME (per query)                      │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 1. Embed query text (Candle) → 384-dim vector       │   │
│  │ 2. Query pgvector for top-K similar patterns        │   │
│  │ 3. Query phonetic index for sound-alike matches     │   │
│  │ 4. Combine scores → ranked verb suggestions         │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## Task 1: Database Schema (pgvector)

### 1.1 Enable pgvector Extension

```sql
-- Run once on database
CREATE EXTENSION IF NOT EXISTS vector;
```

### 1.2 Create Pattern Embeddings Table

```sql
-- Store embeddings for each intent pattern
CREATE TABLE IF NOT EXISTS "ob-poc".verb_pattern_embeddings (
    id SERIAL PRIMARY KEY,
    verb_full_name TEXT NOT NULL,
    pattern TEXT NOT NULL,
    embedding vector(384) NOT NULL,
    phonetic_code TEXT,  -- Double Metaphone primary code
    model_version TEXT DEFAULT 'all-MiniLM-L6-v2',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Constraints
    UNIQUE(verb_full_name, pattern),
    CONSTRAINT fk_verb FOREIGN KEY (verb_full_name) 
        REFERENCES "ob-poc".dsl_verbs(full_name) ON DELETE CASCADE
);

-- Create IVFFlat index for fast approximate nearest neighbor search
-- lists = sqrt(num_patterns) is a good starting point
-- For ~1000 patterns, lists = 32 is reasonable
CREATE INDEX IF NOT EXISTS idx_pattern_embedding_ivfflat
ON "ob-poc".verb_pattern_embeddings 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 32);

-- Index for phonetic lookups
CREATE INDEX IF NOT EXISTS idx_phonetic_code
ON "ob-poc".verb_pattern_embeddings(phonetic_code);

-- Index for model version (for cache invalidation)
CREATE INDEX IF NOT EXISTS idx_model_version
ON "ob-poc".verb_pattern_embeddings(model_version);
```

### 1.3 Migration File

**File:** `migrations/XXXXXX_add_pattern_embeddings.sql`

```sql
-- Up
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS "ob-poc".verb_pattern_embeddings (
    id SERIAL PRIMARY KEY,
    verb_full_name TEXT NOT NULL,
    pattern TEXT NOT NULL,
    embedding vector(384) NOT NULL,
    phonetic_code TEXT,
    model_version TEXT DEFAULT 'all-MiniLM-L6-v2',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(verb_full_name, pattern)
);

CREATE INDEX idx_pattern_embedding_ivfflat
ON "ob-poc".verb_pattern_embeddings 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 32);

CREATE INDEX idx_phonetic_code
ON "ob-poc".verb_pattern_embeddings(phonetic_code);

-- Down
DROP TABLE IF EXISTS "ob-poc".verb_pattern_embeddings;
```

---

## Task 2: Rust Dependencies

### 2.1 Add to Cargo.toml

```toml
[dependencies]
# Embedding model (Candle - pure Rust)
candle-core = "0.8"
candle-nn = "0.8"
candle-transformers = "0.8"
hf-hub = "0.3"              # Download models from HuggingFace Hub
tokenizers = "0.20"         # Fast BPE tokenizer

# pgvector support
pgvector = "0.4"            # pgvector types for sqlx

# Phonetic matching
rphonetic = "2.0"           # Double Metaphone implementation

# String similarity (backup)
strsim = "0.11"             # Levenshtein, Jaro-Winkler
```

### 2.2 Feature Flags (Optional GPU)

```toml
[features]
default = ["cpu"]
cpu = []
cuda = ["candle-core/cuda", "candle-nn/cuda", "candle-transformers/cuda"]
metal = ["candle-core/metal", "candle-nn/metal", "candle-transformers/metal"]
```

---

## Task 3: Embedding Model Loader

**File:** `rust/src/session/embeddings/model.rs`

```rust
//! Sentence embedding model using Candle
//!
//! Loads all-MiniLM-L6-v2 from HuggingFace Hub and provides
//! sentence embedding functionality.

use anyhow::{Context, Result};
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;
use std::path::PathBuf;

/// Model identifier on HuggingFace Hub
const MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Expected embedding dimension
pub const EMBEDDING_DIM: usize = 384;

/// Sentence embedding model
pub struct EmbeddingModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl EmbeddingModel {
    /// Load model from HuggingFace Hub (downloads on first use, cached after)
    pub fn load() -> Result<Self> {
        let device = Device::Cpu; // Use Device::cuda_if_available(0)? for GPU
        
        // Download model files from HuggingFace Hub
        let api = Api::new()?;
        let repo = api.repo(Repo::new(MODEL_ID.to_string(), RepoType::Model));
        
        let config_path = repo.get("config.json")?;
        let tokenizer_path = repo.get("tokenizer.json")?;
        let weights_path = repo.get("model.safetensors")?;
        
        // Load config
        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
        
        // Load tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;
        
        // Load model weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
        };
        
        let model = BertModel::load(vb, &config)?;
        
        tracing::info!("Loaded embedding model: {}", MODEL_ID);
        
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }
    
    /// Embed a single sentence, returns 384-dim vector
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        
        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let token_type_ids = encoding.get_type_ids();
        
        let input_ids = Tensor::new(input_ids, &self.device)?.unsqueeze(0)?;
        let attention_mask = Tensor::new(attention_mask, &self.device)?.unsqueeze(0)?;
        let token_type_ids = Tensor::new(token_type_ids, &self.device)?.unsqueeze(0)?;
        
        // Forward pass
        let output = self.model.forward(&input_ids, &token_type_ids, Some(&attention_mask))?;
        
        // Mean pooling over sequence length (dim 1)
        let sum = output.sum(1)?;
        let count = attention_mask.sum(1)?.to_dtype(candle_core::DType::F32)?;
        let pooled = sum.broadcast_div(&count.unsqueeze(1)?)?;
        
        // Normalize to unit vector
        let norm = pooled.sqr()?.sum(1)?.sqrt()?;
        let normalized = pooled.broadcast_div(&norm.unsqueeze(1)?)?;
        
        // Extract as Vec<f32>
        let embedding: Vec<f32> = normalized.squeeze(0)?.to_vec1()?;
        
        Ok(embedding)
    }
    
    /// Embed multiple sentences (batch processing)
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // For simplicity, process one at a time
        // TODO: Implement proper batching for performance
        texts.iter()
            .map(|text| self.embed(text))
            .collect()
    }
    
    /// Get model version string (for cache invalidation)
    pub fn version(&self) -> &'static str {
        MODEL_ID
    }
}
```

---

## Task 4: Phonetic Matcher

**File:** `rust/src/session/embeddings/phonetic.rs`

```rust
//! Phonetic matching using Double Metaphone
//!
//! Handles voice transcription errors like "enhawnce" → "enhance"

use rphonetic::{DoubleMetaphone, Encoder};
use std::collections::HashMap;

/// Phonetic matcher for voice error recovery
pub struct PhoneticMatcher {
    encoder: DoubleMetaphone,
    /// Map from phonetic code to (verb, original_pattern) pairs
    code_index: HashMap<String, Vec<(String, String)>>,
}

impl PhoneticMatcher {
    pub fn new() -> Self {
        Self {
            encoder: DoubleMetaphone::default(),
            code_index: HashMap::new(),
        }
    }
    
    /// Build index from verb patterns
    pub fn build_index(&mut self, patterns: &[(String, String)]) {
        self.code_index.clear();
        
        for (verb, pattern) in patterns {
            let code = self.encode_phrase(pattern);
            self.code_index
                .entry(code)
                .or_default()
                .push((verb.clone(), pattern.clone()));
        }
        
        tracing::info!(
            "Built phonetic index: {} unique codes from {} patterns",
            self.code_index.len(),
            patterns.len()
        );
    }
    
    /// Encode a phrase (space-separated words) to phonetic code
    pub fn encode_phrase(&self, phrase: &str) -> String {
        phrase
            .split_whitespace()
            .filter_map(|word| {
                let clean = word.chars()
                    .filter(|c| c.is_alphabetic())
                    .collect::<String>();
                if clean.is_empty() {
                    None
                } else {
                    Some(self.encoder.encode(&clean))
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    
    /// Find exact phonetic matches
    pub fn find_exact(&self, query: &str) -> Vec<(String, String, f32)> {
        let query_code = self.encode_phrase(query);
        
        self.code_index
            .get(&query_code)
            .map(|matches| {
                matches.iter()
                    .map(|(verb, pattern)| (verb.clone(), pattern.clone(), 1.0))
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Find fuzzy phonetic matches (edit distance on codes)
    pub fn find_fuzzy(&self, query: &str, threshold: f32) -> Vec<(String, String, f32)> {
        let query_code = self.encode_phrase(query);
        let query_len = query_code.len();
        
        if query_len == 0 {
            return Vec::new();
        }
        
        let mut results = Vec::new();
        
        for (code, matches) in &self.code_index {
            let distance = strsim::levenshtein(&query_code, code);
            let max_len = query_len.max(code.len());
            let similarity = 1.0 - (distance as f32 / max_len as f32);
            
            if similarity >= threshold {
                for (verb, pattern) in matches {
                    results.push((verb.clone(), pattern.clone(), similarity));
                }
            }
        }
        
        results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

impl Default for PhoneticMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_voice_error_recovery() {
        let mut matcher = PhoneticMatcher::new();
        matcher.build_index(&[
            ("ui.zoom-in".to_string(), "enhance".to_string()),
            ("ui.pan-right".to_string(), "track right".to_string()),
            ("ui.drill-through".to_string(), "drill through".to_string()),
        ]);
        
        // Common voice transcription errors
        assert!(!matcher.find_exact("enhawnce").is_empty()); // enhance
        assert!(!matcher.find_fuzzy("trak right", 0.7).is_empty()); // track right
        assert!(!matcher.find_fuzzy("drill frew", 0.7).is_empty()); // drill through
    }
}
```

---

## Task 5: pgvector Repository

**File:** `rust/src/session/embeddings/repository.rs`

```rust
//! pgvector repository for storing and querying pattern embeddings

use anyhow::Result;
use pgvector::Vector;
use sqlx::PgPool;
use std::sync::Arc;

/// Pattern embedding row from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PatternEmbeddingRow {
    pub id: i32,
    pub verb_full_name: String,
    pub pattern: String,
    pub embedding: Vector,
    pub phonetic_code: Option<String>,
    pub model_version: String,
}

/// Search result with similarity score
#[derive(Debug, Clone)]
pub struct SimilarPattern {
    pub verb: String,
    pub pattern: String,
    pub similarity: f32,
}

/// Repository for pattern embeddings in pgvector
pub struct EmbeddingRepository {
    pool: Arc<PgPool>,
}

impl EmbeddingRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    
    /// Check if embeddings exist for given model version
    pub async fn has_embeddings(&self, model_version: &str) -> Result<bool> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "ob-poc".verb_pattern_embeddings
            WHERE model_version = $1
            "#
        )
        .bind(model_version)
        .fetch_one(self.pool.as_ref())
        .await?;
        
        Ok(count.0 > 0)
    }
    
    /// Get count of embeddings
    pub async fn count(&self) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".verb_pattern_embeddings"#
        )
        .fetch_one(self.pool.as_ref())
        .await?;
        
        Ok(count.0)
    }
    
    /// Clear all embeddings (for rebuild)
    pub async fn clear(&self) -> Result<()> {
        sqlx::query(r#"DELETE FROM "ob-poc".verb_pattern_embeddings"#)
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }
    
    /// Insert a pattern embedding
    pub async fn insert(
        &self,
        verb: &str,
        pattern: &str,
        embedding: &[f32],
        phonetic_code: &str,
        model_version: &str,
    ) -> Result<()> {
        let vector = Vector::from(embedding.to_vec());
        
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".verb_pattern_embeddings 
                (verb_full_name, pattern, embedding, phonetic_code, model_version)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (verb_full_name, pattern) DO UPDATE SET
                embedding = EXCLUDED.embedding,
                phonetic_code = EXCLUDED.phonetic_code,
                model_version = EXCLUDED.model_version,
                created_at = NOW()
            "#
        )
        .bind(verb)
        .bind(pattern)
        .bind(vector)
        .bind(phonetic_code)
        .bind(model_version)
        .execute(self.pool.as_ref())
        .await?;
        
        Ok(())
    }
    
    /// Batch insert pattern embeddings
    pub async fn insert_batch(
        &self,
        embeddings: &[(String, String, Vec<f32>, String)], // (verb, pattern, embedding, phonetic)
        model_version: &str,
    ) -> Result<()> {
        // Use transaction for atomicity
        let mut tx = self.pool.begin().await?;
        
        for (verb, pattern, embedding, phonetic) in embeddings {
            let vector = Vector::from(embedding.clone());
            
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".verb_pattern_embeddings 
                    (verb_full_name, pattern, embedding, phonetic_code, model_version)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (verb_full_name, pattern) DO UPDATE SET
                    embedding = EXCLUDED.embedding,
                    phonetic_code = EXCLUDED.phonetic_code,
                    model_version = EXCLUDED.model_version,
                    created_at = NOW()
                "#
            )
            .bind(verb)
            .bind(pattern)
            .bind(vector)
            .bind(phonetic)
            .bind(model_version)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }
    
    /// Search for similar patterns using cosine similarity
    /// Returns top-K most similar patterns
    pub async fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<SimilarPattern>> {
        let query_vector = Vector::from(query_embedding.to_vec());
        
        // Use cosine distance: 1 - cosine_similarity = distance
        // So similarity = 1 - distance
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            r#"
            SELECT 
                verb_full_name,
                pattern,
                1 - (embedding <=> $1) as similarity
            FROM "ob-poc".verb_pattern_embeddings
            ORDER BY embedding <=> $1
            LIMIT $2
            "#
        )
        .bind(query_vector)
        .bind(limit)
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(rows.into_iter()
            .map(|(verb, pattern, similarity)| SimilarPattern {
                verb,
                pattern,
                similarity: similarity as f32,
            })
            .collect())
    }
    
    /// Search by phonetic code (exact match)
    pub async fn search_phonetic(&self, phonetic_code: &str) -> Result<Vec<SimilarPattern>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT verb_full_name, pattern
            FROM "ob-poc".verb_pattern_embeddings
            WHERE phonetic_code = $1
            "#
        )
        .bind(phonetic_code)
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(rows.into_iter()
            .map(|(verb, pattern)| SimilarPattern {
                verb,
                pattern,
                similarity: 1.0, // Exact phonetic match
            })
            .collect())
    }
    
    /// Get all patterns (for phonetic index building)
    pub async fn get_all_patterns(&self) -> Result<Vec<(String, String, Option<String>)>> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT verb_full_name, pattern, phonetic_code
            FROM "ob-poc".verb_pattern_embeddings
            "#
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(rows)
    }
}
```

---

## Task 6: Hybrid Matcher Service

**File:** `rust/src/session/embeddings/matcher.rs`

```rust
//! Hybrid semantic + phonetic matcher
//!
//! Combines semantic similarity (pgvector) with phonetic matching
//! for robust intent recognition from voice/text input.

use super::{
    model::EmbeddingModel,
    phonetic::PhoneticMatcher,
    repository::{EmbeddingRepository, SimilarPattern},
};
use crate::session::verb_rag_metadata::get_intent_patterns;
use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Match confidence level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchConfidence {
    /// > 0.85 - Execute immediately
    High,
    /// 0.6 - 0.85 - Show confirmation
    Medium,
    /// < 0.6 - Show alternatives
    Low,
}

/// A matched verb with scoring details
#[derive(Debug, Clone)]
pub struct VerbMatch {
    pub verb: String,
    pub matched_pattern: String,
    pub semantic_score: f32,
    pub phonetic_score: f32,
    pub final_score: f32,
    pub confidence: MatchConfidence,
}

/// Configuration for hybrid matching
#[derive(Debug, Clone)]
pub struct MatcherConfig {
    /// Weight for semantic similarity (0.0 - 1.0)
    pub semantic_weight: f32,
    /// Weight for phonetic matching (0.0 - 1.0)
    pub phonetic_weight: f32,
    /// Minimum score to consider a match
    pub min_score: f32,
    /// Number of candidates to retrieve from each method
    pub top_k: usize,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self {
            semantic_weight: 0.7,
            phonetic_weight: 0.3,
            min_score: 0.4,
            top_k: 10,
        }
    }
}

/// Hybrid matcher combining semantic embeddings and phonetic matching
pub struct HybridMatcher {
    model: Arc<EmbeddingModel>,
    repository: EmbeddingRepository,
    phonetic: RwLock<PhoneticMatcher>,
    config: MatcherConfig,
}

impl HybridMatcher {
    /// Create new hybrid matcher
    pub async fn new(pool: Arc<PgPool>, config: MatcherConfig) -> Result<Self> {
        tracing::info!("Initializing hybrid matcher...");
        
        // Load embedding model
        let model = Arc::new(EmbeddingModel::load()?);
        
        // Create repository
        let repository = EmbeddingRepository::new(pool.clone());
        
        // Create phonetic matcher
        let phonetic = RwLock::new(PhoneticMatcher::new());
        
        let matcher = Self {
            model,
            repository,
            phonetic,
            config,
        };
        
        // Initialize embeddings if needed
        matcher.ensure_embeddings_initialized().await?;
        
        Ok(matcher)
    }
    
    /// Ensure all pattern embeddings are in pgvector
    async fn ensure_embeddings_initialized(&self) -> Result<()> {
        let model_version = self.model.version();
        
        // Check if embeddings exist for current model version
        if self.repository.has_embeddings(model_version).await? {
            let count = self.repository.count().await?;
            tracing::info!("Found {} existing pattern embeddings for {}", count, model_version);
            
            // Build phonetic index from existing patterns
            self.rebuild_phonetic_index().await?;
            return Ok(());
        }
        
        tracing::info!("Building pattern embeddings for {}...", model_version);
        
        // Get all intent patterns from verb_rag_metadata
        let patterns = get_intent_patterns();
        
        // Clear old embeddings
        self.repository.clear().await?;
        
        // Build phonetic matcher
        let phonetic_matcher = PhoneticMatcher::new();
        
        // Embed all patterns and store
        let mut batch = Vec::new();
        for (verb, verb_patterns) in &patterns {
            for pattern in verb_patterns {
                let embedding = self.model.embed(pattern)?;
                let phonetic_code = phonetic_matcher.encode_phrase(pattern);
                batch.push((verb.clone(), pattern.clone(), embedding, phonetic_code));
            }
        }
        
        tracing::info!("Storing {} pattern embeddings...", batch.len());
        self.repository.insert_batch(&batch, model_version).await?;
        
        // Build phonetic index
        self.rebuild_phonetic_index().await?;
        
        tracing::info!("Pattern embeddings initialized");
        Ok(())
    }
    
    /// Rebuild phonetic index from database
    async fn rebuild_phonetic_index(&self) -> Result<()> {
        let patterns = self.repository.get_all_patterns().await?;
        
        let pattern_pairs: Vec<(String, String)> = patterns
            .into_iter()
            .map(|(verb, pattern, _)| (verb, pattern))
            .collect();
        
        let mut phonetic = self.phonetic.write().await;
        phonetic.build_index(&pattern_pairs);
        
        Ok(())
    }
    
    /// Match user input to verbs
    pub async fn match_intent(&self, query: &str, limit: usize) -> Result<Vec<VerbMatch>> {
        // 1. Semantic search via pgvector
        let query_embedding = self.model.embed(query)?;
        let semantic_results = self.repository
            .search_similar(&query_embedding, limit as i32)
            .await?;
        
        // 2. Phonetic search
        let phonetic = self.phonetic.read().await;
        let phonetic_exact = phonetic.find_exact(query);
        let phonetic_fuzzy = phonetic.find_fuzzy(query, 0.7);
        drop(phonetic);
        
        // 3. Combine results
        let mut verb_scores: std::collections::HashMap<String, VerbMatch> = 
            std::collections::HashMap::new();
        
        // Add semantic results
        for result in semantic_results {
            verb_scores.insert(result.verb.clone(), VerbMatch {
                verb: result.verb,
                matched_pattern: result.pattern,
                semantic_score: result.similarity,
                phonetic_score: 0.0,
                final_score: 0.0,
                confidence: MatchConfidence::Low,
            });
        }
        
        // Add phonetic results
        for (verb, pattern, score) in phonetic_exact.into_iter().chain(phonetic_fuzzy.into_iter()) {
            verb_scores
                .entry(verb.clone())
                .and_modify(|m| {
                    m.phonetic_score = m.phonetic_score.max(score);
                })
                .or_insert(VerbMatch {
                    verb,
                    matched_pattern: pattern,
                    semantic_score: 0.0,
                    phonetic_score: score,
                    final_score: 0.0,
                    confidence: MatchConfidence::Low,
                });
        }
        
        // 4. Calculate final scores
        let mut results: Vec<VerbMatch> = verb_scores
            .into_values()
            .map(|mut m| {
                m.final_score = 
                    m.semantic_score * self.config.semantic_weight +
                    m.phonetic_score * self.config.phonetic_weight;
                
                m.confidence = if m.final_score > 0.85 {
                    MatchConfidence::High
                } else if m.final_score > 0.6 {
                    MatchConfidence::Medium
                } else {
                    MatchConfidence::Low
                };
                
                m
            })
            .filter(|m| m.final_score >= self.config.min_score)
            .collect();
        
        // 5. Sort by final score
        results.sort_by(|a, b| {
            b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        results.truncate(limit);
        
        Ok(results)
    }
    
    /// Force rebuild of all embeddings (e.g., after pattern changes)
    pub async fn rebuild_embeddings(&self) -> Result<()> {
        self.repository.clear().await?;
        self.ensure_embeddings_initialized().await
    }
}
```

---

## Task 7: Module Structure

**File:** `rust/src/session/embeddings/mod.rs`

```rust
//! Semantic embeddings and intent matching
//!
//! This module provides hybrid semantic + phonetic matching for
//! mapping natural language input to DSL verbs.

mod matcher;
mod model;
mod phonetic;
mod repository;

pub use matcher::{HybridMatcher, MatchConfidence, MatcherConfig, VerbMatch};
pub use model::{EmbeddingModel, EMBEDDING_DIM};
pub use phonetic::PhoneticMatcher;
pub use repository::{EmbeddingRepository, SimilarPattern};
```

**Update:** `rust/src/session/mod.rs`

```rust
pub mod embeddings;
// ... existing modules
```

---

## Task 8: Integration with VerbDiscoveryService

**File:** Update `rust/src/session/verb_discovery.rs`

```rust
use crate::session::embeddings::{HybridMatcher, MatcherConfig, VerbMatch};

pub struct VerbDiscoveryService {
    pool: Arc<PgPool>,
    hybrid_matcher: Arc<HybridMatcher>, // ADD THIS
}

impl VerbDiscoveryService {
    pub async fn new(pool: Arc<PgPool>) -> Result<Self, VerbDiscoveryError> {
        // Initialize hybrid matcher
        let matcher_config = MatcherConfig::default();
        let hybrid_matcher = Arc::new(
            HybridMatcher::new(pool.clone(), matcher_config)
                .await
                .map_err(|e| VerbDiscoveryError::Init(e.to_string()))?
        );
        
        Ok(Self {
            pool,
            hybrid_matcher,
        })
    }
    
    pub async fn discover(
        &self,
        query: &DiscoveryQuery,
    ) -> Result<Vec<VerbSuggestion>, VerbDiscoveryError> {
        let mut suggestions = Vec::new();
        let mut seen_verbs = std::collections::HashSet::new();
        
        // NEW: Use hybrid matcher for semantic + phonetic matching
        if let Some(ref text) = query.query_text {
            let matches = self.hybrid_matcher
                .match_intent(text, query.limit)
                .await
                .map_err(|e| VerbDiscoveryError::Database(sqlx::Error::Protocol(e.to_string())))?;
            
            for verb_match in matches {
                if seen_verbs.insert(verb_match.verb.clone()) {
                    suggestions.push(VerbSuggestion {
                        verb: verb_match.verb,
                        domain: extract_domain(&verb_match.verb),
                        description: None, // TODO: lookup from dsl_verbs
                        example: None,
                        category: None,
                        score: verb_match.final_score,
                        reason: SuggestionReason::SemanticMatch {
                            pattern: verb_match.matched_pattern,
                            semantic_score: verb_match.semantic_score,
                            phonetic_score: verb_match.phonetic_score,
                            confidence: verb_match.confidence,
                        },
                    });
                }
            }
        }
        
        // ... rest of existing logic (graph_context, workflow_phase, etc.)
        
        Ok(suggestions)
    }
}

// Add new reason variant
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SuggestionReason {
    // ... existing variants
    
    /// Matched via semantic + phonetic hybrid matching
    SemanticMatch {
        pattern: String,
        semantic_score: f32,
        phonetic_score: f32,
        confidence: MatchConfidence,
    },
}
```

---

## Task 9: Helper for Pattern Extraction

**File:** Update `rust/src/session/verb_rag_metadata.rs`

Add function to extract patterns for embedding:

```rust
/// Get all intent patterns for embedding
/// Returns: Vec<(verb_name, Vec<patterns>)>
pub fn get_intent_patterns() -> Vec<(String, Vec<String>)> {
    let patterns = get_intent_pattern_map();
    patterns
        .into_iter()
        .map(|(verb, pats)| (verb.to_string(), pats.iter().map(|s| s.to_string()).collect()))
        .collect()
}
```

---

## Task 10: Tests

**File:** `rust/src/session/embeddings/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_embedding_model_loads() {
        let model = EmbeddingModel::load().expect("Model should load");
        let embedding = model.embed("test sentence").expect("Should embed");
        assert_eq!(embedding.len(), 384);
    }
    
    #[test]
    fn test_semantic_similarity() {
        let model = EmbeddingModel::load().expect("Model should load");
        
        let e1 = model.embed("show owners").unwrap();
        let e2 = model.embed("list owners").unwrap();
        let e3 = model.embed("delete everything").unwrap();
        
        let sim_same = cosine_similarity(&e1, &e2);
        let sim_diff = cosine_similarity(&e1, &e3);
        
        assert!(sim_same > 0.8, "Similar intents should have high similarity");
        assert!(sim_diff < 0.5, "Different intents should have low similarity");
    }
    
    #[test]
    fn test_phonetic_voice_errors() {
        let mut matcher = PhoneticMatcher::new();
        matcher.build_index(&[
            ("ui.zoom-in".to_string(), "enhance".to_string()),
            ("ui.pan-right".to_string(), "track right".to_string()),
        ]);
        
        // Voice transcription errors should still match
        let results = matcher.find_fuzzy("enhawnce", 0.7);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "ui.zoom-in");
    }
    
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (norm_a * norm_b)
    }
}
```

---

## Implementation Order

| Step | Task | Effort | Dependencies |
|------|------|--------|--------------|
| 1 | Run migration (pgvector extension + table) | 5 min | PostgreSQL access |
| 2 | Add Cargo dependencies | 5 min | None |
| 3 | Implement `model.rs` (Candle loader) | 2 hr | Deps installed |
| 4 | Implement `phonetic.rs` | 30 min | None |
| 5 | Implement `repository.rs` (pgvector CRUD) | 1 hr | Migration done |
| 6 | Implement `matcher.rs` (hybrid scoring) | 1 hr | Steps 3-5 |
| 7 | Add `get_intent_patterns()` to verb_rag_metadata | 15 min | None |
| 8 | Integrate with VerbDiscoveryService | 30 min | Step 6 |
| 9 | Write tests | 1 hr | All above |
| 10 | Test with voice transcripts | 30 min | All above |

**Total estimated effort: ~7-8 hours**

---

## Expected Results

| Input | Before | After |
|-------|--------|-------|
| "show me who owns this" | ❌ No match | ✅ `ubo.list-owners` (0.87) |
| "enhawnce" (voice error) | ❌ No match | ✅ `ui.zoom-in` (0.92 phonetic) |
| "follow the bunny" | ❌ No match | ✅ `ui.follow-the-rabbit` (0.78) |
| "trak left" (accent) | ❌ No match | ✅ `ui.pan-left` (0.85 phonetic) |
| "who's really behind this" | ⚠️ Maybe pattern | ✅ `ui.follow-the-rabbit` (0.91) |

---

## Notes

- **First run:** Model download from HuggingFace Hub (~22MB), cached after
- **Startup time:** ~2-3 seconds to load model + verify embeddings
- **Query latency:** ~20-50ms (embed query + pgvector search)
- **Memory:** ~100MB for model + ~1.5MB for pattern embeddings

//! Types for semantic matching

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A matched verb pattern with confidence score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    /// The verb name (e.g., "ui.follow-the-rabbit", "ubo.list-owners")
    pub verb_name: String,

    /// The pattern phrase that matched
    pub pattern_phrase: String,

    /// Similarity score (0.0 - 1.0)
    pub similarity: f32,

    /// How the match was found
    pub match_method: MatchMethod,

    /// Category of the verb
    pub category: String,

    /// Whether this requires agent processing
    pub is_agent_bound: bool,
}

/// How the match was determined
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchMethod {
    /// Semantic similarity via embedding
    Semantic,
    /// Phonetic matching via Double Metaphone
    Phonetic,
    /// Exact string match (normalized)
    Exact,
    /// Cached from previous lookup
    Cached,
}

impl std::fmt::Display for MatchMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchMethod::Semantic => write!(f, "semantic"),
            MatchMethod::Phonetic => write!(f, "phonetic"),
            MatchMethod::Exact => write!(f, "exact"),
            MatchMethod::Cached => write!(f, "cached"),
        }
    }
}

/// A verb pattern stored in the database
#[derive(Debug, Clone)]
pub struct VerbPattern {
    pub id: Uuid,
    pub verb_name: String,
    pub pattern_phrase: String,
    pub pattern_normalized: String,
    pub phonetic_codes: Vec<String>,
    pub embedding: Vec<f32>,
    pub category: String,
    pub is_agent_bound: bool,
    pub priority: i32,
}

/// Configuration for the semantic matcher
#[derive(Debug, Clone)]
pub struct MatcherConfig {
    /// Minimum similarity score to consider a match (default: 0.5)
    pub min_similarity: f32,

    /// Similarity threshold above which we skip phonetic fallback (default: 0.85)
    pub high_confidence_threshold: f32,

    /// Maximum number of candidates to retrieve from pgvector (default: 5)
    pub top_k: usize,

    /// Whether to use the cache (default: true)
    pub use_cache: bool,

    /// Model name for the embedder (default: "sentence-transformers/all-MiniLM-L6-v2")
    pub model_name: String,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self {
            min_similarity: 0.5,
            high_confidence_threshold: 0.85,
            top_k: 5,
            use_cache: true,
            model_name: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        }
    }
}

/// Error types for semantic matching
#[derive(Debug, thiserror::Error)]
pub enum MatcherError {
    #[error("Failed to load model: {0}")]
    ModelLoad(String),

    #[error("Failed to tokenize input: {0}")]
    Tokenization(String),

    #[error("Failed to compute embedding: {0}")]
    Embedding(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("No match found for input")]
    NoMatch,
}

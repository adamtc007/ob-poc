//! Core types for the Sage pack matcher.

use serde::{Deserialize, Serialize};

/// Contextual signals available to the pack matcher.
///
/// All fields are optional — the matcher degrades gracefully with less context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SageContext {
    /// Current domain/workspace scope (e.g., `"kyc"`, `"cbu"`).
    pub domain: Option<String>,
    /// Conversation history — last N turns as plain strings.
    pub history: Vec<String>,
    /// Currently loaded process name (if any).
    pub process_name: Option<String>,
}

impl SageContext {
    /// Empty context — no domain, no history.
    pub fn empty() -> Self {
        Self {
            domain: None,
            history: vec![],
            process_name: None,
        }
    }

    /// Context with a specific domain.
    pub fn with_domain(domain: impl Into<String>) -> Self {
        Self {
            domain: Some(domain.into()),
            history: vec![],
            process_name: None,
        }
    }
}

/// A single ranked candidate returned by the pack matcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedCandidate {
    /// The decision pack's name (e.g., `"conjunctive-gate"`).
    pub pack_name: String,
    /// The decision pack's version string.
    pub pack_version: String,
    /// Combined confidence score `[0, 1]`.
    ///
    /// Scoring function (§1.5):
    ///   `confidence = 0.5 * embedding_score + 0.5 * rank_score`
    ///   where `rank_score = 1.0 - (llm_rank - 1) / N`.
    /// In embedding-only mode `rank_score` is derived from sorted position.
    pub confidence: f32,
    /// Human-readable rationale (from LLM or embedding score fallback).
    pub rationale: String,
    /// Cosine / Jaccard similarity from the retrieval layer `[0, 1]`.
    pub embedding_score: f32,
    /// LLM ranking position (1 = best match).
    ///
    /// `Some` after an LLM ranking call, or after embedding-only ranking
    /// (position within sorted list).  `None` only if the pack was not
    /// evaluated.
    pub llm_rank: Option<usize>,
}

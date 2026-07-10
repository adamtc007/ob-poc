//! Pack matching — two-layer pipeline (embedding retrieval + optional LLM ranking).
//!
//! # Architecture
//!
//! ```text
//! utterance
//!   │
//!   ▼  Layer 1: embedding retrieval
//! ┌──────────────────────────────────────────────────────────────────────┐
//! │  PackEmbedder::similarity(utterance, example_utterance)             │
//! │  For each pack: pack_score = max similarity over example_utterances │
//! │  Sort descending → top-K candidates (K = 5)                         │
//! └──────────────────────────────────────────────────────────────────────┘
//!   │
//!   ▼  Layer 2: optional LLM ranking
//! ┌──────────────────────────────────────────────────────────────────────┐
//! │  LlmClient::rank_packs(utterance, top_k_summaries)                  │
//! │  Returns ranked list with per-pack rationale                        │
//! │  Combine: confidence = 0.5 * embedding_score + 0.5 * rank_score    │
//! └──────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashSet;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use dsl_resolution::{DecisionPack, PackRegistry};

use crate::types::{RankedCandidate, SageContext};

// ---------------------------------------------------------------------------
// Embedder trait
// ---------------------------------------------------------------------------

/// Compute similarity between a query string and a candidate string.
///
/// Both the `BagOfWordsEmbedder` (pure Rust, no ML) and any ML-backed
/// implementation satisfy this trait.
pub trait PackEmbedder: Send + Sync {
    /// Return a similarity score in `[0, 1]`.
    fn similarity(&self, query: &str, candidate: &str) -> f32;
}

// ---------------------------------------------------------------------------
// BagOfWordsEmbedder — pure Rust, no ML required
// ---------------------------------------------------------------------------

/// Jaccard-similarity bag-of-words embedder.
///
/// No ML dependencies.  Used in tests and as a baseline.  Production
/// accuracy is lower than the BGE model; see the evaluation harness for
/// measured numbers.
pub struct BagOfWordsEmbedder;

impl PackEmbedder for BagOfWordsEmbedder {
    fn similarity(&self, query: &str, candidate: &str) -> f32 {
        // Lower-case both sides so "KYC" matches "kyc", etc.
        let q_lower = query.to_lowercase();
        let c_lower = candidate.to_lowercase();
        let query_tokens: HashSet<&str> = q_lower.split_whitespace().collect();
        let cand_tokens: HashSet<&str> = c_lower.split_whitespace().collect();
        if query_tokens.is_empty() || cand_tokens.is_empty() {
            return 0.0;
        }
        let intersection = query_tokens.intersection(&cand_tokens).count() as f32;
        let union = query_tokens.union(&cand_tokens).count() as f32;
        intersection / union // Jaccard similarity
    }
}

// ---------------------------------------------------------------------------
// LLM client trait
// ---------------------------------------------------------------------------

/// Summary of a pack passed to the LLM ranking prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSummary {
    /// Pack name (e.g., `"conjunctive-gate"`).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Up to 3 example utterances shown to the LLM.
    pub example_utterances: Vec<String>,
}

/// A single entry in the LLM's ranked response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRankEntry {
    /// Pack name — must match one of the input [`PackSummary::name`] values.
    pub pack_name: String,
    /// Rank position (1 = best match).
    pub rank: usize,
    /// One-sentence rationale from the LLM.
    pub rationale: String,
}

/// Trait for the LLM ranking layer.
///
/// The real implementation (Tranche 4) will call the Anthropic API.
/// Tests use [`MockLlmClient`].
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Ask the LLM to rank `candidates` for `utterance`.
    ///
    /// Returns entries in rank order (rank 1 = best fit).  Missing packs
    /// (i.e., packs in `candidates` not mentioned in the response) are
    /// appended at the end with `rank = candidates.len()`.
    async fn rank_packs(
        &self,
        utterance: &str,
        candidates: &[PackSummary],
    ) -> Result<Vec<LlmRankEntry>>;
}

// ---------------------------------------------------------------------------
// Domain filter helper
// ---------------------------------------------------------------------------

/// De-rank packs whose `domain_scope` does not contain the context domain.
///
/// De-ranking (not removal) preserves recall when the context domain is
/// wrong or the utterance is cross-domain.
fn apply_domain_filter(
    mut ranked: Vec<(usize, f32)>,
    packs: &[&DecisionPack],
    context: &SageContext,
) -> Vec<(usize, f32)> {
    if let Some(domain) = &context.domain {
        for (pack_idx, score) in &mut ranked {
            let in_scope = packs[*pack_idx]
                .domain_scope
                .iter()
                .any(|d| d.eq_ignore_ascii_case(domain));
            if !in_scope {
                // Halve the score to de-rank without removing
                *score *= 0.5;
            }
        }
        // Re-sort after domain adjustment
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }
    ranked
}

// ---------------------------------------------------------------------------
// Scoring combination
// ---------------------------------------------------------------------------

/// Combine embedding score and LLM rank into a single `[0, 1]` confidence.
///
/// Scoring function (§1.5):
/// ```text
/// rank_score  = 1.0 − (llm_rank − 1) / n_candidates
/// confidence  = 0.5 * embedding_score + 0.5 * rank_score
/// ```
fn combine_scores(embedding_score: f32, llm_rank: usize, n_candidates: usize) -> f32 {
    let rank_score = 1.0 - (llm_rank as f32 - 1.0) / n_candidates.max(1) as f32;
    0.5 * embedding_score + 0.5 * rank_score
}

// ---------------------------------------------------------------------------
// Public sync entry point (embedding-only, no async, no LLM)
// ---------------------------------------------------------------------------

/// Match `utterance` against packs using embedding similarity only.
///
/// This is **synchronous** and requires no LLM API key.  It is used
/// in unit tests and as the inner layer of [`match_packs`].
pub fn match_packs_embedding_only(
    utterance: &str,
    context: &SageContext,
    registry: &PackRegistry,
    embedder: &dyn PackEmbedder,
) -> Vec<RankedCandidate> {
    let packs = registry.list_active();
    if packs.is_empty() {
        return vec![];
    }

    // Compute per-pack score: max similarity over example utterances
    let mut pack_scores: Vec<(usize, f32)> = packs
        .iter()
        .enumerate()
        .map(|(i, pack)| {
            let score = if pack.example_utterances.is_empty() {
                // Fall back to description similarity if no examples
                embedder.similarity(utterance, &pack.description)
            } else {
                pack.example_utterances
                    .iter()
                    .map(|ex| embedder.similarity(utterance, ex))
                    .fold(0.0_f32, f32::max)
            };
            (i, score)
        })
        .collect();

    // Sort descending by embedding score
    pack_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Keep top-K
    let k = 5.min(packs.len());
    pack_scores.truncate(k);

    // Apply optional domain filter
    let pack_scores = apply_domain_filter(pack_scores, &packs, context);

    // Build RankedCandidate list
    pack_scores
        .iter()
        .enumerate()
        .map(|(pos, (pack_idx, emb_score))| {
            let pack = packs[*pack_idx];
            let llm_rank = pos + 1;
            let confidence = combine_scores(*emb_score, llm_rank, pack_scores.len());
            RankedCandidate {
                pack_name: pack.name.clone(),
                pack_version: pack.version.clone(),
                confidence,
                rationale: format!(
                    "Embedding similarity {:.3} (rank {} of {})",
                    emb_score,
                    llm_rank,
                    pack_scores.len()
                ),
                embedding_score: *emb_score,
                llm_rank: Some(llm_rank),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Public async entry point (embedding + optional LLM ranking)
// ---------------------------------------------------------------------------

/// Match `utterance` against the pack registry, returning ranked candidates.
///
/// # Layers
///
/// 1. **Embedding retrieval** — always runs; uses `embedder`.
/// 2. **LLM ranking** — runs only when `llm_client` is `Some`.
///
/// When `llm_client` is `None` the result is embedding-only ranking.
/// This allows tests to run without an API key.
pub async fn match_packs(
    utterance: &str,
    context: &SageContext,
    registry: &PackRegistry,
    embedder: &dyn PackEmbedder,
    llm_client: Option<&dyn LlmClient>,
) -> Result<Vec<RankedCandidate>> {
    let packs = registry.list_active();
    if packs.is_empty() {
        return Ok(vec![]);
    }

    // Layer 1: embedding retrieval
    let mut pack_scores: Vec<(usize, f32)> = packs
        .iter()
        .enumerate()
        .map(|(i, pack)| {
            let score = if pack.example_utterances.is_empty() {
                embedder.similarity(utterance, &pack.description)
            } else {
                pack.example_utterances
                    .iter()
                    .map(|ex| embedder.similarity(utterance, ex))
                    .fold(0.0_f32, f32::max)
            };
            (i, score)
        })
        .collect();

    // Sort and keep top-K
    pack_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let k = 5.min(packs.len());
    pack_scores.truncate(k);

    // Domain filter
    let pack_scores = apply_domain_filter(pack_scores, &packs, context);

    // Layer 2: LLM ranking (if client provided)
    let ranked = if let Some(client) = llm_client {
        // Build summaries for top-K candidates
        let summaries: Vec<PackSummary> = pack_scores
            .iter()
            .map(|(pack_idx, _)| {
                let pack = packs[*pack_idx];
                PackSummary {
                    name: pack.name.clone(),
                    description: pack.description.clone(),
                    example_utterances: pack.example_utterances.iter().take(3).cloned().collect(),
                }
            })
            .collect();

        let llm_entries = client.rank_packs(utterance, &summaries).await?;

        // Map LLM entries back to (pack_idx, embedding_score, llm_rank)
        let n = pack_scores.len();
        let mut result: Vec<RankedCandidate> = Vec::with_capacity(n);

        for entry in &llm_entries {
            if let Some((pack_idx, emb_score)) = pack_scores
                .iter()
                .find(|(pi, _)| packs[*pi].name == entry.pack_name)
            {
                let emb_score = *emb_score;
                let confidence = combine_scores(emb_score, entry.rank, n);
                result.push(RankedCandidate {
                    pack_name: packs[*pack_idx].name.clone(),
                    pack_version: packs[*pack_idx].version.clone(),
                    confidence,
                    rationale: entry.rationale.clone(),
                    embedding_score: emb_score,
                    llm_rank: Some(entry.rank),
                });
            }
        }

        // Append any packs that the LLM didn't mention (defensive)
        for (pos, (pack_idx, emb_score)) in pack_scores.iter().enumerate() {
            let emb_score = *emb_score;
            let name = &packs[*pack_idx].name;
            if !result.iter().any(|r| &r.pack_name == name) {
                let llm_rank = llm_entries.len() + pos + 1;
                let confidence = combine_scores(emb_score, llm_rank, n);
                result.push(RankedCandidate {
                    pack_name: name.clone(),
                    pack_version: packs[*pack_idx].version.clone(),
                    confidence,
                    rationale: format!("Embedding fallback (not ranked by LLM): {:.3}", emb_score),
                    embedding_score: emb_score,
                    llm_rank: Some(llm_rank),
                });
            }
        }

        // Sort by confidence descending
        result.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        result
    } else {
        // Pure embedding path
        pack_scores
            .iter()
            .enumerate()
            .map(|(pos, (pack_idx, emb_score))| {
                let emb_score = *emb_score;
                let pack = packs[*pack_idx];
                let llm_rank = pos + 1;
                let confidence = combine_scores(emb_score, llm_rank, pack_scores.len());
                RankedCandidate {
                    pack_name: pack.name.clone(),
                    pack_version: pack.version.clone(),
                    confidence,
                    rationale: format!(
                        "Embedding similarity {:.3} (rank {} of {})",
                        emb_score,
                        llm_rank,
                        pack_scores.len()
                    ),
                    embedding_score: emb_score,
                    llm_rank: Some(llm_rank),
                }
            })
            .collect()
    };

    Ok(ranked)
}

// ---------------------------------------------------------------------------
// Mock LLM client (for tests)
// ---------------------------------------------------------------------------

/// Identity-ranking LLM client for tests.
///
/// Returns candidates in the same order as input, each with rank = position + 1
/// and rationale = `"mock"`.
#[cfg(any(test, feature = "test-util"))]
pub struct MockLlmClient;

#[cfg(any(test, feature = "test-util"))]
#[async_trait]
impl LlmClient for MockLlmClient {
    async fn rank_packs(
        &self,
        _utterance: &str,
        candidates: &[PackSummary],
    ) -> Result<Vec<LlmRankEntry>> {
        Ok(candidates
            .iter()
            .enumerate()
            .map(|(i, c)| LlmRankEntry {
                pack_name: c.name.clone(),
                rank: i + 1,
                rationale: "mock".to_string(),
            })
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bag_of_words_identical_strings() {
        let emb = BagOfWordsEmbedder;
        assert!((emb.similarity("foo bar", "foo bar") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bag_of_words_disjoint_strings() {
        let emb = BagOfWordsEmbedder;
        assert_eq!(emb.similarity("foo bar", "baz qux"), 0.0);
    }

    #[test]
    fn bag_of_words_partial_overlap() {
        let emb = BagOfWordsEmbedder;
        // "a b" vs "a c" → intersection={a}, union={a,b,c} → 1/3
        let s = emb.similarity("a b", "a c");
        assert!((s - 1.0 / 3.0).abs() < 1e-6, "got {s}");
    }

    #[test]
    fn bag_of_words_case_insensitive() {
        let emb = BagOfWordsEmbedder;
        assert!((emb.similarity("KYC approved", "kyc approved") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn combine_scores_rank1_max() {
        // rank 1 of 1 → rank_score = 1.0; combined = 0.5 * emb + 0.5
        let c = combine_scores(0.8, 1, 5);
        // rank_score = 1.0 - 0/5 = 1.0; confidence = 0.5*0.8 + 0.5*1.0 = 0.9
        assert!((c - 0.9).abs() < 1e-6, "got {c}");
    }

    #[test]
    fn combine_scores_last_rank() {
        // rank 5 of 5 → rank_score = 1.0 - 4/5 = 0.2
        let c = combine_scores(0.0, 5, 5);
        // confidence = 0.5*0.0 + 0.5*0.2 = 0.1
        assert!((c - 0.1).abs() < 1e-6, "got {c}");
    }
}

//! Hybrid Verb Search
//!
//! Combines multiple verb discovery strategies in priority order:
//! 1. Learned invocation phrases (from user corrections) - EXACT MATCH
//! 2. Exact phrase match from YAML invocation_phrases
//! 3. Substring phrase match
//! 4. Semantic embedding similarity (pgvector) - future enhancement
//!
//! Key insight: Learned phrases bypass semantic similarity entirely.
//! They're exact matches from real user vocabulary → your verbs.

use anyhow::Result;
use ob_agentic::lexicon::verb_phrases::VerbPhraseIndex;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;

use crate::agent::learning::warmup::SharedLearnedData;

/// A unified verb search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSearchResult {
    pub verb: String,
    pub score: f32,
    pub source: VerbSearchSource,
    pub matched_phrase: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbSearchSource {
    /// From user corrections (highest priority, exact match)
    Learned,
    /// Exact match from YAML invocation_phrases
    PhraseExact,
    /// Substring match from YAML
    PhraseSubstring,
    /// pgvector embedding similarity
    Semantic,
}

/// Hybrid verb searcher combining all discovery strategies
pub struct HybridVerbSearcher {
    phrase_index: Arc<VerbPhraseIndex>,
    learned_data: Option<SharedLearnedData>,
    #[allow(dead_code)]
    pool: Option<PgPool>, // For future semantic search integration
}

impl Clone for HybridVerbSearcher {
    fn clone(&self) -> Self {
        Self {
            phrase_index: Arc::clone(&self.phrase_index),
            learned_data: self.learned_data.clone(),
            pool: self.pool.clone(),
        }
    }
}

impl HybridVerbSearcher {
    /// Create searcher with phrase index only (no DB required)
    pub fn phrase_only(verbs_dir: &str) -> Result<Self> {
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir)?;
        Ok(Self {
            phrase_index: Arc::new(phrase_index),
            learned_data: None,
            pool: None,
        })
    }

    /// Create searcher with full capabilities
    ///
    /// Currently uses phrase matching + learned data.
    /// Semantic embeddings (pgvector) will be added in future enhancement.
    pub async fn full(
        verbs_dir: &str,
        pool: PgPool,
        learned_data: Option<SharedLearnedData>,
    ) -> Result<Self> {
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir)?;

        Ok(Self {
            phrase_index: Arc::new(phrase_index),
            learned_data,
            pool: Some(pool),
        })
    }

    /// Create searcher with learned data only (no semantic matcher)
    pub fn with_learned_data(verbs_dir: &str, learned_data: SharedLearnedData) -> Result<Self> {
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir)?;
        Ok(Self {
            phrase_index: Arc::new(phrase_index),
            learned_data: Some(learned_data),
            pool: None,
        })
    }

    /// Search for verbs matching user intent
    ///
    /// Priority order:
    /// 1. Learned phrases (from agent.invocation_phrases) - score 1.0
    /// 2. Exact phrase match from YAML - score 1.0
    /// 3. Substring phrase match - score 0.7-0.9
    /// 4. Semantic similarity - score 0.5-0.95
    pub async fn search(
        &self,
        query: &str,
        domain_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let mut results = Vec::new();
        let mut seen_verbs: HashSet<String> = HashSet::new();
        let normalized = query.trim().to_lowercase();

        // 1. Check LEARNED invocation phrases FIRST (highest priority)
        // These bypass all fuzzy matching - they're exact user vocabulary
        if let Some(learned) = &self.learned_data {
            let guard = learned.read().await;
            if let Some(verb) = guard.resolve_phrase(&normalized) {
                if self.matches_domain(verb, domain_filter) {
                    results.push(VerbSearchResult {
                        verb: verb.to_string(),
                        score: 1.0, // Perfect score - user taught us this
                        source: VerbSearchSource::Learned,
                        matched_phrase: query.to_string(),
                        description: self.get_verb_description(verb),
                    });
                    seen_verbs.insert(verb.to_string());
                }
            }
        }

        // 2. Phrase index (exact + substring from YAML)
        let phrase_matches = self.phrase_index.find_matches(query);
        for m in phrase_matches {
            if seen_verbs.contains(&m.fq_name) {
                continue;
            }
            if !self.matches_domain(&m.fq_name, domain_filter) {
                continue;
            }

            let source = if m.confidence >= 1.0 {
                VerbSearchSource::PhraseExact
            } else {
                VerbSearchSource::PhraseSubstring
            };

            results.push(VerbSearchResult {
                verb: m.fq_name.clone(),
                score: m.confidence,
                source,
                matched_phrase: m.matched_phrase,
                description: self.get_verb_description(&m.fq_name),
            });
            seen_verbs.insert(m.fq_name);
        }

        // 3. Semantic search (fallback for novel phrases) - TODO: re-enable with ob-semantic-matcher
        // Currently phrase-based matching is sufficient for most use cases.
        // Semantic embeddings will be added when the learning loop generates enough data.
        let _ = &seen_verbs; // Suppress unused warning

        // Sort by score descending, truncate
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    fn matches_domain(&self, verb: &str, filter: Option<&str>) -> bool {
        match filter {
            Some(d) => verb.starts_with(&format!("{}.", d)) || verb.starts_with(d),
            None => true,
        }
    }

    fn get_verb_description(&self, verb: &str) -> Option<String> {
        self.phrase_index
            .get_verb(verb)
            .map(|v| v.description.clone())
    }

    /// Get the phrase index for direct access
    pub fn phrase_index(&self) -> &VerbPhraseIndex {
        &self.phrase_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_phrase_only_search() {
        // This test requires config/verbs to exist
        let verbs_dir = "config/verbs";
        if !std::path::Path::new(verbs_dir).exists() {
            return;
        }

        let searcher = HybridVerbSearcher::phrase_only(verbs_dir).unwrap();
        let results = searcher.search("create cbu", None, 5).await.unwrap();

        // Should find some matches
        println!("Results for 'create cbu': {:?}", results);
    }
}

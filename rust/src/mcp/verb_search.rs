//! Hybrid Verb Search
//!
//! Combines multiple verb discovery strategies in priority order:
//! 1. User-specific learned phrases (exact match) - highest priority
//! 2. Global learned phrases (exact match)
//! 3. User-specific learned phrases (semantic match via pgvector)
//! 4. Global learned phrases (semantic match via pgvector)
//! 5. Blocklist check (semantic filtering)
//! 6. YAML invocation_phrases (exact/substring)
//! 7. Global semantic (cold start fallback)
//!
//! Key insight: Learned phrases become semantic anchors - one learned phrase
//! catches 5-10 paraphrases via embedding similarity.

use anyhow::Result;
use ob_agentic::lexicon::verb_phrases::VerbPhraseIndex;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::learning::embedder::SharedEmbedder;
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
    /// User-specific exact match (highest priority)
    UserLearnedExact,
    /// User-specific semantic match
    UserLearnedSemantic,
    /// Global learned exact match
    LearnedExact,
    /// Global learned semantic match
    LearnedSemantic,
    /// Exact match from YAML invocation_phrases
    PhraseExact,
    /// Substring match from YAML
    PhraseSubstring,
    /// pgvector embedding similarity (cold start)
    Semantic,
}

/// Hybrid verb searcher combining all discovery strategies
pub struct HybridVerbSearcher {
    phrase_index: Arc<VerbPhraseIndex>,
    learned_data: Option<SharedLearnedData>,
    pool: Option<PgPool>,
    embedder: Option<SharedEmbedder>,
    /// Similarity threshold for semantic matches (0.0-1.0)
    semantic_threshold: f32,
    /// Similarity threshold for blocklist matches
    blocklist_threshold: f32,
}

impl Clone for HybridVerbSearcher {
    fn clone(&self) -> Self {
        Self {
            phrase_index: Arc::clone(&self.phrase_index),
            learned_data: self.learned_data.clone(),
            pool: self.pool.clone(),
            embedder: self.embedder.clone(),
            semantic_threshold: self.semantic_threshold,
            blocklist_threshold: self.blocklist_threshold,
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
            embedder: None,
            semantic_threshold: 0.80,
            blocklist_threshold: 0.75,
        })
    }

    /// Create searcher with full capabilities including pgvector semantic search
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
            embedder: None, // Embedder added separately via with_embedder
            semantic_threshold: 0.80,
            blocklist_threshold: 0.75,
        })
    }

    /// Create searcher with learned data only (no semantic matching)
    pub fn with_learned_data(verbs_dir: &str, learned_data: SharedLearnedData) -> Result<Self> {
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir)?;
        Ok(Self {
            phrase_index: Arc::new(phrase_index),
            learned_data: Some(learned_data),
            pool: None,
            embedder: None,
            semantic_threshold: 0.80,
            blocklist_threshold: 0.75,
        })
    }

    /// Add embedder for semantic search capabilities
    pub fn with_embedder(mut self, embedder: SharedEmbedder) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Add database pool for semantic queries
    pub fn with_pool(mut self, pool: PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Set custom similarity threshold
    pub fn with_semantic_threshold(mut self, threshold: f32) -> Self {
        self.semantic_threshold = threshold;
        self
    }

    /// Search for verbs matching user intent
    ///
    /// Extended priority order with user-specific and semantic tiers:
    /// 1. User-specific learned (exact) - score 1.0
    /// 2. Global learned (exact) - score 1.0
    /// 3. User-specific learned (semantic) - score 0.8-0.99
    /// 4. Global learned (semantic) - score 0.8-0.99
    /// 5. Blocklist filter (rejects blocked verbs)
    /// 6. YAML phrase match - score 0.7-1.0
    /// 7. Global semantic (cold start) - score 0.5-0.95
    pub async fn search(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        domain_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let mut results = Vec::new();
        let mut seen_verbs: HashSet<String> = HashSet::new();
        let normalized = query.trim().to_lowercase();

        // 1. User-specific learned phrases (exact match)
        if let Some(uid) = user_id {
            if let Some(result) = self.search_user_learned_exact(uid, &normalized).await? {
                if self.matches_domain(&result.verb, domain_filter) {
                    seen_verbs.insert(result.verb.clone());
                    results.push(result);
                }
            }
        }

        // 2. Global learned phrases (exact match)
        if results.is_empty() {
            if let Some(learned) = &self.learned_data {
                let guard = learned.read().await;
                if let Some(verb) = guard.resolve_phrase(&normalized) {
                    if self.matches_domain(verb, domain_filter) {
                        results.push(VerbSearchResult {
                            verb: verb.to_string(),
                            score: 1.0,
                            source: VerbSearchSource::LearnedExact,
                            matched_phrase: query.to_string(),
                            description: self.get_verb_description(verb),
                        });
                        seen_verbs.insert(verb.to_string());
                    }
                }
            }
        }

        // 3. User-specific learned (SEMANTIC match)
        if results.is_empty() && user_id.is_some() && self.has_semantic_capability() {
            if let Some(result) = self
                .search_user_learned_semantic(user_id.unwrap(), query)
                .await?
            {
                if self.matches_domain(&result.verb, domain_filter)
                    && !seen_verbs.contains(&result.verb)
                {
                    seen_verbs.insert(result.verb.clone());
                    results.push(result);
                }
            }
        }

        // 4. Global learned (SEMANTIC match)
        if results.is_empty() && self.has_semantic_capability() {
            if let Some(result) = self.search_learned_semantic(query).await? {
                if self.matches_domain(&result.verb, domain_filter)
                    && !seen_verbs.contains(&result.verb)
                {
                    seen_verbs.insert(result.verb.clone());
                    results.push(result);
                }
            }
        }

        // 5. Blocklist check - remove blocked verbs from results
        if !results.is_empty() && self.has_semantic_capability() {
            let verb = &results[0].verb;
            if self.check_blocklist(query, user_id, verb).await? {
                tracing::info!(
                    query = query,
                    verb = verb,
                    "Verb blocked by blocklist, continuing search"
                );
                seen_verbs.insert(results.remove(0).verb);
            }
        }

        // 6. YAML phrase index (exact + substring)
        if results.len() < limit {
            let phrase_matches = self.phrase_index.find_matches(query);
            for m in phrase_matches {
                if seen_verbs.contains(&m.fq_name) {
                    continue;
                }
                if !self.matches_domain(&m.fq_name, domain_filter) {
                    continue;
                }

                // Check blocklist for this candidate too
                if self.has_semantic_capability()
                    && self.check_blocklist(query, user_id, &m.fq_name).await?
                {
                    seen_verbs.insert(m.fq_name.clone());
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

                if results.len() >= limit {
                    break;
                }
            }
        }

        // 7. Global semantic search (cold start fallback)
        // This uses the semantic_verb_patterns table for general verb matching
        if results.len() < limit && self.has_semantic_capability() {
            if let Ok(semantic_results) = self
                .search_global_semantic(query, limit - results.len())
                .await
            {
                for result in semantic_results {
                    if seen_verbs.contains(&result.verb) {
                        continue;
                    }
                    if !self.matches_domain(&result.verb, domain_filter) {
                        continue;
                    }
                    if self.check_blocklist(query, user_id, &result.verb).await? {
                        seen_verbs.insert(result.verb.clone());
                        continue;
                    }

                    seen_verbs.insert(result.verb.clone());
                    results.push(result);

                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        // Sort by score descending, truncate
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Backward-compatible search without user_id
    pub async fn search_simple(
        &self,
        query: &str,
        domain_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        self.search(query, None, domain_filter, limit).await
    }

    /// Check if semantic search is available
    fn has_semantic_capability(&self) -> bool {
        self.pool.is_some() && self.embedder.is_some()
    }

    /// Search user-specific learned phrases by exact match
    async fn search_user_learned_exact(
        &self,
        user_id: Uuid,
        phrase: &str,
    ) -> Result<Option<VerbSearchResult>> {
        let pool = match &self.pool {
            Some(p) => p,
            None => return Ok(None),
        };

        let row = sqlx::query_as::<_, (String, String, f32)>(
            r#"
            SELECT phrase, verb, confidence
            FROM agent.user_learned_phrases
            WHERE user_id = $1 AND LOWER(phrase) = $2
            "#,
        )
        .bind(user_id)
        .bind(phrase)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|(phrase, verb, confidence)| VerbSearchResult {
            verb: verb.clone(),
            score: confidence,
            source: VerbSearchSource::UserLearnedExact,
            matched_phrase: phrase,
            description: self.get_verb_description(&verb),
        }))
    }

    /// Search user-specific learned phrases by semantic similarity
    async fn search_user_learned_semantic(
        &self,
        user_id: Uuid,
        query: &str,
    ) -> Result<Option<VerbSearchResult>> {
        let (pool, embedder) = match (&self.pool, &self.embedder) {
            (Some(p), Some(e)) => (p, e),
            _ => return Ok(None),
        };

        let query_embedding = embedder.embed(query).await?;

        let row = sqlx::query_as::<_, (String, String, f32, f64)>(
            r#"
            SELECT phrase, verb, confidence, 1 - (embedding <=> $1::vector) as similarity
            FROM agent.user_learned_phrases
            WHERE user_id = $2
              AND embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
            "#,
        )
        .bind(&query_embedding)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((phrase, verb, confidence, similarity))
                if similarity as f32 > self.semantic_threshold =>
            {
                Ok(Some(VerbSearchResult {
                    verb: verb.clone(),
                    score: (similarity as f32) * confidence, // Combine similarity with confidence
                    source: VerbSearchSource::UserLearnedSemantic,
                    matched_phrase: phrase,
                    description: self.get_verb_description(&verb),
                }))
            }
            _ => Ok(None),
        }
    }

    /// Search global learned phrases by semantic similarity
    async fn search_learned_semantic(&self, query: &str) -> Result<Option<VerbSearchResult>> {
        let (pool, embedder) = match (&self.pool, &self.embedder) {
            (Some(p), Some(e)) => (p, e),
            _ => return Ok(None),
        };

        let query_embedding = embedder.embed(query).await?;

        let row = sqlx::query_as::<_, (String, String, f64)>(
            r#"
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
            FROM agent.invocation_phrases
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
            "#,
        )
        .bind(&query_embedding)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((phrase, verb, similarity)) if similarity as f32 > self.semantic_threshold => {
                Ok(Some(VerbSearchResult {
                    verb: verb.clone(),
                    score: similarity as f32,
                    source: VerbSearchSource::LearnedSemantic,
                    matched_phrase: phrase,
                    description: self.get_verb_description(&verb),
                }))
            }
            _ => Ok(None),
        }
    }

    /// Search global semantic verb patterns (cold start)
    async fn search_global_semantic(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let (pool, embedder) = match (&self.pool, &self.embedder) {
            (Some(p), Some(e)) => (p, e),
            _ => return Ok(Vec::new()),
        };

        let query_embedding = embedder.embed(query).await?;

        // Search semantic_verb_patterns table if it exists
        let rows = sqlx::query_as::<_, (String, String, f64)>(
            r#"
            SELECT pattern_phrase, verb_name, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".semantic_verb_patterns
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > 0.5
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(&query_embedding)
        .bind(limit as i32)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|(phrase, verb, similarity)| VerbSearchResult {
                verb: verb.clone(),
                score: similarity as f32,
                source: VerbSearchSource::Semantic,
                matched_phrase: phrase,
                description: self.get_verb_description(&verb),
            })
            .collect())
    }

    /// Check if a verb is blocked for this query (semantic match)
    async fn check_blocklist(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        verb: &str,
    ) -> Result<bool> {
        let (pool, embedder) = match (&self.pool, &self.embedder) {
            (Some(p), Some(e)) => (p, e),
            _ => return Ok(false),
        };

        let query_embedding = embedder.embed(query).await?;

        // Check if any blocklist entry matches semantically
        let blocked = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM agent.phrase_blocklist
                WHERE blocked_verb = $1
                  AND (user_id IS NULL OR user_id = $2)
                  AND (expires_at IS NULL OR expires_at > now())
                  AND embedding IS NOT NULL
                  AND 1 - (embedding <=> $3::vector) > $4
            )
            "#,
        )
        .bind(verb)
        .bind(user_id)
        .bind(&query_embedding)
        .bind(self.blocklist_threshold)
        .fetch_one(pool)
        .await?;

        Ok(blocked)
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
        let results = searcher.search_simple("create cbu", None, 5).await.unwrap();

        // Should find some matches
        println!("Results for 'create cbu': {:?}", results);
    }

    #[test]
    fn test_search_source_serialization() {
        let sources = vec![
            VerbSearchSource::UserLearnedExact,
            VerbSearchSource::UserLearnedSemantic,
            VerbSearchSource::LearnedExact,
            VerbSearchSource::LearnedSemantic,
            VerbSearchSource::PhraseExact,
            VerbSearchSource::PhraseSubstring,
            VerbSearchSource::Semantic,
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            println!("{:?} -> {}", source, json);
        }
    }
}

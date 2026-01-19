//! Hybrid Verb Search
//!
//! Combines multiple verb discovery strategies in priority order:
//! 1. User-specific learned phrases (exact match) - highest priority
//! 2. Global learned phrases (exact match)
//! 3. User-specific learned phrases (semantic match via pgvector)
//! 4. Global learned phrases (semantic match via pgvector)
//! 5. Blocklist check (semantic filtering)
//! 6. Global semantic via verb_pattern_embeddings - PRIMARY LOOKUP
//!
//! Architecture (DB as source of truth):
//!
//!   Pattern sources (two columns in dsl_verbs):
//!   - yaml_intent_patterns: from YAML invocation_phrases (overwritten on startup)
//!   - intent_patterns: learned from user feedback (preserved across restarts)
//!
//!   YAML invocation_phrases → VerbSyncService → dsl_verbs.yaml_intent_patterns
//!   Learning loop feedback  → PatternLearner  → dsl_verbs.intent_patterns
//!                                                       ↓
//!   v_verb_intent_patterns (UNION of both) → populate_embeddings binary
//!                                                       ↓
//!   verb_pattern_embeddings (Candle 384-dim vectors)
//!                                                       ↓
//!   HybridVerbSearcher.search_global_semantic() ← PRIMARY SEMANTIC LOOKUP
//!
//! All DB access goes through VerbService (no direct sqlx calls).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::learning::embedder::SharedEmbedder;
use crate::agent::learning::warmup::SharedLearnedData;
use crate::database::VerbService;

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
    /// pgvector embedding similarity (cold start)
    Semantic,
}

/// Hybrid verb searcher combining all discovery strategies
///
/// All DB access is through VerbService - no direct sqlx calls.
pub struct HybridVerbSearcher {
    verb_service: Option<Arc<VerbService>>,
    learned_data: Option<SharedLearnedData>,
    embedder: Option<SharedEmbedder>,
    /// Similarity threshold for semantic matches (0.0-1.0)
    semantic_threshold: f32,
    /// Similarity threshold for blocklist matches
    blocklist_threshold: f32,
}

impl Clone for HybridVerbSearcher {
    fn clone(&self) -> Self {
        Self {
            verb_service: self.verb_service.clone(),
            learned_data: self.learned_data.clone(),
            embedder: self.embedder.clone(),
            semantic_threshold: self.semantic_threshold,
            blocklist_threshold: self.blocklist_threshold,
        }
    }
}

impl HybridVerbSearcher {
    /// Create searcher with full capabilities including pgvector semantic search
    pub fn new(verb_service: Arc<VerbService>, learned_data: Option<SharedLearnedData>) -> Self {
        Self {
            verb_service: Some(verb_service),
            learned_data,
            embedder: None, // Embedder added separately via with_embedder
            semantic_threshold: 0.80,
            blocklist_threshold: 0.75,
        }
    }

    /// Create searcher with learned data only (no DB)
    pub fn with_learned_data_only(learned_data: SharedLearnedData) -> Self {
        Self {
            verb_service: None,
            learned_data: Some(learned_data),
            embedder: None,
            semantic_threshold: 0.80,
            blocklist_threshold: 0.75,
        }
    }

    /// Create minimal searcher (for testing)
    pub fn minimal() -> Self {
        Self {
            verb_service: None,
            learned_data: None,
            embedder: None,
            semantic_threshold: 0.80,
            blocklist_threshold: 0.75,
        }
    }

    /// Add embedder for semantic search capabilities
    pub fn with_embedder(mut self, embedder: SharedEmbedder) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Set custom similarity threshold
    pub fn with_semantic_threshold(mut self, threshold: f32) -> Self {
        self.semantic_threshold = threshold;
        self
    }

    /// Search for verbs matching user intent
    ///
    /// Priority order:
    /// 1. User-specific learned (exact) - score 1.0
    /// 2. Global learned (exact) - score 1.0
    /// 3. User-specific learned (semantic) - score 0.8-0.99
    /// 4. Global learned (semantic) - score 0.8-0.99
    /// 5. Blocklist filter (rejects blocked verbs)
    /// 6. Global semantic (cold start) - score 0.5-0.95
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
                        let description = self.get_verb_description(verb).await;
                        results.push(VerbSearchResult {
                            verb: verb.to_string(),
                            score: 1.0,
                            source: VerbSearchSource::LearnedExact,
                            matched_phrase: query.to_string(),
                            description,
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

        // 6. Global semantic search (cold start fallback)
        // Uses verb_pattern_embeddings for primary semantic lookup
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
        self.verb_service.is_some() && self.embedder.is_some()
    }

    /// Search user-specific learned phrases by exact match
    async fn search_user_learned_exact(
        &self,
        user_id: Uuid,
        phrase: &str,
    ) -> Result<Option<VerbSearchResult>> {
        let verb_service = match &self.verb_service {
            Some(s) => s,
            None => return Ok(None),
        };

        let result = verb_service
            .find_user_learned_exact(user_id, phrase)
            .await?;

        match result {
            Some(m) => {
                let description = self.get_verb_description(&m.verb).await;
                Ok(Some(VerbSearchResult {
                    verb: m.verb,
                    score: m.confidence,
                    source: VerbSearchSource::UserLearnedExact,
                    matched_phrase: m.phrase,
                    description,
                }))
            }
            None => Ok(None),
        }
    }

    /// Search user-specific learned phrases by semantic similarity
    async fn search_user_learned_semantic(
        &self,
        user_id: Uuid,
        query: &str,
    ) -> Result<Option<VerbSearchResult>> {
        let (verb_service, embedder) = match (&self.verb_service, &self.embedder) {
            (Some(s), Some(e)) => (s, e),
            _ => return Ok(None),
        };

        let query_embedding = embedder.embed(query).await?;

        let result = verb_service
            .find_user_learned_semantic(user_id, &query_embedding, self.semantic_threshold)
            .await?;

        match result {
            Some(m) => {
                let description = self.get_verb_description(&m.verb).await;
                let score = (m.similarity as f32) * m.confidence.unwrap_or(1.0);
                Ok(Some(VerbSearchResult {
                    verb: m.verb,
                    score,
                    source: VerbSearchSource::UserLearnedSemantic,
                    matched_phrase: m.phrase,
                    description,
                }))
            }
            None => Ok(None),
        }
    }

    /// Search global learned phrases by semantic similarity
    async fn search_learned_semantic(&self, query: &str) -> Result<Option<VerbSearchResult>> {
        let (verb_service, embedder) = match (&self.verb_service, &self.embedder) {
            (Some(s), Some(e)) => (s, e),
            _ => return Ok(None),
        };

        let query_embedding = embedder.embed(query).await?;

        let result = verb_service
            .find_global_learned_semantic(&query_embedding, self.semantic_threshold)
            .await?;

        match result {
            Some(m) => {
                let description = self.get_verb_description(&m.verb).await;
                Ok(Some(VerbSearchResult {
                    verb: m.verb,
                    score: m.similarity as f32,
                    source: VerbSearchSource::LearnedSemantic,
                    matched_phrase: m.phrase,
                    description,
                }))
            }
            None => Ok(None),
        }
    }

    /// Search global semantic verb patterns (cold start)
    ///
    /// Uses verb_pattern_embeddings table which is populated from dsl_verbs.intent_patterns
    /// by the populate_embeddings binary. This is the primary semantic lookup.
    async fn search_global_semantic(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let (verb_service, embedder) = match (&self.verb_service, &self.embedder) {
            (Some(s), Some(e)) => (s, e),
            _ => return Ok(Vec::new()),
        };

        let query_embedding = embedder.embed(query).await?;

        let matches = verb_service
            .search_verb_patterns_semantic(&query_embedding, limit, 0.5)
            .await
            .unwrap_or_default();

        let mut results = Vec::with_capacity(matches.len());
        for m in matches {
            let description = self.get_verb_description(&m.verb).await;
            results.push(VerbSearchResult {
                verb: m.verb,
                score: m.similarity as f32,
                source: VerbSearchSource::Semantic,
                matched_phrase: m.phrase,
                description,
            });
        }

        Ok(results)
    }

    /// Check if a verb is blocked for this query (semantic match)
    async fn check_blocklist(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        verb: &str,
    ) -> Result<bool> {
        let (verb_service, embedder) = match (&self.verb_service, &self.embedder) {
            (Some(s), Some(e)) => (s, e),
            _ => return Ok(false),
        };

        let query_embedding = embedder.embed(query).await?;

        let blocked = verb_service
            .check_blocklist(&query_embedding, user_id, verb, self.blocklist_threshold)
            .await?;

        Ok(blocked)
    }

    fn matches_domain(&self, verb: &str, filter: Option<&str>) -> bool {
        match filter {
            Some(d) => verb.starts_with(&format!("{}.", d)) || verb.starts_with(d),
            None => true,
        }
    }

    async fn get_verb_description(&self, verb: &str) -> Option<String> {
        match &self.verb_service {
            Some(s) => s.get_verb_description(verb).await.ok().flatten(),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_minimal_searcher() {
        let searcher = HybridVerbSearcher::minimal();
        let results = searcher.search_simple("create cbu", None, 5).await.unwrap();

        // Minimal searcher has no DB, so no results
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_source_serialization() {
        let sources = vec![
            VerbSearchSource::UserLearnedExact,
            VerbSearchSource::UserLearnedSemantic,
            VerbSearchSource::LearnedExact,
            VerbSearchSource::LearnedSemantic,
            VerbSearchSource::Semantic,
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            println!("{:?} -> {}", source, json);
        }
    }
}

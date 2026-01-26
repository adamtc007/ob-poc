//! Hybrid Verb Search
//!
//! Combines multiple verb discovery strategies in priority order:
//! 1. User-specific learned phrases (exact match) - highest priority
//! 2. Global learned phrases (exact match)
//! 3. User-specific learned phrases (semantic match via pgvector, top-k)
//! 4. [REMOVED] - was redundant with step 6, see Issue I/J
//! 5. Blocklist check (semantic filtering)
//! 6. Global semantic - UNION of learned + cold-start patterns (top-k)
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
//!   verb_pattern_embeddings (BGE-small-en-v1.5 384-dim vectors)
//!                                                       ↓
//!   HybridVerbSearcher.search_global_semantic() ← PRIMARY SEMANTIC LOOKUP
//!
//! All DB access goes through VerbService (no direct sqlx calls).
//!
//! ## Threshold Calibration (BGE Asymmetric Mode)
//!
//! BGE uses asymmetric retrieval: queries get an instruction prefix, targets don't.
//! This produces LOWER similarity scores than symmetric (target→target) mode:
//! - Symmetric (target→target): scores 0.6-1.0 (same-embedding comparison)
//! - Asymmetric (query→target): scores 0.5-0.8 (instruction-prefixed query vs raw target)
//!
//! Thresholds for BGE asymmetric mode:
//! - fallback_threshold: 0.55 (retrieval cutoff - must retrieve candidates)
//! - semantic_threshold: 0.65 (decision gate - accepting a match)
//! - blocklist_threshold: 0.80 (collision detection)

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
    /// Direct DSL input (user typed DSL directly)
    DirectDsl,
    /// Global learned via invocation_phrases (Issue I distinction)
    GlobalLearned,
    /// Cold start pattern embeddings (Issue I distinction)
    PatternEmbedding,
}

// ============================================================================
// Ambiguity Detection (Issue D/J)
// ============================================================================

/// Margin threshold for ambiguity detection
/// If top two candidates are within this margin, flag as ambiguous
pub const AMBIGUITY_MARGIN: f32 = 0.05;

/// Outcome of verb search with ambiguity detection (Issue D/J)
#[derive(Debug, Clone)]
pub enum VerbSearchOutcome {
    /// Clear winner - proceed with LLM extraction
    Matched(VerbSearchResult),
    /// Top candidates too close - need user clarification
    Ambiguous {
        top: VerbSearchResult,
        runner_up: VerbSearchResult,
        margin: f32,
    },
    /// Nothing matched threshold
    NoMatch,
}

/// Check for ambiguity in search results (Issue D/J)
///
/// Ambiguity rule: If top >= threshold AND runner_up >= threshold
/// AND (top.score - runner_up.score) < AMBIGUITY_MARGIN, flag ambiguous.
///
/// IMPORTANT: Run this AFTER union+dedupe+sort (Issue I), so margin
/// reflects true best alternatives across all semantic sources.
pub fn check_ambiguity(candidates: &[VerbSearchResult], threshold: f32) -> VerbSearchOutcome {
    match candidates.first() {
        None => VerbSearchOutcome::NoMatch,
        Some(top) if top.score < threshold => VerbSearchOutcome::NoMatch,
        Some(top) => match candidates.get(1) {
            // Only one candidate above threshold
            None => VerbSearchOutcome::Matched(top.clone()),
            Some(runner_up) if runner_up.score < threshold => {
                // Runner-up below threshold - clear winner
                VerbSearchOutcome::Matched(top.clone())
            }
            Some(runner_up) => {
                let margin = top.score - runner_up.score;
                if margin < AMBIGUITY_MARGIN {
                    VerbSearchOutcome::Ambiguous {
                        top: top.clone(),
                        runner_up: runner_up.clone(),
                        margin,
                    }
                } else {
                    VerbSearchOutcome::Matched(top.clone())
                }
            }
        },
    }
}

/// Normalize candidate list: dedupe by verb (keep highest score), sort desc, truncate
///
/// Essential for J/D correctness — candidates are appended tier-by-tier during search,
/// so without this, candidates[0] is not guaranteed to be the best match.
///
/// INVARIANT: When same verb appears from multiple sources (trie, semantic, phonetic),
/// we keep the BEST score, not first-seen. This ensures the final ranking reflects
/// true match quality across all discovery methods.
pub fn normalize_candidates(
    mut results: Vec<VerbSearchResult>,
    limit: usize,
) -> Vec<VerbSearchResult> {
    use std::collections::HashMap;

    // Deduplicate by verb (keep highest score; preserve best metadata)
    let mut by_verb: HashMap<String, VerbSearchResult> = HashMap::new();
    for r in results.drain(..) {
        by_verb
            .entry(r.verb.clone())
            .and_modify(|existing| {
                if r.score > existing.score {
                    *existing = r.clone();
                }
            })
            .or_insert(r);
    }

    let mut v: Vec<VerbSearchResult> = by_verb.into_values().collect();
    v.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    v.truncate(limit);
    v
}

/// Hybrid verb searcher combining all discovery strategies
///
/// All DB access is through VerbService - no direct sqlx calls.
pub struct HybridVerbSearcher {
    verb_service: Option<Arc<VerbService>>,
    learned_data: Option<SharedLearnedData>,
    embedder: Option<SharedEmbedder>,
    /// Similarity threshold for learned semantic matches (high confidence, 0.80)
    semantic_threshold: f32,
    /// Similarity threshold for cold start / fallback semantic matches (0.65)
    fallback_threshold: f32,
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
            fallback_threshold: self.fallback_threshold,
            blocklist_threshold: self.blocklist_threshold,
        }
    }
}

impl HybridVerbSearcher {
    /// Create searcher with full capabilities including pgvector semantic search
    ///
    /// Thresholds calibrated for BGE-small-en-v1.5 (retrieval-optimized model).
    /// BGE asymmetric mode: query embeddings use instruction prefix, targets don't.
    /// This produces LOWER similarity scores than symmetric (target-to-target) mode.
    /// Thresholds must be set accordingly:
    /// - fallback_threshold: 0.55 (retrieval cutoff - must be low enough to retrieve candidates)
    /// - semantic_threshold: 0.65 (decision gate - accepting a match)
    pub fn new(verb_service: Arc<VerbService>, learned_data: Option<SharedLearnedData>) -> Self {
        Self {
            verb_service: Some(verb_service),
            learned_data,
            embedder: None, // Embedder added separately via with_embedder
            // BGE asymmetric mode thresholds (query→target is lower than target→target)
            semantic_threshold: 0.65,  // Decision gate for accepting match
            fallback_threshold: 0.55,  // Retrieval cutoff for DB queries
            blocklist_threshold: 0.80, // Collision detection
        }
    }

    /// Create searcher with learned data only (no DB)
    pub fn with_learned_data_only(learned_data: SharedLearnedData) -> Self {
        Self {
            verb_service: None,
            learned_data: Some(learned_data),
            embedder: None,
            // BGE asymmetric mode thresholds
            semantic_threshold: 0.65,
            fallback_threshold: 0.55,
            blocklist_threshold: 0.80,
        }
    }

    /// Create minimal searcher (for testing)
    pub fn minimal() -> Self {
        Self {
            verb_service: None,
            learned_data: None,
            embedder: None,
            // BGE asymmetric mode thresholds
            semantic_threshold: 0.65,
            fallback_threshold: 0.55,
            blocklist_threshold: 0.80,
        }
    }

    /// Add embedder for semantic search capabilities
    pub fn with_embedder(mut self, embedder: SharedEmbedder) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Set custom semantic threshold (decision gate for top match)
    pub fn with_semantic_threshold(mut self, threshold: f32) -> Self {
        self.semantic_threshold = threshold;
        self
    }

    /// Set custom fallback threshold (retrieval cutoff for DB queries)
    pub fn with_fallback_threshold(mut self, threshold: f32) -> Self {
        self.fallback_threshold = threshold;
        self
    }

    /// Get the semantic threshold (for ambiguity checks in IntentPipeline)
    pub fn semantic_threshold(&self) -> f32 {
        self.semantic_threshold
    }

    /// Get the fallback threshold (retrieval cutoff)
    pub fn fallback_threshold(&self) -> f32 {
        self.fallback_threshold
    }

    /// Check if semantic search is available (embedder configured)
    pub fn has_semantic_search(&self) -> bool {
        self.embedder.is_some()
    }

    /// Search for verbs matching user intent
    ///
    /// Priority order:
    /// 1. User-specific learned (exact) - score 1.0
    /// 2. Global learned (exact) - score 1.0
    /// 3. User-specific learned (semantic) - score 0.8-0.99
    /// 4. Global learned (semantic) - score 0.8-0.99
    /// 5. Blocklist filter (rejects blocked verbs)
    /// 6. Global semantic (cold start) - score fallback_threshold-0.95
    pub async fn search(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        domain_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let mut results = Vec::new();
        let mut seen_verbs: HashSet<String> = HashSet::new();

        // Normalize query ONCE at the start (used for exact matching)
        let normalized = query.trim().to_lowercase();

        // Debug: Log semantic capability status
        tracing::debug!(
            has_verb_service = self.verb_service.is_some(),
            has_embedder = self.embedder.is_some(),
            has_semantic = self.has_semantic_capability(),
            query = %query,
            "VerbSearch: checking semantic capability"
        );

        // Compute embedding ONCE at the start (used for all semantic lookups)
        // This avoids computing the same embedding 4 times (user semantic, learned semantic,
        // global semantic, blocklist) - saves ~15-30ms per search
        // Use embed_query for user input (applies BGE instruction prefix)
        let query_embedding: Option<Vec<f32>> = if self.has_semantic_capability() {
            tracing::debug!("VerbSearch: computing query embedding...");
            match self.embedder.as_ref().unwrap().embed_query(query).await {
                Ok(emb) => {
                    tracing::debug!(
                        embedding_len = emb.len(),
                        "VerbSearch: embedding computed successfully"
                    );
                    Some(emb)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to compute query embedding, falling back to exact matches only");
                    None
                }
            }
        } else {
            tracing::warn!(
                "VerbSearch: semantic capability NOT available, falling back to exact matches only"
            );
            None
        };

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

        // 3. User-specific learned (SEMANTIC match) - top-k for ambiguity detection
        if results.is_empty() && user_id.is_some() {
            if let Some(ref embedding) = query_embedding {
                let user_results = self
                    .search_user_learned_semantic_with_embedding(user_id.unwrap(), embedding, 3)
                    .await?;
                for result in user_results {
                    if self.matches_domain(&result.verb, domain_filter)
                        && !seen_verbs.contains(&result.verb)
                    {
                        seen_verbs.insert(result.verb.clone());
                        results.push(result);
                    }
                }
            }
        }

        // 4. [REMOVED] Global learned semantic was redundant with step 6
        //    Step 6 fetches from BOTH sources (learned + cold start) via union.
        //    Keeping a separate LIMIT 1 learned lookup here would:
        //    - Block consultation of cold-start patterns if learned had a mediocre 0.81 match
        //    - Prevent ambiguity detection across sources
        //    See Issue I in intent-pipeline-fixes-todo.md for rationale.

        // 5. Blocklist check - remove blocked verbs from results
        if !results.is_empty() {
            if let Some(ref embedding) = query_embedding {
                let verb = &results[0].verb;
                if self
                    .check_blocklist_with_embedding(embedding, user_id, verb)
                    .await?
                {
                    tracing::info!(
                        query = query,
                        verb = verb,
                        "Verb blocked by blocklist, continuing search"
                    );
                    seen_verbs.insert(results.remove(0).verb);
                }
            }
        }

        // 6. Global semantic search (cold start fallback)
        // Uses verb_pattern_embeddings for primary semantic lookup
        if results.len() < limit {
            tracing::debug!(
                results_so_far = results.len(),
                has_embedding = query_embedding.is_some(),
                "VerbSearch: checking global semantic search"
            );
            if let Some(ref embedding) = query_embedding {
                tracing::debug!("VerbSearch: calling search_global_semantic_with_embedding...");
                match self
                    .search_global_semantic_with_embedding(embedding, limit - results.len())
                    .await
                {
                    Ok(semantic_results) => {
                        tracing::debug!(
                            semantic_results_count = semantic_results.len(),
                            "VerbSearch: global semantic search returned"
                        );
                        for result in semantic_results {
                            if seen_verbs.contains(&result.verb) {
                                continue;
                            }
                            if !self.matches_domain(&result.verb, domain_filter) {
                                continue;
                            }
                            if self
                                .check_blocklist_with_embedding(embedding, user_id, &result.verb)
                                .await?
                            {
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
                    Err(e) => {
                        tracing::warn!(error = %e, "VerbSearch: global semantic search failed");
                    }
                }
            }
        }

        // Dedupe by verb, sort by score descending, truncate (Issue J/D fix)
        let mut results = normalize_candidates(results, limit);

        // Final blocklist filter across entire candidate list (ChatGPT review feedback)
        // Earlier checks only filtered per-tier; this ensures no blocked verbs slip through
        if let Some(ref embedding) = query_embedding {
            let mut blocked_verbs = Vec::new();
            for result in &results {
                if self
                    .check_blocklist_with_embedding(embedding, user_id, &result.verb)
                    .await
                    .unwrap_or(false)
                {
                    blocked_verbs.push(result.verb.clone());
                }
            }
            if !blocked_verbs.is_empty() {
                results.retain(|r| !blocked_verbs.contains(&r.verb));
            }
        }

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

    /// Search user-specific learned phrases by semantic similarity (top-k)
    ///
    /// Takes pre-computed embedding to avoid redundant computation.
    /// Returns top-k results for ambiguity detection.
    async fn search_user_learned_semantic_with_embedding(
        &self,
        user_id: Uuid,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let verb_service = match &self.verb_service {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        let matches = verb_service
            .find_user_learned_semantic_topk(
                user_id,
                query_embedding,
                self.semantic_threshold,
                limit,
            )
            .await?;

        let mut results = Vec::with_capacity(matches.len());
        for m in matches {
            let description = self.get_verb_description(&m.verb).await;
            let score = (m.similarity as f32) * m.confidence.unwrap_or(1.0);
            results.push(VerbSearchResult {
                verb: m.verb,
                score,
                source: VerbSearchSource::UserLearnedSemantic,
                matched_phrase: m.phrase,
                description,
            });
        }
        Ok(results)
    }

    // NOTE: search_learned_semantic_with_embedding was REMOVED (Issue I/J).
    // It was redundant with search_global_semantic_with_embedding which
    // fetches from BOTH learned + cold-start sources via union.
    // See step 4 comment in search() for rationale.

    /// Search global semantic verb patterns (cold start)
    ///
    /// Uses verb_pattern_embeddings table which is populated from v_verb_intent_patterns
    /// (UNION of yaml_intent_patterns + intent_patterns) by populate_embeddings binary.
    /// Global semantic search - union of learned phrases + cold start patterns (Issue I)
    ///
    /// Takes pre-computed embedding to avoid redundant computation.
    /// Uses `fallback_threshold` (0.65) instead of hardcoded 0.5.
    ///
    /// Issue I fix: Fetches from BOTH sources:
    /// 1. agent.invocation_phrases (learned)
    /// 2. ob-poc.verb_pattern_embeddings (cold start)
    ///
    /// Then unions and dedupes by verb, keeping highest score.
    async fn search_global_semantic_with_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        use std::collections::HashMap;

        let verb_service = match &self.verb_service {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        // Fetch top-k from BOTH sources (Issue I)
        let learned_matches = verb_service
            .find_global_learned_semantic_topk(query_embedding, self.fallback_threshold, limit)
            .await
            .unwrap_or_default();

        let pattern_matches = verb_service
            .search_verb_patterns_semantic(query_embedding, limit, self.fallback_threshold)
            .await
            .unwrap_or_default();

        // Convert to VerbSearchResult with source metadata
        let learned_results: Vec<VerbSearchResult> = learned_matches
            .into_iter()
            .map(|m| VerbSearchResult {
                verb: m.verb,
                score: m.similarity as f32,
                source: VerbSearchSource::GlobalLearned,
                matched_phrase: m.phrase,
                description: None,
            })
            .collect();

        let pattern_results: Vec<VerbSearchResult> = pattern_matches
            .into_iter()
            .map(|m| VerbSearchResult {
                verb: m.verb,
                score: m.similarity as f32,
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: m.phrase,
                description: None,
            })
            .collect();

        // Union and dedupe by verb, keeping highest score (Issue I)
        let mut combined: HashMap<String, VerbSearchResult> = HashMap::new();
        for result in learned_results.into_iter().chain(pattern_results) {
            combined
                .entry(result.verb.clone())
                .and_modify(|existing| {
                    if result.score > existing.score {
                        *existing = result.clone();
                    }
                })
                .or_insert(result);
        }

        // Sort by score descending
        let mut sorted: Vec<VerbSearchResult> = combined.into_values().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(limit);

        // Add descriptions
        for result in &mut sorted {
            result.description = self.get_verb_description(&result.verb).await;
        }

        Ok(sorted)
    }

    /// Check if a verb is blocked for this query (semantic match)
    ///
    /// Takes pre-computed embedding to avoid redundant computation.
    async fn check_blocklist_with_embedding(
        &self,
        query_embedding: &[f32],
        user_id: Option<Uuid>,
        verb: &str,
    ) -> Result<bool> {
        let verb_service = match &self.verb_service {
            Some(s) => s,
            None => return Ok(false),
        };

        let blocked = verb_service
            .check_blocklist(query_embedding, user_id, verb, self.blocklist_threshold)
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

    // =========================================================================
    // Issue J/D Acceptance Tests - Ambiguity Detection
    // =========================================================================

    #[test]
    fn test_normalize_candidates_dedupes_and_sorts() {
        // Simulate tier-by-tier appending: same verb appears twice with different scores
        let candidates = vec![
            VerbSearchResult {
                verb: "cbu.create".to_string(),
                score: 0.82, // lower score, added first (tier 3)
                source: VerbSearchSource::LearnedSemantic,
                matched_phrase: "make a cbu".to_string(),
                description: None,
            },
            VerbSearchResult {
                verb: "cbu.ensure".to_string(),
                score: 0.80,
                source: VerbSearchSource::Semantic,
                matched_phrase: "ensure cbu".to_string(),
                description: None,
            },
            VerbSearchResult {
                verb: "cbu.create".to_string(),
                score: 0.91, // higher score, added later (tier 6)
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: "create cbu".to_string(),
                description: None,
            },
        ];

        let normalized = normalize_candidates(candidates, 5);

        // Should have 2 unique verbs
        assert_eq!(normalized.len(), 2);

        // First should be cbu.create with the HIGHER score (0.91)
        assert_eq!(normalized[0].verb, "cbu.create");
        assert!((normalized[0].score - 0.91).abs() < 0.001);
        assert!(matches!(
            normalized[0].source,
            VerbSearchSource::PatternEmbedding
        ));

        // Second should be cbu.ensure
        assert_eq!(normalized[1].verb, "cbu.ensure");
    }

    #[test]
    fn test_check_ambiguity_blocks_on_close_margin() {
        let threshold = 0.80;

        // Two candidates within margin, both above threshold
        let candidates = vec![
            VerbSearchResult {
                verb: "cbu.create".to_string(),
                score: 0.85,
                source: VerbSearchSource::Semantic,
                matched_phrase: "create cbu".to_string(),
                description: None,
            },
            VerbSearchResult {
                verb: "cbu.ensure".to_string(),
                score: 0.83, // margin = 0.02 < AMBIGUITY_MARGIN (0.05)
                source: VerbSearchSource::Semantic,
                matched_phrase: "ensure cbu".to_string(),
                description: None,
            },
        ];

        let outcome = check_ambiguity(&candidates, threshold);

        match outcome {
            VerbSearchOutcome::Ambiguous {
                top,
                runner_up,
                margin,
            } => {
                assert_eq!(top.verb, "cbu.create");
                assert_eq!(runner_up.verb, "cbu.ensure");
                assert!(
                    margin < AMBIGUITY_MARGIN,
                    "margin {} should be < {}",
                    margin,
                    AMBIGUITY_MARGIN
                );
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    #[test]
    fn test_check_ambiguity_passes_on_clear_winner() {
        let threshold = 0.80;

        // Clear winner - margin > AMBIGUITY_MARGIN
        let candidates = vec![
            VerbSearchResult {
                verb: "cbu.create".to_string(),
                score: 0.92,
                source: VerbSearchSource::Semantic,
                matched_phrase: "create cbu".to_string(),
                description: None,
            },
            VerbSearchResult {
                verb: "cbu.ensure".to_string(),
                score: 0.82, // margin = 0.10 > AMBIGUITY_MARGIN (0.05)
                source: VerbSearchSource::Semantic,
                matched_phrase: "ensure cbu".to_string(),
                description: None,
            },
        ];

        let outcome = check_ambiguity(&candidates, threshold);

        match outcome {
            VerbSearchOutcome::Matched(result) => {
                assert_eq!(result.verb, "cbu.create");
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_check_ambiguity_no_match_below_threshold() {
        let threshold = 0.80;

        // All candidates below threshold
        let candidates = vec![VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.75, // below threshold
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        }];

        let outcome = check_ambiguity(&candidates, threshold);

        assert!(matches!(outcome, VerbSearchOutcome::NoMatch));
    }

    #[test]
    fn test_check_ambiguity_empty_candidates() {
        let threshold = 0.80;
        let candidates: Vec<VerbSearchResult> = vec![];

        let outcome = check_ambiguity(&candidates, threshold);

        assert!(matches!(outcome, VerbSearchOutcome::NoMatch));
    }

    #[test]
    fn test_check_ambiguity_single_candidate_above_threshold() {
        let threshold = 0.80;

        let candidates = vec![VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.90,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        }];

        let outcome = check_ambiguity(&candidates, threshold);

        match outcome {
            VerbSearchOutcome::Matched(result) => {
                assert_eq!(result.verb, "cbu.create");
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }
}

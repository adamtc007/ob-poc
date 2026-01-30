//! Hybrid Verb Search
//!
//! Combines multiple verb discovery strategies in priority order:
//! 0. Operator macros (business vocabulary) - highest priority for UI verb picker
//! 1. User-specific learned phrases (exact match)
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
use crate::macros::OperatorMacroRegistry;

/// A unified verb search result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerbSearchResult {
    pub verb: String,
    pub score: f32,
    pub source: VerbSearchSource,
    pub matched_phrase: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(PartialEq)]
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
    /// Phonetic (dmetaphone) match - fallback for typos
    Phonetic,
    /// Operator macro match (business vocabulary layer)
    Macro,
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
    /// Low confidence but has suggestions - offer menu for user to select
    /// This is the "Suggest" path for queries between fallback_threshold and semantic_threshold
    /// Critical for learning: vague queries get a menu instead of "no match"
    Suggest {
        /// Top candidates to suggest (ordered by score)
        candidates: Vec<VerbSearchResult>,
    },
    /// Nothing matched even fallback threshold
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
    // Use default fallback threshold if not specified
    check_ambiguity_with_fallback(candidates, threshold, DEFAULT_FALLBACK_THRESHOLD)
}

/// Default fallback threshold for suggestions
const DEFAULT_FALLBACK_THRESHOLD: f32 = 0.55;

/// Check for ambiguity with explicit fallback threshold for suggestions
///
/// Returns:
/// - Matched: top >= threshold AND clear winner (margin > AMBIGUITY_MARGIN)
/// - Ambiguous: top >= threshold AND runner_up close (margin <= AMBIGUITY_MARGIN)
/// - Suggest: top < threshold BUT top >= fallback_threshold (has suggestions)
/// - NoMatch: top < fallback_threshold (nothing useful)
///
/// The Suggest path is CRITICAL for learning: vague queries like "show me the cbus"
/// get a menu instead of "no match", allowing user selection to create training data.
pub fn check_ambiguity_with_fallback(
    candidates: &[VerbSearchResult],
    threshold: f32,
    fallback_threshold: f32,
) -> VerbSearchOutcome {
    match candidates.first() {
        None => VerbSearchOutcome::NoMatch,
        Some(top) if top.score < fallback_threshold => {
            // Below even fallback - nothing useful
            VerbSearchOutcome::NoMatch
        }
        Some(top) if top.score < threshold => {
            // Between fallback and semantic threshold - SUGGEST path
            // This is where we capture "vague but close" queries for learning
            let suggestion_candidates: Vec<VerbSearchResult> = candidates
                .iter()
                .filter(|c| c.score >= fallback_threshold)
                .take(5)
                .cloned()
                .collect();
            VerbSearchOutcome::Suggest {
                candidates: suggestion_candidates,
            }
        }
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
///
/// ## Two-Stage Centroid Search (Optimization)
///
/// When centroids are available (verb_centroids table populated), the global semantic
/// search uses a two-stage approach:
/// 1. Query ~500 centroids to get top-25 candidate verbs (fast, stable)
/// 2. Refine with pattern-level matches within shortlist (precise, evidenced)
/// 3. Combine scores: `0.4 * centroid + 0.6 * pattern` (tunable)
///
/// This reduces variance from noisy individual phrases and provides larger score gaps.
pub struct HybridVerbSearcher {
    verb_service: Option<Arc<VerbService>>,
    learned_data: Option<SharedLearnedData>,
    embedder: Option<SharedEmbedder>,
    /// Operator macro registry for business vocabulary search
    macro_registry: Option<Arc<OperatorMacroRegistry>>,
    /// Similarity threshold for learned semantic matches (high confidence, 0.80)
    semantic_threshold: f32,
    /// Similarity threshold for cold start / fallback semantic matches (0.65)
    fallback_threshold: f32,
    /// Similarity threshold for blocklist matches
    blocklist_threshold: f32,
    /// Use centroid-based two-stage search (default: true if centroids available)
    use_centroids: bool,
    /// Centroid shortlist size (default: 25)
    centroid_shortlist_size: i32,
    /// Weight for centroid score in combined scoring (default: 0.4)
    centroid_weight: f32,
    /// Weight for pattern score in combined scoring (default: 0.6)
    pattern_weight: f32,
}

impl Clone for HybridVerbSearcher {
    fn clone(&self) -> Self {
        Self {
            verb_service: self.verb_service.clone(),
            learned_data: self.learned_data.clone(),
            embedder: self.embedder.clone(),
            macro_registry: self.macro_registry.clone(),
            semantic_threshold: self.semantic_threshold,
            fallback_threshold: self.fallback_threshold,
            blocklist_threshold: self.blocklist_threshold,
            use_centroids: self.use_centroids,
            centroid_shortlist_size: self.centroid_shortlist_size,
            centroid_weight: self.centroid_weight,
            pattern_weight: self.pattern_weight,
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
            embedder: None,       // Embedder added separately via with_embedder
            macro_registry: None, // Macro registry added separately via with_macro_registry
            // BGE asymmetric mode thresholds (query→target is lower than target→target)
            semantic_threshold: 0.65,  // Decision gate for accepting match
            fallback_threshold: 0.55,  // Retrieval cutoff for DB queries
            blocklist_threshold: 0.80, // Collision detection
            // Centroid optimization (DISABLED - Option A caused regression 79.2% → 77.4%)
            // Infrastructure ready, but adding centroid candidates hurts more than helps
            use_centroids: false,
            centroid_shortlist_size: 25,
            centroid_weight: 0.4,
            pattern_weight: 0.6,
        }
    }

    /// Create searcher with learned data only (no DB)
    pub fn with_learned_data_only(learned_data: SharedLearnedData) -> Self {
        Self {
            verb_service: None,
            learned_data: Some(learned_data),
            embedder: None,
            macro_registry: None,
            // BGE asymmetric mode thresholds
            semantic_threshold: 0.65,
            fallback_threshold: 0.55,
            blocklist_threshold: 0.80,
            // Centroid optimization (disabled without DB)
            use_centroids: false,
            centroid_shortlist_size: 25,
            centroid_weight: 0.4,
            pattern_weight: 0.6,
        }
    }

    /// Create minimal searcher (for testing)
    pub fn minimal() -> Self {
        Self {
            verb_service: None,
            learned_data: None,
            embedder: None,
            macro_registry: None,
            // BGE asymmetric mode thresholds
            semantic_threshold: 0.65,
            fallback_threshold: 0.55,
            blocklist_threshold: 0.80,
            // Centroid optimization (disabled without DB)
            use_centroids: false,
            centroid_shortlist_size: 25,
            centroid_weight: 0.4,
            pattern_weight: 0.6,
        }
    }

    /// Add embedder for semantic search capabilities
    pub fn with_embedder(mut self, embedder: SharedEmbedder) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Add macro registry for operator vocabulary search
    pub fn with_macro_registry(mut self, registry: Arc<OperatorMacroRegistry>) -> Self {
        self.macro_registry = Some(registry);
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

    /// Enable or disable centroid-based two-stage search
    pub fn with_centroids(mut self, enabled: bool) -> Self {
        self.use_centroids = enabled;
        self
    }

    /// Set centroid shortlist size (how many verbs to consider in stage 1)
    pub fn with_centroid_shortlist_size(mut self, size: i32) -> Self {
        self.centroid_shortlist_size = size;
        self
    }

    /// Set centroid vs pattern score weights (must sum to 1.0)
    pub fn with_centroid_weights(mut self, centroid_weight: f32, pattern_weight: f32) -> Self {
        self.centroid_weight = centroid_weight;
        self.pattern_weight = pattern_weight;
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
    /// 0. Operator macros (business vocabulary) - score 1.0 exact, 0.95 fuzzy
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

        // 0. Operator macro search (business vocabulary layer - HIGHEST PRIORITY)
        // Macros are the PRIMARY UI mechanism (verb picker). If a macro matches,
        // it takes precedence over verb patterns.
        {
            let macro_results = self.search_macros(query, limit);
            for result in macro_results {
                if self.matches_domain(&result.verb, domain_filter)
                    && !seen_verbs.contains(&result.verb)
                {
                    tracing::debug!(
                        verb = %result.verb,
                        score = result.score,
                        label = %result.matched_phrase,
                        "VerbSearch: macro match"
                    );
                    seen_verbs.insert(result.verb.clone());
                    results.push(result);
                }
            }
        }

        // If we got a high-confidence macro match (exact label/FQN), return early
        if !results.is_empty() && results[0].score >= 1.0 {
            tracing::debug!(
                verb = %results[0].verb,
                "VerbSearch: returning early with exact macro match"
            );
            return Ok(results);
        }

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

        // 7. Phonetic fallback (typo handling)
        // If semantic search returned low-confidence results, try phonetic matching
        // This handles typos like "allainz" → "allianz" via dmetaphone codes
        let top_score = results.first().map(|r| r.score).unwrap_or(0.0);
        if top_score < self.semantic_threshold && results.len() < limit {
            if let Some(verb_service) = &self.verb_service {
                tracing::debug!(
                    top_score = top_score,
                    threshold = self.semantic_threshold,
                    "VerbSearch: semantic confidence low, trying phonetic fallback"
                );
                match verb_service
                    .search_by_phonetic(query, (limit - results.len()) as i64)
                    .await
                {
                    Ok(phonetic_results) => {
                        tracing::debug!(
                            phonetic_results_count = phonetic_results.len(),
                            "VerbSearch: phonetic search returned"
                        );
                        for pm in phonetic_results {
                            if seen_verbs.contains(&pm.verb) {
                                continue;
                            }
                            if !self.matches_domain(&pm.verb, domain_filter) {
                                continue;
                            }
                            // Score phonetic matches slightly below semantic threshold
                            // to indicate they're from phonetic fallback
                            let phonetic_score = (pm.phonetic_score * 0.7) as f32;
                            let description = self.get_verb_description(&pm.verb).await;
                            seen_verbs.insert(pm.verb.clone());
                            results.push(VerbSearchResult {
                                verb: pm.verb,
                                score: phonetic_score.max(0.5), // Floor at 0.5
                                source: VerbSearchSource::Phonetic,
                                matched_phrase: pm.pattern,
                                description,
                            });
                            if results.len() >= limit {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "VerbSearch: phonetic search failed");
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
    /// ## Two-Stage Centroid Search (when enabled)
    ///
    /// When `use_centroids` is true and centroids are available:
    /// 1. Query centroids to get top-K candidate verbs (fast, stable)
    /// 2. Refine with pattern-level matches within shortlist (precise, evidenced)
    /// 3. Combine scores: `centroid_weight * centroid + pattern_weight * pattern`
    ///
    /// This reduces variance from noisy individual phrases and provides larger score gaps.
    ///
    /// Issue I fix: Also fetches from agent.invocation_phrases (learned) and unions.
    async fn search_global_semantic_with_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let verb_service = match &self.verb_service {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        // Try centroid-based two-stage search first (if enabled)
        if self.use_centroids {
            match self
                .search_with_centroids(verb_service, query_embedding, limit)
                .await
            {
                Ok(results) if !results.is_empty() => {
                    tracing::debug!(
                        results_count = results.len(),
                        "VerbSearch: centroid search returned results"
                    );
                    return Ok(results);
                }
                Ok(_) => {
                    tracing::debug!(
                        "VerbSearch: centroid search returned empty, falling back to pattern search"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "VerbSearch: centroid search failed, falling back to pattern search"
                    );
                }
            }
        }

        // Fallback: direct pattern search (original behavior)
        self.search_patterns_directly(verb_service, query_embedding, limit)
            .await
    }

    /// Option A: Centroids as candidate generator only
    ///
    /// Strategy:
    /// 1. Run pattern retrieval (existing) → get top-K verbs with pattern scores
    /// 2. Run centroid retrieval → get top-M candidate verbs
    /// 3. Union the candidate sets
    /// 4. For centroid-only verbs (missing pattern evidence), rescore via restricted pattern search
    /// 5. Rank ONLY by pattern score (centroids don't affect final score)
    /// 6. Acceptance gate uses pattern score and pattern gap only
    ///
    /// This preserves baseline accuracy while using centroids to boost recall.
    ///
    /// ## Instrumentation (OB_INTENT_TRACE=1)
    /// - PATTERN_TOP5: verb, pattern_score, best_phrase
    /// - CENTROID_TOP5: verb, centroid_score
    /// - ADDED_FROM_CENTROIDS: count of verbs not in pattern top-K
    /// - RESCORED_FROM_CENTROIDS: verb, rescored_pattern_score, best_phrase
    #[allow(clippy::type_complexity)]
    async fn search_with_centroids(
        &self,
        verb_service: &VerbService,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        use std::collections::HashMap;

        let trace_enabled = std::env::var("OB_INTENT_TRACE").is_ok();

        // =====================================================================
        // Stage 1: Get pattern top-K (existing baseline behavior)
        // =====================================================================
        let pattern_results = self
            .search_patterns_directly(verb_service, query_embedding, limit)
            .await?;

        // Trace: PATTERN_TOP5
        if trace_enabled {
            eprintln!("=== CENTROID TRACE ===");
            eprintln!("PATTERN_TOP5:");
            for (i, pr) in pattern_results.iter().take(5).enumerate() {
                eprintln!(
                    "  {}: {} (score={:.3}) phrase=\"{}\"",
                    i + 1,
                    pr.verb,
                    pr.score,
                    pr.matched_phrase
                );
            }
        }

        // Build map of pattern results (verb -> result)
        let mut candidates: HashMap<String, VerbSearchResult> = HashMap::new();
        for pr in pattern_results {
            candidates.insert(pr.verb.clone(), pr);
        }

        let pattern_verbs: HashSet<String> = candidates.keys().cloned().collect();

        // =====================================================================
        // Stage 2: Get centroid shortlist (candidate generator)
        // =====================================================================
        let centroid_matches = verb_service
            .query_centroids(query_embedding, self.centroid_shortlist_size)
            .await?;

        // Trace: CENTROID_TOP5
        if trace_enabled {
            eprintln!("CENTROID_TOP5:");
            for (i, cm) in centroid_matches.iter().take(5).enumerate() {
                let in_pattern = if pattern_verbs.contains(&cm.verb_name) {
                    " [in pattern]"
                } else {
                    ""
                };
                eprintln!(
                    "  {}: {} (centroid_score={:.3}){}",
                    i + 1,
                    cm.verb_name,
                    cm.score,
                    in_pattern
                );
            }
        }

        if centroid_matches.is_empty() {
            if trace_enabled {
                eprintln!("ADDED_FROM_CENTROIDS: 0 (no centroids available)");
                eprintln!("=== END TRACE ===");
            }
            // No centroids, just return pattern results
            let mut results: Vec<VerbSearchResult> = candidates.into_values().collect();
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            return Ok(results);
        }

        // Find centroid-only verbs (not in pattern results)
        let centroid_only: Vec<String> = centroid_matches
            .iter()
            .map(|m| m.verb_name.clone())
            .filter(|v| !pattern_verbs.contains(v))
            .collect();

        let centroid_only_count = centroid_only.len();

        if trace_enabled {
            eprintln!("ADDED_FROM_CENTROIDS: {}", centroid_only_count);
        }

        tracing::debug!(
            centroid_count = centroid_matches.len(),
            centroid_only_count = centroid_only_count,
            top_centroid = %centroid_matches.first().map(|m| &m.verb_name).unwrap_or(&"".to_string()),
            "VerbSearch: centroid candidate generation"
        );

        // =====================================================================
        // Stage 3: Rescore centroid-only verbs via restricted pattern search
        // =====================================================================
        let mut rescored_count = 0;
        if !centroid_only.is_empty() {
            let centroid_only_refs: Vec<&str> = centroid_only.iter().map(|s| s.as_str()).collect();

            // Get pattern evidence for centroid-only verbs (RESTRICTED to these verbs only)
            let rescored_patterns = verb_service
                .search_patterns_for_verbs(
                    query_embedding,
                    &centroid_only_refs,
                    (centroid_only.len() * 3) as i32,
                )
                .await?;

            // Build best pattern score per verb
            let mut best_patterns: HashMap<String, (f64, String)> = HashMap::new();
            for pm in rescored_patterns {
                best_patterns
                    .entry(pm.verb.clone())
                    .and_modify(|(score, phrase)| {
                        if pm.similarity > *score {
                            *score = pm.similarity;
                            *phrase = pm.phrase.clone();
                        }
                    })
                    .or_insert((pm.similarity, pm.phrase));
            }

            if trace_enabled {
                eprintln!("RESCORED_FROM_CENTROIDS:");
            }

            // Add centroid-only verbs with their rescored pattern scores
            for verb in &centroid_only {
                if let Some((pattern_score, matched_phrase)) = best_patterns.get(verb) {
                    let score = *pattern_score as f32;

                    if trace_enabled {
                        let status = if score >= self.fallback_threshold {
                            "ADDED"
                        } else {
                            "BELOW_THRESHOLD"
                        };
                        eprintln!(
                            "  {} (rescored={:.3}) phrase=\"{}\" [{}]",
                            verb, score, matched_phrase, status
                        );
                    }

                    // Only add if rescored pattern score meets threshold
                    if score >= self.fallback_threshold {
                        rescored_count += 1;
                        tracing::debug!(
                            verb = %verb,
                            pattern_score = score,
                            phrase = %matched_phrase,
                            "VerbSearch: centroid-boosted candidate"
                        );

                        candidates.insert(
                            verb.clone(),
                            VerbSearchResult {
                                verb: verb.clone(),
                                score,
                                source: VerbSearchSource::PatternEmbedding,
                                matched_phrase: matched_phrase.clone(),
                                description: None,
                            },
                        );
                    }
                } else if trace_enabled {
                    eprintln!("  {} (NO_PATTERN_EVIDENCE) [SKIPPED]", verb);
                }
                // If no pattern evidence found, verb is NOT added (centroid alone insufficient)
            }
        }

        if trace_enabled {
            eprintln!(
                "CENTROID_SUMMARY: {} centroid-only, {} rescored and added",
                centroid_only_count, rescored_count
            );
            eprintln!("=== END TRACE ===");
        }

        // =====================================================================
        // Stage 4: Rank by pattern score only
        // =====================================================================
        let mut results: Vec<VerbSearchResult> = candidates.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        // Add descriptions
        for result in &mut results {
            if result.description.is_none() {
                result.description = self.get_verb_description(&result.verb).await;
            }
        }

        Ok(results)
    }

    /// Direct pattern search (fallback when centroids unavailable)
    async fn search_patterns_directly(
        &self,
        verb_service: &VerbService,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        use std::collections::HashMap;

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

    /// Search operator macros by label/FQN match
    ///
    /// Macros are the PRIMARY UI mechanism (verb picker). This search:
    /// 1. Exact FQN match (e.g., "structure.setup")
    /// 2. Exact label match (case-insensitive, e.g., "Set up Structure")
    /// 3. Fuzzy label match (contains query, e.g., "structure" matches "Set up Structure")
    ///
    /// Returns matches with score 1.0 for exact, 0.95 for fuzzy.
    fn search_macros(&self, query: &str, limit: usize) -> Vec<VerbSearchResult> {
        let registry = match &self.macro_registry {
            Some(r) => r,
            None => return vec![],
        };

        if registry.is_empty() {
            return vec![];
        }

        let query_lower = query.trim().to_lowercase();
        let mut results = Vec::new();

        // 1. Exact FQN match
        if let Some(macro_def) = registry.get(&query_lower) {
            results.push(VerbSearchResult {
                verb: macro_def.fqn.clone(),
                score: 1.0,
                source: VerbSearchSource::Macro,
                matched_phrase: macro_def.ui.label.clone(),
                description: Some(macro_def.ui.description.clone()),
            });
            return results; // Exact FQN is definitive
        }

        // 2. Exact label match (case-insensitive)
        for macro_def in registry.list(None) {
            if macro_def.ui.label.to_lowercase() == query_lower {
                results.push(VerbSearchResult {
                    verb: macro_def.fqn.clone(),
                    score: 1.0,
                    source: VerbSearchSource::Macro,
                    matched_phrase: macro_def.ui.label.clone(),
                    description: Some(macro_def.ui.description.clone()),
                });
            }
        }

        if !results.is_empty() {
            results.truncate(limit);
            return results;
        }

        // 3. Fuzzy label match with word-level scoring
        // Match based on word overlap between query and label/description
        let query_words: std::collections::HashSet<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 2) // Skip tiny words like "a", "to"
            .collect();

        for macro_def in registry.list(None) {
            let label_lower = macro_def.ui.label.to_lowercase();
            let desc_lower = macro_def.ui.description.to_lowercase();
            let fqn_lower = macro_def.fqn.to_lowercase();

            // Combine label, description, and domain for matching
            let combined = format!("{} {} {}", label_lower, desc_lower, fqn_lower);
            let combined_words: std::collections::HashSet<&str> = combined
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() > 2)
                .collect();

            // Count matching words
            let matching_words = query_words.intersection(&combined_words).count();

            // Also check for key phrases
            let has_setup = query_lower.contains("setup") || query_lower.contains("set up");
            let has_create = query_lower.contains("create") || query_lower.contains("new");
            let has_structure = query_lower.contains("structure") || query_lower.contains("fund");

            // Score based on word overlap and phrase matching
            let is_structure_macro = fqn_lower.starts_with("structure.");
            let is_case_macro = fqn_lower.starts_with("case.");
            let is_mandate_macro = fqn_lower.starts_with("mandate.");

            let mut score = 0.0;

            // Word overlap scoring
            if matching_words > 0 {
                score = 0.7 + (matching_words as f32 * 0.05).min(0.2);
            }

            // Boost for setup/create + structure domain match
            if (has_setup || has_create) && has_structure && is_structure_macro {
                score = score.max(0.92);
            }

            // Boost for case-related queries
            if (query_lower.contains("case") || query_lower.contains("kyc")) && is_case_macro {
                score = score.max(0.92);
            }

            // Boost for mandate/trading queries
            if (query_lower.contains("mandate") || query_lower.contains("trading"))
                && is_mandate_macro
            {
                score = score.max(0.92);
            }

            if score > 0.7 {
                results.push(VerbSearchResult {
                    verb: macro_def.fqn.clone(),
                    score,
                    source: VerbSearchSource::Macro,
                    matched_phrase: macro_def.ui.label.clone(),
                    description: Some(macro_def.ui.description.clone()),
                });
            }
        }

        // Sort by score descending (all same score, but be consistent)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        results
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
    fn test_check_ambiguity_suggest_below_threshold() {
        let threshold = 0.80;

        // Candidate below semantic threshold but above fallback threshold (0.55)
        // should trigger Suggest path, not NoMatch
        let candidates = vec![VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.75, // below semantic threshold, above fallback
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        }];

        let outcome = check_ambiguity(&candidates, threshold);

        // With fallback_threshold=0.55, score 0.75 falls into Suggest range
        assert!(matches!(outcome, VerbSearchOutcome::Suggest { .. }));
    }

    #[test]
    fn test_check_ambiguity_no_match_below_fallback() {
        let threshold = 0.80;

        // Candidate below BOTH thresholds should be NoMatch
        let candidates = vec![VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.50, // below fallback threshold (0.55)
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

    // =========================================================================
    // Macro Search Tests
    // =========================================================================

    #[test]
    fn test_macro_search_exact_fqn() {
        use crate::macros::{MacroArgs, MacroRouting, MacroTarget, MacroUi, OperatorMacroDef};

        let mut registry = OperatorMacroRegistry::new();
        registry.register(OperatorMacroDef {
            fqn: "structure.setup".to_string(),
            kind: "macro".to_string(),
            ui: MacroUi {
                label: "Set up Structure".to_string(),
                description: "Create a new fund or mandate structure".to_string(),
                target_label: None,
            },
            routing: MacroRouting {
                mode_tags: vec!["onboarding".to_string()],
                operator_domain: "structure".to_string(),
            },
            target: MacroTarget {
                operates_on: "client_ref".to_string(),
                produces: Some("structure_ref".to_string()),
            },
            args: MacroArgs {
                style: "keyworded".to_string(),
                required: Default::default(),
                optional: Default::default(),
            },
            prereqs: vec![],
            expands_to: vec![],
            sets_state: vec![],
            unlocks: vec![],
        });

        let searcher =
            HybridVerbSearcher::minimal().with_macro_registry(std::sync::Arc::new(registry));

        let results = searcher.search_macros("structure.setup", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verb, "structure.setup");
        assert_eq!(results[0].score, 1.0);
        assert!(matches!(results[0].source, VerbSearchSource::Macro));
    }

    #[test]
    fn test_macro_search_exact_label() {
        use crate::macros::{MacroArgs, MacroRouting, MacroTarget, MacroUi, OperatorMacroDef};

        let mut registry = OperatorMacroRegistry::new();
        registry.register(OperatorMacroDef {
            fqn: "structure.setup".to_string(),
            kind: "macro".to_string(),
            ui: MacroUi {
                label: "Set up Structure".to_string(),
                description: "Create a new fund or mandate structure".to_string(),
                target_label: None,
            },
            routing: MacroRouting {
                mode_tags: vec!["onboarding".to_string()],
                operator_domain: "structure".to_string(),
            },
            target: MacroTarget {
                operates_on: "client_ref".to_string(),
                produces: Some("structure_ref".to_string()),
            },
            args: MacroArgs {
                style: "keyworded".to_string(),
                required: Default::default(),
                optional: Default::default(),
            },
            prereqs: vec![],
            expands_to: vec![],
            sets_state: vec![],
            unlocks: vec![],
        });

        let searcher =
            HybridVerbSearcher::minimal().with_macro_registry(std::sync::Arc::new(registry));

        // Exact label match (case-insensitive)
        let results = searcher.search_macros("set up structure", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verb, "structure.setup");
        assert_eq!(results[0].score, 1.0);
    }

    #[test]
    fn test_macro_search_fuzzy_label() {
        use crate::macros::{MacroArgs, MacroRouting, MacroTarget, MacroUi, OperatorMacroDef};

        let mut registry = OperatorMacroRegistry::new();
        registry.register(OperatorMacroDef {
            fqn: "structure.setup".to_string(),
            kind: "macro".to_string(),
            ui: MacroUi {
                label: "Set up Structure".to_string(),
                description: "Create a new fund or mandate structure".to_string(),
                target_label: None,
            },
            routing: MacroRouting {
                mode_tags: vec!["onboarding".to_string()],
                operator_domain: "structure".to_string(),
            },
            target: MacroTarget {
                operates_on: "client_ref".to_string(),
                produces: Some("structure_ref".to_string()),
            },
            args: MacroArgs {
                style: "keyworded".to_string(),
                required: Default::default(),
                optional: Default::default(),
            },
            prereqs: vec![],
            expands_to: vec![],
            sets_state: vec![],
            unlocks: vec![],
        });

        let searcher =
            HybridVerbSearcher::minimal().with_macro_registry(std::sync::Arc::new(registry));

        // Fuzzy match - query contains "structure" (1 matching word = 0.75)
        let results = searcher.search_macros("structure", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verb, "structure.setup");
        assert_eq!(results[0].score, 0.75); // Word overlap: 0.7 + 0.05 for 1 word
    }

    #[test]
    fn test_macro_search_no_match() {
        use crate::macros::{MacroArgs, MacroRouting, MacroTarget, MacroUi, OperatorMacroDef};

        let mut registry = OperatorMacroRegistry::new();
        registry.register(OperatorMacroDef {
            fqn: "structure.setup".to_string(),
            kind: "macro".to_string(),
            ui: MacroUi {
                label: "Set up Structure".to_string(),
                description: "Create a new fund or mandate structure".to_string(),
                target_label: None,
            },
            routing: MacroRouting {
                mode_tags: vec!["onboarding".to_string()],
                operator_domain: "structure".to_string(),
            },
            target: MacroTarget {
                operates_on: "client_ref".to_string(),
                produces: Some("structure_ref".to_string()),
            },
            args: MacroArgs {
                style: "keyworded".to_string(),
                required: Default::default(),
                optional: Default::default(),
            },
            prereqs: vec![],
            expands_to: vec![],
            sets_state: vec![],
            unlocks: vec![],
        });

        let searcher =
            HybridVerbSearcher::minimal().with_macro_registry(std::sync::Arc::new(registry));

        // No match - completely different query
        let results = searcher.search_macros("something completely different", 5);
        assert!(results.is_empty());
    }
}

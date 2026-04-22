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
use crate::dsl_v2::macros::MacroRegistry;
use crate::dsl_v2::runtime_registry::runtime_registry;
use dsl_runtime::entity_kind::canonicalize as canonicalize_entity_kind;
use crate::lexicon::LexiconService;
use crate::mcp::compound_intent::extract_compound_signals;
use crate::mcp::macro_index::{MacroIndex, MacroResolveOutcome};
use crate::mcp::scenario_index::{ResolvedRoute, ScenarioIndex, ScenarioResolveOutcome};

/// Shared lexicon service type alias
pub type SharedLexicon = Arc<dyn LexiconService>;

/// A unified verb search result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerbSearchResult {
    pub verb: String,
    pub score: f32,
    pub source: VerbSearchSource,
    pub matched_phrase: String,
    pub description: Option<String>,
    /// Journey metadata for Tier -2 matches (ScenarioIndex / MacroIndex).
    /// Carries the resolved route so the orchestrator can expand macros
    /// instead of sending to LLM for arg extraction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey: Option<JourneyMetadata>,
}

/// Metadata attached to Tier -2 verb search results (ScenarioIndex / MacroIndex).
/// Enables the orchestrator to expand macros deterministically instead of
/// falling through to LLM-based arg extraction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JourneyMetadata {
    /// Scenario ID if matched via ScenarioIndex (Tier -2A).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<String>,
    /// Scenario title for progress narration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_title: Option<String>,
    /// Resolved route: how to expand the matched macro(s).
    pub route: JourneyRoute,
}

/// Serializable resolved route for journey-level matches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JourneyRoute {
    /// Expand a single macro.
    Macro { macro_fqn: String },
    /// Expand a sequence of macros in order.
    MacroSequence { macros: Vec<String> },
    /// Needs user selection (e.g., jurisdiction) before routing.
    NeedsSelection {
        select_on: String,
        options: Vec<(String, String)>,
        then: Vec<String>,
    },
    /// Resolved to a single verb (no macro expansion).
    Verb { verb_fqn: String },
    /// Verb selector needs clarification.
    NeedsVerbSelection {
        select_on: String,
        options: Vec<(String, String)>,
    },
}

impl From<&ResolvedRoute> for JourneyRoute {
    fn from(route: &ResolvedRoute) -> Self {
        match route {
            ResolvedRoute::Macro { macro_fqn } => JourneyRoute::Macro {
                macro_fqn: macro_fqn.clone(),
            },
            ResolvedRoute::MacroSequence { macros } => JourneyRoute::MacroSequence {
                macros: macros.clone(),
            },
            ResolvedRoute::NeedsSelection {
                select_on,
                options,
                then,
            } => JourneyRoute::NeedsSelection {
                select_on: select_on.clone(),
                options: options
                    .iter()
                    .filter_map(|o| {
                        o.macro_fqn
                            .clone()
                            .or_else(|| o.sub_select.as_ref().map(|s| s.default_fqn.clone()))
                            .map(|fqn| (o.value.clone(), fqn))
                    })
                    .collect(),
                then: then.clone(),
            },
            ResolvedRoute::Verb { verb_fqn } => JourneyRoute::Verb {
                verb_fqn: verb_fqn.clone(),
            },
            ResolvedRoute::NeedsVerbSelection { select_on, options } => {
                JourneyRoute::NeedsVerbSelection {
                    select_on: select_on.clone(),
                    options: options.clone(),
                }
            }
        }
    }
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
    /// Global learned via invocation_phrases (Issue I distinction)
    GlobalLearned,
    /// Cold start pattern embeddings (Issue I distinction)
    PatternEmbedding,
    /// Phonetic (dmetaphone) match - fallback for typos
    Phonetic,
    /// Operator macro match (business vocabulary layer)
    Macro,
    /// Lexicon exact label match (lexical search lane)
    LexiconExact,
    /// Lexicon token overlap match (lexical search lane)
    LexiconToken,
    /// Constellation state-aware index match (Tier -0.5)
    ConstellationIndex,
    /// MacroIndex deterministic scored match (Tier -2B)
    MacroIndex,
    /// ScenarioIndex journey-level match (Tier -2A)
    ScenarioIndex,
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
        runner_up: Box<VerbSearchResult>,
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
/// The Suggest path is CRITICAL for learning: vague queries like "show me the deals"
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
                        runner_up: Box::new(runner_up.clone()),
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

/// Apply narration boost to search results.
///
/// Verbs that were suggested by the NarrationEngine in the previous turn
/// get a small score bump (+0.05), biasing disambiguation toward the
/// contextually expected action. The boost is small enough that a strong
/// unrelated match (0.10+ gap) still wins.
///
/// Design: ADR 044 (ai-thoughts/044-narration-boost-signal.md)
pub fn apply_narration_boost(results: &mut [VerbSearchResult], hot_verbs: &[String]) {
    const NARRATION_BOOST: f32 = 0.05;
    if hot_verbs.is_empty() {
        return;
    }
    for result in results.iter_mut() {
        if hot_verbs.iter().any(|hv| hv == &result.verb) {
            result.score += NARRATION_BOOST;
            tracing::debug!(
                verb = %result.verb,
                new_score = result.score,
                "NarrationBoost: +{} applied",
                NARRATION_BOOST,
            );
        }
    }
    // Re-sort after boost to maintain descending score order
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Hybrid verb searcher combining all discovery strategies
///
/// All DB access is through VerbService - no direct sqlx calls.
pub struct HybridVerbSearcher {
    verb_service: Option<Arc<VerbService>>,
    learned_data: Option<SharedLearnedData>,
    embedder: Option<SharedEmbedder>,
    /// Operator macro registry for business vocabulary search
    macro_registry: Option<Arc<MacroRegistry>>,
    /// Lexicon service for fast lexical search (runs BEFORE semantic embedding)
    lexicon: Option<SharedLexicon>,
    /// Macro index for deterministic Tier -2B macro search (replaces search_macros)
    macro_index: Option<Arc<MacroIndex>>,
    /// Scenario index for journey-level Tier -2A resolution
    scenario_index: Option<Arc<ScenarioIndex>>,
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
            macro_registry: self.macro_registry.clone(),
            lexicon: self.lexicon.clone(),
            macro_index: self.macro_index.clone(),
            scenario_index: self.scenario_index.clone(),
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
            embedder: None,       // Embedder added separately via with_embedder
            macro_registry: None, // Macro registry added separately via with_macro_registry
            lexicon: None,        // Lexicon added separately via with_lexicon
            macro_index: None,    // Macro index added separately via with_macro_index
            scenario_index: None, // Scenario index added separately via with_scenario_index
            // BGE asymmetric mode thresholds (query→target is lower than target→target)
            semantic_threshold: 0.65,  // Decision gate for accepting match
            fallback_threshold: 0.55,  // Retrieval cutoff for DB queries
            blocklist_threshold: 0.80, // Collision detection
        }
    }

    /// Create minimal searcher (for testing)
    pub fn minimal() -> Self {
        Self {
            verb_service: None,
            learned_data: None,
            embedder: None,
            macro_registry: None,
            lexicon: None,
            macro_index: None,
            scenario_index: None,
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

    /// Add macro registry for operator vocabulary search
    pub fn with_macro_registry(mut self, registry: Arc<MacroRegistry>) -> Self {
        self.macro_registry = Some(registry);
        self
    }

    /// Get a reference to the macro registry (if configured).
    /// Used by the orchestrator for sequence validation and macro expansion.
    pub fn macro_registry(&self) -> Option<&Arc<MacroRegistry>> {
        self.macro_registry.as_ref()
    }

    /// Add lexicon service for fast lexical search (runs BEFORE semantic embedding)
    pub fn with_lexicon(mut self, lexicon: SharedLexicon) -> Self {
        self.lexicon = Some(lexicon);
        self
    }

    /// Add macro index for deterministic Tier -2B macro search
    pub fn with_macro_index(mut self, macro_index: Arc<MacroIndex>) -> Self {
        self.macro_index = Some(macro_index);
        self
    }

    /// Add scenario index for journey-level Tier -2A resolution
    pub fn with_scenario_index(mut self, scenario_index: Arc<ScenarioIndex>) -> Self {
        self.scenario_index = Some(scenario_index);
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

    /// Check if semantic search is available (embedder AND verb_service configured)
    ///
    /// Note: Both embedder and verb_service are required for semantic search.
    /// - embedder: generates query embeddings
    /// - verb_service: queries verb_pattern_embeddings table
    pub fn has_semantic_search(&self) -> bool {
        let has_embedder = self.embedder.is_some();
        let has_verb_service = self.verb_service.is_some();

        if has_embedder && !has_verb_service {
            tracing::debug!(
                "Embedder configured but verb_service missing - semantic search disabled"
            );
        } else if !has_embedder && has_verb_service {
            tracing::debug!(
                "VerbService configured but embedder missing - semantic search disabled"
            );
        }

        has_embedder && has_verb_service
    }

    /// Search for verbs matching user intent
    ///
    /// Priority order:
    /// 0. Operator macros (business vocabulary) - score 1.0 exact, 0.95 fuzzy
    /// 1. Lexicon (exact label/token overlap) - score 0.34-1.0 (Phase C of 072)
    /// 2. User-specific learned (exact) - score 1.0
    /// 3. Global learned (exact) - score 1.0
    /// 4. User-specific learned (semantic) - score 0.8-0.99
    /// 5. Global learned (semantic) - score 0.8-0.99
    /// 6. Blocklist filter (rejects blocked verbs)
    /// 7. Global semantic (cold start) - score fallback_threshold-0.95
    /// 8. Phonetic fallback (typo handling) - score 0.5-0.7
    #[allow(clippy::too_many_arguments)]
    pub async fn search(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        domain_filter: Option<&str>,
        entity_kind: Option<&str>,
        limit: usize,
        allowed_verbs: Option<&HashSet<String>>,
        _entity_mention_spans: Option<&[(usize, usize)]>,
        constellation_index: Option<
            &crate::agent::constellation_verb_index::ConstellationVerbIndex,
        >,
    ) -> Result<Vec<VerbSearchResult>> {
        let mut results = Vec::new();
        let mut seen_verbs: HashSet<String> = HashSet::new();

        // Normalize query ONCE at the start (used for exact matching)
        let normalized = query.trim().to_lowercase();

        // Debug: Log semantic capability status
        tracing::debug!(
            has_verb_service = self.verb_service.is_some(),
            has_embedder = self.embedder.is_some(),
            has_semantic = self.has_semantic_search(),
            query = %query,
            "VerbSearch: checking semantic capability"
        );

        // Compute embedding ONCE at the start (used for all semantic lookups)
        // This avoids computing the same embedding 4 times (user semantic, learned semantic,
        // global semantic, blocklist) - saves ~15-30ms per search
        // Use embed_query for user input (applies BGE instruction prefix)
        let query_embedding: Option<Vec<f32>> = if self.has_semantic_search() {
            tracing::debug!("VerbSearch: computing query embedding...");
            match self.embedder.as_ref().unwrap().embed_query(query).await {
                Ok(emb) => {
                    tracing::debug!(
                        embedding_len = emb.len(),
                        first_3 = ?&emb[..3.min(emb.len())],
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

        // Scale fallback threshold for short queries — BGE asymmetric prefix
        // adds noise for very short inputs
        let effective_fallback_threshold = {
            let word_count = query.split_whitespace().count();
            if word_count <= 2 {
                (self.fallback_threshold - 0.15).max(0.35)
            } else if word_count <= 4 {
                (self.fallback_threshold - 0.10).max(0.40)
            } else {
                self.fallback_threshold
            }
        };

        // ── Feature extraction: compound signals (shared by Tier -2A and CVI) ──
        let compound_signals = extract_compound_signals(&normalized);
        let has_compound = compound_signals.has_any();
        if has_compound {
            tracing::debug!(
                strength = compound_signals.strength(),
                action = ?compound_signals.compound_action,
                jurisdiction = ?compound_signals.jurisdiction,
                structure_nouns = ?compound_signals.structure_nouns,
                "Compound signals detected — Tier -2A eligible"
            );
        }

        // ── Tier -2A: ScenarioIndex journey-level resolution ─────────────────
        // Fires when compound signals are present OR always (scenarios with
        // `any_of: [phrase_match]` gates handle their own phrase matching).
        // Score 0.97 — higher than MacroIndex (0.96).
        {
            if let Some(ref scenario_idx) = self.scenario_index {
                let outcome = scenario_idx.resolve(
                    &normalized,
                    None, // active_mode — TODO: thread session mode when available
                    self.macro_index.as_deref(),
                );
                match outcome {
                    ScenarioResolveOutcome::Matched(m) => {
                        let route_fqn = match &m.route {
                            ResolvedRoute::Macro { macro_fqn } => Some(macro_fqn.clone()),
                            ResolvedRoute::MacroSequence { macros } => macros.first().cloned(),
                            ResolvedRoute::Verb { verb_fqn } => Some(verb_fqn.clone()),
                            ResolvedRoute::NeedsSelection { .. }
                            | ResolvedRoute::NeedsVerbSelection { .. } => None,
                        };
                        if let Some(fqn) = route_fqn {
                            // F14 fix (2026-04-22): scenario routes MUST pass the CCIR
                            // allowed_verbs check. Previously this path bypassed it — any
                            // scenario-routed verb could skip SemOS gating. Other tiers
                            // (scenario-ambiguous line ~684, macro-matched line ~725,
                            // macro-ambiguous line ~765) already filter; this aligns the
                            // scenario-matched path with them. The fail-open semantics of
                            // `is_none_or` (permit when allowed_verbs is None) is handled
                            // separately by Slice 3.2 / F17 (runbook compiler fail-closed).
                            if self.matches_entity_kind(&fqn, entity_kind)
                                && allowed_verbs.is_none_or(|av| av.contains(&fqn))
                                && !seen_verbs.contains(&fqn)
                            {
                                tracing::info!(
                                    scenario = %m.scenario_id,
                                    title = %m.title,
                                    verb = %fqn,
                                    score = m.score,
                                    tier = "Tier2A_ScenarioIndex",
                                    "ScenarioIndex: matched journey scenario"
                                );
                                seen_verbs.insert(fqn.clone());
                                // Macros score above exact phrase matches (1.0) because
                                // they capture compound intent — safer, atomic, complete.
                                // Constellation-scoped macros are the preferred path.
                                results.push(VerbSearchResult {
                                    verb: fqn,
                                    score: 1.05,
                                    source: VerbSearchSource::ScenarioIndex,
                                    matched_phrase: m.title.clone(),
                                    description: Some(m.title.clone()),
                                    journey: Some(JourneyMetadata {
                                        scenario_id: Some(m.scenario_id.clone()),
                                        scenario_title: Some(m.title.clone()),
                                        route: JourneyRoute::from(&m.route),
                                    }),
                                });
                                return Ok(normalize_candidates(results, limit));
                            }
                        } else {
                            // NeedsSelection — return as ambiguous for DecisionPacket
                            tracing::info!(
                                scenario = %m.scenario_id,
                                tier = "Tier2A_ScenarioIndex",
                                "ScenarioIndex: needs jurisdiction selection"
                            );
                        }
                    }
                    ScenarioResolveOutcome::Ambiguous(candidates) => {
                        tracing::debug!(
                            count = candidates.len(),
                            tier = "Tier2A_ScenarioIndex",
                            "ScenarioIndex: ambiguous, returning multiple candidates"
                        );
                        for m in candidates {
                            let route_fqn = match &m.route {
                                ResolvedRoute::Macro { macro_fqn } => Some(macro_fqn.clone()),
                                ResolvedRoute::MacroSequence { macros } => macros.first().cloned(),
                                ResolvedRoute::Verb { verb_fqn } => Some(verb_fqn.clone()),
                                ResolvedRoute::NeedsSelection { .. }
                                | ResolvedRoute::NeedsVerbSelection { .. } => None,
                            };
                            if let Some(fqn) = route_fqn {
                                // Scenarios are compound intents — domain_filter should not suppress them
                                if allowed_verbs.is_none_or(|av| av.contains(&fqn))
                                    && !seen_verbs.contains(&fqn)
                                {
                                    seen_verbs.insert(fqn.clone());
                                    results.push(VerbSearchResult {
                                        verb: fqn,
                                        score: 0.97,
                                        source: VerbSearchSource::ScenarioIndex,
                                        matched_phrase: m.title.clone(),
                                        description: Some(m.title.clone()),
                                        journey: Some(JourneyMetadata {
                                            scenario_id: Some(m.scenario_id.clone()),
                                            scenario_title: Some(m.title.clone()),
                                            route: JourneyRoute::from(&m.route),
                                        }),
                                    });
                                }
                            }
                        }
                        if !results.is_empty() {
                            return Ok(normalize_candidates(results, limit));
                        }
                    }
                    ScenarioResolveOutcome::NoMatch => {
                        tracing::debug!(
                            "ScenarioIndex: no scenario matched, falling through to Tier -2B"
                        );
                    }
                }
            }
        }

        // ── Tier -2B: MacroIndex deterministic scoring ────────────────────────
        // Replaces old Tier 0 search_macros() with multi-signal deterministic
        // scoring. When MacroIndex is available, uses it; falls back to old
        // search_macros() if only macro_registry is present.
        if let Some(ref macro_idx) = self.macro_index {
            let outcome = macro_idx.resolve(query, None, None);
            match outcome {
                MacroResolveOutcome::Matched(m) => {
                    // Macros are compound intents — domain_filter should not suppress them
                    if allowed_verbs.is_none_or(|av| av.contains(&m.fqn))
                        && !seen_verbs.contains(&m.fqn)
                    {
                        tracing::info!(
                            verb = %m.fqn,
                            score = m.score,
                            tier = "Tier2B_MacroIndex",
                            signals = ?m.explain.matched_signals.iter().map(|s| &s.signal).collect::<Vec<_>>(),
                            "MacroIndex: matched"
                        );
                        seen_verbs.insert(m.fqn.clone());
                        let entry = macro_idx.get_entry(&m.fqn);
                        let macro_fqn_clone = m.fqn.clone();
                        results.push(VerbSearchResult {
                            verb: m.fqn,
                            score: 1.04,
                            source: VerbSearchSource::MacroIndex,
                            matched_phrase: entry.map(|e| e.label.clone()).unwrap_or_default(),
                            description: entry.map(|e| e.description.clone()),
                            journey: Some(JourneyMetadata {
                                scenario_id: None,
                                scenario_title: None,
                                route: JourneyRoute::Macro {
                                    macro_fqn: macro_fqn_clone,
                                },
                            }),
                        });
                        // MacroIndex match at 0.96 — return early
                        return Ok(normalize_candidates(results, limit));
                    }
                }
                MacroResolveOutcome::Ambiguous(candidates) => {
                    tracing::debug!(
                        count = candidates.len(),
                        tier = "Tier2B_MacroIndex",
                        "MacroIndex: ambiguous, returning multiple candidates"
                    );
                    for m in candidates {
                        // Macros bypass domain_filter — pack scoring handles relevance
                        if self.matches_entity_kind(&m.fqn, entity_kind)
                            && allowed_verbs.is_none_or(|av| av.contains(&m.fqn))
                            && !seen_verbs.contains(&m.fqn)
                        {
                            seen_verbs.insert(m.fqn.clone());
                            let entry = macro_idx.get_entry(&m.fqn);
                            let macro_fqn_clone = m.fqn.clone();
                            results.push(VerbSearchResult {
                                verb: m.fqn,
                                score: 1.04,
                                source: VerbSearchSource::MacroIndex,
                                matched_phrase: entry.map(|e| e.label.clone()).unwrap_or_default(),
                                description: entry.map(|e| e.description.clone()),
                                journey: Some(JourneyMetadata {
                                    scenario_id: None,
                                    scenario_title: None,
                                    route: JourneyRoute::Macro {
                                        macro_fqn: macro_fqn_clone,
                                    },
                                }),
                            });
                        }
                    }
                    if !results.is_empty() {
                        return Ok(normalize_candidates(results, limit));
                    }
                }
                MacroResolveOutcome::NoMatch => {
                    // Fall through to lower tiers
                }
            }
        } else {
            // Fallback: old search_macros() when MacroIndex is not wired
            let macro_results = self.search_macros(query, limit);
            for result in macro_results {
                if self.matches_domain(&result.verb, domain_filter)
                    && self.matches_entity_kind(&result.verb, entity_kind)
                    && allowed_verbs.is_none_or(|av| av.contains(&result.verb))
                    && !seen_verbs.contains(&result.verb)
                {
                    tracing::debug!(
                        verb = %result.verb,
                        score = result.score,
                        label = %result.matched_phrase,
                        "VerbSearch: legacy macro match"
                    );
                    seen_verbs.insert(result.verb.clone());
                    results.push(result);
                }
            }
            // If we got a high-confidence legacy macro match (exact label/FQN), return early
            if !results.is_empty() && results[0].score >= 1.0 {
                tracing::debug!(
                    verb = %results[0].verb,
                    "VerbSearch: returning early with exact legacy macro match"
                );
                return Ok(results);
            }
        }

        // ── Tier -0.5: ConstellationVerbIndex (state-aware, deterministic) ──────
        // Uses the live hydrated constellation's available verbs, keyed by
        // (noun, action_stem). Two clues = direct resolution; one clue = boost set.
        // Score 0.94 — above all embedding tiers.
        if let Some(cvi) = constellation_index {
            let action_stem = compound_signals.action_stem.as_deref();

            // Use phase_nouns from compound_signals for noun extraction
            let cvi_nouns: Vec<String> = compound_signals.phase_nouns.clone();

            let mut cvi_matches: Vec<VerbSearchResult> = Vec::new();

            // Two-clue: noun + action_stem (highest signal)
            if let Some(action) = action_stem {
                for noun in &cvi_nouns {
                    for vm in cvi.lookup(noun, action) {
                        if allowed_verbs.is_none_or(|av| av.contains(&vm.verb_fqn))
                            && !seen_verbs.contains(&vm.verb_fqn)
                        {
                            cvi_matches.push(VerbSearchResult {
                                verb: vm.verb_fqn.clone(),
                                score: 0.94,
                                source: VerbSearchSource::ConstellationIndex,
                                matched_phrase: format!("constellation:({},{})", noun, action),
                                journey: None,
                                description: None,
                            });
                        }
                    }
                }
            }

            // Single-clue fallback: by-noun only (logged, no boost)
            if cvi_matches.is_empty() && !cvi_nouns.is_empty() {
                tracing::debug!(
                    nouns = ?cvi_nouns,
                    "ConstellationIndex: noun-only match, no two-clue resolution"
                );
            }

            // If two-clue matched exactly 1 verb, short-circuit
            if cvi_matches.len() == 1 {
                tracing::info!(
                    verb = %cvi_matches[0].verb,
                    matched_phrase = %cvi_matches[0].matched_phrase,
                    "ConstellationIndex: deterministic single-verb resolution"
                );
                seen_verbs.insert(cvi_matches[0].verb.clone());
                results.push(cvi_matches.remove(0));
                return Ok(normalize_candidates(results, limit));
            }

            // Multiple constellation matches — add as high-confidence candidates
            if !cvi_matches.is_empty() {
                tracing::info!(
                    count = cvi_matches.len(),
                    verbs = ?cvi_matches.iter().map(|m| m.verb.as_str()).collect::<Vec<_>>(),
                    "ConstellationIndex: multi-candidate set"
                );
                for m in cvi_matches {
                    seen_verbs.insert(m.verb.clone());
                    results.push(m);
                }
            }
        }

        // 0.5. Lexicon search (fast in-memory lexical matching - Phase C of 072)
        // Runs BEFORE semantic embedding computation for two reasons:
        // 1. Performance: if lexicon finds exact match, we might skip embedding entirely
        // 2. Accuracy: lexicon provides high-confidence matches for known vocabulary
        // The lexicon uses label_to_concepts (exact) and token_to_concepts (overlap)
        if let Some(ref lexicon) = self.lexicon {
            let lexicon_results = lexicon.search_verbs(&normalized, None, limit);
            for candidate in lexicon_results {
                if self.matches_domain(&candidate.dsl_verb, domain_filter)
                    && self.matches_entity_kind(&candidate.dsl_verb, entity_kind)
                    && allowed_verbs.is_none_or(|av| av.contains(&candidate.dsl_verb))
                    && !seen_verbs.contains(&candidate.dsl_verb)
                {
                    // Determine source based on score: exact (1.0) vs token overlap (<1.0)
                    let source = if candidate.score >= 1.0 {
                        VerbSearchSource::LexiconExact
                    } else {
                        VerbSearchSource::LexiconToken
                    };

                    let description = self.get_verb_description(&candidate.dsl_verb).await;

                    tracing::debug!(
                        verb = %candidate.dsl_verb,
                        score = candidate.score,
                        source = ?source,
                        "VerbSearch: lexicon match"
                    );

                    seen_verbs.insert(candidate.dsl_verb.clone());
                    results.push(VerbSearchResult {
                        verb: candidate.dsl_verb,
                        score: candidate.score,
                        source,
                        matched_phrase: query.to_string(),
                        description,
                        journey: None,
                    });
                }
            }
        }

        // 1. User-specific learned phrases (exact match)
        // Learned phrases are user-validated — domain_filter should not suppress them
        if let Some(uid) = user_id {
            if let Some(result) = self.search_user_learned_exact(uid, &normalized).await? {
                if self.matches_entity_kind(&result.verb, entity_kind)
                    && allowed_verbs.is_none_or(|av| av.contains(&result.verb))
                {
                    seen_verbs.insert(result.verb.clone());
                    results.push(result);
                }
            }
        }

        // 2. Global learned phrases (exact match)
        // Learned phrases are user-validated — domain_filter should not suppress them
        if results.is_empty() {
            if let Some(learned) = &self.learned_data {
                let guard = learned.read().await;
                if let Some(verb) = guard.resolve_phrase(&normalized) {
                    if self.matches_entity_kind(verb, entity_kind)
                        && allowed_verbs.is_none_or(|av| av.contains(verb))
                    {
                        let description = self.get_verb_description(verb).await;
                        results.push(VerbSearchResult {
                            verb: verb.to_string(),
                            score: 1.0,
                            source: VerbSearchSource::LearnedExact,
                            matched_phrase: query.to_string(),
                            description,
                            journey: None,
                        });
                        seen_verbs.insert(verb.to_string());
                    }
                }
            }
        }

        // 2.4 Governed phrase_bank lookup (highest precedence exact match).
        // Workspace-qualified, precedence-ordered per §4.5.2.
        // Falls through to dsl_verbs if no match.
        if let Some(ref verb_service) = self.verb_service {
            let allowed_vec =
                allowed_verbs.map(|verbs| verbs.iter().cloned().collect::<Vec<String>>());
            // TODO: pass workspace from MatchContext when available
            let workspace: Option<&str> = None;
            match verb_service
                .find_phrase_bank_exact(&normalized, workspace, allowed_vec.as_deref())
                .await
            {
                Ok(Some(matched)) => {
                    if !seen_verbs.contains(&matched.verb) {
                        let description = self.get_verb_description(&matched.verb).await;
                        seen_verbs.insert(matched.verb.clone());
                        results.push(VerbSearchResult {
                            verb: matched.verb,
                            score: 1.0,
                            source: VerbSearchSource::GlobalLearned,
                            matched_phrase: matched.phrase,
                            description,
                            journey: None,
                        });
                        tracing::debug!(
                            query = %normalized,
                            "VerbSearch: phrase_bank exact match"
                        );
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "VerbSearch: phrase_bank lookup failed");
                }
            }
        }

        // 2.5 Exact pattern match from dsl_verbs arrays (yaml_intent_patterns + intent_patterns).
        // Fallback for phrases not yet migrated to phrase_bank.
        // Always runs — results from other tiers must not suppress exact matches
        // for the user's actual intent.
        if let Some(ref verb_service) = self.verb_service {
            let allowed_vec =
                allowed_verbs.map(|verbs| verbs.iter().cloned().collect::<Vec<String>>());
            match verb_service
                .find_exact_verb_patterns(&normalized, allowed_vec.as_deref(), limit)
                .await
            {
                Ok(exact_matches) => {
                    tracing::debug!(
                        query = %normalized,
                        matches = exact_matches.len(),
                        "VerbSearch: exact dsl_verbs pattern lookup"
                    );
                    for matched in exact_matches {
                        // Exact invocation phrase matches bypass domain_filter —
                        // the user typed exactly what the verb expects, so domain
                        // hints from pack context must not suppress it.
                        if seen_verbs.contains(&matched.verb) {
                            continue;
                        }
                        let description = self.get_verb_description(&matched.verb).await;
                        seen_verbs.insert(matched.verb.clone());
                        results.push(VerbSearchResult {
                            verb: matched.verb,
                            score: 1.0,
                            source: VerbSearchSource::GlobalLearned,
                            matched_phrase: matched.phrase,
                            description,
                            journey: None,
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "VerbSearch: exact dsl_verbs pattern lookup failed");
                }
            }
        }

        // 3. User-specific learned (SEMANTIC match) - top-k for ambiguity detection
        // Learned phrases are user-validated — domain_filter should not suppress them
        if results.is_empty() {
            if let (Some(user_id), Some(embedding)) = (user_id, query_embedding.as_ref()) {
                let user_results = self
                    .search_user_learned_semantic_with_embedding(user_id, embedding, 3)
                    .await?;
                for result in user_results {
                    if self.matches_entity_kind(&result.verb, entity_kind)
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
                tracing::debug!(
                    allowed_verbs_count = allowed_verbs.map(|a| a.len()),
                    "VerbSearch: calling search_global_semantic_with_embedding..."
                );
                match self
                    .search_global_semantic_with_embedding(
                        embedding,
                        limit - results.len(),
                        None,
                        allowed_verbs,
                        effective_fallback_threshold,
                    )
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
                            // Semantic search results are NOT filtered by domain_hint —
                            // pack scoring handles domain relevance via boost/penalty.
                            // domain_filter is too aggressive for cross-domain packs.
                            if !self.matches_entity_kind(&result.verb, entity_kind) {
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
                            // Phonetic fallback is NOT filtered by domain_hint —
                            // same rationale as semantic search above.
                            if !self.matches_entity_kind(&pm.verb, entity_kind) {
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
                                journey: None,
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

        // ── Action stem boost ──────────────────────────────────────────────────
        // When an action stem is detected (e.g., "create" from "create a fund"),
        // boost embedding results whose verb FQN contains that stem as the action
        // segment (after the domain dot). This is a soft preference, not a filter.
        if let Some(ref stem) = compound_signals.action_stem {
            let boost = 0.05_f32;
            let mut boosted_count = 0u32;
            for result in &mut results {
                // Check if verb FQN's action part starts with the stem.
                // E.g., stem "create" matches "cbu.create", "entity.create-placeholder"
                let action_part = result.verb.split('.').next_back().unwrap_or("");
                if action_part == stem.as_str() || action_part.starts_with(&format!("{}-", stem)) {
                    result.score += boost;
                    boosted_count += 1;
                }
            }
            if boosted_count > 0 {
                tracing::debug!(
                    stem = %stem,
                    boosted = boosted_count,
                    "Action stem: boosted matching verbs"
                );
            }
        }

        // Dedupe by verb, sort by score descending, truncate (Issue J/D fix)
        let mut results = normalize_candidates(results, limit);

        // Pre-constrained filter: remove verbs not in the SemReg allowed set (Phase 3 CCIR)
        // This runs after normalize_candidates to avoid interfering with per-tier logic.
        // When allowed_verbs is Some, only verbs in the set survive.
        if let Some(allowed) = allowed_verbs {
            let before_count = results.len();
            results.retain(|r| allowed.contains(&r.verb));
            let pruned = before_count - results.len();
            if pruned > 0 {
                tracing::debug!(
                    pruned_count = pruned,
                    remaining = results.len(),
                    "VerbSearch: SemReg allowed_verbs filter removed candidates"
                );
            }
        }

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

    /// Search only semantic tiers, optionally constrained to a verb whitelist.
    ///
    /// This bypasses lexicon, macro, and learned exact stages and goes straight
    /// to embedding-based retrieval. When `allowed_verbs` is provided with a
    /// small set, the search is restricted to those verb patterns only.
    ///
    /// # Examples
    /// ```ignore
    /// let results = searcher
    ///     .search_embeddings_only("request identity documents", 5, Some(&allowed_verbs))
    ///     .await?;
    /// assert!(results.len() <= 5);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub async fn search_embeddings_only(
        &self,
        query: &str,
        limit: usize,
        allowed_verbs: Option<&HashSet<String>>,
    ) -> Result<Vec<VerbSearchResult>> {
        if !self.has_semantic_search() {
            return Ok(Vec::new());
        }

        let Some(embedder) = self.embedder.as_ref() else {
            return Ok(Vec::new());
        };
        let Some(verb_service) = self.verb_service.as_ref() else {
            return Ok(Vec::new());
        };

        let query_embedding = embedder.embed_query(query).await?;

        // Scale fallback threshold for short queries
        let effective_fallback_threshold = {
            let word_count = query.split_whitespace().count();
            if word_count <= 2 {
                (self.fallback_threshold - 0.15).max(0.35)
            } else if word_count <= 4 {
                (self.fallback_threshold - 0.10).max(0.40)
            } else {
                self.fallback_threshold
            }
        };

        if let Some(allowed) = allowed_verbs {
            if !allowed.is_empty() && allowed.len() <= 100 {
                let verb_fqns: Vec<String> = allowed.iter().cloned().collect();
                return self
                    .search_patterns_constrained(
                        verb_service,
                        &query_embedding,
                        limit,
                        &verb_fqns,
                        effective_fallback_threshold,
                    )
                    .await;
            }
        }

        self.search_global_semantic_with_embedding(
            &query_embedding,
            limit,
            None,
            allowed_verbs,
            effective_fallback_threshold,
        )
        .await
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
                    journey: None,
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
                journey: None,
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
    /// Uses `fallback_threshold` (0.55) instead of hardcoded 0.5.
    ///
    /// Issue I fix: Also fetches from agent.invocation_phrases (learned) and unions.
    /// Search embedding space with 2-strategy selection:
    ///
    /// 1. **Constrained** (best): If `allowed_verbs` is provided with ≤100 verbs,
    ///    search ONLY patterns for those verbs (`verb_name = ANY($verbs)`).
    ///    This is the SemOS-scoped resolution path — 40-100x search space reduction.
    ///
    /// 2. **Full-space** (fallback): Search all ~23K patterns.
    ///
    /// Each strategy falls back to the next if no results exceed `semantic_threshold`.
    async fn search_global_semantic_with_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
        _domain: Option<&str>,
        allowed_verbs: Option<&HashSet<String>>,
        fallback_threshold: f32,
    ) -> Result<Vec<VerbSearchResult>> {
        let verb_service = match &self.verb_service {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        // Strategy 1: Verb-set-constrained search (SemOS-scoped resolution)
        // When SessionVerbSurface provides ≤100 allowed verbs, search only those.
        if let Some(allowed) = allowed_verbs {
            if !allowed.is_empty() && allowed.len() <= 100 {
                let verb_fqns: Vec<String> = allowed.iter().cloned().collect();
                let constrained_results = self
                    .search_patterns_constrained(
                        verb_service,
                        query_embedding,
                        limit,
                        &verb_fqns,
                        fallback_threshold,
                    )
                    .await?;

                let has_good_result = constrained_results
                    .iter()
                    .any(|r| r.score >= self.semantic_threshold);
                if has_good_result {
                    tracing::info!(
                        allowed_count = allowed.len(),
                        results = constrained_results.len(),
                        top_score = constrained_results.first().map(|r| r.score),
                        "VerbSearch: SemOS-constrained embedding search succeeded"
                    );
                    return Ok(constrained_results);
                }
                tracing::debug!(
                    allowed_count = allowed.len(),
                    "VerbSearch: SemOS-constrained search found nothing above threshold, falling back to full space"
                );
            }
        }

        // Strategy 2: Full-space search (fallback)
        self.search_patterns_directly_scoped(
            verb_service,
            query_embedding,
            limit,
            None,
            fallback_threshold,
        )
        .await
    }

    /// Search patterns constrained to a specific set of verb FQNs.
    async fn search_patterns_constrained(
        &self,
        verb_service: &VerbService,
        query_embedding: &[f32],
        limit: usize,
        verb_fqns: &[String],
        fallback_threshold: f32,
    ) -> Result<Vec<VerbSearchResult>> {
        use std::collections::HashMap;

        let learned_matches = verb_service
            .find_global_learned_semantic_topk_constrained(
                query_embedding,
                fallback_threshold,
                limit,
                verb_fqns,
            )
            .await
            .unwrap_or_default();

        let pattern_matches = verb_service
            .search_verb_patterns_semantic_constrained(
                query_embedding,
                limit,
                fallback_threshold,
                verb_fqns,
            )
            .await
            .unwrap_or_default();

        let learned_results: Vec<VerbSearchResult> = learned_matches
            .into_iter()
            .map(|m| VerbSearchResult {
                verb: m.verb,
                score: m.similarity as f32,
                source: VerbSearchSource::GlobalLearned,
                matched_phrase: m.phrase,
                description: None,
                journey: None,
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
                journey: None,
            })
            .collect();

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

        let mut sorted: Vec<VerbSearchResult> = combined.into_values().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(limit);

        Ok(sorted)
    }

    /// Direct pattern search with optional domain scoping.
    async fn search_patterns_directly_scoped(
        &self,
        verb_service: &VerbService,
        query_embedding: &[f32],
        limit: usize,
        domain_prefix: Option<&str>,
        fallback_threshold: f32,
    ) -> Result<Vec<VerbSearchResult>> {
        use std::collections::HashMap;

        // Fetch top-k from BOTH sources, optionally scoped to domain
        let learned_matches = verb_service
            .find_global_learned_semantic_topk_scoped(
                query_embedding,
                fallback_threshold,
                limit,
                domain_prefix,
            )
            .await
            .unwrap_or_default();

        let pattern_matches = verb_service
            .search_verb_patterns_semantic_scoped(
                query_embedding,
                limit,
                fallback_threshold,
                domain_prefix,
            )
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
                journey: None,
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
                journey: None,
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

    fn matches_entity_kind(&self, verb: &str, entity_kind: Option<&str>) -> bool {
        let Some(kind) = entity_kind.map(canonicalize_entity_kind) else {
            return true;
        };

        let Some(runtime_verb) = runtime_registry().get_by_name(verb) else {
            return true;
        };

        if runtime_verb.subject_kinds.is_empty() {
            return true;
        }

        runtime_verb
            .subject_kinds
            .iter()
            .map(|sk| canonicalize_entity_kind(sk))
            .any(|sk| sk == kind)
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
        if let Some(schema) = registry.get(&query_lower) {
            results.push(VerbSearchResult {
                verb: query_lower.clone(),
                score: 1.0,
                source: VerbSearchSource::Macro,
                matched_phrase: schema.ui.label.clone(),
                description: Some(schema.ui.description.clone()),
                journey: None,
            });
            return results; // Exact FQN is definitive
        }

        // 2. Exact label match (case-insensitive)
        for (fqn, schema) in registry.all() {
            if schema.ui.label.to_lowercase() == query_lower {
                results.push(VerbSearchResult {
                    verb: fqn.clone(),
                    score: 1.0,
                    source: VerbSearchSource::Macro,
                    matched_phrase: schema.ui.label.clone(),
                    description: Some(schema.ui.description.clone()),
                    journey: None,
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

        for (fqn, schema) in registry.all() {
            let label_lower = schema.ui.label.to_lowercase();
            let desc_lower = schema.ui.description.to_lowercase();
            let fqn_lower = fqn.to_lowercase();

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
                    verb: fqn.clone(),
                    score,
                    source: VerbSearchSource::Macro,
                    matched_phrase: schema.ui.label.clone(),
                    description: Some(schema.ui.description.clone()),
                    journey: None,
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
        let results = searcher
            .search("create cbu", None, None, None, 5, None, None, None)
            .await
            .unwrap();

        // Minimal searcher has no DB, so no results
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_source_serialization() {
        let sources = vec![
            VerbSearchSource::UserLearnedExact,
            VerbSearchSource::UserLearnedSemantic,
            VerbSearchSource::LearnedExact,
            VerbSearchSource::GlobalLearned,
            VerbSearchSource::PatternEmbedding,
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            println!("{:?} -> {}", source, json);
        }
    }

    #[test]
    fn test_matches_entity_kind_allows_empty_subject_kinds() {
        let searcher = HybridVerbSearcher::minimal();
        assert!(searcher.matches_entity_kind("session.help", Some("company")));
    }

    #[test]
    fn test_matches_entity_kind_filters_mismatched_subject_kind() {
        let searcher = HybridVerbSearcher::minimal();
        assert!(!searcher.matches_entity_kind("deal.create", Some("company")));
    }

    #[test]
    fn test_matches_entity_kind_canonicalizes_aliases() {
        let searcher = HybridVerbSearcher::minimal();
        assert!(searcher.matches_entity_kind("deal.create", Some("deal-record")));
    }

    // =========================================================================
    // Issue J/D Acceptance Tests - Ambiguity Detection
    // =========================================================================

    #[test]
    fn test_normalize_candidates_dedupes_and_sorts() {
        // Simulate tier-by-tier appending: same verb appears twice with different scores
        let candidates = vec![
            VerbSearchResult {
                verb: "deal.create".to_string(),
                score: 0.82, // lower score, added first (tier 3)
                source: VerbSearchSource::GlobalLearned,
                matched_phrase: "make a deal".to_string(),
                description: None,
                journey: None,
            },
            VerbSearchResult {
                verb: "deal.ensure".to_string(),
                score: 0.80,
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: "ensure deal".to_string(),
                description: None,
                journey: None,
            },
            VerbSearchResult {
                verb: "deal.create".to_string(),
                score: 0.91, // higher score, added later (tier 6)
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: "create deal".to_string(),
                description: None,
                journey: None,
            },
        ];

        let normalized = normalize_candidates(candidates, 5);

        // Should have 2 unique verbs
        assert_eq!(normalized.len(), 2);

        // First should be deal.create with the HIGHER score (0.91)
        assert_eq!(normalized[0].verb, "deal.create");
        assert!((normalized[0].score - 0.91).abs() < 0.001);
        assert!(matches!(
            normalized[0].source,
            VerbSearchSource::PatternEmbedding
        ));

        // Second should be deal.ensure
        assert_eq!(normalized[1].verb, "deal.ensure");
    }

    #[test]
    fn test_check_ambiguity_blocks_on_close_margin() {
        let threshold = 0.80;

        // Two candidates within margin, both above threshold
        let candidates = vec![
            VerbSearchResult {
                verb: "deal.create".to_string(),
                score: 0.85,
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: "create deal".to_string(),
                description: None,
                journey: None,
            },
            VerbSearchResult {
                verb: "deal.ensure".to_string(),
                score: 0.83, // margin = 0.02 < AMBIGUITY_MARGIN (0.05)
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: "ensure deal".to_string(),
                description: None,
                journey: None,
            },
        ];

        let outcome = check_ambiguity(&candidates, threshold);

        match outcome {
            VerbSearchOutcome::Ambiguous {
                top,
                runner_up,
                margin,
            } => {
                assert_eq!(top.verb, "deal.create");
                assert_eq!(runner_up.verb, "deal.ensure");
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
                verb: "deal.create".to_string(),
                score: 0.92,
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: "create deal".to_string(),
                description: None,
                journey: None,
            },
            VerbSearchResult {
                verb: "deal.ensure".to_string(),
                score: 0.82, // margin = 0.10 > AMBIGUITY_MARGIN (0.05)
                source: VerbSearchSource::PatternEmbedding,
                matched_phrase: "ensure deal".to_string(),
                description: None,
                journey: None,
            },
        ];

        let outcome = check_ambiguity(&candidates, threshold);

        match outcome {
            VerbSearchOutcome::Matched(result) => {
                assert_eq!(result.verb, "deal.create");
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
            verb: "deal.create".to_string(),
            score: 0.75, // below semantic threshold, above fallback
            source: VerbSearchSource::PatternEmbedding,
            matched_phrase: "create deal".to_string(),
            description: None,
            journey: None,
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
            verb: "deal.create".to_string(),
            score: 0.50, // below fallback threshold (0.55)
            source: VerbSearchSource::PatternEmbedding,
            matched_phrase: "create deal".to_string(),
            description: None,
            journey: None,
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
            verb: "deal.create".to_string(),
            score: 0.90,
            source: VerbSearchSource::PatternEmbedding,
            matched_phrase: "create deal".to_string(),
            description: None,
            journey: None,
        }];

        let outcome = check_ambiguity(&candidates, threshold);

        match outcome {
            VerbSearchOutcome::Matched(result) => {
                assert_eq!(result.verb, "deal.create");
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    // =========================================================================
    // Macro Search Tests
    // =========================================================================

    /// Build a test macro registry with a single structure.setup macro
    fn test_macro_registry() -> MacroRegistry {
        use crate::dsl_v2::macros::{
            MacroArgs, MacroKind, MacroRouting, MacroSchema, MacroTarget, MacroUi,
        };

        let mut registry = MacroRegistry::new();
        registry.add(
            "structure.setup".to_string(),
            MacroSchema {
                id: None,
                kind: MacroKind::Macro,
                tier: None,
                aliases: vec![],
                taxonomy: None,
                ui: MacroUi {
                    label: "Set up Structure".to_string(),
                    description: "Create a new fund or mandate structure".to_string(),
                    target_label: "Structure".to_string(),
                },
                routing: MacroRouting {
                    mode_tags: vec!["onboarding".to_string()],
                    operator_domain: Some("structure".to_string()),
                },
                target: MacroTarget {
                    operates_on: "client-ref".to_string(),
                    produces: Some("structure-ref".to_string()),
                    allowed_structure_types: vec![],
                },
                args: MacroArgs {
                    style: crate::dsl_v2::macros::ArgStyle::Keyworded,
                    required: Default::default(),
                    optional: Default::default(),
                },
                required_roles: vec![],
                optional_roles: vec![],
                docs_bundle: None,
                prereqs: vec![],
                expands_to: vec![],
                sets_state: vec![],
                unlocks: vec![],
            },
        );
        registry
    }

    #[test]
    fn test_macro_search_exact_fqn() {
        let registry = test_macro_registry();
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
        let registry = test_macro_registry();
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
        let registry = test_macro_registry();
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
        let registry = test_macro_registry();
        let searcher =
            HybridVerbSearcher::minimal().with_macro_registry(std::sync::Arc::new(registry));

        // No match - completely different query
        let results = searcher.search_macros("something completely different", 5);
        assert!(results.is_empty());
    }

    // =========================================================================
    // F14 Regression Tests — SemOS tollgate: Tier -2A ScenarioIndex matched path
    // MUST filter against allowed_verbs before returning a candidate. Before the
    // fix this path bypassed the CCIR check, allowing scenario-routed verbs to
    // slip past SemOS gating.
    // =========================================================================

    /// Minimal scenario YAML used by the F14 tests. One scenario that maps an
    /// utterance to a deterministic macro FQN via `kind: macro`. Because the
    /// scenario-matched code path pushes the route FQN as the `verb` field
    /// (even for macro routes), we can assert on that FQN directly.
    fn f14_scenario_yaml() -> &'static str {
        r#"
scenarios:
  - id: lux-sicav-setup
    title: Luxembourg UCITS SICAV Setup
    modes: [onboarding, structure]
    requires:
      any_of: [compound_action, jurisdiction_structure_pair]
    signals:
      actions: [onboard, "set up", establish, launch]
      jurisdictions: [LU]
      nouns_any: [sicav, ucits, fund]
    routes:
      kind: macro
      macro_fqn: struct.lux.ucits.sicav
    explain:
      summary: "Full Luxembourg UCITS SICAV setup"
"#
    }

    fn f14_searcher() -> HybridVerbSearcher {
        let idx = ScenarioIndex::from_yaml_str(f14_scenario_yaml()).unwrap();
        HybridVerbSearcher::minimal().with_scenario_index(std::sync::Arc::new(idx))
    }

    /// Scenario matches, and its target FQN is in `allowed_verbs`.
    /// Expect the verb to be returned with `ScenarioIndex` source and score 1.05.
    #[tokio::test]
    async fn test_f14_scenario_matched_verb_in_allowed_set_passes() {
        let searcher = f14_searcher();
        let mut allowed = HashSet::new();
        allowed.insert("struct.lux.ucits.sicav".to_string());
        allowed.insert("other.verb".to_string());

        let results = searcher
            .search(
                "Onboard a Luxembourg SICAV",
                None,
                None,
                None,
                5,
                Some(&allowed),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1, "expected single scenario match");
        assert_eq!(results[0].verb, "struct.lux.ucits.sicav");
        assert!(matches!(results[0].source, VerbSearchSource::ScenarioIndex));
        assert!((results[0].score - 1.05).abs() < 0.001);
    }

    /// Scenario matches, but its target FQN is NOT in `allowed_verbs`.
    /// **P0 regression test**: before the F14 fix this verb leaked past the
    /// filter. After the fix, the filter rejects it at the Matched path, and
    /// the search falls through lower tiers (which have no matches in the
    /// minimal searcher), producing an empty result set.
    #[tokio::test]
    async fn test_f14_scenario_matched_verb_not_in_allowed_set_filtered() {
        let searcher = f14_searcher();
        let mut allowed = HashSet::new();
        allowed.insert("some.other.verb".to_string()); // deliberately NOT the scenario FQN

        let results = searcher
            .search(
                "Onboard a Luxembourg SICAV",
                None,
                None,
                None,
                5,
                Some(&allowed),
                None,
                None,
            )
            .await
            .unwrap();

        assert!(
            results.is_empty(),
            "P0 bypass regressed: scenario-routed verb {:?} returned despite being outside \
             allowed_verbs. This means F14 has regressed — the Tier -2A Matched path is \
             bypassing CCIR again.",
            results.iter().map(|r| &r.verb).collect::<Vec<_>>()
        );
    }

    /// When `allowed_verbs` is `None`, the existing semantics (fail-open) are
    /// preserved: the verb passes. This is intentional at the Tier -2A level —
    /// the `is_none_or` filter passes through None. The "fail-closed on None"
    /// semantic is out of scope for F14; see Slice 3.2 / F17 where the
    /// runbook compiler enforces fail-closed on envelope unavailability.
    #[tokio::test]
    async fn test_f14_scenario_matched_with_none_allowed_verbs_passes() {
        let searcher = f14_searcher();

        let results = searcher
            .search(
                "Onboard a Luxembourg SICAV",
                None,
                None,
                None,
                5,
                None, // allowed_verbs = None
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verb, "struct.lux.ucits.sicav");
    }

    /// Regression test: the Tier -2A Ambiguous path already had an
    /// `is_none_or(|av| av.contains(&fqn))` filter before F14. This test
    /// pins that behaviour so a future refactor doesn't undo it.
    ///
    /// Uses a two-scenario YAML that both match "Onboard a Luxembourg SICAV"
    /// at the same score so the resolver produces an Ambiguous outcome.
    #[tokio::test]
    async fn test_f14_scenario_ambiguous_path_filters_allowed_verbs() {
        // Two identical-signal scenarios — resolver flags them ambiguous.
        let yaml = r#"
scenarios:
  - id: scenario-one
    title: Scenario One
    modes: [onboarding]
    requires:
      any_of: [compound_action]
    signals:
      actions: [onboard]
      jurisdictions: [LU]
      nouns_any: [sicav]
    routes:
      kind: macro
      macro_fqn: macro.scenario.one
  - id: scenario-two
    title: Scenario Two
    modes: [onboarding]
    requires:
      any_of: [compound_action]
    signals:
      actions: [onboard]
      jurisdictions: [LU]
      nouns_any: [sicav]
    routes:
      kind: macro
      macro_fqn: macro.scenario.two
"#;
        let idx = ScenarioIndex::from_yaml_str(yaml).unwrap();
        let searcher = HybridVerbSearcher::minimal().with_scenario_index(std::sync::Arc::new(idx));

        // Allow only scenario-two. Scenario-one must be filtered from the
        // Ambiguous result set.
        let mut allowed = HashSet::new();
        allowed.insert("macro.scenario.two".to_string());

        let results = searcher
            .search(
                "Onboard a Luxembourg SICAV",
                None,
                None,
                None,
                5,
                Some(&allowed),
                None,
                None,
            )
            .await
            .unwrap();

        // If both scenarios reach "Matched" individually we could see either
        // one-with-filter or two-without-filter. Assert the filter holds:
        // anything returned must be in the allowed set.
        for r in &results {
            assert!(
                allowed.contains(&r.verb),
                "scenario verb {} leaked past allowed_verbs filter",
                r.verb
            );
        }
    }
}

//! Intent Matcher Service
//!
//! Pure service for intent matching - NO side effects, NO session mutation.
//! This enables isolated testing and clean separation of concerns.
//!
//! The IntentMatcher combines:
//! - Verb search (semantic + exact match)
//! - Entity linking (mention extraction + resolution)
//! - DSL generation (LLM-based arg extraction)
//!
//! ## Deprecation Note (Phase 2)
//!
//! Direct use of `IntentMatcher` by the orchestrator is deprecated in favor
//! of `IntentService` (see `repl::intent_service`). `IntentService` wraps
//! `IntentMatcher` internally and adds clarification checking via
//! `sentences.clarify` templates and unified sentence generation.
//!
//! The `IntentMatcher` trait itself is NOT deprecated — it is still used
//! internally by `IntentService`.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;

use super::types::{
    EntityCandidate, EntityMention, IntentMatchResult, MatchContext, MatchDebugInfo, MatchOutcome,
    VerbCandidate,
};
use crate::mcp::verb_search::{
    check_ambiguity_with_fallback, HybridVerbSearcher, VerbSearchOutcome, VerbSearchResult,
};

// ============================================================================
// IntentMatcher Trait
// ============================================================================

/// Pure service for intent matching
///
/// This trait enables:
/// - Testing without a real session
/// - Swappable implementations (mock, cached, etc.)
/// - Clear API contract
#[async_trait]
pub trait IntentMatcher: Send + Sync {
    /// Match user intent from natural language
    ///
    /// This is a **pure function** with no side effects:
    /// - Does NOT mutate session state
    /// - Does NOT write to database
    /// - Only reads from DB/embeddings for matching
    ///
    /// # Arguments
    /// - `utterance`: User's natural language input
    /// - `context`: Immutable context for matching (client group, scope, etc.)
    ///
    /// # Returns
    /// - `IntentMatchResult` with outcome, candidates, and optionally generated DSL
    async fn match_intent(
        &self,
        utterance: &str,
        context: &MatchContext,
    ) -> Result<IntentMatchResult>;

    /// Check if a query looks like direct DSL input
    fn is_direct_dsl(&self, input: &str) -> bool {
        let trimmed = input.trim();
        // DSL starts with ( and ends with )
        trimmed.starts_with('(') && trimmed.ends_with(')')
    }
}

// ============================================================================
// HybridIntentMatcher Implementation
// ============================================================================

/// Default implementation combining verb search, entity linking, and DSL generation
pub struct HybridIntentMatcher {
    /// Verb searcher (semantic + exact + lexicon + macro)
    verb_searcher: Arc<HybridVerbSearcher>,

    /// Entity linking service (optional - graceful degradation if not available)
    entity_linker: Option<Arc<dyn EntityLinkingService>>,

    /// LLM client for DSL generation (optional - returns match without DSL if not available)
    llm_client: Option<Arc<dyn LlmClient>>,

    /// Semantic threshold for accepting a verb match
    semantic_threshold: f32,

    /// Fallback threshold for suggestions
    fallback_threshold: f32,

    /// Ambiguity margin for verb disambiguation
    #[allow(dead_code)]
    ambiguity_margin: f32,
}

impl HybridIntentMatcher {
    /// Create a new HybridIntentMatcher
    pub fn new(verb_searcher: Arc<HybridVerbSearcher>) -> Self {
        Self {
            verb_searcher,
            entity_linker: None,
            llm_client: None,
            semantic_threshold: 0.65,
            fallback_threshold: 0.55,
            ambiguity_margin: 0.05,
        }
    }

    /// Add entity linking service
    pub fn with_entity_linker(mut self, linker: Arc<dyn EntityLinkingService>) -> Self {
        self.entity_linker = Some(linker);
        self
    }

    /// Add LLM client for DSL generation
    pub fn with_llm_client(mut self, client: Arc<dyn LlmClient>) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Set semantic threshold
    pub fn with_semantic_threshold(mut self, threshold: f32) -> Self {
        self.semantic_threshold = threshold;
        self
    }

    /// Set fallback threshold
    pub fn with_fallback_threshold(mut self, threshold: f32) -> Self {
        self.fallback_threshold = threshold;
        self
    }

    /// Convert VerbSearchResult to VerbCandidate
    fn to_verb_candidate(&self, result: &VerbSearchResult) -> VerbCandidate {
        VerbCandidate {
            verb_fqn: result.verb.clone(),
            description: result.description.clone().unwrap_or_default(),
            score: result.score,
            example: Some(result.matched_phrase.clone()),
            domain: result.verb.split('.').next().map(|s| s.to_string()),
        }
    }

    /// Check if verb should auto-execute (navigation verbs)
    fn can_auto_execute(&self, verb: &str) -> bool {
        // Navigation verbs auto-execute without confirmation
        matches!(
            verb,
            "session.load-galaxy"
                | "session.load-cbu"
                | "session.load-jurisdiction"
                | "session.unload-cbu"
                | "session.clear"
                | "session.undo"
                | "session.redo"
                | "session.info"
                | "view.drill"
                | "view.surface"
                | "view.universe"
                | "view.cbu"
        )
    }
}

#[async_trait]
impl IntentMatcher for HybridIntentMatcher {
    async fn match_intent(
        &self,
        utterance: &str,
        context: &MatchContext,
    ) -> Result<IntentMatchResult> {
        let start = Instant::now();
        let mut timing = vec![];
        let mut notes = vec![];

        // Check for direct DSL input first
        if self.is_direct_dsl(utterance) {
            return Ok(IntentMatchResult {
                outcome: MatchOutcome::DirectDsl {
                    source: utterance.to_string(),
                },
                verb_candidates: vec![],
                entity_mentions: vec![],
                scope_candidates: None,
                generated_dsl: Some(utterance.to_string()),
                unresolved_refs: vec![],
                debug: Some(MatchDebugInfo {
                    timing: vec![("direct_dsl".to_string(), 0)],
                    search_tier: Some("direct".to_string()),
                    entity_linking: None,
                    notes: vec!["Direct DSL input detected".to_string()],
                }),
            });
        }

        // Step 1: Verb search
        let verb_search_start = Instant::now();
        let search_results = self
            .verb_searcher
            .search(
                utterance,
                context.user_id,
                context.domain_hint.as_deref(),
                10,
            )
            .await?;
        timing.push((
            "verb_search".to_string(),
            verb_search_start.elapsed().as_millis() as u64,
        ));

        // Convert to candidates
        let verb_candidates: Vec<VerbCandidate> = search_results
            .iter()
            .map(|r| self.to_verb_candidate(r))
            .collect();

        // Step 2: Check for ambiguity
        let outcome = check_ambiguity_with_fallback(
            &search_results,
            self.semantic_threshold,
            self.fallback_threshold,
        );

        match outcome {
            VerbSearchOutcome::NoMatch => {
                notes.push("No verbs matched fallback threshold".to_string());
                return Ok(IntentMatchResult {
                    outcome: MatchOutcome::NoMatch {
                        reason: "I couldn't find a matching action for your request.".to_string(),
                    },
                    verb_candidates,
                    entity_mentions: vec![],
                    scope_candidates: None,
                    generated_dsl: None,
                    unresolved_refs: vec![],
                    debug: Some(MatchDebugInfo {
                        timing,
                        search_tier: None,
                        entity_linking: None,
                        notes,
                    }),
                });
            }

            VerbSearchOutcome::Ambiguous {
                top,
                runner_up,
                margin,
            } => {
                notes.push(format!(
                    "Ambiguous: {} ({:.2}) vs {} ({:.2}), margin={:.3}",
                    top.verb, top.score, runner_up.verb, runner_up.score, margin
                ));
                return Ok(IntentMatchResult {
                    outcome: MatchOutcome::Ambiguous { margin },
                    verb_candidates,
                    entity_mentions: vec![],
                    scope_candidates: None,
                    generated_dsl: None,
                    unresolved_refs: vec![],
                    debug: Some(MatchDebugInfo {
                        timing,
                        search_tier: Some(format!("{:?}", top.source)),
                        entity_linking: None,
                        notes,
                    }),
                });
            }

            VerbSearchOutcome::Suggest { candidates } => {
                notes.push(format!(
                    "Suggest: {} candidates below threshold, offering menu",
                    candidates.len()
                ));
                // Return as ambiguous to show selection menu
                let margin = if candidates.len() >= 2 {
                    candidates[0].score - candidates[1].score
                } else {
                    0.0
                };
                return Ok(IntentMatchResult {
                    outcome: MatchOutcome::Ambiguous { margin },
                    verb_candidates,
                    entity_mentions: vec![],
                    scope_candidates: None,
                    generated_dsl: None,
                    unresolved_refs: vec![],
                    debug: Some(MatchDebugInfo {
                        timing,
                        search_tier: Some("suggest".to_string()),
                        entity_linking: None,
                        notes,
                    }),
                });
            }

            VerbSearchOutcome::Matched(top_result) => {
                notes.push(format!(
                    "Matched: {} ({:.2}) from {:?}",
                    top_result.verb, top_result.score, top_result.source
                ));

                // Step 3: Entity linking (if available)
                let entity_link_start = Instant::now();
                let entity_mentions = if let Some(linker) = &self.entity_linker {
                    match linker.extract_mentions(utterance, context).await {
                        Ok(mentions) => mentions,
                        Err(e) => {
                            notes.push(format!("Entity linking failed: {}", e));
                            vec![]
                        }
                    }
                } else {
                    vec![]
                };
                timing.push((
                    "entity_linking".to_string(),
                    entity_link_start.elapsed().as_millis() as u64,
                ));

                // Step 4: DSL generation (if LLM available)
                let dsl_gen_start = Instant::now();
                let generated_dsl = if let Some(llm) = &self.llm_client {
                    match llm
                        .generate_dsl(&top_result.verb, utterance, context, &entity_mentions)
                        .await
                    {
                        Ok(dsl) => Some(dsl),
                        Err(e) => {
                            notes.push(format!("DSL generation failed: {}", e));
                            None
                        }
                    }
                } else {
                    // No LLM - return simple DSL for navigation verbs
                    if self.can_auto_execute(&top_result.verb) {
                        Some(format!("({} )", top_result.verb))
                    } else {
                        None
                    }
                };
                timing.push((
                    "dsl_generation".to_string(),
                    dsl_gen_start.elapsed().as_millis() as u64,
                ));

                // Check for unresolved references
                // TODO: Parse generated DSL and check for <entity> refs
                let unresolved_refs = vec![];

                timing.push(("total".to_string(), start.elapsed().as_millis() as u64));

                return Ok(IntentMatchResult {
                    outcome: MatchOutcome::Matched {
                        verb: top_result.verb.clone(),
                        confidence: top_result.score,
                    },
                    verb_candidates,
                    entity_mentions,
                    scope_candidates: None,
                    generated_dsl,
                    unresolved_refs,
                    debug: Some(MatchDebugInfo {
                        timing,
                        search_tier: Some(format!("{:?}", top_result.source)),
                        entity_linking: None,
                        notes,
                    }),
                });
            }
        }
    }
}

// ============================================================================
// Supporting Traits (to be implemented by existing services)
// ============================================================================

/// Entity linking service trait
///
/// Implementations can wrap the existing EntityLinkingService from entity_linking module.
#[async_trait]
pub trait EntityLinkingService: Send + Sync {
    /// Extract entity mentions from utterance
    async fn extract_mentions(
        &self,
        utterance: &str,
        context: &MatchContext,
    ) -> Result<Vec<EntityMention>>;

    /// Resolve an entity reference
    async fn resolve_reference(
        &self,
        text: &str,
        expected_kind: Option<&str>,
        context: &MatchContext,
    ) -> Result<Vec<EntityCandidate>>;
}

/// LLM client trait for DSL generation
///
/// Implementations can wrap the existing LlmClient.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generate DSL from matched verb and utterance
    async fn generate_dsl(
        &self,
        verb: &str,
        utterance: &str,
        context: &MatchContext,
        entity_mentions: &[EntityMention],
    ) -> Result<String>;
}

// ============================================================================
// Stub Implementations (for graceful degradation)
// ============================================================================

/// Stub entity linking service (returns no entities)
pub struct StubEntityLinkingService;

#[async_trait]
impl EntityLinkingService for StubEntityLinkingService {
    async fn extract_mentions(
        &self,
        _utterance: &str,
        _context: &MatchContext,
    ) -> Result<Vec<EntityMention>> {
        Ok(vec![])
    }

    async fn resolve_reference(
        &self,
        _text: &str,
        _expected_kind: Option<&str>,
        _context: &MatchContext,
    ) -> Result<Vec<EntityCandidate>> {
        Ok(vec![])
    }
}

/// Stub LLM client (returns error)
pub struct StubLlmClient;

#[async_trait]
impl LlmClient for StubLlmClient {
    async fn generate_dsl(
        &self,
        _verb: &str,
        _utterance: &str,
        _context: &MatchContext,
        _entity_mentions: &[EntityMention],
    ) -> Result<String> {
        Err(anyhow::anyhow!("LLM client not configured"))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::verb_search::VerbSearchSource;

    /// Mock verb searcher for testing
    struct MockVerbSearcher {
        results: Vec<VerbSearchResult>,
    }

    impl MockVerbSearcher {
        fn new(results: Vec<VerbSearchResult>) -> Self {
            Self { results }
        }
    }

    fn make_result(verb: &str, score: f32) -> VerbSearchResult {
        VerbSearchResult {
            verb: verb.to_string(),
            score,
            source: VerbSearchSource::Semantic,
            matched_phrase: format!("test phrase for {}", verb),
            description: Some(format!("{} description", verb)),
        }
    }

    #[test]
    fn test_is_direct_dsl() {
        let matcher = HybridIntentMatcher::new(Arc::new(HybridVerbSearcher::minimal()));

        assert!(matcher.is_direct_dsl("(cbu.create :name \"test\")"));
        assert!(matcher.is_direct_dsl("  (session.load-cbu :cbu-id \"...\")  "));
        assert!(!matcher.is_direct_dsl("load the allianz book"));
        assert!(!matcher.is_direct_dsl("(incomplete"));
        assert!(!matcher.is_direct_dsl("not dsl)"));
    }

    #[test]
    fn test_can_auto_execute() {
        let matcher = HybridIntentMatcher::new(Arc::new(HybridVerbSearcher::minimal()));

        // Navigation verbs auto-execute
        assert!(matcher.can_auto_execute("session.load-galaxy"));
        assert!(matcher.can_auto_execute("session.undo"));
        assert!(matcher.can_auto_execute("view.drill"));

        // Other verbs require confirmation
        assert!(!matcher.can_auto_execute("cbu.create"));
        assert!(!matcher.can_auto_execute("entity.delete"));
    }

    #[test]
    fn test_to_verb_candidate() {
        let matcher = HybridIntentMatcher::new(Arc::new(HybridVerbSearcher::minimal()));

        let result = make_result("session.load-galaxy", 0.85);
        let candidate = matcher.to_verb_candidate(&result);

        assert_eq!(candidate.verb_fqn, "session.load-galaxy");
        assert_eq!(candidate.score, 0.85);
        assert_eq!(candidate.domain, Some("session".to_string()));
    }
}

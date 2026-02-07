//! Intent Matcher Trait
//!
//! Pure service contract for intent matching — NO side effects, NO session mutation.
//!
//! The `IntentMatcher` trait is used internally by `IntentService` (V2).
//! The `HybridIntentMatcher` implementation was removed in Phase 6 when the
//! V1 REPL was decommissioned. Tests use `MockIntentMatcher` directly.

use anyhow::Result;
use async_trait::async_trait;

use super::types::{IntentMatchResult, MatchContext, MatchOutcome};

#[cfg(feature = "vnext-repl")]
use super::context_stack::ContextStack;

#[cfg(feature = "vnext-repl")]
use super::scoring::{apply_ambiguity_policy, apply_pack_scoring, AmbiguityOutcome};

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
    async fn match_intent(
        &self,
        utterance: &str,
        context: &MatchContext,
    ) -> Result<IntentMatchResult>;

    /// Pack-scoped intent matching with context-aware re-ranking.
    ///
    /// Steps:
    /// 1. Delegate to `match_intent()` for raw semantic search
    /// 2. Apply pack scoring (boost/penalty/forbidden)
    /// 3. Apply ambiguity policy
    /// 4. Return re-ranked result
    ///
    /// This is a default method — implementations that want custom
    /// context-aware search can override it.
    #[cfg(feature = "vnext-repl")]
    async fn search_with_context(
        &self,
        utterance: &str,
        context: &MatchContext,
        stack: &ContextStack,
    ) -> Result<IntentMatchResult> {
        // Fast path: direct DSL input
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
                debug: None,
            });
        }

        // Step 1: Raw semantic search
        let mut result = self.match_intent(utterance, context).await?;

        // Step 2: Apply pack scoring to candidates
        apply_pack_scoring(&mut result.verb_candidates, stack);

        // Step 3: Apply ambiguity policy and update outcome
        let outcome = apply_ambiguity_policy(&result.verb_candidates);
        result.outcome = match outcome {
            AmbiguityOutcome::NoMatch => MatchOutcome::NoMatch {
                reason: "No verb matched after pack scoring".to_string(),
            },
            AmbiguityOutcome::Confident { verb, score } => MatchOutcome::Matched {
                verb,
                confidence: score,
            },
            AmbiguityOutcome::Ambiguous { margin, .. } => MatchOutcome::Ambiguous { margin },
            AmbiguityOutcome::Proposed { verb, score } => MatchOutcome::Matched {
                verb,
                confidence: score,
            },
        };

        Ok(result)
    }

    /// Check if a query looks like direct DSL input
    fn is_direct_dsl(&self, input: &str) -> bool {
        let trimmed = input.trim();
        trimmed.starts_with('(') && trimmed.ends_with(')')
    }
}

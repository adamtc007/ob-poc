//! Intent Matcher Trait
//!
//! Pure service contract for intent matching — NO side effects, NO session mutation.
//!
//! The `IntentMatcher` trait is used internally by `IntentService` (V2).
//! The `HybridIntentMatcher` implementation was removed in Phase 6 when the
//! V1 REPL was decommissioned. Tests use `MockIntentMatcher` directly.

use anyhow::Result;
use async_trait::async_trait;

use super::types::{IntentMatchResult, MatchContext};

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

    /// Check if a query looks like direct DSL input
    fn is_direct_dsl(&self, input: &str) -> bool {
        let trimmed = input.trim();
        trimmed.starts_with('(') && trimmed.ends_with(')')
    }
}

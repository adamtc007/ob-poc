//! IntentMatcher backed by HybridVerbSearcher
//!
//! Bridges the V1 semantic verb search pipeline (`HybridVerbSearcher`) to the
//! V2 REPL `IntentMatcher` trait. This enables the V2 REPL orchestrator to use
//! the same 10-tier verb search, Candle embeddings, macro registry, and lexicon
//! that the V1 agent chat pipeline uses.
//!
//! The bridge maps `VerbSearchResult` → `VerbCandidate` and derives the
//! `MatchOutcome` from the candidate list using the same ambiguity margin
//! logic that V1 uses.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use super::verb_search::HybridVerbSearcher;
use crate::repl::intent_matcher::IntentMatcher;
use crate::repl::types::{IntentMatchResult, MatchContext, MatchOutcome, VerbCandidate};

/// Ambiguity margin — if the gap between #1 and #2 is smaller than this,
/// the result is ambiguous and needs user disambiguation.
const AMBIGUITY_MARGIN: f32 = 0.05;

/// Minimum score to accept a match (BGE asymmetric mode).
const SEMANTIC_THRESHOLD: f32 = 0.55;

/// IntentMatcher implementation backed by HybridVerbSearcher.
///
/// This is the production implementation used by the V2 REPL orchestrator.
/// It wraps the existing `HybridVerbSearcher` which provides 10-tier
/// verb search with Candle embeddings, learned phrases, macro registry,
/// and lexicon.
pub struct VerbSearchIntentMatcher {
    searcher: Arc<HybridVerbSearcher>,
}

impl VerbSearchIntentMatcher {
    /// Create a new matcher wrapping a `HybridVerbSearcher`.
    pub fn new(searcher: Arc<HybridVerbSearcher>) -> Self {
        Self { searcher }
    }
}

#[async_trait]
impl IntentMatcher for VerbSearchIntentMatcher {
    async fn match_intent(
        &self,
        utterance: &str,
        context: &MatchContext,
    ) -> Result<IntentMatchResult> {
        // Map MatchContext fields to HybridVerbSearcher params
        let user_id: Option<Uuid> = context.user_id;
        let domain_filter: Option<String> = context.domain_hint.clone();

        // Run the 10-tier verb search, constrained by SemReg allowed verbs if present
        let results = self
            .searcher
            .search(
                utterance,
                user_id,
                domain_filter.as_deref(),
                10, // top-k candidates
                context.allowed_verbs.as_ref(),
            )
            .await?;

        // Map VerbSearchResult → VerbCandidate
        let candidates: Vec<VerbCandidate> = results
            .iter()
            .map(|r| VerbCandidate {
                verb_fqn: r.verb.clone(),
                description: r.description.clone().unwrap_or_default(),
                score: r.score,
                example: Some(r.matched_phrase.clone()),
                domain: r.verb.split('.').next().map(|d| d.to_string()),
            })
            .collect();

        // Derive outcome from candidate list
        let outcome = derive_outcome(&candidates);

        Ok(IntentMatchResult {
            outcome,
            verb_candidates: candidates,
            entity_mentions: vec![],
            scope_candidates: None,
            generated_dsl: None,
            unresolved_refs: vec![],
            debug: None,
        })
    }
}

/// Derive `MatchOutcome` from a sorted candidate list.
///
/// Uses the same ambiguity margin logic as the V1 pipeline:
/// - No candidates or all below threshold → `NoMatch`
/// - Single candidate above threshold → `Matched`
/// - Top two within `AMBIGUITY_MARGIN` → `Ambiguous`
/// - Clear winner (gap > margin) → `Matched`
fn derive_outcome(candidates: &[VerbCandidate]) -> MatchOutcome {
    if candidates.is_empty() {
        return MatchOutcome::NoMatch {
            reason: "No verb candidates found".to_string(),
        };
    }

    let top = &candidates[0];

    if top.score < SEMANTIC_THRESHOLD {
        return MatchOutcome::NoMatch {
            reason: format!(
                "Top score {:.3} below threshold {:.3}",
                top.score, SEMANTIC_THRESHOLD
            ),
        };
    }

    if candidates.len() >= 2 {
        let runner_up = &candidates[1];
        let margin = top.score - runner_up.score;

        if margin < AMBIGUITY_MARGIN && runner_up.score >= SEMANTIC_THRESHOLD {
            return MatchOutcome::Ambiguous { margin };
        }
    }

    MatchOutcome::Matched {
        verb: top.verb_fqn.clone(),
        confidence: top.score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_outcome_no_candidates() {
        let result = derive_outcome(&[]);
        assert!(matches!(result, MatchOutcome::NoMatch { .. }));
    }

    #[test]
    fn test_derive_outcome_clear_winner() {
        let candidates = vec![
            VerbCandidate {
                verb_fqn: "session.load-galaxy".to_string(),
                description: "Load galaxy".to_string(),
                score: 0.92,
                example: None,
                domain: None,
            },
            VerbCandidate {
                verb_fqn: "cbu.create".to_string(),
                description: "Create CBU".to_string(),
                score: 0.65,
                example: None,
                domain: None,
            },
        ];
        match derive_outcome(&candidates) {
            MatchOutcome::Matched { verb, confidence } => {
                assert_eq!(verb, "session.load-galaxy");
                assert!(confidence > 0.9);
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_derive_outcome_ambiguous() {
        let candidates = vec![
            VerbCandidate {
                verb_fqn: "session.load-galaxy".to_string(),
                description: "Load galaxy".to_string(),
                score: 0.82,
                example: None,
                domain: None,
            },
            VerbCandidate {
                verb_fqn: "session.load-cbu".to_string(),
                description: "Load CBU".to_string(),
                score: 0.79,
                example: None,
                domain: None,
            },
        ];
        match derive_outcome(&candidates) {
            MatchOutcome::Ambiguous { margin } => {
                assert!(margin < AMBIGUITY_MARGIN);
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    #[test]
    fn test_derive_outcome_below_threshold() {
        let candidates = vec![VerbCandidate {
            verb_fqn: "cbu.create".to_string(),
            description: "Create CBU".to_string(),
            score: 0.40,
            example: None,
            domain: None,
        }];
        assert!(matches!(
            derive_outcome(&candidates),
            MatchOutcome::NoMatch { .. }
        ));
    }

    #[test]
    fn test_derive_outcome_single_above_threshold() {
        let candidates = vec![VerbCandidate {
            verb_fqn: "cbu.create".to_string(),
            description: "Create CBU".to_string(),
            score: 0.88,
            example: None,
            domain: None,
        }];
        match derive_outcome(&candidates) {
            MatchOutcome::Matched { verb, confidence } => {
                assert_eq!(verb, "cbu.create");
                assert!(confidence > 0.85);
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }
}

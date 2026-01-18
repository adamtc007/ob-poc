//! Feedback capture service - integrates with intent matching

use super::{
    analysis::{AnalysisReport, FeedbackAnalyzer},
    repository::FeedbackRepository,
    sanitize::sanitize_input,
    types::*,
};
use crate::MatchResult;
use anyhow::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Feedback capture service
pub struct FeedbackService {
    repository: FeedbackRepository,
    analyzer: FeedbackAnalyzer,
    /// Cache of known entity names for sanitization
    known_entities: RwLock<Vec<String>>,
}

impl FeedbackService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repository: FeedbackRepository::new(pool.clone()),
            analyzer: FeedbackAnalyzer::new(pool),
            known_entities: RwLock::new(Vec::new()),
        }
    }

    /// Update known entities cache (call periodically or on entity load)
    pub async fn update_known_entities(&self, entities: Vec<String>) {
        let mut cache = self.known_entities.write().await;
        *cache = entities;
    }

    /// Capture an intent match result
    #[allow(clippy::too_many_arguments)]
    pub async fn capture_match(
        &self,
        session_id: Uuid,
        user_input: &str,
        input_source: InputSource,
        match_result: Option<&MatchResult>,
        alternatives: &[MatchResult],
        graph_context: Option<&str>,
        workflow_phase: Option<&str>,
    ) -> Result<Uuid> {
        // Sanitize input
        let known = self.known_entities.read().await;
        let entity_refs: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
        let (sanitized_input, input_hash) = sanitize_input(user_input, &entity_refs);
        drop(known);

        let interaction_id = Uuid::new_v4();

        let feedback = IntentFeedback {
            session_id,
            interaction_id,
            user_input: sanitized_input,
            user_input_hash: input_hash,
            input_source,
            matched_verb: match_result.map(|m| m.verb_name.clone()),
            match_score: match_result.map(|m| m.similarity),
            match_confidence: match_result.map(|m| MatchConfidence::from_score(m.similarity)),
            semantic_score: match_result.map(|m| m.similarity), // For now, same as match_score
            phonetic_score: None, // TODO: Track phonetic separately if needed
            alternatives: alternatives
                .iter()
                .take(5)
                .map(|a| Alternative {
                    verb: a.verb_name.clone(),
                    score: a.similarity,
                })
                .collect(),
            graph_context: graph_context.map(String::from),
            workflow_phase: workflow_phase.map(String::from),
        };

        self.repository.capture(&feedback).await?;

        Ok(interaction_id)
    }

    /// Record the outcome of an interaction
    pub async fn record_outcome(
        &self,
        interaction_id: Uuid,
        outcome: Outcome,
        outcome_verb: Option<String>,
        correction_input: Option<String>,
        time_to_outcome_ms: Option<i32>,
    ) -> Result<bool> {
        let update = OutcomeUpdate {
            interaction_id,
            outcome,
            outcome_verb,
            correction_input,
            time_to_outcome_ms,
        };

        self.repository.record_outcome(&update).await
    }

    /// Run analysis and get report
    pub async fn analyze(&self, days_back: i32) -> Result<AnalysisReport> {
        self.analyzer.run_full_analysis(days_back).await
    }

    /// Expire stale pending interactions
    pub async fn expire_pending(&self, older_than_minutes: i32) -> Result<u64> {
        self.repository.expire_pending(older_than_minutes).await
    }

    /// Get count of pending interactions
    pub async fn count_pending(&self) -> Result<i64> {
        self.repository.count_pending().await
    }

    /// Get the underlying analyzer for advanced queries
    pub fn analyzer(&self) -> &FeedbackAnalyzer {
        &self.analyzer
    }

    /// Get the underlying repository for advanced operations
    pub fn repository(&self) -> &FeedbackRepository {
        &self.repository
    }
}

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
use tracing::warn;
use uuid::Uuid;

/// Feedback capture service
pub struct FeedbackService {
    pool: PgPool,
    repository: FeedbackRepository,
    analyzer: FeedbackAnalyzer,
    /// Cache of known entity names for sanitization
    known_entities: RwLock<Vec<String>>,
}

impl FeedbackService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: pool.clone(),
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
        self.record_outcome_with_dsl(
            interaction_id,
            outcome,
            outcome_verb,
            correction_input,
            time_to_outcome_ms,
            None,
            None,
            None,
        )
        .await
    }

    /// Record the outcome of an interaction with DSL diff tracking
    /// Also triggers learning signal recording for strong signals
    #[allow(clippy::too_many_arguments)]
    pub async fn record_outcome_with_dsl(
        &self,
        interaction_id: Uuid,
        outcome: Outcome,
        outcome_verb: Option<String>,
        correction_input: Option<String>,
        time_to_outcome_ms: Option<i32>,
        generated_dsl: Option<String>,
        final_dsl: Option<String>,
        user_edits: Option<serde_json::Value>,
    ) -> Result<bool> {
        let update = OutcomeUpdate {
            interaction_id,
            outcome,
            outcome_verb: outcome_verb.clone(),
            correction_input,
            time_to_outcome_ms,
            generated_dsl,
            final_dsl,
            user_edits,
        };

        let updated = self.repository.record_outcome(&update).await?;

        if !updated {
            return Ok(false);
        }

        // Get original feedback to extract phrase and verb for learning
        if let Some((phrase, original_verb)) = self.get_feedback_for_learning(interaction_id).await
        {
            // Determine if this is a strong signal worth learning from
            let learning_info = match outcome {
                Outcome::Executed => {
                    // Success: learn phrase -> matched verb
                    let verb = outcome_verb.as_ref().or(original_verb.as_ref());
                    verb.map(|v| (v.clone(), true, "executed"))
                }
                Outcome::SelectedAlt => {
                    // User selected different verb - learn the correction
                    outcome_verb.map(|v| (v, true, "selected_alt"))
                }
                Outcome::Corrected => {
                    // Explicit correction - strong signal
                    outcome_verb.map(|v| (v, true, "corrected"))
                }
                Outcome::Rephrased | Outcome::Abandoned => {
                    // Weak signals - don't learn
                    None
                }
            };

            if let Some((verb, is_success, signal_type)) = learning_info {
                if let Err(e) = self
                    .record_learning_signal(&phrase, &verb, is_success, signal_type, None)
                    .await
                {
                    warn!(
                        "Failed to record learning signal for '{}' -> {}: {}",
                        phrase, verb, e
                    );
                }
            }
        }

        Ok(true)
    }

    /// Get phrase and verb from feedback for learning signal
    async fn get_feedback_for_learning(
        &self,
        interaction_id: Uuid,
    ) -> Option<(String, Option<String>)> {
        let result: Option<(String, Option<String>)> = sqlx::query_as(
            r#"SELECT user_input, matched_verb
               FROM "ob-poc".intent_feedback
               WHERE interaction_id = $1"#,
        )
        .bind(interaction_id)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        result
    }

    /// Record a learning signal from a resolved interaction
    /// Called when we have a strong signal (executed, selected_alt, corrected)
    /// Returns the candidate ID if recorded, None if rejected by quality gates
    pub async fn record_learning_signal(
        &self,
        phrase: &str,
        verb: &str,
        is_success: bool,
        signal_type: &str, // "executed", "selected_alt", "corrected"
        domain_hint: Option<&str>,
    ) -> Result<Option<i64>> {
        let result: Option<(Option<i64>,)> =
            sqlx::query_as(r#"SELECT agent.record_learning_signal($1, $2, $3, $4, $5)"#)
                .bind(phrase)
                .bind(verb)
                .bind(is_success)
                .bind(signal_type)
                .bind(domain_hint)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.and_then(|r| r.0))
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

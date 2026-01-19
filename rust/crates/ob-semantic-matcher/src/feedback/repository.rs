//! Repository for intent feedback capture and analysis

use super::types::*;
use anyhow::Result;
use sqlx::PgPool;

/// Repository for append-only feedback capture
pub struct FeedbackRepository {
    pool: PgPool,
}

impl FeedbackRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Capture an intent match (append-only insert)
    pub async fn capture(&self, feedback: &IntentFeedback) -> Result<()> {
        let alternatives_json = serde_json::to_value(&feedback.alternatives)?;
        let confidence_str = feedback.match_confidence.map(|c| c.as_str());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".intent_feedback (
                session_id, interaction_id, user_input, user_input_hash,
                input_source, matched_verb, match_score, match_confidence,
                semantic_score, phonetic_score, alternatives,
                graph_context, workflow_phase
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            )
            "#,
        )
        .bind(feedback.session_id)
        .bind(feedback.interaction_id)
        .bind(&feedback.user_input)
        .bind(&feedback.user_input_hash)
        .bind(feedback.input_source.as_str())
        .bind(&feedback.matched_verb)
        .bind(feedback.match_score)
        .bind(confidence_str)
        .bind(feedback.semantic_score)
        .bind(feedback.phonetic_score)
        .bind(&alternatives_json)
        .bind(&feedback.graph_context)
        .bind(&feedback.workflow_phase)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record outcome for an interaction
    /// Note: This updates the existing row - acceptable for outcome tracking
    pub async fn record_outcome(&self, update: &OutcomeUpdate) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".intent_feedback
            SET outcome = $2,
                outcome_verb = $3,
                correction_input = $4,
                time_to_outcome_ms = $5,
                generated_dsl = $6,
                final_dsl = $7,
                user_edits = $8
            WHERE interaction_id = $1
              AND outcome IS NULL
            "#,
        )
        .bind(update.interaction_id)
        .bind(update.outcome.as_str())
        .bind(&update.outcome_verb)
        .bind(&update.correction_input)
        .bind(update.time_to_outcome_ms)
        .bind(&update.generated_dsl)
        .bind(&update.final_dsl)
        .bind(&update.user_edits)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Mark stale pending interactions as abandoned
    /// Run periodically (e.g., every hour)
    pub async fn expire_pending(&self, older_than_minutes: i32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".intent_feedback
            SET outcome = 'abandoned'
            WHERE outcome IS NULL
              AND created_at < NOW() - make_interval(mins => $1)
            "#,
        )
        .bind(older_than_minutes)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get count of pending interactions (for monitoring)
    pub async fn count_pending(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "ob-poc".intent_feedback
            WHERE outcome IS NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Delete old feedback records (data retention)
    pub async fn delete_older_than_days(&self, days: i32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".intent_feedback
            WHERE created_at < NOW() - make_interval(days => $1)
            "#,
        )
        .bind(days)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

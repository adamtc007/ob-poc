//! Automatic pattern learning from feedback analysis

use super::types::AnalysisResult;
use anyhow::Result;
use sqlx::PgPool;
use tracing::{info, warn};

/// Pattern learner that auto-applies discovered patterns
pub struct PatternLearner {
    pool: PgPool,
}

impl PatternLearner {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Apply high-confidence pattern discoveries
    /// Only auto-applies patterns seen 5+ times with clear verb association
    pub async fn auto_apply_discoveries(
        &self,
        discoveries: &[AnalysisResult],
        min_occurrences: i64,
    ) -> Result<Vec<(String, String)>> {
        let mut applied = Vec::new();

        for discovery in discoveries {
            if let AnalysisResult::PatternDiscovery {
                user_input,
                verb,
                occurrence_count,
                avg_score,
            } = discovery
            {
                // Only auto-apply high-confidence discoveries
                if *occurrence_count >= min_occurrences && *avg_score > 0.5 {
                    match self.add_pattern(verb, user_input).await {
                        Ok(true) => {
                            applied.push((verb.clone(), user_input.clone()));
                            info!(
                                "Auto-applied pattern: '{}' → {} (seen {} times, avg score {:.2})",
                                user_input, verb, occurrence_count, avg_score
                            );
                        }
                        Ok(false) => {
                            // Pattern already exists or no matching verb
                        }
                        Err(e) => {
                            warn!("Failed to add pattern '{}' → {}: {}", user_input, verb, e);
                        }
                    }
                }
            }
        }

        Ok(applied)
    }

    /// Add a new pattern to verb_rag_metadata (will be picked up by embedding rebuild)
    async fn add_pattern(&self, verb: &str, pattern: &str) -> Result<bool> {
        // First check if the verb exists and pattern isn't already there
        let exists: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".verb_rag_metadata
                WHERE verb_full_name = $1
                  AND $2 = ANY(intent_patterns)
            )
            "#,
        )
        .bind(verb)
        .bind(pattern)
        .fetch_optional(&self.pool)
        .await?;

        if exists.map(|e| e.0).unwrap_or(false) {
            // Pattern already exists
            return Ok(false);
        }

        // Add pattern to intent_patterns array
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".verb_rag_metadata
            SET intent_patterns = array_append(
                COALESCE(intent_patterns, ARRAY[]::text[]),
                $2
            )
            WHERE verb_full_name = $1
              AND NOT ($2 = ANY(COALESCE(intent_patterns, ARRAY[]::text[])))
            "#,
        )
        .bind(verb)
        .bind(pattern)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() > 0 {
            // Mark as applied in analysis table
            sqlx::query(
                r#"
                UPDATE "ob-poc".intent_feedback_analysis
                SET applied = true
                WHERE analysis_type = 'pattern_discovery'
                  AND data->>'user_input' = $1
                  AND data->>'verb' = $2
                  AND NOT applied
                "#,
            )
            .bind(pattern)
            .bind(verb)
            .execute(&self.pool)
            .await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get count of patterns that need embedding rebuild
    pub async fn count_pending_embeddings(&self) -> Result<i64> {
        // Count patterns in verb_rag_metadata that don't have embeddings
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT unnest)
            FROM "ob-poc".verb_rag_metadata,
                 LATERAL unnest(intent_patterns) as unnest
            WHERE NOT EXISTS (
                SELECT 1 FROM "ob-poc".verb_pattern_embeddings e
                WHERE e.verb_name = verb_rag_metadata.verb_full_name
                  AND e.pattern_phrase = unnest
            )
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }
}

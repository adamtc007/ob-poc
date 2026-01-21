//! Automatic pattern learning from feedback analysis
//!
//! Learns new intent_patterns from user feedback and adds them to dsl_verbs.
//! These patterns are then picked up by populate_embeddings to create searchable vectors.
//!
//! Architecture:
//!   User feedback → PatternLearner → dsl_verbs.intent_patterns → populate_embeddings → verb_pattern_embeddings

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

    /// Get reference to the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
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

    /// Add a new pattern to dsl_verbs.intent_patterns
    ///
    /// Uses the add_learned_pattern SQL function which:
    /// - Checks if verb exists
    /// - Checks if pattern already present
    /// - Appends to intent_patterns array if not
    async fn add_pattern(&self, verb: &str, pattern: &str) -> Result<bool> {
        // Use the SQL function we created in migration 037
        let result: (bool,) = sqlx::query_as(r#"SELECT "ob-poc".add_learned_pattern($1, $2)"#)
            .bind(verb)
            .bind(pattern)
            .fetch_one(&self.pool)
            .await?;

        if result.0 {
            // Mark as applied in analysis table if it exists
            let _ = sqlx::query(
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
            .await;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get count of patterns in dsl_verbs that don't have embeddings yet
    pub async fn count_pending_embeddings(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".v_verb_intent_patterns v
            WHERE NOT EXISTS (
                SELECT 1 FROM "ob-poc".verb_pattern_embeddings e
                WHERE e.verb_name = v.verb_full_name
                  AND e.pattern_normalized = LOWER(TRIM(v.pattern))
            )
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Get list of verbs that have learned patterns (not from YAML)
    pub async fn get_learned_verbs(&self) -> Result<Vec<(String, i32)>> {
        let rows: Vec<(String, i32)> = sqlx::query_as(
            r#"
            SELECT full_name, array_length(intent_patterns, 1) as pattern_count
            FROM "ob-poc".dsl_verbs
            WHERE intent_patterns IS NOT NULL
              AND array_length(intent_patterns, 1) > 0
              AND source = 'learned'
            ORDER BY pattern_count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

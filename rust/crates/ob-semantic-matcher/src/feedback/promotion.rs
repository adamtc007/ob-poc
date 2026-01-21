//! Automatic pattern promotion from learning candidates
//!
//! Runs as a background job to:
//! 1. Expire pending outcomes
//! 2. Run collision checks on candidates
//! 3. Auto-promote qualified candidates
//! 4. Queue borderline candidates for review
//!
//! ## Thresholds
//!
//! | Parameter | Value | Rationale |
//! |-----------|-------|-----------|
//! | `min_occurrences` | 5 | Enough signal, not one-off |
//! | `min_success_rate` | 0.80 | 4/5 successful uses |
//! | `min_age_hours` | 24 | Cool-down for burst patterns |
//! | `collision_threshold` | 0.92 | Prevent verb confusion |

use anyhow::Result;
use sqlx::PgPool;
use tracing::{info, warn};

use crate::Embedder;

/// Promotion service for background pattern learning
pub struct PromotionService {
    pool: PgPool,
    embedder: Option<Embedder>,

    // Thresholds (configurable)
    min_occurrences: i32,
    min_success_rate: f32,
    min_age_hours: i32,
    collision_threshold: f32,
}

/// Candidate ready for promotion
#[derive(Debug, sqlx::FromRow)]
pub struct PromotableCandidate {
    pub id: i64,
    pub phrase: String,
    pub verb: String,
    pub occurrence_count: i32,
    pub success_count: i32,
    pub total_count: i32,
    pub success_rate: f32,
    pub domain_hint: Option<String>,
}

/// Candidate needing manual review
#[derive(Debug, sqlx::FromRow)]
pub struct ReviewCandidate {
    pub id: i64,
    pub phrase: String,
    pub verb: String,
    pub occurrence_count: i32,
    pub success_count: i32,
    pub total_count: i32,
    pub success_rate: f32,
    pub domain_hint: Option<String>,
    pub first_seen: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub collision_verb: Option<String>,
}

/// Report from a promotion cycle
#[derive(Debug, Default)]
pub struct PromotionReport {
    pub expired_outcomes: i64,
    pub promoted: Vec<String>,
    pub skipped: i32,
    pub collisions: i32,
    pub errors: i32,
}

impl PromotionReport {
    pub fn summary(&self) -> String {
        format!(
            "Promotion cycle: {} expired, {} promoted, {} skipped, {} collisions, {} errors",
            self.expired_outcomes,
            self.promoted.len(),
            self.skipped,
            self.collisions,
            self.errors
        )
    }
}

impl PromotionService {
    /// Create a new promotion service with default thresholds
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            embedder: None,
            min_occurrences: 5,
            min_success_rate: 0.80,
            min_age_hours: 24,
            collision_threshold: 0.92,
        }
    }

    /// Add embedder for semantic collision checking
    pub fn with_embedder(mut self, embedder: Embedder) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Configure minimum occurrences threshold
    pub fn with_min_occurrences(mut self, min: i32) -> Self {
        self.min_occurrences = min;
        self
    }

    /// Configure minimum success rate threshold
    pub fn with_min_success_rate(mut self, rate: f32) -> Self {
        self.min_success_rate = rate;
        self
    }

    /// Configure minimum age threshold in hours
    pub fn with_min_age_hours(mut self, hours: i32) -> Self {
        self.min_age_hours = hours;
        self
    }

    /// Configure collision threshold
    pub fn with_collision_threshold(mut self, threshold: f32) -> Self {
        self.collision_threshold = threshold;
        self
    }

    /// Run full promotion cycle
    pub async fn run_promotion_cycle(&self) -> Result<PromotionReport> {
        // 1. Expire stale pending outcomes
        let expired_outcomes = self.expire_pending_outcomes(30).await?;
        if expired_outcomes > 0 {
            info!("Expired {} pending outcomes", expired_outcomes);
        }

        // 2. Get promotable candidates
        let candidates = self.get_promotable_candidates().await?;
        if !candidates.is_empty() {
            info!("Found {} promotable candidates", candidates.len());
        }

        // 3. Run collision checks and promote
        let mut promoted = Vec::new();
        let mut skipped = 0;
        let mut collisions = 0;
        let mut errors = 0;

        for candidate in candidates {
            match self.try_promote(&candidate).await {
                Ok(PromoteResult::Promoted) => {
                    promoted.push(candidate.phrase.clone());
                    info!("Promoted: '{}' -> {}", candidate.phrase, candidate.verb);
                }
                Ok(PromoteResult::Collision(verb)) => {
                    collisions += 1;
                    info!(
                        "Collision: '{}' conflicts with {} (target: {})",
                        candidate.phrase, verb, candidate.verb
                    );
                }
                Ok(PromoteResult::Skipped) => {
                    skipped += 1;
                }
                Err(e) => {
                    warn!("Failed to promote '{}': {}", candidate.phrase, e);
                    errors += 1;
                }
            }
        }

        Ok(PromotionReport {
            expired_outcomes,
            promoted,
            skipped,
            collisions,
            errors,
        })
    }

    /// Expire pending outcomes older than N minutes
    async fn expire_pending_outcomes(&self, older_than_minutes: i32) -> Result<i64> {
        let result: (i32,) = sqlx::query_as(r#"SELECT agent.expire_pending_outcomes($1)"#)
            .bind(older_than_minutes)
            .fetch_one(&self.pool)
            .await?;

        Ok(result.0 as i64)
    }

    /// Get candidates that meet promotion thresholds
    pub async fn get_promotable_candidates(&self) -> Result<Vec<PromotableCandidate>> {
        let candidates: Vec<PromotableCandidate> = sqlx::query_as(
            r#"SELECT id, phrase, verb, occurrence_count, success_count, total_count,
                      success_rate, domain_hint
               FROM agent.get_promotable_candidates($1, $2, $3, 50)"#,
        )
        .bind(self.min_occurrences)
        .bind(self.min_success_rate)
        .bind(self.min_age_hours)
        .fetch_all(&self.pool)
        .await?;

        Ok(candidates)
    }

    /// Get candidates that need manual review
    pub async fn get_review_candidates(
        &self,
        min_occurrences: i32,
        min_age_days: i32,
        limit: i32,
    ) -> Result<Vec<ReviewCandidate>> {
        let candidates: Vec<ReviewCandidate> = sqlx::query_as(
            r#"SELECT id, phrase, verb, occurrence_count, success_count, total_count,
                      success_rate, domain_hint, first_seen, last_seen, collision_verb
               FROM agent.get_review_candidates($1, $2, $3)"#,
        )
        .bind(min_occurrences)
        .bind(min_age_days)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(candidates)
    }

    /// Try to promote a candidate
    async fn try_promote(&self, candidate: &PromotableCandidate) -> Result<PromoteResult> {
        // Collision check (semantic similarity to other verbs)
        if let Some(embedder) = &self.embedder {
            match self.check_collision_safe(candidate, embedder).await {
                Ok(CollisionResult::Safe) => {}
                Ok(CollisionResult::Collision(verb)) => {
                    // Mark as collision detected
                    sqlx::query(
                        r#"UPDATE agent.learning_candidates
                           SET collision_safe = false,
                               collision_check_at = NOW(),
                               collision_verb = $2
                           WHERE id = $1"#,
                    )
                    .bind(candidate.id)
                    .bind(&verb)
                    .execute(&self.pool)
                    .await?;

                    return Ok(PromoteResult::Collision(verb));
                }
                Err(e) => {
                    warn!("Collision check failed for '{}': {}", candidate.phrase, e);
                    // Continue without collision check
                }
            }
        }

        // Apply promotion
        let result: (bool,) = sqlx::query_as(r#"SELECT agent.apply_promotion($1, 'system_auto')"#)
            .bind(candidate.id)
            .fetch_one(&self.pool)
            .await?;

        if result.0 {
            Ok(PromoteResult::Promoted)
        } else {
            Ok(PromoteResult::Skipped)
        }
    }

    /// Check if a candidate is collision-safe (doesn't match another verb too closely)
    async fn check_collision_safe(
        &self,
        candidate: &PromotableCandidate,
        embedder: &Embedder,
    ) -> Result<CollisionResult> {
        // Embed the candidate phrase
        let embedding = embedder.embed(&candidate.phrase)?;
        let embedding_vec = pgvector::Vector::from(embedding);

        // Check if it matches another verb too closely
        let collision: Option<(String, f32)> = sqlx::query_as(
            r#"
            SELECT verb_name, (1 - (embedding <=> $1))::real as similarity
            FROM "ob-poc".verb_pattern_embeddings
            WHERE verb_name != $2
              AND embedding IS NOT NULL
              AND (1 - (embedding <=> $1)) > $3
            ORDER BY similarity DESC
            LIMIT 1
            "#,
        )
        .bind(&embedding_vec)
        .bind(&candidate.verb)
        .bind(self.collision_threshold)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((colliding_verb, similarity)) = collision {
            warn!(
                "Collision detected: '{}' matches {} at {:.3} (target: {})",
                candidate.phrase, colliding_verb, similarity, candidate.verb
            );
            Ok(CollisionResult::Collision(colliding_verb))
        } else {
            Ok(CollisionResult::Safe)
        }
    }

    /// Manually approve a candidate for promotion
    pub async fn approve_candidate(&self, candidate_id: i64, actor: &str) -> Result<bool> {
        let result: (bool,) = sqlx::query_as(r#"SELECT agent.apply_promotion($1, $2)"#)
            .bind(candidate_id)
            .bind(actor)
            .fetch_one(&self.pool)
            .await?;

        Ok(result.0)
    }

    /// Reject a candidate and add to blocklist
    pub async fn reject_candidate(
        &self,
        candidate_id: i64,
        reason: &str,
        actor: &str,
    ) -> Result<bool> {
        let result: (bool,) = sqlx::query_as(r#"SELECT agent.reject_candidate($1, $2, $3)"#)
            .bind(candidate_id)
            .bind(reason)
            .bind(actor)
            .fetch_one(&self.pool)
            .await?;

        Ok(result.0)
    }

    /// Get learning health metrics for the last N weeks
    pub async fn get_health_metrics(&self, weeks: i32) -> Result<Vec<WeeklyHealthMetrics>> {
        let metrics: Vec<WeeklyHealthMetrics> = sqlx::query_as(
            r#"SELECT week, total_interactions, successes, corrections, no_matches,
                      false_positives, top1_hit_rate_pct, avg_success_score,
                      avg_correction_score, high_confidence, medium_confidence,
                      low_confidence, no_match_confidence
               FROM agent.v_learning_health_weekly
               LIMIT $1"#,
        )
        .bind(weeks)
        .fetch_all(&self.pool)
        .await?;

        Ok(metrics)
    }

    /// Get candidate pipeline status summary
    pub async fn get_pipeline_status(&self) -> Result<Vec<PipelineStatus>> {
        let status: Vec<PipelineStatus> = sqlx::query_as(
            r#"SELECT status, count, avg_occurrences, avg_success_rate, oldest, newest
               FROM agent.v_candidate_pipeline"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(status)
    }
}

/// Result of a collision check
enum CollisionResult {
    Safe,
    Collision(String),
}

/// Result of a promotion attempt
enum PromoteResult {
    Promoted,
    Collision(String),
    Skipped,
}

/// Weekly health metrics from the view
#[derive(Debug, sqlx::FromRow)]
pub struct WeeklyHealthMetrics {
    pub week: chrono::DateTime<chrono::Utc>,
    pub total_interactions: i64,
    pub successes: i64,
    pub corrections: i64,
    pub no_matches: i64,
    pub false_positives: i64,
    pub top1_hit_rate_pct: Option<f64>,
    pub avg_success_score: Option<f64>,
    pub avg_correction_score: Option<f64>,
    pub high_confidence: i64,
    pub medium_confidence: i64,
    pub low_confidence: i64,
    pub no_match_confidence: i64,
}

/// Pipeline status summary
#[derive(Debug, sqlx::FromRow)]
pub struct PipelineStatus {
    pub status: String,
    pub count: i64,
    pub avg_occurrences: Option<f64>,
    pub avg_success_rate: Option<f64>,
    pub oldest: Option<chrono::DateTime<chrono::Utc>>,
    pub newest: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promotion_report_summary() {
        let mut report = PromotionReport::default();
        report.expired_outcomes = 5;
        report.promoted.push("test phrase".to_string());
        report.skipped = 2;
        report.collisions = 1;
        report.errors = 0;

        let summary = report.summary();
        assert!(summary.contains("5 expired"));
        assert!(summary.contains("1 promoted"));
        assert!(summary.contains("2 skipped"));
        assert!(summary.contains("1 collisions"));
    }
}

//! Batch analysis of intent feedback for pattern learning

use super::types::*;
use anyhow::Result;
use chrono::NaiveDate;
use sqlx::PgPool;

/// Analyzer for batch feedback analysis
pub struct FeedbackAnalyzer {
    pool: PgPool,
}

impl FeedbackAnalyzer {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Discover new patterns from successful executions
    /// Finds inputs that led to execution but aren't in current patterns
    pub async fn discover_patterns(
        &self,
        min_occurrences: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, String, i64, f32)> = sqlx::query_as(
            r#"
            SELECT
                f.user_input,
                f.outcome_verb,
                COUNT(*) as occurrence_count,
                AVG(f.match_score)::real as avg_score
            FROM "ob-poc".intent_feedback f
            WHERE f.outcome IN ('executed', 'selected_alt')
              AND f.outcome_verb IS NOT NULL
              AND f.created_at > NOW() - make_interval(days => $2)
              -- Exclude inputs already in patterns
              AND NOT EXISTS (
                  SELECT 1 FROM "ob-poc".verb_pattern_embeddings p
                  WHERE p.verb_name = f.outcome_verb
                    AND LOWER(p.pattern_phrase) = LOWER(f.user_input)
              )
            GROUP BY f.user_input, f.outcome_verb
            HAVING COUNT(*) >= $1
            ORDER BY COUNT(*) DESC
            LIMIT 100
            "#,
        )
        .bind(min_occurrences)
        .bind(days_back)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(user_input, verb, occurrence_count, avg_score)| {
                AnalysisResult::PatternDiscovery {
                    user_input,
                    verb,
                    occurrence_count,
                    avg_score,
                }
            })
            .collect())
    }

    /// Find confusion pairs (verb A matched but user wanted verb B)
    pub async fn find_confusion_pairs(
        &self,
        min_confusions: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, String, i64, Vec<String>)> = sqlx::query_as(
            r#"
            SELECT
                matched_verb,
                outcome_verb,
                COUNT(*) as confusion_count,
                ARRAY_AGG(DISTINCT user_input ORDER BY user_input) as example_inputs
            FROM "ob-poc".intent_feedback
            WHERE outcome IN ('corrected', 'selected_alt')
              AND matched_verb IS NOT NULL
              AND outcome_verb IS NOT NULL
              AND matched_verb != outcome_verb
              AND created_at > NOW() - make_interval(days => $2)
            GROUP BY matched_verb, outcome_verb
            HAVING COUNT(*) >= $1
            ORDER BY COUNT(*) DESC
            LIMIT 50
            "#,
        )
        .bind(min_confusions)
        .bind(days_back)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(matched_verb, actual_verb, confusion_count, example_inputs)| {
                    AnalysisResult::ConfusionPair {
                        matched_verb,
                        actual_verb,
                        confusion_count,
                        example_inputs: example_inputs.into_iter().take(5).collect(),
                    }
                },
            )
            .collect())
    }

    /// Find gaps - inputs where matching failed or user abandoned
    pub async fn find_gaps(
        &self,
        min_occurrences: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, i64, Option<String>, Option<f32>)> = sqlx::query_as(
            r#"
            SELECT
                user_input,
                COUNT(*) as occurrence_count,
                matched_verb as best_match,
                MAX(match_score) as best_score
            FROM "ob-poc".intent_feedback
            WHERE (outcome = 'abandoned' OR match_confidence IN ('low', 'none'))
              AND created_at > NOW() - make_interval(days => $2)
            GROUP BY user_input, matched_verb
            HAVING COUNT(*) >= $1
            ORDER BY COUNT(*) DESC
            LIMIT 100
            "#,
        )
        .bind(min_occurrences)
        .bind(days_back)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(user_input, occurrence_count, best_match, best_score)| AnalysisResult::Gap {
                    user_input,
                    occurrence_count,
                    best_match,
                    best_score,
                },
            )
            .collect())
    }

    /// Find low-score matches that were actually correct
    /// These are candidates for new patterns to boost scores
    pub async fn find_low_score_successes(
        &self,
        max_score: f32,
        min_occurrences: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, String, f32, i64)> = sqlx::query_as(
            r#"
            SELECT
                user_input,
                outcome_verb,
                AVG(match_score)::real as avg_score,
                COUNT(*) as occurrence_count
            FROM "ob-poc".intent_feedback
            WHERE outcome = 'executed'
              AND match_score < $1
              AND match_score IS NOT NULL
              AND outcome_verb IS NOT NULL
              AND created_at > NOW() - make_interval(days => $3)
            GROUP BY user_input, outcome_verb
            HAVING COUNT(*) >= $2
            ORDER BY COUNT(*) DESC
            LIMIT 100
            "#,
        )
        .bind(max_score)
        .bind(min_occurrences)
        .bind(days_back)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(user_input, verb, avg_score, occurrence_count)| {
                AnalysisResult::LowScoreSuccess {
                    user_input,
                    verb,
                    avg_score,
                    occurrence_count,
                }
            })
            .collect())
    }

    /// Run full analysis and store results
    pub async fn run_full_analysis(&self, days_back: i32) -> Result<AnalysisReport> {
        let mut report = AnalysisReport::default();

        // Pattern discoveries (seen 3+ times)
        report.pattern_discoveries = self.discover_patterns(3, days_back).await?;

        // Confusion pairs (seen 2+ times)
        report.confusion_pairs = self.find_confusion_pairs(2, days_back).await?;

        // Gaps (seen 3+ times)
        report.gaps = self.find_gaps(3, days_back).await?;

        // Low score successes (score < 0.7, seen 3+ times)
        report.low_score_successes = self.find_low_score_successes(0.7, 3, days_back).await?;

        // Store results for review
        self.store_analysis_results(&report).await?;

        Ok(report)
    }

    /// Store analysis results for review
    async fn store_analysis_results(&self, report: &AnalysisReport) -> Result<()> {
        let today = chrono::Utc::now().date_naive();

        for result in &report.pattern_discoveries {
            self.store_single_result("pattern_discovery", today, result)
                .await?;
        }

        for result in &report.confusion_pairs {
            self.store_single_result("confusion_pair", today, result)
                .await?;
        }

        for result in &report.gaps {
            self.store_single_result("gap", today, result).await?;
        }

        for result in &report.low_score_successes {
            self.store_single_result("low_score_success", today, result)
                .await?;
        }

        Ok(())
    }

    async fn store_single_result(
        &self,
        analysis_type: &str,
        date: NaiveDate,
        result: &AnalysisResult,
    ) -> Result<()> {
        let data = serde_json::to_value(result)?;
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".intent_feedback_analysis
                (analysis_type, analysis_date, data)
            VALUES ($1, $2, $3)
            ON CONFLICT (analysis_type, analysis_date, data) DO NOTHING
            "#,
        )
        .bind(analysis_type)
        .bind(date)
        .bind(data)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete old analysis records (data retention)
    pub async fn delete_analysis_older_than_days(&self, days: i32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".intent_feedback_analysis
            WHERE analysis_date < CURRENT_DATE - $1
            "#,
        )
        .bind(days)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

/// Analysis report containing all findings
#[derive(Debug, Default)]
pub struct AnalysisReport {
    pub pattern_discoveries: Vec<AnalysisResult>,
    pub confusion_pairs: Vec<AnalysisResult>,
    pub gaps: Vec<AnalysisResult>,
    pub low_score_successes: Vec<AnalysisResult>,
}

impl AnalysisReport {
    pub fn summary(&self) -> String {
        format!(
            "Analysis: {} patterns, {} confusions, {} gaps, {} low-score successes",
            self.pattern_discoveries.len(),
            self.confusion_pairs.len(),
            self.gaps.len(),
            self.low_score_successes.len()
        )
    }

    pub fn is_empty(&self) -> bool {
        self.pattern_discoveries.is_empty()
            && self.confusion_pairs.is_empty()
            && self.gaps.is_empty()
            && self.low_score_successes.is_empty()
    }

    pub fn total_findings(&self) -> usize {
        self.pattern_discoveries.len()
            + self.confusion_pairs.len()
            + self.gaps.len()
            + self.low_score_successes.len()
    }
}

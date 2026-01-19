//! DSL Generation Log Repository
//!
//! Captures and queries agent generation iterations for:
//! - Audit trail of agent interactions
//! - Training data extraction for fine-tuning
//! - Few-shot RAG retrieval of successful examples
//! - Error recovery pattern learning
//! - Prompt effectiveness analysis
//!
//! # Learning Loop Integration
//!
//! The `intent_feedback_id` FK links to `intent_feedback` table for learning:
//! ```text
//! intent_feedback (phrase → verb match)
//!        ↓ FK
//! dsl_generation_log (LLM → DSL + execution outcome)
//!
//! Learning query: JOIN both to find false positives
//! (high confidence match → DSL generated → execution failed)
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// A single generation attempt within an iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationAttempt {
    pub attempt: i32,
    pub timestamp: DateTime<Utc>,
    pub prompt_template: Option<String>,
    pub prompt_text: String,
    pub raw_response: String,
    pub extracted_dsl: Option<String>,
    pub parse_result: ParseResult,
    pub lint_result: LintResult,
    pub compile_result: CompileResult,
    pub latency_ms: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
}

/// Result of parsing DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Result of linting DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Result of compiling DSL to execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub success: bool,
    pub error: Option<String>,
    pub step_count: i32,
}

/// Execution status for DSL (matches DB enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "\"ob-poc\".execution_status", rename_all = "lowercase")]
pub enum ExecutionStatus {
    Pending,
    Executed,
    Failed,
    Cancelled,
    Skipped,
}

/// Training pair: user intent → valid DSL
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrainingPair {
    pub user_intent: String,
    pub valid_dsl: Option<String>,
}

/// Correction pair: bad DSL + error → fixed DSL
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CorrectionPair {
    pub user_intent: String,
    pub bad_dsl: Option<String>,
    pub error_message: Option<String>,
    pub fixed_dsl: Option<String>,
}

/// Prompt template effectiveness statistics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PromptStats {
    pub template_name: Option<String>,
    pub total_uses: i64,
    pub first_try_success: i64,
    pub avg_attempts: f64,
    pub avg_latency_ms: Option<f64>,
}

/// Full generation log row
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GenerationLogRow {
    pub log_id: Uuid,
    pub instance_id: Option<Uuid>,
    pub user_intent: String,
    pub final_valid_dsl: Option<String>,
    pub iterations: serde_json::Value,
    pub domain_name: String,
    pub session_id: Option<Uuid>,
    pub cbu_id: Option<Uuid>,
    pub model_used: Option<String>,
    pub total_attempts: i32,
    pub success: bool,
    pub total_latency_ms: Option<i32>,
    pub total_input_tokens: Option<i32>,
    pub total_output_tokens: Option<i32>,
    pub created_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    // Learning loop fields (migration 039)
    pub intent_feedback_id: Option<i64>,
    pub execution_status: Option<ExecutionStatus>,
    pub execution_error: Option<String>,
    pub executed_at: Option<DateTime<Utc>>,
    pub affected_entity_ids: Option<Vec<Uuid>>,
}

/// Repository for generation log operations
pub struct GenerationLogRepository {
    pool: PgPool,
}

impl GenerationLogRepository {
    /// Create a new repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Start a new generation log entry
    /// Returns log_id for adding iterations
    ///
    /// # Arguments
    /// * `intent_feedback_id` - Links to intent_feedback for learning loop (optional)
    pub async fn start_log(
        &self,
        user_intent: &str,
        domain_name: &str,
        session_id: Option<Uuid>,
        cbu_id: Option<Uuid>,
        model_used: Option<&str>,
        intent_feedback_id: Option<i64>,
    ) -> Result<Uuid, sqlx::Error> {
        let log_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_generation_log
            (log_id, user_intent, domain_name, session_id, cbu_id, model_used, intent_feedback_id, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
            "#,
        )
        .bind(log_id)
        .bind(user_intent)
        .bind(domain_name)
        .bind(session_id)
        .bind(cbu_id)
        .bind(model_used)
        .bind(intent_feedback_id)
        .execute(&self.pool)
        .await?;

        Ok(log_id)
    }

    /// Add an iteration attempt to existing log
    pub async fn add_attempt(
        &self,
        log_id: Uuid,
        attempt: &GenerationAttempt,
    ) -> Result<(), sqlx::Error> {
        let attempt_json =
            serde_json::to_value(attempt).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET
                iterations = iterations || $2::jsonb,
                total_attempts = jsonb_array_length(iterations) + 1,
                total_latency_ms = COALESCE(total_latency_ms, 0) + COALESCE($3, 0),
                total_input_tokens = COALESCE(total_input_tokens, 0) + COALESCE($4, 0),
                total_output_tokens = COALESCE(total_output_tokens, 0) + COALESCE($5, 0)
            WHERE log_id = $1
            "#,
        )
        .bind(log_id)
        .bind(attempt_json)
        .bind(attempt.latency_ms)
        .bind(attempt.input_tokens)
        .bind(attempt.output_tokens)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark generation as successful and store final DSL
    pub async fn mark_success(
        &self,
        log_id: Uuid,
        final_dsl: &str,
        instance_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET
                success = true,
                final_valid_dsl = $2,
                instance_id = $3,
                completed_at = NOW()
            WHERE log_id = $1
            "#,
        )
        .bind(log_id)
        .bind(final_dsl)
        .bind(instance_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark generation as failed
    pub async fn mark_failed(&self, log_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET success = false, completed_at = NOW()
            WHERE log_id = $1
            "#,
        )
        .bind(log_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record execution outcome for learning loop
    ///
    /// Called after DSL execution completes (success or failure)
    pub async fn record_execution_outcome(
        &self,
        log_id: Uuid,
        status: ExecutionStatus,
        error: Option<&str>,
        affected_entities: Option<&[Uuid]>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET
                execution_status = $2,
                execution_error = $3,
                affected_entity_ids = $4,
                executed_at = NOW()
            WHERE log_id = $1
            "#,
        )
        .bind(log_id)
        .bind(status)
        .bind(error)
        .bind(affected_entities)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Link an existing log entry to intent_feedback (if not set at creation)
    pub async fn link_to_feedback(
        &self,
        log_id: Uuid,
        intent_feedback_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_generation_log
            SET intent_feedback_id = $2
            WHERE log_id = $1 AND intent_feedback_id IS NULL
            "#,
        )
        .bind(log_id)
        .bind(intent_feedback_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a generation log by ID
    pub async fn get_by_id(&self, log_id: Uuid) -> Result<Option<GenerationLogRow>, sqlx::Error> {
        sqlx::query_as::<_, GenerationLogRow>(
            r#"
            SELECT log_id, instance_id, user_intent, final_valid_dsl, iterations,
                   domain_name, session_id, cbu_id, model_used, total_attempts,
                   success, total_latency_ms, total_input_tokens, total_output_tokens,
                   created_at, completed_at,
                   intent_feedback_id, execution_status, execution_error, executed_at, affected_entity_ids
            FROM "ob-poc".dsl_generation_log
            WHERE log_id = $1
            "#,
        )
        .bind(log_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// List recent generation logs
    pub async fn list_recent(&self, limit: i32) -> Result<Vec<GenerationLogRow>, sqlx::Error> {
        sqlx::query_as::<_, GenerationLogRow>(
            r#"
            SELECT log_id, instance_id, user_intent, final_valid_dsl, iterations,
                   domain_name, session_id, cbu_id, model_used, total_attempts,
                   success, total_latency_ms, total_input_tokens, total_output_tokens,
                   created_at, completed_at,
                   intent_feedback_id, execution_status, execution_error, executed_at, affected_entity_ids
            FROM "ob-poc".dsl_generation_log
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Export successful intent → DSL pairs for fine-tuning
    pub async fn export_training_pairs(
        &self,
        domain: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<TrainingPair>, sqlx::Error> {
        sqlx::query_as::<_, TrainingPair>(
            r#"
            SELECT user_intent, final_valid_dsl as valid_dsl
            FROM "ob-poc".dsl_generation_log
            WHERE success = true
              AND final_valid_dsl IS NOT NULL
              AND ($1::text IS NULL OR domain_name = $1)
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(domain)
        .bind(limit.unwrap_or(1000))
        .fetch_all(&self.pool)
        .await
    }

    /// Export error correction pairs (bad DSL + error → fixed DSL)
    /// Returns pairs where first attempt failed but later succeeded
    pub async fn export_correction_pairs(
        &self,
        limit: Option<i32>,
    ) -> Result<Vec<CorrectionPair>, sqlx::Error> {
        sqlx::query_as::<_, CorrectionPair>(
            r#"
            WITH corrections AS (
                SELECT
                    log_id,
                    user_intent,
                    iterations,
                    final_valid_dsl,
                    total_attempts
                FROM "ob-poc".dsl_generation_log
                WHERE success = true AND total_attempts > 1
            )
            SELECT
                user_intent,
                iterations->0->>'extracted_dsl' as bad_dsl,
                COALESCE(
                    iterations->0->'lint_result'->'errors'->>0,
                    iterations->0->'parse_result'->>'error',
                    iterations->0->'compile_result'->>'error'
                ) as error_message,
                final_valid_dsl as fixed_dsl
            FROM corrections
            WHERE final_valid_dsl IS NOT NULL
            LIMIT $1
            "#,
        )
        .bind(limit.unwrap_or(500))
        .fetch_all(&self.pool)
        .await
    }

    /// Find similar successful generations for few-shot RAG
    /// Requires pg_trgm extension for similarity search
    pub async fn find_similar_examples(
        &self,
        intent: &str,
        domain: &str,
        limit: i32,
    ) -> Result<Vec<TrainingPair>, sqlx::Error> {
        sqlx::query_as::<_, TrainingPair>(
            r#"
            SELECT user_intent, final_valid_dsl as valid_dsl
            FROM "ob-poc".dsl_generation_log
            WHERE success = true
              AND final_valid_dsl IS NOT NULL
              AND domain_name = $2
              AND user_intent % $1
            ORDER BY similarity(user_intent, $1) DESC
            LIMIT $3
            "#,
        )
        .bind(intent)
        .bind(domain)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Get prompt template effectiveness stats
    pub async fn prompt_effectiveness_stats(&self) -> Result<Vec<PromptStats>, sqlx::Error> {
        sqlx::query_as::<_, PromptStats>(
            r#"
            SELECT
                iterations->0->>'prompt_template' as template_name,
                COUNT(*)::bigint as total_uses,
                SUM(CASE WHEN total_attempts = 1 AND success THEN 1 ELSE 0 END)::bigint as first_try_success,
                AVG(total_attempts)::float8 as avg_attempts,
                AVG(total_latency_ms)::float8 as avg_latency_ms
            FROM "ob-poc".dsl_generation_log
            WHERE iterations->0->>'prompt_template' IS NOT NULL
            GROUP BY iterations->0->>'prompt_template'
            ORDER BY total_uses DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get logs for a specific session
    pub async fn get_by_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<GenerationLogRow>, sqlx::Error> {
        sqlx::query_as::<_, GenerationLogRow>(
            r#"
            SELECT log_id, instance_id, user_intent, final_valid_dsl, iterations,
                   domain_name, session_id, cbu_id, model_used, total_attempts,
                   success, total_latency_ms, total_input_tokens, total_output_tokens,
                   created_at, completed_at,
                   intent_feedback_id, execution_status, execution_error, executed_at, affected_entity_ids
            FROM "ob-poc".dsl_generation_log
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get generation stats summary
    pub async fn get_stats_summary(&self) -> Result<Option<GenerationStatsSummary>, sqlx::Error> {
        sqlx::query_as::<_, GenerationStatsSummary>(
            r#"
            SELECT
                COUNT(*)::bigint as total_generations,
                SUM(CASE WHEN success THEN 1 ELSE 0 END)::bigint as successful,
                SUM(CASE WHEN NOT success THEN 1 ELSE 0 END)::bigint as failed,
                AVG(total_attempts)::float8 as avg_attempts,
                AVG(CASE WHEN success THEN total_latency_ms END)::float8 as avg_success_latency_ms,
                SUM(total_input_tokens)::bigint as total_input_tokens,
                SUM(total_output_tokens)::bigint as total_output_tokens
            FROM "ob-poc".dsl_generation_log
            WHERE completed_at IS NOT NULL
            "#,
        )
        .fetch_optional(&self.pool)
        .await
    }
}

/// Summary statistics for generation logs
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GenerationStatsSummary {
    pub total_generations: i64,
    pub successful: i64,
    pub failed: i64,
    pub avg_attempts: Option<f64>,
    pub avg_success_latency_ms: Option<f64>,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_result_serialization() {
        let result = ParseResult {
            success: true,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_lint_result_serialization() {
        let result = LintResult {
            valid: false,
            errors: vec!["Unknown verb".to_string()],
            warnings: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"valid\":false"));
        assert!(json.contains("Unknown verb"));
    }

    #[test]
    fn test_generation_attempt_serialization() {
        let attempt = GenerationAttempt {
            attempt: 1,
            timestamp: Utc::now(),
            prompt_template: Some("cbu_create_v1".to_string()),
            prompt_text: "Create a CBU".to_string(),
            raw_response: "I'll create...".to_string(),
            extracted_dsl: Some("(cbu.create :name \"Test\")".to_string()),
            parse_result: ParseResult {
                success: true,
                error: None,
            },
            lint_result: LintResult {
                valid: true,
                errors: vec![],
                warnings: vec![],
            },
            compile_result: CompileResult {
                success: true,
                error: None,
                step_count: 1,
            },
            latency_ms: Some(1500),
            input_tokens: Some(500),
            output_tokens: Some(200),
        };

        let json = serde_json::to_value(&attempt).unwrap();
        assert_eq!(json["attempt"], 1);
        assert_eq!(json["prompt_template"], "cbu_create_v1");
    }
}

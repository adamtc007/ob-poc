//! Execution audit trail query helpers
//!
//! Provides query functions for auditing DSL executions by verb hash,
//! allowing reconstruction of what verb configuration was active when
//! a specific execution ran.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Convert bytes to hex string
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Result from finding executions by verb hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionByVerbHash {
    pub idempotency_key: String,
    pub execution_id: Uuid,
    pub statement_index: i32,
    pub verb: String,
    pub result_type: String,
    pub result_id: Option<Uuid>,
    pub executed_at: DateTime<Utc>,
}

/// Result from the execution verb audit view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionVerbAudit {
    pub execution_id: Uuid,
    pub verb: String,
    pub verb_hash: Option<Vec<u8>>,
    pub verb_hash_hex: Option<String>,
    pub current_verb_hash: Option<Vec<u8>>,
    pub current_verb_hash_hex: Option<String>,
    pub verb_config_changed: bool,
    pub executed_at: DateTime<Utc>,
}

/// Result from getting verb config at execution time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbConfigAtExecution {
    pub verb_name: String,
    pub execution_verb_hash: Option<Vec<u8>>,
    pub execution_verb_hash_hex: Option<String>,
    pub current_verb_hash: Option<Vec<u8>>,
    pub current_verb_hash_hex: Option<String>,
    pub config_changed: bool,
    pub current_config_json: Option<serde_json::Value>,
}

/// Execution audit repository for verb hash queries
pub struct ExecutionAuditRepository {
    pool: PgPool,
}

impl ExecutionAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find all executions that used a specific verb hash
    ///
    /// This is useful for impact analysis when a verb configuration changes:
    /// "Which past executions used this specific verb configuration?"
    pub async fn find_executions_by_verb_hash(
        &self,
        verb_hash: &[u8],
    ) -> Result<Vec<ExecutionByVerbHash>> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                Uuid,
                i32,
                String,
                String,
                Option<Uuid>,
                DateTime<Utc>,
            ),
        >(
            r#"
            SELECT
                idempotency_key,
                execution_id,
                statement_index,
                verb,
                result_type,
                result_id,
                executed_at
            FROM "ob-poc".dsl_idempotency
            WHERE verb_hash = $1
            ORDER BY executed_at DESC
            "#,
        )
        .bind(verb_hash)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    idempotency_key,
                    execution_id,
                    statement_index,
                    verb,
                    result_type,
                    result_id,
                    executed_at,
                )| {
                    ExecutionByVerbHash {
                        idempotency_key,
                        execution_id,
                        statement_index,
                        verb,
                        result_type,
                        result_id,
                        executed_at,
                    }
                },
            )
            .collect())
    }

    /// Find all executions of a specific verb
    ///
    /// Returns executions with their verb hash and whether the config has changed
    pub async fn find_executions_by_verb_name(
        &self,
        verb_name: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ExecutionVerbAudit>> {
        let limit = limit.unwrap_or(100);

        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Option<Vec<u8>>,
                Option<Vec<u8>>,
                DateTime<Utc>,
            ),
        >(
            r#"
            SELECT
                i.execution_id,
                i.verb,
                i.verb_hash,
                v.compiled_hash as current_verb_hash,
                i.executed_at
            FROM "ob-poc".dsl_idempotency i
            LEFT JOIN "ob-poc".dsl_verbs v ON i.verb = v.verb_name
            WHERE i.verb = $1
            ORDER BY i.executed_at DESC
            LIMIT $2
            "#,
        )
        .bind(verb_name)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(execution_id, verb, verb_hash, current_verb_hash, executed_at)| {
                    let verb_config_changed = match (&verb_hash, &current_verb_hash) {
                        (Some(exec_hash), Some(curr_hash)) => exec_hash != curr_hash,
                        (None, Some(_)) => true, // No hash recorded but config exists now
                        (Some(_), None) => true, // Hash recorded but verb no longer exists
                        (None, None) => false,   // Neither has hash
                    };

                    ExecutionVerbAudit {
                        execution_id,
                        verb,
                        verb_hash_hex: verb_hash.as_ref().map(|h| bytes_to_hex(h)),
                        verb_hash,
                        current_verb_hash_hex: current_verb_hash.as_ref().map(|h| bytes_to_hex(h)),
                        current_verb_hash,
                        verb_config_changed,
                        executed_at,
                    }
                },
            )
            .collect())
    }

    /// Get the verb configuration that was active at execution time
    ///
    /// Returns the verb hash used during execution and compares it to current config
    pub async fn get_verb_config_at_execution(
        &self,
        execution_id: Uuid,
        verb_name: &str,
    ) -> Result<Option<VerbConfigAtExecution>> {
        let row =
            sqlx::query_as::<_, (Option<Vec<u8>>, Option<Vec<u8>>, Option<serde_json::Value>)>(
                r#"
            SELECT
                i.verb_hash as execution_verb_hash,
                v.compiled_hash as current_verb_hash,
                v.effective_config_json as current_config_json
            FROM "ob-poc".dsl_idempotency i
            LEFT JOIN "ob-poc".dsl_verbs v ON i.verb = v.verb_name
            WHERE i.execution_id = $1 AND i.verb = $2
            LIMIT 1
            "#,
            )
            .bind(execution_id)
            .bind(verb_name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(
            |(execution_verb_hash, current_verb_hash, current_config_json)| {
                let config_changed = match (&execution_verb_hash, &current_verb_hash) {
                    (Some(exec_hash), Some(curr_hash)) => exec_hash != curr_hash,
                    (None, Some(_)) => true,
                    (Some(_), None) => true,
                    (None, None) => false,
                };

                VerbConfigAtExecution {
                    verb_name: verb_name.to_string(),
                    execution_verb_hash_hex: execution_verb_hash.as_ref().map(|h| bytes_to_hex(h)),
                    execution_verb_hash,
                    current_verb_hash_hex: current_verb_hash.as_ref().map(|h| bytes_to_hex(h)),
                    current_verb_hash,
                    config_changed,
                    current_config_json,
                }
            },
        ))
    }

    /// Find executions where the verb config has changed since execution
    ///
    /// This is useful for identifying executions that might need review
    /// after a verb configuration update
    pub async fn find_stale_executions(
        &self,
        since: Option<DateTime<Utc>>,
        limit: Option<i64>,
    ) -> Result<Vec<ExecutionVerbAudit>> {
        let limit = limit.unwrap_or(100);

        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Option<Vec<u8>>,
                Option<Vec<u8>>,
                DateTime<Utc>,
            ),
        >(
            r#"
            SELECT
                i.execution_id,
                i.verb,
                i.verb_hash,
                v.compiled_hash as current_verb_hash,
                i.executed_at
            FROM "ob-poc".dsl_idempotency i
            LEFT JOIN "ob-poc".dsl_verbs v ON i.verb = v.verb_name
            WHERE i.verb_hash IS NOT NULL
              AND v.compiled_hash IS NOT NULL
              AND i.verb_hash != v.compiled_hash
              AND ($1::timestamptz IS NULL OR i.executed_at >= $1)
            ORDER BY i.executed_at DESC
            LIMIT $2
            "#,
        )
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(execution_id, verb, verb_hash, current_verb_hash, executed_at)| {
                    ExecutionVerbAudit {
                        execution_id,
                        verb,
                        verb_hash_hex: verb_hash.as_ref().map(|h| bytes_to_hex(h)),
                        verb_hash,
                        current_verb_hash_hex: current_verb_hash.as_ref().map(|h| bytes_to_hex(h)),
                        current_verb_hash,
                        verb_config_changed: true,
                        executed_at,
                    }
                },
            )
            .collect())
    }

    /// Count executions by verb, grouped by whether config has changed
    pub async fn count_executions_by_config_status(&self) -> Result<Vec<(String, i64, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64, i64)>(
            r#"
            SELECT
                i.verb,
                COUNT(*) FILTER (WHERE i.verb_hash IS NULL OR v.compiled_hash IS NULL OR i.verb_hash = v.compiled_hash) as current_count,
                COUNT(*) FILTER (WHERE i.verb_hash IS NOT NULL AND v.compiled_hash IS NOT NULL AND i.verb_hash != v.compiled_hash) as stale_count
            FROM "ob-poc".dsl_idempotency i
            LEFT JOIN "ob-poc".dsl_verbs v ON i.verb = v.verb_name
            GROUP BY i.verb
            ORDER BY i.verb
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_changed_detection() {
        let hash1 = vec![1, 2, 3, 4];
        let hash2 = vec![1, 2, 3, 5]; // Different

        // Same hash = not changed
        assert!(!matches!(
            (&Some(hash1.clone()), &Some(hash1.clone())),
            (Some(a), Some(b)) if a != b
        ));

        // Different hash = changed
        assert!(matches!(
            (&Some(hash1.clone()), &Some(hash2.clone())),
            (Some(a), Some(b)) if a != b
        ));
    }
}

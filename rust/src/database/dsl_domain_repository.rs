//! DSL Domain Repository implementation
//!
//! This module provides database operations for managing DSL domains, versions,
//! AST storage, and execution tracking using the new domain-based architecture.

use crate::models::domain_models::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Repository trait for DSL domain operations
#[async_trait]
pub trait DslDomainRepositoryTrait {
    // Domain management
    async fn get_domain_by_name(&self, name: &str) -> Result<Option<DslDomain>, DslError>;
    async fn get_domain_by_id(&self, domain_id: &Uuid) -> Result<Option<DslDomain>, DslError>;
    async fn list_domains(&self, active_only: bool) -> Result<Vec<DslDomain>, DslError>;
    async fn create_domain(&self, domain: NewDslDomain) -> Result<DslDomain, DslError>;

    // Version management
    async fn get_dsl_version(
        &self,
        domain_name: &str,
        version_number: i32,
    ) -> Result<Option<DslVersion>, DslError>;
    async fn get_dsl_version_by_id(
        &self,
        version_id: &Uuid,
    ) -> Result<Option<DslVersion>, DslError>;
    async fn get_latest_version(&self, domain_name: &str) -> Result<Option<DslVersion>, DslError>;
    async fn list_versions(
        &self,
        domain_name: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DslVersion>, DslError>;
    async fn create_new_version(&self, version: NewDslVersion) -> Result<DslVersion, DslError>;
    async fn update_compilation_status(
        &self,
        version_id: &Uuid,
        status: CompilationStatus,
    ) -> Result<(), DslError>;

    // AST management
    async fn get_parsed_ast(&self, version_id: &Uuid) -> Result<Option<ParsedAst>, DslError>;
    async fn store_parsed_ast(&self, ast: NewParsedAst) -> Result<ParsedAst, DslError>;
    async fn invalidate_ast(&self, version_id: &Uuid) -> Result<(), DslError>;
    async fn cleanup_old_asts(&self, retention_days: i32) -> Result<i32, DslError>;

    // Execution tracking
    async fn log_execution(&self, log: NewExecutionLog) -> Result<DslExecutionLog, DslError>;
    async fn get_execution_history(
        &self,
        version_id: &Uuid,
        limit: Option<i32>,
    ) -> Result<Vec<DslExecutionLog>, DslError>;

    // Views and summaries
    async fn get_latest_versions(&self) -> Result<Vec<DslLatestVersion>, DslError>;
    async fn get_execution_summary(
        &self,
        domain_name: Option<&str>,
    ) -> Result<Vec<DslExecutionSummary>, DslError>;
}

/// Concrete implementation of the DSL domain repository
#[derive(Clone)]
pub struct DslDomainRepository {
    pool: PgPool,
}

impl DslDomainRepository {
    /// Create a new repository instance
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the next version number for a domain
    pub async fn get_next_version_number(&self, domain_name: &str) -> Result<i32, DslError> {
        let row = sqlx::query(r#"SELECT "dsl-ob-poc".get_next_version_number($1) as next_version"#)
            .bind(domain_name)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(row.get("next_version"))
    }

    /// Get domain statistics for monitoring
    pub async fn get_domain_statistics(
        &self,
        domain_name: &str,
    ) -> Result<DomainStatistics, DslError> {
        let stats = sqlx::query(
            r#"
            SELECT
                d.domain_name,
                COUNT(dv.version_id) as total_versions,
                COUNT(CASE WHEN dv.compilation_status = 'ACTIVE' THEN 1 END) as active_versions,
                COUNT(CASE WHEN dv.compilation_status = 'COMPILED' OR dv.compilation_status = 'ACTIVE' THEN 1 END) as compiled_versions,
                COALESCE(SUM(el.total_executions), 0) as total_executions,
                COALESCE(AVG(el.success_rate), 0) as success_rate,
                MAX(dv.created_at) as last_activity
            FROM "dsl-ob-poc".dsl_domains d
            LEFT JOIN "dsl-ob-poc".dsl_versions dv ON d.domain_id = dv.domain_id
            LEFT JOIN (
                SELECT
                    version_id,
                    COUNT(*) as total_executions,
                    (COUNT(CASE WHEN status = 'SUCCESS' THEN 1 END)::float / COUNT(*)::float) * 100 as success_rate
                FROM "dsl-ob-poc".dsl_execution_log
                GROUP BY version_id
            ) el ON dv.version_id = el.version_id
            WHERE d.domain_name = $1
            GROUP BY d.domain_name
            "#
        )
        .bind(domain_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        match stats {
            Some(row) => Ok(DomainStatistics {
                domain_name: row.get("domain_name"),
                total_versions: row.get("total_versions"),
                active_versions: row.get("active_versions"),
                compiled_versions: row.get("compiled_versions"),
                total_executions: row.get("total_executions"),
                success_rate: row.get("success_rate"),
                avg_compilation_time_ms: None,
                avg_execution_time_ms: None,
                last_activity: row.get("last_activity"),
            }),
            None => Err(DslError::NotFound {
                id: format!("domain: {}", domain_name),
            }),
        }
    }
}

#[async_trait]
impl DslDomainRepositoryTrait for DslDomainRepository {
    async fn get_domain_by_name(&self, name: &str) -> Result<Option<DslDomain>, DslError> {
        debug!("Getting domain by name: {}", name);

        let result = sqlx::query_as::<_, DslDomain>(
            r#"
            SELECT domain_id, domain_name, description, base_grammar_version,
                   vocabulary_version, active, created_at, updated_at
            FROM "dsl-ob-poc".dsl_domains
            WHERE domain_name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to get domain by name {}: {}", name, e);
            DslError::DatabaseError(e.to_string())
        })?;

        debug!("Domain query result: {:?}", result.is_some());
        Ok(result)
    }

    async fn get_domain_by_id(&self, domain_id: &Uuid) -> Result<Option<DslDomain>, DslError> {
        let result = sqlx::query_as::<_, DslDomain>(
            r#"
            SELECT domain_id, domain_name, description, base_grammar_version,
                   vocabulary_version, active, created_at, updated_at
            FROM "dsl-ob-poc".dsl_domains
            WHERE domain_id = $1
            "#,
        )
        .bind(domain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn list_domains(&self, active_only: bool) -> Result<Vec<DslDomain>, DslError> {
        let query = if active_only {
            r#"
            SELECT domain_id, domain_name, description, base_grammar_version,
                   vocabulary_version, active, created_at, updated_at
            FROM "dsl-ob-poc".dsl_domains
            WHERE active = true
            ORDER BY domain_name
            "#
        } else {
            r#"
            SELECT domain_id, domain_name, description, base_grammar_version,
                   vocabulary_version, active, created_at, updated_at
            FROM "dsl-ob-poc".dsl_domains
            ORDER BY domain_name
            "#
        };

        let result = sqlx::query_as::<_, DslDomain>(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        info!(
            "Retrieved {} domains (active_only: {})",
            result.len(),
            active_only
        );
        Ok(result)
    }

    async fn create_domain(&self, domain: NewDslDomain) -> Result<DslDomain, DslError> {
        let result = sqlx::query_as::<_, DslDomain>(
            r#"
            INSERT INTO "dsl-ob-poc".dsl_domains
                (domain_name, description, base_grammar_version, vocabulary_version, active)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING domain_id, domain_name, description, base_grammar_version,
                      vocabulary_version, active, created_at, updated_at
            "#,
        )
        .bind(&domain.domain_name)
        .bind(&domain.description)
        .bind(&domain.base_grammar_version)
        .bind(&domain.vocabulary_version)
        .bind(domain.active)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("duplicate key") {
                DslError::AlreadyExists {
                    id: format!("domain: {}", domain.domain_name),
                }
            } else {
                DslError::DatabaseError(e.to_string())
            }
        })?;

        info!("Created new domain: {}", result.domain_name);
        Ok(result)
    }

    async fn get_dsl_version(
        &self,
        domain_name: &str,
        version_number: i32,
    ) -> Result<Option<DslVersion>, DslError> {
        debug!("Getting DSL version: {} v{}", domain_name, version_number);

        let result = sqlx::query_as::<_, DslVersion>(
            r#"
            SELECT dv.version_id, dv.domain_id, dv.version_number, dv.functional_state,
                   dv.dsl_source_code, dv.compilation_status, dv.change_description,
                   dv.parent_version_id, dv.created_by, dv.created_at, dv.compiled_at, dv.activated_at
            FROM "dsl-ob-poc".dsl_versions dv
            JOIN "dsl-ob-poc".dsl_domains d ON dv.domain_id = d.domain_id
            WHERE d.domain_name = $1 AND dv.version_number = $2
            "#,
        )
        .bind(domain_name)
        .bind(version_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        debug!("DSL version query result: {:?}", result.is_some());
        Ok(result)
    }

    async fn get_dsl_version_by_id(
        &self,
        version_id: &Uuid,
    ) -> Result<Option<DslVersion>, DslError> {
        let result = sqlx::query_as::<_, DslVersion>(
            r#"
            SELECT version_id, domain_id, version_number, functional_state,
                   dsl_source_code, compilation_status, change_description,
                   parent_version_id, created_by, created_at, compiled_at, activated_at
            FROM "dsl-ob-poc".dsl_versions
            WHERE version_id = $1
            "#,
        )
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn get_latest_version(&self, domain_name: &str) -> Result<Option<DslVersion>, DslError> {
        let result = sqlx::query_as::<_, DslVersion>(
            r#"
            SELECT dv.version_id, dv.domain_id, dv.version_number, dv.functional_state,
                   dv.dsl_source_code, dv.compilation_status, dv.change_description,
                   dv.parent_version_id, dv.created_by, dv.created_at, dv.compiled_at, dv.activated_at
            FROM "dsl-ob-poc".dsl_versions dv
            JOIN "dsl-ob-poc".dsl_domains d ON dv.domain_id = d.domain_id
            WHERE d.domain_name = $1
            ORDER BY dv.version_number DESC
            LIMIT 1
            "#,
        )
        .bind(domain_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn list_versions(
        &self,
        domain_name: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DslVersion>, DslError> {
        let query = match limit {
            Some(_) => {
                r#"
                SELECT dv.version_id, dv.domain_id, dv.version_number, dv.functional_state,
                       dv.dsl_source_code, dv.compilation_status, dv.change_description,
                       dv.parent_version_id, dv.created_by, dv.created_at, dv.compiled_at, dv.activated_at
                FROM "dsl-ob-poc".dsl_versions dv
                JOIN "dsl-ob-poc".dsl_domains d ON dv.domain_id = d.domain_id
                WHERE d.domain_name = $1
                ORDER BY dv.version_number DESC
                LIMIT $2
                "#
            }
            None => {
                r#"
                SELECT dv.version_id, dv.domain_id, dv.version_number, dv.functional_state,
                       dv.dsl_source_code, dv.compilation_status, dv.change_description,
                       dv.parent_version_id, dv.created_by, dv.created_at, dv.compiled_at, dv.activated_at
                FROM "dsl-ob-poc".dsl_versions dv
                JOIN "dsl-ob-poc".dsl_domains d ON dv.domain_id = d.domain_id
                WHERE d.domain_name = $1
                ORDER BY dv.version_number DESC
                "#
            }
        };

        let result = match limit {
            Some(l) => {
                sqlx::query_as::<_, DslVersion>(query)
                    .bind(domain_name)
                    .bind(l)
                    .fetch_all(&self.pool)
                    .await
            }
            None => {
                sqlx::query_as::<_, DslVersion>(query)
                    .bind(domain_name)
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn create_new_version(&self, version: NewDslVersion) -> Result<DslVersion, DslError> {
        // First get the domain ID and next version number
        let domain = self
            .get_domain_by_name(&version.domain_name)
            .await?
            .ok_or_else(|| DslError::NotFound {
                id: format!("domain: {}", version.domain_name),
            })?;

        let next_version = self.get_next_version_number(&version.domain_name).await?;

        let result = sqlx::query_as::<_, DslVersion>(
            r#"
            INSERT INTO "dsl-ob-poc".dsl_versions
                (domain_id, version_number, functional_state, dsl_source_code,
                 change_description, parent_version_id, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING version_id, domain_id, version_number, functional_state,
                      dsl_source_code, compilation_status, change_description,
                      parent_version_id, created_by, created_at, compiled_at, activated_at
            "#,
        )
        .bind(domain.domain_id)
        .bind(next_version)
        .bind(&version.functional_state)
        .bind(&version.dsl_source_code)
        .bind(&version.change_description)
        .bind(&version.parent_version_id)
        .bind(&version.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        info!(
            "Created new DSL version: {} v{}",
            version.domain_name, next_version
        );
        Ok(result)
    }

    async fn update_compilation_status(
        &self,
        version_id: &Uuid,
        status: CompilationStatus,
    ) -> Result<(), DslError> {
        let now = match status {
            CompilationStatus::Compiled => Some(Utc::now()),
            CompilationStatus::Active => Some(Utc::now()),
            _ => None,
        };

        sqlx::query(
            r#"
            UPDATE "dsl-ob-poc".dsl_versions
            SET compilation_status = $2,
                compiled_at = CASE WHEN $2 = 'COMPILED' THEN $3 ELSE compiled_at END,
                activated_at = CASE WHEN $2 = 'ACTIVE' THEN $3 ELSE activated_at END
            WHERE version_id = $1
            "#,
        )
        .bind(version_id)
        .bind(status.to_string())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        debug!(
            "Updated compilation status for version {}: {}",
            version_id, status
        );
        Ok(())
    }

    async fn get_parsed_ast(&self, version_id: &Uuid) -> Result<Option<ParsedAst>, DslError> {
        let result = sqlx::query_as::<_, ParsedAst>(
            r#"
            SELECT ast_id, version_id, ast_json, parse_metadata, grammar_version,
                   parser_version, ast_hash, node_count, complexity_score,
                   parsed_at, invalidated_at
            FROM "dsl-ob-poc".parsed_asts
            WHERE version_id = $1 AND invalidated_at IS NULL
            "#,
        )
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn store_parsed_ast(&self, ast: NewParsedAst) -> Result<ParsedAst, DslError> {
        let result = sqlx::query_as::<_, ParsedAst>(
            r#"
            INSERT INTO "dsl-ob-poc".parsed_asts
                (version_id, ast_json, parse_metadata, grammar_version, parser_version,
                 ast_hash, node_count, complexity_score)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (version_id)
            DO UPDATE SET
                ast_json = EXCLUDED.ast_json,
                parse_metadata = EXCLUDED.parse_metadata,
                grammar_version = EXCLUDED.grammar_version,
                parser_version = EXCLUDED.parser_version,
                ast_hash = EXCLUDED.ast_hash,
                node_count = EXCLUDED.node_count,
                complexity_score = EXCLUDED.complexity_score,
                parsed_at = now(),
                invalidated_at = NULL
            RETURNING ast_id, version_id, ast_json, parse_metadata, grammar_version,
                      parser_version, ast_hash, node_count, complexity_score,
                      parsed_at, invalidated_at
            "#,
        )
        .bind(&ast.version_id)
        .bind(&ast.ast_json)
        .bind(&ast.parse_metadata)
        .bind(&ast.grammar_version)
        .bind(&ast.parser_version)
        .bind(&ast.ast_hash)
        .bind(&ast.node_count)
        .bind(&ast.complexity_score)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        debug!("Stored parsed AST for version: {}", ast.version_id);
        Ok(result)
    }

    async fn invalidate_ast(&self, version_id: &Uuid) -> Result<(), DslError> {
        sqlx::query(
            r#"
            UPDATE "dsl-ob-poc".parsed_asts
            SET invalidated_at = now()
            WHERE version_id = $1
            "#,
        )
        .bind(version_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        debug!("Invalidated AST for version: {}", version_id);
        Ok(())
    }

    async fn cleanup_old_asts(&self, retention_days: i32) -> Result<i32, DslError> {
        let result = sqlx::query(
            r#"
            DELETE FROM "dsl-ob-poc".parsed_asts
            WHERE invalidated_at IS NOT NULL
            AND invalidated_at < now() - interval '%d days'
            "#,
        )
        .bind(retention_days)
        .execute(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        let cleaned = result.rows_affected() as i32;
        if cleaned > 0 {
            info!("Cleaned up {} old AST records", cleaned);
        }

        Ok(cleaned)
    }

    async fn log_execution(&self, log: NewExecutionLog) -> Result<DslExecutionLog, DslError> {
        let result = sqlx::query_as::<_, DslExecutionLog>(
            r#"
            INSERT INTO "dsl-ob-poc".dsl_execution_log
                (version_id, cbu_id, execution_phase, status, result_data,
                 error_details, performance_metrics, executed_by, started_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING execution_id, version_id, cbu_id, execution_phase, status,
                      result_data, error_details, performance_metrics, executed_by,
                      started_at, completed_at, duration_ms
            "#,
        )
        .bind(&log.version_id)
        .bind(&log.cbu_id)
        .bind(log.execution_phase.to_string())
        .bind(log.status.to_string())
        .bind(&log.result_data)
        .bind(&log.error_details)
        .bind(&log.performance_metrics)
        .bind(&log.executed_by)
        .bind(&log.started_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn get_execution_history(
        &self,
        version_id: &Uuid,
        limit: Option<i32>,
    ) -> Result<Vec<DslExecutionLog>, DslError> {
        let query = match limit {
            Some(_) => {
                r#"
                SELECT execution_id, version_id, cbu_id, execution_phase, status,
                       result_data, error_details, performance_metrics, executed_by,
                       started_at, completed_at, duration_ms
                FROM "dsl-ob-poc".dsl_execution_log
                WHERE version_id = $1
                ORDER BY started_at DESC
                LIMIT $2
                "#
            }
            None => {
                r#"
                SELECT execution_id, version_id, cbu_id, execution_phase, status,
                       result_data, error_details, performance_metrics, executed_by,
                       started_at, completed_at, duration_ms
                FROM "dsl-ob-poc".dsl_execution_log
                WHERE version_id = $1
                ORDER BY started_at DESC
                "#
            }
        };

        let result = match limit {
            Some(l) => {
                sqlx::query_as::<_, DslExecutionLog>(query)
                    .bind(version_id)
                    .bind(l)
                    .fetch_all(&self.pool)
                    .await
            }
            None => {
                sqlx::query_as::<_, DslExecutionLog>(query)
                    .bind(version_id)
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn get_latest_versions(&self) -> Result<Vec<DslLatestVersion>, DslError> {
        let result = sqlx::query_as::<_, DslLatestVersion>(
            r#"
            SELECT domain_name, domain_description, version_id, version_number,
                   functional_state, compilation_status, change_description,
                   created_by, created_at, has_compiled_ast
            FROM "dsl-ob-poc".dsl_latest_versions
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }

    async fn get_execution_summary(
        &self,
        domain_name: Option<&str>,
    ) -> Result<Vec<DslExecutionSummary>, DslError> {
        let query = match domain_name {
            Some(_) => {
                r#"
                SELECT domain_name, version_number, compilation_status,
                       total_executions, successful_executions, failed_executions,
                       avg_duration_ms, last_execution_at
                FROM "dsl-ob-poc".dsl_execution_summary
                WHERE domain_name = $1
                "#
            }
            None => {
                r#"
                SELECT domain_name, version_number, compilation_status,
                       total_executions, successful_executions, failed_executions,
                       avg_duration_ms, last_execution_at
                FROM "dsl-ob-poc".dsl_execution_summary
                "#
            }
        };

        let result = match domain_name {
            Some(name) => {
                sqlx::query_as::<_, DslExecutionSummary>(query)
                    .bind(name)
                    .fetch_all(&self.pool)
                    .await
            }
            None => {
                sqlx::query_as::<_, DslExecutionSummary>(query)
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        Ok(result)
    }
}

/// Additional request types for new operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDslDomain {
    pub domain_name: String,
    pub description: Option<String>,
    pub base_grammar_version: String,
    pub vocabulary_version: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewExecutionLog {
    pub version_id: Uuid,
    pub cbu_id: Option<String>,
    pub execution_phase: ExecutionPhase,
    pub status: ExecutionStatus,
    pub result_data: Option<Value>,
    pub error_details: Option<Value>,
    pub performance_metrics: Option<Value>,
    pub executed_by: Option<String>,
    pub started_at: DateTime<Utc>,
}

/// Repository-specific errors
#[derive(Debug, thiserror::Error)]
pub enum DslError {
    #[error("DSL not found: {id}")]
    NotFound { id: String },

    #[error("DSL already exists: {id}")]
    AlreadyExists { id: String },

    #[error("Invalid DSL content: {reason}")]
    InvalidContent { reason: String },

    #[error("Validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Compilation error: {0}")]
    CompileError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Domain mismatch - expected: {expected}, found: {found}")]
    DomainMismatch { expected: String, found: String },
}

pub type DslResult<T> = Result<T, DslError>;

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    // Helper function to create test repository (would need actual DB for integration tests)
    fn create_test_repository(pool: PgPool) -> DslDomainRepository {
        DslDomainRepository::new(pool)
    }

    #[test]
    fn test_new_dsl_domain_creation() {
        let domain = NewDslDomain {
            domain_name: "Test".to_string(),
            description: Some("Test domain".to_string()),
            base_grammar_version: "1.0.0".to_string(),
            vocabulary_version: "1.0.0".to_string(),
            active: true,
        };

        assert_eq!(domain.domain_name, "Test");
        assert!(domain.active);
    }

    #[test]
    fn test_dsl_error_display() {
        let error = DslError::NotFound {
            id: "test-domain".to_string(),
        };

        assert_eq!(error.to_string(), "DSL not found: test-domain");
    }
}

//! DSL Repository - Database operations for DSL instances and parsed ASTs
//!
//! This module centralizes all database operations for the DSL/AST tables,
//! providing transactional saves with automatic version tracking.

use sqlx::PgPool;
use std::collections::HashMap;

/// Result of saving a DSL/AST pair
#[derive(Debug, Clone)]
pub struct DslSaveResult {
    pub case_id: String,
    pub version: i32,
    pub success: bool,
}

/// DSL Repository for database operations
pub struct DslRepository {
    pool: PgPool,
}

impl DslRepository {
    /// Create a new DSL repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Save DSL and AST atomically in a transaction
    /// Returns the new version number
    pub async fn save_dsl_ast(
        &self,
        case_id: &str,
        dsl_content: &str,
        ast_json: &str,
        domain: &str,
        operation_type: &str,
        parse_time_ms: i64,
    ) -> Result<DslSaveResult, sqlx::Error> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Get next version number
        let version_result: (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) + 1 FROM "ob-poc".dsl_instances WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&mut *tx)
                .await?;

        let version = version_result.0 as i32;

        // Insert DSL instance
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_instances
            (case_id, dsl_content, domain, operation_type, status, processing_time_ms, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'PROCESSED', $5, NOW(), NOW())
            "#
        )
        .bind(case_id)
        .bind(dsl_content)
        .bind(domain)
        .bind(operation_type)
        .bind(parse_time_ms)
        .execute(&mut *tx)
        .await?;

        // Insert parsed AST
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".parsed_asts
            (case_id, ast_json, ast_format_version, parse_time_ms, created_at)
            VALUES ($1, $2::jsonb, '3.1', $3, NOW())
            "#,
        )
        .bind(case_id)
        .bind(ast_json)
        .bind(parse_time_ms)
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(DslSaveResult {
            case_id: case_id.to_string(),
            version,
            success: true,
        })
    }

    /// Load latest DSL for a case
    pub async fn load_dsl(&self, case_id: &str) -> Result<Option<(String, i32)>, sqlx::Error> {
        let result = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT dsl_content,
                   (SELECT COUNT(*) FROM "ob-poc".dsl_instances WHERE case_id = $1) as version
            FROM "ob-poc".dsl_instances
            WHERE case_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(content, ver)| (content, ver as i32)))
    }

    /// Load latest AST for a case
    pub async fn load_ast(&self, case_id: &str) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT ast_json::text
            FROM "ob-poc".parsed_asts
            WHERE case_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(ast,)| ast))
    }

    /// Get version count for a case
    pub async fn get_version_count(&self, case_id: &str) -> Result<i32, sqlx::Error> {
        let result: (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".dsl_instances WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(result.0 as i32)
    }

    /// Create or update CBU
    pub async fn upsert_cbu(
        &self,
        cbu_id: &str,
        client_name: &str,
        client_type: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at)
            VALUES ($1, $2, $3, 'US', 'ACTIVE', NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET client_type = $3, updated_at = NOW()
            "#
        )
        .bind(cbu_id)
        .bind(client_name)
        .bind(client_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Save attribute value
    pub async fn save_attribute(
        &self,
        entity_id: &str,
        attribute_id: &str,
        value: &str,
        value_type: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values (attribute_id, entity_id, attribute_value, value_type, created_at)
            VALUES ($1::uuid, $2, $3, $4, NOW())
            "#
        )
        .bind(attribute_id)
        .bind(entity_id)
        .bind(value)
        .bind(value_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Save DSL execution results atomically in a single transaction
    /// This includes: DSL instance, parsed AST, CBU, and all attributes
    /// If any operation fails, the entire transaction is rolled back
    pub async fn save_execution_transactionally(
        &self,
        case_id: &str,
        dsl_content: &str,
        ast_json: &str,
        domain: &str,
        operation_type: &str,
        parse_time_ms: i64,
        client_name: &str,
        client_type: &str,
        attributes: &HashMap<String, (String, String)>, // attr_id -> (value, value_type)
    ) -> Result<DslSaveResult, sqlx::Error> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Get next version number
        let version_result: (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) + 1 FROM "ob-poc".dsl_instances WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&mut *tx)
                .await?;

        let version = version_result.0 as i32;

        // Insert DSL instance
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_instances
            (case_id, dsl_content, domain, operation_type, status, processing_time_ms, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'PROCESSED', $5, NOW(), NOW())
            "#
        )
        .bind(case_id)
        .bind(dsl_content)
        .bind(domain)
        .bind(operation_type)
        .bind(parse_time_ms)
        .execute(&mut *tx)
        .await?;

        // Insert parsed AST
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".parsed_asts
            (case_id, ast_json, ast_format_version, parse_time_ms, created_at)
            VALUES ($1, $2::jsonb, '3.1', $3, NOW())
            "#,
        )
        .bind(case_id)
        .bind(ast_json)
        .bind(parse_time_ms)
        .execute(&mut *tx)
        .await?;

        // Upsert CBU
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at)
            VALUES ($1, $2, $3, 'US', 'ACTIVE', NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET client_type = $3, updated_at = NOW()
            "#
        )
        .bind(case_id)
        .bind(client_name)
        .bind(client_type)
        .execute(&mut *tx)
        .await?;

        // Save all attributes
        for (attr_id, (value, value_type)) in attributes {
            // Skip invalid UUIDs for attribute_id
            if attr_id.starts_with(':') || attr_id.len() < 36 {
                continue;
            }

            sqlx::query(
                r#"
                INSERT INTO "ob-poc".attribute_values (attribute_id, entity_id, attribute_value, value_type, created_at)
                VALUES ($1::uuid, $2, $3, $4, NOW())
                "#
            )
            .bind(attr_id)
            .bind(case_id)
            .bind(value)
            .bind(value_type)
            .execute(&mut *tx)
            .await?;
        }

        // Commit transaction - if this fails, everything is rolled back
        tx.commit().await?;

        Ok(DslSaveResult {
            case_id: case_id.to_string(),
            version,
            success: true,
        })
    }
}

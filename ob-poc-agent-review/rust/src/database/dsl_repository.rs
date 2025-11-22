//! DSL Repository - Database operations for DSL instances and versions
//!
//! Per Section 3.4 of master architecture:
//! - `business_reference` (DB): long-lived, business-level identifier
//! - `case_id` (RuntimeEnv): transient execution context ID (for logs & traces)
//!
//! Canonical schema:
//! - dsl_instances: instance_id, domain_name, business_reference, current_version, status
//! - dsl_instance_versions: version_id, instance_id, version_number, dsl_content, ast_json, operation_type

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Result of saving a DSL instance
#[derive(Debug, Clone)]
pub struct DslSaveResult {
    pub instance_id: Uuid,
    pub version: i32,
    pub business_reference: String,
    pub success: bool,
}

/// DSL Instance row - matches canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslInstanceRow {
    pub instance_id: Uuid,
    pub domain_name: String,
    pub business_reference: String,
    pub current_version: i32,
    pub status: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// DSL Instance Version row - matches canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslInstanceVersionRow {
    pub version_id: Uuid,
    pub instance_id: Uuid,
    pub version_number: i32,
    pub dsl_content: String,
    pub operation_type: String,
    pub compilation_status: String,
    pub ast_json: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
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

    /// Save DSL instance with version atomically in a transaction
    /// Returns the instance_id and version number
    pub async fn save_dsl_instance(
        &self,
        business_reference: &str,
        domain_name: &str,
        dsl_content: &str,
        ast_json: Option<&serde_json::Value>,
        operation_type: &str,
    ) -> Result<DslSaveResult, sqlx::Error> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Check if instance exists for this business_reference
        let existing: Option<(Uuid, i32)> = sqlx::query_as(
            r#"
            SELECT instance_id, current_version
            FROM "ob-poc".dsl_instances
            WHERE business_reference = $1
            "#,
        )
        .bind(business_reference)
        .fetch_optional(&mut *tx)
        .await?;

        let (instance_id, version) = if let Some((id, current_ver)) = existing {
            // Update existing instance version
            let new_version = current_ver + 1;
            sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_instances
                SET current_version = $1, updated_at = NOW()
                WHERE instance_id = $2
                "#,
            )
            .bind(new_version)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            (id, new_version)
        } else {
            // Create new instance
            let new_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".dsl_instances
                (instance_id, domain_name, business_reference, current_version, status, created_at, updated_at)
                VALUES ($1, $2, $3, 1, 'ACTIVE', NOW(), NOW())
                "#,
            )
            .bind(new_id)
            .bind(domain_name)
            .bind(business_reference)
            .execute(&mut *tx)
            .await?;
            (new_id, 1)
        };

        // Insert version record with DSL content and AST
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_instance_versions
            (instance_id, version_number, dsl_content, operation_type, compilation_status, ast_json, created_at)
            VALUES ($1, $2, $3, $4, 'COMPILED', $5, NOW())
            "#,
        )
        .bind(instance_id)
        .bind(version)
        .bind(dsl_content)
        .bind(operation_type)
        .bind(ast_json)
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(DslSaveResult {
            instance_id,
            version,
            business_reference: business_reference.to_string(),
            success: true,
        })
    }

    /// Save DSL execution (simplified interface)
    pub async fn save_execution(
        &self,
        dsl_content: &str,
        domain: &str,
        business_reference: &str,
        _cbu_id: Option<Uuid>,
        ast_json: &serde_json::Value,
    ) -> Result<DslSaveResult, sqlx::Error> {
        self.save_dsl_instance(
            business_reference,
            domain,
            dsl_content,
            Some(ast_json),
            "EXECUTE",
        )
        .await
    }

    /// Get DSL instance by business_reference
    pub async fn get_instance_by_reference(
        &self,
        business_reference: &str,
    ) -> Result<Option<DslInstanceRow>, sqlx::Error> {
        sqlx::query_as::<_, DslInstanceRow>(
            r#"
            SELECT instance_id, domain_name, business_reference, current_version, status, created_at, updated_at
            FROM "ob-poc".dsl_instances
            WHERE business_reference = $1
            "#,
        )
        .bind(business_reference)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get DSL instance by instance_id
    pub async fn get_instance_by_id(
        &self,
        instance_id: Uuid,
    ) -> Result<Option<DslInstanceRow>, sqlx::Error> {
        sqlx::query_as::<_, DslInstanceRow>(
            r#"
            SELECT instance_id, domain_name, business_reference, current_version, status, created_at, updated_at
            FROM "ob-poc".dsl_instances
            WHERE instance_id = $1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get DSL content by instance ID (latest version)
    pub async fn get_dsl_content(&self, instance_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT v.dsl_content
            FROM "ob-poc".dsl_instance_versions v
            WHERE v.instance_id = $1
            ORDER BY v.version_number DESC
            LIMIT 1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(content,)| content))
    }

    /// Load latest DSL for a business_reference
    pub async fn load_dsl(
        &self,
        business_reference: &str,
    ) -> Result<Option<(String, i32)>, sqlx::Error> {
        let result = sqlx::query_as::<_, (String, i32)>(
            r#"
            SELECT v.dsl_content, v.version_number
            FROM "ob-poc".dsl_instance_versions v
            JOIN "ob-poc".dsl_instances i ON i.instance_id = v.instance_id
            WHERE i.business_reference = $1
            ORDER BY v.version_number DESC
            LIMIT 1
            "#,
        )
        .bind(business_reference)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Load latest AST for a business_reference
    pub async fn load_ast(
        &self,
        business_reference: &str,
    ) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let result = sqlx::query_as::<_, (serde_json::Value,)>(
            r#"
            SELECT v.ast_json
            FROM "ob-poc".dsl_instance_versions v
            JOIN "ob-poc".dsl_instances i ON i.instance_id = v.instance_id
            WHERE i.business_reference = $1 AND v.ast_json IS NOT NULL
            ORDER BY v.version_number DESC
            LIMIT 1
            "#,
        )
        .bind(business_reference)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(ast,)| ast))
    }

    /// Get version count for a business_reference
    pub async fn get_version_count(&self, business_reference: &str) -> Result<i32, sqlx::Error> {
        let result: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT current_version
            FROM "ob-poc".dsl_instances
            WHERE business_reference = $1
            "#,
        )
        .bind(business_reference)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(v,)| v).unwrap_or(0))
    }

    /// Get all versions for a business_reference
    pub async fn get_all_versions(
        &self,
        business_reference: &str,
    ) -> Result<Vec<DslInstanceVersionRow>, sqlx::Error> {
        sqlx::query_as::<_, DslInstanceVersionRow>(
            r#"
            SELECT v.version_id, v.instance_id, v.version_number, v.dsl_content,
                   v.operation_type, v.compilation_status, v.ast_json, v.created_at
            FROM "ob-poc".dsl_instance_versions v
            JOIN "ob-poc".dsl_instances i ON i.instance_id = v.instance_id
            WHERE i.business_reference = $1
            ORDER BY v.version_number ASC
            "#,
        )
        .bind(business_reference)
        .fetch_all(&self.pool)
        .await
    }

    /// Update instance status
    pub async fn update_status(
        &self,
        instance_id: Uuid,
        status: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_instances
            SET status = $1, updated_at = NOW()
            WHERE instance_id = $2
            "#,
        )
        .bind(status)
        .bind(instance_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// List instances by domain
    pub async fn list_by_domain(
        &self,
        domain_name: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DslInstanceRow>, sqlx::Error> {
        sqlx::query_as::<_, DslInstanceRow>(
            r#"
            SELECT instance_id, domain_name, business_reference, current_version, status, created_at, updated_at
            FROM "ob-poc".dsl_instances
            WHERE domain_name = $1
            ORDER BY updated_at DESC
            LIMIT $2
            "#,
        )
        .bind(domain_name)
        .bind(limit.unwrap_or(100))
        .fetch_all(&self.pool)
        .await
    }
}

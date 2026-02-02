//! CBU Service - CRUD operations for Client Business Units
//!
//! This module provides database operations for CBUs following the canonical
//! DB schema. DSL field names are mapped to DB columns in CrudExecutor.
//!
//! Canonical DB schema:
//! - cbu_id uuid PK
//! - name text (DSL :cbu-name maps here)
//! - description text
//! - nature_purpose text
//! - source_of_funds text
//! - client_type varchar(100)
//! - jurisdiction varchar(50)
//! - created_at, updated_at timestamps

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Client Business Unit record - matches canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuRow {
    pub cbu_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub source_of_funds: Option<String>,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
    /// Template discriminator: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, etc.
    pub cbu_category: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Fields for creating a new CBU
#[derive(Debug, Clone)]
pub struct NewCbuFields {
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub source_of_funds: Option<String>,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
}

/// Service for CBU operations
#[derive(Clone, Debug)]
pub struct CbuService {
    pool: PgPool,
}

impl CbuService {
    /// Create a new CBU service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new CBU
    pub async fn create_cbu(&self, fields: &NewCbuFields) -> Result<Uuid> {
        let cbu_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, source_of_funds, client_type, jurisdiction, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            "#,
        )
        .bind(cbu_id)
        .bind(&fields.name)
        .bind(&fields.description)
        .bind(&fields.nature_purpose)
        .bind(&fields.source_of_funds)
        .bind(&fields.client_type)
        .bind(&fields.jurisdiction)
        .execute(&self.pool)
        .await
        .context("Failed to create CBU")?;

        info!("Created CBU {} for '{}'", cbu_id, fields.name);

        Ok(cbu_id)
    }

    /// Get CBU by ID
    pub async fn get_cbu_by_id(&self, cbu_id: Uuid) -> Result<Option<CbuRow>> {
        let result = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT cbu_id, name, description, nature_purpose, source_of_funds, client_type, jurisdiction, cbu_category, created_at, updated_at
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CBU by ID")?;

        Ok(result)
    }

    /// Get CBU by name (business_reference lookup)
    pub async fn get_cbu_by_name(&self, name: &str) -> Result<Option<CbuRow>> {
        let result = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT cbu_id, name, description, nature_purpose, source_of_funds, client_type, jurisdiction, cbu_category, created_at, updated_at
            FROM "ob-poc".cbus
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CBU by name")?;

        Ok(result)
    }

    /// List all CBUs
    pub async fn list_cbus(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<CbuRow>> {
        let results = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT cbu_id, name, description, nature_purpose, source_of_funds, client_type, jurisdiction, cbu_category, created_at, updated_at
            FROM "ob-poc".cbus
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit.unwrap_or(100))
        .bind(offset.unwrap_or(0))
        .fetch_all(&self.pool)
        .await
        .context("Failed to list CBUs")?;

        Ok(results)
    }

    /// Update CBU
    pub async fn update_cbu(
        &self,
        cbu_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        nature_purpose: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbus
            SET name = COALESCE($1, name),
                description = COALESCE($2, description),
                nature_purpose = COALESCE($3, nature_purpose),
                updated_at = NOW()
            WHERE cbu_id = $4
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(nature_purpose)
        .bind(cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to update CBU")?;

        if result.rows_affected() > 0 {
            info!("Updated CBU {}", cbu_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete CBU
    pub async fn delete_cbu(&self, cbu_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete CBU")?;

        if result.rows_affected() > 0 {
            info!("Deleted CBU {}", cbu_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Upsert CBU (create or update)
    pub async fn upsert_cbu(&self, cbu_id: Option<Uuid>, fields: &NewCbuFields) -> Result<Uuid> {
        let id = cbu_id.unwrap_or_else(Uuid::now_v7);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, source_of_funds, client_type, jurisdiction, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                nature_purpose = EXCLUDED.nature_purpose,
                source_of_funds = EXCLUDED.source_of_funds,
                client_type = EXCLUDED.client_type,
                jurisdiction = EXCLUDED.jurisdiction,
                updated_at = NOW()
            "#,
        )
        .bind(id)
        .bind(&fields.name)
        .bind(&fields.description)
        .bind(&fields.nature_purpose)
        .bind(&fields.source_of_funds)
        .bind(&fields.client_type)
        .bind(&fields.jurisdiction)
        .execute(&self.pool)
        .await
        .context("Failed to upsert CBU")?;

        info!("Upserted CBU {} for '{}'", id, fields.name);

        Ok(id)
    }

    /// Get sink attributes for CBU (attributes that should be populated)
    pub async fn get_sink_attributes_for_cbu(&self) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE sink IS NOT NULL
              AND (sink::text ILIKE '%CBU%' OR sink::text ILIKE '%cbu%')
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get sink attributes for CBU")?;

        Ok(rows)
    }
}

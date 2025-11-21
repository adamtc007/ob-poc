//! Attribute Values Service - CRUD operations for runtime attribute values
//!
//! This module provides database operations for the attribute_values table,
//! which stores actual attribute values associated with CBUs.
//!
//! Canonical schema (DB is master):
//! - cbu_id uuid NOT NULL
//! - dsl_version integer NOT NULL
//! - attribute_id uuid NOT NULL
//! - value jsonb NOT NULL
//! - state text NOT NULL (e.g., 'proposed', 'confirmed', 'derived')
//! - source jsonb (doc ref, extraction method, etc.)
//! - observed_at timestamp

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

/// Row struct matching canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AttributeValueRow {
    pub av_id: Uuid,
    pub cbu_id: Uuid,
    pub dsl_ob_id: Option<Uuid>,
    pub dsl_version: i32,
    pub attribute_id: Uuid,
    pub value: JsonValue,
    pub state: String,
    pub source: Option<JsonValue>,
    pub observed_at: Option<DateTime<Utc>>,
}

/// Service for attribute value operations
#[derive(Clone, Debug)]
pub struct AttributeValuesService {
    pool: PgPool,
}

impl AttributeValuesService {
    /// Create a new attribute values service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Upsert an attribute value for a CBU (canonical method per Section 3.2)
    pub async fn upsert_for_cbu(
        &self,
        cbu_id: Uuid,
        dsl_version: i32,
        attribute_id: Uuid,
        value: JsonValue,
        state: &str,
        source: Option<JsonValue>,
    ) -> Result<Uuid> {
        let av_id = Uuid::new_v4();
        let observed_at = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values
                (av_id, cbu_id, dsl_version, attribute_id, value, state, source, observed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (cbu_id, dsl_version, attribute_id)
            DO UPDATE SET
                value = EXCLUDED.value,
                state = EXCLUDED.state,
                source = EXCLUDED.source,
                observed_at = EXCLUDED.observed_at
            "#,
        )
        .bind(av_id)
        .bind(cbu_id)
        .bind(dsl_version)
        .bind(attribute_id)
        .bind(&value)
        .bind(state)
        .bind(&source)
        .bind(observed_at)
        .execute(&self.pool)
        .await
        .context("Failed to upsert attribute value")?;

        info!(
            "Upserted attribute {} for CBU {} (version {})",
            attribute_id, cbu_id, dsl_version
        );
        Ok(av_id)
    }

    /// Get an attribute value for a CBU at a specific version
    pub async fn get_attribute_value(
        &self,
        cbu_id: Uuid,
        dsl_version: i32,
        attribute_id: Uuid,
    ) -> Result<Option<AttributeValueRow>> {
        let result = sqlx::query_as::<_, AttributeValueRow>(
            r#"
            SELECT av_id, cbu_id, dsl_ob_id, dsl_version, attribute_id, value, state, source, observed_at
            FROM "ob-poc".attribute_values
            WHERE cbu_id = $1 AND dsl_version = $2 AND attribute_id = $3
            "#,
        )
        .bind(cbu_id)
        .bind(dsl_version)
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get attribute value")?;

        Ok(result)
    }

    /// Get the latest attribute value for a CBU (highest dsl_version)
    pub async fn get_latest_attribute_value(
        &self,
        cbu_id: Uuid,
        attribute_id: Uuid,
    ) -> Result<Option<AttributeValueRow>> {
        let result = sqlx::query_as::<_, AttributeValueRow>(
            r#"
            SELECT av_id, cbu_id, dsl_ob_id, dsl_version, attribute_id, value, state, source, observed_at
            FROM "ob-poc".attribute_values
            WHERE cbu_id = $1 AND attribute_id = $2
            ORDER BY dsl_version DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get latest attribute value")?;

        Ok(result)
    }

    /// Get all attribute values for a CBU at a specific version
    pub async fn get_cbu_attributes(
        &self,
        cbu_id: Uuid,
        dsl_version: i32,
    ) -> Result<Vec<AttributeValueRow>> {
        let rows = sqlx::query_as::<_, AttributeValueRow>(
            r#"
            SELECT av_id, cbu_id, dsl_ob_id, dsl_version, attribute_id, value, state, source, observed_at
            FROM "ob-poc".attribute_values
            WHERE cbu_id = $1 AND dsl_version = $2
            ORDER BY attribute_id
            "#,
        )
        .bind(cbu_id)
        .bind(dsl_version)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU attributes")?;

        Ok(rows)
    }

    /// Get all latest attribute values for a CBU (one per attribute_id)
    pub async fn get_cbu_latest_attributes(&self, cbu_id: Uuid) -> Result<Vec<AttributeValueRow>> {
        let rows = sqlx::query_as::<_, AttributeValueRow>(
            r#"
            SELECT DISTINCT ON (attribute_id)
                av_id, cbu_id, dsl_ob_id, dsl_version, attribute_id, value, state, source, observed_at
            FROM "ob-poc".attribute_values
            WHERE cbu_id = $1
            ORDER BY attribute_id, dsl_version DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU latest attributes")?;

        Ok(rows)
    }

    /// Batch upsert attribute values for a CBU
    pub async fn upsert_batch_for_cbu(
        &self,
        cbu_id: Uuid,
        dsl_version: i32,
        attributes: &[(Uuid, JsonValue, String)], // (attribute_id, value, state)
        source: Option<JsonValue>,
    ) -> Result<usize> {
        let mut count = 0;

        for (attr_id, value, state) in attributes {
            self.upsert_for_cbu(
                cbu_id,
                dsl_version,
                *attr_id,
                value.clone(),
                state,
                source.clone(),
            )
            .await?;
            count += 1;
        }

        info!(
            "Batch upserted {} attribute values for CBU {} (version {})",
            count, cbu_id, dsl_version
        );
        Ok(count)
    }

    /// Delete an attribute value
    pub async fn delete_attribute_value(
        &self,
        cbu_id: Uuid,
        dsl_version: i32,
        attribute_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".attribute_values
            WHERE cbu_id = $1 AND dsl_version = $2 AND attribute_id = $3
            "#,
        )
        .bind(cbu_id)
        .bind(dsl_version)
        .bind(attribute_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete attribute value")?;

        Ok(result.rows_affected() > 0)
    }

    /// Get sink attributes for a specific asset type from dictionary
    /// Returns attributes where the sink field contains the asset type
    pub async fn get_sink_attributes_for_asset(&self, asset_type: &str) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE sink IS NOT NULL
              AND sink::text ILIKE $1
            "#,
        )
        .bind(format!("%{}%", asset_type))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get sink attributes for asset")?;

        Ok(rows)
    }

    /// Get source attributes for a specific document type from dictionary
    /// Returns attributes that are produced/sourced from documents
    pub async fn get_source_attributes_for_doc_type(&self, doc_type: &str) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE source IS NOT NULL
              AND source::text ILIKE $1
            "#,
        )
        .bind(format!("%{}%", doc_type))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get source attributes for doc type")?;

        Ok(rows)
    }

    /// Update state for an attribute value (e.g., 'proposed' -> 'confirmed')
    pub async fn update_state(
        &self,
        cbu_id: Uuid,
        dsl_version: i32,
        attribute_id: Uuid,
        new_state: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".attribute_values
            SET state = $1, observed_at = NOW()
            WHERE cbu_id = $2 AND dsl_version = $3 AND attribute_id = $4
            "#,
        )
        .bind(new_state)
        .bind(cbu_id)
        .bind(dsl_version)
        .bind(attribute_id)
        .execute(&self.pool)
        .await
        .context("Failed to update attribute state")?;

        Ok(result.rows_affected() > 0)
    }
}

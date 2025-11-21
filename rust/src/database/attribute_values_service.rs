//! Attribute Values Service - CRUD operations for runtime attribute values
//!
//! This module provides database operations for the attribute_values table,
//! which stores actual attribute values associated with CBUs and entities.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

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

    /// Get an attribute value for a specific entity
    pub async fn get_attribute_value(
        &self,
        entity_id: &str,
        attribute_id: Uuid,
    ) -> Result<Option<String>> {
        let result = sqlx::query_scalar::<_, String>(
            r#"
            SELECT attribute_value
            FROM "ob-poc".attribute_values
            WHERE entity_id = $1 AND attribute_id = $2
            "#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get attribute value")?;

        Ok(result)
    }

    /// Set an attribute value for an entity (upsert)
    pub async fn set_attribute_value(
        &self,
        entity_id: &str,
        attribute_id: Uuid,
        value: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values (entity_id, attribute_id, attribute_value, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (entity_id, attribute_id)
            DO UPDATE SET attribute_value = $3, updated_at = NOW()
            "#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(value)
        .execute(&self.pool)
        .await
        .context("Failed to set attribute value")?;

        info!(
            "Set attribute {} = '{}' for entity {}",
            attribute_id, value, entity_id
        );
        Ok(())
    }

    /// Batch upsert attribute values
    pub async fn upsert_attribute_values(
        &self,
        entity_id: &str,
        attributes: &[(Uuid, String)],
    ) -> Result<usize> {
        let mut count = 0;

        for (attr_id, value) in attributes {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".attribute_values (entity_id, attribute_id, attribute_value, created_at, updated_at)
                VALUES ($1, $2, $3, NOW(), NOW())
                ON CONFLICT (entity_id, attribute_id)
                DO UPDATE SET attribute_value = $3, updated_at = NOW()
                "#,
            )
            .bind(entity_id)
            .bind(attr_id)
            .bind(value)
            .execute(&self.pool)
            .await
            .context("Failed to upsert attribute value")?;

            count += 1;
        }

        info!(
            "Upserted {} attribute values for entity {}",
            count, entity_id
        );
        Ok(count)
    }

    /// Get all attribute values for an entity
    pub async fn get_entity_attributes(&self, entity_id: &str) -> Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT attribute_id, attribute_value
            FROM "ob-poc".attribute_values
            WHERE entity_id = $1
            ORDER BY attribute_id
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entity attributes")?;

        Ok(rows)
    }

    /// Delete an attribute value
    pub async fn delete_attribute_value(
        &self,
        entity_id: &str,
        attribute_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".attribute_values
            WHERE entity_id = $1 AND attribute_id = $2
            "#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete attribute value")?;

        Ok(result.rows_affected() > 0)
    }

    /// Get sink attributes for a specific asset type
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

    /// Get source attributes for a specific document type
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
}

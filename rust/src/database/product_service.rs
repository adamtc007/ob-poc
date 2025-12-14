//! Product Service - CRUD operations for Products

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductRow {
    pub product_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub product_code: Option<String>,
    pub product_category: Option<String>,
    pub regulatory_framework: Option<String>,
    pub min_asset_requirement: Option<rust_decimal::Decimal>,
    pub is_active: Option<bool>,
    pub metadata: Option<JsonValue>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewProductFields {
    pub name: String,
    pub description: Option<String>,
    pub product_code: Option<String>,
    pub product_category: Option<String>,
    pub regulatory_framework: Option<String>,
    pub min_asset_requirement: Option<rust_decimal::Decimal>,
    pub is_active: Option<bool>,
    pub metadata: Option<JsonValue>,
}

#[derive(Clone, Debug)]
pub struct ProductService {
    pool: PgPool,
}

impl ProductService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_product(&self, fields: &NewProductFields) -> Result<Uuid> {
        let product_id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO "ob-poc".products (product_id, name, description, product_code, product_category, regulatory_framework, min_asset_requirement, is_active, metadata, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())"#)
            .bind(product_id).bind(&fields.name).bind(&fields.description).bind(&fields.product_code)
            .bind(&fields.product_category).bind(&fields.regulatory_framework).bind(fields.min_asset_requirement)
            .bind(fields.is_active.unwrap_or(true)).bind(&fields.metadata)
            .execute(&self.pool).await.context("Failed to create Product")?;
        info!("Created Product {} for '{}'", product_id, fields.name);
        Ok(product_id)
    }

    pub async fn get_product_by_id(&self, product_id: Uuid) -> Result<Option<ProductRow>> {
        sqlx::query_as::<_, ProductRow>(r#"SELECT product_id, name, description, product_code, product_category, regulatory_framework, min_asset_requirement, is_active, metadata, created_at, updated_at FROM "ob-poc".products WHERE product_id = $1"#)
            .bind(product_id).fetch_optional(&self.pool).await.context("Failed to get Product by ID")
    }

    pub async fn get_product_by_name(&self, name: &str) -> Result<Option<ProductRow>> {
        sqlx::query_as::<_, ProductRow>(r#"SELECT product_id, name, description, product_code, product_category, regulatory_framework, min_asset_requirement, is_active, metadata, created_at, updated_at FROM "ob-poc".products WHERE name = $1"#)
            .bind(name).fetch_optional(&self.pool).await.context("Failed to get Product by name")
    }

    pub async fn list_products(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<ProductRow>> {
        sqlx::query_as::<_, ProductRow>(r#"SELECT product_id, name, description, product_code, product_category, regulatory_framework, min_asset_requirement, is_active, metadata, created_at, updated_at FROM "ob-poc".products ORDER BY created_at DESC LIMIT $1 OFFSET $2"#)
            .bind(limit.unwrap_or(100)).bind(offset.unwrap_or(0)).fetch_all(&self.pool).await.context("Failed to list Products")
    }

    pub async fn update_product(
        &self,
        product_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(r#"UPDATE "ob-poc".products SET name = COALESCE($1, name), description = COALESCE($2, description), updated_at = NOW() WHERE product_id = $3"#)
            .bind(name).bind(description).bind(product_id).execute(&self.pool).await.context("Failed to update Product")?;
        if result.rows_affected() > 0 {
            info!("Updated Product {}", product_id);
        }
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_product(&self, product_id: Uuid) -> Result<bool> {
        let result = sqlx::query(r#"DELETE FROM "ob-poc".products WHERE product_id = $1"#)
            .bind(product_id)
            .execute(&self.pool)
            .await
            .context("Failed to delete Product")?;
        if result.rows_affected() > 0 {
            info!("Deleted Product {}", product_id);
        }
        Ok(result.rows_affected() > 0)
    }
}

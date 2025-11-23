//! Service Service - CRUD operations for Services

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceRow {
    pub service_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub service_code: Option<String>,
    pub service_category: Option<String>,
    pub sla_definition: Option<JsonValue>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewServiceFields {
    pub name: String,
    pub description: Option<String>,
    pub service_code: Option<String>,
    pub service_category: Option<String>,
    pub sla_definition: Option<JsonValue>,
    pub is_active: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct ServiceService {
    pool: PgPool,
}

impl ServiceService {
    pub fn new(pool: PgPool) -> Self { Self { pool } }
    pub fn pool(&self) -> &PgPool { &self.pool }

    pub async fn create_service(&self, fields: &NewServiceFields) -> Result<Uuid> {
        let service_id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO "ob-poc".services (service_id, name, description, service_code, service_category, sla_definition, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())"#)
            .bind(service_id).bind(&fields.name).bind(&fields.description).bind(&fields.service_code)
            .bind(&fields.service_category).bind(&fields.sla_definition).bind(fields.is_active.unwrap_or(true))
            .execute(&self.pool).await.context("Failed to create Service")?;
        info!("Created Service {} for '{}'", service_id, fields.name);
        Ok(service_id)
    }

    pub async fn get_service_by_id(&self, service_id: Uuid) -> Result<Option<ServiceRow>> {
        sqlx::query_as::<_, ServiceRow>(r#"SELECT service_id, name, description, service_code, service_category, sla_definition, is_active, created_at, updated_at FROM "ob-poc".services WHERE service_id = $1"#)
            .bind(service_id).fetch_optional(&self.pool).await.context("Failed to get Service by ID")
    }

    pub async fn get_service_by_name(&self, name: &str) -> Result<Option<ServiceRow>> {
        sqlx::query_as::<_, ServiceRow>(r#"SELECT service_id, name, description, service_code, service_category, sla_definition, is_active, created_at, updated_at FROM "ob-poc".services WHERE name = $1"#)
            .bind(name).fetch_optional(&self.pool).await.context("Failed to get Service by name")
    }

    pub async fn list_services(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<ServiceRow>> {
        sqlx::query_as::<_, ServiceRow>(r#"SELECT service_id, name, description, service_code, service_category, sla_definition, is_active, created_at, updated_at FROM "ob-poc".services ORDER BY created_at DESC LIMIT $1 OFFSET $2"#)
            .bind(limit.unwrap_or(100)).bind(offset.unwrap_or(0)).fetch_all(&self.pool).await.context("Failed to list Services")
    }

    pub async fn update_service(&self, service_id: Uuid, name: Option<&str>, description: Option<&str>) -> Result<bool> {
        let result = sqlx::query(r#"UPDATE "ob-poc".services SET name = COALESCE($1, name), description = COALESCE($2, description), updated_at = NOW() WHERE service_id = $3"#)
            .bind(name).bind(description).bind(service_id).execute(&self.pool).await.context("Failed to update Service")?;
        if result.rows_affected() > 0 { info!("Updated Service {}", service_id); }
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_service(&self, service_id: Uuid) -> Result<bool> {
        let result = sqlx::query(r#"DELETE FROM "ob-poc".services WHERE service_id = $1"#)
            .bind(service_id).execute(&self.pool).await.context("Failed to delete Service")?;
        if result.rows_affected() > 0 { info!("Deleted Service {}", service_id); }
        Ok(result.rows_affected() > 0)
    }

    pub async fn link_product(&self, service_id: Uuid, product_id: Uuid) -> Result<()> {
        sqlx::query(r#"INSERT INTO "ob-poc".product_services (product_id, service_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"#)
            .bind(product_id).bind(service_id).execute(&self.pool).await.context("Failed to link service to product")?;
        info!("Linked service {} to product {}", service_id, product_id);
        Ok(())
    }
}

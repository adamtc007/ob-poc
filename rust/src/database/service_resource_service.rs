//! Service Resource Service - CRUD operations for Service Resource Types
//! Note: Table renamed from prod_resources to service_resource_types via migration 017

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct ServiceResourceRow {
    pub resource_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner: String,
    pub dictionary_group: Option<String>,
    pub resource_code: Option<String>,
    pub resource_type: Option<String>,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub api_endpoint: Option<String>,
    pub api_version: Option<String>,
    pub authentication_method: Option<String>,
    pub authentication_config: Option<JsonValue>,
    pub capabilities: Option<JsonValue>,
    pub capacity_limits: Option<JsonValue>,
    pub is_active: Option<bool>,
    pub metadata: Option<JsonValue>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct NewServiceResourceFields {
    pub name: String,
    pub description: Option<String>,
    pub owner: String,
    pub dictionary_group: Option<String>,
    pub resource_code: Option<String>,
    pub resource_type: Option<String>,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub api_endpoint: Option<String>,
    pub api_version: Option<String>,
    pub authentication_method: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Clone, Debug)]
pub(crate) struct ServiceResourceService {
    pool: PgPool,
}

impl ServiceResourceService {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    #[allow(dead_code)]
    pub(crate) async fn create_service_resource(
        &self,
        fields: &NewServiceResourceFields,
    ) -> Result<Uuid> {
        let resource_id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO "ob-poc".service_resource_types (resource_id, name, description, owner, dictionary_group, resource_code, resource_type, vendor, version, api_endpoint, api_version, authentication_method, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW(), NOW())"#)
            .bind(resource_id).bind(&fields.name).bind(&fields.description).bind(&fields.owner)
            .bind(&fields.dictionary_group).bind(&fields.resource_code).bind(&fields.resource_type)
            .bind(&fields.vendor).bind(&fields.version).bind(&fields.api_endpoint).bind(&fields.api_version)
            .bind(&fields.authentication_method).bind(fields.is_active.unwrap_or(true))
            .execute(&self.pool).await.context("Failed to create Service Resource")?;
        info!(
            "Created Service Resource {} for '{}'",
            resource_id, fields.name
        );
        Ok(resource_id)
    }




    #[allow(dead_code)]
    pub(crate) async fn update_service_resource(
        &self,
        resource_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        owner: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(r#"UPDATE "ob-poc".service_resource_types SET name = COALESCE($1, name), description = COALESCE($2, description), owner = COALESCE($3, owner), updated_at = NOW() WHERE resource_id = $4"#)
            .bind(name).bind(description).bind(owner).bind(resource_id).execute(&self.pool).await.context("Failed to update Service Resource")?;
        if result.rows_affected() > 0 {
            info!("Updated Service Resource {}", resource_id);
        }
        Ok(result.rows_affected() > 0)
    }

    #[allow(dead_code)]
    pub(crate) async fn delete_service_resource(&self, resource_id: Uuid) -> Result<bool> {
        let result =
            sqlx::query(r#"DELETE FROM "ob-poc".service_resource_types WHERE resource_id = $1"#)
                .bind(resource_id)
                .execute(&self.pool)
                .await
                .context("Failed to delete Service Resource")?;
        if result.rows_affected() > 0 {
            info!("Deleted Service Resource {}", resource_id);
        }
        Ok(result.rows_affected() > 0)
    }

    #[allow(dead_code)]
    pub(crate) async fn link_service(&self, resource_id: Uuid, service_id: Uuid) -> Result<()> {
        sqlx::query(r#"INSERT INTO "ob-poc".service_resources (service_id, resource_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"#)
            .bind(service_id).bind(resource_id).execute(&self.pool).await.context("Failed to link service resource to service")?;
        info!(
            "Linked service resource {} to service {}",
            resource_id, service_id
        );
        Ok(())
    }
}

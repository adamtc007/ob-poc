//! CBU Service - CRUD operations for Client Business Units
//!
//! This module provides database operations for CBUs, entity roles,
//! and related structures following the CBU builder pattern.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Client Business Unit record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Cbu {
    pub cbu_id: Uuid,
    pub client_name: String,
    pub client_type: String,
    pub jurisdiction: Option<String>,
    pub status: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Role definition
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub role_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

/// CBU-Entity role assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuEntityRole {
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub role_id: Uuid,
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
    pub async fn create_cbu(
        &self,
        client_name: &str,
        client_type: &str,
        jurisdiction: Option<&str>,
    ) -> Result<Uuid> {
        let cbu_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (
                cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            )
            VALUES ($1, $2, $3, COALESCE($4, 'US'), 'ACTIVE', NOW(), NOW())
            "#,
        )
        .bind(cbu_id)
        .bind(client_name)
        .bind(client_type)
        .bind(jurisdiction)
        .execute(&self.pool)
        .await
        .context("Failed to create CBU")?;

        info!(
            "Created CBU {} for client '{}' (type: {})",
            cbu_id, client_name, client_type
        );

        Ok(cbu_id)
    }

    /// Get CBU by ID
    pub async fn get_cbu_by_id(&self, cbu_id: Uuid) -> Result<Option<Cbu>> {
        let result = sqlx::query_as::<_, Cbu>(
            r#"
            SELECT cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
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

    /// Get CBU by client name
    pub async fn get_cbu_by_name(&self, client_name: &str) -> Result<Option<Cbu>> {
        let result = sqlx::query_as::<_, Cbu>(
            r#"
            SELECT cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            FROM "ob-poc".cbus
            WHERE client_name = $1
            "#,
        )
        .bind(client_name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CBU by name")?;

        Ok(result)
    }

    /// List all CBUs
    pub async fn list_cbus(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<Cbu>> {
        let results = sqlx::query_as::<_, Cbu>(
            r#"
            SELECT cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
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
        client_name: Option<&str>,
        client_type: Option<&str>,
        jurisdiction: Option<&str>,
        status: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbus
            SET client_name = COALESCE($1, client_name),
                client_type = COALESCE($2, client_type),
                jurisdiction = COALESCE($3, jurisdiction),
                status = COALESCE($4, status),
                updated_at = NOW()
            WHERE cbu_id = $5
            "#,
        )
        .bind(client_name)
        .bind(client_type)
        .bind(jurisdiction)
        .bind(status)
        .bind(cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to update CBU")?;

        if result.rows_affected() > 0 {
            info!("Updated CBU {}", cbu_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete CBU (soft delete - sets status to DELETED)
    pub async fn delete_cbu(&self, cbu_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbus
            SET status = 'DELETED', updated_at = NOW()
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete CBU")?;

        if result.rows_affected() > 0 {
            info!("Soft deleted CBU {}", cbu_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Ensure a role exists, creating it if necessary
    pub async fn ensure_role(&self, name: &str, description: &str) -> Result<Uuid> {
        // Try to get existing role
        let existing = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT role_id FROM "ob-poc".roles WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check for existing role")?;

        if let Some(role_id) = existing {
            return Ok(role_id);
        }

        // Create new role
        let role_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".roles (role_id, name, description, created_at)
            VALUES ($1, $2, $3, NOW())
            "#,
        )
        .bind(role_id)
        .bind(name)
        .bind(description)
        .execute(&self.pool)
        .await
        .context("Failed to create role")?;

        info!("Created role '{}' with ID {}", name, role_id);
        Ok(role_id)
    }

    /// Attach an entity to a CBU with a specific role
    pub async fn attach_entity_to_cbu(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (cbu_id, entity_id, role_id)
            DO NOTHING
            "#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .execute(&self.pool)
        .await
        .context("Failed to attach entity to CBU")?;

        info!(
            "Attached entity {} to CBU {} with role {}",
            entity_id, cbu_id, role_id
        );

        Ok(())
    }

    /// Get all entities attached to a CBU
    pub async fn get_cbu_entities(&self, cbu_id: Uuid) -> Result<Vec<(Uuid, Uuid, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, String)>(
            r#"
            SELECT cer.entity_id, cer.role_id, r.name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU entities")?;

        Ok(rows)
    }

    /// Detach an entity from a CBU
    pub async fn detach_entity_from_cbu(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_id: Option<Uuid>,
    ) -> Result<bool> {
        let result = if let Some(rid) = role_id {
            sqlx::query(
                r#"
                DELETE FROM "ob-poc".cbu_entity_roles
                WHERE cbu_id = $1 AND entity_id = $2 AND role_id = $3
                "#,
            )
            .bind(cbu_id)
            .bind(entity_id)
            .bind(rid)
            .execute(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                DELETE FROM "ob-poc".cbu_entity_roles
                WHERE cbu_id = $1 AND entity_id = $2
                "#,
            )
            .bind(cbu_id)
            .bind(entity_id)
            .execute(&self.pool)
            .await
        }
        .context("Failed to detach entity from CBU")?;

        Ok(result.rows_affected() > 0)
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

    /// Upsert CBU (create or update)
    pub async fn upsert_cbu(
        &self,
        cbu_id: Option<Uuid>,
        client_name: &str,
        client_type: &str,
        jurisdiction: Option<&str>,
    ) -> Result<Uuid> {
        let id = cbu_id.unwrap_or_else(Uuid::new_v4);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (
                cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            )
            VALUES ($1, $2, $3, COALESCE($4, 'US'), 'ACTIVE', NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET
                client_name = $2,
                client_type = $3,
                jurisdiction = COALESCE($4, "ob-poc".cbus.jurisdiction),
                updated_at = NOW()
            "#,
        )
        .bind(id)
        .bind(client_name)
        .bind(client_type)
        .bind(jurisdiction)
        .execute(&self.pool)
        .await
        .context("Failed to upsert CBU")?;

        info!(
            "Upserted CBU {} for client '{}' (type: {})",
            id, client_name, client_type
        );

        Ok(id)
    }
}

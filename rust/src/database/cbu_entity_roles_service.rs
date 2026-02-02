//! CBU Entity Roles Service - Manages CBU-Entity-Role relationships
//!
//! This module provides database operations for the cbu_entity_roles table
//! which links CBUs to entities with specific roles.
//!
//! Canonical DB schema (DB is master per Section 3.3):
//! - cbu_entity_roles: cbu_entity_role_id, cbu_id (FK), entity_id (FK), role_id (FK)
//! - roles: role_id, name, description
//!
//! DSL uses string role names (e.g., "BeneficialOwner") which are resolved
//! to role_id via roles table.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// CBU Entity Role row - matches canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuEntityRoleRow {
    pub cbu_entity_role_id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub role_id: Uuid,
    pub created_at: Option<DateTime<Utc>>,
}

/// Role row - matches canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RoleRow {
    pub role_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

/// Expanded view of CBU entity role with resolved names
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuEntityRoleExpanded {
    pub cbu_entity_role_id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub role_id: Uuid,
    pub role_name: String,
}

/// Service for CBU entity roles operations
#[derive(Clone, Debug)]
pub struct CbuEntityRolesService {
    pool: PgPool,
}

impl CbuEntityRolesService {
    /// Create a new service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Resolve role name to role_id
    pub async fn resolve_role_id(&self, role_name: &str) -> Result<Uuid> {
        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT role_id
            FROM "ob-poc".roles
            WHERE name = $1
            "#,
        )
        .bind(role_name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query roles")?;

        result.ok_or_else(|| anyhow!("Role '{}' not found in roles table", role_name))
    }

    /// Ensure a role exists, creating it if necessary
    pub async fn ensure_role(&self, name: &str, description: Option<&str>) -> Result<Uuid> {
        // Try to get existing role
        if let Ok(role_id) = self.resolve_role_id(name).await {
            return Ok(role_id);
        }

        // Create new role
        let role_id = Uuid::now_v7();
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

    /// Attach an entity to a CBU with a role (using string role name)
    pub async fn attach_entity_to_cbu(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_name: &str,
    ) -> Result<Uuid> {
        let role_id = self.resolve_role_id(role_name).await?;
        self.attach_entity_to_cbu_by_id(cbu_id, entity_id, role_id)
            .await
    }

    /// Attach an entity to a CBU with a role (using role_id)
    pub async fn attach_entity_to_cbu_by_id(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_id: Uuid,
    ) -> Result<Uuid> {
        let cbu_entity_role_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (cbu_id, entity_id, role_id)
            DO NOTHING
            "#,
        )
        .bind(cbu_entity_role_id)
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

        Ok(cbu_entity_role_id)
    }

    /// Get all entities attached to a CBU with expanded info
    pub async fn get_entities_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<CbuEntityRoleExpanded>> {
        let rows: Vec<CbuEntityRoleExpanded> = sqlx::query_as(
            r#"
            SELECT cer.cbu_entity_role_id, cer.cbu_id, cer.entity_id, e.name as entity_name, cer.role_id, r.name as role_name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            ORDER BY r.name, e.name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entities for CBU")?;

        Ok(rows)
    }

    /// Get entities for a CBU with a specific role
    pub async fn get_entities_for_cbu_by_role(
        &self,
        cbu_id: Uuid,
        role_name: &str,
    ) -> Result<Vec<CbuEntityRoleExpanded>> {
        let role_id = self.resolve_role_id(role_name).await?;

        let rows: Vec<CbuEntityRoleExpanded> = sqlx::query_as(
            r#"
            SELECT cer.cbu_entity_role_id, cer.cbu_id, cer.entity_id, e.name as entity_name, cer.role_id, r.name as role_name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1 AND cer.role_id = $2
            ORDER BY e.name
            "#,
        )
        .bind(cbu_id)
        .bind(role_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entities for CBU by role")?;

        Ok(rows)
    }

    /// Detach an entity from a CBU (all roles)
    pub async fn detach_entity_from_cbu(&self, cbu_id: Uuid, entity_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".cbu_entity_roles
            WHERE cbu_id = $1 AND entity_id = $2
            "#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .execute(&self.pool)
        .await
        .context("Failed to detach entity from CBU")?;

        if result.rows_affected() > 0 {
            info!(
                "Detached entity {} from CBU {} ({} roles)",
                entity_id,
                cbu_id,
                result.rows_affected()
            );
        }

        Ok(result.rows_affected())
    }

    /// Detach an entity from a CBU for a specific role
    pub async fn detach_entity_from_cbu_role(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_name: &str,
    ) -> Result<bool> {
        let role_id = self.resolve_role_id(role_name).await?;

        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".cbu_entity_roles
            WHERE cbu_id = $1 AND entity_id = $2 AND role_id = $3
            "#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .execute(&self.pool)
        .await
        .context("Failed to detach entity from CBU role")?;

        Ok(result.rows_affected() > 0)
    }

    /// List all roles
    pub async fn list_roles(&self) -> Result<Vec<RoleRow>> {
        let rows = sqlx::query_as::<_, RoleRow>(
            r#"
            SELECT role_id, name, description
            FROM "ob-poc".roles
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list roles")?;

        Ok(rows)
    }
}

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
pub(crate) struct CbuEntityRoleRow {
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
pub(crate) struct CbuEntityRoleExpanded {
    pub cbu_entity_role_id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub role_id: Uuid,
    pub role_name: String,
}

/// Service for CBU entity roles operations
#[derive(Clone, Debug)]
pub(crate) struct CbuEntityRolesService {
    pool: PgPool,
}

impl CbuEntityRolesService {
    /// Create a new service
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Resolve role name to role_id
    pub(crate) async fn resolve_role_id(&self, role_name: &str) -> Result<Uuid> {
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


    /// Attach an entity to a CBU with a role (using string role name)
    pub(crate) async fn attach_entity_to_cbu(
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
    pub(crate) async fn attach_entity_to_cbu_by_id(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_id: Uuid,
    ) -> Result<Uuid> {
        let cbu_entity_role_id = Uuid::new_v4();

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
    pub(crate) async fn get_entities_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<CbuEntityRoleExpanded>> {
        let rows: Vec<CbuEntityRoleExpanded> = sqlx::query_as(
            r#"
            SELECT cer.cbu_entity_role_id, cer.cbu_id, cer.entity_id, e.name as entity_name, cer.role_id, r.name as role_name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".cbus c ON cer.cbu_id = c.cbu_id
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
              AND c.deleted_at IS NULL
              AND e.deleted_at IS NULL
            ORDER BY r.name, e.name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entities for CBU")?;

        Ok(rows)
    }


    /// Detach an entity from a CBU (all roles)
    pub(crate) async fn detach_entity_from_cbu(&self, cbu_id: Uuid, entity_id: Uuid) -> Result<u64> {
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


}

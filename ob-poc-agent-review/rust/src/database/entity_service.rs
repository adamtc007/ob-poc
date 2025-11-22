//! Entity Service - CRUD operations for Entities and Proper Persons
//!
//! This module provides database operations for the canonical normalized
//! entity model: entities, entity_types, proper_persons.
//!
//! Canonical DB schema (DB is master per Section 3.3):
//! - entities: entity_id, entity_type_id (FK), external_id, name
//! - entity_types: entity_type_id, code (e.g., "PROPER_PERSON", "COMPANY")
//! - proper_persons: proper_person_id, first_name, last_name, etc.
//!
//! DSL uses string type names (e.g., "PROPER_PERSON") which are resolved
//! to entity_type_id via entity_types table.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Entity row - matches canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityRow {
    pub entity_id: Uuid,
    pub entity_type_id: Uuid,
    pub external_id: Option<String>,
    pub name: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Proper person row - matches canonical DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProperPersonRow {
    pub proper_person_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub middle_names: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub residence_address: Option<String>,
    pub id_document_type: Option<String>,
    pub id_document_number: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Fields for creating a new entity
#[derive(Debug, Clone)]
pub struct NewEntityFields {
    pub entity_type: String, // e.g., "PROPER_PERSON", "COMPANY"
    pub name: String,
    pub external_id: Option<String>,
}

/// Fields for creating a proper person
#[derive(Debug, Clone)]
pub struct NewProperPersonFields {
    pub first_name: String,
    pub last_name: String,
    pub middle_names: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub residence_address: Option<String>,
    pub id_document_type: Option<String>,
    pub id_document_number: Option<String>,
}

/// Service for entity operations
#[derive(Clone, Debug)]
pub struct EntityService {
    pool: PgPool,
}

impl EntityService {
    /// Create a new entity service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Resolve entity type code to entity_type_id
    pub async fn resolve_entity_type_id(&self, type_code: &str) -> Result<Uuid> {
        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT entity_type_id
            FROM "ob-poc".entity_types
            WHERE code = $1 OR name = $1
            "#,
        )
        .bind(type_code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query entity_types")?;

        result.ok_or_else(|| {
            anyhow!(
                "Entity type '{}' not found in entity_types table",
                type_code
            )
        })
    }

    /// Create a new entity (generic)
    pub async fn create_entity(&self, fields: &NewEntityFields) -> Result<Uuid> {
        let entity_id = Uuid::new_v4();
        let entity_type_id = self.resolve_entity_type_id(&fields.entity_type).await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, entity_type_id, external_id, name, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(&fields.external_id)
        .bind(&fields.name)
        .execute(&self.pool)
        .await
        .context("Failed to create entity")?;

        info!(
            "Created entity {} of type '{}' with name '{}'",
            entity_id, fields.entity_type, fields.name
        );

        Ok(entity_id)
    }

    /// Create a proper person (entity + proper_persons record)
    /// Returns (entity_id, proper_person_id)
    pub async fn create_proper_person(
        &self,
        person_fields: &NewProperPersonFields,
    ) -> Result<(Uuid, Uuid)> {
        let entity_id = Uuid::new_v4();
        let proper_person_id = entity_id; // Use same UUID for simplicity
        let entity_type_id = self.resolve_entity_type_id("PROPER_PERSON").await?;

        // Full name for entities table
        let full_name = if let Some(middle) = &person_fields.middle_names {
            format!(
                "{} {} {}",
                person_fields.first_name, middle, person_fields.last_name
            )
        } else {
            format!("{} {}", person_fields.first_name, person_fields.last_name)
        };

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Insert into entities
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(&full_name)
        .execute(&mut *tx)
        .await
        .context("Failed to create entity for proper person")?;

        // Insert into proper_persons
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_proper_persons (
                proper_person_id, first_name, last_name, middle_names,
                date_of_birth, nationality, residence_address,
                id_document_type, id_document_number, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())
            "#,
        )
        .bind(proper_person_id)
        .bind(&person_fields.first_name)
        .bind(&person_fields.last_name)
        .bind(&person_fields.middle_names)
        .bind(person_fields.date_of_birth)
        .bind(&person_fields.nationality)
        .bind(&person_fields.residence_address)
        .bind(&person_fields.id_document_type)
        .bind(&person_fields.id_document_number)
        .execute(&mut *tx)
        .await
        .context("Failed to create proper person record")?;

        tx.commit().await?;

        info!(
            "Created proper person {} ({} {})",
            proper_person_id, person_fields.first_name, person_fields.last_name
        );

        Ok((entity_id, proper_person_id))
    }

    /// Get entity by ID
    pub async fn get_entity_by_id(&self, entity_id: Uuid) -> Result<Option<EntityRow>> {
        let result = sqlx::query_as::<_, EntityRow>(
            r#"
            SELECT entity_id, entity_type_id, external_id, name, created_at, updated_at
            FROM "ob-poc".entities
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get entity by ID")?;

        Ok(result)
    }

    /// Get entity by name
    pub async fn get_entity_by_name(&self, name: &str) -> Result<Option<EntityRow>> {
        let result = sqlx::query_as::<_, EntityRow>(
            r#"
            SELECT entity_id, entity_type_id, external_id, name, created_at, updated_at
            FROM "ob-poc".entities
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get entity by name")?;

        Ok(result)
    }

    /// Get proper person details by entity_id
    pub async fn get_proper_person(&self, entity_id: Uuid) -> Result<Option<ProperPersonRow>> {
        let result = sqlx::query_as::<_, ProperPersonRow>(
            r#"
            SELECT proper_person_id, first_name, last_name, middle_names,
                   date_of_birth, nationality, residence_address,
                   id_document_type, id_document_number, created_at, updated_at
            FROM "ob-poc".entity_proper_persons
            WHERE proper_person_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get proper person")?;

        Ok(result)
    }

    /// Update entity
    pub async fn update_entity(
        &self,
        entity_id: Uuid,
        name: Option<&str>,
        external_id: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".entities
            SET name = COALESCE($1, name),
                external_id = COALESCE($2, external_id),
                updated_at = NOW()
            WHERE entity_id = $3
            "#,
        )
        .bind(name)
        .bind(external_id)
        .bind(entity_id)
        .execute(&self.pool)
        .await
        .context("Failed to update entity")?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete entity (cascades to proper_persons if applicable)
    pub async fn delete_entity(&self, entity_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".entities
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete entity")?;

        if result.rows_affected() > 0 {
            info!("Deleted entity {}", entity_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// List entities by type
    pub async fn list_entities_by_type(
        &self,
        entity_type: &str,
        limit: Option<i32>,
    ) -> Result<Vec<EntityRow>> {
        let entity_type_id = self.resolve_entity_type_id(entity_type).await?;

        let results = sqlx::query_as::<_, EntityRow>(
            r#"
            SELECT entity_id, entity_type_id, external_id, name, created_at, updated_at
            FROM "ob-poc".entities
            WHERE entity_type_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(entity_type_id)
        .bind(limit.unwrap_or(100))
        .fetch_all(&self.pool)
        .await
        .context("Failed to list entities by type")?;

        Ok(results)
    }
}
